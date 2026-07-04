use crate::ast::*;
use crate::error;
use crate::token::{Token, TokenKind};
use std::sync::Arc;

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
    super_depth: usize,
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
            no_in: false,
            stmt_start_line: 0,
            expr_depth: 0,
            stmt_depth: 0,
            loop_depth: 0,
            switch_depth: 0,
            label_stack: Vec::new(),
            function_depth: 0,
            super_depth: 0,
        }
    }

    pub fn parse(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        p.parse_program()
    }

    /// Parse with an inherited strict-mode flag (used by direct eval in a
    /// strict caller context). The parser enforces strict-mode early errors
    /// even without an explicit "use strict" directive in the source.
    pub fn parse_strict_inherited(src: &str, inherited: bool) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        if inherited {
            p.is_strict_context = true;
        }
        p.parse_program()
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

    /// ES spec: these are FutureReservedWords that cannot be used as
    /// BindingIdentifier (variable name, parameter name, etc.).
    fn is_future_reserved(name: &str) -> bool {
        matches!(
            name,
            "enum" | "implements" | "interface" | "package" | "private" | "protected" | "public"
        )
    }

    /// Check that an identifier name is a valid binding name (not a
    /// FutureReservedWord). Returns the name on success, SyntaxError on failure.
    fn check_binding_name(&self, name: &str) -> error::Result<()> {
        if Self::is_future_reserved(name) {
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
        Ok(())
    }

    /// Determine if `let` at the current position is a lexical declaration
    /// (as opposed to an identifier). Per spec, `let` is a declaration when
    /// followed by `[`, `{`, or an identifier name. When followed by `=`, `in`,
    /// `of`, `;`, etc., it's an identifier (in non-strict mode).
    fn is_let_lexical_position(&self) -> bool {
        // In strict mode, `let` is always a lexical declaration.
        if self.is_strict_context {
            return true;
        }
        // Non-strict: `let` is lexical only when followed by `[`, `{`, or an
        // identifier (the start of a binding pattern or name) ON THE SAME LINE.
        // If a newline separates `let` from the next token, ASI applies and
        // `let` is treated as an identifier (expression statement).
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].preceded_by_newline {
            return false;
        }
        match self.peek_at_tok(1).kind {
            TokenKind::LBracket | TokenKind::LBrace => true,
            TokenKind::Ident(_) => true,
            _ => false,
        }
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
        // Surface any lexer-level error (e.g. an invalid escape
        // sequence in a string literal) as a SyntaxError before parsing.
        for t in &self.tokens {
            if let TokenKind::LexError(msg) = &t.kind {
                return Err(error::Error::syntax(msg.clone()));
            }
        }
        // Detect a leading "use strict" directive from the raw token stream
        // *before* parsing the body, so that nested function declarations
        // parsed within the body inherit strictness. A directive prologue is
        // a run of string-literal expression statements; only the leading
        // "use strict" matters here.
        let is_strict = self.peek_use_strict_directive();
        self.is_strict_context = is_strict || self.is_strict_context;
        let mut body = Vec::new();
        while !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        Ok(Program { body, is_strict })
    }

    /// Peek the token stream for a leading `"use strict"` string-literal
    /// directive (optionally followed by a semicolon and more directives).
    /// Does not consume tokens.
    fn peek_use_strict_directive(&self) -> bool {
        let mut i = self.pos;
        loop {
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::String(s)) if &**s == "use strict" => {
                    return true;
                }
                Some(TokenKind::String(_)) => {
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
        if let TokenKind::Ident(s) = self.peek().clone() {
            if matches!(self.peek_at_tok(1).kind, TokenKind::Colon) {
                let label = Arc::from(s.as_str());
                let label: Arc<str> = label;
                self.advance(); // ident
                self.advance(); // ':'
                                // Peek the body's first token to determine if it's a loop.
                let is_loop = matches!(
                    self.peek(),
                    TokenKind::While | TokenKind::Do | TokenKind::For
                );
                // ES spec: lexical declarations (let/const/class) cannot be
                // the body of a labelled statement.
                if matches!(
                    self.peek(),
                    TokenKind::Let | TokenKind::Const | TokenKind::Class
                ) {
                    return Err(error::Error::syntax(
                        "Lexical declaration cannot be the body of a labelled statement"
                            .to_string(),
                    ));
                }
                self.label_stack.push((label.clone(), is_loop));
                let body = self.parse_stmt_inner()?;
                self.label_stack.pop();
                return Ok(self.stmt(StmtNode::Labeled(label, Box::new(body))));
            }
        }
        match self.peek().clone() {
            TokenKind::LBrace => self.parse_block(),
            TokenKind::Var | TokenKind::Const => self.parse_var_decl(),
            TokenKind::Let if self.is_let_lexical_position() => self.parse_var_decl(),
            TokenKind::Function => self.parse_function_decl(),
            TokenKind::Async => {
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function) {
                    self.advance(); // async
                    let mut d = self.parse_function_decl()?;
                    if let StmtNode::FunctionDecl(fe) = &mut d.node {
                        fe.is_async = true;
                    }
                    Ok(d)
                } else {
                    let e = self.parse_expr()?;
                    self.expect_semi()?;
                    Ok(self.stmt(StmtNode::ExprStmt(e)))
                }
            }
            TokenKind::Class => self.parse_class_decl(),
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
            TokenKind::Throw => {
                self.advance();
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

    fn parse_opt_label(&mut self) -> Option<Arc<str>> {
        // Per spec, `break` and `continue` must not have a line terminator
        // between the keyword and the label. If a newline precedes the next
        // token, ASI applies and the statement is unlabelled.
        if self.peek_at_tok(0).preceded_by_newline {
            return None;
        }
        if let TokenKind::Ident(s) = self.peek().clone() {
            self.advance();
            Some(Arc::from(s.as_str()))
        } else {
            None
        }
    }

    fn parse_block(&mut self) -> error::Result<Stmt> {
        self.expect(&TokenKind::LBrace, "{")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(self.stmt(StmtNode::Block(body)))
    }

    fn parse_var_decl(&mut self) -> error::Result<Stmt> {
        let stmt = self.parse_var_decl_no_semi()?;
        self.expect_semi()?;
        Ok(stmt)
    }

    fn parse_function_decl(&mut self) -> error::Result<Stmt> {
        self.advance(); // function
        let is_generator = self.eat(&TokenKind::Star);
        let name = match self.advance() {
            TokenKind::Ident(s) => Some(Arc::from(s.as_str())),
            other => {
                return Err(error::Error::syntax(format!(
                    "Expected function name, got {:?}",
                    other
                )))
            }
        };
        let params = self.parse_params()?;
        let param_defaults = std::mem::take(&mut self.cur_param_defaults);
        let rest_param = self.cur_rest_param.take();
        let mut body = self.parse_fn_body(false)?;
        {
            let mut pre = self.take_dstr_prelude();
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
        // Strict mode (inherited or from body directive): validate that no
        // parameter name is `eval` or `arguments`, and no duplicate params.
        if is_strict {
            if let Some(ref n) = name {
                if matches!(&**n, "eval" | "arguments") {
                    return Err(error::Error::syntax(format!(
                        "'{}' cannot be used as a function name in strict mode",
                        n
                    )));
                }
            }
            for p in &params {
                if matches!(&**p, "eval" | "arguments") {
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
            is_async: false,
            is_generator,
            param_decls: Vec::new(),
            is_strict,
            is_method: false,
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
                    let tmp = format!("__arg{}", params.len());
                    self.cur_rest_param = Some(Arc::from(tmp.as_str()));
                    self.cur_param_destructure_decls.push((p, tmp, None));
                    break;
                }
                if let TokenKind::Ident(s) = self.advance() {
                    self.check_binding_name(&s)?;
                    self.cur_rest_param = Some(Arc::from(s.as_str()));
                } else {
                    return Err(error::Error::syntax(
                        "Expected rest parameter name".to_string(),
                    ));
                }
                break;
            }
            match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.advance();
                    self.check_binding_name(&s)?;
                    params.push(Arc::from(s.as_str()));
                    let default = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    self.cur_param_defaults.push(default);
                }
                TokenKind::LBracket | TokenKind::LBrace => {
                    // Destructuring parameter: `function f([a, b])` / `f({x, y})`.
                    let p = self.parse_destructure_pattern()?;
                    let tmp = format!("__arg{}", params.len());
                    params.push(Arc::from(tmp.as_str()));
                    self.cur_param_defaults.push(None);
                    let default = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    self.cur_param_destructure_decls.push((p, tmp, default));
                }
                _ => return Err(error::Error::syntax("Expected parameter name".to_string())),
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

    fn parse_fn_body(&mut self, super_allowed: bool) -> error::Result<Vec<Stmt>> {
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
        if super_allowed {
            self.super_depth += 1;
        } else {
            self.super_depth = 0;
        }
        self.function_depth += 1;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace, "}")?;
        self.function_depth -= 1;
        self.is_strict_context = saved_strict;
        self.super_depth = saved_super;
        Ok(body)
    }

    /// Take the destructuring-parameter declarations collected by the most
    /// recent `parse_params` and turn them into a prelude of `let <pat> =
    /// __argN;` statements to prepend to a function body.
    fn take_dstr_prelude(&mut self) -> Vec<Stmt> {
        let dstr_decls = std::mem::take(&mut self.cur_param_destructure_decls);
        dstr_decls
            .into_iter()
            .map(|(pattern, tmp, default)| {
                // If the destructuring parameter had a default, the binding
                // source is `__argN === undefined ? <default> : __argN`. We
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
        let stmt = self.parse_stmt()?;
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
            if self.check(&TokenKind::Of) {
                // for-of head declarations must not have an initializer.
                match &stmt.node {
                    StmtNode::VarDecl { decls, .. } => {
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
                && matches!(self.peek_at_tok(1).kind, TokenKind::Of)
            {
                return Err(error::Error::syntax(
                    "let followed by of in for-of head is not valid".to_string(),
                ));
            }
            self.no_in = true;
            let e = self.parse_assign()?;
            self.no_in = false;
            if self.check(&TokenKind::In) {
                // Validate that the LHS is a valid assignment target.
                let is_valid_target = match &e {
                    Expr::Ident(_)
                    | Expr::Member { .. }
                    | Expr::Array(_)
                    | Expr::Object(_)
                    | Expr::PrivateGet { .. } => true,
                    _ => false,
                };
                if !is_valid_target {
                    return Err(error::Error::syntax(
                        "Invalid left-hand side in for-in".to_string(),
                    ));
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
            if self.check(&TokenKind::Of) {
                // Validate that the LHS is a valid assignment target.
                let is_valid_target = match &e {
                    Expr::Ident(_)
                    | Expr::Member { .. }
                    | Expr::Array(_)
                    | Expr::Object(_)
                    | Expr::PrivateGet { .. } => true,
                    _ => false,
                };
                if !is_valid_target {
                    return Err(error::Error::syntax(
                        "Invalid left-hand side in for-of".to_string(),
                    ));
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
        Ok(self.stmt(StmtNode::For {
            init,
            cond,
            update,
            body,
        }))
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
                TokenKind::Of => Arc::from("of"),
                TokenKind::Let if !self.is_strict_context => Arc::from("let"),
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
                if self.is_strict_context {
                    check_pattern_strict(&pat)?;
                }
                self.expect(&TokenKind::RParen, ")")?;
                catch_param = Some(pat);
            }
            catch_body = Some(Box::new(self.parse_block()?));
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
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let test = if self.eat(&TokenKind::Case) {
                Some(self.parse_expr()?)
            } else if self.eat(&TokenKind::Default) {
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
            self.expect(&TokenKind::Colon, ":")?;
            let mut body = Vec::new();
            while !self.check(&TokenKind::Case)
                && !self.check(&TokenKind::Default)
                && !self.check(&TokenKind::RBrace)
                && !self.check(&TokenKind::Eof)
            {
                body.push(self.parse_stmt()?);
            }
            cases.push(SwitchCase { test, body });
        }
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
                collect_decl_names(
                    &stmt.node,
                    &mut lexical_names,
                    &mut var_names,
                    self.is_strict_context,
                );
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
        let left = self.parse_ternary()?;
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
            _ => return Ok(left),
        };
        self.advance();
        let mut right = self.parse_assign()?;
        // Validate that the left side is a valid assignment target.
        // Invalid: literals, binary ops, unary ops, function calls, etc.
        match &left {
            Expr::Ident(_)
            | Expr::Member { .. }
            | Expr::PrivateGet { .. }
            | Expr::Array(_)
            | Expr::Object(_) => {}
            _ => {
                return Err(error::Error::syntax(
                    "Invalid left-hand side in assignment".to_string(),
                ));
            }
        }
        // Strict mode: assignment to `eval` or `arguments` is a SyntaxError.
        if self.is_strict_context {
            if let Expr::Ident(ref id) = left {
                if matches!(&**id, "eval" | "arguments") {
                    return Err(error::Error::syntax(format!(
                        "Assignment to '{}' is not allowed in strict mode",
                        id
                    )));
                }
            }
        }
        // SetFunctionName for `obj.prop = <anon function>` / `obj[prop] = ...`.
        if matches!(op, AssignOp::Assign) {
            if let Some(key_name) = Self::assign_target_name(&left) {
                Self::name_function_from_ident(&mut right, &key_name);
            }
        }
        Ok(Expr::Assign(op, Box::new(left), Box::new(right)))
    }

    /// Extract the property name for SetFunctionName from an assignment
    /// target: `o.p` -> Some("p"), `o[computed]` -> None, identifier -> Some(name).
    fn assign_target_name(target: &Expr) -> Option<Arc<str>> {
        match target {
            Expr::Ident(s) => Some(s.clone()),
            Expr::Member {
                property,
                computed: false,
                ..
            } => match property.as_ref() {
                Expr::Ident(s) => Some(s.clone()),
                Expr::String(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn parse_ternary(&mut self) -> error::Result<Expr> {
        let cond = self.parse_nullish()?;
        if self.eat(&TokenKind::Question) {
            let then = self.parse_assign()?;
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
        let mut left = self.parse_logical_or()?;
        while self.check(&TokenKind::Nullish) {
            self.advance();
            let right = self.parse_logical_or()?;
            left = Expr::Logical(LogicalOp::Nullish, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_or(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_logical_and()?;
        while self.check(&TokenKind::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::Logical(LogicalOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> error::Result<Expr> {
        let mut left = self.parse_bit_or()?;
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
            if !matches!(
                &e,
                Expr::Ident(_) | Expr::Member { .. } | Expr::PrivateGet { .. }
            ) {
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
            let e = self.parse_unary()?;
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
            if !matches!(
                &e,
                Expr::Ident(_) | Expr::Member { .. } | Expr::PrivateGet { .. }
            ) {
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
        let mut e = self.parse_primary()?;
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
                        };
                    } else {
                        let name = self.read_property_name()?;
                        let prop = Expr::String(Arc::from(name.as_str()));
                        e = Expr::Member {
                            object: Box::new(e),
                            property: Box::new(prop),
                            computed: false,
                            optional: false,
                        };
                    }
                }
                TokenKind::QuestionDot => {
                    self.advance();
                    match self.peek() {
                        TokenKind::LParen => {
                            self.advance();
                            let args = self.parse_args()?;
                            self.expect(&TokenKind::RParen, ")")?;
                            e = Expr::Call {
                                callee: Box::new(e),
                                args,
                                optional: true,
                            };
                        }
                        TokenKind::LBracket => {
                            self.advance();
                            let prop = self.parse_expr()?;
                            self.expect(&TokenKind::RBracket, "]")?;
                            e = Expr::Member {
                                object: Box::new(e),
                                property: Box::new(prop),
                                computed: true,
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
                            };
                        }
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let prop = self.parse_expr()?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    e = Expr::Member {
                        object: Box::new(e),
                        property: Box::new(prop),
                        computed: true,
                        optional: false,
                    };
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&TokenKind::RParen, ")")?;
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        optional: false,
                    };
                }
                TokenKind::TemplateString { cooked, raw } => {
                    // Tagged template: tag`str${expr}str`
                    let quasi0: Option<Arc<str>> = cooked.map(|s| Arc::from(s.as_str()));
                    let raw0: Arc<str> = Arc::from(raw.as_str());
                    self.advance(); // consume the TemplateString token
                    let tag = e;
                    e = self.parse_tagged_template(tag, quasi0, raw0)?;
                }
                _ => break,
            }
        }
        Ok(e)
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
        match self.peek().clone() {
            TokenKind::Await => {
                self.advance();
                let inner = self.parse_unary()?;
                Ok(Expr::Await(Box::new(inner)))
            }
            TokenKind::Yield => {
                self.advance();
                // `yield* expr` - delegate to another iterable/generator.
                if matches!(self.peek(), TokenKind::Star) {
                    self.advance(); // consume '*'
                    let inner = self.parse_assign()?;
                    return Ok(Expr::YieldDelegate(Box::new(inner)));
                }
                let inner = if matches!(
                    self.peek(),
                    TokenKind::Semicolon
                        | TokenKind::RBrace
                        | TokenKind::RParen
                        | TokenKind::Comma
                        | TokenKind::Eof
                ) {
                    None
                } else {
                    // Per spec, `yield` is a low-precedence operator: its
                    // operand extends through the assignment-expression level,
                    // so `yield 1 + 1` means `yield (1 + 1)`, not `(yield 1) + 1`.
                    Some(Box::new(self.parse_assign()?))
                };
                Ok(Expr::Yield(inner))
            }
            TokenKind::Async => {
                // `async function ...` expression; `async () =>` arrow; otherwise
                // `async` is treated as a plain identifier.
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function) {
                    self.advance(); // async
                    let mut f = self.parse_function_expr()?;
                    if let Expr::Function(fe) = &mut f {
                        fe.is_async = true;
                    }
                    return Ok(f);
                }
                // async arrow: `async (params) => body` or `async ident => body`
                let is_async_arrow_paren = matches!(self.peek_at_tok(1).kind, TokenKind::LParen);
                let is_async_arrow_ident = matches!(self.peek_at_tok(1).kind, TokenKind::Ident(_))
                    && matches!(self.peek_at_tok(2).kind, TokenKind::Arrow);
                if is_async_arrow_paren {
                    self.advance(); // async
                                    // Now at `(`; parse like a parenthesized arrow.
                    self.advance(); // (
                    if self.try_parse_arrow_params()? {
                        let params = self.last_arrow_params.take().unwrap();
                        self.expect(&TokenKind::Arrow, "=>")?;
                        let mut f = self.parse_arrow_body(params)?;
                        if let Expr::Arrow(fe) = &mut f {
                            fe.is_async = true;
                        }
                        return Ok(f);
                    }
                    // Not an arrow; rewind and treat async as identifier.
                    self.pos -= 2;
                    self.advance();
                    return Ok(Expr::Ident(Arc::from("async")));
                }
                if is_async_arrow_ident {
                    self.advance(); // async
                    let name = match self.peek().clone() {
                        TokenKind::Ident(s) => {
                            self.advance();
                            Arc::from(s.as_str())
                        }
                        _ => unreachable!(),
                    };
                    self.advance(); // =>
                    let mut f = self.parse_arrow_body(vec![name])?;
                    if let Expr::Arrow(fe) = &mut f {
                        fe.is_async = true;
                    }
                    return Ok(f);
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
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::BigInt(s) => {
                self.advance();
                let n = num_bigint::BigInt::parse_bytes(s.as_bytes(), 10).unwrap_or_default();
                Ok(Expr::BigInt(n))
            }
            TokenKind::String(s) => {
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
                Ok(Expr::Undefined)
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
                self.parse_class_body().map(Expr::Class)
            }
            TokenKind::Ident(s) => {
                // Strict mode: FutureReservedWords cannot be used as identifiers.
                if self.is_strict_context && Self::is_future_reserved(&s) {
                    return Err(error::Error::syntax(format!(
                        "'{}' is a reserved word in strict mode",
                        s
                    )));
                }
                // Could be arrow: x => ...
                if let TokenKind::Arrow = self.peek_at_tok(1).kind {
                    self.arrow_defaults = Vec::new();
                    self.arrow_rest = None;
                    self.advance(); // ident
                    self.advance(); // =>
                    self.check_binding_name(&s)?;
                    return self.parse_arrow_body(vec![Arc::from(s.as_str())]);
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
            TokenKind::LParen => {
                // Could be arrow: (a, b) => ...
                self.advance();
                if self.try_parse_arrow_params()? {
                    let params = self.last_arrow_params.take().unwrap();
                    self.expect(&TokenKind::Arrow, "=>")?;
                    return self.parse_arrow_body(params);
                }
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
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
                elements.push(Expr::Undefined); // hole
                continue;
            }
            if self.check(&TokenKind::Spread) {
                self.advance();
                elements.push(Expr::Spread(Box::new(self.parse_assign()?)));
            } else {
                elements.push(self.parse_assign()?);
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
                && matches!(
                    self.peek_at_tok(1).kind,
                    TokenKind::Ident(_)
                        | TokenKind::String(_)
                        | TokenKind::Number(_)
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
                    && matches!(
                        self.peek_at_tok(1).kind,
                        TokenKind::Ident(_)
                            | TokenKind::String(_)
                            | TokenKind::Number(_)
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
                TokenKind::Number(n) => {
                    self.advance();
                    (PropertyKey::Number(n), false)
                }
                TokenKind::LBracket => {
                    self.advance();
                    let e = self.parse_assign()?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    // Computed key: the expression is evaluated at runtime, so even a
                    // bare identifier `[key]` must become a Computed key (not the
                    // constant Ident form used by shorthand `{x}`).
                    let key = match e {
                        Expr::String(s) => PropertyKey::String(s),
                        Expr::Number(n) => PropertyKey::Number(n),
                        other => PropertyKey::Computed(Box::new(other)),
                    };
                    (key, true)
                }
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected property key, got {:?}",
                        other
                    )))
                }
            };
            if is_getter || is_setter {
                let params = self.parse_params()?;
                let param_defaults = std::mem::take(&mut self.cur_param_defaults);
                let rest_param = self.cur_rest_param.take();
                let mut body = self.parse_fn_body(true)?;
                {
                    let mut pre = self.take_dstr_prelude();
                    pre.append(&mut body);
                    body = pre;
                }
                let accessor_name = Self::prop_key_name(&key).map(|n| {
                    let prefix = if is_getter { "get " } else { "set " };
                    Arc::from(format!("{}{}", prefix, n).as_str())
                });
                let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
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
                        is_method: false,
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
                let params = self.parse_params()?;
                let param_defaults = std::mem::take(&mut self.cur_param_defaults);
                let rest_param = self.cur_rest_param.take();
                let mut body = self.parse_fn_body(true)?;
                {
                    let mut pre = self.take_dstr_prelude();
                    pre.append(&mut body);
                    body = pre;
                }
                let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
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
                    Expr::Ident(s.clone())
                } else {
                    return Err(error::Error::syntax(
                        "Shorthand property requires an identifier key".to_string(),
                    ));
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
                let mut value = self.parse_assign()?;
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
        Ok(Expr::Object(props))
    }

    fn parse_function_expr(&mut self) -> error::Result<Expr> {
        self.advance(); // function
        let is_generator = self.eat(&TokenKind::Star);
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => {
                self.advance();
                Some(Arc::from(s.as_str()))
            }
            TokenKind::Await if !self.is_strict_context => {
                self.advance();
                Some(Arc::from("await"))
            }
            TokenKind::Yield if !self.is_strict_context => {
                self.advance();
                Some(Arc::from("yield"))
            }
            _ => None,
        };
        let params = self.parse_params()?;
        let param_defaults = std::mem::take(&mut self.cur_param_defaults);
        let rest_param = self.cur_rest_param.take();
        let mut body = self.parse_fn_body(false)?;
        {
            let mut pre = self.take_dstr_prelude();
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
        // Strict mode: validate parameter names and duplicates (same as decl).
        if is_strict {
            if let Some(ref n) = name {
                if matches!(&**n, "eval" | "arguments") {
                    return Err(error::Error::syntax(format!(
                        "'{}' cannot be used as a function name in strict mode",
                        n
                    )));
                }
            }
            for p in &params {
                if matches!(&**p, "eval" | "arguments") {
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
        Ok(Expr::Function(FunctionExpr {
            name,
            params,
            param_defaults,
            rest_param,
            body,
            is_arrow: false,
            is_async: false,
            is_generator,
            param_decls: Vec::new(),
            is_strict,
            is_method: false,
        }))
    }

    fn parse_new(&mut self) -> error::Result<Expr> {
        self.advance(); // new
                        // new.target
        if self.check(&TokenKind::Dot) {
            // peek at the property name
            if let TokenKind::Ident(s) = self.peek_at_tok(1).kind.clone() {
                if s == "target" {
                    self.advance(); // .
                    self.advance(); // target
                    return Ok(Expr::NewTarget);
                }
            }
        }
        // parse the constructor (primary + member access, but NOT call parens)
        let mut callee = self.parse_primary()?;
        // allow member access on the constructor: new Foo.Bar()
        while self.check(&TokenKind::Dot) {
            self.advance();
            let name = self.read_property_name()?;
            let prop = Expr::String(Arc::from(name.as_str()));
            callee = Expr::Member {
                object: Box::new(callee),
                property: Box::new(prop),
                computed: false,
                optional: false,
            };
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
                    let tmp = format!("__arg{}", params.len());
                    rest = Some(Arc::from(tmp.as_str()));
                    dstr_decls.push((p, tmp, None));
                    break;
                }
                if let TokenKind::Ident(s) = self.advance() {
                    rest = Some(Arc::from(s.as_str()));
                } else {
                    self.pos = save;
                    return Ok(false);
                }
                break;
            }
            match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.advance();
                    params.push(Arc::from(s.as_str()));
                    let d = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    defaults.push(d);
                }
                TokenKind::LBracket | TokenKind::LBrace => {
                    // Destructuring parameter: `([a, b]) =>` / `({x, y}) =>`.
                    // Synthesize a positional temp param and remember the
                    // pattern so the body can bind it: `let <pat> = __argN;`.
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
                    let tmp = format!("__arg{}", params.len());
                    params.push(Arc::from(tmp.as_str()));
                    defaults.push(None);
                    // Optional default: `({a} = {}) =>`
                    let default = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    dstr_decls.push((p, tmp, default));
                }
                _ => {
                    self.pos = save;
                    return Ok(false);
                }
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

    /// Validate arrow-function parameters per strict-mode rules, which apply
    /// to arrow functions even when the enclosing context is sloppy.
    fn validate_arrow_params(
        &self,
        params: &[Arc<str>],
        dstr_decls: &[(Pattern, String, Option<Expr>)],
        rest_param: Option<&Arc<str>>,
    ) -> error::Result<()> {
        let mut seen = std::collections::HashSet::new();
        for p in params {
            if matches!(&**p, "eval" | "arguments") {
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
            Self::pattern_binding_names(pattern, &mut seen);
        }
        if let Some(r) = rest_param {
            if matches!(&**r, "eval" | "arguments") {
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

    /// Collect all binding identifiers introduced by a destructuring pattern.
    fn pattern_binding_names(pattern: &Pattern, out: &mut std::collections::HashSet<Arc<str>>) {
        match pattern {
            Pattern::Ident(name) => {
                out.insert(name.clone());
            }
            Pattern::Hole => {}
            Pattern::Array(elems) => {
                for el in elems {
                    Self::pattern_binding_names(el, out);
                }
            }
            Pattern::Object(props, rest) => {
                for (_, target) in props {
                    Self::pattern_binding_names(target, out);
                }
                if let Some(r) = rest {
                    Self::pattern_binding_names(r, out);
                }
            }
            Pattern::Assign(inner, _) => Self::pattern_binding_names(inner, out),
            Pattern::Rest(inner) => Self::pattern_binding_names(inner, out),
        }
    }

    fn parse_arrow_body(&mut self, params: Vec<Arc<str>>) -> error::Result<Expr> {
        let param_defaults = std::mem::take(&mut self.arrow_defaults);
        let rest_param = self.arrow_rest.take();
        let dstr_decls = std::mem::take(&mut self.arrow_destructure_decls);
        // Arrow functions always use strict-mode parameter rules, even in
        // sloppy mode: eval/arguments are forbidden and duplicate bindings are
        // rejected. Validate before consuming the destructuring declarations.
        self.validate_arrow_params(&params, &dstr_decls, rest_param.as_ref())?;
        // Synthesize `let <pattern> = __argN;` prelude statements that bind
        // each destructuring parameter from its positional temp argument.
        // A parameter default wraps the pattern so the compiler applies it
        // when the source value is undefined.
        let prelude: Vec<Stmt> = dstr_decls
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
            .collect();
        // arrow body: expression or block
        if self.check(&TokenKind::LBrace) {
            let mut body = self.parse_fn_body(false)?;
            {
                let mut pre = self.take_dstr_prelude();
                pre.append(&mut body);
                body = pre;
            }
            if !prelude.is_empty() {
                let mut combined = prelude;
                combined.append(&mut body);
                body = combined;
            }
            let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
            // 'use strict' directive not allowed with non-simple params.
            let has_non_simple = !param_defaults.is_empty()
                || rest_param.is_some()
                || !self.arrow_destructure_decls.is_empty();
            if has_non_simple && Self::scan_directive_prologue(&body) {
                return Err(error::Error::syntax(
                    "'use strict' not allowed with non-simple parameters".to_string(),
                ));
            }
            Ok(Expr::Arrow(FunctionExpr {
                name: None,
                params,
                param_defaults,
                rest_param,
                body,
                is_arrow: true,
                is_async: false,
                is_generator: false,
                param_decls: Vec::new(),
                is_strict,
                is_method: false,
            }))
        } else {
            let e = self.parse_assign()?;
            let mut body = prelude;
            body.push(self.stmt(StmtNode::Return(Some(e))));
            Ok(Expr::Arrow(FunctionExpr {
                name: None,
                params,
                param_defaults,
                rest_param,
                body,
                is_arrow: true,
                is_async: false,
                is_generator: false,
                param_decls: Vec::new(),
                // Arrow with expression body has no directive prologue; inherit.
                is_strict: self.is_strict_context,
                is_method: false,
            }))
        }
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

    /// SetFunctionName for `var x = <function>`: if `value` is an anonymous
    /// function/arrow and `name` is a plain identifier, set its `name` to it.
    fn name_function_from_ident(value: &mut Expr, name: &Arc<str>) {
        match value {
            Expr::Function(f) if f.name.is_none() => f.name = Some(name.clone()),
            Expr::Arrow(f) if f.name.is_none() => f.name = Some(name.clone()),
            _ => {}
        }
    }

    fn read_property_name(&mut self) -> error::Result<String> {
        // Accept identifiers and keywords as property names after `.`
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => s,
            TokenKind::Delete => "delete".to_string(),
            TokenKind::Typeof => "typeof".to_string(),
            TokenKind::Void => "void".to_string(),
            TokenKind::New => "new".to_string(),
            TokenKind::Of => "of".to_string(),
            TokenKind::In => "in".to_string(),
            TokenKind::Instanceof => "instanceof".to_string(),
            TokenKind::This => "this".to_string(),
            TokenKind::Null => "null".to_string(),
            TokenKind::True => "true".to_string(),
            TokenKind::False => "false".to_string(),
            TokenKind::Undefined => "undefined".to_string(),
            TokenKind::Catch => "catch".to_string(),
            TokenKind::Class => "class".to_string(),
            TokenKind::Extends => "extends".to_string(),
            TokenKind::Function => "function".to_string(),
            TokenKind::Return => "return".to_string(),
            TokenKind::If => "if".to_string(),
            TokenKind::Else => "else".to_string(),
            TokenKind::For => "for".to_string(),
            TokenKind::While => "while".to_string(),
            TokenKind::Do => "do".to_string(),
            TokenKind::Break => "break".to_string(),
            TokenKind::Continue => "continue".to_string(),
            TokenKind::Throw => "throw".to_string(),
            TokenKind::Try => "try".to_string(),
            TokenKind::Finally => "finally".to_string(),
            TokenKind::Switch => "switch".to_string(),
            TokenKind::With => "with".to_string(),
            TokenKind::Case => "case".to_string(),
            TokenKind::Default => "default".to_string(),
            TokenKind::Var => "var".to_string(),
            TokenKind::Let => "let".to_string(),
            TokenKind::Const => "const".to_string(),
            TokenKind::Async => "async".to_string(),
            TokenKind::Await => "await".to_string(),
            TokenKind::Yield => "yield".to_string(),
            TokenKind::Super => "super".to_string(),
            other => {
                return Err(error::Error::syntax(format!(
                    "Expected property name after ., got {:?}",
                    other
                )))
            }
        };
        self.advance();
        Ok(name)
    }

    fn parse_class_decl(&mut self) -> error::Result<Stmt> {
        // Parse a class declaration as a statement that evaluates the class expr.
        let cls = self.parse_class_body()?;
        Ok(self.stmt(StmtNode::ExprStmt(Expr::Class(cls))))
    }

    fn parse_class_body(&mut self) -> error::Result<ClassExpr> {
        self.advance(); // 'class'
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => {
                self.advance();
                Some(Arc::from(s.as_str()))
            }
            TokenKind::Await if !self.is_strict_context => {
                self.advance();
                Some(Arc::from("await"))
            }
            TokenKind::Yield if !self.is_strict_context => {
                self.advance();
                Some(Arc::from("yield"))
            }
            _ => None,
        };
        let superclass = if self.eat(&TokenKind::Extends) {
            Some(Box::new(self.parse_postfix()?))
        } else {
            None
        };
        self.expect(&TokenKind::LBrace, "{")?;
        let mut methods = Vec::new();
        let mut static_blocks: Vec<Vec<Stmt>> = Vec::new();
        let mut private_fields: Vec<crate::ast::PrivateFieldDecl> = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            // static { ... } initialization block
            if self.check(&TokenKind::Static)
                && matches!(self.peek_at_tok(1).kind, TokenKind::LBrace)
            {
                self.advance(); // static
                let block = self.parse_fn_body(false)?;
                static_blocks.push(block);
                continue;
            }
            let is_static = self.eat(&TokenKind::Static);
            // Private field declaration: #name = init  or  #name;
            // Private method: #name(params) { body }  (also static #name() {})
            if let TokenKind::PrivateName(name) = self.peek().clone() {
                // Peek ahead: if next is `(`, this is a private method.
                let is_private_method = matches!(self.peek_at_tok(1).kind, TokenKind::LParen);
                if is_private_method {
                    self.advance(); // consume #name
                    let params = self.parse_params()?;
                    let param_defaults = std::mem::take(&mut self.cur_param_defaults);
                    let rest_param = self.cur_rest_param.take();
                    let mut body = self.parse_fn_body(true)?;
                    {
                        let mut pre = self.take_dstr_prelude();
                        pre.append(&mut body);
                        body = pre;
                    }
                    methods.push(ClassMethod {
                        name: Arc::from(name.as_str()),
                        computed_name: None,
                        params,
                        param_defaults,
                        rest_param,
                        body,
                        is_static,
                        is_constructor: false,
                        kind: crate::ast::PropKind::Method,
                        is_private: true,
                    });
                    continue;
                }
                self.advance();
                let init = if self.eat(&TokenKind::Assign) {
                    Some(Box::new(self.parse_assign()?))
                } else {
                    None
                };
                self.expect_semi()?;
                private_fields.push(crate::ast::PrivateFieldDecl {
                    name: Arc::from(name.as_str()),
                    init,
                });
                continue;
            }
            // Getter/setter in class body.
            let (is_getter, is_setter) = match self.peek().clone() {
                TokenKind::Ident(s)
                    if (s == "get" || s == "set")
                        && !self.tokens[self.pos].had_escape
                        && !matches!(
                            self.peek_at_tok(1).kind,
                            TokenKind::LParen | TokenKind::Assign | TokenKind::Semicolon
                        ) =>
                {
                    (s == "get", s == "set")
                }
                _ => (false, false),
            };
            if is_getter || is_setter {
                self.advance();
            }
            let is_constructor = !is_getter
                && !is_setter
                && matches!(self.peek().clone(), TokenKind::Ident(ref s) if s == "constructor");
            // Computed method name: [expr]
            let computed_name = if !is_getter && !is_setter && self.check(&TokenKind::LBracket) {
                self.advance();
                let e = self.parse_assign()?;
                self.expect(&TokenKind::RBracket, "]")?;
                Some(Box::new(e))
            } else {
                None
            };
            let method_name = if is_constructor {
                self.advance();
                Arc::from("constructor")
            } else if computed_name.is_some() {
                // Placeholder name; the compiler uses computed_name for the actual key.
                Arc::from("")
            } else {
                // Class method names can be identifiers, keywords, numbers,
                // strings, or computed expressions.
                match self.peek().clone() {
                    TokenKind::Number(n) => {
                        self.advance();
                        Arc::from(format!("{}", n))
                    }
                    TokenKind::String(s) => {
                        self.advance();
                        Arc::from(s.as_str())
                    }
                    _ => Arc::from(self.read_property_name()?.as_str()),
                }
            };
            let params = self.parse_params()?;
            let param_defaults = std::mem::take(&mut self.cur_param_defaults);
            let rest_param = self.cur_rest_param.take();
            let mut body = self.parse_fn_body(true)?;
            {
                let mut pre = self.take_dstr_prelude();
                pre.append(&mut body);
                body = pre;
            }
            methods.push(ClassMethod {
                name: method_name,
                computed_name,
                params,
                param_defaults,
                rest_param,
                body,
                is_static,
                is_constructor,
                kind: if is_getter {
                    crate::ast::PropKind::Get
                } else if is_setter {
                    crate::ast::PropKind::Set
                } else {
                    crate::ast::PropKind::Method
                },
                is_private: false,
            });
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(ClassExpr {
            name,
            superclass,
            methods,
            static_blocks,
            private_fields,
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
                        TokenKind::String(s) => {
                            self.advance();
                            PropertyKey::String(Arc::from(s.as_str()))
                        }
                        TokenKind::Number(n) => {
                            self.advance();
                            PropertyKey::Number(n)
                        }
                        TokenKind::LBracket => {
                            self.advance();
                            let e = self.parse_assign()?;
                            self.expect(&TokenKind::RBracket, "]")?;
                            PropertyKey::Computed(Box::new(e))
                        }
                        other => {
                            return Err(error::Error::syntax(format!(
                                "Expected property name in object pattern, got {:?}",
                                other
                            )))
                        }
                    };
                    // `key: target` renames; otherwise bind to same name (ident/string only).
                    let target = if self.eat(&TokenKind::Colon) {
                        self.parse_destructure_pattern()?
                    } else {
                        match &key {
                            PropertyKey::Ident(s) => Pattern::Ident(s.clone()),
                            PropertyKey::String(s) => Pattern::Ident(s.clone()),
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
            TokenKind::Ident(s) => {
                self.advance();
                Ok(Pattern::Ident(Arc::from(s.as_str())))
            }
            other => Err(error::Error::syntax(format!(
                "Expected pattern, got {:?}",
                other
            ))),
        }
    }
}

/// Collect top-level declaration names from a statement node that appears
/// directly inside a switch case body. Lexical declarations (let/const/class,
/// and function in strict mode) go into `lexical`; var declarations (var,
/// and function in sloppy mode) go into `var`. Does NOT descend into blocks
/// or nested function bodies — only the case body's direct children matter.
/// Check if a statement is a labelled function declaration (possibly nested
/// through multiple labels).
fn is_labelled_function(node: &StmtNode) -> bool {
    match node {
        StmtNode::Labeled(_, body) => is_labelled_function(&body.node),
        StmtNode::FunctionDecl(_) => true,
        _ => false,
    }
}

fn collect_decl_names(
    node: &StmtNode,
    lexical: &mut Vec<Arc<str>>,
    var: &mut Vec<Arc<str>>,
    is_strict: bool,
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
                if is_strict {
                    lexical.push(name.clone());
                } else {
                    var.push(name.clone());
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
    fn parse_try_catch() {
        let p = parse("try { f(); } catch (e) { g(); } finally { h(); }");
        assert!(matches!(&p.body[0].node, StmtNode::TryCatch { .. }));
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
