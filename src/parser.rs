use crate::ast::*;
use crate::error;
use crate::token::{Token, TokenKind};
use std::rc::Rc;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    last_arrow_params: Option<Vec<Rc<str>>>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0, last_arrow_params: None }
    }

    pub fn parse(src: &str) -> error::Result<Program> {
        let mut lx = crate::lexer::Lexer::new(src);
        let tokens = lx.tokens();
        let mut p = Parser::new(tokens);
        p.parse_program()
    }

    fn peek(&self) -> &TokenKind { &self.tokens[self.pos].kind }
    fn peek_at_tok(&self, off: usize) -> &Token { &self.tokens[(self.pos + off).min(self.tokens.len() - 1)] }
    fn at_newline_before(&self) -> bool { self.tokens[self.pos].preceded_by_newline }

    fn advance(&mut self) -> TokenKind {
        let k = self.tokens[self.pos].kind.clone();
        if self.pos < self.tokens.len() - 1 { self.pos += 1; }
        k
    }

    fn check(&self, k: &TokenKind) -> bool {
        std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(k)
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.check(k) { self.advance(); true } else { false }
    }

    fn expect(&mut self, k: &TokenKind, what: &str) -> error::Result<()> {
        if self.check(k) { self.advance(); Ok(()) }
        else { Err(error::Error::syntax(format!("Expected {}, got {:?}", what, self.peek()))) }
    }

    fn expect_semi(&mut self) -> error::Result<()> {
        // ASI: semicolon optional before } or EOF or after newline
        if self.check(&TokenKind::Semicolon) { self.advance(); return Ok(()); }
        if self.check(&TokenKind::RBrace) || self.check(&TokenKind::Eof) { return Ok(()); }
        if self.at_newline_before() { return Ok(()); }
        Err(error::Error::syntax(format!("Expected ; got {:?}", self.peek())))
    }

    fn parse_program(&mut self) -> error::Result<Program> {
        let mut body = Vec::new();
        while !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        Ok(Program { body })
    }

    fn parse_stmt(&mut self) -> error::Result<Stmt> {
        match self.peek().clone() {
            TokenKind::LBrace => self.parse_block(),
            TokenKind::Var | TokenKind::Let | TokenKind::Const => self.parse_var_decl(),
            TokenKind::Function => self.parse_function_decl(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => { self.advance(); let l = self.parse_opt_label(); self.expect_semi()?; Ok(Stmt::Break(l)) }
            TokenKind::Continue => { self.advance(); let l = self.parse_opt_label(); self.expect_semi()?; Ok(Stmt::Continue(l)) }
            TokenKind::Throw => { self.advance(); let e = self.parse_expr()?; self.expect_semi()?; Ok(Stmt::Throw(e)) }
            TokenKind::Try => self.parse_try(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Semicolon => { self.advance(); Ok(Stmt::Empty) }
            _ => {
                let e = self.parse_expr()?;
                self.expect_semi()?;
                Ok(Stmt::ExprStmt(e))
            }
        }
    }

    fn parse_opt_label(&mut self) -> Option<Rc<str>> {
        if let TokenKind::Ident(s) = self.peek().clone() {
            self.advance();
            Some(Rc::from(s.as_str()))
        } else { None }
    }

    fn parse_block(&mut self) -> error::Result<Stmt> {
        self.expect(&TokenKind::LBrace, "{")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(Stmt::Block(body))
    }

    fn parse_var_decl(&mut self) -> error::Result<Stmt> {
        let kind = match self.advance() {
            TokenKind::Var => VarKind::Var,
            TokenKind::Let => VarKind::Let,
            TokenKind::Const => VarKind::Const,
            _ => unreachable!(),
        };
        let mut decls = Vec::new();
        loop {
            let name = match self.advance() {
                TokenKind::Ident(s) => Rc::from(s.as_str()),
                other => return Err(error::Error::syntax(format!("Expected identifier in decl, got {:?}", other))),
            };
            let init = if self.eat(&TokenKind::Assign) {
                Some(self.parse_assign()?)
            } else { None };
            decls.push((name, init));
            if !self.eat(&TokenKind::Comma) { break; }
        }
        self.expect_semi()?;
        Ok(Stmt::VarDecl { kind, decls })
    }

    fn parse_function_decl(&mut self) -> error::Result<Stmt> {
        self.advance(); // function
        let name = match self.advance() {
            TokenKind::Ident(s) => Some(Rc::from(s.as_str())),
            other => return Err(error::Error::syntax(format!("Expected function name, got {:?}", other))),
        };
        let params = self.parse_params()?;
        let body = self.parse_fn_body()?;
        Ok(Stmt::FunctionDecl(FunctionExpr { name, params, body, is_arrow: false }))
    }

    fn parse_params(&mut self) -> error::Result<Vec<Rc<str>>> {
        self.expect(&TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) {
            if let TokenKind::Ident(s) = self.advance() {
                params.push(Rc::from(s.as_str()));
            } else {
                return Err(error::Error::syntax("Expected parameter name".to_string()));
            }
            if !self.eat(&TokenKind::Comma) { break; }
        }
        self.expect(&TokenKind::RParen, ")")?;
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
        } else { None };
        Ok(Stmt::If { cond, then, else_ })
    }

    fn parse_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { cond, body })
    }

    fn parse_do_while(&mut self) -> error::Result<Stmt> {
        self.advance();
        let body = Box::new(self.parse_stmt()?);
        self.expect(&TokenKind::While, "while")?;
        self.expect(&TokenKind::LParen, "(")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, ")")?;
        self.eat(&TokenKind::Semicolon);
        Ok(Stmt::DoWhile { body, cond })
    }

    fn parse_for(&mut self) -> error::Result<Stmt> {
        self.advance();
        self.expect(&TokenKind::LParen, "(")?;
        // init
        let init: Option<Box<Stmt>> = if self.check(&TokenKind::Semicolon) {
            self.advance(); None
        } else if matches!(self.peek(), TokenKind::Var | TokenKind::Let | TokenKind::Const) {
            // could be for-in / for-of
            let stmt = self.parse_var_decl_no_semi()?;
            if self.check(&TokenKind::In) {
                self.advance();
                let right = self.parse_expr()?;
                self.expect(&TokenKind::RParen, ")")?;
                let body = Box::new(self.parse_stmt()?);
                return Ok(Stmt::ForIn { left: Box::new(stmt), right, body });
            }
            if self.check(&TokenKind::Of) {
                self.advance();
                let right = self.parse_assign()?;
                self.expect(&TokenKind::RParen, ")")?;
                let body = Box::new(self.parse_stmt()?);
                return Ok(Stmt::ForOf { left: Box::new(stmt), right, body });
            }
            Some(Box::new(stmt))
        } else {
            let e = self.parse_expr()?;
            Some(Box::new(Stmt::ExprStmt(e)))
        };
        self.expect(&TokenKind::Semicolon, ";")?;
        let cond = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expr()?) };
        self.expect(&TokenKind::Semicolon, ";")?;
        let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expr()?) };
        self.expect(&TokenKind::RParen, ")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For { init, cond, update, body })
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
            let name = match self.advance() {
                TokenKind::Ident(s) => Rc::from(s.as_str()),
                other => return Err(error::Error::syntax(format!("Expected identifier, got {:?}", other))),
            };
            let init = if self.eat(&TokenKind::Assign) { Some(self.parse_assign()?) } else { None };
            decls.push((name, init));
            if !self.eat(&TokenKind::Comma) { break; }
        }
        Ok(Stmt::VarDecl { kind, decls })
    }

    fn parse_return(&mut self) -> error::Result<Stmt> {
        self.advance();
        if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RBrace) || self.check(&TokenKind::Eof) || self.at_newline_before() {
            self.eat(&TokenKind::Semicolon);
            return Ok(Stmt::Return(None));
        }
        let e = self.parse_expr()?;
        self.expect_semi()?;
        Ok(Stmt::Return(Some(e)))
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
        let catch_body = match catch_body {
            Some(cb) => cb,
            None => Box::new(Stmt::Block(vec![])),
        };
        Ok(Stmt::TryCatch { try_body, catch_param, catch_body, finally_body })
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
            while !self.check(&TokenKind::Case) && !self.check(&TokenKind::Default) && !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
                body.push(self.parse_stmt()?);
            }
            cases.push(SwitchCase { test, body });
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(Stmt::Switch { disc, cases })
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

    fn parse_assign(&mut self) -> error::Result<Expr> {
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
            Ok(Expr::Conditional(Box::new(cond), Box::new(then), Box::new(else_)))
        } else { Ok(cond) }
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
        // prefix ++/--
        if matches!(self.peek(), TokenKind::Inc | TokenKind::Dec) {
            let op = if matches!(self.peek(), TokenKind::Inc) { UpdateOp::Inc } else { UpdateOp::Dec };
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::Update(op, true, Box::new(e)));
        }
        let op = match self.peek() {
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Plus => Some(UnOp::Neg), // will coerce to number; reuse for now
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
            let op = if matches!(self.peek(), TokenKind::Inc) { UpdateOp::Inc } else { UpdateOp::Dec };
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
                    let prop = match self.advance() {
                        TokenKind::Ident(s) => Expr::String(Rc::from(s.as_str())),
                        other => return Err(error::Error::syntax(format!("Expected property name after ., got {:?}", other))),
                    };
                    e = Expr::Member { object: Box::new(e), property: Box::new(prop), computed: false };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let prop = self.parse_expr()?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    e = Expr::Member { object: Box::new(e), property: Box::new(prop), computed: true };
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&TokenKind::RParen, ")")?;
                    e = Expr::Call { callee: Box::new(e), args };
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
            if !self.eat(&TokenKind::Comma) { break; }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> error::Result<Expr> {
        match self.peek().clone() {
            TokenKind::Number(n) => { self.advance(); Ok(Expr::Number(n)) }
            TokenKind::String(s) => { self.advance(); Ok(Expr::String(Rc::from(s.as_str()))) }
            TokenKind::True => { self.advance(); Ok(Expr::Bool(true)) }
            TokenKind::False => { self.advance(); Ok(Expr::Bool(false)) }
            TokenKind::Null => { self.advance(); Ok(Expr::Null) }
            TokenKind::Undefined => { self.advance(); Ok(Expr::Undefined) }
            TokenKind::This => { self.advance(); Ok(Expr::This) }
            TokenKind::Ident(s) => {
                // Could be arrow: x => ...
                if let TokenKind::Arrow = self.peek_at_tok(1).kind {
                    self.advance(); // ident
                    self.advance(); // =>
                    return self.parse_arrow_body(vec![Rc::from(s.as_str())]);
                }
                self.advance(); Ok(Expr::Ident(Rc::from(s.as_str())))
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
                // For now treat template as plain string (no interpolation support in parser yet)
                Ok(Expr::String(Rc::from(s.as_str())))
            }
            other => Err(error::Error::syntax(format!("Unexpected token in expression: {:?}", other))),
        }
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
            if !self.eat(&TokenKind::Comma) { break; }
        }
        self.expect(&TokenKind::RBracket, "]")?;
        Ok(Expr::Array(elements))
    }

    fn parse_object(&mut self) -> error::Result<Expr> {
        self.advance(); // {
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let (key, computed) = match self.peek().clone() {
                TokenKind::Ident(s) => { self.advance(); (PropertyKey::Ident(Rc::from(s.as_str())), false) }
                TokenKind::String(s) => { self.advance(); (PropertyKey::String(Rc::from(s.as_str())), false) }
                TokenKind::Number(n) => { self.advance(); (PropertyKey::Number(n), false) }
                TokenKind::LBracket => {
                    self.advance();
                    let e = self.parse_assign()?;
                    self.expect(&TokenKind::RBracket, "]")?;
                    // computed key - store as string for now via a special marker
                    // We'll use PropertyKey::String with a sentinel; better: extend PropertyKey. For now use Ident of expr text - not possible.
                    // Simplest: only support computed keys that are identifiers.
                    let key = if let Expr::Ident(name) = e {
                        PropertyKey::Ident(name)
                    } else if let Expr::String(s) = e {
                        PropertyKey::String(s)
                    } else {
                        return Err(error::Error::syntax("Complex computed property keys not supported yet".to_string()));
                    };
                    (key, true)
                }
                other => return Err(error::Error::syntax(format!("Expected property key, got {:?}", other))),
            };
            // method shorthand or value
            if self.check(&TokenKind::LParen) {
                let params = self.parse_params()?;
                let body = self.parse_fn_body()?;
                props.push(Property { key, value: Expr::Function(FunctionExpr { name: None, params, body, is_arrow: false }), computed, method: true, shorthand: false });
            } else {
                self.expect(&TokenKind::Colon, ":")?;
                let value = self.parse_assign()?;
                props.push(Property { key, value, computed, method: false, shorthand: false });
            }
            if !self.eat(&TokenKind::Comma) { break; }
        }
        self.expect(&TokenKind::RBrace, "}")?;
        Ok(Expr::Object(props))
    }

    fn parse_function_expr(&mut self) -> error::Result<Expr> {
        self.advance(); // function
        let name = match self.peek().clone() {
            TokenKind::Ident(s) => { self.advance(); Some(Rc::from(s.as_str())) }
            _ => None,
        };
        let params = self.parse_params()?;
        let body = self.parse_fn_body()?;
        Ok(Expr::Function(FunctionExpr { name, params, body, is_arrow: false }))
    }

    fn parse_new(&mut self) -> error::Result<Expr> {
        self.advance(); // new
        let callee = self.parse_call()?;
        // callee already consumed the call parens; distinguish constructor call.
        if let Expr::Call { callee: c, args } = callee {
            Ok(Expr::New { callee: c, args })
        } else {
            Ok(Expr::New { callee: Box::new(callee), args: Vec::new() })
        }
    }

    /// After consuming `(`, try to parse arrow params followed by `) =>`.
    /// Returns true and sets `last_arrow_params` if it looks like an arrow function.
    fn try_parse_arrow_params(&mut self) -> error::Result<bool> {
        let save = self.pos;
        let mut params = Vec::new();
        // empty params: () =>
        if self.check(&TokenKind::RParen) {
            self.advance();
            if self.check(&TokenKind::Arrow) {
                self.last_arrow_params = Some(params);
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        loop {
            match self.peek().clone() {
                TokenKind::Ident(s) => { self.advance(); params.push(Rc::from(s.as_str())); }
                TokenKind::Spread => {
                    self.advance();
                    if let TokenKind::Ident(s) = self.advance() {
                        let mut name = String::from("...");
                        name.push_str(&s);
                        params.push(Rc::from(name.as_str()));
                    }
                }
                _ => { self.pos = save; return Ok(false); }
            }
            if self.check(&TokenKind::Comma) { self.advance(); continue; }
            break;
        }
        if self.check(&TokenKind::RParen) {
            self.advance();
            if self.check(&TokenKind::Arrow) {
                self.last_arrow_params = Some(params);
                return Ok(true);
            }
            self.pos = save;
            return Ok(false);
        }
        self.pos = save;
        Ok(false)
    }

    fn parse_arrow_body(&mut self, params: Vec<Rc<str>>) -> error::Result<Expr> {
        // arrow body: expression or block
        if self.check(&TokenKind::LBrace) {
            let body = self.parse_fn_body()?;
            Ok(Expr::Arrow(FunctionExpr { name: None, params, body, is_arrow: true }))
        } else {
            let e = self.parse_assign()?;
            Ok(Expr::Arrow(FunctionExpr { name: None, params, body: vec![Stmt::Return(Some(e))], is_arrow: true }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn parse(src: &str) -> Program {
        Parser::parse(src).unwrap()
    }

    #[test]
    fn parse_number_expr() {
        let p = parse("42;");
        assert_eq!(p.body.len(), 1);
        match &p.body[0] {
            Stmt::ExprStmt(Expr::Number(n)) => assert_eq!(*n, 42.0),
            other => panic!("expected number expr, got {:?}", other),
        }
    }

    #[test]
    fn parse_var_decl() {
        let p = parse("let x = 1 + 2;");
        assert_eq!(p.body.len(), 1);
        match &p.body[0] {
            Stmt::VarDecl { kind, decls } => {
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
        match &p.body[0] {
            Stmt::FunctionDecl(f) => {
                assert_eq!(f.name.as_ref().map(|s| s.as_ref()), Some("add"));
                assert_eq!(f.params.len(), 2);
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_arrow_in_obj() {
        let p = parse("let o = { x: 1, y: 2 };");
        match &p.body[0] {
            Stmt::VarDecl { decls, .. } => match &decls[0].1 {
                Some(Expr::Object(props)) => assert_eq!(props.len(), 2),
                other => panic!("{:?}", other),
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_precedence() {
        // 1 + 2 * 3 should be Add(1, Mul(2,3))
        let p = parse("1 + 2 * 3;");
        match &p.body[0] {
            Stmt::ExprStmt(Expr::Binary(BinOp::Add, _, right)) => {
                match right.as_ref() {
                    Expr::Binary(BinOp::Mul, _, _) => {}
                    other => panic!("expected mul on right, got {:?}", other),
                }
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_inc_dec() {
        let p = parse("++x; y--;");
        match &p.body[0] {
            Stmt::ExprStmt(Expr::Update(UpdateOp::Inc, true, _)) => {}
            other => panic!("{:?}", other),
        }
        match &p.body[1] {
            Stmt::ExprStmt(Expr::Update(UpdateOp::Dec, false, _)) => {}
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn parse_for_loop() {
        let p = parse("for (let i = 0; i < 10; i++) { sum += i; }");
        assert!(matches!(&p.body[0], Stmt::For { .. }));
    }

    #[test]
    fn parse_try_catch() {
        let p = parse("try { f(); } catch (e) { g(); } finally { h(); }");
        assert!(matches!(&p.body[0], Stmt::TryCatch { .. }));
    }
}
