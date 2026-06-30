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
            stmt_start_line: 0,
            expr_depth: 0,
            stmt_depth: 0,
        }
    }

    pub fn parse(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
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
        self.is_strict_context = is_strict;
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
                self.advance(); // ident
                self.advance(); // ':'
                let body = self.parse_stmt_inner()?;
                return Ok(self.stmt(StmtNode::Labeled(label, Box::new(body))));
            }
        }
        match self.peek().clone() {
            TokenKind::LBrace => self.parse_block(),
            TokenKind::Var | TokenKind::Let | TokenKind::Const => self.parse_var_decl(),
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
                Ok(self.stmt(StmtNode::Break(l)))
            }
            TokenKind::Continue => {
                self.advance();
                let l = self.parse_opt_label();
                self.expect_semi()?;
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
        let mut body = self.parse_fn_body()?;
        {
            let mut pre = self.take_dstr_prelude();
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
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

    fn parse_fn_body(&mut self) -> error::Result<Vec<Stmt>> {
        self.expect(&TokenKind::LBrace, "{")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace, "}")?;
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
        let then = Box::new(self.parse_stmt()?);
        let else_ = if self.eat(&TokenKind::Else) {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(self.stmt(StmtNode::If { cond, then, else_ }))
    }

    fn parse_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(self.stmt(StmtNode::While { cond, body }))
    }

    fn parse_with(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let object = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(self.stmt(StmtNode::With { object, body }))
    }

    fn parse_do_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        let body = Box::new(self.parse_stmt()?);
        self.expect(&TokenKind::While, "while")?;
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        self.eat(&TokenKind::Semicolon);
        Ok(self.stmt(StmtNode::DoWhile { body, cond }))
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
        } else if matches!(
            self.peek(),
            TokenKind::Var | TokenKind::Let | TokenKind::Const
        ) {
            // could be for-in / for-of
            let stmt = self.parse_var_decl_no_semi()?;
            if self.check(&TokenKind::In) {
                self.advance();
                let right = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
                let body = Box::new(self.parse_stmt()?);
                return Ok(self.stmt(StmtNode::ForIn {
                    left: Box::new(stmt),
                    right,
                    body,
                }));
            }
            if self.check(&TokenKind::Of) {
                self.advance();
                let right = self.parse_assign()?;
                self.expect(&TokenKind::RParen, ")")?;
                let body = Box::new(self.parse_stmt()?);
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
            let e = self.parse_expr()?;
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
        let body = Box::new(self.parse_stmt()?);
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
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected identifier, got {:?}",
                        other
                    )))
                }
            };
            let init = if self.eat(&TokenKind::Assign) {
                let mut e = self.parse_assign()?;
                Self::name_function_from_ident(&mut e, &name);
                Some(e)
            } else {
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
                if let TokenKind::Ident(s) = self.advance() {
                    catch_param = Some(Arc::from(s.as_str()));
                }
                self.expect(&TokenKind::RParen, ")")?;
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
        let mut cases = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let test = if self.eat(&TokenKind::Case) {
                Some(self.parse_expr()?)
            } else if self.eat(&TokenKind::Default) {
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
        // SetFunctionName for `obj.prop = <anon function>` / `obj[prop] = ...`.
        if matches!(op, AssignOp::Assign) {
            if let Some(key_name) = Self::assign_target_name(&left) {
                Self::name_function_from_ident(&mut right, &key_name);
            }
        }
        Ok(Expr::Assign(op, Box::new(left), Box::new(right)))
    }

    /// Extract the property name for SetFunctionName from an assignment
    /// target: `o.p` -> Some("p"), `o[computed]` -> None, identifier -> None.
    fn assign_target_name(target: &Expr) -> Option<Arc<str>> {
        match target {
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
                TokenKind::In => BinOp::In,
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
        let left = self.parse_unary()?;
        if self.check(&TokenKind::StarStar) {
            self.advance();
            let right = self.parse_exponent()?; // right-assoc
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
            let op = if matches!(self.peek(), TokenKind::Inc) {
                UpdateOp::Inc
            } else {
                UpdateOp::Dec
            };
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
                    let quasi0: Arc<str> = Arc::from(cooked.as_str());
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
                self.advance();
                Ok(Expr::Super)
            }
            TokenKind::Ident(s) => {
                // Could be arrow: x => ...
                if let TokenKind::Arrow = self.peek_at_tok(1).kind {
                    self.arrow_defaults = Vec::new();
                    self.arrow_rest = None;
                    self.advance(); // ident
                    self.advance(); // =>
                    return self.parse_arrow_body(vec![Arc::from(s.as_str())]);
                }
                self.advance();
                Ok(Expr::Ident(Arc::from(s.as_str())))
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
                TokenKind::TemplateString { cooked, .. } => quasis.push(Arc::from(cooked.as_str())),
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
        first: Arc<str>,
        first_raw: Arc<str>,
    ) -> error::Result<Expr> {
        let mut quasis: Vec<Arc<str>> = vec![first];
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
                    let c: Arc<str> = Arc::from(cooked.as_str());
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
            // Getter/setter: `get prop() {}` / `set prop(v) {}`
            let (is_getter, is_setter) = match self.peek().clone() {
                TokenKind::Ident(s)
                    if (s == "get" || s == "set")
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
                let mut body = self.parse_fn_body()?;
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
                let mut body = self.parse_fn_body()?;
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
                if !computed {
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
            _ => None,
        };
        let params = self.parse_params()?;
        let param_defaults = std::mem::take(&mut self.cur_param_defaults);
        let rest_param = self.cur_rest_param.take();
        let mut body = self.parse_fn_body()?;
        {
            let mut pre = self.take_dstr_prelude();
            pre.append(&mut body);
            body = pre;
        }
        let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
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

    fn parse_arrow_body(&mut self, params: Vec<Arc<str>>) -> error::Result<Expr> {
        let param_defaults = std::mem::take(&mut self.arrow_defaults);
        let rest_param = self.arrow_rest.take();
        let dstr_decls = std::mem::take(&mut self.arrow_destructure_decls);
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
            let mut body = self.parse_fn_body()?;
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
                let block = self.parse_fn_body()?;
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
                    let mut body = self.parse_fn_body()?;
                    {
                        let mut pre = self.take_dstr_prelude();
                        pre.append(&mut body);
                        body = pre;
                    }
                    methods.push(ClassMethod {
                        name: Arc::from(name.as_str()),
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
            let method_name = if is_constructor {
                self.advance();
                Arc::from("constructor")
            } else {
                Arc::from(self.read_property_name()?.as_str())
            };
            let params = self.parse_params()?;
            let param_defaults = std::mem::take(&mut self.cur_param_defaults);
            let rest_param = self.cur_rest_param.take();
            let mut body = self.parse_fn_body()?;
            {
                let mut pre = self.take_dstr_prelude();
                pre.append(&mut body);
                body = pre;
            }
            methods.push(ClassMethod {
                name: method_name,
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
