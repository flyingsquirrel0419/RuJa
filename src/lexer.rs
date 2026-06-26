use crate::token::{Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    saw_newline: bool,
    pub pending_template: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0, line: 1, col: 1, saw_newline: true, pending_template: false }
    }

    fn peek(&self) -> Option<u8> { self.src.get(self.pos).copied() }
    fn peek_at(&self, off: usize) -> Option<u8> { self.src.get(self.pos + off).copied() }

    fn advance(&mut self) -> Option<u8> {
        let c = self.src.get(self.pos).copied();
        if let Some(b) = c {
            self.pos += 1;
            if b == b'\n' { self.line += 1; self.col = 1; self.saw_newline = true; }
            else { self.col += 1; }
        }
        c
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') => { self.advance(); }
                Some(b'\n') => { self.advance(); }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' { break; }
                        self.advance();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.advance(); self.advance();
                    while let Some(c) = self.peek() {
                        if c == b'*' && self.peek_at(1) == Some(b'/') {
                            self.advance(); self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn read_number(&mut self) -> TokenKind {
        let start = self.pos;
        if self.peek() == Some(b'0') && (self.peek_at(1) == Some(b'x') || self.peek_at(1) == Some(b'X')) {
            self.advance(); self.advance();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() { self.advance(); } else { break; }
            }
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
            return TokenKind::Number(i64::from_str_radix(&s[2..], 16).unwrap_or(0) as f64);
        }
        if self.peek() == Some(b'0') && (self.peek_at(1) == Some(b'o') || self.peek_at(1) == Some(b'O')) {
            self.advance(); self.advance();
            while let Some(c) = self.peek() {
                if (b'0'..=b'7').contains(&c) { self.advance(); } else { break; }
            }
            let s = std::str::from_utf8(&self.src[start+2..self.pos]).unwrap_or("0");
            return TokenKind::Number(i64::from_str_radix(s, 8).unwrap_or(0) as f64);
        }
        if self.peek() == Some(b'0') && (self.peek_at(1) == Some(b'b') || self.peek_at(1) == Some(b'B')) {
            self.advance(); self.advance();
            while let Some(c) = self.peek() {
                if c == b'0' || c == b'1' { self.advance(); } else { break; }
            }
            let s = std::str::from_utf8(&self.src[start+2..self.pos]).unwrap_or("0");
            return TokenKind::Number(i64::from_str_radix(s, 2).unwrap_or(0) as f64);
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E'
                || (c == b'+' || c == b'-') && (self.src.get(self.pos.wrapping_sub(1)) == Some(&b'e') || self.src.get(self.pos.wrapping_sub(1)) == Some(&b'E')) {
                self.advance();
            } else { break; }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        TokenKind::Number(s.parse::<f64>().unwrap_or(f64::NAN))
    }

    fn read_string(&mut self, quote: u8) -> TokenKind {
        self.advance(); // opening quote
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == quote { self.advance(); break; }
            if c == b'\\' {
                self.advance();
                match self.advance() {
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(b'r') => s.push('\r'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'\'') => s.push('\''),
                    Some(b'"') => s.push('"'),
                    Some(b'`') => s.push('`'),
                    Some(b'0') => s.push('\0'),
                    Some(b'b') => s.push('\u{0008}'),
                    Some(b'f') => s.push('\u{000C}'),
                    Some(b'v') => s.push('\u{000B}'),
                    Some(b'x') => {
                        let h1 = self.advance().unwrap_or(b'0');
                        let h2 = self.advance().unwrap_or(b'0');
                        let hex_bytes = [h1, h2];
                        let hex = std::str::from_utf8(&hex_bytes).unwrap_or("00");
                        if let Ok(n) = u32::from_str_radix(hex, 16) {
                            s.push(char::from_u32(n).unwrap_or(' '));
                        }
                    }
                    Some(b'u') => {
                        if self.peek() == Some(b'{') {
                            self.advance();
                            let mut hex = String::new();
                            while let Some(c) = self.peek() {
                                if c == b'}' { self.advance(); break; }
                                hex.push(c as char);
                                self.advance();
                            }
                            if let Ok(n) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(n) { s.push(ch); }
                            }
                        } else {
                            let h: String = (0..4).filter_map(|_| self.advance().map(|c| c as char)).collect();
                            if let Ok(n) = u32::from_str_radix(&h, 16) {
                                if let Some(ch) = char::from_u32(n) { s.push(ch); }
                            }
                        }
                    }
                    Some(c) => s.push(c as char),
                    None => break,
                }
            } else {
                self.advance();
                s.push(c as char);
            }
        }
        TokenKind::String(s)
    }

    fn read_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                self.advance();
            } else { break; }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
        match s {
            "var" => TokenKind::Var,
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "function" => TokenKind::Function,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "do" => TokenKind::Do,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "null" => TokenKind::Null,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "undefined" => TokenKind::Undefined,
            "new" => TokenKind::New,
            "this" => TokenKind::This,
            "typeof" => TokenKind::Typeof,
            "instanceof" => TokenKind::Instanceof,
            "in" => TokenKind::In,
            "of" => TokenKind::Of,
            "delete" => TokenKind::Delete,
            "void" => TokenKind::Void,
            "throw" => TokenKind::Throw,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            _ => TokenKind::Ident(s.to_string()),
        }
    }

    fn read_operator(&mut self) -> Option<TokenKind> {
        let c = self.peek()?;
        match c {
            b'+' => {
                self.advance();
                if self.peek() == Some(b'+') { self.advance(); return Some(TokenKind::Inc); }
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::PlusAssign); }
                return Some(TokenKind::Plus);
            }
            b'-' => {
                self.advance();
                if self.peek() == Some(b'-') { self.advance(); return Some(TokenKind::Dec); }
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::MinusAssign); }
                return Some(TokenKind::Minus);
            }
            b'*' => {
                self.advance();
                if self.peek() == Some(b'*') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::StarStarAssign); }
                    return Some(TokenKind::StarStar);
                }
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::StarAssign); }
                return Some(TokenKind::Star);
            }
            b'/' => {
                self.advance();
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::SlashAssign); }
                return Some(TokenKind::Slash);
            }
            b'%' => {
                self.advance();
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::PercentAssign); }
                return Some(TokenKind::Percent);
            }
            b'=' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::EqEqEq); }
                    return Some(TokenKind::Eq);
                }
                if self.peek() == Some(b'>') { self.advance(); return Some(TokenKind::Arrow); }
                return Some(TokenKind::Assign);
            }
            b'!' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::NotEqEqEq); }
                    return Some(TokenKind::NotEq);
                }
                return Some(TokenKind::Not);
            }
            b'<' => {
                self.advance();
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::Lte); }
                if self.peek() == Some(b'<') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::ShlAssign); }
                    return Some(TokenKind::Shl);
                }
                return Some(TokenKind::Lt);
            }
            b'>' => {
                self.advance();
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::Gte); }
                if self.peek() == Some(b'>') {
                    self.advance();
                    if self.peek() == Some(b'>') {
                        self.advance();
                        if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::UshrAssign); }
                        return Some(TokenKind::Ushr);
                    }
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::ShrAssign); }
                    return Some(TokenKind::Shr);
                }
                return Some(TokenKind::Gt);
            }
            b'&' => {
                self.advance();
                if self.peek() == Some(b'&') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::AndAssign); }
                    return Some(TokenKind::And);
                }
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::AmpAssign); }
                return Some(TokenKind::BitAnd);
            }
            b'|' => {
                self.advance();
                if self.peek() == Some(b'|') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::OrAssign); }
                    return Some(TokenKind::Or);
                }
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::PipeAssign); }
                return Some(TokenKind::BitOr);
            }
            b'^' => {
                self.advance();
                if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::CaretAssign); }
                return Some(TokenKind::BitXor);
            }
            b'~' => { self.advance(); return Some(TokenKind::BitNot); }
            b'?' => {
                self.advance();
                if self.peek() == Some(b'?') {
                    self.advance();
                    if self.peek() == Some(b'=') { self.advance(); return Some(TokenKind::NullishAssign); }
                    return Some(TokenKind::Nullish);
                }
                return Some(TokenKind::Question);
            }
            b'.' => {
                self.advance();
                if self.peek() == Some(b'.') && self.peek_at(1) == Some(b'.') {
                    self.advance(); self.advance();
                    return Some(TokenKind::Spread);
                }
                return Some(TokenKind::Dot);
            }
            b':' => { self.advance(); return Some(TokenKind::Colon); }
            b',' => { self.advance(); return Some(TokenKind::Comma); }
            b';' => { self.advance(); return Some(TokenKind::Semicolon); }
            b'(' => { self.advance(); return Some(TokenKind::LParen); }
            b')' => { self.advance(); return Some(TokenKind::RParen); }
            b'{' => { self.advance(); return Some(TokenKind::LBrace); }
            b'}' => { self.advance(); return Some(TokenKind::RBrace); }
            b'[' => { self.advance(); return Some(TokenKind::LBracket); }
            b']' => { self.advance(); return Some(TokenKind::RBracket); }
            _ => None,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_ws_and_comments();
        let line = self.line;
        let col = self.col;
        let preceded_by_newline = self.saw_newline;
        self.saw_newline = false;

        let kind = match self.peek() {
            None => TokenKind::Eof,
            Some(c) if c.is_ascii_digit() => self.read_number(),
            Some(c) if c == b'.' && self.peek_at(1).map(|d| d.is_ascii_digit()).unwrap_or(false) => self.read_number(),
            Some(b'"') => self.read_string(b'"'),
            Some(b'\'') => self.read_string(b'\''),
            Some(b'`') => return self.read_template_start(line, col, preceded_by_newline),
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => self.read_ident_or_keyword(),
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => { return self.next_token(); }
            _ => {
                if let Some(k) = self.read_operator() {
                    k
                } else {
                    self.advance();
                    TokenKind::Ident(format!("Unexpected char '{}'", self.src.get(self.pos-1).copied().unwrap_or(b'?') as char))
                }
            }
        };

        let mut tok = Token::new(kind, line, col);
        tok.preceded_by_newline = preceded_by_newline;
        tok
    }

    fn read_template_start(&mut self, line: usize, col: usize, preceded_by_newline: bool) -> Token {
        self.advance(); // consume backtick
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == b'`' { self.advance(); break; }
            if c == b'$' && self.peek_at(1) == Some(b'{') {
                self.advance(); self.advance();
                self.pending_template = true;
                let mut tok = Token::new(TokenKind::TemplateString(s), line, col);
                tok.preceded_by_newline = preceded_by_newline;
                return tok;
            }
            if c == b'\\' {
                self.advance();
                match self.advance() {
                    Some(b'n') => s.push('\n'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'`') => s.push('`'),
                    Some(b'$') => s.push('$'),
                    Some(c) => s.push(c as char),
                    None => break,
                }
            } else {
                self.advance();
                s.push(c as char);
            }
        }
        let mut tok = Token::new(TokenKind::TemplateString(s), line, col);
        tok.preceded_by_newline = preceded_by_newline;
        tok
    }

    pub fn tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token();
            let is_eof = t.kind == TokenKind::Eof;
            out.push(t);
            if is_eof { break; }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src).tokens().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(kinds("42"), vec![Number(42.0), Eof]);
        assert_eq!(kinds("3.14"), vec![Number(3.14), Eof]);
        assert_eq!(kinds("0xff"), vec![Number(255.0), Eof]);
        assert_eq!(kinds("0b101"), vec![Number(5.0), Eof]);
        assert_eq!(kinds("0o17"), vec![Number(15.0), Eof]);
    }

    #[test]
    fn strings() {
        assert_eq!(kinds("\"hi\""), vec![String("hi".into()), Eof]);
        assert_eq!(kinds("'a\\nb'"), vec![String("a\nb".into()), Eof]);
    }

    #[test]
    fn keywords() {
        assert_eq!(kinds("var let const"), vec![Var, Let, Const, Eof]);
        assert_eq!(kinds("function return"), vec![Function, Return, Eof]);
    }

    #[test]
    fn operators() {
        assert_eq!(kinds("=>"), vec![Arrow, Eof]);
        assert_eq!(kinds("==="), vec![EqEqEq, Eof]);
        assert_eq!(kinds("!=="), vec![NotEqEqEq, Eof]);
        assert_eq!(kinds("**"), vec![StarStar, Eof]);
        assert_eq!(kinds("..."), vec![Spread, Eof]);
        assert_eq!(kinds("++"), vec![Inc, Eof]);
        assert_eq!(kinds("--"), vec![Dec, Eof]);
    }

    #[test]
    fn comments() {
        assert_eq!(kinds("1 // hi\n2"), vec![Number(1.0), Number(2.0), Eof]);
        assert_eq!(kinds("1 /* x */ 2"), vec![Number(1.0), Number(2.0), Eof]);
    }
}
