use crate::ast::*;
use crate::error;
use crate::token::{Token, TokenKind};
use std::sync::Arc;

struct ParsedExportSpecifier {
    local_name: Arc<str>,
    export_name: Arc<str>,
    local_name_is_string: bool,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    last_arrow_params: Option<Vec<Arc<str>>>,
    /// Parameter defaults collected by the most recent `parse_params` / arrow parse.
    cur_param_defaults: Vec<Option<Expr>>,
    /// Rest parameter name from the most recent `parse_params` / arrow parse.
    cur_rest_param: Option<Arc<str>>,
    /// Destructuring parameters from the most recent `parse_params`: each is
    /// (pattern, temp-name) to be bound from the positional temp arg in the
    /// body prelude.
    cur_param_destructure_decls: Vec<(Pattern, String, Option<Expr>)>,
    /// Arrow-specific defaults/rest (carried alongside `last_arrow_params`).
    arrow_defaults: Vec<Option<Expr>>,
    arrow_rest: Option<Arc<str>>,
    /// Arrow destructuring params: each entry is (pattern, temp-name) where the
    /// temp-name is the synthesized positional parameter that receives the
    /// argument; the body is rewritten to bind the pattern from that temp.
    arrow_destructure_decls: Vec<(Pattern, String, Option<Expr>)>,
    /// Whether the current parse context is strict (inherited from an
    /// enclosing strict function/program). Drives directive inheritance.
    is_strict_context: bool,
    /// Token-based result from the most recently parsed function body.
    last_fn_body_use_strict_directive: bool,
    /// When true, `in` is not treated as a binary operator (for-in head parsing).
    no_in: bool,
    /// Source line of the first token of the statement currently being parsed
    /// (captured at `parse_stmt` entry). Used by `stmt()` so a statement's line
    /// reflects where it begins, not where its construction helper finishes.
    stmt_start_line: u32,
    /// Current nesting depth of expressions (parens, arrays, objects,
    /// ternaries, etc.). Capped to keep untrusted deeply-nested input from
    /// overflowing the Rust parser stack and aborting the process.
    expr_depth: usize,
    /// Current nesting depth of statements (blocks, if/else, while, for,
    /// do-while, with, switch bodies). Capped for the same reason as
    /// `expr_depth`: deeply nested `{{...}}` / `if(1) if(1) ...` would
    /// otherwise overflow the Rust parser stack on untrusted input.
    stmt_depth: usize,
    /// Object literals are parsed as cover grammar before assignment parsing
    /// decides whether they are expressions or assignment patterns. While
    /// parsing a property value in that cover position, defer duplicate
    /// `__proto__` early errors until the surrounding expression is known not
    /// to be an assignment pattern.
    defer_object_proto_duplicate_check: usize,
    /// Nesting depth of iteration statements (while, do-while, for, for-in,
    /// for-of). `break` (unlabelled) is valid inside loops or switch;
    /// `continue` is valid only inside loops.
    loop_depth: usize,
    /// Nesting depth of switch statements. `break` (unlabelled) is valid
    /// inside switch even without an enclosing loop.
    switch_depth: usize,
    /// Stack of labels visible in the current scope: (label, is_loop).
    /// Used to validate `break label` / `continue label` targets.
    label_stack: Vec<(Arc<str>, bool)>,
    /// Current function nesting depth. 0 = top-level program code.
    /// `return` at depth 0 is a SyntaxError.
    function_depth: usize,
    /// Whether the current syntactic context permits `new.target`.
    /// Ordinary function/method bodies enable it; arrow functions inherit it
    /// from their enclosing context; direct eval enables it only when the eval
    /// call is contained in non-arrow function code.
    new_target_allowed: bool,
    /// Current generator-function parsing depth. `yield` is an expression
    /// keyword only inside generator parameter/body contexts; outside strict
    /// mode non-generator code it remains an ordinary identifier.
    generator_depth: usize,
    /// YieldExpression is disabled while parsing generator formal parameters.
    /// Nested generator bodies in parameter initializers re-enable it.
    yield_expression_allowed: bool,
    /// Current async-function parsing depth. `await` is an expression keyword
    /// inside async parameter/body contexts; in sloppy non-async contexts it
    /// remains available as a contextual identifier.
    async_depth: usize,
    /// AwaitExpression is disabled while parsing async formal parameters.
    /// Nested async function bodies in parameter initializers re-enable it.
    await_expression_allowed: bool,
    /// Whether the current statement position accepts LexicalDeclaration as a
    /// StatementListItem. Single-statement bodies (`if (x) stmt`, labels,
    /// `with (x) stmt`, etc.) parse `let` with ExpressionStatement lookahead
    /// rules instead.
    lexical_declaration_allowed: bool,
    super_depth: usize,
    super_call_depth: usize,
    last_primary_parenthesized: bool,
    source_type: SourceType,
    nested_function_context_depth: usize,
}

type ParsedParams = (
    Vec<Arc<str>>,
    Vec<Option<Expr>>,
    Option<Arc<str>>,
    Vec<(Pattern, String, Option<Expr>)>,
);

#[derive(Debug)]
struct PrivateBoundName {
    name: Arc<str>,
    is_static: bool,
    getter: bool,
    setter: bool,
    other: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            last_arrow_params: None,
            cur_param_defaults: Vec::new(),
            cur_rest_param: None,
            cur_param_destructure_decls: Vec::new(),
            arrow_defaults: Vec::new(),
            arrow_rest: None,
            arrow_destructure_decls: Vec::new(),
            is_strict_context: false,
            last_fn_body_use_strict_directive: false,
            no_in: false,
            stmt_start_line: 0,
            expr_depth: 0,
            stmt_depth: 0,
            defer_object_proto_duplicate_check: 0,
            loop_depth: 0,
            switch_depth: 0,
            label_stack: Vec::new(),
            function_depth: 0,
            new_target_allowed: false,
            generator_depth: 0,
            yield_expression_allowed: true,
            async_depth: 0,
            await_expression_allowed: true,
            lexical_declaration_allowed: true,
            super_depth: 0,
            super_call_depth: 0,
            last_primary_parenthesized: false,
            source_type: SourceType::Script,
            nested_function_context_depth: 0,
        }
    }

    /// Parse well-formed host Unicode Script source. Already-canonical
    /// JavaScript strings must use an internal-source parser entry point.
    pub fn parse(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        p.parse_program()
    }

    pub(crate) fn parse_internal(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new_internal(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        p.parse_program()
    }

    /// Parse well-formed host Unicode Module source.
    pub fn parse_module(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        p.source_type = SourceType::Module;
        p.is_strict_context = true;
        p.parse_program()
    }

    /// Parse well-formed host Unicode source with an inherited strict-mode flag. Direct eval uses
    /// `parse_direct_eval_inherited` because its input is already an internal
    /// JavaScript string.
    pub fn parse_strict_inherited(src: &str, inherited: bool) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        if inherited {
            p.is_strict_context = true;
        }
        p.parse_program()
    }

    pub(crate) fn parse_direct_eval_inherited(
        src: &str,
        inherited_strict: bool,
        super_allowed: bool,
        super_call_allowed: bool,
        new_target_allowed: bool,
        inherited_private_names: &[Arc<str>],
    ) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new_internal(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        if inherited_strict {
            p.is_strict_context = true;
        }
        if super_allowed {
            p.super_depth = 1;
        }
        if super_call_allowed {
            p.super_call_depth = 1;
        }
        p.new_target_allowed = new_target_allowed;
        p.parse_program_with_private_names(inherited_private_names)
    }

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }
    /// Source line (1-based) of the current token.
    fn current_line(&self) -> u32 {
        self.tokens[self.pos].line as u32
    }
    /// Wrap a `StmtNode` with the current token's source line.
    fn stmt(&self, node: crate::ast::StmtNode) -> crate::ast::Stmt {
        crate::ast::Stmt {
            line: self.stmt_start_line,
            node,
        }
    }
    fn peek_at_tok(&self, off: usize) -> &Token {
        &self.tokens[(self.pos + off).min(self.tokens.len() - 1)]
    }
    fn at_newline_before(&self) -> bool {
        self.tokens[self.pos].preceded_by_newline
    }

    fn advance(&mut self) -> TokenKind {
        let k = self.tokens[self.pos].kind.clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        k
    }

    fn check(&self, k: &TokenKind) -> bool {
        std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(k)
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.check(k) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, k: &TokenKind, what: &str) -> error::Result<()> {
        if self.check(k) {
            self.advance();
            Ok(())
        } else {
            Err(error::Error::syntax(format!(
                "Expected {}, got {:?}",
                what,
                self.peek()
            )))
        }
    }

    /// ES spec: `enum` is always reserved, while the words below are reserved
    /// only in strict BindingIdentifier/IdentifierReference positions.
    fn is_future_reserved(name: &str) -> bool {
        matches!(name, "enum")
    }

    fn is_strict_only_reserved(name: &str) -> bool {
        matches!(
            name,
            "implements"
                | "interface"
                | "let"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "static"
                | "yield"
        )
    }

    fn is_strict_identifier_reference_reserved(name: &str) -> bool {
        Self::is_future_reserved(name) || Self::is_strict_only_reserved(name)
    }

    fn is_reserved_identifier_reference_word(name: &str) -> bool {
        matches!(
            name,
            "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "export"
                | "extends"
                | "false"
                | "finally"
                | "for"
                | "function"
                | "if"
                | "import"
                | "in"
                | "instanceof"
                | "new"
                | "null"
                | "return"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typeof"
                | "var"
                | "void"
                | "while"
                | "with"
        )
    }

    /// Check that an identifier name is a valid binding name (not a
    /// FutureReservedWord). Returns the name on success, SyntaxError on failure.
    fn check_binding_name(&self, name: &str) -> error::Result<()> {
        if Self::is_reserved_identifier_reference_word(name) {
            return Err(error::Error::syntax(format!(
                "'{}' is a reserved word and cannot be used as a binding name",
                name
            )));
        }
        if Self::is_future_reserved(name) {
            return Err(error::Error::syntax(format!(
                "'{}' is a reserved word and cannot be used as a binding name",
                name
            )));
        }
        if matches!(name, "import" | "export") {
            return Err(error::Error::syntax(format!(
                "'{}' is a reserved word and cannot be used as a binding name",
                name
            )));
        }
        if self.is_strict_context && Self::is_strict_only_reserved(name) {
            return Err(error::Error::syntax(format!(
                "'{}' is a reserved word and cannot be used as a binding name",
                name
            )));
        }
        if self.is_strict_context && matches!(name, "eval" | "arguments") {
            return Err(error::Error::syntax(format!(
                "'{}' cannot be used as a binding name in strict mode",
                name
            )));
        }
        if name == "yield" && !self.yield_as_identifier_allowed() {
            return Err(error::Error::syntax(
                "'yield' cannot be used as a binding name here".to_string(),
            ));
        }
        if name == "await" && !self.await_as_identifier_allowed() {
            return Err(error::Error::syntax(
                "'await' cannot be used as a binding name here".to_string(),
            ));
        }
        Ok(())
    }

    /// Return the source name when the current token can participate in the
    /// BindingIdentifier grammar. The lexer promotes several contextual names
    /// to dedicated tokens, so formal parameters and patterns must not assume
    /// every valid binding arrives as `TokenKind::Ident`.
    fn binding_identifier_name(&self) -> Option<Arc<str>> {
        self.binding_identifier_name_at(0)
    }

    fn binding_identifier_name_at(&self, offset: usize) -> Option<Arc<str>> {
        let name = match &self.peek_at_tok(offset).kind {
            TokenKind::Ident(name) => return Some(Arc::from(name.as_str())),
            TokenKind::Undefined => "undefined",
            TokenKind::Of => "of",
            TokenKind::Async => "async",
            TokenKind::Get => "get",
            TokenKind::Set => "set",
            TokenKind::Static => "static",
            TokenKind::Let => "let",
            TokenKind::Await if self.await_as_identifier_allowed() => "await",
            TokenKind::Yield if self.yield_as_identifier_allowed() => "yield",
            _ => return None,
        };
        Some(Arc::from(name))
    }

    fn parse_binding_identifier(&mut self, expected: &str) -> error::Result<Arc<str>> {
        let name = self
            .binding_identifier_name()
            .ok_or_else(|| error::Error::syntax(expected.to_string()))?;
        self.check_binding_name(&name)?;
        self.advance();
        Ok(name)
    }

    fn yield_as_identifier_allowed(&self) -> bool {
        !self.is_strict_context && self.generator_depth == 0
    }

    fn await_as_identifier_allowed(&self) -> bool {
        self.async_depth == 0
            && (self.source_type != SourceType::Module || self.nested_function_context_depth > 0)
    }

    fn with_generator_context<T>(
        &mut self,
        enabled: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved_generator_depth = self.generator_depth;
        self.generator_depth = if enabled { 1 } else { 0 };
        let result = f(self);
        self.generator_depth = saved_generator_depth;
        result
    }

    fn with_yield_expression_context<T>(
        &mut self,
        allowed: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved = self.yield_expression_allowed;
        self.yield_expression_allowed = allowed;
        let result = f(self);
        self.yield_expression_allowed = saved;
        result
    }

    fn with_async_context<T>(
        &mut self,
        enabled: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved_async_depth = self.async_depth;
        self.async_depth = if enabled { 1 } else { 0 };
        let result = f(self);
        self.async_depth = saved_async_depth;
        result
    }

    fn with_await_expression_context<T>(
        &mut self,
        allowed: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved = self.await_expression_allowed;
        self.await_expression_allowed = allowed;
        let result = f(self);
        self.await_expression_allowed = saved;
        result
    }

    fn with_function_context<T>(
        &mut self,
        generator_enabled: bool,
        async_enabled: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        self.nested_function_context_depth += 1;
        let result = self.with_generator_context(generator_enabled, |p| {
            p.with_async_context(async_enabled, f)
        });
        self.nested_function_context_depth -= 1;
        result
    }

    fn with_formal_parameter_expression_context<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        self.with_yield_expression_context(false, |p| p.with_await_expression_context(false, f))
    }

    fn with_function_body_context<T>(
        &mut self,
        generator_enabled: bool,
        async_enabled: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        self.with_function_context(generator_enabled, async_enabled, |p| {
            p.with_yield_expression_context(true, |p| p.with_await_expression_context(true, f))
        })
    }

    fn parse_params_scoped(
        &mut self,
        generator_enabled: bool,
        async_enabled: bool,
        super_property_allowed: bool,
    ) -> error::Result<ParsedParams> {
        let saved_defaults = std::mem::take(&mut self.cur_param_defaults);
        let saved_rest = self.cur_rest_param.take();
        let saved_dstr = std::mem::take(&mut self.cur_param_destructure_decls);
        let saved_super = self.super_depth;
        let saved_super_call = self.super_call_depth;
        let saved_new_target_allowed = self.new_target_allowed;
        self.super_depth = if super_property_allowed { 1 } else { 0 };
        self.super_call_depth = 0;
        self.new_target_allowed = true;
        let params = match self.with_function_context(generator_enabled, async_enabled, |p| {
            p.with_formal_parameter_expression_context(|p| p.parse_params())
        }) {
            Ok(params) => params,
            Err(err) => {
                self.cur_param_defaults = saved_defaults;
                self.cur_rest_param = saved_rest;
                self.cur_param_destructure_decls = saved_dstr;
                self.super_depth = saved_super;
                self.super_call_depth = saved_super_call;
                self.new_target_allowed = saved_new_target_allowed;
                return Err(err);
            }
        };
        let param_defaults = std::mem::take(&mut self.cur_param_defaults);
        let rest_param = self.cur_rest_param.take();
        let dstr_decls = std::mem::take(&mut self.cur_param_destructure_decls);
        self.cur_param_defaults = saved_defaults;
        self.cur_rest_param = saved_rest;
        self.cur_param_destructure_decls = saved_dstr;
        self.super_depth = saved_super;
        self.super_call_depth = saved_super_call;
        self.new_target_allowed = saved_new_target_allowed;
        Ok((params, param_defaults, rest_param, dstr_decls))
    }

    fn with_lexical_declaration_context<T>(
        &mut self,
        allowed: bool,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved = self.lexical_declaration_allowed;
        self.lexical_declaration_allowed = allowed;
        let result = f(self);
        self.lexical_declaration_allowed = saved;
        result
    }

    fn with_function_statement_control_context<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved_loop_depth = self.loop_depth;
        let saved_switch_depth = self.switch_depth;
        let saved_label_stack = std::mem::take(&mut self.label_stack);
        self.loop_depth = 0;
        self.switch_depth = 0;
        let result = f(self);
        self.loop_depth = saved_loop_depth;
        self.switch_depth = saved_switch_depth;
        self.label_stack = saved_label_stack;
        result
    }

    /// Determine if `let` at the current position is a lexical declaration
    /// (as opposed to an identifier). Per spec, `let` is a declaration when
    /// followed by `[`, `{`, or an identifier name. ASI does not apply between
    /// `let` and a following binding name just because a line terminator
    /// appears there; the lexical declaration is selected first and then
    /// static semantics may reject the binding name.
    fn is_let_lexical_position(&self) -> bool {
        if self.peek_at_tok(0).had_escape {
            return false;
        }
        // In strict mode, `let` is always a lexical declaration.
        if self.is_strict_context {
            return true;
        }
        // Non-strict: `let` is lexical only when followed by `[`, `{`, or an
        // identifier-name token (the start of a binding pattern or name).
        match self.peek_at_tok(1).kind {
            TokenKind::LBracket | TokenKind::LBrace => true,
            TokenKind::Ident(_)
            | TokenKind::Async
            | TokenKind::Await
            | TokenKind::Yield
            | TokenKind::Let
            | TokenKind::Static
            | TokenKind::Of => true,
            _ => false,
        }
    }

    fn await_token_is_identifier_ref(&self) -> bool {
        matches!(
            self.peek_at_tok(1).kind,
            TokenKind::Assign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::StarAssign
                | TokenKind::SlashAssign
                | TokenKind::PercentAssign
                | TokenKind::StarStarAssign
                | TokenKind::AmpAssign
                | TokenKind::PipeAssign
                | TokenKind::CaretAssign
                | TokenKind::ShlAssign
                | TokenKind::ShrAssign
                | TokenKind::UshrAssign
                | TokenKind::AndAssign
                | TokenKind::OrAssign
                | TokenKind::NullishAssign
                | TokenKind::Eq
                | TokenKind::NotEq
                | TokenKind::EqEqEq
                | TokenKind::NotEqEqEq
                | TokenKind::Lt
                | TokenKind::Gt
                | TokenKind::Lte
                | TokenKind::Gte
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::StarStar
                | TokenKind::BitAnd
                | TokenKind::BitOr
                | TokenKind::BitXor
                | TokenKind::Shl
                | TokenKind::Shr
                | TokenKind::Ushr
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::Nullish
                | TokenKind::Inc
                | TokenKind::Dec
                | TokenKind::Dot
                | TokenKind::QuestionDot
                | TokenKind::LBracket
                | TokenKind::LParen
                | TokenKind::Comma
                | TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Colon
                | TokenKind::Question
                | TokenKind::Instanceof
                | TokenKind::In
                | TokenKind::Arrow
                | TokenKind::Eof
        )
    }

    fn expect_semi(&mut self) -> error::Result<()> {
        // ASI: semicolon optional before } or EOF or after newline
        if self.check(&TokenKind::Semicolon) {
            self.advance();
            return Ok(());
        }
        if self.check(&TokenKind::RBrace) || self.check(&TokenKind::Eof) {
            return Ok(());
        }
        if self.at_newline_before() {
            return Ok(());
        }
        Err(error::Error::syntax(format!(
            "Expected ; got {:?}",
            self.peek()
        )))
    }

    fn parse_program(&mut self) -> error::Result<Program> {
        self.parse_program_with_private_names(&[])
    }

    fn parse_program_with_private_names(
        &mut self,
        inherited_private_names: &[Arc<str>],
    ) -> error::Result<Program> {
        // Surface any lexer-level error (e.g. an invalid escape
        // sequence in a string literal) as a SyntaxError before parsing.
        for t in &self.tokens {
            if let TokenKind::LexError(msg) = &t.kind {
                return Err(error::Error::syntax(msg.clone()));
            }
            if let TokenKind::ResourceError(msg) = &t.kind {
                return Err(error::Error::range(msg.clone()));
            }
        }
        // Detect a leading "use strict" directive from the raw token stream
        // *before* parsing the body, so that nested function declarations
        // parsed within the body inherit strictness. A directive prologue is
        // a run of string-literal expression statements; escaped string
        // literals can be part of the prologue, but cannot be Use Strict
        // Directives.
        let has_strict_directive = self.peek_use_strict_directive();
        self.is_strict_context = has_strict_directive || self.is_strict_context;
        let mut body = Vec::new();
        let mut module_requests = Vec::new();
        let mut import_entries = Vec::new();
        let mut export_entries = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if self.source_type == SourceType::Module
                && self.is_unescaped_ident("import")
                && !matches!(self.peek_at_tok(1).kind, TokenKind::LParen | TokenKind::Dot)
            {
                self.parse_import_declaration(&mut module_requests, &mut import_entries)?;
            } else if self.source_type == SourceType::Module && self.check(&TokenKind::At) {
                let decorators = self.parse_decorator_list()?;
                if !self.is_unescaped_ident("export") {
                    if !self.check(&TokenKind::Class) {
                        return Err(error::Error::syntax(
                            "Class decorators must be followed by a class or export declaration"
                                .to_string(),
                        ));
                    }
                    let class = self.parse_class_body_with_decorators(true, decorators)?;
                    body.push(self.stmt(StmtNode::ExprStmt(Expr::Class(class))));
                    continue;
                }
                if let Some(stmt) = self.parse_export_declaration(
                    &mut module_requests,
                    &mut export_entries,
                    decorators,
                )? {
                    body.push(stmt);
                }
            } else if self.source_type == SourceType::Module && self.is_unescaped_ident("export") {
                if let Some(stmt) = self.parse_export_declaration(
                    &mut module_requests,
                    &mut export_entries,
                    Vec::new(),
                )? {
                    body.push(stmt);
                }
            } else {
                body.push(self.parse_stmt()?);
            }
        }
        let statement_list_scope = if self.source_type == SourceType::Module {
            StatementListScope::Module
        } else {
            StatementListScope::Script
        };
        check_statement_list_declaration_early_errors(&body, statement_list_scope)?;
        Self::check_module_early_errors(&body, &import_entries, &export_entries)?;
        Self::normalize_import_reexports(&import_entries, &mut export_entries);
        Self::validate_private_names_statement_list(&body, inherited_private_names)?;
        Ok(Program {
            body,
            is_strict: self.is_strict_context,
            source_type: self.source_type,
            module_requests,
            import_entries,
            export_entries,
        })
    }

    fn is_unescaped_ident(&self, word: &str) -> bool {
        matches!(&self.peek_at_tok(0).kind, TokenKind::Ident(name) if name == word)
            && !self.peek_at_tok(0).had_escape
    }

    fn eat_contextual(&mut self, word: &str) -> bool {
        if self.is_unescaped_ident(word) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_contextual(&mut self, word: &str) -> error::Result<()> {
        if self.eat_contextual(word) {
            Ok(())
        } else {
            Err(error::Error::syntax(format!(
                "Expected '{}', got {:?}",
                word,
                self.peek()
            )))
        }
    }

    /// Parse IdentifierName for module import/export names. Unlike binding
    /// names, keywords such as `default` are permitted here.
    fn parse_module_identifier_name(&mut self) -> error::Result<Arc<str>> {
        let token = self.peek().clone();
        let name = match token {
            TokenKind::Ident(name) => name,
            other => other.as_keyword_str().map(str::to_owned).ok_or_else(|| {
                error::Error::syntax(format!("Expected module identifier name, got {:?}", other))
            })?,
        };
        self.advance();
        Ok(Arc::from(name.as_str()))
    }

    fn parse_module_export_name(&mut self) -> error::Result<(Arc<str>, bool)> {
        if let TokenKind::String(name) = self.peek().clone() {
            if self.peek_at_tok(0).string_had_legacy_escape {
                return Err(error::Error::syntax(
                    "Legacy string escapes are not allowed in module export names".to_string(),
                ));
            }
            if self.peek_at_tok(0).string_not_well_formed {
                return Err(error::Error::syntax(
                    "Module export names must be well-formed Unicode strings".to_string(),
                ));
            }
            self.advance();
            return Ok((Arc::from(name.as_str()), true));
        }
        self.parse_module_identifier_name()
            .map(|name| (name, false))
    }

    fn parse_module_binding_name(&mut self) -> error::Result<Arc<str>> {
        let name = match self.peek().clone() {
            TokenKind::Ident(name) => Arc::from(name.as_str()),
            TokenKind::Undefined => Arc::from("undefined"),
            TokenKind::Of => Arc::from("of"),
            TokenKind::Async => Arc::from("async"),
            TokenKind::Await if self.await_as_identifier_allowed() => Arc::from("await"),
            TokenKind::Yield if self.yield_as_identifier_allowed() => Arc::from("yield"),
            other => {
                return Err(error::Error::syntax(format!(
                    "Expected binding identifier, got {:?}",
                    other
                )))
            }
        };
        self.advance();
        self.check_binding_name(&name)?;
        Ok(name)
    }

    fn parse_module_specifier(&mut self) -> error::Result<ModuleRequest> {
        let specifier = self.parse_module_string_literal("module specifier")?;
        let attributes = self.parse_import_attributes()?;
        Ok(ModuleRequest {
            specifier,
            attributes,
        })
    }

    fn parse_module_string_literal(&mut self, context: &str) -> error::Result<Arc<str>> {
        let token = self.peek_at_tok(0).clone();
        let TokenKind::String(value) = token.kind else {
            return Err(error::Error::syntax(format!(
                "Expected {context} string, got {:?}",
                self.peek()
            )));
        };
        if token.string_had_legacy_escape {
            return Err(error::Error::syntax(format!(
                "Legacy string escapes are not allowed in {context}"
            )));
        }
        self.advance();
        Ok(Arc::from(value.as_str()))
    }

    fn at_import_attributes_with(&self) -> bool {
        let token = self.peek_at_tok(0);
        !token.had_escape
            && (matches!(&token.kind, TokenKind::Ident(name) if name == "with")
                || token.kind.as_keyword_str() == Some("with"))
    }

    fn parse_import_attributes(&mut self) -> error::Result<Vec<ImportAttribute>> {
        if !self.at_import_attributes_with() {
            return Ok(Vec::new());
        }
        self.advance();
        self.expect(&TokenKind::LBrace, "{")?;
        let mut attributes = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let key = if matches!(self.peek(), TokenKind::String(_)) {
                self.parse_module_string_literal("import attribute key")?
            } else {
                self.parse_module_identifier_name()?
            };
            self.expect(&TokenKind::Colon, ":")?;
            let value = self.parse_module_string_literal("import attribute value")?;
            if attributes
                .iter()
                .any(|attribute: &ImportAttribute| attribute.key == key)
            {
                return Err(error::Error::syntax(format!(
                    "Duplicate import attribute '{}'",
                    key
                )));
            }
            attributes.push(ImportAttribute { key, value });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "}")?;
        attributes.sort_by(|left, right| {
            crate::value::utf16_from_str(&left.key).cmp(&crate::value::utf16_from_str(&right.key))
        });
        Ok(attributes)
    }

    fn push_module_request(requests: &mut Vec<ModuleRequest>, request: ModuleRequest) {
        if !requests.contains(&request) {
            requests.push(request);
        }
    }

    fn parse_import_declaration(
        &mut self,
        module_requests: &mut Vec<ModuleRequest>,
        import_entries: &mut Vec<ImportEntry>,
    ) -> error::Result<()> {
        self.stmt_start_line = self.current_line();
        self.advance(); // import
        if matches!(self.peek(), TokenKind::String(_)) {
            let request = self.parse_module_specifier()?;
            self.expect_semi()?;
            Self::push_module_request(module_requests, request);
            return Ok(());
        }

        let mut entries = Vec::new();
        let mut parse_named = false;
        if self.eat(&TokenKind::Star) {
            self.expect_contextual("as")?;
            entries.push((Arc::from("*"), self.parse_module_binding_name()?));
        } else if self.check(&TokenKind::LBrace) {
            parse_named = true;
        } else {
            let local_name = self.parse_module_binding_name()?;
            entries.push((Arc::from("default"), local_name));
            if self.eat(&TokenKind::Comma) {
                if self.eat(&TokenKind::Star) {
                    self.expect_contextual("as")?;
                    entries.push((Arc::from("*"), self.parse_module_binding_name()?));
                } else {
                    parse_named = true;
                }
            }
        }
        if parse_named {
            self.expect(&TokenKind::LBrace, "{")?;
            while !self.check(&TokenKind::RBrace) {
                let (import_name, import_name_is_string) = self.parse_module_export_name()?;
                let local_name = if self.eat_contextual("as") {
                    self.parse_module_binding_name()?
                } else {
                    if import_name_is_string {
                        return Err(error::Error::syntax(
                            "A string import name requires an imported binding".to_string(),
                        ));
                    }
                    self.check_binding_name(&import_name)?;
                    import_name.clone()
                };
                entries.push((import_name, local_name));
                if !self.eat(&TokenKind::Comma) || self.check(&TokenKind::RBrace) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, "}")?;
        }
        self.expect_contextual("from")?;
        let request = self.parse_module_specifier()?;
        self.expect_semi()?;
        for (import_name, local_name) in entries {
            import_entries.push(ImportEntry {
                module_request: request.clone(),
                import_name,
                local_name,
            });
        }
        Self::push_module_request(module_requests, request);
        Ok(())
    }

    fn parse_export_specifiers(&mut self) -> error::Result<Vec<ParsedExportSpecifier>> {
        self.expect(&TokenKind::LBrace, "{")?;
        let mut specifiers = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let (local_name, local_name_is_string) = self.parse_module_export_name()?;
            let export_name = if self.eat_contextual("as") {
                self.parse_module_export_name()?.0
            } else {
                local_name.clone()
            };
            specifiers.push(ParsedExportSpecifier {
                local_name,
                export_name,
                local_name_is_string,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(specifiers)
    }

    fn parse_default_function_declaration(
        &mut self,
        is_async: bool,
    ) -> error::Result<(Stmt, Arc<str>)> {
        if is_async {
            self.advance(); // async
        }
        let Expr::Function(mut function) = self.parse_function_expr_with_async(is_async)? else {
            unreachable!("function parser must return a function expression")
        };
        let local_name = function
            .name
            .clone()
            .unwrap_or_else(|| Arc::from("default"));
        if function.name.is_some() {
            self.check_binding_name(&local_name)?;
        }
        if function.name.is_none() {
            function.name = Some(Arc::from("default"));
        }
        function.has_name_binding = false;
        Ok((self.stmt(StmtNode::FunctionDecl(function)), local_name))
    }

    fn parse_export_declaration(
        &mut self,
        module_requests: &mut Vec<ModuleRequest>,
        export_entries: &mut Vec<ExportEntry>,
        leading_decorators: Vec<Expr>,
    ) -> error::Result<Option<Stmt>> {
        self.stmt_start_line = self.current_line();
        self.advance(); // export
        if self.eat(&TokenKind::Default) {
            if !leading_decorators.is_empty() && self.check(&TokenKind::At) {
                return Err(error::Error::syntax(
                    "Class decorators cannot appear on both sides of export default".to_string(),
                ));
            }
            let (stmt, local_name) = match self.peek() {
                TokenKind::Function => self.parse_default_function_declaration(false)?,
                TokenKind::Async
                    if matches!(self.peek_at_tok(1).kind, TokenKind::Function)
                        && !self.peek_at_tok(1).preceded_by_newline =>
                {
                    self.parse_default_function_declaration(true)?
                }
                TokenKind::Class | TokenKind::At => {
                    let decorators = if !leading_decorators.is_empty() {
                        leading_decorators
                    } else if self.check(&TokenKind::At) {
                        self.parse_decorator_list()?
                    } else {
                        Vec::new()
                    };
                    if !self.check(&TokenKind::Class) {
                        return Err(error::Error::syntax(
                            "Class decorators must be followed by a class".to_string(),
                        ));
                    }
                    let mut class = self.parse_class_body_with_decorators(true, decorators)?;
                    let local_name = class.name.clone().unwrap_or_else(|| Arc::from("default"));
                    if class.name.is_none() {
                        class.name = Some(Arc::from("default"));
                    }
                    (
                        self.stmt(StmtNode::ExprStmt(Expr::Class(class))),
                        local_name,
                    )
                }
                _ => {
                    if !leading_decorators.is_empty() {
                        return Err(error::Error::syntax(
                            "Decorators before export default require a class declaration"
                                .to_string(),
                        ));
                    }
                    let mut expression = self.parse_assign()?;
                    Self::name_function_from_ident(&mut expression, &Arc::from("default"));
                    self.expect_semi()?;
                    (
                        self.stmt(StmtNode::VarDecl {
                            kind: VarKind::Const,
                            decls: vec![(Arc::from("default"), Some(expression))],
                        }),
                        Arc::from("default"),
                    )
                }
            };
            export_entries.push(ExportEntry::Local {
                local_name,
                export_name: Arc::from("default"),
            });
            return Ok(Some(stmt));
        }
        if self.eat(&TokenKind::Star) {
            if self.eat_contextual("as") {
                let export_name = self.parse_module_export_name()?.0;
                self.expect_contextual("from")?;
                let request = self.parse_module_specifier()?;
                self.expect_semi()?;
                export_entries.push(ExportEntry::NamespaceReExport {
                    module_request: request.clone(),
                    export_name,
                });
                Self::push_module_request(module_requests, request);
                return Ok(None);
            }
            self.expect_contextual("from")?;
            let request = self.parse_module_specifier()?;
            self.expect_semi()?;
            export_entries.push(ExportEntry::Star {
                module_request: request.clone(),
            });
            Self::push_module_request(module_requests, request);
            return Ok(None);
        }
        if self.check(&TokenKind::LBrace) {
            let specifiers = self.parse_export_specifiers()?;
            if self.eat_contextual("from") {
                let request = self.parse_module_specifier()?;
                self.expect_semi()?;
                for specifier in specifiers {
                    export_entries.push(ExportEntry::ReExport {
                        module_request: request.clone(),
                        import_name: specifier.local_name,
                        export_name: specifier.export_name,
                    });
                }
                Self::push_module_request(module_requests, request);
            } else {
                self.expect_semi()?;
                for specifier in specifiers {
                    if specifier.local_name_is_string {
                        return Err(error::Error::syntax(
                            "A local export name cannot be a string literal".to_string(),
                        ));
                    }
                    if specifier.local_name.as_ref() == "default" {
                        return Err(error::Error::syntax(
                            "'default' is not a local binding name".to_string(),
                        ));
                    }
                    export_entries.push(ExportEntry::Local {
                        local_name: specifier.local_name,
                        export_name: specifier.export_name,
                    });
                }
            }
            return Ok(None);
        }

        if !leading_decorators.is_empty() && self.check(&TokenKind::At) {
            return Err(error::Error::syntax(
                "Class decorators cannot appear on both sides of export".to_string(),
            ));
        }
        let stmt = match self.peek() {
            TokenKind::Var | TokenKind::Let | TokenKind::Const => self.parse_var_decl()?,
            TokenKind::Function => self.parse_function_decl()?,
            TokenKind::Async
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function)
                    && !self.peek_at_tok(1).preceded_by_newline =>
            {
                self.advance();
                self.parse_function_decl_with_async(true)?
            }
            TokenKind::Class if !leading_decorators.is_empty() => {
                let class = self.parse_class_body_with_decorators(true, leading_decorators)?;
                self.stmt(StmtNode::ExprStmt(Expr::Class(class)))
            }
            TokenKind::Class | TokenKind::At => self.parse_class_decl()?,
            _ => {
                if !leading_decorators.is_empty() {
                    return Err(error::Error::syntax(
                        "Decorators before export require a class declaration".to_string(),
                    ));
                }
                return Err(error::Error::syntax(
                    "Expected export declaration".to_string(),
                ));
            }
        };
        let mut names = Vec::new();
        Self::collect_top_level_decl_names(&stmt.node, &mut names);
        if names.is_empty() {
            return Err(error::Error::syntax(
                "Expected named export declaration".to_string(),
            ));
        }
        for name in names {
            export_entries.push(ExportEntry::Local {
                local_name: name.clone(),
                export_name: name,
            });
        }
        Ok(Some(stmt))
    }

    fn collect_top_level_decl_names(node: &StmtNode, names: &mut Vec<Arc<str>>) {
        match node {
            StmtNode::VarDecl { decls, .. } => {
                names.extend(decls.iter().map(|(name, _)| name.clone()));
            }
            StmtNode::Destructure { pattern, .. } => collect_pattern_names(pattern, names),
            StmtNode::FunctionDecl(function) => {
                if let Some(name) = &function.name {
                    names.push(name.clone());
                }
            }
            StmtNode::ExprStmt(Expr::Class(class)) if class.is_declaration => {
                if let Some(name) = &class.name {
                    names.push(name.clone());
                }
            }
            _ => {}
        }
    }

    fn check_module_early_errors(
        body: &[Stmt],
        import_entries: &[ImportEntry],
        export_entries: &[ExportEntry],
    ) -> error::Result<()> {
        let mut lexical = Vec::new();
        let mut vars = Vec::new();
        for stmt in body {
            collect_decl_names(
                &stmt.node,
                &mut lexical,
                &mut vars,
                StatementListScope::Module,
            );
        }
        let declared: Vec<Arc<str>> = lexical.into_iter().chain(vars).collect();
        let mut import_names = Vec::new();
        for import in import_entries {
            if import_names.contains(&import.local_name) || declared.contains(&import.local_name) {
                return Err(error::Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    import.local_name
                )));
            }
            import_names.push(import.local_name.clone());
        }

        let mut exported_names = Vec::new();
        for export in export_entries {
            match export {
                ExportEntry::Local {
                    local_name,
                    export_name,
                } => {
                    if !declared.contains(local_name) && !import_names.contains(local_name) {
                        return Err(error::Error::syntax(format!(
                            "Export '{}' is not defined",
                            local_name
                        )));
                    }
                    if exported_names.contains(export_name) {
                        return Err(error::Error::syntax(format!(
                            "Duplicate export name '{}'",
                            export_name
                        )));
                    }
                    exported_names.push(export_name.clone());
                }
                ExportEntry::ReExport { export_name, .. } => {
                    if exported_names.contains(export_name) {
                        return Err(error::Error::syntax(format!(
                            "Duplicate export name '{}'",
                            export_name
                        )));
                    }
                    exported_names.push(export_name.clone());
                }
                ExportEntry::Star { .. } => {}
                ExportEntry::NamespaceReExport { export_name, .. } => {
                    if exported_names.contains(export_name) {
                        return Err(error::Error::syntax(format!(
                            "Duplicate export name '{}'",
                            export_name
                        )));
                    }
                    exported_names.push(export_name.clone());
                }
            }
        }
        Ok(())
    }

    fn normalize_import_reexports(
        import_entries: &[ImportEntry],
        export_entries: &mut [ExportEntry],
    ) {
        for export in export_entries {
            let ExportEntry::Local {
                local_name,
                export_name,
            } = export
            else {
                continue;
            };
            let Some(import) = import_entries
                .iter()
                .find(|import| import.local_name == *local_name)
            else {
                continue;
            };
            *export = if import.import_name.as_ref() == "*" {
                ExportEntry::NamespaceReExport {
                    module_request: import.module_request.clone(),
                    export_name: export_name.clone(),
                }
            } else {
                ExportEntry::ReExport {
                    module_request: import.module_request.clone(),
                    import_name: import.import_name.clone(),
                    export_name: export_name.clone(),
                }
            };
        }
    }

    /// Peek the token stream for a leading `"use strict"` string-literal
    /// directive (optionally followed by a semicolon and more directives).
    /// Does not consume tokens.
    fn peek_use_strict_directive(&self) -> bool {
        let mut i = self.pos;
        loop {
            match self.tokens.get(i) {
                Some(t)
                    if matches!(&t.kind, TokenKind::String(s) if &**s == "use strict")
                        && !t.string_had_escape =>
                {
                    return true;
                }
                Some(t) if matches!(&t.kind, TokenKind::String(_)) => {
                    // Another directive; skip it and its optional semicolon.
                    i += 1;
                    if matches!(
                        self.tokens.get(i).map(|t| &t.kind),
                        Some(TokenKind::Semicolon)
                    ) {
                        i += 1;
                    }
                    continue;
                }
                _ => return false,
            }
        }
    }

    /// Scan a statement list's directive prologue (leading string-literal
    /// expression statements) and return true if a `"use strict"` directive
    /// is present. Per spec, only the leading run of string-literal
    /// expression statements counts; the first non-directive statement ends it.
    pub fn scan_directive_prologue(body: &[Stmt]) -> bool {
        for stmt in body {
            match &stmt.node {
                StmtNode::ExprStmt(Expr::String(s)) if s.as_ref() == "use strict" => {
                    return true;
                }
                StmtNode::ExprStmt(Expr::String(_)) => continue,
                _ => break,
            }
        }
        false
    }

    fn has_non_simple_params(
        param_defaults: &[Option<Expr>],
        rest_param: Option<&Arc<str>>,
        has_destructuring_params: bool,
    ) -> bool {
        param_defaults.iter().any(Option::is_some)
            || rest_param.is_some()
            || has_destructuring_params
    }

    fn reject_use_strict_with_non_simple_params(
        body_contains_use_strict: bool,
        param_defaults: &[Option<Expr>],
        rest_param: Option<&Arc<str>>,
        has_destructuring_params: bool,
    ) -> error::Result<()> {
        if body_contains_use_strict
            && Self::has_non_simple_params(param_defaults, rest_param, has_destructuring_params)
        {
            return Err(error::Error::syntax(
                "'use strict' not allowed with non-simple parameters".to_string(),
            ));
        }
        Ok(())
    }

    fn reject_duplicate_formal_params(
        params: &[Arc<str>],
        dstr_decls: &[(Pattern, String, Option<Expr>)],
        rest_param: Option<&Arc<str>>,
    ) -> error::Result<()> {
        let synthetic_params: std::collections::HashSet<&str> =
            dstr_decls.iter().map(|(_, tmp, _)| tmp.as_str()).collect();
        let mut names = Vec::new();
        for param in params {
            if !synthetic_params.contains(param.as_ref()) {
                names.push(param.clone());
            }
        }
        for (pattern, _tmp, _default) in dstr_decls {
            collect_pattern_names(pattern, &mut names);
        }
        if let Some(rest) = rest_param {
            if !synthetic_params.contains(rest.as_ref()) {
                names.push(rest.clone());
            }
        }
        check_duplicate_bound_names(&names)
    }

    fn reject_accessor_parameter_early_errors(
        kind: PropKind,
        params: &[Arc<str>],
        rest_param: Option<&Arc<str>>,
    ) -> error::Result<()> {
        let valid = match kind {
            PropKind::Get => params.is_empty() && rest_param.is_none(),
            PropKind::Set => params.len() == 1 && rest_param.is_none(),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(error::Error::syntax(match kind {
                PropKind::Get => "Getter must not have parameters".to_string(),
                PropKind::Set => "Setter must have exactly one non-rest parameter".to_string(),
                _ => unreachable!(),
            }))
        }
    }

    fn parse_stmt(&mut self) -> error::Result<Stmt> {
        // Bound statement recursion so deeply nested `{{...}}` / `if(1) if(1)
        // ...` fails with a SyntaxError instead of overflowing the Rust
        // parser stack and aborting the process. The counter is bumped here
        // and restored on every exit path (including `?` errors via the
        // trailing decrement after `parse_stmt_inner`).
        if self.stmt_depth >= Self::MAX_STMT_DEPTH {
            return Err(error::Error::syntax(format!(
                "Maximum statement nesting depth ({}) exceeded",
                Self::MAX_STMT_DEPTH
            )));
        }
        self.stmt_depth += 1;
        let result = self.parse_stmt_inner();
        self.stmt_depth -= 1;
        result
    }

    fn parse_stmt_inner(&mut self) -> error::Result<Stmt> {
        self.stmt_start_line = self.current_line();
        // Labeled statement: `ident:` followed by any statement. Detect by
        // peeking two tokens so a leading identifier isn't misread as an
        // expression statement.
        if let Some(label) = self.peek_label_identifier() {
            if matches!(self.peek_at_tok(1).kind, TokenKind::Colon) {
                self.advance(); // label identifier
                self.advance(); // ':'
                                // Peek the body's first token to determine if it's a loop.
                let is_loop = matches!(
                    self.peek(),
                    TokenKind::While | TokenKind::Do | TokenKind::For
                );
                // ES spec: lexical declarations cannot be the body of a
                // labelled statement. `let` is contextual here: if it does
                // not hit the `let [` ExpressionStatement lookahead
                // restriction, parse it as an expression statement under ASI.
                if matches!(self.peek(), TokenKind::Const | TokenKind::Class) {
                    return Err(error::Error::syntax(
                        "Lexical declaration cannot be the body of a labelled statement"
                            .to_string(),
                    ));
                }
                if self.label_body_starts_with_disallowed_function_decl() {
                    return Err(error::Error::syntax(
                        "Function declaration cannot be the body of this labelled statement"
                            .to_string(),
                    ));
                }
                if self.label_stack.iter().any(|(name, _)| name == &label) {
                    return Err(error::Error::syntax(format!(
                        "Label '{}' has already been declared",
                        label
                    )));
                }
                self.label_stack.push((label.clone(), is_loop));
                let body =
                    self.with_lexical_declaration_context(false, |p| p.parse_stmt_inner())?;
                self.label_stack.pop();
                return Ok(self.stmt(StmtNode::Labeled(label, Box::new(body))));
            }
        }
        match self.peek().clone() {
            TokenKind::LBrace => self.parse_block(),
            TokenKind::Var | TokenKind::Const => self.parse_var_decl(),
            TokenKind::Let
                if self.lexical_declaration_allowed && self.is_let_lexical_position() =>
            {
                self.parse_var_decl()
            }
            TokenKind::Let if matches!(self.peek_at_tok(1).kind, TokenKind::LBracket) => Err(
                error::Error::syntax("Expression statement cannot start with 'let ['".to_string()),
            ),
            TokenKind::Function => self.parse_function_decl(),
            TokenKind::Async => {
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function)
                    && !self.peek_at_tok(1).preceded_by_newline
                {
                    self.advance(); // async
                    self.parse_function_decl_with_async(true)
                } else {
                    let e = self.parse_expr()?;
                    self.expect_semi()?;
                    Ok(self.stmt(StmtNode::ExprStmt(e)))
                }
            }
            TokenKind::Class | TokenKind::At => self.parse_class_decl(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                self.advance();
                let l = self.parse_opt_label();
                self.expect_semi()?;
                // `break` (unlabelled) is only valid inside a loop or switch.
                if l.is_none() && self.loop_depth == 0 && self.switch_depth == 0 {
                    return Err(error::Error::syntax("Illegal break statement".to_string()));
                }
                // `break label` must target a visible label.
                if let Some(ref label) = l {
                    if !self.label_stack.iter().any(|(name, _)| name == label) {
                        return Err(error::Error::syntax(format!("Undefined label '{}'", label)));
                    }
                }
                Ok(self.stmt(StmtNode::Break(l)))
            }
            TokenKind::Continue => {
                self.advance();
                let l = self.parse_opt_label();
                self.expect_semi()?;
                // `continue` (unlabelled) is only valid inside a loop.
                if l.is_none() && self.loop_depth == 0 {
                    return Err(error::Error::syntax(
                        "Illegal continue statement".to_string(),
                    ));
                }
                // `continue label` must target a label on an enclosing
                // iteration statement (not just any labeled statement).
                if let Some(ref label) = l {
                    if !self
                        .label_stack
                        .iter()
                        .any(|(name, is_loop)| name == label && *is_loop)
                    {
                        return Err(error::Error::syntax(format!(
                            "Continue label '{}' must target an enclosing iteration statement",
                            label
                        )));
                    }
                }
                Ok(self.stmt(StmtNode::Continue(l)))
            }
            TokenKind::Debugger => {
                self.advance();
                self.expect_semi()?;
                Ok(self.stmt(StmtNode::Empty))
            }
            TokenKind::Throw => {
                self.advance();
                if self.at_newline_before() {
                    return Err(error::Error::syntax(
                        "Line terminator not allowed after throw".to_string(),
                    ));
                }
                let e = self.parse_expr()?;
                self.expect_semi()?;
                Ok(self.stmt(StmtNode::Throw(e)))
            }
            TokenKind::Try => self.parse_try(),
            TokenKind::With => self.parse_with(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Semicolon => {
                self.advance();
                Ok(self.stmt(StmtNode::Empty))
            }
            _ => {
                let e = self.parse_expr()?;
                self.expect_semi()?;
                Ok(self.stmt(StmtNode::ExprStmt(e)))
            }
        }
    }

    fn peek_label_identifier(&self) -> Option<Arc<str>> {
        match self.peek().clone() {
            TokenKind::Ident(s)
                if !(Self::is_reserved_identifier_reference_word(&s)
                    || self.is_strict_context
                        && Self::is_strict_identifier_reference_reserved(&s))
                    && (s != "await" || self.await_as_identifier_allowed())
                    && (s != "yield" || self.yield_as_identifier_allowed()) =>
            {
                Some(Arc::from(s.as_str()))
            }
            TokenKind::Await if self.await_as_identifier_allowed() => Some(Arc::from("await")),
            TokenKind::Yield if self.yield_as_identifier_allowed() => Some(Arc::from("yield")),
            _ => None,
        }
    }

    fn label_body_starts_with_disallowed_function_decl(&self) -> bool {
        match self.peek() {
            // Annex B only permits an ordinary FunctionDeclaration here in
            // sloppy code; generators remain declarations, not statements.
            TokenKind::Function => {
                self.is_strict_context || matches!(self.peek_at_tok(1).kind, TokenKind::Star)
            }
            // Async declarations have no Annex B labelled-statement exception.
            TokenKind::Async => {
                !self.peek_at_tok(1).preceded_by_newline
                    && matches!(self.peek_at_tok(1).kind, TokenKind::Function)
            }
            _ => false,
        }
    }

    fn parse_opt_label(&mut self) -> Option<Arc<str>> {
        // Per spec, `break` and `continue` must not have a line terminator
        // between the keyword and the label. If a newline precedes the next
        // token, ASI applies and the statement is unlabelled.
        if self.peek_at_tok(0).preceded_by_newline {
            return None;
        }
        let label = self.peek_label_identifier();
        if label.is_some() {
            self.advance();
        }
        label
    }

    fn parse_block(&mut self) -> error::Result<Stmt> {
        self.expect(&TokenKind::LBrace, "{")?;
        let mut body = Vec::new();
        self.with_lexical_declaration_context(true, |p| {
            while !p.check(&TokenKind::RBrace) && !p.check(&TokenKind::Eof) {
                body.push(p.parse_stmt()?);
            }
            Ok(())
        })?;
        self.expect(&TokenKind::RBrace, "}")?;
        check_statement_list_declaration_early_errors(&body, StatementListScope::Block)?;
        Ok(self.stmt(StmtNode::Block(body)))
    }

    fn parse_var_decl(&mut self) -> error::Result<Stmt> {
        let stmt = self.parse_var_decl_no_semi()?;
        self.expect_semi()?;
        Ok(stmt)
    }

    fn parse_function_decl(&mut self) -> error::Result<Stmt> {
        self.parse_function_decl_with_async(false)
    }

    fn parse_function_decl_with_async(&mut self, is_async: bool) -> error::Result<Stmt> {
        self.advance(); // function
        let is_generator = self.eat(&TokenKind::Star);
        let name = match self.advance() {
            TokenKind::Ident(s) => Some(Arc::from(s.as_str())),
            TokenKind::Await if self.await_as_identifier_allowed() => Some(Arc::from("await")),
            TokenKind::Yield if self.yield_as_identifier_allowed() => Some(Arc::from("yield")),
            other => {
                return Err(error::Error::syntax(format!(
                    "Expected function name, got {:?}",
                    other
                )))
            }
        };
        if let Some(ref name) = name {
            self.check_binding_name(name)?;
        }
        let (params, param_defaults, rest_param, dstr_decls) =
            self.parse_params_scoped(is_generator, is_async, false)?;
        let mut body = self.parse_fn_body(false, false, is_generator, is_async)?;
        let body_contains_use_strict = self.last_fn_body_use_strict_directive;
        let has_destructuring_params = !dstr_decls.is_empty();
        if Self::has_non_simple_params(
            &param_defaults,
            rest_param.as_ref(),
            has_destructuring_params,
        ) {
            Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
        }
        Self::reject_use_strict_with_non_simple_params(
            body_contains_use_strict,
            &param_defaults,
            rest_param.as_ref(),
            has_destructuring_params,
        )?;
        {
            let mut pre = Self::dstr_prelude_from(dstr_decls);
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || body_contains_use_strict;
        // A body directive is discovered after parsing the parameters, so
        // strict-only reserved words must be validated again here.
        if is_strict {
            if let Some(ref n) = name {
                if matches!(&**n, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(n)
                {
                    return Err(error::Error::syntax(format!(
                        "'{}' cannot be used as a function name in strict mode",
                        n
                    )));
                }
            }
            for p in &params {
                if matches!(&**p, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(p)
                {
                    return Err(error::Error::syntax(format!(
                        "Parameter name '{}' is not allowed in strict mode",
                        p
                    )));
                }
            }
            let mut seen = std::collections::HashSet::new();
            for p in &params {
                if !seen.insert(p.clone()) {
                    return Err(error::Error::syntax(format!(
                        "Duplicate parameter '{}' is not allowed in strict mode",
                        p
                    )));
                }
            }
        }
        let saved = self.is_strict_context;
        self.is_strict_context = is_strict;
        // Re-scan not needed; params already parsed before body. Strictness from
        // the directive applies to the body; we set it for any nested parse.
        self.is_strict_context = saved;
        Ok(self.stmt(StmtNode::FunctionDecl(FunctionExpr {
            name,
            params,
            param_defaults,
            rest_param,
            body,
            is_arrow: false,
            is_async,
            is_generator,
            param_decls: Vec::new(),
            is_strict,
            is_method: false,
            has_name_binding: false,
        })))
    }

    fn parse_params(&mut self) -> error::Result<Vec<Arc<str>>> {
        self.expect(&TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            if self.check(&TokenKind::Spread) {
                // rest parameter: ...name (must be last)
                self.advance();
                // rest may be a destructuring pattern: `function f(...[a, b])`
                if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                    let p = self.parse_destructure_pattern()?;
                    // Keep compiler-only carriers outside IdentifierName so
                    // source bindings can never collide with them.
                    let tmp = format!("#arg{}", params.len());
                    self.cur_rest_param = Some(Arc::from(tmp.as_str()));
                    self.cur_param_destructure_decls.push((p, tmp, None));
                    break;
                }
                self.cur_rest_param =
                    Some(self.parse_binding_identifier("Expected rest parameter name")?);
                break;
            }
            if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                // Destructuring parameter: `function f([a, b])` / `f({x, y})`.
                let p = self.parse_destructure_pattern()?;
                let tmp = format!("#arg{}", params.len());
                params.push(Arc::from(tmp.as_str()));
                self.cur_param_defaults.push(None);
                let default = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                self.cur_param_destructure_decls.push((p, tmp, default));
            } else {
                let param = self.parse_binding_identifier("Expected parameter name")?;
                params.push(param);
                let default = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                self.cur_param_defaults.push(default);
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, ")")?;
        // Pad defaults to match params length.
        while self.cur_param_defaults.len() < params.len() {
            self.cur_param_defaults.push(None);
        }
        Ok(params)
    }

    fn parse_fn_body(
        &mut self,
        super_allowed: bool,
        super_call_allowed: bool,
        generator_body: bool,
        async_body: bool,
    ) -> error::Result<Vec<Stmt>> {
        self.with_function_body_context(generator_body, async_body, |p| {
            p.parse_fn_body_inner(super_allowed, super_call_allowed)
        })
    }

    fn parse_arrow_block_body(&mut self, async_body: bool) -> error::Result<Vec<Stmt>> {
        self.with_function_body_context(false, async_body, |p| {
            p.parse_fn_body_inner_inherited_super()
        })
    }

    fn parse_static_block_body(&mut self) -> error::Result<Vec<Stmt>> {
        self.expect(&TokenKind::LBrace, "{")?;
        let saved_loop_depth = self.loop_depth;
        let saved_switch_depth = self.switch_depth;
        let saved_label_stack = std::mem::take(&mut self.label_stack);
        let saved_function_depth = self.function_depth;
        let saved_super = self.super_depth;
        let saved_super_call = self.super_call_depth;
        let saved_new_target_allowed = self.new_target_allowed;

        self.loop_depth = 0;
        self.switch_depth = 0;
        self.function_depth = 0;
        self.super_depth += 1;
        self.super_call_depth = 0;
        self.new_target_allowed = true;

        let result = (|| {
            let mut body = Vec::new();
            self.with_lexical_declaration_context(true, |p| {
                while !p.check(&TokenKind::RBrace) && !p.check(&TokenKind::Eof) {
                    body.push(p.parse_stmt()?);
                }
                Ok(())
            })?;
            self.expect(&TokenKind::RBrace, "}")?;
            Self::reject_static_block_early_errors(&body)?;
            Ok(body)
        })();

        self.loop_depth = saved_loop_depth;
        self.switch_depth = saved_switch_depth;
        self.label_stack = saved_label_stack;
        self.function_depth = saved_function_depth;
        self.super_depth = saved_super;
        self.super_call_depth = saved_super_call;
        self.new_target_allowed = saved_new_target_allowed;
        result
    }

    fn parse_class_field_initializer(&mut self) -> error::Result<Expr> {
        let saved_super = self.super_depth;
        let saved_super_call = self.super_call_depth;
        let saved_new_target_allowed = self.new_target_allowed;
        self.super_depth += 1;
        self.super_call_depth = 0;
        self.new_target_allowed = true;
        let result = self.parse_assign();
        self.super_depth = saved_super;
        self.super_call_depth = saved_super_call;
        self.new_target_allowed = saved_new_target_allowed;
        result
    }

    fn parse_fn_body_inner_inherited_super(&mut self) -> error::Result<Vec<Stmt>> {
        self.expect(&TokenKind::LBrace, "{")?;
        let body_is_strict = self.peek_use_strict_directive();
        let saved_strict = self.is_strict_context;
        if body_is_strict {
            self.is_strict_context = true;
        }
        self.function_depth += 1;
        let result = self.with_function_statement_control_context(|p| {
            let mut body = Vec::new();
            p.with_lexical_declaration_context(true, |p| {
                while !p.check(&TokenKind::RBrace) && !p.check(&TokenKind::Eof) {
                    body.push(p.parse_stmt()?);
                }
                Ok(())
            })?;
            p.expect(&TokenKind::RBrace, "}")?;
            check_statement_list_declaration_early_errors(&body, StatementListScope::FunctionBody)?;
            Ok(body)
        });
        self.function_depth -= 1;
        self.is_strict_context = saved_strict;
        self.last_fn_body_use_strict_directive = body_is_strict;
        result
    }

    fn parse_fn_body_inner(
        &mut self,
        super_allowed: bool,
        super_call_allowed: bool,
    ) -> error::Result<Vec<Stmt>> {
        self.expect(&TokenKind::LBrace, "{")?;
        // Detect a leading "use strict" directive BEFORE parsing body
        // statements, so strict-mode early errors (e.g. assignment to `eval`)
        // are caught during parsing.
        let body_is_strict = self.peek_use_strict_directive();
        let saved_strict = self.is_strict_context;
        if body_is_strict {
            self.is_strict_context = true;
        }
        let saved_super = self.super_depth;
        let saved_super_call = self.super_call_depth;
        let saved_new_target_allowed = self.new_target_allowed;
        if super_allowed {
            self.super_depth += 1;
        } else {
            self.super_depth = 0;
        }
        if super_call_allowed {
            self.super_call_depth += 1;
        } else {
            self.super_call_depth = 0;
        }
        self.new_target_allowed = true;
        self.function_depth += 1;
        let result = self.with_function_statement_control_context(|p| {
            let mut body = Vec::new();
            p.with_lexical_declaration_context(true, |p| {
                while !p.check(&TokenKind::RBrace) && !p.check(&TokenKind::Eof) {
                    body.push(p.parse_stmt()?);
                }
                Ok(())
            })?;
            p.expect(&TokenKind::RBrace, "}")?;
            check_statement_list_declaration_early_errors(&body, StatementListScope::FunctionBody)?;
            Ok(body)
        });
        self.function_depth -= 1;
        self.is_strict_context = saved_strict;
        self.super_depth = saved_super;
        self.super_call_depth = saved_super_call;
        self.new_target_allowed = saved_new_target_allowed;
        self.last_fn_body_use_strict_directive = body_is_strict;
        result
    }

    /// Take the destructuring-parameter declarations collected by the most
    /// recent `parse_params` and turn them into a prelude of `let <pat> =
    /// #argN;` statements to prepend to a function body.
    fn take_dstr_prelude(&mut self) -> Vec<Stmt> {
        let dstr_decls = std::mem::take(&mut self.cur_param_destructure_decls);
        Self::dstr_prelude_from(dstr_decls)
    }

    fn dstr_prelude_from(dstr_decls: Vec<(Pattern, String, Option<Expr>)>) -> Vec<Stmt> {
        dstr_decls
            .into_iter()
            .map(|(pattern, tmp, default)| {
                // If the destructuring parameter had a default, the binding
                // source is `#argN === undefined ? <default> : #argN`. We
                // encode that by wrapping the pattern's default into the
                // pattern via Pattern::Assign, which the compiler already
                // lowers as "use default when the source value is undefined".
                let pattern = match default {
                    Some(d) => Pattern::Assign(Box::new(pattern), d),
                    None => pattern,
                };
                Stmt {
                    line: 0,
                    node: StmtNode::Destructure {
                        kind: VarKind::Let,
                        pattern,
                        init: Some(Expr::Ident(Arc::from(tmp.as_str()))),
                    },
                }
            })
            .collect()
    }

    fn parse_if(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        let then = Box::new(self.parse_single_stmt()?);
        let else_ = if self.eat(&TokenKind::Else) {
            Some(Box::new(self.parse_single_stmt()?))
        } else {
            None
        };
        Ok(self.stmt(StmtNode::If { cond, then, else_ }))
    }

    /// Parse a single statement in a position where class declarations
    /// are not allowed (if/else/while/for/with body). ES6 spec forbids
    /// class declarations as the body of a single statement.
    fn parse_single_stmt(&mut self) -> error::Result<Stmt> {
        let stmt = self.with_lexical_declaration_context(false, |p| p.parse_stmt())?;
        match &stmt.node {
            StmtNode::ExprStmt(Expr::Class(_)) => {
                return Err(error::Error::syntax(
                    "Class declaration cannot be used as a single statement body".to_string(),
                ));
            }
            // ES6: labelled function declarations are not allowed as the
            // body of if/else/while/do-while/for/with without a block.
            // Check recursively through nested labels.
            StmtNode::Labeled(_, body) if is_labelled_function(&stmt.node) => {
                return Err(error::Error::syntax(
                    "Labelled function declaration cannot be used as a single statement body"
                        .to_string(),
                ));
            }
            // ES6: function declarations are not allowed as the body of
            // if/else/while/do-while/for/with without a block (in strict mode).
            StmtNode::FunctionDecl(_) => {
                return Err(error::Error::syntax(
                    "Function declaration cannot be used as a single statement body".to_string(),
                ));
            }
            // ES6: lexical declarations (let/const) are not allowed as the
            // body of if/else/while/do-while/for/with without a block.
            StmtNode::VarDecl { kind, .. } if *kind != VarKind::Var => {
                return Err(error::Error::syntax(
                    "Lexical declaration cannot be used as a single statement body".to_string(),
                ));
            }
            _ => {}
        }
        Ok(stmt)
    }

    fn parse_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        self.loop_depth += 1;
        let body = Box::new(self.parse_single_stmt()?);
        self.loop_depth -= 1;
        Ok(self.stmt(StmtNode::While { cond, body }))
    }

    fn parse_with(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let object = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        let body = Box::new(self.parse_single_stmt()?);
        Ok(self.stmt(StmtNode::With { object, body }))
    }

    fn parse_do_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.loop_depth += 1;
        let body = Box::new(self.parse_single_stmt()?);
        self.loop_depth -= 1;
        self.expect(&TokenKind::While, "while")?;
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        self.eat(&TokenKind::Semicolon);
        Ok(self.stmt(StmtNode::DoWhile { body, cond }))
    }

    /// Check for duplicate bound names in a for-in/for-of declaration.
    /// ES spec: It is a Syntax Error if the BoundNames of ForDeclaration
    /// contains any duplicate entries.
    fn check_for_dup_bound_names(&self, node: &StmtNode) -> error::Result<()> {
        let mut names = Vec::new();
        let is_var = match node {
            StmtNode::VarDecl { kind, .. } => *kind == VarKind::Var,
            StmtNode::Destructure { kind, .. } => *kind == VarKind::Var,
            _ => return Ok(()),
        };
        // Per spec, duplicate bound names are only a Syntax Error for
        // lexical declarations (let/const), not for var.
        if is_var {
            return Ok(());
        }
        match node {
            StmtNode::VarDecl { decls, .. } => {
                for (name, _) in decls {
                    names.push(name.clone());
                }
            }
            StmtNode::Destructure { pattern, .. } => {
                collect_pattern_names(pattern, &mut names);
            }
            _ => return Ok(()),
        }
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            if !seen.insert(name.clone()) {
                return Err(error::Error::syntax(format!(
                    "Duplicate declaration '{}' in for-in/for-of head",
                    name
                )));
            }
        }
        Ok(())
    }

    fn reject_strict_formal_param_names(
        params: &[Arc<str>],
        dstr_decls: &[(Pattern, String, Option<Expr>)],
        rest_param: Option<&Arc<str>>,
    ) -> error::Result<()> {
        let synthetic_params: std::collections::HashSet<&str> =
            dstr_decls.iter().map(|(_, tmp, _)| tmp.as_str()).collect();
        let mut names = Vec::new();
        for param in params {
            if !synthetic_params.contains(param.as_ref()) {
                names.push(param.clone());
            }
        }
        for (pattern, _tmp, _default) in dstr_decls {
            collect_pattern_names(pattern, &mut names);
        }
        if let Some(rest) = rest_param {
            if !synthetic_params.contains(rest.as_ref()) {
                names.push(rest.clone());
            }
        }
        for name in names {
            if matches!(&*name, "eval" | "arguments")
                || Self::is_strict_identifier_reference_reserved(&name)
            {
                return Err(error::Error::syntax(format!(
                    "Parameter name '{}' is not allowed in strict mode",
                    name
                )));
            }
        }
        Ok(())
    }

    /// Check that no var-declared name in `body` shadows a lexical binding
    /// declared in the for-in/for-of `head`. Per spec: "It is a Syntax Error
    /// if any element of the BoundNames of ForDeclaration also occurs in the
    /// VarDeclaredNames of Statement."
    fn check_for_head_body_clash(&self, head: &StmtNode, body: &Stmt) -> error::Result<()> {
        // Collect head bound names.
        let mut head_names = Vec::new();
        match head {
            StmtNode::VarDecl {
                kind: VarKind::Let | VarKind::Const,
                decls,
                ..
            } => {
                for (name, _) in decls {
                    head_names.push(name.clone());
                }
            }
            StmtNode::Destructure {
                kind: VarKind::Let | VarKind::Const,
                pattern,
                ..
            } => {
                collect_pattern_names(pattern, &mut head_names);
            }
            _ => return Ok(()), // var heads don't trigger this rule
        }
        if head_names.is_empty() {
            return Ok(());
        }
        // Collect body var-declared names.
        let mut body_vars = Vec::new();
        Self::collect_var_names_in_stmt(&body.node, &mut body_vars);
        for name in &head_names {
            if body_vars.contains(name) {
                return Err(error::Error::syntax(format!(
                    "Variable '{}' declared in for-of/for-in head is redeclared with var in body",
                    name
                )));
            }
        }
        Ok(())
    }

    fn collect_var_names_in_stmt(node: &StmtNode, out: &mut Vec<Arc<str>>) {
        match node {
            StmtNode::VarDecl {
                kind: VarKind::Var,
                decls,
            } => {
                for (name, _) in decls {
                    out.push(name.clone());
                }
            }
            StmtNode::Destructure {
                kind: VarKind::Var,
                pattern,
                ..
            } => {
                collect_pattern_names(pattern, out);
            }
            StmtNode::Block(body) => {
                for s in body {
                    Self::collect_var_names_in_stmt(&s.node, out);
                }
            }
            StmtNode::If { then, else_, .. } => {
                Self::collect_var_names_in_stmt(&then.node, out);
                if let Some(e) = else_ {
                    Self::collect_var_names_in_stmt(&e.node, out);
                }
            }
            StmtNode::While { body, .. }
            | StmtNode::DoWhile { body, .. }
            | StmtNode::For { body, .. }
            | StmtNode::ForIn { body, .. }
            | StmtNode::ForOf { body, .. }
            | StmtNode::Labeled(_, body) => {
                Self::collect_var_names_in_stmt(&body.node, out);
            }
            StmtNode::Switch { cases, .. } => {
                for case in cases {
                    for s in &case.body {
                        Self::collect_var_names_in_stmt(&s.node, out);
                    }
                }
            }
            StmtNode::TryCatch {
                try_body,
                catch_body,
                finally_body,
                ..
            } => {
                Self::collect_var_names_in_stmt(&try_body.node, out);
                if let Some(cb) = catch_body {
                    Self::collect_var_names_in_stmt(&cb.node, out);
                }
                if let Some(fb) = finally_body {
                    Self::collect_var_names_in_stmt(&fb.node, out);
                }
            }
            _ => {}
        }
    }

    fn parse_for(&mut self) -> error::Result<Stmt> {
        self.advance();
        // `for await (x of asyncIterable)` — async iteration. Only the for-of
        // form is valid; `for await` requires an enclosing async function.
        let is_await = self.eat(&TokenKind::Await);
        self.expect(&TokenKind::LParen, "(")?;
        // init
        let init: Option<Box<Stmt>> = if self.check(&TokenKind::Semicolon) {
            None
        } else if matches!(self.peek(), TokenKind::Var | TokenKind::Const)
            || (matches!(self.peek(), TokenKind::Let) && self.is_let_lexical_position())
        {
            // could be for-in / for-of; set no_in so the initializer
            // expression doesn't consume the `in` as a binary operator.
            self.no_in = true;
            let stmt = self.parse_var_decl_no_semi()?;
            self.no_in = false;
            // Check for duplicate bound names in for-in/for-of declarations.
            self.check_for_dup_bound_names(&stmt.node)?;
            if self.check(&TokenKind::In) {
                // for-in/for-of head declarations must not have an initializer.
                match &stmt.node {
                    StmtNode::VarDecl { decls, .. } => {
                        if decls.len() != 1 {
                            return Err(error::Error::syntax(
                                "for-in head declaration must have exactly one binding".to_string(),
                            ));
                        }
                        if decls.iter().any(|d| d.1.is_some()) {
                            return Err(error::Error::syntax(
                                "for-in head declaration must not have an initializer".to_string(),
                            ));
                        }
                    }
                    StmtNode::Destructure { init, .. } if init.is_some() => {
                        return Err(error::Error::syntax(
                            "for-in head declaration must not have an initializer".to_string(),
                        ));
                    }
                    _ => {}
                }
                self.advance();
                let right = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
                self.loop_depth += 1;
                let body = Box::new(self.parse_single_stmt()?);
                self.loop_depth -= 1;
                self.check_for_head_body_clash(&stmt.node, &body)?;
                return Ok(self.stmt(StmtNode::ForIn {
                    left: Box::new(stmt),
                    right,
                    body,
                }));
            }
            if self.is_raw_of() {
                // for-of head declarations must not have an initializer.
                match &stmt.node {
                    StmtNode::VarDecl { decls, .. } => {
                        if decls.len() != 1 {
                            return Err(error::Error::syntax(
                                "for-of head declaration must have exactly one binding".to_string(),
                            ));
                        }
                        if decls.iter().any(|d| d.1.is_some()) {
                            return Err(error::Error::syntax(
                                "for-of head declaration must not have an initializer".to_string(),
                            ));
                        }
                    }
                    StmtNode::Destructure { init, .. } if init.is_some() => {
                        return Err(error::Error::syntax(
                            "for-of head declaration must not have an initializer".to_string(),
                        ));
                    }
                    _ => {}
                }
                self.advance();
                let right = self.parse_assign()?;
                self.expect(&TokenKind::RParen, ")")?;
                self.loop_depth += 1;
                let body = Box::new(self.parse_single_stmt()?);
                self.loop_depth -= 1;
                self.check_for_head_body_clash(&stmt.node, &body)?;
                return Ok(self.stmt(StmtNode::ForOf {
                    left: Box::new(stmt),
                    right,
                    body,
                    is_await,
                }));
            } else if is_await {
                return Err(error::Error::syntax(
                    "'for await' is only valid with for...of".to_string(),
                ));
            }
            Some(Box::new(stmt))
        } else {
            // Non-declaration for-head: `for (expr in/of rhs)`.
            // Parse the left-hand side with `no_in` so that `in` is not
            // consumed as a binary operator, allowing us to detect the
            // for-in form.
            // 'let of' in for-of head is a SyntaxError: 'let' is treated as
            // a ForDeclaration but 'of' is not a valid binding name.
            if matches!(self.peek(), TokenKind::Let)
                && !self.is_strict_context
                && self.is_raw_of_at(1)
            {
                return Err(error::Error::syntax(
                    "let followed by of in for-of head is not valid".to_string(),
                ));
            }
            if matches!(self.peek(), TokenKind::Async)
                && !is_await
                && !self.peek_at_tok(0).had_escape
                && self.is_raw_of_at(1)
                && !matches!(self.peek_at_tok(2).kind, TokenKind::Arrow)
            {
                return Err(error::Error::syntax(
                    "async cannot be the for-of left-hand side".to_string(),
                ));
            }
            let allow_in_in_lhs = self.has_top_level_for_of_delimiter();
            self.no_in = !allow_in_in_lhs;
            let lhs_start = self.pos;
            let e = self.with_deferred_object_proto_duplicate_check(|p| p.parse_assign())?;
            self.no_in = false;
            if self.check(&TokenKind::In) {
                // Validate that the LHS is a valid assignment target.
                if !Self::is_for_in_of_assignment_target(&e) {
                    return Err(error::Error::syntax(
                        "Invalid left-hand side in for-in".to_string(),
                    ));
                }
                self.reject_array_rest_continuation_assignment_target(&e, lhs_start, self.pos)?;
                if self.is_strict_context {
                    Self::reject_strict_eval_arguments_assignment_target(&e)?;
                }
                self.advance();
                let right = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
                self.loop_depth += 1;
                let body = Box::new(self.parse_single_stmt()?);
                self.loop_depth -= 1;
                return Ok(self.stmt(StmtNode::ForIn {
                    left: Box::new(self.stmt(StmtNode::ExprStmt(e))),
                    right,
                    body,
                }));
            }
            if self.is_raw_of() {
                // Validate that the LHS is a valid assignment target.
                if !Self::is_for_in_of_assignment_target(&e) {
                    return Err(error::Error::syntax(
                        "Invalid left-hand side in for-of".to_string(),
                    ));
                }
                self.reject_array_rest_continuation_assignment_target(&e, lhs_start, self.pos)?;
                if self.is_strict_context {
                    Self::reject_strict_eval_arguments_assignment_target(&e)?;
                }
                self.advance();
                let right = self.parse_assign()?;
                self.expect(&TokenKind::RParen, ")")?;
                self.loop_depth += 1;
                let body = Box::new(self.parse_single_stmt()?);
                self.loop_depth -= 1;
                return Ok(self.stmt(StmtNode::ForOf {
                    left: Box::new(self.stmt(StmtNode::ExprStmt(e))),
                    right,
                    body,
                    is_await,
                }));
            }
            // Not for-in/for-of: regular for-loop with expression init.
            Self::reject_object_literal_assignment_cover(&e)?;
            Self::reject_duplicate_proto_object_literal(&e)?;
            // The expression may contain a comma sequence.
            let mut e = e;
            if self.check(&TokenKind::Comma) {
                let mut exprs = vec![e];
                while self.eat(&TokenKind::Comma) {
                    self.no_in = true;
                    exprs.push(self.parse_assign()?);
                    self.no_in = false;
                }
                e = Expr::Sequence(exprs);
            }
            Some(Box::new(self.stmt(StmtNode::ExprStmt(e))))
        };
        self.expect(&TokenKind::Semicolon, ";")?;
        let cond = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::Semicolon, ";")?;
        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::RParen, ")")?;
        self.loop_depth += 1;
        let body = Box::new(self.parse_single_stmt()?);
        self.loop_depth -= 1;
        if let Some(init_stmt) = &init {
            self.check_for_head_body_clash(&init_stmt.node, &body)?;
        }
        Ok(self.stmt(StmtNode::For {
            init,
            cond,
            update,
            body,
        }))
    }

    fn is_raw_of(&self) -> bool {
        self.is_raw_of_at(0)
    }

    fn is_raw_of_at(&self, offset: usize) -> bool {
        let tok = self.peek_at_tok(offset);
        matches!(tok.kind, TokenKind::Of) && !tok.had_escape
    }

    fn has_top_level_for_of_delimiter(&self) -> bool {
        let mut depth = 0usize;
        let mut offset = 0usize;
        loop {
            let tok = self.peek_at_tok(offset);
            match &tok.kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Eof => return false,
                TokenKind::Semicolon if depth == 0 => return false,
                TokenKind::In if depth == 0 => return false,
                TokenKind::Of if depth == 0 && offset > 0 && !tok.had_escape => return true,
                _ => {}
            }
            offset += 1;
        }
    }

    fn parse_var_decl_no_semi(&mut self) -> error::Result<Stmt> {
        let kind = match self.advance() {
            TokenKind::Var => VarKind::Var,
            TokenKind::Let => VarKind::Let,
            TokenKind::Const => VarKind::Const,
            _ => unreachable!(),
        };
        let mut decls = Vec::new();
        loop {
            // Destructuring pattern: `let [a,b] = ...` / `let {x,y} = ...`.
            if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                let pattern = self.parse_destructure_pattern()?;
                if kind != VarKind::Var {
                    check_pattern_disallows_let(&pattern)?;
                }
                // `for (let [a,b] of ...)` has no `=`; a plain decl requires one.
                let init = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                return Ok(self.stmt(StmtNode::Destructure {
                    kind,
                    pattern,
                    init,
                }));
            }
            let name = match self.advance() {
                TokenKind::Ident(s) => Arc::from(s.as_str()),
                TokenKind::Undefined => Arc::from("undefined"),
                TokenKind::Of => Arc::from("of"),
                TokenKind::Async => Arc::from("async"),
                TokenKind::Static if !self.is_strict_context => Arc::from("static"),
                TokenKind::Await if self.await_as_identifier_allowed() => Arc::from("await"),
                TokenKind::Yield if self.yield_as_identifier_allowed() => Arc::from("yield"),
                TokenKind::Let if kind == VarKind::Var && !self.is_strict_context => {
                    Arc::from("let")
                }
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected identifier, got {:?}",
                        other
                    )))
                }
            };
            self.check_binding_name(&name)?;
            let init = if self.eat(&TokenKind::Assign) {
                let mut e = self.parse_assign()?;
                Self::name_function_from_ident(&mut e, &name);
                Some(e)
            } else {
                // ES6: `const` declarations must have an initializer,
                // UNLESS this is a for-in/for-of head (`for (const x of/of ...)`).
                if kind == VarKind::Const
                    && !self.check(&TokenKind::In)
                    && !self.check(&TokenKind::Of)
                {
                    return Err(error::Error::syntax(
                        "Missing initializer in const declaration".to_string(),
                    ));
                }
                None
            };
            decls.push((name, init));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(self.stmt(StmtNode::VarDecl { kind, decls }))
    }

    fn parse_return(&mut self) -> error::Result<Stmt> {
        // `return` at the top level (outside any function) is a SyntaxError.
        if self.function_depth == 0 {
            return Err(error::Error::syntax("Illegal return statement".to_string()));
        }
        self.advance();
        if self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.check(&TokenKind::Eof)
            || self.at_newline_before()
        {
            self.eat(&TokenKind::Semicolon);
            return Ok(self.stmt(StmtNode::Return(None)));
        }
        let e = self.parse_expr()?;
        self.expect_semi()?;
        Ok(self.stmt(StmtNode::Return(Some(e))))
    }

    fn parse_try(&mut self) -> error::Result<Stmt> {
        self.advance();
        let try_body = Box::new(self.parse_block()?);
        let mut catch_param = None;
        let mut catch_body = None;
        let mut finally_body = None;
        if self.eat(&TokenKind::Catch) {
            if self.eat(&TokenKind::LParen) {
                let pat = self.parse_destructure_pattern()?;
                if !matches!(pat, Pattern::Ident(_)) {
                    check_pattern_disallows_let(&pat)?;
                }
                if self.is_strict_context {
                    check_pattern_strict(&pat)?;
                }
                self.expect(&TokenKind::RParen, ")")?;
                catch_param = Some(pat);
            }
            let body = self.parse_block()?;
            if let Some(param) = &catch_param {
                check_catch_parameter_early_errors(param, &body.node)?;
            }
            catch_body = Some(Box::new(body));
        }
        if self.eat(&TokenKind::Finally) {
            finally_body = Some(Box::new(self.parse_block()?));
        }
        // catch_body stays `None` when there is no `catch` clause; the
        // compiler must not push a catch handler in that case (otherwise an
        // empty catch silently swallows throws). The spec requires try/finally
        // with no catch to propagate exceptions through the finally block.
        Ok(self.stmt(StmtNode::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        }))
    }

    fn parse_switch(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let disc = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        self.expect(&TokenKind::LBrace, "{")?;
        self.switch_depth += 1;
        let mut cases = Vec::new();
        let mut seen_default = false;
        self.with_lexical_declaration_context(true, |p| {
            while !p.check(&TokenKind::RBrace) && !p.check(&TokenKind::Eof) {
                let test = if p.eat(&TokenKind::Case) {
                    Some(p.parse_expr()?)
                } else if p.eat(&TokenKind::Default) {
                    if seen_default {
                        return Err(error::Error::syntax(
                            "Duplicate default clause in switch".to_string(),
                        ));
                    }
                    seen_default = true;
                    None
                } else {
                    return Err(error::Error::syntax("Expected case or default".to_string()));
                };
                p.expect(&TokenKind::Colon, ":")?;
                let mut body = Vec::new();
                while !p.check(&TokenKind::Case)
                    && !p.check(&TokenKind::Default)
                    && !p.check(&TokenKind::RBrace)
                    && !p.check(&TokenKind::Eof)
                {
                    body.push(p.parse_stmt()?);
                }
                cases.push(SwitchCase { test, body });
            }
            Ok(())
        })?;
        self.expect(&TokenKind::RBrace, "}")?;
        self.switch_depth -= 1;
        // Static semantic early errors: detect duplicate lexical names
        // and lexical/var name clashes within a single switch CaseBlock.
        // (ES2025: LexicallyDeclaredNames of CaseBlock must not contain
        // duplicates, and must not intersect with VarDeclaredNames.)
        let mut lexical_names: Vec<Arc<str>> = Vec::new();
        let mut var_names: Vec<Arc<str>> = Vec::new();
        for case in &cases {
            for stmt in &case.body {
                collect_switch_decl_names(&stmt.node, &mut lexical_names, &mut var_names);
            }
        }
        for n in &lexical_names {
            if lexical_names.iter().filter(|x| *x == n).count() > 1 {
                return Err(error::Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    n
                )));
            }
            if var_names.contains(n) {
                return Err(error::Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    n
                )));
            }
        }
        Ok(self.stmt(StmtNode::Switch { disc, cases }))
    }

    // ---- Expressions (Pratt) ----

    fn parse_expr(&mut self) -> error::Result<Expr> {
        let mut e = self.parse_assign()?;
        if self.check(&TokenKind::Comma) {
            let mut exprs = vec![e];
            while self.eat(&TokenKind::Comma) {
                exprs.push(self.parse_assign()?);
            }
            e = Expr::Sequence(exprs);
        }
        Ok(e)
    }

    /// Maximum expression nesting depth. Generous for legitimate code (V8
    /// allows ~100 per `[]`/`{}` nesting), but bounded so untrusted deeply-
    /// nested input fails with a SyntaxError instead of overflowing the Rust
    /// parser stack and aborting the process.
    const MAX_EXPR_DEPTH: usize = 300;
    /// Maximum statement nesting depth. Bounds recursion through
    /// `parse_stmt` -> `parse_block`/`parse_if`/`parse_while`/`parse_for`/
    /// `parse_with` so deeply nested `{{...}}` or `if(1) if(1) ...` fails
    /// with a SyntaxError instead of aborting the process via stack overflow.
    const MAX_STMT_DEPTH: usize = 400;

    fn parse_assign(&mut self) -> error::Result<Expr> {
        if self.expr_depth >= Self::MAX_EXPR_DEPTH {
            return Err(error::Error::syntax(format!(
                "Maximum expression nesting depth ({}) exceeded",
                Self::MAX_EXPR_DEPTH
            )));
        }
        self.expr_depth += 1;
        let result = self.parse_assign_inner();
        self.expr_depth -= 1;
        result
    }

    fn parse_assign_inner(&mut self) -> error::Result<Expr> {
        if self.generator_depth > 0
            && self.yield_expression_allowed
            && matches!(self.peek(), TokenKind::Yield)
        {
            return self.parse_yield_expression();
        }

        let left_start = self.pos;
        let left = self.parse_ternary()?;
        let left_is_parenthesized_pattern = matches!(
            self.tokens.get(left_start).map(|t| &t.kind),
            Some(TokenKind::LParen)
        ) && matches!(left, Expr::Array(_) | Expr::Object(_));
        let left_is_parenthesized_ident = matches!(
            self.tokens.get(left_start).map(|t| &t.kind),
            Some(TokenKind::LParen)
        ) && matches!(left, Expr::Ident(_));
        let op = match self.peek() {
            TokenKind::Assign => AssignOp::Assign,
            TokenKind::PlusAssign => AssignOp::AddAssign,
            TokenKind::MinusAssign => AssignOp::SubAssign,
            TokenKind::StarAssign => AssignOp::MulAssign,
            TokenKind::SlashAssign => AssignOp::DivAssign,
            TokenKind::PercentAssign => AssignOp::ModAssign,
            TokenKind::StarStarAssign => AssignOp::PowAssign,
            TokenKind::AmpAssign => AssignOp::BitAndAssign,
            TokenKind::PipeAssign => AssignOp::BitOrAssign,
            TokenKind::CaretAssign => AssignOp::BitXorAssign,
            TokenKind::ShlAssign => AssignOp::ShlAssign,
            TokenKind::ShrAssign => AssignOp::ShrAssign,
            TokenKind::UshrAssign => AssignOp::UshrAssign,
            TokenKind::AndAssign => AssignOp::AndAssign,
            TokenKind::OrAssign => AssignOp::OrAssign,
            TokenKind::NullishAssign => AssignOp::NullishAssign,
            _ => {
                if self.defer_object_proto_duplicate_check == 0 {
                    Self::reject_object_literal_assignment_cover(&left)?;
                    Self::reject_duplicate_proto_object_literal(&left)?;
                }
                return Ok(left);
            }
        };
        self.advance();
        let mut right = self.parse_assign()?;
        if !matches!(op, AssignOp::Assign) && self.defer_object_proto_duplicate_check == 0 {
            Self::reject_duplicate_proto_object_literal(&left)?;
        }
        if !matches!(op, AssignOp::Assign) && matches!(left, Expr::Array(_) | Expr::Object(_)) {
            return Err(error::Error::syntax(
                "Invalid left-hand side in assignment".to_string(),
            ));
        }
        // Validate that the left side is a valid assignment target.
        // Invalid: literals, binary ops, unary ops, function calls, etc.
        if left_is_parenthesized_pattern || !Self::is_assignment_target(&left) {
            return Err(error::Error::syntax(
                "Invalid left-hand side in assignment".to_string(),
            ));
        }
        if matches!(op, AssignOp::Assign) {
            self.reject_array_rest_continuation_assignment_target(&left, left_start, self.pos)?;
        }
        // Strict mode: assignment to `eval` or `arguments` is a SyntaxError.
        if self.is_strict_context {
            Self::reject_strict_eval_arguments_assignment_target(&left)?;
        }
        // SetFunctionName for assignment applies only when the left side is a
        // bare IdentifierRef. Logical assignment has the same NamedEvaluation
        // path for anonymous function definitions; arithmetic compound
        // assignments do not.
        if matches!(
            op,
            AssignOp::Assign | AssignOp::AndAssign | AssignOp::OrAssign | AssignOp::NullishAssign
        ) && !left_is_parenthesized_ident
        {
            if let Some(key_name) = Self::assign_target_name(&left) {
                Self::name_function_from_ident(&mut right, &key_name);
            }
        }
        if matches!(op, AssignOp::Assign) && self.defer_object_proto_duplicate_check == 0 {
            Self::reject_duplicate_proto_assignment_pattern(&left)?;
        }
        Ok(Expr::Assign(op, Box::new(left), Box::new(right)))
    }

    fn parse_yield_expression(&mut self) -> error::Result<Expr> {
        self.advance();

        // Both YieldExpression productions have a no-LineTerminator-here
        // restriction after `yield`. A line break therefore makes this the
        // operand-less form and leaves the following token for ASI.
        if self.at_newline_before() {
            return Ok(Expr::Yield(None));
        }

        if self.eat(&TokenKind::Star) {
            return Ok(Expr::YieldDelegate(Box::new(self.parse_assign()?)));
        }

        let inner = if matches!(
            self.peek(),
            TokenKind::Semicolon
                | TokenKind::RBrace
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::Comma
                | TokenKind::Colon
                | TokenKind::TemplateExprEnd
                | TokenKind::Eof
        ) {
            None
        } else {
            Some(Box::new(self.parse_assign()?))
        };
        Ok(Expr::Yield(inner))
    }

    fn is_assignment_target(target: &Expr) -> bool {
        if Self::is_import_meta(target) {
            return false;
        }
        match target {
            Expr::Ident(_) | Expr::PrivateGet { .. } => true,
            Expr::Member {
                optional,
                optional_chain,
                ..
            } => !*optional && !*optional_chain,
            Expr::Array(_) | Expr::Object(_) => Self::is_assignment_pattern(target),
            _ => false,
        }
    }

    fn is_simple_assignment_target(target: &Expr) -> bool {
        match target {
            Expr::Ident(_) | Expr::PrivateGet { .. } => true,
            Expr::Member {
                optional,
                optional_chain,
                ..
            } => !*optional && !*optional_chain,
            _ => false,
        }
    }

    fn reject_strict_eval_arguments_assignment_target(target: &Expr) -> error::Result<()> {
        match target {
            Expr::Ident(id) if matches!(&**id, "eval" | "arguments") => Err(error::Error::syntax(
                format!("Assignment to '{}' is not allowed in strict mode", id),
            )),
            Expr::Array(elements) => {
                for element in elements {
                    match element {
                        Expr::ArrayHole => {}
                        Expr::Spread(inner) => {
                            Self::reject_strict_eval_arguments_assignment_target(inner)?
                        }
                        Expr::Assign(AssignOp::Assign, left, _) => {
                            Self::reject_strict_eval_arguments_assignment_target(left)?
                        }
                        other => Self::reject_strict_eval_arguments_assignment_target(other)?,
                    }
                }
                Ok(())
            }
            Expr::Object(props) => {
                for prop in props {
                    match &prop.key {
                        PropertyKey::Spread(expr) => {
                            Self::reject_strict_eval_arguments_assignment_target(expr)?
                        }
                        _ => Self::reject_strict_eval_arguments_assignment_target(&prop.value)?,
                    }
                }
                Ok(())
            }
            Expr::Assign(AssignOp::Assign, left, _) => {
                Self::reject_strict_eval_arguments_assignment_target(left)
            }
            Expr::Spread(inner) => Self::reject_strict_eval_arguments_assignment_target(inner),
            Expr::Ident(_) | Expr::Member { .. } | Expr::PrivateGet { .. } => Ok(()),
            _ => Ok(()),
        }
    }

    fn is_import_meta(target: &Expr) -> bool {
        matches!(target, Expr::ImportMeta)
    }

    fn is_for_in_of_assignment_target(target: &Expr) -> bool {
        Self::is_assignment_target(target)
    }

    fn is_assignment_pattern(target: &Expr) -> bool {
        match target {
            Expr::Ident(_) | Expr::PrivateGet { .. } => true,
            Expr::Member {
                optional,
                optional_chain,
                ..
            } => !*optional && !*optional_chain,
            Expr::Assign(AssignOp::Assign, left, _) => Self::is_assignment_pattern(left),
            Expr::Array(elements) => {
                elements
                    .iter()
                    .enumerate()
                    .all(|(idx, element)| match element {
                        Expr::ArrayHole => true,
                        Expr::Spread(inner) => {
                            idx + 1 == elements.len()
                                && !matches!(inner.as_ref(), Expr::Assign(AssignOp::Assign, _, _))
                                && Self::is_assignment_pattern(inner)
                        }
                        other => Self::is_assignment_pattern(other),
                    })
            }
            Expr::Object(props) => props.iter().enumerate().all(|(idx, prop)| {
                if prop.method
                    || matches!(prop.kind, PropKind::Method | PropKind::Get | PropKind::Set)
                {
                    return false;
                }
                match &prop.key {
                    PropertyKey::Spread(expr) => {
                        idx + 1 == props.len() && Self::is_assignment_pattern(expr)
                    }
                    _ => Self::is_assignment_pattern(&prop.value),
                }
            }),
            _ => false,
        }
    }

    fn reject_array_rest_continuation_assignment_target(
        &self,
        target: &Expr,
        start: usize,
        end: usize,
    ) -> error::Result<()> {
        if !matches!(target, Expr::Array(_)) {
            return Ok(());
        }

        let mut i = start;
        while i < end && matches!(&self.tokens[i].kind, TokenKind::LParen) {
            i += 1;
        }
        if i >= end || !matches!(&self.tokens[i].kind, TokenKind::LBracket) {
            return Ok(());
        }

        let mut depth = 0usize;
        let mut saw_top_level_rest = false;
        while i < end {
            match &self.tokens[i].kind {
                TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
                TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                    if depth == 1 && matches!(&self.tokens[i].kind, TokenKind::RBracket) {
                        break;
                    }
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Spread if depth == 1 => saw_top_level_rest = true,
                TokenKind::Comma if depth == 1 && saw_top_level_rest => {
                    return Err(error::Error::syntax(
                        "rest element must be last in array pattern".to_string(),
                    ));
                }
                _ => {}
            }
            i += 1;
        }
        Ok(())
    }

    fn reject_object_literal_assignment_cover(expr: &Expr) -> error::Result<()> {
        match expr {
            Expr::Object(props) => {
                for prop in props {
                    if prop.shorthand && matches!(prop.value, Expr::Assign(AssignOp::Assign, _, _))
                    {
                        return Err(error::Error::syntax(
                            "Invalid shorthand property initializer".to_string(),
                        ));
                    }
                    Self::reject_object_literal_assignment_cover(&prop.value)?;
                    match &prop.key {
                        PropertyKey::Computed(expr) | PropertyKey::Spread(expr) => {
                            Self::reject_object_literal_assignment_cover(expr)?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Expr::Array(elements) | Expr::Sequence(elements) => {
                for element in elements {
                    Self::reject_object_literal_assignment_cover(element)?;
                }
                Ok(())
            }
            Expr::Assign(_, left, right)
            | Expr::Binary(_, left, right)
            | Expr::Logical(_, left, right) => {
                Self::reject_object_literal_assignment_cover(left)?;
                Self::reject_object_literal_assignment_cover(right)
            }
            Expr::Conditional(cond, then_expr, else_expr) => {
                Self::reject_object_literal_assignment_cover(cond)?;
                Self::reject_object_literal_assignment_cover(then_expr)?;
                Self::reject_object_literal_assignment_cover(else_expr)
            }
            Expr::Call { callee, args, .. } | Expr::New { callee, args } => {
                Self::reject_object_literal_assignment_cover(callee)?;
                for arg in args {
                    Self::reject_object_literal_assignment_cover(arg)?;
                }
                Ok(())
            }
            Expr::ImportCall { specifier, options } => {
                Self::reject_object_literal_assignment_cover(specifier)?;
                if let Some(options) = options {
                    Self::reject_object_literal_assignment_cover(options)?;
                }
                Ok(())
            }
            Expr::Member {
                object, property, ..
            } => {
                Self::reject_object_literal_assignment_cover(object)?;
                Self::reject_object_literal_assignment_cover(property)
            }
            Expr::PrivateGet { object, .. } => Self::reject_object_literal_assignment_cover(object),
            Expr::PrivateSet { object, value, .. } | Expr::PrivateInit { object, value, .. } => {
                Self::reject_object_literal_assignment_cover(object)?;
                Self::reject_object_literal_assignment_cover(value)
            }
            Expr::PrivateDefineAccessor {
                object, get, set, ..
            } => {
                Self::reject_object_literal_assignment_cover(object)?;
                if let Some(get) = get {
                    Self::reject_object_literal_assignment_cover(get)?;
                }
                if let Some(set) = set {
                    Self::reject_object_literal_assignment_cover(set)?;
                }
                Ok(())
            }
            Expr::PrivateFieldDecl {
                init: Some(init), ..
            } => Self::reject_object_literal_assignment_cover(init),
            Expr::TaggedTemplate { tag, exprs, .. } => {
                Self::reject_object_literal_assignment_cover(tag)?;
                for expr in exprs {
                    Self::reject_object_literal_assignment_cover(expr)?;
                }
                Ok(())
            }
            Expr::TemplateInterp { exprs, .. } => {
                for expr in exprs {
                    Self::reject_object_literal_assignment_cover(expr)?;
                }
                Ok(())
            }
            Expr::Spread(expr)
            | Expr::Unary(_, expr)
            | Expr::Update(_, _, expr)
            | Expr::Await(expr)
            | Expr::YieldDelegate(expr)
            | Expr::Yield(Some(expr)) => Self::reject_object_literal_assignment_cover(expr),
            Expr::Function(_) | Expr::Arrow(_) | Expr::Class(_) => Ok(()),
            _ => Ok(()),
        }
    }

    /// Extract the name used by assignment SetFunctionName. Per spec this is
    /// only a bare IdentifierRef, not a member expression or parenthesized
    /// identifier.
    fn assign_target_name(target: &Expr) -> Option<Arc<str>> {
        match target {
            Expr::Ident(s) => Some(s.clone()),
            _ => None,
        }
    }

    fn parse_ternary(&mut self) -> error::Result<Expr> {
        let cond = self.parse_nullish()?;
        if self.eat(&TokenKind::Question) {
            let then = self.with_in_allowed(|p| p.parse_assign())?;
            self.expect(&TokenKind::Colon, ":")?;
            let else_ = self.parse_assign()?;
            Ok(Expr::Conditional(
                Box::new(cond),
                Box::new(then),
                Box::new(else_),
            ))
        } else {
            Ok(cond)
        }
    }

    fn parse_nullish(&mut self) -> error::Result<Expr> {
        let left = self.parse_bit_or()?;
        if self.check(&TokenKind::Nullish) {
            return self.parse_nullish_tail(left);
        }
        let left = self.parse_logical_and_tail(left)?;
        self.parse_logical_or_tail(left)
    }

    fn parse_nullish_tail(&mut self, mut left: Expr) -> error::Result<Expr> {
        while self.check(&TokenKind::Nullish) {
            self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::Logical(LogicalOp::Nullish, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_or(&mut self) -> error::Result<Expr> {
        let left = self.parse_logical_and()?;
        self.parse_logical_or_tail(left)
    }

    fn parse_logical_or_tail(&mut self, mut left: Expr) -> error::Result<Expr> {
        while self.check(&TokenKind::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::Logical(LogicalOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> error::Result<Expr> {
        let left = self.parse_bit_or()?;
        self.parse_logical_and_tail(left)
    }

    fn parse_logical_and_tail(&mut self, mut left: Expr) -> error::Result<Expr> {
        while self.check(&TokenKind::And) {
            self.advance();
            let right = self.parse_bit_or()?;
            left = Expr::Logical(LogicalOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_bit_xor()?;
        while self.check(&TokenKind::BitOr) {
            self.advance();
            let right = self.parse_bit_xor()?;
            left = Expr::Binary(BinOp::BitOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_bit_and()?;
        while self.check(&TokenKind::BitXor) {
            self.advance();
            let right = self.parse_bit_and()?;
            left = Expr::Binary(BinOp::BitXor, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_equality()?;
        while self.check(&TokenKind::BitAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary(BinOp::BitAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_relational()?;
        loop {
            let op = match self.peek() {
                TokenKind::Eq => BinOp::Eq,
                TokenKind::NotEq => BinOp::NotEq,
                TokenKind::EqEqEq => BinOp::StrictEq,
                TokenKind::NotEqEqEq => BinOp::StrictNotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_relational()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_shift()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Lte => BinOp::Lte,
                TokenKind::Gte => BinOp::Gte,
                TokenKind::Instanceof => BinOp::Instanceof,
                TokenKind::In if !self.no_in => BinOp::In,
                _ => break,
            };
            self.advance();
            let right = self.parse_shift()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                TokenKind::Shl => BinOp::Shl,
                TokenKind::Shr => BinOp::Shr,
                TokenKind::Ushr => BinOp::Ushr,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_exponent()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_exponent()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_exponent(&mut self) -> error::Result<Expr> {
        // Record position before parsing the left operand so we can detect
        // whether a unary operator was used directly (not parenthesized).
        let pos_before = self.pos;
        let left = self.parse_unary()?;
        if self.check(&TokenKind::StarStar) {
            self.advance();
            let right = self.parse_exponent()?; // right-assoc
                                                // ES spec: unary operators cannot be the base of ** unless
                                                // parenthesized. If the left is a Unary and the first token
                                                // we parsed was a unary operator (not LParen), it is a syntax error.
            if let Expr::Unary(_, _) = &left {
                let first_tok = self.tokens.get(pos_before).map(|t| &t.kind);
                // If the first token was LParen, the unary was inside parens.
                if !matches!(first_tok, Some(TokenKind::LParen)) {
                    return Err(error::Error::syntax(
                        "Unary operator used before exponentiation operator".to_string(),
                    ));
                }
            }
            return Ok(Expr::Binary(BinOp::Pow, Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> error::Result<Expr> {
        // Bound prefix-unary recursion (`!!!!...x`, `----x`, `typeof typeof
        // ... x`) which self-recurses without going through `parse_assign`
        // and so would otherwise bypass `MAX_EXPR_DEPTH`.
        if self.expr_depth >= Self::MAX_EXPR_DEPTH {
            return Err(error::Error::syntax(format!(
                "Maximum expression nesting depth ({}) exceeded",
                Self::MAX_EXPR_DEPTH
            )));
        }
        self.expr_depth += 1;
        let result = self.parse_unary_inner();
        self.expr_depth -= 1;
        result
    }

    fn parse_unary_inner(&mut self) -> error::Result<Expr> {
        // prefix ++/--
        if matches!(self.peek(), TokenKind::Inc | TokenKind::Dec) {
            let op = if matches!(self.peek(), TokenKind::Inc) {
                UpdateOp::Inc
            } else {
                UpdateOp::Dec
            };
            self.advance();
            let e = self.parse_unary()?;
            // Validate: only identifiers and member expressions are valid
            // update targets. Call expressions, literals, etc. are not.
            if !Self::is_simple_assignment_target(&e) {
                return Err(error::Error::syntax(
                    "Invalid left-hand side expression in prefix update operation".to_string(),
                ));
            }
            // Strict mode: eval/arguments cannot be the operand of update.
            if self.is_strict_context {
                if let Expr::Ident(ref id) = e {
                    if matches!(&**id, "eval" | "arguments") {
                        return Err(error::Error::syntax(format!(
                            "'{}' cannot be used as the target of an update operator in strict mode",
                            id
                        )));
                    }
                }
            }
            return Ok(Expr::Update(op, true, Box::new(e)));
        }
        let op = match self.peek() {
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Plus => Some(UnOp::Plus),
            TokenKind::Not => Some(UnOp::Not),
            TokenKind::BitNot => Some(UnOp::BitNot),
            TokenKind::Typeof => Some(UnOp::Typeof),
            TokenKind::Void => Some(UnOp::Void),
            TokenKind::Delete => Some(UnOp::Delete),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            if self.generator_depth > 0 && matches!(self.peek(), TokenKind::Yield) {
                return Err(error::Error::syntax(
                    "Yield expression cannot be used directly as a unary operand".to_string(),
                ));
            }
            let e = self.parse_unary()?;
            let deletes_private = matches!(e, Expr::PrivateGet { .. })
                || matches!(
                    &e,
                    Expr::OptionalChain(inner)
                        if matches!(inner.as_ref(), Expr::PrivateGet { .. })
                );
            if self.is_strict_context && matches!(op, UnOp::Delete) && deletes_private {
                return Err(error::Error::syntax(
                    "Cannot delete private field".to_string(),
                ));
            }
            return Ok(Expr::Unary(op, Box::new(e)));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> error::Result<Expr> {
        let mut e = self.parse_call()?;
        // postfix ++/--
        if matches!(self.peek(), TokenKind::Inc | TokenKind::Dec) {
            // Validate: only identifiers and member expressions are valid
            // update targets. Call expressions, literals, etc. are not.
            if !Self::is_simple_assignment_target(&e) {
                return Err(error::Error::syntax(
                    "Invalid left-hand side expression in postfix operation".to_string(),
                ));
            }
            // Strict mode: eval/arguments cannot be the operand of update.
            if self.is_strict_context {
                if let Expr::Ident(ref id) = e {
                    if matches!(&**id, "eval" | "arguments") {
                        return Err(error::Error::syntax(format!(
                            "'{}' cannot be used as the target of an update operator in strict mode",
                            id
                        )));
                    }
                }
            }
            let op = if matches!(self.peek(), TokenKind::Inc) {
                UpdateOp::Inc
            } else {
                UpdateOp::Dec
            };
            // Postfix update: no LineTerminator allowed between operand and operator.
            if self
                .tokens
                .get(self.pos)
                .map(|t| t.preceded_by_newline)
                .unwrap_or(false)
            {
                return Ok(e);
            }
            self.advance();
            e = Expr::Update(op, false, Box::new(e));
        }
        Ok(e)
    }

    fn parse_call(&mut self) -> error::Result<Expr> {
        self.last_primary_parenthesized = false;
        let mut e = self.parse_primary()?;
        let mut parenthesized_base = self.last_primary_parenthesized;
        self.last_primary_parenthesized = false;
        loop {
            match self.peek().clone() {
                TokenKind::Dot => {
                    self.advance();
                    // Private field access: obj.#field
                    if let TokenKind::PrivateName(name) = self.peek().clone() {
                        self.advance();
                        e = Expr::PrivateGet {
                            object: Box::new(e),
                            name: Arc::from(name.as_str()),
                            optional: false,
                        };
                    } else {
                        let name = self.read_property_name()?;
                        let prop = Expr::String(Arc::from(name.as_str()));
                        let optional_chain =
                            !parenthesized_base && Self::continues_optional_chain(&e);
                        e = Expr::Member {
                            object: Box::new(e),
                            property: Box::new(prop),
                            computed: false,
                            optional: false,
                            optional_chain,
                        };
                    }
                    parenthesized_base = false;
                }
                TokenKind::QuestionDot => {
                    self.advance();
                    match self.peek().clone() {
                        TokenKind::LParen => {
                            self.advance();
                            let args = self.parse_args()?;
                            self.expect(&TokenKind::RParen, ")")?;
                            e = Expr::Call {
                                callee: Box::new(e),
                                args,
                                optional: true,
                                optional_chain: true,
                            };
                        }
                        TokenKind::LBracket => {
                            self.advance();
                            let prop = self.with_in_allowed(|p| p.parse_expr())?;
                            self.expect(&TokenKind::RBracket, "]")?;
                            e = Expr::Member {
                                object: Box::new(e),
                                property: Box::new(prop),
                                computed: true,
                                optional: true,
                                optional_chain: true,
                            };
                        }
                        TokenKind::PrivateName(name) => {
                            self.advance();
                            e = Expr::PrivateGet {
                                object: Box::new(e),
                                name: Arc::from(name.as_str()),
                                optional: true,
                            };
                        }
                        _ => {
                            let name = self.read_property_name()?;
                            let prop = Expr::String(Arc::from(name.as_str()));
                            e = Expr::Member {
                                object: Box::new(e),
                                property: Box::new(prop),
                                computed: false,
                                optional: true,
                                optional_chain: true,
                            };
                        }
                    }
                    parenthesized_base = false;
                }
                TokenKind::LBracket => {
                    self.advance();
                    let prop = self.with_in_allowed(|p| p.parse_expr())?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    let optional_chain = !parenthesized_base && Self::continues_optional_chain(&e);
                    e = Expr::Member {
                        object: Box::new(e),
                        property: Box::new(prop),
                        computed: true,
                        optional: false,
                        optional_chain,
                    };
                    parenthesized_base = false;
                }
                TokenKind::LParen => {
                    if matches!(e, Expr::Super) && self.super_call_depth == 0 {
                        return Err(error::Error::syntax("super call unexpected here"));
                    }
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&TokenKind::RParen, ")")?;
                    let optional_chain = !parenthesized_base && Self::continues_optional_chain(&e);
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        optional: false,
                        optional_chain,
                    };
                    parenthesized_base = false;
                }
                TokenKind::TemplateString { cooked, raw } => {
                    if !parenthesized_base && Self::continues_optional_chain(&e) {
                        return Err(error::Error::syntax(
                            "Optional chain cannot be used as a tagged template".to_string(),
                        ));
                    }
                    // Tagged template: tag`str${expr}str`
                    let quasi0: Option<Arc<str>> = cooked.map(|s| Arc::from(s.as_str()));
                    let raw0: Arc<str> = Arc::from(raw.as_str());
                    self.advance(); // consume the TemplateString token
                    let tag = e;
                    e = self.parse_tagged_template(tag, quasi0, raw0)?;
                    parenthesized_base = false;
                }
                _ => break,
            }
        }
        if Self::continues_optional_chain(&e) {
            Ok(Expr::OptionalChain(Box::new(e)))
        } else {
            Ok(e)
        }
    }

    fn continues_optional_chain(expr: &Expr) -> bool {
        match expr {
            Expr::Member { optional_chain, .. } | Expr::Call { optional_chain, .. } => {
                *optional_chain
            }
            Expr::PrivateGet {
                object, optional, ..
            } => *optional || Self::continues_optional_chain(object),
            _ => false,
        }
    }

    fn parse_args(&mut self) -> error::Result<Vec<Expr>> {
        let mut args = Vec::new();
        while !self.check(&TokenKind::RParen) {
            if self.check(&TokenKind::Spread) {
                self.advance();
                args.push(Expr::Spread(Box::new(self.parse_assign()?)));
            } else {
                args.push(self.parse_assign()?);
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> error::Result<Expr> {
        // Dedicated lexer tokens such as `undefined`, `of`, and `static` are
        // still BindingIdentifiers where the grammar permits them.
        if matches!(self.peek_at_tok(1).kind, TokenKind::Arrow)
            && self.binding_identifier_name().is_some()
        {
            if self.peek_at_tok(1).preceded_by_newline {
                return Err(error::Error::syntax(
                    "Line terminator not allowed before =>".to_string(),
                ));
            }
            let name = self.parse_binding_identifier("Expected arrow parameter")?;
            self.arrow_defaults = Vec::new();
            self.arrow_rest = None;
            self.advance(); // =>
            return self.parse_arrow_body(vec![name]);
        }
        match self.peek().clone() {
            TokenKind::Await => {
                if self.async_depth > 0 && !self.await_expression_allowed {
                    return Err(error::Error::syntax(
                        "Await expression is not valid in formal parameters".to_string(),
                    ));
                }
                if self.await_as_identifier_allowed() && self.await_token_is_identifier_ref() {
                    self.advance();
                    return Ok(Expr::Ident(Arc::from("await")));
                }
                let top_level_await = self.nested_function_context_depth == 0;
                if self.async_depth == 0 && !top_level_await {
                    return Err(error::Error::syntax(
                        "Await expression is not valid in this context".to_string(),
                    ));
                }
                self.advance();
                let inner = self.parse_unary()?;
                Ok(Expr::Await(Box::new(inner)))
            }
            TokenKind::Yield => {
                if self.yield_as_identifier_allowed() {
                    self.advance();
                    return Ok(Expr::Ident(Arc::from("yield")));
                }
                Err(error::Error::syntax(
                    "Yield expression is not valid in this expression position".to_string(),
                ))
            }
            TokenKind::Async => {
                // `async function ...` expression; `async () =>` arrow; otherwise
                // `async` is treated as a plain identifier.
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function)
                    && !self.peek_at_tok(1).preceded_by_newline
                {
                    self.advance(); // async
                    return self.parse_function_expr_with_async(true);
                }
                // async arrow: `async (params) => body` or `async ident => body`
                let is_async_arrow_paren = !self.peek_at_tok(1).preceded_by_newline
                    && matches!(self.peek_at_tok(1).kind, TokenKind::LParen);
                let is_async_arrow_ident = self
                    .binding_identifier_name_at(1)
                    .is_some_and(|name| name.as_ref() != "await")
                    && !self.peek_at_tok(1).preceded_by_newline
                    && matches!(self.peek_at_tok(2).kind, TokenKind::Arrow);
                if is_async_arrow_paren {
                    self.advance(); // async
                                    // Now at `(`; parse like a parenthesized arrow.
                    self.advance(); // (
                    if self.current_parenthesized_group_is_arrow_head()
                        && self.with_async_context(true, |p| {
                            p.with_formal_parameter_expression_context(|p| {
                                p.try_parse_arrow_params()
                            })
                        })?
                    {
                        let params = self.last_arrow_params.take().unwrap();
                        self.expect(&TokenKind::Arrow, "=>")?;
                        return self.parse_arrow_body_with_async(params, true);
                    }
                    // Not an arrow; rewind and treat async as identifier.
                    self.pos -= 2;
                    self.advance();
                    return Ok(Expr::Ident(Arc::from("async")));
                }
                if is_async_arrow_ident {
                    if self.peek_at_tok(2).preceded_by_newline {
                        self.advance();
                        return Ok(Expr::Ident(Arc::from("async")));
                    }
                    self.advance(); // async
                    let name = self.with_async_context(true, |p| {
                        p.parse_binding_identifier("Expected async arrow parameter")
                    })?;
                    self.advance(); // =>
                    return self.parse_arrow_body_with_async(vec![name], true);
                }
                // fall through to identifier
                self.advance();
                Ok(Expr::Ident(Arc::from("async")))
            }
            TokenKind::Regex(pat, flags) => {
                self.advance();
                Ok(Expr::Regex(
                    Arc::from(pat.as_str()),
                    Arc::from(flags.as_str()),
                ))
            }
            TokenKind::Slash => self.parse_slash_regex_literal(),
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::LegacyNumber(n) => {
                if self.is_strict_context {
                    return Err(error::Error::syntax(
                        "Legacy numeric literals are not allowed in strict mode",
                    ));
                }
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::BigInt(s) => {
                self.advance();
                let n = num_bigint::BigInt::parse_bytes(s.as_bytes(), 10).unwrap_or_default();
                Ok(Expr::BigInt(n))
            }
            TokenKind::String(s) => {
                if self.is_strict_context && self.peek_at_tok(0).string_had_legacy_escape {
                    return Err(error::Error::syntax(
                        "Legacy string escapes are not allowed in strict mode",
                    ));
                }
                self.advance();
                Ok(Expr::String(Arc::from(s.as_str())))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            TokenKind::Undefined => {
                self.advance();
                Ok(Expr::Ident(Arc::from("undefined")))
            }
            TokenKind::This => {
                self.advance();
                Ok(Expr::This)
            }
            TokenKind::Super => {
                if self.super_depth == 0 {
                    return Err(error::Error::syntax("super keyword unexpected here"));
                }
                self.advance();
                Ok(Expr::Super)
            }
            TokenKind::Class => {
                // Class expression: `var C = class C { ... }`
                self.parse_class_body(false).map(Expr::Class)
            }
            TokenKind::At => {
                let decorators = self.parse_decorator_list()?;
                if !self.check(&TokenKind::Class) {
                    return Err(error::Error::syntax(
                        "Class decorators must be followed by a class".to_string(),
                    ));
                }
                self.parse_class_body_with_decorators(false, decorators)
                    .map(Expr::Class)
            }
            TokenKind::Ident(s) => {
                if s == "import" && !self.peek_at_tok(0).had_escape {
                    if matches!(self.peek_at_tok(1).kind, TokenKind::Dot)
                        && matches!(
                            &self.peek_at_tok(2).kind,
                            TokenKind::Ident(name) if name == "meta"
                        )
                        && !self.peek_at_tok(2).had_escape
                    {
                        if self.source_type != SourceType::Module {
                            return Err(error::Error::syntax(
                                "import.meta is only valid in module code",
                            ));
                        }
                        self.advance();
                        self.advance();
                        self.advance();
                        return Ok(Expr::ImportMeta);
                    }
                    if !matches!(self.peek_at_tok(1).kind, TokenKind::LParen) {
                        return Err(error::Error::syntax(
                            "import must be followed by a call or meta-property",
                        ));
                    }
                    self.advance();
                    self.advance();
                    if self.check(&TokenKind::RParen) {
                        return Err(error::Error::syntax(
                            "Dynamic import requires a module specifier",
                        ));
                    }
                    let specifier = self.with_in_allowed(|p| p.parse_assign())?;
                    let options = if self.eat(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) {
                            None
                        } else {
                            let options =
                                Some(Box::new(self.with_in_allowed(|p| p.parse_assign())?));
                            self.eat(&TokenKind::Comma);
                            options
                        }
                    } else {
                        None
                    };
                    self.expect(&TokenKind::RParen, ")")?;
                    return Ok(Expr::ImportCall {
                        specifier: Box::new(specifier),
                        options,
                    });
                }
                if (s == "await" && !self.await_as_identifier_allowed())
                    || (s == "yield" && !self.yield_as_identifier_allowed())
                {
                    return Err(error::Error::syntax(format!(
                        "'{}' cannot be used as an identifier here",
                        s
                    )));
                }
                if Self::is_reserved_identifier_reference_word(&s) {
                    return Err(error::Error::syntax(format!(
                        "'{}' is a reserved word and cannot be used as an identifier",
                        s
                    )));
                }
                // Strict mode: FutureReservedWords cannot be used as identifiers.
                if self.is_strict_context && Self::is_strict_identifier_reference_reserved(&s) {
                    return Err(error::Error::syntax(format!(
                        "'{}' is a reserved word in strict mode",
                        s
                    )));
                }
                self.advance();
                Ok(Expr::Ident(Arc::from(s.as_str())))
            }
            TokenKind::Let if !self.is_strict_context => {
                // In non-strict mode, `let` can be used as an identifier
                // (e.g. `for (let in obj)` or `let = 1`).
                self.advance();
                Ok(Expr::Ident(Arc::from("let")))
            }
            TokenKind::Of => {
                // `of` is a contextual keyword, usable as an identifier
                // outside of for-of heads.
                self.advance();
                Ok(Expr::Ident(Arc::from("of")))
            }
            TokenKind::Static if !self.is_strict_context => {
                self.advance();
                Ok(Expr::Ident(Arc::from("static")))
            }
            TokenKind::Get => {
                self.advance();
                Ok(Expr::Ident(Arc::from("get")))
            }
            TokenKind::Set => {
                self.advance();
                Ok(Expr::Ident(Arc::from("set")))
            }
            TokenKind::LParen => {
                // Could be arrow: (a, b) => ...
                self.advance();
                if self.current_parenthesized_group_is_arrow_head()
                    && self
                        .with_formal_parameter_expression_context(|p| p.try_parse_arrow_params())?
                {
                    let params = self.last_arrow_params.take().unwrap();
                    self.expect(&TokenKind::Arrow, "=>")?;
                    return self.parse_arrow_body(params);
                }
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
                self.last_primary_parenthesized = true;
                Ok(e)
            }
            TokenKind::LBracket => self.parse_array(),
            TokenKind::LBrace => self.parse_object(),
            TokenKind::Function => self.parse_function_expr(),
            TokenKind::New => self.parse_new(),
            TokenKind::TemplateString { cooked, .. } => {
                self.advance();
                let cooked = cooked.ok_or_else(|| {
                    error::Error::syntax("Invalid escape sequence in template literal".to_string())
                })?;
                self.parse_template_rest(Arc::from(cooked.as_str()))
            }
            other => Err(error::Error::syntax(format!(
                "Unexpected token in expression: {:?}",
                other
            ))),
        }
    }

    fn parse_slash_regex_literal(&mut self) -> error::Result<Expr> {
        self.expect(&TokenKind::Slash, "/")?;
        let mut pattern = String::new();
        while !self.check(&TokenKind::Slash) {
            if self.check(&TokenKind::Eof) {
                return Err(error::Error::syntax(
                    "unterminated regular expression literal".to_string(),
                ));
            }
            pattern.push_str(&Self::regex_token_fragment(self.peek())?);
            self.advance();
        }
        self.expect(&TokenKind::Slash, "/")?;

        let mut flags = String::new();
        loop {
            if self.peek_at_tok(0).had_escape {
                return Err(error::Error::syntax(
                    "invalid escaped regular expression flag".to_string(),
                ));
            }
            match self.peek().clone() {
                TokenKind::Ident(s) => {
                    flags.push_str(&s);
                    self.advance();
                }
                TokenKind::Number(number) | TokenKind::LegacyNumber(number) => {
                    if number.fract() == 0.0 {
                        flags.push_str(&format!("{number:.0}"));
                    } else {
                        flags.push_str(&number.to_string());
                    }
                    self.advance();
                }
                other if other.as_keyword_str().is_some() => {
                    flags.push_str(other.as_keyword_str().unwrap());
                    self.advance();
                }
                _ => break,
            }
        }

        crate::lexer::validate_regex_flags_for_constructor(&flags).map_err(error::Error::syntax)?;
        crate::lexer::validate_regex_pattern_for_constructor(&pattern, &flags).map_err(
            |error| match error {
                crate::lexer::RegexValidationError::Syntax(message) => {
                    error::Error::syntax(message)
                }
                crate::lexer::RegexValidationError::Resource(message) => {
                    error::Error::range(message)
                }
            },
        )?;
        Ok(Expr::Regex(Arc::from(pattern), Arc::from(flags)))
    }

    fn regex_token_fragment(tok: &TokenKind) -> error::Result<String> {
        let fragment = match tok {
            TokenKind::Number(n) | TokenKind::LegacyNumber(n) => {
                if n.fract() == 0.0 {
                    format!("{:.0}", n)
                } else {
                    n.to_string()
                }
            }
            TokenKind::BigInt(s) | TokenKind::Ident(s) | TokenKind::String(s) => s.clone(),
            other if other.as_keyword_str().is_some() => other.as_keyword_str().unwrap().into(),
            TokenKind::Plus => "+".into(),
            TokenKind::Minus => "-".into(),
            TokenKind::Star => "*".into(),
            TokenKind::Percent => "%".into(),
            TokenKind::Dot => ".".into(),
            TokenKind::LParen => "(".into(),
            TokenKind::RParen => ")".into(),
            TokenKind::LBracket => "[".into(),
            TokenKind::RBracket => "]".into(),
            TokenKind::LBrace => "{".into(),
            TokenKind::RBrace => "}".into(),
            TokenKind::Question => "?".into(),
            TokenKind::Colon => ":".into(),
            TokenKind::Comma => ",".into(),
            TokenKind::BitAnd => "&".into(),
            TokenKind::BitOr => "|".into(),
            TokenKind::BitXor => "^".into(),
            TokenKind::BitNot => "~".into(),
            TokenKind::Lt => "<".into(),
            TokenKind::Gt => ">".into(),
            TokenKind::Assign => "=".into(),
            TokenKind::Not => "!".into(),
            TokenKind::Regex(pattern, flags) => format!("{}/{}", pattern, flags),
            other => {
                return Err(error::Error::syntax(format!(
                    "Unexpected token in regular expression literal: {:?}",
                    other
                )))
            }
        };
        Ok(fragment)
    }

    /// Finish parsing a template literal after consuming its first `TemplateString` quasi.
    /// If followed by `${ ... }` interpolations, build an interpolated template; otherwise
    /// it is a plain string literal.
    fn parse_template_rest(&mut self, first: Arc<str>) -> error::Result<Expr> {
        if !self.check(&TokenKind::TemplateExprStart) {
            // No interpolation: plain string.
            return Ok(Expr::String(first));
        }
        let mut quasis: Vec<Arc<str>> = vec![first];
        let mut exprs: Vec<Expr> = Vec::new();
        loop {
            self.expect(&TokenKind::TemplateExprStart, "${")?;
            let e = self.parse_expr()?;
            self.expect(&TokenKind::TemplateExprEnd, "}")?;
            exprs.push(e);
            // next quasi
            match self.advance() {
                TokenKind::TemplateString { cooked, .. } => {
                    let cooked = cooked.ok_or_else(|| {
                        error::Error::syntax(
                            "Invalid escape sequence in template literal".to_string(),
                        )
                    })?;
                    quasis.push(Arc::from(cooked.as_str()))
                }
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected template string, got {:?}",
                        other
                    )))
                }
            }
            if !self.check(&TokenKind::TemplateExprStart) {
                break;
            }
        }
        Ok(Expr::TemplateInterp { quasis, exprs })
    }

    /// Parse a tagged template after the tag expression and first quasi.
    fn parse_tagged_template(
        &mut self,
        tag: Expr,
        first: Option<Arc<str>>,
        first_raw: Arc<str>,
    ) -> error::Result<Expr> {
        let mut quasis: Vec<Option<Arc<str>>> = vec![first];
        let mut raw: Vec<Arc<str>> = vec![first_raw];
        let mut exprs: Vec<Expr> = Vec::new();
        if !self.check(&TokenKind::TemplateExprStart) {
            // No interpolation.
            return Ok(Expr::TaggedTemplate {
                tag: Box::new(tag),
                quasis,
                raw,
                exprs,
            });
        }
        loop {
            self.expect(&TokenKind::TemplateExprStart, "${")?;
            let e = self.parse_expr()?;
            self.expect(&TokenKind::TemplateExprEnd, "}")?;
            exprs.push(e);
            match self.advance() {
                TokenKind::TemplateString { cooked, raw: rstr } => {
                    let c: Option<Arc<str>> = cooked.map(|s| Arc::from(s.as_str()));
                    let r: Arc<str> = Arc::from(rstr.as_str());
                    quasis.push(c);
                    raw.push(r);
                }
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected template string, got {:?}",
                        other
                    )))
                }
            }
            if !self.check(&TokenKind::TemplateExprStart) {
                break;
            }
        }
        Ok(Expr::TaggedTemplate {
            tag: Box::new(tag),
            quasis,
            raw,
            exprs,
        })
    }

    fn parse_array(&mut self) -> error::Result<Expr> {
        self.advance(); // [
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) {
            if self.check(&TokenKind::Comma) {
                self.advance();
                elements.push(Expr::ArrayHole);
                continue;
            }
            if self.check(&TokenKind::Spread) {
                self.advance();
                let element =
                    self.with_deferred_object_proto_duplicate_check(|p| p.parse_assign())?;
                elements.push(Expr::Spread(Box::new(element)));
            } else {
                let element =
                    self.with_deferred_object_proto_duplicate_check(|p| p.parse_assign())?;
                elements.push(element);
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBracket, "]")?;
        Ok(Expr::Array(elements))
    }

    fn parse_object(&mut self) -> error::Result<Expr> {
        self.advance(); // {
        let mut props = Vec::new();
        let mut seen_proto_mutation = false;
        while !self.check(&TokenKind::RBrace) {
            // Spread element: {...expr}
            if self.check(&TokenKind::Spread) {
                self.advance();
                let e = self.parse_assign()?;
                props.push(Property {
                    key: PropertyKey::Spread(Box::new(e)),
                    value: Expr::Undefined,
                    computed: false,
                    method: false,
                    shorthand: false,
                    kind: PropKind::Normal,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                continue;
            }
            // Async method: `async foo() {}` / `async *foo() {}` / async
            // generator. Detect `async` followed by a property-name start.
            let is_async_method = matches!(self.peek(), TokenKind::Ident(s) if s == "async")
                && !self.peek_at_tok(0).had_escape
                && !self.peek_at_tok(1).preceded_by_newline
                && matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::Ident(_)
                        | TokenKind::String(_)
                        | TokenKind::Number(_)
                        | TokenKind::LegacyNumber(_)
                        | TokenKind::LBracket
                        | TokenKind::Star
                        | TokenKind::LParen
                )
                && !matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::Colon | TokenKind::Comma | TokenKind::RBrace | TokenKind::Assign
                );
            // Also recognise the `async` keyword token as a method prefix.
            let is_async_method = is_async_method
                || matches!(self.peek(), TokenKind::Async)
                    && !self.peek_at_tok(1).preceded_by_newline
                    && matches!(
                        self.peek_at_tok(1).kind,
                        TokenKind::Ident(_)
                            | TokenKind::String(_)
                            | TokenKind::Number(_)
                            | TokenKind::LegacyNumber(_)
                            | TokenKind::LBracket
                            | TokenKind::Star
                            | TokenKind::LParen
                    )
                    && !matches!(
                        self.peek_at_tok(1).kind,
                        TokenKind::Colon | TokenKind::Comma | TokenKind::RBrace | TokenKind::Assign
                    );
            if is_async_method {
                self.advance(); // consume `async`
            }
            // Getter/setter: `get prop() {}` / `set prop(v) {}`.
            // An escaped identifier (e.g. `\u0067et`) is NOT treated as the
            // contextual keyword `get`/`set`.
            let (is_getter, is_setter) = match self.peek().clone() {
                TokenKind::Ident(s)
                    if (s == "get" || s == "set")
                        && !self.tokens[self.pos].had_escape
                        && !matches!(
                            self.peek_at_tok(1).kind,
                            TokenKind::Colon
                                | TokenKind::Comma
                                | TokenKind::RBrace
                                | TokenKind::LParen
                                | TokenKind::Assign
                        ) =>
                {
                    (s == "get", s == "set")
                }
                _ => (false, false),
            };
            if is_getter || is_setter {
                self.advance(); // consume get/set
            }
            // Generator method: `*foo() {}` / async generator `async *foo()`.
            let is_generator_method = self.eat(&TokenKind::Star);
            let (key, computed) = match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.advance();
                    (PropertyKey::Ident(Arc::from(s.as_str())), false)
                }
                other if other.as_keyword_str().is_some() => {
                    let s = other.as_keyword_str().unwrap();
                    self.advance();
                    (PropertyKey::Ident(Arc::from(s)), false)
                }
                TokenKind::String(s) => {
                    self.advance();
                    (PropertyKey::String(Arc::from(s.as_str())), false)
                }
                TokenKind::Number(n) | TokenKind::LegacyNumber(n) => {
                    self.advance();
                    (PropertyKey::Number(n), false)
                }
                TokenKind::LBracket => {
                    self.advance();
                    let e = self.with_in_allowed(|p| p.parse_assign())?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    // Computed keys have no static PropName, even when the
                    // expression is a literal such as ["__proto__"].
                    (PropertyKey::Computed(Box::new(e)), true)
                }
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected property key, got {:?}",
                        other
                    )))
                }
            };
            if is_getter || is_setter {
                let (params, param_defaults, rest_param, dstr_decls) =
                    self.parse_params_scoped(false, false, true)?;
                Self::reject_accessor_parameter_early_errors(
                    if is_getter {
                        PropKind::Get
                    } else {
                        PropKind::Set
                    },
                    &params,
                    rest_param.as_ref(),
                )?;
                Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
                let mut body = self.parse_fn_body(true, false, false, false)?;
                let body_contains_use_strict = self.last_fn_body_use_strict_directive;
                let has_destructuring_params = !dstr_decls.is_empty();
                Self::reject_use_strict_with_non_simple_params(
                    body_contains_use_strict,
                    &param_defaults,
                    rest_param.as_ref(),
                    has_destructuring_params,
                )?;
                let is_strict = self.is_strict_context || body_contains_use_strict;
                if is_strict {
                    Self::reject_strict_formal_param_names(
                        &params,
                        &dstr_decls,
                        rest_param.as_ref(),
                    )?;
                }
                {
                    let mut pre = Self::dstr_prelude_from(dstr_decls);
                    pre.append(&mut body);
                    body = pre;
                }
                let accessor_name = Self::prop_key_name(&key).map(|n| {
                    let prefix = if is_getter { "get " } else { "set " };
                    Arc::from(format!("{}{}", prefix, n).as_str())
                });
                props.push(Property {
                    key,
                    value: Expr::Function(FunctionExpr {
                        name: accessor_name,
                        params,
                        param_defaults,
                        rest_param,
                        body,
                        is_arrow: false,
                        is_async: false,
                        is_generator: false,
                        param_decls: Vec::new(),
                        is_strict,
                        is_method: true,
                        has_name_binding: false,
                    }),
                    computed,
                    method: false,
                    shorthand: false,
                    kind: if is_getter {
                        PropKind::Get
                    } else {
                        PropKind::Set
                    },
                });
            } else if self.check(&TokenKind::LParen) {
                // method shorthand or value
                let (params, param_defaults, rest_param, dstr_decls) =
                    self.parse_params_scoped(is_generator_method, is_async_method, true)?;
                Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
                let mut body =
                    self.parse_fn_body(true, false, is_generator_method, is_async_method)?;
                let body_contains_use_strict = self.last_fn_body_use_strict_directive;
                let has_destructuring_params = !dstr_decls.is_empty();
                Self::reject_use_strict_with_non_simple_params(
                    body_contains_use_strict,
                    &param_defaults,
                    rest_param.as_ref(),
                    has_destructuring_params,
                )?;
                let is_strict = self.is_strict_context || body_contains_use_strict;
                if is_strict {
                    Self::reject_strict_formal_param_names(
                        &params,
                        &dstr_decls,
                        rest_param.as_ref(),
                    )?;
                }
                {
                    let mut pre = Self::dstr_prelude_from(dstr_decls);
                    pre.append(&mut body);
                    body = pre;
                }
                let method_name = Self::prop_key_name(&key);
                props.push(Property {
                    key,
                    value: Expr::Function(FunctionExpr {
                        name: method_name,
                        params,
                        param_defaults,
                        rest_param,
                        body,
                        is_arrow: false,
                        is_async: is_async_method,
                        is_generator: is_generator_method,
                        param_decls: Vec::new(),
                        is_strict,
                        is_method: true,
                        has_name_binding: false,
                    }),
                    computed,
                    method: true,
                    shorthand: false,
                    kind: PropKind::Method,
                });
            } else if !self.check(&TokenKind::Colon) && !computed {
                // A generator method without a body is malformed; if `*` was
                // seen, this is a parse error.
                if is_generator_method {
                    return Err(error::Error::syntax(
                        "generator method requires a body".to_string(),
                    ));
                }
                // Shorthand property: `{x}` is equivalent to `{x: x}`.
                let value = if let PropertyKey::Ident(s) = &key {
                    if s.as_ref() == "yield" && !self.yield_as_identifier_allowed() {
                        return Err(error::Error::syntax(
                            "'yield' cannot be used as a shorthand property name in a generator"
                                .to_string(),
                        ));
                    }
                    if s.as_ref() == "await" && !self.await_as_identifier_allowed() {
                        return Err(error::Error::syntax(
                            "'await' cannot be used as a shorthand property name here".to_string(),
                        ));
                    }
                    if Self::is_reserved_identifier_reference_word(s) || Self::is_future_reserved(s)
                    {
                        return Err(error::Error::syntax(format!(
                            "'{}' cannot be used as a shorthand property name",
                            s
                        )));
                    }
                    if self.is_strict_context && Self::is_strict_identifier_reference_reserved(s) {
                        return Err(error::Error::syntax(format!(
                            "'{}' is a reserved word in strict mode",
                            s
                        )));
                    }
                    Expr::Ident(s.clone())
                } else {
                    return Err(error::Error::syntax(
                        "Shorthand property requires an identifier key".to_string(),
                    ));
                };
                let value = if self.eat(&TokenKind::Assign) {
                    Expr::Assign(
                        AssignOp::Assign,
                        Box::new(value),
                        Box::new(self.parse_assign()?),
                    )
                } else {
                    value
                };
                props.push(Property {
                    key,
                    value,
                    computed,
                    method: false,
                    shorthand: true,
                    kind: PropKind::Normal,
                });
            } else {
                self.expect(&TokenKind::Colon, ":")?;
                if !computed && Self::prop_key_name(&key).as_deref() == Some("__proto__") {
                    seen_proto_mutation = true;
                }
                let mut value =
                    self.with_deferred_object_proto_duplicate_check(|p| p.parse_assign())?;
                // SetFunctionName: assigning a function/arrow to a property
                // sets its `name` to the property key (when the function has
                // no explicit name). Computed keys use "".
                // Exception: __proto__ is a special property that does NOT
                // trigger SetFunctionName per spec.
                if !computed && Self::prop_key_name(&key).as_deref() != Some("__proto__") {
                    if let Expr::Function(f) = &mut value {
                        if f.name.is_none() {
                            f.name = Self::prop_key_name(&key);
                        }
                    } else if let Expr::Arrow(f) = &mut value {
                        if f.name.is_none() {
                            f.name = Self::prop_key_name(&key);
                        }
                    }
                }
                props.push(Property {
                    key,
                    value,
                    computed,
                    method: false,
                    shorthand: false,
                    kind: PropKind::Normal,
                });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "}")?;
        let object = Expr::Object(props);
        if seen_proto_mutation
            && self.defer_object_proto_duplicate_check == 0
            && !self.check(&TokenKind::Assign)
        {
            Self::reject_duplicate_proto_object_literal(&object)?;
        }
        Ok(object)
    }

    fn parse_function_expr(&mut self) -> error::Result<Expr> {
        self.parse_function_expr_with_async(false)
    }

    fn parse_function_expr_with_async(&mut self, is_async: bool) -> error::Result<Expr> {
        self.advance(); // function
        let is_generator = self.eat(&TokenKind::Star);
        // FunctionExpression names use the function's own Yield/Await grammar
        // parameters rather than inheriting the surrounding function body.
        let name = self.with_function_context(is_generator, is_async, |p| {
            let name = match p.peek().clone() {
                TokenKind::Ident(s) => {
                    p.advance();
                    Some(Arc::from(s.as_str()))
                }
                TokenKind::Await if p.await_as_identifier_allowed() => {
                    p.advance();
                    Some(Arc::from("await"))
                }
                TokenKind::Yield if p.yield_as_identifier_allowed() => {
                    p.advance();
                    Some(Arc::from("yield"))
                }
                _ => None,
            };
            if let Some(ref name) = name {
                p.check_binding_name(name)?;
            }
            Ok(name)
        })?;
        let (params, param_defaults, rest_param, dstr_decls) =
            self.parse_params_scoped(is_generator, is_async, false)?;
        let mut body = self.parse_fn_body(false, false, is_generator, is_async)?;
        let body_contains_use_strict = self.last_fn_body_use_strict_directive;
        let has_destructuring_params = !dstr_decls.is_empty();
        if Self::has_non_simple_params(
            &param_defaults,
            rest_param.as_ref(),
            has_destructuring_params,
        ) {
            Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
        }
        Self::reject_use_strict_with_non_simple_params(
            body_contains_use_strict,
            &param_defaults,
            rest_param.as_ref(),
            has_destructuring_params,
        )?;
        {
            let mut pre = Self::dstr_prelude_from(dstr_decls);
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || body_contains_use_strict;
        // A body directive is discovered after parsing the parameters, so
        // strict-only reserved words must be validated again here.
        if is_strict {
            if let Some(ref n) = name {
                if matches!(&**n, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(n)
                {
                    return Err(error::Error::syntax(format!(
                        "'{}' cannot be used as a function name in strict mode",
                        n
                    )));
                }
            }
            for p in &params {
                if matches!(&**p, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(p)
                {
                    return Err(error::Error::syntax(format!(
                        "Parameter name '{}' is not allowed in strict mode",
                        p
                    )));
                }
            }
            let mut seen = std::collections::HashSet::new();
            for p in &params {
                if !seen.insert(p.clone()) {
                    return Err(error::Error::syntax(format!(
                        "Duplicate parameter '{}' is not allowed in strict mode",
                        p
                    )));
                }
            }
        }
        let has_name_binding = name.is_some();
        Ok(Expr::Function(FunctionExpr {
            name,
            params,
            param_defaults,
            rest_param,
            body,
            is_arrow: false,
            is_async,
            is_generator,
            param_decls: Vec::new(),
            is_strict,
            is_method: false,
            has_name_binding,
        }))
    }

    fn parse_new(&mut self) -> error::Result<Expr> {
        let new_had_escape = self.peek_at_tok(0).had_escape;
        self.advance(); // new
                        // new.target
        if self.check(&TokenKind::Dot) {
            // peek at the property name
            let target = self.peek_at_tok(1);
            if let TokenKind::Ident(s) = target.kind.clone() {
                if s == "target" && !new_had_escape && !target.had_escape {
                    if !self.new_target_allowed {
                        return Err(error::Error::syntax(
                            "new.target is not allowed here".to_string(),
                        ));
                    }
                    self.advance(); // .
                    self.advance(); // target
                    return Ok(Expr::NewTarget);
                } else if s == "target" {
                    return Err(error::Error::syntax(
                        "new.target must use raw new and target tokens".to_string(),
                    ));
                }
            }
        }
        // parse the constructor (primary + member/tagged access, but NOT call parens)
        self.last_primary_parenthesized = false;
        let mut callee = self.parse_primary()?;
        let parenthesized_callee = self.last_primary_parenthesized;
        self.last_primary_parenthesized = false;
        if matches!(callee, Expr::ImportCall { .. }) && !parenthesized_callee {
            return Err(error::Error::syntax(
                "import() cannot be used as a new expression constructor".to_string(),
            ));
        }
        loop {
            match self.peek().clone() {
                TokenKind::Dot => {
                    self.advance();
                    let name = self.read_property_name()?;
                    let prop = Expr::String(Arc::from(name.as_str()));
                    callee = Expr::Member {
                        object: Box::new(callee),
                        property: Box::new(prop),
                        computed: false,
                        optional: false,
                        optional_chain: false,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let prop = self.with_in_allowed(|p| p.parse_expr())?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    callee = Expr::Member {
                        object: Box::new(callee),
                        property: Box::new(prop),
                        computed: true,
                        optional: false,
                        optional_chain: false,
                    };
                }
                TokenKind::TemplateString { cooked, raw } => {
                    let quasi0: Option<Arc<str>> = cooked.map(|s| Arc::from(s.as_str()));
                    let raw0: Arc<str> = Arc::from(raw.as_str());
                    self.advance();
                    callee = self.parse_tagged_template(callee, quasi0, raw0)?;
                }
                _ => break,
            }
        }
        if self.check(&TokenKind::LParen) {
            self.advance();
            let args = self.parse_args()?;
            self.expect(&TokenKind::RParen, ")")?;
            Ok(Expr::New {
                callee: Box::new(callee),
                args,
            })
        } else {
            Ok(Expr::New {
                callee: Box::new(callee),
                args: Vec::new(),
            })
        }
    }

    /// Return whether the already-consumed `(` closes immediately before an
    /// arrow. This avoids applying formal-parameter grammar while speculatively
    /// parsing an ordinary parenthesized expression.
    fn current_parenthesized_group_is_arrow_head(&self) -> bool {
        let mut depth = 1usize;
        let mut index = self.pos;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(index + 1).map(|token| &token.kind),
                            Some(TokenKind::Arrow)
                        );
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            index += 1;
        }
        false
    }

    /// After consuming `(`, try to parse arrow params followed by `) =>`.
    /// Returns true and sets `last_arrow_params` if it looks like an arrow function.
    fn try_parse_arrow_params(&mut self) -> error::Result<bool> {
        let save = self.pos;
        let mut params = Vec::new();
        let mut defaults: Vec<Option<Expr>> = Vec::new();
        let mut rest: Option<Arc<str>> = None;
        let mut dstr_decls: Vec<(Pattern, String, Option<Expr>)> = Vec::new();
        // empty params: () =>
        if self.check(&TokenKind::RParen) {
            self.advance();
            if self.check(&TokenKind::Arrow) {
                if self.tokens[self.pos].preceded_by_newline {
                    return Err(error::Error::syntax(
                        "Line terminator not allowed before =>".to_string(),
                    ));
                }
                self.last_arrow_params = Some(params);
                self.arrow_defaults = defaults;
                self.arrow_rest = rest;
                self.arrow_destructure_decls = dstr_decls;
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        loop {
            if self.check(&TokenKind::Spread) {
                self.advance();
                // rest may itself be a destructuring pattern: `(...[a, b])`
                if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                    let p = self.parse_destructure_pattern()?;
                    let tmp = format!("#arg{}", params.len());
                    rest = Some(Arc::from(tmp.as_str()));
                    dstr_decls.push((p, tmp, None));
                    break;
                }
                if self.binding_identifier_name().is_none() {
                    self.pos = save;
                    return Ok(false);
                }
                rest = Some(self.parse_binding_identifier("Expected rest parameter name")?);
                break;
            }
            if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                // Destructuring parameter: `([a, b]) =>` / `({x, y}) =>`.
                // Synthesize a positional temp param and remember the
                // pattern so the body can bind it: `let <pat> = #argN;`.
                // If the pattern fails to parse (e.g. `({a:1})` is an object
                // literal, not a binding pattern), rewind and treat this as
                // not-an-arrow so the caller parses a parenthesised expr.
                let saved = self.pos;
                let p = match self.parse_destructure_pattern() {
                    Ok(p) => p,
                    Err(_) => {
                        self.pos = save;
                        return Ok(false);
                    }
                };
                let _ = saved;
                let tmp = format!("#arg{}", params.len());
                params.push(Arc::from(tmp.as_str()));
                defaults.push(None);
                // Optional default: `({a} = {}) =>`
                let default = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                dstr_decls.push((p, tmp, default));
            } else if self.binding_identifier_name().is_some() {
                params.push(self.parse_binding_identifier("Expected arrow parameter")?);
                let default = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                defaults.push(default);
            } else {
                self.pos = save;
                return Ok(false);
            }
            if self.check(&TokenKind::Comma) {
                self.advance();
                // Trailing comma: `(a, b,) => ...`
                if self.check(&TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        if self.check(&TokenKind::RParen) {
            self.advance();
            if self.check(&TokenKind::Arrow) {
                if self.tokens[self.pos].preceded_by_newline {
                    return Err(error::Error::syntax(
                        "Line terminator not allowed before =>".to_string(),
                    ));
                }
                while defaults.len() < params.len() {
                    defaults.push(None);
                }
                self.last_arrow_params = Some(params);
                self.arrow_defaults = defaults;
                self.arrow_rest = rest;
                self.arrow_destructure_decls = dstr_decls;
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        self.pos = save;
        Ok(false)
    }

    /// Validate arrow-function parameter bound names.
    fn validate_arrow_params(
        &self,
        params: &[Arc<str>],
        dstr_decls: &[(Pattern, String, Option<Expr>)],
        rest_param: Option<&Arc<str>>,
        reject_eval_arguments: bool,
    ) -> error::Result<()> {
        let mut seen = std::collections::HashSet::new();
        for p in params {
            if reject_eval_arguments
                && (matches!(&**p, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(p))
            {
                return Err(error::Error::syntax(format!(
                    "Parameter name '{}' is not allowed in arrow function",
                    p
                )));
            }
            if !seen.insert(p.clone()) {
                return Err(error::Error::syntax(format!(
                    "Duplicate parameter '{}' is not allowed in arrow function",
                    p
                )));
            }
        }
        for (pattern, _tmp, _default) in dstr_decls {
            Self::check_pattern_binding_names(pattern, &mut seen, reject_eval_arguments)?;
        }
        if let Some(r) = rest_param {
            if reject_eval_arguments
                && (matches!(&**r, "eval" | "arguments")
                    || Self::is_strict_identifier_reference_reserved(r))
            {
                return Err(error::Error::syntax(format!(
                    "Parameter name '{}' is not allowed in arrow function",
                    r
                )));
            }
            if !seen.insert(r.clone()) {
                return Err(error::Error::syntax(format!(
                    "Duplicate parameter '{}' is not allowed in arrow function",
                    r
                )));
            }
        }
        Ok(())
    }

    /// Validate and record binding identifiers introduced by a pattern.
    fn check_pattern_binding_names(
        pattern: &Pattern,
        seen: &mut std::collections::HashSet<Arc<str>>,
        reject_eval_arguments: bool,
    ) -> error::Result<()> {
        match pattern {
            Pattern::Ident(name) => {
                if reject_eval_arguments
                    && (matches!(&**name, "eval" | "arguments")
                        || Self::is_strict_identifier_reference_reserved(name))
                {
                    return Err(error::Error::syntax(format!(
                        "Parameter name '{}' is not allowed in arrow function",
                        name
                    )));
                }
                if !seen.insert(name.clone()) {
                    return Err(error::Error::syntax(format!(
                        "Duplicate parameter '{}' is not allowed in arrow function",
                        name
                    )));
                }
            }
            Pattern::Hole => {}
            Pattern::Array(elems) => {
                for el in elems {
                    Self::check_pattern_binding_names(el, seen, reject_eval_arguments)?;
                }
            }
            Pattern::Object(props, rest) => {
                for (_, target) in props {
                    Self::check_pattern_binding_names(target, seen, reject_eval_arguments)?;
                }
                if let Some(r) = rest {
                    Self::check_pattern_binding_names(r, seen, reject_eval_arguments)?;
                }
            }
            Pattern::Assign(inner, _) => {
                Self::check_pattern_binding_names(inner, seen, reject_eval_arguments)?
            }
            Pattern::Rest(inner) => {
                Self::check_pattern_binding_names(inner, seen, reject_eval_arguments)?
            }
        }
        Ok(())
    }

    fn parse_arrow_body(&mut self, params: Vec<Arc<str>>) -> error::Result<Expr> {
        self.parse_arrow_body_with_async(params, false)
    }

    fn parse_arrow_body_with_async(
        &mut self,
        params: Vec<Arc<str>>,
        is_async: bool,
    ) -> error::Result<Expr> {
        let param_defaults = std::mem::take(&mut self.arrow_defaults);
        let rest_param = self.arrow_rest.take();
        let dstr_decls = std::mem::take(&mut self.arrow_destructure_decls);
        let has_destructuring_params = !dstr_decls.is_empty();
        // arrow body: expression or block
        if self.check(&TokenKind::LBrace) {
            let mut body = self.parse_arrow_block_body(is_async)?;
            let body_contains_use_strict = self.last_fn_body_use_strict_directive;
            self.validate_arrow_params(
                &params,
                &dstr_decls,
                rest_param.as_ref(),
                self.is_strict_context || body_contains_use_strict,
            )?;
            Self::reject_use_strict_with_non_simple_params(
                body_contains_use_strict,
                &param_defaults,
                rest_param.as_ref(),
                has_destructuring_params,
            )?;
            // Synthesize `let <pattern> = #argN;` prelude statements that bind
            // each destructuring parameter from its internal positional carrier.
            // A parameter default wraps the pattern so the compiler applies it
            // when the source value is undefined.
            let prelude = Self::arrow_destructuring_prelude(dstr_decls);
            if !prelude.is_empty() {
                let mut combined = prelude;
                combined.append(&mut body);
                body = combined;
            }
            let is_strict = self.is_strict_context || body_contains_use_strict;
            Ok(Expr::Arrow(FunctionExpr {
                name: None,
                params,
                param_defaults,
                rest_param,
                body,
                is_arrow: true,
                is_async,
                is_generator: false,
                param_decls: Vec::new(),
                is_strict,
                is_method: false,
                has_name_binding: false,
            }))
        } else {
            self.validate_arrow_params(
                &params,
                &dstr_decls,
                rest_param.as_ref(),
                self.is_strict_context,
            )?;
            let e = self.with_function_body_context(false, is_async, |p| p.parse_assign())?;
            let mut body = Self::arrow_destructuring_prelude(dstr_decls);
            body.push(self.stmt(StmtNode::Return(Some(e))));
            Ok(Expr::Arrow(FunctionExpr {
                name: None,
                params,
                param_defaults,
                rest_param,
                body,
                is_arrow: true,
                is_async,
                is_generator: false,
                param_decls: Vec::new(),
                // Arrow with expression body has no directive prologue; inherit.
                is_strict: self.is_strict_context,
                is_method: false,
                has_name_binding: false,
            }))
        }
    }

    fn arrow_destructuring_prelude(dstr_decls: Vec<(Pattern, String, Option<Expr>)>) -> Vec<Stmt> {
        dstr_decls
            .into_iter()
            .map(|(pattern, tmp, default)| {
                let pattern = match default {
                    Some(d) => Pattern::Assign(Box::new(pattern), d),
                    None => pattern,
                };
                Stmt {
                    line: 0,
                    node: StmtNode::Destructure {
                        kind: VarKind::Let,
                        pattern,
                        init: Some(Expr::Ident(Arc::from(tmp.as_str()))),
                    },
                }
            })
            .collect()
    }

    /// Derive a function name from an object-literal property key, for the
    /// `name` own-property of concise methods / accessors. Computed keys have
    /// no static name, so they return None (matching the spec's "" case only
    /// approximately; a true computed name is set at runtime, which we don't do).
    fn prop_key_name(key: &PropertyKey) -> Option<Arc<str>> {
        match key {
            PropertyKey::Ident(s) | PropertyKey::String(s) => Some(s.clone()),
            PropertyKey::Number(n) => Some(Arc::from(crate::value::num_to_string(*n).as_str())),
            _ => None,
        }
    }

    fn with_deferred_object_proto_duplicate_check<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        self.defer_object_proto_duplicate_check += 1;
        let result = f(self);
        self.defer_object_proto_duplicate_check -= 1;
        result
    }

    fn with_in_allowed<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> error::Result<T>,
    ) -> error::Result<T> {
        let saved = self.no_in;
        self.no_in = false;
        let result = f(self);
        self.no_in = saved;
        result
    }

    fn is_proto_mutation_property(prop: &Property) -> bool {
        !prop.computed
            && !prop.shorthand
            && !prop.method
            && matches!(prop.kind, PropKind::Normal)
            && Self::prop_key_name(&prop.key).as_deref() == Some("__proto__")
    }

    fn reject_duplicate_proto_object_literal(expr: &Expr) -> error::Result<()> {
        match expr {
            Expr::Object(props) => {
                let mut seen_proto_mutation = false;
                for prop in props {
                    if Self::is_proto_mutation_property(prop) {
                        if seen_proto_mutation {
                            return Err(error::Error::syntax(
                                "Duplicate __proto__ property in object literal".to_string(),
                            ));
                        }
                        seen_proto_mutation = true;
                    }
                    Self::reject_duplicate_proto_property_key(&prop.key)?;
                    Self::reject_duplicate_proto_object_literal(&prop.value)?;
                }
            }
            Expr::Array(elements) | Expr::Sequence(elements) => {
                for element in elements {
                    Self::reject_duplicate_proto_object_literal(element)?;
                }
            }
            Expr::TaggedTemplate { tag, exprs, .. } => {
                Self::reject_duplicate_proto_object_literal(tag)?;
                for expr in exprs {
                    Self::reject_duplicate_proto_object_literal(expr)?;
                }
            }
            Expr::TemplateInterp { exprs, .. } => {
                for expr in exprs {
                    Self::reject_duplicate_proto_object_literal(expr)?;
                }
            }
            Expr::Member {
                object, property, ..
            } => {
                Self::reject_duplicate_proto_object_literal(object)?;
                Self::reject_duplicate_proto_object_literal(property)?;
            }
            Expr::Spread(inner)
            | Expr::Await(inner)
            | Expr::YieldDelegate(inner)
            | Expr::Unary(_, inner)
            | Expr::Update(_, _, inner) => {
                Self::reject_duplicate_proto_object_literal(inner)?;
            }
            Expr::Yield(Some(inner)) => {
                Self::reject_duplicate_proto_object_literal(inner)?;
            }
            Expr::Binary(_, left, right) | Expr::Logical(_, left, right) => {
                Self::reject_duplicate_proto_object_literal(left)?;
                Self::reject_duplicate_proto_object_literal(right)?;
            }
            Expr::Assign(op, left, right) => {
                if matches!(op, AssignOp::Assign) && Self::is_assignment_target(left) {
                    Self::reject_duplicate_proto_assignment_pattern(left)?;
                } else {
                    Self::reject_duplicate_proto_object_literal(left)?;
                }
                Self::reject_duplicate_proto_object_literal(right)?;
            }
            Expr::Conditional(cond, then_expr, else_expr) => {
                Self::reject_duplicate_proto_object_literal(cond)?;
                Self::reject_duplicate_proto_object_literal(then_expr)?;
                Self::reject_duplicate_proto_object_literal(else_expr)?;
            }
            Expr::Call { callee, args, .. } | Expr::New { callee, args } => {
                Self::reject_duplicate_proto_object_literal(callee)?;
                for arg in args {
                    Self::reject_duplicate_proto_object_literal(arg)?;
                }
            }
            Expr::PrivateGet { object, .. } => {
                Self::reject_duplicate_proto_object_literal(object)?;
            }
            Expr::PrivateSet { object, value, .. } | Expr::PrivateInit { object, value, .. } => {
                Self::reject_duplicate_proto_object_literal(object)?;
                Self::reject_duplicate_proto_object_literal(value)?;
            }
            Expr::PrivateDefineAccessor {
                object, get, set, ..
            } => {
                Self::reject_duplicate_proto_object_literal(object)?;
                if let Some(get) = get {
                    Self::reject_duplicate_proto_object_literal(get)?;
                }
                if let Some(set) = set {
                    Self::reject_duplicate_proto_object_literal(set)?;
                }
            }
            Expr::PrivateFieldDecl {
                init: Some(init), ..
            } => {
                Self::reject_duplicate_proto_object_literal(init)?;
            }
            Expr::Function(_) | Expr::Arrow(_) | Expr::Class(_) => {}
            _ => {}
        }
        Ok(())
    }

    fn reject_duplicate_proto_assignment_pattern(pattern: &Expr) -> error::Result<()> {
        match pattern {
            Expr::Ident(_) => {}
            Expr::Member {
                object, property, ..
            } => {
                Self::reject_duplicate_proto_object_literal(object)?;
                Self::reject_duplicate_proto_object_literal(property)?;
            }
            Expr::PrivateGet { object, .. } => {
                Self::reject_duplicate_proto_object_literal(object)?;
            }
            Expr::Array(elements) => {
                for element in elements {
                    match element {
                        Expr::ArrayHole => {}
                        Expr::Spread(inner) => {
                            Self::reject_duplicate_proto_assignment_pattern(inner)?;
                        }
                        other => Self::reject_duplicate_proto_assignment_pattern(other)?,
                    }
                }
            }
            Expr::Object(props) => {
                for prop in props {
                    Self::reject_duplicate_proto_property_key(&prop.key)?;
                    Self::reject_duplicate_proto_assignment_pattern(&prop.value)?;
                }
            }
            Expr::Assign(AssignOp::Assign, left, default) => {
                Self::reject_duplicate_proto_assignment_pattern(left)?;
                Self::reject_duplicate_proto_object_literal(default)?;
            }
            other => {
                Self::reject_duplicate_proto_object_literal(other)?;
            }
        }
        Ok(())
    }

    fn reject_duplicate_proto_property_key(key: &PropertyKey) -> error::Result<()> {
        match key {
            PropertyKey::Computed(expr) | PropertyKey::Spread(expr) => {
                Self::reject_duplicate_proto_object_literal(expr)
            }
            _ => Ok(()),
        }
    }

    fn reject_static_block_early_errors(body: &[Stmt]) -> error::Result<()> {
        let mut labels = std::collections::HashSet::new();
        for stmt in body {
            Self::check_static_block_stmt(stmt, &mut labels)?;
        }
        Ok(())
    }

    fn reject_class_field_initializer_contains_arguments(expr: &Expr) -> error::Result<()> {
        if Self::class_field_initializer_contains_arguments_expr(expr) {
            return Err(error::Error::syntax(
                "'arguments' is not allowed in class field initializer".to_string(),
            ));
        }
        Ok(())
    }

    pub fn reject_class_field_initializer_program_contains_arguments(
        program: &Program,
    ) -> error::Result<()> {
        if program
            .body
            .iter()
            .any(Self::class_field_initializer_contains_arguments_stmt)
        {
            return Err(error::Error::syntax(
                "'arguments' is not allowed in class field initializer".to_string(),
            ));
        }
        Ok(())
    }

    fn class_field_initializer_contains_arguments_stmt(stmt: &Stmt) -> bool {
        match &stmt.node {
            StmtNode::VarDecl { decls, .. } => decls.iter().any(|(_, init)| {
                init.as_ref()
                    .is_some_and(Self::class_field_initializer_contains_arguments_expr)
            }),
            StmtNode::ExprStmt(expr) | StmtNode::Throw(expr) => {
                Self::class_field_initializer_contains_arguments_expr(expr)
            }
            StmtNode::Block(body) => body
                .iter()
                .any(Self::class_field_initializer_contains_arguments_stmt),
            StmtNode::If { cond, then, else_ } => {
                Self::class_field_initializer_contains_arguments_expr(cond)
                    || Self::class_field_initializer_contains_arguments_stmt(then)
                    || else_.as_ref().is_some_and(|stmt| {
                        Self::class_field_initializer_contains_arguments_stmt(stmt)
                    })
            }
            StmtNode::While { cond, body } | StmtNode::DoWhile { body, cond } => {
                Self::class_field_initializer_contains_arguments_expr(cond)
                    || Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::For {
                init,
                cond,
                update,
                body,
            } => {
                init.as_ref()
                    .is_some_and(|stmt| Self::class_field_initializer_contains_arguments_stmt(stmt))
                    || cond
                        .as_ref()
                        .is_some_and(Self::class_field_initializer_contains_arguments_expr)
                    || update
                        .as_ref()
                        .is_some_and(Self::class_field_initializer_contains_arguments_expr)
                    || Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::ForIn { left, right, body } => {
                Self::class_field_initializer_contains_arguments_stmt(left)
                    || Self::class_field_initializer_contains_arguments_expr(right)
                    || Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::ForOf {
                left, right, body, ..
            } => {
                Self::class_field_initializer_contains_arguments_stmt(left)
                    || Self::class_field_initializer_contains_arguments_expr(right)
                    || Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::With { object, body } => {
                Self::class_field_initializer_contains_arguments_expr(object)
                    || Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::Return(expr) => expr
                .as_ref()
                .is_some_and(Self::class_field_initializer_contains_arguments_expr),
            StmtNode::TryCatch {
                try_body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                Self::class_field_initializer_contains_arguments_stmt(try_body)
                    || catch_param
                        .as_ref()
                        .is_some_and(Self::class_field_initializer_contains_arguments_pattern)
                    || catch_body.as_ref().is_some_and(|stmt| {
                        Self::class_field_initializer_contains_arguments_stmt(stmt)
                    })
                    || finally_body.as_ref().is_some_and(|stmt| {
                        Self::class_field_initializer_contains_arguments_stmt(stmt)
                    })
            }
            StmtNode::Labeled(_, body) => {
                Self::class_field_initializer_contains_arguments_stmt(body)
            }
            StmtNode::Switch { disc, cases } => {
                Self::class_field_initializer_contains_arguments_expr(disc)
                    || cases.iter().any(|case| {
                        case.test
                            .as_ref()
                            .is_some_and(Self::class_field_initializer_contains_arguments_expr)
                            || case
                                .body
                                .iter()
                                .any(Self::class_field_initializer_contains_arguments_stmt)
                    })
            }
            StmtNode::Destructure { pattern, init, .. } => {
                Self::class_field_initializer_contains_arguments_pattern(pattern)
                    || init
                        .as_ref()
                        .is_some_and(Self::class_field_initializer_contains_arguments_expr)
            }
            StmtNode::FunctionDecl(_)
            | StmtNode::Break(_)
            | StmtNode::Continue(_)
            | StmtNode::Empty => false,
        }
    }

    fn class_field_initializer_contains_arguments_expr(expr: &Expr) -> bool {
        match expr {
            Expr::Ident(name) => name.as_ref() == "arguments",
            Expr::Array(elements) | Expr::Sequence(elements) => elements
                .iter()
                .any(Self::class_field_initializer_contains_arguments_expr),
            Expr::Object(props) => props.iter().any(|prop| {
                Self::class_field_initializer_contains_arguments_property_key(&prop.key)
                    || Self::class_field_initializer_contains_arguments_expr(&prop.value)
            }),
            Expr::Unary(_, expr)
            | Expr::Update(_, _, expr)
            | Expr::Spread(expr)
            | Expr::Await(expr)
            | Expr::YieldDelegate(expr)
            | Expr::OptionalChain(expr) => {
                Self::class_field_initializer_contains_arguments_expr(expr)
            }
            Expr::Yield(Some(expr)) => Self::class_field_initializer_contains_arguments_expr(expr),
            Expr::Binary(_, left, right)
            | Expr::Logical(_, left, right)
            | Expr::Assign(_, left, right) => {
                Self::class_field_initializer_contains_arguments_expr(left)
                    || Self::class_field_initializer_contains_arguments_expr(right)
            }
            Expr::Conditional(cond, then_expr, else_expr) => {
                Self::class_field_initializer_contains_arguments_expr(cond)
                    || Self::class_field_initializer_contains_arguments_expr(then_expr)
                    || Self::class_field_initializer_contains_arguments_expr(else_expr)
            }
            Expr::Call { callee, args, .. } | Expr::New { callee, args } => {
                Self::class_field_initializer_contains_arguments_expr(callee)
                    || args
                        .iter()
                        .any(Self::class_field_initializer_contains_arguments_expr)
            }
            Expr::CallWithThis {
                callee,
                this_value,
                args,
            } => {
                Self::class_field_initializer_contains_arguments_expr(callee)
                    || Self::class_field_initializer_contains_arguments_expr(this_value)
                    || args
                        .iter()
                        .any(Self::class_field_initializer_contains_arguments_expr)
            }
            Expr::DecoratorAddInitializer { initializer, .. } => {
                Self::class_field_initializer_contains_arguments_expr(initializer)
            }
            Expr::DecoratorAccess {
                receiver,
                key,
                value,
                ..
            } => {
                Self::class_field_initializer_contains_arguments_expr(receiver)
                    || Self::class_field_initializer_contains_arguments_expr(key)
                    || value.as_ref().is_some_and(|value| {
                        Self::class_field_initializer_contains_arguments_expr(value)
                    })
            }
            Expr::ImportCall { specifier, options } => {
                Self::class_field_initializer_contains_arguments_expr(specifier)
                    || options.as_ref().is_some_and(|options| {
                        Self::class_field_initializer_contains_arguments_expr(options)
                    })
            }
            Expr::Member {
                object, property, ..
            } => {
                Self::class_field_initializer_contains_arguments_expr(object)
                    || Self::class_field_initializer_contains_arguments_expr(property)
            }
            Expr::PrivateGet { object, .. } => {
                Self::class_field_initializer_contains_arguments_expr(object)
            }
            Expr::PrivateSet { object, value, .. } | Expr::PrivateInit { object, value, .. } => {
                Self::class_field_initializer_contains_arguments_expr(object)
                    || Self::class_field_initializer_contains_arguments_expr(value)
            }
            Expr::PrivateDefineAccessor {
                object, get, set, ..
            } => {
                Self::class_field_initializer_contains_arguments_expr(object)
                    || get.as_ref().is_some_and(|expr| {
                        Self::class_field_initializer_contains_arguments_expr(expr)
                    })
                    || set.as_ref().is_some_and(|expr| {
                        Self::class_field_initializer_contains_arguments_expr(expr)
                    })
            }
            Expr::PublicFieldInit {
                object,
                computed_name,
                value,
                ..
            } => {
                Self::class_field_initializer_contains_arguments_expr(object)
                    || computed_name.as_ref().is_some_and(|expr| {
                        Self::class_field_initializer_contains_arguments_expr(expr)
                    })
                    || Self::class_field_initializer_contains_arguments_expr(value)
            }
            Expr::PrivateFieldDecl {
                init: Some(init), ..
            } => Self::class_field_initializer_contains_arguments_expr(init),
            Expr::TemplateInterp { exprs, .. } => exprs
                .iter()
                .any(Self::class_field_initializer_contains_arguments_expr),
            Expr::TaggedTemplate { tag, exprs, .. } => {
                Self::class_field_initializer_contains_arguments_expr(tag)
                    || exprs
                        .iter()
                        .any(Self::class_field_initializer_contains_arguments_expr)
            }
            Expr::Arrow(func) => {
                func.param_defaults
                    .iter()
                    .flatten()
                    .any(Self::class_field_initializer_contains_arguments_expr)
                    || func
                        .param_decls
                        .iter()
                        .any(Self::class_field_initializer_contains_arguments_pattern)
                    || func
                        .body
                        .iter()
                        .any(Self::class_field_initializer_contains_arguments_stmt)
            }
            Expr::Class(cls) => {
                cls.superclass
                    .as_ref()
                    .is_some_and(|expr| Self::class_field_initializer_contains_arguments_expr(expr))
                    || cls.methods.iter().any(|method| {
                        method.computed_name.as_ref().is_some_and(|expr| {
                            Self::class_field_initializer_contains_arguments_expr(expr)
                        })
                    })
                    || cls.public_fields.iter().any(|field| {
                        field.computed_name.as_ref().is_some_and(|expr| {
                            Self::class_field_initializer_contains_arguments_expr(expr)
                        }) || field.init.as_ref().is_some_and(|expr| {
                            Self::class_field_initializer_contains_arguments_expr(expr)
                        })
                    })
                    || cls.private_fields.iter().any(|field| {
                        field.init.as_ref().is_some_and(|expr| {
                            Self::class_field_initializer_contains_arguments_expr(expr)
                        })
                    })
                    || cls.static_blocks.iter().any(|body| {
                        body.iter()
                            .any(Self::class_field_initializer_contains_arguments_stmt)
                    })
            }
            Expr::Function(_)
            | Expr::Number(_)
            | Expr::BigInt(_)
            | Expr::String(_)
            | Expr::TemplateStr(_)
            | Expr::Bool(_)
            | Expr::Null
            | Expr::Undefined
            | Expr::This
            | Expr::Super
            | Expr::ArrayHole
            | Expr::Regex(_, _)
            | Expr::NewTarget
            | Expr::ImportMeta
            | Expr::Yield(None)
            | Expr::PrivateFieldDecl { init: None, .. } => false,
        }
    }

    fn class_field_initializer_contains_arguments_pattern(pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Assign(pattern, expr) => {
                Self::class_field_initializer_contains_arguments_pattern(pattern)
                    || Self::class_field_initializer_contains_arguments_expr(expr)
            }
            Pattern::Array(elements) => elements
                .iter()
                .any(Self::class_field_initializer_contains_arguments_pattern),
            Pattern::Object(props, rest) => {
                props.iter().any(|(key, pattern)| {
                    Self::class_field_initializer_contains_arguments_property_key(key)
                        || Self::class_field_initializer_contains_arguments_pattern(pattern)
                }) || rest.as_ref().is_some_and(|pattern| {
                    Self::class_field_initializer_contains_arguments_pattern(pattern)
                })
            }
            Pattern::Rest(pattern) => {
                Self::class_field_initializer_contains_arguments_pattern(pattern)
            }
            Pattern::Ident(name) => name.as_ref() == "arguments",
            Pattern::Hole => false,
        }
    }

    fn class_field_initializer_contains_arguments_property_key(key: &PropertyKey) -> bool {
        match key {
            PropertyKey::Computed(expr) | PropertyKey::Spread(expr) => {
                Self::class_field_initializer_contains_arguments_expr(expr)
            }
            PropertyKey::Ident(_) | PropertyKey::String(_) | PropertyKey::Number(_) => false,
        }
    }

    fn check_static_block_stmt(
        stmt: &Stmt,
        labels: &mut std::collections::HashSet<Arc<str>>,
    ) -> error::Result<()> {
        match &stmt.node {
            StmtNode::VarDecl { decls, .. } => {
                for (name, init) in decls {
                    Self::check_static_block_name(name)?;
                    if let Some(init) = init {
                        Self::check_static_block_expr(init)?;
                    }
                }
            }
            StmtNode::ExprStmt(expr) | StmtNode::Throw(expr) => {
                Self::check_static_block_expr(expr)?;
            }
            StmtNode::Block(body) => {
                for stmt in body {
                    Self::check_static_block_stmt(stmt, labels)?;
                }
            }
            StmtNode::If { cond, then, else_ } => {
                Self::check_static_block_expr(cond)?;
                Self::check_static_block_stmt(then, labels)?;
                if let Some(else_) = else_ {
                    Self::check_static_block_stmt(else_, labels)?;
                }
            }
            StmtNode::While { cond, body } | StmtNode::DoWhile { body, cond } => {
                Self::check_static_block_expr(cond)?;
                Self::check_static_block_stmt(body, labels)?;
            }
            StmtNode::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    Self::check_static_block_stmt(init, labels)?;
                }
                if let Some(cond) = cond {
                    Self::check_static_block_expr(cond)?;
                }
                if let Some(update) = update {
                    Self::check_static_block_expr(update)?;
                }
                Self::check_static_block_stmt(body, labels)?;
            }
            StmtNode::ForIn { left, right, body } => {
                Self::check_static_block_stmt(left, labels)?;
                Self::check_static_block_expr(right)?;
                Self::check_static_block_stmt(body, labels)?;
            }
            StmtNode::ForOf {
                left, right, body, ..
            } => {
                Self::check_static_block_stmt(left, labels)?;
                Self::check_static_block_expr(right)?;
                Self::check_static_block_stmt(body, labels)?;
            }
            StmtNode::With { object, body } => {
                Self::check_static_block_expr(object)?;
                Self::check_static_block_stmt(body, labels)?;
            }
            StmtNode::Return(expr) => {
                if let Some(expr) = expr {
                    Self::check_static_block_expr(expr)?;
                }
            }
            StmtNode::TryCatch {
                try_body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                Self::check_static_block_stmt(try_body, labels)?;
                if let Some(param) = catch_param {
                    Self::check_static_block_pattern(param)?;
                }
                if let Some(catch_body) = catch_body {
                    Self::check_static_block_stmt(catch_body, labels)?;
                }
                if let Some(finally_body) = finally_body {
                    Self::check_static_block_stmt(finally_body, labels)?;
                }
            }
            StmtNode::FunctionDecl(func) => {
                if let Some(name) = &func.name {
                    Self::check_static_block_name(name)?;
                }
            }
            StmtNode::Labeled(label, body) => {
                Self::check_static_block_name(label)?;
                if !labels.insert(label.clone()) {
                    return Err(error::Error::syntax(
                        "Duplicate label in static block".to_string(),
                    ));
                }
                Self::check_static_block_stmt(body, labels)?;
                labels.remove(label);
            }
            StmtNode::Switch { disc, cases } => {
                Self::check_static_block_expr(disc)?;
                for case in cases {
                    if let Some(test) = &case.test {
                        Self::check_static_block_expr(test)?;
                    }
                    for stmt in &case.body {
                        Self::check_static_block_stmt(stmt, labels)?;
                    }
                }
            }
            StmtNode::Destructure { pattern, init, .. } => {
                Self::check_static_block_pattern(pattern)?;
                if let Some(init) = init {
                    Self::check_static_block_expr(init)?;
                }
            }
            StmtNode::Break(_) | StmtNode::Continue(_) | StmtNode::Empty => {}
        }
        Ok(())
    }

    fn check_static_block_expr(expr: &Expr) -> error::Result<()> {
        match expr {
            Expr::Ident(name) => Self::check_static_block_name(name),
            Expr::Await(_) => Err(error::Error::syntax(
                "await is not allowed in class static block".to_string(),
            )),
            Expr::Array(elements) => {
                for element in elements {
                    Self::check_static_block_expr(element)?;
                }
                Ok(())
            }
            Expr::Object(props) => {
                for prop in props {
                    Self::check_static_block_property_key(&prop.key)?;
                    Self::check_static_block_expr(&prop.value)?;
                }
                Ok(())
            }
            Expr::Unary(_, expr)
            | Expr::Update(_, _, expr)
            | Expr::Spread(expr)
            | Expr::YieldDelegate(expr)
            | Expr::OptionalChain(expr) => Self::check_static_block_expr(expr),
            Expr::Binary(_, left, right)
            | Expr::Logical(_, left, right)
            | Expr::Assign(_, left, right) => {
                Self::check_static_block_expr(left)?;
                Self::check_static_block_expr(right)
            }
            Expr::Conditional(cond, then_expr, else_expr) => {
                Self::check_static_block_expr(cond)?;
                Self::check_static_block_expr(then_expr)?;
                Self::check_static_block_expr(else_expr)
            }
            Expr::Call { callee, args, .. } => {
                Self::check_static_block_expr(callee)?;
                for arg in args {
                    Self::check_static_block_expr(arg)?;
                }
                Ok(())
            }
            Expr::CallWithThis {
                callee,
                this_value,
                args,
            } => {
                Self::check_static_block_expr(callee)?;
                Self::check_static_block_expr(this_value)?;
                for arg in args {
                    Self::check_static_block_expr(arg)?;
                }
                Ok(())
            }
            Expr::DecoratorAddInitializer { initializer, .. } => {
                Self::check_static_block_expr(initializer)
            }
            Expr::DecoratorAccess {
                receiver,
                key,
                value,
                ..
            } => {
                Self::check_static_block_expr(receiver)?;
                Self::check_static_block_expr(key)?;
                if let Some(value) = value {
                    Self::check_static_block_expr(value)?;
                }
                Ok(())
            }
            Expr::ImportCall { specifier, options } => {
                Self::check_static_block_expr(specifier)?;
                if let Some(options) = options {
                    Self::check_static_block_expr(options)?;
                }
                Ok(())
            }
            Expr::New { callee, args } => {
                Self::check_static_block_expr(callee)?;
                for arg in args {
                    Self::check_static_block_expr(arg)?;
                }
                Ok(())
            }
            Expr::Member {
                object, property, ..
            } => {
                Self::check_static_block_expr(object)?;
                Self::check_static_block_expr(property)
            }
            Expr::PrivateGet { object, .. } => Self::check_static_block_expr(object),
            Expr::PrivateSet { object, value, .. } | Expr::PrivateInit { object, value, .. } => {
                Self::check_static_block_expr(object)?;
                Self::check_static_block_expr(value)
            }
            Expr::PrivateDefineAccessor { object, .. } => Self::check_static_block_expr(object),
            Expr::PublicFieldInit {
                object,
                computed_name,
                value,
                ..
            } => {
                Self::check_static_block_expr(object)?;
                if let Some(computed_name) = computed_name {
                    Self::check_static_block_expr(computed_name)?;
                }
                Self::check_static_block_expr(value)
            }
            Expr::PrivateFieldDecl {
                init: Some(init), ..
            } => Self::check_static_block_expr(init),
            Expr::TemplateInterp { exprs, .. } => {
                for expr in exprs {
                    Self::check_static_block_expr(expr)?;
                }
                Ok(())
            }
            Expr::TaggedTemplate { tag, exprs, .. } => {
                Self::check_static_block_expr(tag)?;
                for expr in exprs {
                    Self::check_static_block_expr(expr)?;
                }
                Ok(())
            }
            Expr::Sequence(exprs) => {
                for expr in exprs {
                    Self::check_static_block_expr(expr)?;
                }
                Ok(())
            }
            Expr::Class(cls) => {
                if let Some(name) = &cls.name {
                    Self::check_static_block_name(name)?;
                }
                if let Some(superclass) = &cls.superclass {
                    Self::check_static_block_expr(superclass)?;
                }
                for method in &cls.methods {
                    if let Some(computed_name) = &method.computed_name {
                        Self::check_static_block_expr(computed_name)?;
                    }
                }
                for field in &cls.public_fields {
                    if let Some(computed_name) = &field.computed_name {
                        Self::check_static_block_expr(computed_name)?;
                    }
                    if let Some(init) = &field.init {
                        Self::check_static_block_expr(init)?;
                    }
                }
                for field in &cls.private_fields {
                    if let Some(init) = &field.init {
                        Self::check_static_block_expr(init)?;
                    }
                }
                Ok(())
            }
            Expr::Arrow(func) => {
                for param in &func.params {
                    Self::check_static_block_name(param)?;
                }
                if let Some(rest) = &func.rest_param {
                    Self::check_static_block_name(rest)?;
                }
                for default in func.param_defaults.iter().flatten() {
                    Self::check_static_block_expr(default)?;
                }
                for pattern in &func.param_decls {
                    Self::check_static_block_pattern(pattern)?;
                }
                // Destructuring arrow parameters are lowered to line-zero
                // prelude declarations. They remain parameter syntax for the
                // static-block early error even though the executable body
                // must otherwise stay behind the nested-function boundary.
                for statement in func.body.iter().take_while(|statement| statement.line == 0) {
                    if let StmtNode::Destructure { pattern, .. } = &statement.node {
                        Self::check_static_block_pattern(pattern)?;
                    }
                }
                if func
                    .body
                    .iter()
                    .any(Self::class_field_initializer_contains_arguments_stmt)
                {
                    return Err(error::Error::syntax(
                        "'arguments' is not allowed in class static block".to_string(),
                    ));
                }
                Ok(())
            }
            Expr::Function(_) => Ok(()),
            Expr::Yield(_) => Err(error::Error::syntax(
                "yield is not allowed in class static block".to_string(),
            )),
            Expr::Number(_)
            | Expr::BigInt(_)
            | Expr::String(_)
            | Expr::TemplateStr(_)
            | Expr::Bool(_)
            | Expr::Null
            | Expr::Undefined
            | Expr::This
            | Expr::Super
            | Expr::ArrayHole
            | Expr::Regex(_, _)
            | Expr::NewTarget
            | Expr::ImportMeta => Ok(()),
            Expr::PrivateFieldDecl { init: None, .. } => Ok(()),
        }
    }

    fn check_static_block_pattern(pattern: &Pattern) -> error::Result<()> {
        match pattern {
            Pattern::Ident(name) => Self::check_static_block_name(name),
            Pattern::Array(elements) => {
                for element in elements {
                    Self::check_static_block_pattern(element)?;
                }
                Ok(())
            }
            Pattern::Object(props, rest) => {
                for (key, value) in props {
                    Self::check_static_block_property_key(key)?;
                    Self::check_static_block_pattern(value)?;
                }
                if let Some(rest) = rest {
                    Self::check_static_block_pattern(rest)?;
                }
                Ok(())
            }
            Pattern::Assign(pattern, expr) => {
                Self::check_static_block_pattern(pattern)?;
                Self::check_static_block_expr(expr)
            }
            Pattern::Rest(pattern) => Self::check_static_block_pattern(pattern),
            Pattern::Hole => Ok(()),
        }
    }

    fn check_static_block_property_key(key: &PropertyKey) -> error::Result<()> {
        match key {
            PropertyKey::Computed(expr) | PropertyKey::Spread(expr) => {
                Self::check_static_block_expr(expr)
            }
            PropertyKey::Ident(_) | PropertyKey::String(_) | PropertyKey::Number(_) => Ok(()),
        }
    }

    fn check_static_block_name(name: &Arc<str>) -> error::Result<()> {
        if matches!(&**name, "await" | "arguments") {
            return Err(error::Error::syntax(format!(
                "'{}' is not allowed in class static block",
                name
            )));
        }
        Ok(())
    }

    /// SetFunctionName for `var x = <function/class>`: if `value` is an
    /// anonymous function/arrow/class and `name` is a plain identifier, set
    /// its display name to it.
    fn name_function_from_ident(value: &mut Expr, name: &Arc<str>) {
        match value {
            Expr::Function(f) if f.name.is_none() => f.name = Some(name.clone()),
            Expr::Arrow(f) if f.name.is_none() => f.name = Some(name.clone()),
            Expr::Class(c) if c.name.is_none() && c.inferred_name.is_none() => {
                c.inferred_name = Some(name.clone());
            }
            _ => {}
        }
    }

    fn read_property_name(&mut self) -> error::Result<String> {
        // Accept identifiers and keywords as property names after `.`
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => s,
            other => {
                if let Some(keyword) = other.as_keyword_str() {
                    keyword.into()
                } else {
                    return Err(error::Error::syntax(format!(
                        "Expected property name after ., got {:?}",
                        other
                    )));
                }
            }
        };
        self.advance();
        Ok(name)
    }

    fn parse_class_decl(&mut self) -> error::Result<Stmt> {
        // Parse a class declaration as a statement that evaluates the class expr.
        let decorators = if self.check(&TokenKind::At) {
            self.parse_decorator_list()?
        } else {
            Vec::new()
        };
        if !self.check(&TokenKind::Class) {
            return Err(error::Error::syntax(
                "Class decorators must be followed by a class declaration".to_string(),
            ));
        }
        let cls = self.parse_class_body_with_decorators(true, decorators)?;
        Ok(self.stmt(StmtNode::ExprStmt(Expr::Class(cls))))
    }

    fn parse_decorator_list(&mut self) -> error::Result<Vec<Expr>> {
        let mut decorators = Vec::new();
        while self.eat(&TokenKind::At) {
            decorators.push(self.parse_decorator_expression()?);
        }
        Ok(decorators)
    }

    fn parse_decorator_expression(&mut self) -> error::Result<Expr> {
        if self.eat(&TokenKind::LParen) {
            let expression = self.with_in_allowed(|parser| parser.parse_expr())?;
            self.expect(&TokenKind::RParen, ")")?;
            return Ok(expression);
        }

        let mut expression = match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Expr::Ident(Arc::from(name.as_str()))
            }
            TokenKind::Await if self.await_as_identifier_allowed() => {
                self.advance();
                Expr::Ident(Arc::from("await"))
            }
            TokenKind::Yield if self.yield_as_identifier_allowed() => {
                self.advance();
                Expr::Ident(Arc::from("yield"))
            }
            token => {
                return Err(error::Error::syntax(format!(
                    "Invalid decorator expression starting with {:?}",
                    token
                )))
            }
        };

        while self.eat(&TokenKind::Dot) {
            if let TokenKind::PrivateName(name) = self.peek().clone() {
                self.advance();
                expression = Expr::PrivateGet {
                    object: Box::new(expression),
                    name: Arc::from(name.as_str()),
                    optional: false,
                };
            } else {
                let name = self.read_property_name()?;
                expression = Expr::Member {
                    object: Box::new(expression),
                    property: Box::new(Expr::String(Arc::from(name.as_str()))),
                    computed: false,
                    optional: false,
                    optional_chain: false,
                };
            }
        }

        if self.eat(&TokenKind::LParen) {
            let args = self.parse_args()?;
            self.expect(&TokenKind::RParen, ")")?;
            expression = Expr::Call {
                callee: Box::new(expression),
                args,
                optional: false,
                optional_chain: false,
            };
        }
        Ok(expression)
    }

    fn record_private_bound_name(
        private_bound_names: &mut Vec<PrivateBoundName>,
        name: &str,
        is_static: bool,
        kind: crate::ast::PropKind,
    ) -> error::Result<()> {
        if name == "constructor" {
            return Err(error::Error::syntax(
                "Private name cannot be constructor".to_string(),
            ));
        }
        let Some(entry) = private_bound_names
            .iter_mut()
            .find(|entry| entry.name.as_ref() == name)
        else {
            let (getter, setter, other) = match kind {
                crate::ast::PropKind::Get => (true, false, false),
                crate::ast::PropKind::Set => (false, true, false),
                _ => (false, false, true),
            };
            private_bound_names.push(PrivateBoundName {
                name: Arc::from(name),
                is_static,
                getter,
                setter,
                other,
            });
            return Ok(());
        };

        if entry.is_static != is_static {
            return Err(error::Error::syntax(format!(
                "Duplicate private name #{} in class body",
                name
            )));
        }

        match kind {
            crate::ast::PropKind::Get if !entry.getter && entry.setter && !entry.other => {
                entry.getter = true;
                Ok(())
            }
            crate::ast::PropKind::Set if entry.getter && !entry.setter && !entry.other => {
                entry.setter = true;
                Ok(())
            }
            _ => Err(error::Error::syntax(format!(
                "Duplicate private name #{} in class body",
                name
            ))),
        }
    }

    fn validate_private_names_statement_list(
        body: &[Stmt],
        names: &[Arc<str>],
    ) -> error::Result<()> {
        for stmt in body {
            Self::validate_private_names_stmt(stmt, names)?;
        }
        Ok(())
    }

    fn validate_private_names_stmt(stmt: &Stmt, names: &[Arc<str>]) -> error::Result<()> {
        match &stmt.node {
            StmtNode::VarDecl { decls, .. } => {
                for (_, init) in decls {
                    if let Some(init) = init {
                        Self::validate_private_names_expr(init, names)?;
                    }
                }
                Ok(())
            }
            StmtNode::ExprStmt(expr) | StmtNode::Throw(expr) => {
                Self::validate_private_names_expr(expr, names)
            }
            StmtNode::Block(body) => Self::validate_private_names_statement_list(body, names),
            StmtNode::If { cond, then, else_ } => {
                Self::validate_private_names_expr(cond, names)?;
                Self::validate_private_names_stmt(then, names)?;
                if let Some(else_) = else_ {
                    Self::validate_private_names_stmt(else_, names)?;
                }
                Ok(())
            }
            StmtNode::While { cond, body } | StmtNode::DoWhile { body, cond } => {
                Self::validate_private_names_expr(cond, names)?;
                Self::validate_private_names_stmt(body, names)
            }
            StmtNode::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    Self::validate_private_names_stmt(init, names)?;
                }
                if let Some(cond) = cond {
                    Self::validate_private_names_expr(cond, names)?;
                }
                if let Some(update) = update {
                    Self::validate_private_names_expr(update, names)?;
                }
                Self::validate_private_names_stmt(body, names)
            }
            StmtNode::ForIn { left, right, body } => {
                Self::validate_private_names_stmt(left, names)?;
                Self::validate_private_names_expr(right, names)?;
                Self::validate_private_names_stmt(body, names)
            }
            StmtNode::ForOf {
                left, right, body, ..
            } => {
                Self::validate_private_names_stmt(left, names)?;
                Self::validate_private_names_expr(right, names)?;
                Self::validate_private_names_stmt(body, names)
            }
            StmtNode::With { object, body } => {
                Self::validate_private_names_expr(object, names)?;
                Self::validate_private_names_stmt(body, names)
            }
            StmtNode::Return(expr) => {
                if let Some(expr) = expr {
                    Self::validate_private_names_expr(expr, names)?;
                }
                Ok(())
            }
            StmtNode::TryCatch {
                try_body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                Self::validate_private_names_stmt(try_body, names)?;
                if let Some(catch_param) = catch_param {
                    Self::validate_private_names_pattern(catch_param, names)?;
                }
                if let Some(catch_body) = catch_body {
                    Self::validate_private_names_stmt(catch_body, names)?;
                }
                if let Some(finally_body) = finally_body {
                    Self::validate_private_names_stmt(finally_body, names)?;
                }
                Ok(())
            }
            StmtNode::FunctionDecl(func) => Self::validate_private_names_function(func, names),
            StmtNode::Labeled(_, body) => Self::validate_private_names_stmt(body, names),
            StmtNode::Switch { disc, cases } => {
                Self::validate_private_names_expr(disc, names)?;
                for case in cases {
                    if let Some(test) = &case.test {
                        Self::validate_private_names_expr(test, names)?;
                    }
                    Self::validate_private_names_statement_list(&case.body, names)?;
                }
                Ok(())
            }
            StmtNode::Destructure { pattern, init, .. } => {
                Self::validate_private_names_pattern(pattern, names)?;
                if let Some(init) = init {
                    Self::validate_private_names_expr(init, names)?;
                }
                Ok(())
            }
            StmtNode::Break(_) | StmtNode::Continue(_) | StmtNode::Empty => Ok(()),
        }
    }

    fn validate_private_names_expr(expr: &Expr, names: &[Arc<str>]) -> error::Result<()> {
        match expr {
            Expr::TaggedTemplate { tag, exprs, .. } => {
                Self::validate_private_names_expr(tag, names)?;
                for expr in exprs {
                    Self::validate_private_names_expr(expr, names)?;
                }
                Ok(())
            }
            Expr::TemplateInterp { exprs, .. } | Expr::Array(exprs) | Expr::Sequence(exprs) => {
                for expr in exprs {
                    Self::validate_private_names_expr(expr, names)?;
                }
                Ok(())
            }
            Expr::Object(props) => {
                for prop in props {
                    Self::validate_private_names_property_key(&prop.key, names)?;
                    Self::validate_private_names_expr(&prop.value, names)?;
                }
                Ok(())
            }
            Expr::Function(func) | Expr::Arrow(func) => {
                Self::validate_private_names_function(func, names)
            }
            Expr::Class(cls) => Self::validate_private_names_class(cls, names),
            Expr::PrivateGet { object, name, .. } => {
                Self::validate_private_name_use(name, names)?;
                if matches!(object.as_ref(), Expr::Super) {
                    return Err(error::Error::syntax(
                        "Private name cannot be accessed on super".to_string(),
                    ));
                }
                Self::validate_private_names_expr(object, names)
            }
            Expr::PrivateSet {
                object,
                name,
                value,
            }
            | Expr::PrivateInit {
                object,
                name,
                value,
                ..
            } => {
                Self::validate_private_name_use(name, names)?;
                if matches!(object.as_ref(), Expr::Super) {
                    return Err(error::Error::syntax(
                        "Private name cannot be accessed on super".to_string(),
                    ));
                }
                Self::validate_private_names_expr(object, names)?;
                Self::validate_private_names_expr(value, names)
            }
            Expr::PrivateDefineAccessor {
                object,
                name,
                get,
                set,
            } => {
                Self::validate_private_name_use(name, names)?;
                Self::validate_private_names_expr(object, names)?;
                if let Some(get) = get {
                    Self::validate_private_names_expr(get, names)?;
                }
                if let Some(set) = set {
                    Self::validate_private_names_expr(set, names)?;
                }
                Ok(())
            }
            Expr::PublicFieldInit {
                object,
                computed_name,
                value,
                ..
            } => {
                Self::validate_private_names_expr(object, names)?;
                if let Some(computed_name) = computed_name {
                    Self::validate_private_names_expr(computed_name, names)?;
                }
                Self::validate_private_names_expr(value, names)
            }
            Expr::PrivateFieldDecl {
                name,
                init: Some(init),
            } => {
                Self::validate_private_name_use(name, names)?;
                Self::validate_private_names_expr(init, names)
            }
            Expr::PrivateFieldDecl { name, init: None } => {
                Self::validate_private_name_use(name, names)
            }
            Expr::Unary(_, inner)
            | Expr::Update(_, _, inner)
            | Expr::Spread(inner)
            | Expr::Await(inner)
            | Expr::YieldDelegate(inner)
            | Expr::OptionalChain(inner) => Self::validate_private_names_expr(inner, names),
            Expr::Yield(Some(inner)) => Self::validate_private_names_expr(inner, names),
            Expr::Binary(_, left, right)
            | Expr::Logical(_, left, right)
            | Expr::Assign(_, left, right) => {
                Self::validate_private_names_expr(left, names)?;
                Self::validate_private_names_expr(right, names)
            }
            Expr::Conditional(cond, then_expr, else_expr) => {
                Self::validate_private_names_expr(cond, names)?;
                Self::validate_private_names_expr(then_expr, names)?;
                Self::validate_private_names_expr(else_expr, names)
            }
            Expr::Call { callee, args, .. } | Expr::New { callee, args } => {
                Self::validate_private_names_expr(callee, names)?;
                for arg in args {
                    Self::validate_private_names_expr(arg, names)?;
                }
                Ok(())
            }
            Expr::CallWithThis {
                callee,
                this_value,
                args,
            } => {
                Self::validate_private_names_expr(callee, names)?;
                Self::validate_private_names_expr(this_value, names)?;
                for arg in args {
                    Self::validate_private_names_expr(arg, names)?;
                }
                Ok(())
            }
            Expr::DecoratorAddInitializer { initializer, .. } => {
                Self::validate_private_names_expr(initializer, names)
            }
            Expr::DecoratorAccess {
                receiver,
                key,
                value,
                ..
            } => {
                Self::validate_private_names_expr(receiver, names)?;
                Self::validate_private_names_expr(key, names)?;
                if let Some(value) = value {
                    Self::validate_private_names_expr(value, names)?;
                }
                Ok(())
            }
            Expr::ImportCall { specifier, options } => {
                Self::validate_private_names_expr(specifier, names)?;
                if let Some(options) = options {
                    Self::validate_private_names_expr(options, names)?;
                }
                Ok(())
            }
            Expr::Member {
                object, property, ..
            } => {
                Self::validate_private_names_expr(object, names)?;
                Self::validate_private_names_expr(property, names)
            }
            Expr::Number(_)
            | Expr::BigInt(_)
            | Expr::String(_)
            | Expr::TemplateStr(_)
            | Expr::Bool(_)
            | Expr::Null
            | Expr::Undefined
            | Expr::Ident(_)
            | Expr::This
            | Expr::Super
            | Expr::ArrayHole
            | Expr::Regex(_, _)
            | Expr::NewTarget
            | Expr::ImportMeta
            | Expr::Yield(None) => Ok(()),
        }
    }

    fn validate_private_names_function(
        func: &FunctionExpr,
        names: &[Arc<str>],
    ) -> error::Result<()> {
        for default in func.param_defaults.iter().flatten() {
            Self::validate_private_names_expr(default, names)?;
        }
        for pattern in &func.param_decls {
            Self::validate_private_names_pattern(pattern, names)?;
        }
        Self::validate_private_names_statement_list(&func.body, names)
    }

    fn validate_private_names_class(cls: &ClassExpr, names: &[Arc<str>]) -> error::Result<()> {
        for decorator in &cls.decorators {
            Self::validate_private_names_expr(decorator, names)?;
        }
        if let Some(superclass) = &cls.superclass {
            Self::validate_private_names_expr(superclass, names)?;
        }

        let mut class_names = names.to_vec();
        for field in &cls.private_fields {
            class_names.push(field.name.clone());
        }
        for method in &cls.methods {
            if method.is_private {
                class_names.push(method.name.clone());
            }
        }
        for accessor in &cls.auto_accessors {
            if accessor.is_private {
                class_names.push(accessor.name.clone());
            }
        }

        for field in &cls.public_fields {
            for decorator in &field.decorators {
                Self::validate_private_names_expr(decorator, &class_names)?;
            }
            if let Some(computed_name) = &field.computed_name {
                Self::validate_private_names_expr(computed_name, &class_names)?;
            }
            if let Some(init) = &field.init {
                Self::validate_private_names_expr(init, &class_names)?;
            }
        }
        for method in &cls.methods {
            for decorator in &method.decorators {
                Self::validate_private_names_expr(decorator, &class_names)?;
            }
            if let Some(computed_name) = &method.computed_name {
                Self::validate_private_names_expr(computed_name, &class_names)?;
            }
            for default in method.param_defaults.iter().flatten() {
                Self::validate_private_names_expr(default, &class_names)?;
            }
            Self::validate_private_names_statement_list(&method.body, &class_names)?;
        }
        for block in &cls.static_blocks {
            Self::validate_private_names_statement_list(block, &class_names)?;
        }
        for field in &cls.private_fields {
            for decorator in &field.decorators {
                Self::validate_private_names_expr(decorator, &class_names)?;
            }
            if let Some(init) = &field.init {
                Self::validate_private_names_expr(init, &class_names)?;
            }
        }
        for accessor in &cls.auto_accessors {
            for decorator in &accessor.decorators {
                Self::validate_private_names_expr(decorator, &class_names)?;
            }
            if let Some(computed_name) = &accessor.computed_name {
                Self::validate_private_names_expr(computed_name, &class_names)?;
            }
            if let Some(init) = &accessor.init {
                Self::validate_private_names_expr(init, &class_names)?;
            }
        }
        Ok(())
    }

    fn validate_private_names_pattern(pattern: &Pattern, names: &[Arc<str>]) -> error::Result<()> {
        match pattern {
            Pattern::Ident(_) | Pattern::Hole => Ok(()),
            Pattern::Array(elements) => {
                for element in elements {
                    Self::validate_private_names_pattern(element, names)?;
                }
                Ok(())
            }
            Pattern::Object(props, rest) => {
                for (key, pattern) in props {
                    Self::validate_private_names_property_key(key, names)?;
                    Self::validate_private_names_pattern(pattern, names)?;
                }
                if let Some(rest) = rest {
                    Self::validate_private_names_pattern(rest, names)?;
                }
                Ok(())
            }
            Pattern::Assign(pattern, default) => {
                Self::validate_private_names_pattern(pattern, names)?;
                Self::validate_private_names_expr(default, names)
            }
            Pattern::Rest(pattern) => Self::validate_private_names_pattern(pattern, names),
        }
    }

    fn validate_private_names_property_key(
        key: &PropertyKey,
        names: &[Arc<str>],
    ) -> error::Result<()> {
        match key {
            PropertyKey::Computed(expr) | PropertyKey::Spread(expr) => {
                Self::validate_private_names_expr(expr, names)
            }
            PropertyKey::Ident(_) | PropertyKey::String(_) | PropertyKey::Number(_) => Ok(()),
        }
    }

    fn validate_private_name_use(name: &Arc<str>, names: &[Arc<str>]) -> error::Result<()> {
        if names.iter().any(|candidate| candidate == name) {
            return Ok(());
        }
        Err(error::Error::syntax(format!(
            "Private name #{} is not declared in this scope",
            name
        )))
    }

    fn parse_class_body(&mut self, is_declaration: bool) -> error::Result<ClassExpr> {
        self.parse_class_body_with_decorators(is_declaration, Vec::new())
    }

    fn parse_class_body_with_decorators(
        &mut self,
        is_declaration: bool,
        decorators: Vec<Expr>,
    ) -> error::Result<ClassExpr> {
        self.advance(); // 'class'
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => {
                if Self::is_reserved_identifier_reference_word(&s)
                    || Self::is_strict_identifier_reference_reserved(&s)
                    || s == "await"
                        && (self.source_type == SourceType::Module
                            || !self.await_as_identifier_allowed())
                {
                    return Err(error::Error::syntax(format!(
                        "'{}' is not allowed as a class name",
                        s
                    )));
                }
                self.advance();
                Some(Arc::from(s.as_str()))
            }
            TokenKind::Await
                if self.source_type != SourceType::Module && self.await_as_identifier_allowed() =>
            {
                self.advance();
                Some(Arc::from("await"))
            }
            TokenKind::Yield if self.yield_as_identifier_allowed() => {
                return Err(error::Error::syntax(
                    "'yield' is not allowed as a class name".to_string(),
                ));
            }
            _ => None,
        };
        let saved_strict_context = self.is_strict_context;
        self.is_strict_context = true;
        let superclass = if self.eat(&TokenKind::Extends) {
            if matches!(self.peek().clone(), TokenKind::LParen)
                && matches!(self.peek_at_tok(1).kind, TokenKind::RParen)
                && matches!(self.peek_at_tok(2).kind, TokenKind::Arrow)
            {
                return Err(error::Error::syntax(
                    "Invalid class heritage expression".to_string(),
                ));
            }
            if matches!(self.peek().clone(), TokenKind::Async)
                && !self.peek_at_tok(1).preceded_by_newline
                && matches!(self.peek_at_tok(1).kind, TokenKind::LParen)
                && matches!(self.peek_at_tok(2).kind, TokenKind::RParen)
                && matches!(self.peek_at_tok(3).kind, TokenKind::Arrow)
            {
                return Err(error::Error::syntax(
                    "Invalid class heritage expression".to_string(),
                ));
            }
            let heritage = self.parse_postfix()?;
            Some(Box::new(heritage))
        } else {
            None
        };
        self.expect(&TokenKind::LBrace, "{")?;
        let mut elements = Vec::new();
        let mut methods = Vec::new();
        let mut static_blocks: Vec<Vec<Stmt>> = Vec::new();
        let mut private_fields: Vec<crate::ast::PrivateFieldDecl> = Vec::new();
        let mut public_fields: Vec<crate::ast::PublicFieldDecl> = Vec::new();
        let mut auto_accessors: Vec<crate::ast::AutoAccessorDecl> = Vec::new();
        let mut seen_constructor = false;
        let mut private_bound_names = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let element_decorators = if self.check(&TokenKind::At) {
                self.parse_decorator_list()?
            } else {
                Vec::new()
            };
            if self.eat(&TokenKind::Semicolon) {
                if !element_decorators.is_empty() {
                    return Err(error::Error::syntax(
                        "Decorators must precede a class element".to_string(),
                    ));
                }
                continue;
            }
            // static { ... } initialization block
            if self.check(&TokenKind::Static)
                && !self.peek_at_tok(0).had_escape
                && matches!(self.peek_at_tok(1).kind, TokenKind::LBrace)
            {
                if !element_decorators.is_empty() {
                    return Err(error::Error::syntax(
                        "Class static blocks cannot be decorated".to_string(),
                    ));
                }
                self.advance(); // static
                let block = self.parse_static_block_body()?;
                let idx = static_blocks.len();
                static_blocks.push(block);
                elements.push(crate::ast::ClassElement::StaticBlock(idx));
                continue;
            }
            let is_static = self.check(&TokenKind::Static)
                && !self.peek_at_tok(0).had_escape
                && !matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::Assign | TokenKind::Semicolon | TokenKind::RBrace
                )
                && {
                    self.advance();
                    true
                };
            let is_auto_accessor = matches!(
                self.peek(),
                TokenKind::Ident(name) if name == "accessor"
            ) && !self.peek_at_tok(0).had_escape
                && !self.peek_at_tok(1).preceded_by_newline
                && !matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::LParen
                        | TokenKind::Assign
                        | TokenKind::Semicolon
                        | TokenKind::RBrace
                        | TokenKind::Star
                );
            if is_auto_accessor {
                self.advance(); // accessor
                let (name, computed_name, is_private): (Arc<str>, Option<Box<Expr>>, bool) =
                    if self.eat(&TokenKind::LBracket) {
                        let expression = self.with_in_allowed(|parser| parser.parse_assign())?;
                        self.expect(&TokenKind::RBracket, "]")?;
                        (Arc::from(""), Some(Box::new(expression)), false)
                    } else if let TokenKind::PrivateName(name) = self.peek().clone() {
                        self.advance();
                        Self::record_private_bound_name(
                            &mut private_bound_names,
                            &name,
                            is_static,
                            crate::ast::PropKind::Normal,
                        )?;
                        (Arc::from(name.as_str()), None, true)
                    } else {
                        let name = match self.peek().clone() {
                            TokenKind::Number(number) | TokenKind::LegacyNumber(number) => {
                                self.advance();
                                crate::value::num_to_string(number)
                            }
                            TokenKind::String(name) => {
                                self.advance();
                                name
                            }
                            _ => self.read_property_name()?,
                        };
                        (Arc::from(name.as_str()), None, false)
                    };
                if !is_private && computed_name.is_none() && name.as_ref() == "constructor" {
                    return Err(error::Error::syntax(
                        "Class field cannot be named constructor".to_string(),
                    ));
                }
                if !is_private
                    && computed_name.is_none()
                    && is_static
                    && name.as_ref() == "prototype"
                {
                    return Err(error::Error::syntax(
                        "Static class element cannot be named prototype".to_string(),
                    ));
                }
                let init = if self.eat(&TokenKind::Assign) {
                    let mut init = self.parse_class_field_initializer()?;
                    Self::reject_class_field_initializer_contains_arguments(&init)?;
                    if computed_name.is_none() {
                        let function_name = if is_private {
                            Arc::from(format!("#{}", name).as_str())
                        } else {
                            name.clone()
                        };
                        Self::name_function_from_ident(&mut init, &function_name);
                    }
                    Some(Box::new(init))
                } else {
                    None
                };
                self.expect_semi()?;
                let index = auto_accessors.len();
                auto_accessors.push(crate::ast::AutoAccessorDecl {
                    decorators: element_decorators,
                    name,
                    computed_name,
                    init,
                    is_static,
                    is_private,
                });
                elements.push(crate::ast::ClassElement::AutoAccessor(index));
                continue;
            }
            // Private field declaration: #name = init  or  #name;
            // Private method/accessor: #name(params) / get #name() / set #name(v)
            let is_async_token = match self.peek().clone() {
                TokenKind::Async => true,
                TokenKind::Ident(s) => s == "async" && !self.peek_at_tok(0).had_escape,
                _ => false,
            };
            let is_private_async_method = is_async_token
                && !self.peek_at_tok(1).preceded_by_newline
                && (matches!(self.peek_at_tok(1).kind, TokenKind::PrivateName(_))
                    || (matches!(self.peek_at_tok(1).kind, TokenKind::Star)
                        && matches!(self.peek_at_tok(2).kind, TokenKind::PrivateName(_))));
            let is_private_generator_method = matches!(self.peek(), TokenKind::Star)
                && matches!(self.peek_at_tok(1).kind, TokenKind::PrivateName(_));
            if is_private_async_method || is_private_generator_method {
                let is_async = if is_private_async_method {
                    self.advance(); // async
                    true
                } else {
                    false
                };
                let is_generator = self.eat(&TokenKind::Star);
                let name = if let TokenKind::PrivateName(name) = self.peek().clone() {
                    self.advance();
                    name
                } else {
                    unreachable!()
                };
                Self::record_private_bound_name(
                    &mut private_bound_names,
                    &name,
                    is_static,
                    crate::ast::PropKind::Method,
                )?;
                let (params, param_defaults, rest_param, dstr_decls) =
                    self.parse_params_scoped(is_generator, is_async, true)?;
                Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
                let mut body = self.parse_fn_body(true, false, is_generator, is_async)?;
                let body_contains_use_strict = self.last_fn_body_use_strict_directive;
                let has_destructuring_params = !dstr_decls.is_empty();
                Self::reject_use_strict_with_non_simple_params(
                    body_contains_use_strict,
                    &param_defaults,
                    rest_param.as_ref(),
                    has_destructuring_params,
                )?;
                {
                    let mut pre = Self::dstr_prelude_from(dstr_decls);
                    pre.append(&mut body);
                    body = pre;
                }
                let idx = methods.len();
                methods.push(ClassMethod {
                    decorators: element_decorators,
                    name: Arc::from(name.as_str()),
                    computed_name: None,
                    params,
                    param_defaults,
                    rest_param,
                    body,
                    is_static,
                    is_constructor: false,
                    is_async,
                    is_generator,
                    kind: crate::ast::PropKind::Method,
                    is_private: true,
                });
                elements.push(crate::ast::ClassElement::Method(idx));
                continue;
            }
            if matches!(
                self.peek().clone(),
                TokenKind::Ident(ref s)
                    if (s == "get" || s == "set")
                        && !self.tokens[self.pos].had_escape
                        && matches!(self.peek_at_tok(1).kind, TokenKind::PrivateName(_))
            ) {
                let kind = if matches!(self.peek().clone(), TokenKind::Ident(ref s) if s == "get") {
                    crate::ast::PropKind::Get
                } else {
                    crate::ast::PropKind::Set
                };
                self.advance(); // get/set
                let name = if let TokenKind::PrivateName(name) = self.peek().clone() {
                    self.advance();
                    name
                } else {
                    unreachable!()
                };
                Self::record_private_bound_name(&mut private_bound_names, &name, is_static, kind)?;
                let (params, param_defaults, rest_param, dstr_decls) =
                    self.parse_params_scoped(false, false, true)?;
                Self::reject_accessor_parameter_early_errors(kind, &params, rest_param.as_ref())?;
                Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
                let mut body = self.parse_fn_body(true, false, false, false)?;
                let body_contains_use_strict = self.last_fn_body_use_strict_directive;
                let has_destructuring_params = !dstr_decls.is_empty();
                Self::reject_use_strict_with_non_simple_params(
                    body_contains_use_strict,
                    &param_defaults,
                    rest_param.as_ref(),
                    has_destructuring_params,
                )?;
                {
                    let mut pre = Self::dstr_prelude_from(dstr_decls);
                    pre.append(&mut body);
                    body = pre;
                }
                let idx = methods.len();
                methods.push(ClassMethod {
                    decorators: element_decorators,
                    name: Arc::from(name.as_str()),
                    computed_name: None,
                    params,
                    param_defaults,
                    rest_param,
                    body,
                    is_static,
                    is_constructor: false,
                    is_async: false,
                    is_generator: false,
                    kind,
                    is_private: true,
                });
                elements.push(crate::ast::ClassElement::Method(idx));
                continue;
            }
            if let TokenKind::PrivateName(name) = self.peek().clone() {
                // Peek ahead: if next is `(`, this is a private method.
                let is_private_method = matches!(self.peek_at_tok(1).kind, TokenKind::LParen);
                if is_private_method {
                    self.advance(); // consume #name
                    Self::record_private_bound_name(
                        &mut private_bound_names,
                        &name,
                        is_static,
                        crate::ast::PropKind::Method,
                    )?;
                    let (params, param_defaults, rest_param, dstr_decls) =
                        self.parse_params_scoped(false, false, true)?;
                    Self::reject_duplicate_formal_params(
                        &params,
                        &dstr_decls,
                        rest_param.as_ref(),
                    )?;
                    let mut body = self.parse_fn_body(true, false, false, false)?;
                    let body_contains_use_strict = self.last_fn_body_use_strict_directive;
                    let has_destructuring_params = !dstr_decls.is_empty();
                    Self::reject_use_strict_with_non_simple_params(
                        body_contains_use_strict,
                        &param_defaults,
                        rest_param.as_ref(),
                        has_destructuring_params,
                    )?;
                    {
                        let mut pre = Self::dstr_prelude_from(dstr_decls);
                        pre.append(&mut body);
                        body = pre;
                    }
                    let idx = methods.len();
                    methods.push(ClassMethod {
                        decorators: element_decorators,
                        name: Arc::from(name.as_str()),
                        computed_name: None,
                        params,
                        param_defaults,
                        rest_param,
                        body,
                        is_static,
                        is_constructor: false,
                        is_async: false,
                        is_generator: false,
                        kind: crate::ast::PropKind::Method,
                        is_private: true,
                    });
                    elements.push(crate::ast::ClassElement::Method(idx));
                    continue;
                }
                self.advance();
                Self::record_private_bound_name(
                    &mut private_bound_names,
                    &name,
                    is_static,
                    crate::ast::PropKind::Normal,
                )?;
                let init = if self.eat(&TokenKind::Assign) {
                    let init = self.parse_class_field_initializer()?;
                    Self::reject_class_field_initializer_contains_arguments(&init)?;
                    Some(Box::new(init))
                } else {
                    None
                };
                self.expect_semi()?;
                let idx = private_fields.len();
                private_fields.push(crate::ast::PrivateFieldDecl {
                    decorators: element_decorators,
                    name: Arc::from(name.as_str()),
                    init,
                    is_static,
                    kind: crate::ast::PropKind::Normal,
                });
                elements.push(crate::ast::ClassElement::PrivateField(idx));
                continue;
            }
            // Getter/setter in class body.
            let (is_getter, is_setter) = match self.peek().clone() {
                TokenKind::Ident(s)
                    if (s == "get" || s == "set")
                        && !self.tokens[self.pos].had_escape
                        && !self.peek_at_tok(1).preceded_by_newline
                        && !matches!(
                            self.peek_at_tok(1).kind,
                            TokenKind::LParen
                                | TokenKind::Assign
                                | TokenKind::Semicolon
                                | TokenKind::Star
                                | TokenKind::RBrace
                        ) =>
                {
                    (s == "get", s == "set")
                }
                _ => (false, false),
            };
            if is_getter || is_setter {
                self.advance();
            }
            let is_async_method = !is_getter
                && !is_setter
                && matches!(self.peek(), TokenKind::Async)
                && !self.peek_at_tok(1).preceded_by_newline
                && !matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::LParen
                        | TokenKind::Assign
                        | TokenKind::Semicolon
                        | TokenKind::RBrace
                );
            if is_async_method {
                self.advance();
            }
            let is_generator_method = !is_getter && !is_setter && self.eat(&TokenKind::Star);
            // Computed method name: [expr]
            let computed_name = if self.check(&TokenKind::LBracket) {
                self.advance();
                let e = self.with_in_allowed(|p| p.parse_assign())?;
                self.expect(&TokenKind::RBracket, "]")?;
                Some(Box::new(e))
            } else {
                None
            };
            let method_name: Arc<str> = if computed_name.is_some() {
                // Placeholder name; the compiler uses computed_name for the actual key.
                Arc::from("")
            } else {
                // Class method names can be identifiers, keywords, numbers,
                // strings, or computed expressions.
                match self.peek().clone() {
                    TokenKind::Number(n) | TokenKind::LegacyNumber(n) => {
                        self.advance();
                        Arc::from(crate::value::num_to_string(n).as_str())
                    }
                    TokenKind::String(s) => {
                        self.advance();
                        Arc::from(s.as_str())
                    }
                    _ => Arc::from(self.read_property_name()?.as_str()),
                }
            };
            let has_params = self.check(&TokenKind::LParen);
            let is_constructor = !is_getter
                && !is_setter
                && !is_async_method
                && !is_generator_method
                && !is_static
                && computed_name.is_none()
                && method_name.as_ref() == "constructor"
                && has_params;
            if !is_getter && !is_setter && !is_async_method && !is_generator_method && !has_params {
                if computed_name.is_none() && method_name.as_ref() == "constructor" {
                    return Err(error::Error::syntax(
                        "Class field cannot be named constructor".to_string(),
                    ));
                }
                if is_static && computed_name.is_none() && method_name.as_ref() == "prototype" {
                    return Err(error::Error::syntax(
                        "Static class element cannot be named prototype".to_string(),
                    ));
                }
                if !element_decorators.is_empty()
                    && computed_name.is_some()
                    && !matches!(
                        self.peek(),
                        TokenKind::Assign | TokenKind::Semicolon | TokenKind::RBrace
                    )
                {
                    return Err(error::Error::syntax(
                        "Computed member expressions are not valid decorator expressions"
                            .to_string(),
                    ));
                }
                let init = if self.eat(&TokenKind::Assign) {
                    let mut init = self.parse_class_field_initializer()?;
                    Self::reject_class_field_initializer_contains_arguments(&init)?;
                    if computed_name.is_none() {
                        Self::name_function_from_ident(&mut init, &method_name);
                    }
                    Some(Box::new(init))
                } else {
                    None
                };
                self.expect_semi()?;
                let idx = public_fields.len();
                public_fields.push(crate::ast::PublicFieldDecl {
                    decorators: element_decorators,
                    name: method_name,
                    computed_name,
                    init,
                    is_static,
                });
                elements.push(crate::ast::ClassElement::PublicField(idx));
                continue;
            }
            if is_constructor {
                if !element_decorators.is_empty() {
                    return Err(error::Error::syntax(
                        "Class constructors cannot be decorated".to_string(),
                    ));
                }
                if seen_constructor {
                    return Err(error::Error::syntax(
                        "Duplicate constructor in class body".to_string(),
                    ));
                }
                seen_constructor = true;
            }
            if !is_static
                && computed_name.is_none()
                && method_name.as_ref() == "constructor"
                && (is_async_method || is_generator_method)
            {
                return Err(error::Error::syntax(
                    "Class constructor cannot be async or generator".to_string(),
                ));
            }
            if !is_static && (is_getter || is_setter) && method_name.as_ref() == "constructor" {
                return Err(error::Error::syntax(
                    "Class constructor cannot be an accessor".to_string(),
                ));
            }
            if is_static && computed_name.is_none() && method_name.as_ref() == "prototype" {
                return Err(error::Error::syntax(
                    "Static class element cannot be named prototype".to_string(),
                ));
            }
            let (params, param_defaults, rest_param, dstr_decls) =
                self.parse_params_scoped(is_generator_method, is_async_method, true)?;
            if is_getter || is_setter {
                Self::reject_accessor_parameter_early_errors(
                    if is_getter {
                        PropKind::Get
                    } else {
                        PropKind::Set
                    },
                    &params,
                    rest_param.as_ref(),
                )?;
            }
            Self::reject_duplicate_formal_params(&params, &dstr_decls, rest_param.as_ref())?;
            let super_call_allowed = superclass.is_some() && is_constructor;
            let mut body = self.parse_fn_body(
                true,
                super_call_allowed,
                is_generator_method,
                is_async_method,
            )?;
            let body_contains_use_strict = self.last_fn_body_use_strict_directive;
            let has_destructuring_params = !dstr_decls.is_empty();
            Self::reject_use_strict_with_non_simple_params(
                body_contains_use_strict,
                &param_defaults,
                rest_param.as_ref(),
                has_destructuring_params,
            )?;
            {
                let mut pre = Self::dstr_prelude_from(dstr_decls);
                pre.append(&mut body);
                body = pre;
            }
            let idx = methods.len();
            methods.push(ClassMethod {
                decorators: element_decorators,
                name: method_name,
                computed_name,
                params,
                param_defaults,
                rest_param,
                body,
                is_static,
                is_constructor,
                is_async: is_async_method,
                is_generator: is_generator_method,
                kind: if is_getter {
                    crate::ast::PropKind::Get
                } else if is_setter {
                    crate::ast::PropKind::Set
                } else {
                    crate::ast::PropKind::Method
                },
                is_private: false,
            });
            elements.push(crate::ast::ClassElement::Method(idx));
        }
        self.expect(&TokenKind::RBrace, "}")?;
        self.is_strict_context = saved_strict_context;
        Ok(ClassExpr {
            decorators,
            name,
            inferred_name: None,
            is_declaration,
            superclass,
            elements,
            methods,
            static_blocks,
            private_fields,
            public_fields,
            auto_accessors,
        })
    }
    #[allow(dead_code)]
    fn parse_async_or_expr_stmt(&mut self) -> error::Result<Stmt> {
        let e = self.parse_expr()?;
        self.expect_semi()?;
        Ok(self.stmt(StmtNode::ExprStmt(e)))
    }
    #[allow(dead_code)]
    fn parse_pattern(&mut self) -> error::Result<Pattern> {
        if let TokenKind::Ident(s) = self.peek().clone() {
            self.advance();
            Ok(Pattern::Ident(Arc::from(s.as_str())))
        } else {
            Err(error::Error::syntax("expected pattern".to_string()))
        }
    }

    /// Parse a destructuring pattern: `[a, b, ...rest]` or `{x, y: z, k = d}`.
    fn parse_destructure_pattern(&mut self) -> error::Result<Pattern> {
        // Bound recursion through nested array/object patterns
        // (`[[[[...a]]]] = x`), which self-recurses without going through
        // `parse_assign` and so would otherwise bypass `MAX_EXPR_DEPTH`.
        if self.expr_depth >= Self::MAX_EXPR_DEPTH {
            return Err(error::Error::syntax(format!(
                "Maximum expression nesting depth ({}) exceeded",
                Self::MAX_EXPR_DEPTH
            )));
        }
        self.expr_depth += 1;
        let result = self.parse_destructure_pattern_inner();
        self.expr_depth -= 1;
        result
    }

    fn parse_destructure_pattern_inner(&mut self) -> error::Result<Pattern> {
        match self.peek().clone() {
            TokenKind::LBracket => {
                self.advance(); // [
                let mut elems: Vec<Pattern> = Vec::new();
                while !self.check(&TokenKind::RBracket) {
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                        // Elision hole: `[a, , b]` consumes an element but
                        // binds nothing, so the next element keeps its index.
                        elems.push(Pattern::Hole);
                        continue;
                    }
                    if self.check(&TokenKind::Spread) {
                        self.advance();
                        let inner = self.parse_destructure_pattern()?;
                        elems.push(Pattern::Rest(Box::new(inner)));
                        // rest must be last
                        if !self.check(&TokenKind::RBracket) {
                            return Err(error::Error::syntax(
                                "rest element must be last in array pattern".to_string(),
                            ));
                        }
                        break;
                    }
                    let p = self.parse_destructure_pattern()?;
                    // default value: `[x = 4]`
                    let p = if self.eat(&TokenKind::Assign) {
                        let d = self.parse_assign()?;
                        Pattern::Assign(Box::new(p), d)
                    } else {
                        p
                    };
                    elems.push(p);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBracket, "]")?;
                Ok(Pattern::Array(elems))
            }
            TokenKind::LBrace => {
                self.advance(); // {
                let mut props: Vec<(PropertyKey, Pattern)> = Vec::new();
                let mut rest: Option<Box<Pattern>> = None;
                while !self.check(&TokenKind::RBrace) {
                    if self.check(&TokenKind::Spread) {
                        self.advance();
                        let inner = self.parse_destructure_pattern()?;
                        rest = Some(Box::new(inner));
                        // rest must be last
                        if !self.check(&TokenKind::RBrace) {
                            return Err(error::Error::syntax(
                                "rest element must be last in object pattern".to_string(),
                            ));
                        }
                        break;
                    }
                    let key: PropertyKey = match self.peek().clone() {
                        TokenKind::Ident(s) => {
                            self.advance();
                            PropertyKey::Ident(Arc::from(s.as_str()))
                        }
                        TokenKind::Yield if self.yield_as_identifier_allowed() => {
                            self.advance();
                            PropertyKey::Ident(Arc::from("yield"))
                        }
                        TokenKind::Await if self.await_as_identifier_allowed() => {
                            self.advance();
                            PropertyKey::Ident(Arc::from("await"))
                        }
                        TokenKind::String(s) => {
                            self.advance();
                            PropertyKey::String(Arc::from(s.as_str()))
                        }
                        TokenKind::Number(n) | TokenKind::LegacyNumber(n) => {
                            self.advance();
                            PropertyKey::Number(n)
                        }
                        TokenKind::LBracket => {
                            self.advance();
                            let e = self.with_in_allowed(|p| p.parse_assign())?;
                            self.expect(&TokenKind::RBracket, "]")?;
                            PropertyKey::Computed(Box::new(e))
                        }
                        other => {
                            if let Some(keyword) = other.as_keyword_str() {
                                self.advance();
                                PropertyKey::Ident(Arc::from(keyword))
                            } else {
                                return Err(error::Error::syntax(format!(
                                    "Expected property name in object pattern, got {:?}",
                                    other
                                )));
                            }
                        }
                    };
                    // `key: target` renames; otherwise bind to same name (ident/string only).
                    let target = if self.eat(&TokenKind::Colon) {
                        self.parse_destructure_pattern()?
                    } else {
                        match &key {
                            PropertyKey::Ident(s) => {
                                self.check_binding_name(s)?;
                                Pattern::Ident(s.clone())
                            }
                            PropertyKey::String(s) => {
                                self.check_binding_name(s)?;
                                Pattern::Ident(s.clone())
                            }
                            _ => {
                                return Err(error::Error::syntax(
                                    "Numeric/computed destructuring key requires a binding"
                                        .to_string(),
                                ))
                            }
                        }
                    };
                    // default value: `key = default`
                    let target = if self.eat(&TokenKind::Assign) {
                        let d = self.parse_assign()?;
                        Pattern::Assign(Box::new(target), d)
                    } else {
                        target
                    };
                    props.push((key, target));
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace, "}")?;
                Ok(Pattern::Object(props, rest))
            }
            other => self
                .parse_binding_identifier(&format!("Expected pattern, got {other:?}"))
                .map(Pattern::Ident),
        }
    }
}

/// Check if a statement is a labelled function declaration (possibly nested
/// through multiple labels).
fn is_labelled_function(node: &StmtNode) -> bool {
    match node {
        StmtNode::Labeled(_, body) => is_labelled_function(&body.node),
        StmtNode::FunctionDecl(_) => true,
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum StatementListScope {
    Script,
    Block,
    FunctionBody,
    Module,
}

fn collect_decl_names(
    node: &StmtNode,
    lexical: &mut Vec<Arc<str>>,
    var: &mut Vec<Arc<str>>,
    scope: StatementListScope,
) {
    match node {
        StmtNode::VarDecl { kind, decls } => match kind {
            VarKind::Var => {
                for (name, _) in decls {
                    var.push(name.clone());
                }
            }
            VarKind::Let | VarKind::Const => {
                for (name, _) in decls {
                    lexical.push(name.clone());
                }
            }
        },
        StmtNode::Destructure { kind, pattern, .. } => match kind {
            VarKind::Var => collect_pattern_names(pattern, var),
            VarKind::Let | VarKind::Const => collect_pattern_names(pattern, lexical),
        },
        StmtNode::FunctionDecl(f) => {
            if let Some(name) = &f.name {
                if matches!(
                    scope,
                    StatementListScope::Block | StatementListScope::Module
                ) {
                    lexical.push(name.clone());
                } else {
                    var.push(name.clone());
                }
            }
        }
        StmtNode::ExprStmt(Expr::Class(c)) => {
            if c.is_declaration {
                if let Some(name) = &c.name {
                    lexical.push(name.clone());
                }
            }
        }
        _ => Parser::collect_var_names_in_stmt(node, var),
    }
}

fn collect_switch_decl_names(
    node: &StmtNode,
    lexical: &mut Vec<Arc<str>>,
    var: &mut Vec<Arc<str>>,
) {
    match node {
        StmtNode::VarDecl { kind, decls } => match kind {
            VarKind::Var => {
                for (name, _) in decls {
                    var.push(name.clone());
                }
            }
            VarKind::Let | VarKind::Const => {
                for (name, _) in decls {
                    lexical.push(name.clone());
                }
            }
        },
        StmtNode::Destructure { kind, pattern, .. } => match kind {
            VarKind::Var => collect_pattern_names(pattern, var),
            VarKind::Let | VarKind::Const => collect_pattern_names(pattern, lexical),
        },
        StmtNode::FunctionDecl(f) => {
            if let Some(name) = &f.name {
                lexical.push(name.clone());
            }
        }
        StmtNode::ExprStmt(Expr::Class(c)) => {
            if c.is_declaration {
                if let Some(name) = &c.name {
                    lexical.push(name.clone());
                }
            }
        }
        _ => {}
    }
}

fn check_statement_list_declaration_early_errors(
    body: &[Stmt],
    scope: StatementListScope,
) -> error::Result<()> {
    let mut lexical_names = Vec::new();
    let mut var_names = Vec::new();
    for stmt in body {
        collect_decl_names(&stmt.node, &mut lexical_names, &mut var_names, scope);
    }
    for name in &lexical_names {
        if lexical_names.iter().filter(|n| *n == name).count() > 1 || var_names.contains(name) {
            return Err(error::Error::syntax(format!(
                "Identifier '{}' has already been declared",
                name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        Parser::parse(src).unwrap()
    }

    #[test]
    fn parse_number_expr() {
        let p = parse("42;");
        assert_eq!(p.body.len(), 1);
        match &p.body[0].node {
            StmtNode::ExprStmt(Expr::Number(n)) => assert_eq!(*n, 42.0),
            other => panic!("expected number expr, got {:?}", other),
        }
    }

    #[test]
    fn parse_var_decl() {
        let p = parse("let x = 1 + 2;");
        assert_eq!(p.body.len(), 1);
        match &p.body[0].node {
            StmtNode::VarDecl { kind, decls } => {
                assert_eq!(*kind, VarKind::Let);
                assert_eq!(decls.len(), 1);
                assert_eq!(decls[0].0.as_ref(), "x");
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_module_import_export_metadata() {
        let p = Parser::parse_module(
            r#"
                import "side-effect";
                import { x, x as y, } from "dep";
                export var a = 1;
                export let b = 2;
                export const c = 3;
                export function f() {}
                export class C {}
                export { a as renamed, y, };
                export { remote as reexported, } from "other";
                export * from "all";
            "#,
        )
        .unwrap();

        assert_eq!(p.body.len(), 5);
        assert!(matches!(p.body[0].node, StmtNode::VarDecl { .. }));
        assert!(matches!(p.body[3].node, StmtNode::FunctionDecl(_)));
        assert!(matches!(p.body[4].node, StmtNode::ExprStmt(Expr::Class(_))));
        assert_eq!(
            p.module_requests,
            vec![
                ModuleRequest {
                    specifier: Arc::from("side-effect"),
                    attributes: Vec::new(),
                },
                ModuleRequest {
                    specifier: Arc::from("dep"),
                    attributes: Vec::new(),
                },
                ModuleRequest {
                    specifier: Arc::from("other"),
                    attributes: Vec::new(),
                },
                ModuleRequest {
                    specifier: Arc::from("all"),
                    attributes: Vec::new(),
                },
            ]
        );
        assert_eq!(
            p.import_entries,
            vec![
                ImportEntry {
                    module_request: ModuleRequest {
                        specifier: Arc::from("dep"),
                        attributes: Vec::new(),
                    },
                    import_name: Arc::from("x"),
                    local_name: Arc::from("x"),
                },
                ImportEntry {
                    module_request: ModuleRequest {
                        specifier: Arc::from("dep"),
                        attributes: Vec::new(),
                    },
                    import_name: Arc::from("x"),
                    local_name: Arc::from("y"),
                },
            ]
        );
        assert_eq!(
            p.export_entries,
            vec![
                ExportEntry::Local {
                    local_name: Arc::from("a"),
                    export_name: Arc::from("a"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("b"),
                    export_name: Arc::from("b"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("c"),
                    export_name: Arc::from("c"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("f"),
                    export_name: Arc::from("f"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("C"),
                    export_name: Arc::from("C"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("a"),
                    export_name: Arc::from("renamed"),
                },
                ExportEntry::ReExport {
                    module_request: ModuleRequest {
                        specifier: Arc::from("dep"),
                        attributes: Vec::new(),
                    },
                    import_name: Arc::from("x"),
                    export_name: Arc::from("y"),
                },
                ExportEntry::ReExport {
                    module_request: ModuleRequest {
                        specifier: Arc::from("other"),
                        attributes: Vec::new(),
                    },
                    import_name: Arc::from("remote"),
                    export_name: Arc::from("reexported"),
                },
                ExportEntry::Star {
                    module_request: ModuleRequest {
                        specifier: Arc::from("all"),
                        attributes: Vec::new(),
                    },
                },
            ]
        );
    }

    #[test]
    fn parse_dynamic_import_as_a_dedicated_call_expression() {
        let script = Parser::parse("import('./dep.js');").expect("script import call should parse");
        assert!(matches!(
            &script.body[0].node,
            StmtNode::ExprStmt(Expr::ImportCall { options: None, .. })
        ));
        let module = Parser::parse_module("import('./dep.js', {});")
            .expect("module import call should parse");
        assert!(matches!(
            &module.body[0].node,
            StmtNode::ExprStmt(Expr::ImportCall {
                options: Some(_),
                ..
            })
        ));
        assert!(Parser::parse("import();").is_err());
        assert!(Parser::parse("import('./dep.js', {}, 1);").is_err());
        assert!(Parser::parse("import('./dep.js',);").is_ok());
        assert!(Parser::parse("import('./dep.js', {},);").is_ok());
        assert!(Parser::parse("new import('./dep.js');").is_err());
        assert!(Parser::parse("new import('./dep.js').value;").is_err());
        assert!(Parser::parse("new (import('./dep.js'));").is_ok());
    }

    #[test]
    fn parse_static_import_attributes_on_imports_and_reexports() {
        let program = Parser::parse_module(
            r#"
            import value from './data' with { type: 'json', mode: "strict", };
            import './data' with { mode: 'strict', "type": "json" };
            export { default as forwarded } from './data'
              with { mode: 'strict', type: 'json' };
            export * from './text' with { type: 'text' };
            export * as text from './text' with { type: 'text' };
            "#,
        )
        .expect("static import attributes should parse on every request form");

        assert_eq!(program.module_requests.len(), 2);
        assert_eq!(program.module_requests[0].specifier.as_ref(), "./data");
        assert_eq!(
            program.module_requests[0]
                .attributes
                .iter()
                .map(|attribute| (attribute.key.as_ref(), attribute.value.as_ref()))
                .collect::<Vec<_>>(),
            vec![("mode", "strict"), ("type", "json")]
        );
        assert_eq!(program.module_requests[1].specifier.as_ref(), "./text");
        assert_eq!(
            program.module_requests[1].attributes[0].key.as_ref(),
            "type"
        );
        assert!(program
            .import_entries
            .iter()
            .all(|entry| { entry.module_request == program.module_requests[0] }));
        assert!(program
            .export_entries
            .iter()
            .filter_map(|entry| match entry {
                ExportEntry::ReExport { module_request, .. } => Some(module_request),
                _ => None,
            })
            .all(|request| request == &program.module_requests[0]));
        assert!(program
            .export_entries
            .iter()
            .filter_map(|entry| match entry {
                ExportEntry::Star { module_request }
                | ExportEntry::NamespaceReExport { module_request, .. } => Some(module_request),
                _ => None,
            })
            .all(|request| request == &program.module_requests[1]));
    }

    #[test]
    fn static_import_attributes_reject_duplicate_decoded_keys_and_non_strings() {
        for source in [
            r#"import './data' with { type: 'json', "typ\u0065": 'json' };"#,
            r#"import './data' with { '\164ype': 'json' };"#,
            r#"import './data' with { type: '\152son' };"#,
            r#"import './data' with { '\8': 'json' };"#,
            r#"import './data' with { type: '\9' };"#,
            r#"import './\144ata' with { type: 'json' };"#,
            "import './data' with { type: 1 };",
            "import './data' with { type: `json` };",
            "import './data' with { type: 'json' type: 'json' };",
            "import './data' with { type: 'json',, };",
        ] {
            assert!(Parser::parse_module(source).is_err(), "{source}");
        }
        assert!(Parser::parse_module("import './data'\nwith {\n type: 'json',\n };").is_ok());
    }

    #[test]
    fn parse_module_default_import_export_metadata() {
        let program = Parser::parse_module(
            r#"
            import primary, { value as alias } from './dep.js';
            export default function() {}
            export { default as forwarded } from './other.js';
            "#,
        )
        .expect("default module forms should parse");
        assert_eq!(program.import_entries.len(), 2);
        assert_eq!(program.import_entries[0].import_name.as_ref(), "default");
        assert_eq!(program.import_entries[0].local_name.as_ref(), "primary");
        assert!(program.export_entries.iter().any(|entry| matches!(
            entry,
            ExportEntry::Local {
                local_name,
                export_name,
            } if local_name.as_ref() == "default" && export_name.as_ref() == "default"
        )));
        assert!(program.export_entries.iter().any(|entry| matches!(
            entry,
            ExportEntry::ReExport {
                import_name,
                export_name,
                ..
            } if import_name.as_ref() == "default" && export_name.as_ref() == "forwarded"
        )));

        assert!(Parser::parse_module("export default 1; export default 2;").is_err());
        assert!(Parser::parse_module("export default 1; export { default };").is_err());
        let namespace = Parser::parse_module(
            "import value, * as ns from './dep.js'; export * as nested from './dep.js';",
        )
        .expect("namespace module forms should parse");
        assert!(namespace.import_entries.iter().any(|entry| {
            entry.import_name.as_ref() == "*" && entry.local_name.as_ref() == "ns"
        }));
        assert!(namespace.export_entries.iter().any(|entry| matches!(
            entry,
            ExportEntry::NamespaceReExport { export_name, .. }
                if export_name.as_ref() == "nested"
        )));

        let imported_reexports = Parser::parse_module(
            "import * as ns from './dep.js'; import { value as local } from './other.js'; export { ns, local as renamed };",
        )
        .expect("imported bindings should be normalized as indirect exports");
        assert!(imported_reexports
            .export_entries
            .iter()
            .any(|entry| matches!(
                entry,
                ExportEntry::NamespaceReExport {
                    module_request,
                    export_name,
                } if module_request.specifier.as_ref() == "./dep.js" && export_name.as_ref() == "ns"
            )));
        assert!(imported_reexports
            .export_entries
            .iter()
            .any(|entry| matches!(
                entry,
                ExportEntry::ReExport {
                    module_request,
                    import_name,
                    export_name,
                } if module_request.specifier.as_ref() == "./other.js"
                    && import_name.as_ref() == "value"
                    && export_name.as_ref() == "renamed"
            )));
        assert!(Parser::parse_module("export default function await() {}").is_err());
        assert!(Parser::parse_module("export default function* await() {}").is_err());
    }

    #[test]
    fn parse_module_import_export_early_errors_and_escapes() {
        for src in [
            "import { x } from 'dep'; let x;",
            "import { x as y, z as y } from 'dep';",
            "export { missing };",
            "var x; export { x }; export { x as x };",
            "import { x \\u0061s y } from 'dep';",
            "import { x } \\u0066rom 'dep';",
            "export { x \\u0061s y };",
            "\\u0069mport 'dep';",
            "\\u0065xport var x;",
        ] {
            assert!(Parser::parse_module(src).is_err(), "{src}");
        }

        let p =
            Parser::parse_module("import { x as \\u0061s, } from 'dep'; export { as, };").unwrap();
        assert_eq!(p.import_entries[0].local_name.as_ref(), "as");
        assert_eq!(
            p.export_entries[0],
            ExportEntry::ReExport {
                module_request: ModuleRequest {
                    specifier: Arc::from("dep"),
                    attributes: Vec::new(),
                },
                import_name: Arc::from("x"),
                export_name: Arc::from("as"),
            }
        );

        assert!(Parser::parse_module(
            "import { a as async, b as undefined, c as of } from 'dep'; export { async, undefined, of };"
        )
        .is_ok());

        assert!(Parser::parse_module(
            "export { x as publicX }; import { original as x } from 'dep';"
        )
        .is_ok());

        let string_names = Parser::parse_module(
            r#"import { "remote name" as local } from "dep";
               export { local as "public name" };
               export { "remote name" as "forwarded name" } from "dep";
               export * as "namespace name" from "dep";"#,
        )
        .expect("string module export names should parse");
        assert_eq!(
            string_names.import_entries[0].import_name.as_ref(),
            "remote name"
        );
        assert!(string_names.export_entries.iter().any(|entry| matches!(
            entry,
            ExportEntry::ReExport {
                import_name,
                export_name,
                ..
            } if import_name.as_ref() == "remote name" && export_name.as_ref() == "public name"
        )));
        assert!(string_names.export_entries.iter().any(|entry| matches!(
            entry,
            ExportEntry::NamespaceReExport { export_name, .. }
                if export_name.as_ref() == "namespace name"
        )));
        assert!(Parser::parse_module(r#"import { "remote" } from "dep";"#).is_err());
        assert!(Parser::parse_module(r#"export { "local" };"#).is_err());
        for source in [
            r#"import { "\1" as local } from "dep";"#,
            r#"export { "\8" } from "dep";"#,
            r#"export * as "\1" from "dep";"#,
        ] {
            assert!(Parser::parse_module(source).is_err(), "{source}");
        }
    }

    #[test]
    fn parse_module_export_declaration_variants() {
        let p = Parser::parse_module(
            "export const { x, y: z } = value; export async function load() {}",
        )
        .unwrap();

        assert_eq!(p.body.len(), 2);
        assert!(matches!(p.body[0].node, StmtNode::Destructure { .. }));
        assert!(matches!(p.body[1].node, StmtNode::FunctionDecl(_)));
        assert_eq!(
            p.export_entries,
            vec![
                ExportEntry::Local {
                    local_name: Arc::from("x"),
                    export_name: Arc::from("x"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("z"),
                    export_name: Arc::from("z"),
                },
                ExportEntry::Local {
                    local_name: Arc::from("load"),
                    export_name: Arc::from("load"),
                },
            ]
        );
    }

    #[test]
    fn import_export_are_module_top_level_only() {
        for src in ["import 'dep';", "export var x;"] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
        for src in ["{ import 'dep'; }", "if (true) export var x;"] {
            assert!(Parser::parse_module(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_let_declaration_across_newline() {
        for src in [
            "let\nlet;",
            "let\nlet = 1;",
            "function f() { let\nawait 0; }",
            "function f() { let\nyield 0; }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("function f() { let await; }").is_ok());
        assert!(Parser::parse("l\\u0065t\na;\nvar a;").is_ok());
    }

    #[test]
    fn parse_statement_list_declaration_early_errors() {
        for src in [
            "class A {} class A {}",
            "{ class A {} class A {} }",
            "{ let A; class A {} }",
            "{ class A {} var A; }",
            "{ function f() {} var f; }",
            "{ var f; function f() {} }",
            "{ function f() {} { var f; } }",
            "{ { var f; } let f; }",
            "class A {} var A;",
            "class C { st\\u0061tic m() {} }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("{ class A {} { class A {} } }").is_ok());
        assert!(Parser::parse("function f() {} var f;").is_ok());
        assert!(Parser::parse("\"use strict\"; function f() {} var f;").is_ok());
        assert!(Parser::parse("\"use strict\"; var g; function g() {}").is_ok());
        assert!(Parser::parse("\"use strict\"; function f() {} function f() {}").is_ok());
        assert!(Parser::parse("\"use strict\"; function* g() {} var g;").is_ok());
        assert!(Parser::parse("function outer() { var f; function f() {} }").is_ok());
        assert!(Parser::parse("function outer() { let f; function f() {} }").is_err());
        assert!(Parser::parse("function outer() { function f() {} const f = 1; }").is_err());
        assert!(Parser::parse("(() => { class F {} function F() {} })").is_err());
        assert!(Parser::parse_module("function f() {} var f;").is_err());
        assert!(Parser::parse_module("var g; function g() {}").is_err());
        assert!(Parser::parse_module("function f() {} function f() {}").is_err());
        assert!(Parser::parse("class C { st\\u0061tic() {} }").is_ok());
    }

    #[test]
    fn parse_residual_label_and_module_class_early_errors() {
        for src in [
            "label: function* g() {}",
            "label: async function f() {}",
            "label: async function* g() {}",
            "outer: inner: async function f() {}",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        // Annex B keeps only ordinary sloppy labelled function declarations.
        assert!(Parser::parse("label: function f() {}").is_ok());
        assert!(Parser::parse("label: async\nfunction f() {}").is_ok());

        for src in [
            "class aw\\u0061it {}",
            "var C = class aw\\u0061it {};",
            "function f() { class aw\\u0061it {} }",
            "function f() { var C = class aw\\u0061it {}; }",
            "function f() { class await {} }",
            "function f() { var C = class await {}; }",
        ] {
            assert!(Parser::parse_module(src).is_err(), "{src}");
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_for_in_of_declaration_head_early_errors() {
        for src in [
            "for (let x, y in {}) {}",
            "for (const x, y in {}) {}",
            "for (let x, y of []) {}",
            "for (var x, y in {}) {}",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("for (let x in {}) {}").is_ok());
        assert!(Parser::parse("for (let x of []) {}").is_ok());
    }

    #[test]
    fn parse_parenthesized_patterns_are_not_assignment_targets() {
        for src in ["({}) = 1;", "() => ({}) = 1;", "async () => ({}) = 1;"] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("({} = {});").is_ok());
    }

    #[test]
    fn parse_import_meta_is_not_assignment_target() {
        for src in ["import.meta = 1;", "(import.meta) = 1;"] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_import_meta_is_a_module_only_expression() {
        assert!(Parser::parse_module("import.meta; import(import.meta);").is_ok());
        for source in ["import.meta;", "import.\\u006deta;", "import.other;"] {
            assert!(Parser::parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn parse_optional_chain_is_not_assignment_target() {
        for src in [
            "a?.b = 1;",
            "a?.b.c = 1;",
            "(a?.b.c) = 1;",
            "++a?.b;",
            "++a?.b.c;",
            "a?.b++;",
            "a?.b.c++;",
            "0, [a?.b = 1] = [2];",
            "0, [a?.b.c = 1] = [2];",
            "0, { x: a?.b = 1 } = { x: 2 };",
            "0, { x: a?.b.c = 1 } = { x: 2 };",
            "for (a?.b.c in {}) {}",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("(a?.b).c = 1;").is_ok());
        assert!(Parser::parse("for ((a?.b).c in {}) {}").is_ok());
    }

    #[test]
    fn parse_optional_chain_rejects_tagged_template_tail() {
        for src in [
            "a?.b`template`;",
            "a?.b\n`template`;",
            "a?.[b]`template`;",
            "a?.()`template`;",
            "a?.b.c`template`;",
            "a?.`template`;",
            "class C { #tag() {} m() { this?.#tag`template`; } }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "(a?.b)`template`;",
            "(a?.[b])`template`;",
            "(a?.())`template`;",
            "(a?.b.c)`template`;",
            "class C { #tag() {} m() { (this?.#tag)`template`; } }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_duplicate_proto_object_assignment_pattern() {
        assert!(Parser::parse("result = { __proto__: x, __proto__: y } = value;").is_ok());
        assert!(Parser::parse("({ __proto__: x, __proto__: y } = value);").is_ok());
        assert!(Parser::parse("({ a: { __proto__: x, __proto__: y } } = value);").is_ok());

        for src in [
            "({ __proto__: null, '__proto__': null });",
            "var obj = { a: { __proto__: null, __proto__: null } };",
            "({ x = { __proto__: null, __proto__: null } } = value);",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_computed_property_names_allow_in_inside_for_heads() {
        assert!(Parser::parse(
            r#"for (obj = { get ["x" in empty]() { return 1; } }; ; ) { break; }"#,
        )
        .is_ok());
        assert!(Parser::parse(r#"for (value = obj["x" in empty]; ; ) { break; }"#,).is_ok());
        assert!(
            Parser::parse(r#"for (({ ["x" in empty]: value } = obj); ; ) { break; }"#,).is_ok()
        );
    }

    #[test]
    fn parse_object_literal_strict_early_errors() {
        for src in [
            r#"function f() { "use strict"; ({ let }); }"#,
            r#"function f() { "use strict"; ({ yield }); }"#,
            r#"({ this });"#,
            r#"void { set x(eval) { "use strict"; } };"#,
            r#"void { set x(arguments) { "use strict"; } };"#,
            r#"void { m(eval) { "use strict"; } };"#,
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            r#"var let = 1; ({ let });"#,
            r#"var yield = 1; ({ yield });"#,
            r#"void { set x(eval) {} };"#,
            r#"void { m(arguments) {} };"#,
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_throw_requires_same_line_operand() {
        assert!(Parser::parse("throw 1;").is_ok());
        assert!(Parser::parse("throw\n1;").is_err());
    }

    #[test]
    fn parse_unterminated_string_literals_error() {
        assert!(Parser::parse("var str = ';").is_err());
        assert!(Parser::parse("var str = \";").is_err());
    }

    #[test]
    fn parse_undefined_as_var_binding_name() {
        assert!(Parser::parse("var undefined;").is_ok());
        assert!(Parser::parse("var undefined = 1;").is_ok());
        assert!(Parser::parse("var undef\\u0069ned;").is_ok());
    }

    #[test]
    fn parse_escaped_reserved_words_are_not_identifiers() {
        for src in [
            "f\\u{61}lse;",
            "tru\\u{65};",
            "n\\u{75}ll;",
            "v\\u0061r x;",
            "function f(f\\u{61}lse) {}",
            "var f\\u{61}lse = 1;",
            "({ f\\u{61}lse });",
            "f\\u{61}lse: ;",
            r#""use strict"; yi\u0065ld: ;"#,
            "class l\\u0065t {}",
            "class st\\u0061tic {}",
            "class tru\\u0065 {}",
            "var { bre\\u0061k } = {};",
            "({ bre\\u0061k } = {});",
            "var f = ({ bre\\u0061k }) => {};",
            "function f({ bre\\u0061k }) {}",
            "({ \\u0065num });",
            "({ \\u0065num } = {});",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "var obj = {}; obj.st\\u0061tic = 1;",
            "({ st\\u0061tic: 1 });",
            "({ f\\u{61}lse: 1 });",
            "var { bre\\u0061k: x } = {};",
            "({ bre\\u0061k: x } = {});",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_import_export_binding_names_are_reserved() {
        for src in [
            "var import = 1;",
            "var export = 1;",
            "let import;",
            "const export = 1;",
            "function f(import) {}",
            "function export() {}",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_strict_only_future_reserved_bindings() {
        for src in [
            "var implements = 1;",
            "var interface = 1;",
            "var package = 1;",
            "var private = 1;",
            "var protected = 1;",
            "var public = 1;",
            "var static = 1;",
            "var st\\u0061tic = 1;",
            "{ let implements = 1; }",
            "{ let static = 1; }",
            "{ let st\\u0061tic = 1; }",
            "{ const package = 1; }",
            "{ const static = 1; }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        for src in [
            "\"use strict\"; var implements = 1;",
            "\"use strict\"; var interface = 1;",
            "\"use strict\"; var package = 1;",
            "\"use strict\"; var private = 1;",
            "\"use strict\"; var protected = 1;",
            "\"use strict\"; var public = 1;",
            "\"use strict\"; var static = 1;",
            "\"use strict\"; var st\\u0061tic = 1;",
            "\"use strict\"; var yield = 1;",
            "var enum = 1;",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_contextual_formal_binding_identifiers_consistently() {
        for src in [
            "function f(undefined, async, of, static, let, get, set, ...rest) {}",
            "function f([undefined], {x: static}) {}",
            "var f = (undefined, async, of, static, let, get, set, ...rest) => undefined;",
            "undefined => undefined;",
            "async => async;",
            "of => of;",
            "static => static;",
            "let => let;",
            "get => get;",
            "set => set;",
            "async undefined => undefined;",
            "async static => static;",
            "async yield => yield;",
            "class C { static { (() => await); } }",
            "class C { static { (() => { function f() { return arguments; } }); } }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        for src in [
            "function f(public) { 'use strict'; }",
            "function f(package) { 'use strict'; }",
            "function f(yield) { 'use strict'; }",
            "var f = function(static) { 'use strict'; };",
            "var f = public => { 'use strict'; };",
            "var o = { m(package) { 'use strict'; } };",
            "class C { static { (await => 0); } }",
            "class C { static { (arguments => 0); } }",
            "class C { static { (([await]) => 0); } }",
            "class C { static { (({x: arguments}) => 0); } }",
            "class C { static { (() => arguments); } }",
            "class C { static { (() => { return arguments; }); } }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_let_binding_name_obeys_binding_context() {
        for src in [
            "var [let] = [];",
            "var {x: let} = {};",
            "function f([let]) {} function g({x: let}) {}",
            "([let]) => let; ({x: let}) => let;",
            "try {} catch (let) {}",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        for src in [
            "let [let] = [];",
            "const {x: let} = {};",
            "for (let [let] of []);",
            "try {} catch ([let]) {}",
            "try {} catch ({x: let}) {}",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_escaped_use_strict_is_not_strict_directive() {
        for src in [
            "'use\\u0020strict'; var public = 1;",
            concat!("'use\\", "\n", " strict'; var public = 1;"),
            "function f() { 'use\\u0020strict'; var public = 1; }",
            concat!("function f() { 'use\\", "\n", " strict'; var public = 1; }"),
            "var f = function() { 'use\\u0020strict'; var public = 1; };",
            "var f = () => { 'use\\u0020strict'; var public = 1; };",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        assert!(Parser::parse("'use strict'; var public = 1;").is_err());
        assert!(Parser::parse("'use strict'; public = 1;").is_err());
        assert!(Parser::parse("function f() { 'use strict'; var public = 1; }").is_err());
        assert!(Parser::parse("function f() { 'use strict'; public = 1; }").is_err());
        assert!(Parser::parse("var f = () => { 'use strict'; var public = 1; };").is_err());
    }

    #[test]
    fn parse_function() {
        let p = parse("function add(a, b) { return a + b; }");
        match &p.body[0].node {
            StmtNode::FunctionDecl(f) => {
                assert_eq!(f.name.as_ref().map(|s| s.as_ref()), Some("add"));
                assert_eq!(f.params.len(), 2);
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_yield_at_assignment_expression_precedence() {
        for src in [
            "function* g() { yield 3 + yield 4; }",
            "class C { *g() { yield 3 + yield 4; } }",
            "function* g() { yield\n* 1; }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "function* g() { (yield) + (yield 4); }",
            "function* g() { (yield) ? yield : yield; }",
            "function* g() { `a${yield}b`; }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_generator_parameter_early_errors() {
        for src in [
            "function* f(x = 0, x) {}",
            "var f = function*(x = 0, x) {};",
            "function f(x = 0, x) {}",
            "function* f(x = yield) {}",
            "var f = function*(x = yield) {};",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "function f(x, x) {}",
            "function* f(x = function*() { yield 1; }) {}",
            "function* f(x = () => yield) {}",
            "async function* f(x = () => yield) {}",
            "function* f() { (x = yield 1); }",
            "async function f() { (x = await 1); }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_async_and_arrow_parameter_expression_early_errors() {
        for src in [
            "async function f(a = await 1) {}",
            "var f = async function(a = await 1) {};",
            "var f = async (a = await 1) => {};",
            "var o = { async m(a = await 1) {} };",
            "class C { async m(a = await 1) {} }",
            "async function* g(a = await 1) {}",
            "function* g() { let f = (x = yield 1) => x; }",
            "async function f() { let g = (x = await 1) => x; }",
            "function* g() { let f = async (x = yield 1) => x; }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "async function f(a = () => await) {}",
            "async function f(a = async () => await 1) {}",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_arrow_in_obj() {
        let p = parse("let o = { x: 1, y: 2 };");
        match &p.body[0].node {
            StmtNode::VarDecl { decls, .. } => match &decls[0].1 {
                Some(Expr::Object(props)) => assert_eq!(props.len(), 2),
                other => panic!("{:?}", other),
            },
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_arrow_formal_parameter_early_errors() {
        for src in [
            r#""use strict"; var af = eval => 1;"#,
            r#"var af = eval => { "use strict"; };"#,
            r#""use strict"; var af = arguments => 1;"#,
            r#"var af = arguments => { "use strict"; };"#,
            "var af = (x, [x]) => 1;",
            "var af = ([x, x]) => 1;",
            "var af = (x, {x}) => 1;",
            "var af = (x, {y: x}) => 1;",
            "var af = ({x}, {y: x}) => 1;",
            "var af = ({y: x, x}) => 1;",
            "var af = x\n=> x;",
            "var af = x\n=> {};",
            "var af = ()\n=> {};",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_arrow_sloppy_eval_arguments_yield_params() {
        for src in [
            "var af = eval => eval;",
            "var af = arguments => arguments;",
            "var af = yield => 1;",
            "var af = (eval) => eval;",
            "var af = (arguments) => arguments;",
            "var af = (yield) => 1;",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_strict_yield_outside_generator_is_rejected() {
        for src in [
            r#""use strict"; (yield);"#,
            r#""use strict"; '' in (yield);"#,
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in ["var yield = 1; yield;", "'' in (yield);"] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_method_formal_parameter_early_errors() {
        for src in [
            "({ foo(a, a) {} });",
            "({ async foo(a, a) {} });",
            "({ foo([a, a]) {} });",
            "class C { foo(a, a) {} }",
            "class C { #foo(a, a) {} }",
            "({ async\nfoo() {} });",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_object_async_method_prefix_rejects_escaped_keyword() {
        for src in [
            r#"({ \u0061sync method() {} });"#,
            r#"({ \u0061sync *method() {} });"#,
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            r#"({ \u0061sync() {} });"#,
            r#"({ \u0061sync: 1 });"#,
            r#"var async = 1; ({ \u0061sync });"#,
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_yield_identifier_contexts() {
        for src in [
            "var yield = 'prop'; var obj = { method(yield) { return yield; } };",
            "var yield = 'prop'; var obj = { method(x = yield) { return x; } };",
            "var yield = 'prop'; var obj = { [yield]() {} };",
            "var obj = { *g() { function h() { yield = 1; } } };",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        for src in [
            "function* g(yield) {}",
            "var obj = { *g(yield) {} };",
            "function* g() { void yield; }",
            "class C { *#g() { void yield; } }",
            "class C { static async *#g() { void yield; } }",
            r#""use strict"; var yield = 1;"#,
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_await_identifier_contexts() {
        for src in [
            "var await = 0; await = 1; await;",
            "function foo(await) { return await; }",
            "async function await() { return 1; }",
            "var await; async function foo() { function bar() { await = 1; } bar(); }",
            "var await = 'prop'; var obj = { method(await) { return await; } };",
            "var await = 'prop'; var obj = { [await]() {} };",
            "async function await() { return 1; } await instanceof Function;",
            "(await) => await;",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }

        for src in [
            "async function f(await) {}",
            "async function f() { var await = 1; }",
            "async function f() { await = 1; }",
            "async function f() { var \\u0061wait = 1; }",
            "async function f() { void \\u0061wait; }",
            "async function f() { \\u0061wait: ; }",
            "class C { async #m() { var \\u0061wait; } }",
            "class C { static async #m() { void \\u0061wait; } }",
            "async (await) => await;",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        for src in [
            "async function f() { function g() { var await = 1; return await; } }",
            "class C { async #m() { return this.\\u0061wait; } }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_precedence() {
        // 1 + 2 * 3 should be Add(1, Mul(2,3))
        let p = parse("1 + 2 * 3;");
        match &p.body[0].node {
            StmtNode::ExprStmt(Expr::Binary(BinOp::Add, _, right)) => match right.as_ref() {
                Expr::Binary(BinOp::Mul, _, _) => {}
                other => panic!("expected mul on right, got {:?}", other),
            },
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_inc_dec() {
        let p = parse("++x; y--;");
        match &p.body[0].node {
            StmtNode::ExprStmt(Expr::Update(UpdateOp::Inc, true, _)) => {}
            other => panic!("{:?}", other),
        }
        match &p.body[1].node {
            StmtNode::ExprStmt(Expr::Update(UpdateOp::Dec, false, _)) => {}
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_for_loop() {
        let p = parse("for (let i = 0; i < 10; i++) { sum += i; }");
        assert!(matches!(&p.body[0].node, StmtNode::For { .. }));
    }

    #[test]
    fn parse_for_of_head_early_errors() {
        for src in [
            "for (var x o\\u0066 []) ;",
            "for (const let of []) {}",
            "for ([(x, y)] of []) {}",
            "for ({ m() {} } of []) {}",
            "for ([(x, y)] in {}) {}",
            "for ({ m() {} } in {}) {}",
            "for ([...x,] in {}) {}",
            "for ([...x,,] in {}) {}",
            "for ([...x,] of []) {}",
            "for ([...x,,] of []) {}",
            "var async; for (async of [1]) ;",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }

        assert!(Parser::parse("for ([...x] in {}) {}").is_ok());
        assert!(Parser::parse("for ([...x] of []) {}").is_ok());
        assert!(Parser::parse("for ({ x = 1 } of [{}]) {}").is_ok());
        assert!(Parser::parse("for ({ fn = function() {} } of [{}]) {}").is_ok());
        assert!(Parser::parse("for ([x = 'x' in {}] of [[]]) {}").is_ok());
        assert!(Parser::parse("for ({ x = 'x' in {} } of [{}]) {}").is_ok());
        assert!(Parser::parse("for ({ p: x = 'x' in {} } of [{}]) {}").is_ok());
        assert!(Parser::parse("for (of; ; ) {}").is_ok());
        assert!(Parser::parse("for (of in {}) {}").is_ok());
        assert!(Parser::parse("for ({ x = 1 }; ; ) {}").is_err());
        assert!(Parser::parse("for ({ __proto__: null, __proto__: null }; ; ) {}").is_err());
    }

    #[test]
    fn parse_for_of_async_lhs_contextual_identifier() {
        for src in [
            "var async = { x: 0 }; for (async.x of [1]) ;",
            "let async; for ((async) of [7]) ;",
            "let async; for (\\u0061sync of [7]) ;",
            "async function f() { for await (async of [7]) ; }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_method_parameter_defaults_allow_super_property() {
        for src in [
            "var obj = { method(x = super.toString) { return x; } };",
            "var obj = { method(x = super['toString']) { return x; } };",
            "var obj = { *method(x = super.toString) { yield x; } };",
            "var obj = { async method(x = super.toString) { return x; } };",
            "class C { method(x = super.toString) {} }",
            "class C { static method(x = super.toString) {} }",
            "class C extends B { constructor(x = super.toString) { super(); } }",
            "var obj = { method(x = () => super.toString) { return x; } };",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }

    #[test]
    fn parse_method_parameter_defaults_reject_super_call_and_non_method_super() {
        for src in [
            "var obj = { method(x = super()) {} };",
            "class C { method(x = super()) {} }",
            "class C extends B { constructor(x = super()) { super(); } }",
            "function f(x = super.toString) {}",
            "var f = function(x = super.toString) {};",
            "var obj = { method(x = function(y = super.toString) {}) {} };",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_try_catch() {
        let p = parse("try { f(); } catch (e) { g(); } finally { h(); }");
        assert!(matches!(&p.body[0].node, StmtNode::TryCatch { .. }));
    }

    #[test]
    fn parse_catch_parameter_early_errors() {
        for src in [
            "try {} catch ([x, x]) {}",
            "try {} catch (x) { let x; }",
            "try {} catch (x) { const x = 1; }",
            "try {} catch (x) { class x {} }",
            "function f() { try {} catch (e) { function e(){} } }",
            "function f() { try {} catch (e) { label: function e(){} } }",
        ] {
            assert!(Parser::parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn parse_catch_parameter_allows_non_conflicting_scopes() {
        for src in [
            "try {} catch (x) { var x; }",
            "try {} catch (x) { { let x; } }",
            "try {} catch (x) { function y(){} }",
        ] {
            assert!(Parser::parse(src).is_ok(), "{src}");
        }
    }
}

/// Extract all binding names from a destructuring pattern.
fn collect_pattern_names(pattern: &Pattern, names: &mut Vec<Arc<str>>) {
    match pattern {
        Pattern::Ident(name) => names.push(name.clone()),
        Pattern::Hole => {}
        Pattern::Array(elements) => {
            for p in elements {
                collect_pattern_names(p, names);
            }
        }
        Pattern::Object(props, rest) => {
            for (_, p) in props {
                collect_pattern_names(p, names);
            }
            if let Some(r) = rest {
                collect_pattern_names(r, names);
            }
        }
        Pattern::Assign(p, _) => collect_pattern_names(p, names),
        Pattern::Rest(p) => collect_pattern_names(p, names),
    }
}

fn check_pattern_disallows_let(pattern: &Pattern) -> error::Result<()> {
    let mut names = Vec::new();
    collect_pattern_names(pattern, &mut names);
    if names.iter().any(|name| name.as_ref() == "let") {
        return Err(error::Error::syntax(
            "'let' cannot be used as a lexical or catch binding name".to_string(),
        ));
    }
    Ok(())
}

fn check_pattern_strict(pattern: &Pattern) -> error::Result<()> {
    let mut names = Vec::new();
    collect_pattern_names(pattern, &mut names);
    for n in &names {
        if matches!(&**n, "eval" | "arguments") {
            return Err(error::Error::syntax(format!(
                "'{}' cannot be used as a binding name in strict mode",
                n
            )));
        }
    }
    Ok(())
}

fn check_duplicate_bound_names(names: &[Arc<str>]) -> error::Result<()> {
    for (i, name) in names.iter().enumerate() {
        if names[..i].contains(name) {
            return Err(error::Error::syntax(format!(
                "Identifier '{}' has already been declared",
                name
            )));
        }
    }
    Ok(())
}

fn collect_catch_block_lexical_names(node: &StmtNode, lexical: &mut Vec<Arc<str>>) {
    let StmtNode::Block(body) = node else {
        return;
    };

    for stmt in body {
        match &stmt.node {
            StmtNode::VarDecl { kind, decls } => {
                if matches!(kind, VarKind::Let | VarKind::Const) {
                    for (name, _) in decls {
                        lexical.push(name.clone());
                    }
                }
            }
            StmtNode::Destructure { kind, pattern, .. } => {
                if matches!(kind, VarKind::Let | VarKind::Const) {
                    collect_pattern_names(pattern, lexical);
                }
            }
            StmtNode::FunctionDecl(f) => {
                if let Some(name) = &f.name {
                    lexical.push(name.clone());
                }
            }
            StmtNode::Labeled(_, body) if is_labelled_function(&stmt.node) => {
                if let StmtNode::FunctionDecl(f) = labelled_function_decl(&body.node) {
                    if let Some(name) = &f.name {
                        lexical.push(name.clone());
                    }
                }
            }
            StmtNode::ExprStmt(Expr::Class(c)) => {
                if let Some(name) = &c.name {
                    lexical.push(name.clone());
                }
            }
            _ => {}
        }
    }
}

fn check_catch_parameter_early_errors(param: &Pattern, body: &StmtNode) -> error::Result<()> {
    let mut param_names = Vec::new();
    collect_pattern_names(param, &mut param_names);
    check_duplicate_bound_names(&param_names)?;

    let mut lexical_names = Vec::new();
    collect_catch_block_lexical_names(body, &mut lexical_names);
    for name in &param_names {
        if lexical_names.contains(name) {
            return Err(error::Error::syntax(format!(
                "Identifier '{}' has already been declared",
                name
            )));
        }
    }
    Ok(())
}

fn labelled_function_decl(node: &StmtNode) -> &StmtNode {
    match node {
        StmtNode::Labeled(_, body) => labelled_function_decl(&body.node),
        _ => node,
    }
}
