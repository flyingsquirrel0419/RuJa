use crate::ast::*;
use crate::error;
use crate::token::{Token, TokenKind};
use std::rc::Rc;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    last_arrow_params: Option<Vec<Rc<str>>>,
    /// Parameter defaults collected by the most recent `parse_params` / arrow parse.
    cur_param_defaults: Vec<Option<Expr>>,
    /// Rest parameter name from the most recent `parse_params` / arrow parse.
    cur_rest_param: Option<Rc<str>>,
    /// Arrow-specific defaults/rest (carried alongside `last_arrow_params`).
    arrow_defaults: Vec<Option<Expr>>,
    arrow_rest: Option<Rc<str>>,
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
            arrow_defaults: Vec::new(),
            arrow_rest: None,
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
                let label = Rc::from(s.as_str());
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

    fn parse_opt_label(&mut self) -> Option<Rc<str>> {
        if let TokenKind::Ident(s) = self.peek().clone() {
            self.advance();
            Some(Rc::from(s.as_str()))
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
            TokenKind::Ident(s) => Some(Rc::from(s.as_str())),
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
        let body = self.parse_fn_body()?;
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

    fn parse_params(&mut self) -> error::Result<Vec<Rc<str>>> {
        self.expect(&TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            if self.check(&TokenKind::Spread) {
                // rest parameter: ...name (must be last)
                self.advance();
                if let TokenKind::Ident(s) = self.advance() {
                    self.cur_rest_param = Some(Rc::from(s.as_str()));
                } else {
                    return Err(error::Error::syntax(
                        "Expected rest parameter name".to_string(),
                    ));
                }
                break;
            }
            match self.advance() {
                TokenKind::Ident(s) => {
                    params.push(Rc::from(s.as_str()));
                    let default = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    self.cur_param_defaults.push(default);
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
                TokenKind::Ident(s) => Rc::from(s.as_str()),
                other => {
                    return Err(error::Error::syntax(format!(
                        "Expected identifier, got {:?}",
                        other
                    )))
                }
            };
            let init = if self.eat(&TokenKind::Assign) {
                Some(self.parse_assign()?)
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
                    catch_param = Some(Rc::from(s.as_str()));
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
        let right = self.parse_assign()?;
        Ok(Expr::Assign(op, Box::new(left), Box::new(right)))
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
            match self.peek() {
                TokenKind::Dot => {
                    self.advance();
                    let name = self.read_property_name()?;
                    let prop = Expr::String(Rc::from(name.as_str()));
                    e = Expr::Member {
                        object: Box::new(e),
                        property: Box::new(prop),
                        computed: false,
                        optional: false,
                    };
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
                            let prop = Expr::String(Rc::from(name.as_str()));
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
                // `async function ...` expression; otherwise `async` is treated
                // as a plain identifier (handled below).
                if matches!(self.peek_at_tok(1).kind, TokenKind::Function) {
                    self.advance(); // async
                    let mut f = self.parse_function_expr()?;
                    if let Expr::Function(fe) = &mut f {
                        fe.is_async = true;
                    }
                    return Ok(f);
                }
                // fall through to identifier
                self.advance();
                Ok(Expr::Ident(Rc::from("async")))
            }
            TokenKind::Regex(pat, flags) => {
                self.advance();
                Ok(Expr::Regex(
                    Rc::from(pat.as_str()),
                    Rc::from(flags.as_str()),
                ))
            }
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr::String(Rc::from(s.as_str())))
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
                    return self.parse_arrow_body(vec![Rc::from(s.as_str())]);
                }
                self.advance();
                Ok(Expr::Ident(Rc::from(s.as_str())))
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
            TokenKind::TemplateString(s) => {
                self.advance();
                self.parse_template_rest(Rc::from(s.as_str()))
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
    fn parse_template_rest(&mut self, first: Rc<str>) -> error::Result<Expr> {
        if !self.check(&TokenKind::TemplateExprStart) {
            // No interpolation: plain string.
            return Ok(Expr::String(first));
        }
        let mut quasis: Vec<Rc<str>> = vec![first];
        let mut exprs: Vec<Expr> = Vec::new();
        loop {
            self.expect(&TokenKind::TemplateExprStart, "${")?;
            let e = self.parse_expr()?;
            self.expect(&TokenKind::TemplateExprEnd, "}")?;
            exprs.push(e);
            // next quasi
            match self.advance() {
                TokenKind::TemplateString(s) => quasis.push(Rc::from(s.as_str())),
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
            let (key, computed) = match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.advance();
                    (PropertyKey::Ident(Rc::from(s.as_str())), false)
                }
                TokenKind::String(s) => {
                    self.advance();
                    (PropertyKey::String(Rc::from(s.as_str())), false)
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
                let body = self.parse_fn_body()?;
                let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
                props.push(Property {
                    key,
                    value: Expr::Function(FunctionExpr {
                        name: None,
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
                let body = self.parse_fn_body()?;
                let is_strict = self.is_strict_context || Self::scan_directive_prologue(&body);
                props.push(Property {
                    key,
                    value: Expr::Function(FunctionExpr {
                        name: None,
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
                    method: true,
                    shorthand: false,
                    kind: PropKind::Method,
                });
            } else if !self.check(&TokenKind::Colon) && !computed {
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
                let value = self.parse_assign()?;
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
                Some(Rc::from(s.as_str()))
            }
            _ => None,
        };
        let params = self.parse_params()?;
        let param_defaults = std::mem::take(&mut self.cur_param_defaults);
        let rest_param = self.cur_rest_param.take();
        let body = self.parse_fn_body()?;
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
            let prop = Expr::String(Rc::from(name.as_str()));
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
        let mut rest: Option<Rc<str>> = None;
        // empty params: () =>
        if self.check(&TokenKind::RParen) {
            self.advance();
            if self.check(&TokenKind::Arrow) {
                self.last_arrow_params = Some(params);
                self.arrow_defaults = defaults;
                self.arrow_rest = rest;
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        loop {
            if self.check(&TokenKind::Spread) {
                self.advance();
                if let TokenKind::Ident(s) = self.advance() {
                    rest = Some(Rc::from(s.as_str()));
                } else {
                    self.pos = save;
                    return Ok(false);
                }
                break;
            }
            match self.peek().clone() {
                TokenKind::Ident(s) => {
                    self.advance();
                    params.push(Rc::from(s.as_str()));
                    let d = if self.eat(&TokenKind::Assign) {
                        Some(self.parse_assign()?)
                    } else {
                        None
                    };
                    defaults.push(d);
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
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        self.pos = save;
        Ok(false)
    }

    fn parse_arrow_body(&mut self, params: Vec<Rc<str>>) -> error::Result<Expr> {
        let param_defaults = std::mem::take(&mut self.arrow_defaults);
        let rest_param = self.arrow_rest.take();
        // arrow body: expression or block
        if self.check(&TokenKind::LBrace) {
            let body = self.parse_fn_body()?;
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
            Ok(Expr::Arrow(FunctionExpr {
                name: None,
                params,
                param_defaults,
                rest_param,
                body: vec![self.stmt(StmtNode::Return(Some(e)))],
                is_arrow: true,
                is_async: false,
                is_generator: false,
                param_decls: Vec::new(),
                // Arrow with expression body has no directive prologue; inherit.
                is_strict: self.is_strict_context,
            }))
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
                Some(Rc::from(s.as_str()))
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
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let is_static = self.eat(&TokenKind::Static);
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
                Rc::from("constructor")
            } else {
                Rc::from(self.read_property_name()?.as_str())
            };
            let params = self.parse_params()?;
            let param_defaults = std::mem::take(&mut self.cur_param_defaults);
            let rest_param = self.cur_rest_param.take();
            let body = self.parse_fn_body()?;
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
            });
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(ClassExpr {
            name,
            superclass,
            methods,
        })
    }
    fn parse_async_or_expr_stmt(&mut self) -> error::Result<Stmt> {
        let e = self.parse_expr()?;
        self.expect_semi()?;
        Ok(self.stmt(StmtNode::ExprStmt(e)))
    }
    fn parse_pattern(&mut self) -> error::Result<Pattern> {
        if let TokenKind::Ident(s) = self.peek().clone() {
            self.advance();
            Ok(Pattern::Ident(Rc::from(s.as_str())))
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
                        // hole: represent as Ident("_hole") is messy; use a default-only pattern skip.
                        // For simplicity, push a hole as a Rest-less placeholder pattern.
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
                            PropertyKey::Ident(Rc::from(s.as_str()))
                        }
                        TokenKind::String(s) => {
                            self.advance();
                            PropertyKey::String(Rc::from(s.as_str()))
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
                Ok(Pattern::Ident(Rc::from(s.as_str())))
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
