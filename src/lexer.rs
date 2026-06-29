use crate::token::{Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    saw_newline: bool,
    /// Whether the previous significant token ended an expression (so a `/`
    /// means division rather than a regex literal).
    prev_value_ending: bool,
    /// Template-literal scanner state.
    /// 0 = normal, 1 = emit TemplateExprStart next, 2 = inside interpolation expr,
    /// 3 = read next segment after an interpolation closed.
    pub template_state: u8,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            saw_newline: true,
            prev_value_ending: false,
            template_state: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn peek_at(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let c = self.src.get(self.pos).copied();
        if let Some(b) = c {
            self.pos += 1;
            if b == b'\n' {
                self.line += 1;
                self.col = 1;
                self.saw_newline = true;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') => {
                    self.advance();
                }
                Some(0x0b) | Some(0x0c) => {
                    // vertical tab and form feed are whitespace per ES.
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.advance();
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c == b'*' && self.peek_at(1) == Some(b'/') {
                            self.advance();
                            self.advance();
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
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'x') || self.peek_at(1) == Some(b'X'))
        {
            self.advance();
            self.advance();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() || c == b'_' {
                    self.advance();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
            let s: String = s.chars().filter(|&c| c != '_').collect();
            let v = i64::from_str_radix(&s[2..], 16).unwrap_or(0);
            if self.peek() == Some(b'n') {
                self.advance();
                return TokenKind::BigInt(v.to_string());
            }
            return TokenKind::Number(v as f64);
        }
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'o') || self.peek_at(1) == Some(b'O'))
        {
            self.advance();
            self.advance();
            while let Some(c) = self.peek() {
                if (b'0'..=b'7').contains(&c) || c == b'_' {
                    self.advance();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[start + 2..self.pos]).unwrap_or("0");
            let s: String = s.chars().filter(|&c| c != '_').collect();
            let v = i64::from_str_radix(&s, 8).unwrap_or(0);
            if self.peek() == Some(b'n') {
                self.advance();
                return TokenKind::BigInt(v.to_string());
            }
            return TokenKind::Number(v as f64);
        }
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'b') || self.peek_at(1) == Some(b'B'))
        {
            self.advance();
            self.advance();
            while let Some(c) = self.peek() {
                if c == b'0' || c == b'1' || c == b'_' {
                    self.advance();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[start + 2..self.pos]).unwrap_or("0");
            let s: String = s.chars().filter(|&c| c != '_').collect();
            let v = i64::from_str_radix(&s, 2).unwrap_or(0);
            if self.peek() == Some(b'n') {
                self.advance();
                return TokenKind::BigInt(v.to_string());
            }
            return TokenKind::Number(v as f64);
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit()
                || c == b'.'
                || c == b'e'
                || c == b'E'
                || c == b'_'
                || (c == b'+' || c == b'-')
                    && (self.src.get(self.pos.wrapping_sub(1)) == Some(&b'e')
                        || self.src.get(self.pos.wrapping_sub(1)) == Some(&b'E'))
            {
                self.advance();
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let s: String = s.chars().filter(|&c| c != '_').collect();
        // BigInt literal: integer digits followed by `n` (e.g. 123n, 0xffn).
        if self.peek() == Some(b'n') {
            self.advance();
            return TokenKind::BigInt(s);
        }
        TokenKind::Number(s.parse::<f64>().unwrap_or(f64::NAN))
    }

    fn read_string(&mut self, quote: u8) -> TokenKind {
        self.advance(); // opening quote
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == quote {
                self.advance();
                break;
            }
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
                                if c == b'}' {
                                    self.advance();
                                    break;
                                }
                                hex.push(c as char);
                                self.advance();
                            }
                            if let Ok(n) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(n) {
                                    s.push(ch);
                                }
                            }
                        } else {
                            let h: String = (0..4)
                                .filter_map(|_| self.advance().map(|c| c as char))
                                .collect();
                            if let Ok(n) = u32::from_str_radix(&h, 16) {
                                if let Some(ch) = char::from_u32(n) {
                                    s.push(ch);
                                }
                            }
                        }
                    }
                    Some(c) => s.push(c as char),
                    None => break,
                }
            } else {
                // Decode a UTF-8 multibyte sequence (non-ASCII byte). The
                // source is UTF-8; pushing each byte as a Latin-1 char would
                // corrupt supplementary characters (emoji etc.).
                self.advance();
                if c < 0x80 {
                    s.push(c as char);
                } else {
                    // Read the remaining bytes of the UTF-8 sequence.
                    let need = if c >= 0xF0 {
                        3
                    } else if c >= 0xE0 {
                        2
                    } else {
                        1
                    };
                    let mut buf = vec![c];
                    for _ in 0..need {
                        if let Some(b) = self.peek() {
                            self.advance();
                            buf.push(b);
                        }
                    }
                    if let Ok(st) = std::str::from_utf8(&buf) {
                        s.push_str(st);
                    }
                }
            }
        }
        TokenKind::String(s)
    }

    fn read_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                self.advance();
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
        match s {
            "var" => TokenKind::Var,
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "function" => TokenKind::Function,
            "class" => TokenKind::Class,
            "extends" => TokenKind::Extends,
            "static" => TokenKind::Static,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "with" => TokenKind::With,
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
            "super" => TokenKind::Super,
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
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "yield" => TokenKind::Yield,
            _ => TokenKind::Ident(s.to_string()),
        }
    }

    fn read_operator(&mut self) -> Option<TokenKind> {
        let c = self.peek()?;
        match c {
            b'+' => {
                self.advance();
                if self.peek() == Some(b'+') {
                    self.advance();
                    return Some(TokenKind::Inc);
                }
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::PlusAssign);
                }
                Some(TokenKind::Plus)
            }
            b'-' => {
                self.advance();
                if self.peek() == Some(b'-') {
                    self.advance();
                    return Some(TokenKind::Dec);
                }
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::MinusAssign);
                }
                Some(TokenKind::Minus)
            }
            b'*' => {
                self.advance();
                if self.peek() == Some(b'*') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::StarStarAssign);
                    }
                    return Some(TokenKind::StarStar);
                }
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::StarAssign);
                }
                Some(TokenKind::Star)
            }
            b'/' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::SlashAssign);
                }
                Some(TokenKind::Slash)
            }
            b'%' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::PercentAssign);
                }
                Some(TokenKind::Percent)
            }
            b'=' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::EqEqEq);
                    }
                    return Some(TokenKind::Eq);
                }
                if self.peek() == Some(b'>') {
                    self.advance();
                    return Some(TokenKind::Arrow);
                }
                Some(TokenKind::Assign)
            }
            b'!' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::NotEqEqEq);
                    }
                    return Some(TokenKind::NotEq);
                }
                Some(TokenKind::Not)
            }
            b'<' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::Lte);
                }
                if self.peek() == Some(b'<') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::ShlAssign);
                    }
                    return Some(TokenKind::Shl);
                }
                Some(TokenKind::Lt)
            }
            b'>' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::Gte);
                }
                if self.peek() == Some(b'>') {
                    self.advance();
                    if self.peek() == Some(b'>') {
                        self.advance();
                        if self.peek() == Some(b'=') {
                            self.advance();
                            return Some(TokenKind::UshrAssign);
                        }
                        return Some(TokenKind::Ushr);
                    }
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::ShrAssign);
                    }
                    return Some(TokenKind::Shr);
                }
                Some(TokenKind::Gt)
            }
            b'&' => {
                self.advance();
                if self.peek() == Some(b'&') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::AndAssign);
                    }
                    return Some(TokenKind::And);
                }
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::AmpAssign);
                }
                Some(TokenKind::BitAnd)
            }
            b'|' => {
                self.advance();
                if self.peek() == Some(b'|') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::OrAssign);
                    }
                    return Some(TokenKind::Or);
                }
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::PipeAssign);
                }
                Some(TokenKind::BitOr)
            }
            b'^' => {
                self.advance();
                if self.peek() == Some(b'=') {
                    self.advance();
                    return Some(TokenKind::CaretAssign);
                }
                Some(TokenKind::BitXor)
            }
            b'~' => {
                self.advance();
                Some(TokenKind::BitNot)
            }
            b'?' => {
                self.advance();
                if self.peek() == Some(b'?') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        return Some(TokenKind::NullishAssign);
                    }
                    return Some(TokenKind::Nullish);
                }
                // `?.` is optional chaining, but NOT when the `.` is followed by a
                // digit (`?.5` parses as the number `0.5`).
                if self.peek() == Some(b'.') && !matches!(self.peek_at(1), Some(b'0'..=b'9')) {
                    self.advance();
                    return Some(TokenKind::QuestionDot);
                }
                Some(TokenKind::Question)
            }
            b'.' => {
                self.advance();
                if self.peek() == Some(b'.') && self.peek_at(1) == Some(b'.') {
                    self.advance();
                    self.advance();
                    return Some(TokenKind::Spread);
                }
                Some(TokenKind::Dot)
            }
            b':' => {
                self.advance();
                Some(TokenKind::Colon)
            }
            b',' => {
                self.advance();
                Some(TokenKind::Comma)
            }
            b';' => {
                self.advance();
                Some(TokenKind::Semicolon)
            }
            b'#' => {
                self.advance();
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let name = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
                Some(TokenKind::PrivateName(name.to_string()))
            }
            b'(' => {
                self.advance();
                Some(TokenKind::LParen)
            }
            b')' => {
                self.advance();
                Some(TokenKind::RParen)
            }
            b'{' => {
                self.advance();
                Some(TokenKind::LBrace)
            }
            b'}' => {
                self.advance();
                Some(TokenKind::RBrace)
            }
            b'[' => {
                self.advance();
                Some(TokenKind::LBracket)
            }
            b']' => {
                self.advance();
                Some(TokenKind::RBracket)
            }
            _ => None,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_ws_and_comments();
        let line = self.line;
        let col = self.col;
        let preceded_by_newline = self.saw_newline;
        self.saw_newline = false;

        // Template-literal state machine.
        match self.template_state {
            1 => {
                self.template_state = 2;
                return Token::new(TokenKind::TemplateExprStart, line, col);
            }
            2 => {
                // Inside an interpolation; a top-level `}` closes it.
                if self.peek() == Some(b'}') {
                    self.advance();
                    self.template_state = 3;
                    return Token::new(TokenKind::TemplateExprEnd, line, col);
                }
            }
            3 => {
                return self.read_template_segment(line, col, preceded_by_newline);
            }
            _ => {}
        }

        let kind = match self.peek() {
            None => TokenKind::Eof,
            Some(c) if c.is_ascii_digit() => self.read_number(),
            Some(c)
                if c == b'.' && self.peek_at(1).map(|d| d.is_ascii_digit()).unwrap_or(false) =>
            {
                self.read_number()
            }
            Some(b'"') => self.read_string(b'"'),
            Some(b'\'') => self.read_string(b'\''),
            Some(b'`') => return self.read_template_start(line, col, preceded_by_newline),
            Some(b'/') => {
                // Regex literal vs division, decided by the previous token.
                if self.prev_value_ending {
                    self.read_operator()
                        .unwrap_or(TokenKind::Ident(String::from("/")))
                } else {
                    self.read_regex()
                }
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
                self.read_ident_or_keyword()
            }
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => {
                return self.next_token();
            }
            Some(0x0b) | Some(0x0c) => {
                return self.next_token();
            }
            _ => {
                if let Some(k) = self.read_operator() {
                    k
                } else {
                    self.advance();
                    TokenKind::Ident(format!(
                        "Unexpected char '{}'",
                        self.src.get(self.pos - 1).copied().unwrap_or(b'?') as char
                    ))
                }
            }
        };

        // Update the regex/division disambiguator for the next token.
        self.prev_value_ending = matches!(
            &kind,
            TokenKind::Ident(_)
                | TokenKind::Number(_)
                | TokenKind::BigInt(_)
                | TokenKind::String(_)
                | TokenKind::TemplateString(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::Undefined
                | TokenKind::This
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::Regex(_, _)
        );
        let mut tok = Token::new(kind, line, col);
        tok.preceded_by_newline = preceded_by_newline;
        tok
    }

    /// Read a regex literal `/pattern/flags`. The leading `/` is NOT yet consumed.
    fn read_regex(&mut self) -> TokenKind {
        self.advance(); // consume opening `/`
        let mut pattern = String::new();
        let mut in_class = false;
        while let Some(c) = self.peek() {
            if c == b'\\' {
                // Escaped char: keep the backslash and the following char.
                self.advance(); // consume backslash
                pattern.push('\\');
                if let Some(n) = self.peek() {
                    pattern.push(n as char);
                    self.advance();
                }
                continue;
            }
            if c == b'[' {
                in_class = true;
                pattern.push('[');
                self.advance();
                continue;
            }
            if c == b']' && in_class {
                in_class = false;
                pattern.push(']');
                self.advance();
                continue;
            }
            if c == b'/' && !in_class {
                self.advance();
                break;
            }
            pattern.push(c as char);
            self.advance();
        }
        let mut flags = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                flags.push(c as char);
                self.advance();
            } else {
                break;
            }
        }
        TokenKind::Regex(pattern, flags)
    }

    fn read_template_start(&mut self, line: usize, col: usize, preceded_by_newline: bool) -> Token {
        self.advance(); // consume backtick
        self.read_template_segment(line, col, preceded_by_newline)
    }

    /// Read the next segment of a template literal, starting at the current position
    /// (after the opening backtick or after a `}` that closed an interpolation).
    fn read_template_segment(
        &mut self,
        line: usize,
        col: usize,
        preceded_by_newline: bool,
    ) -> Token {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == b'`' {
                self.advance();
                break;
            }
            if c == b'$' && self.peek_at(1) == Some(b'{') {
                self.advance();
                self.advance();
                self.template_state = 1;
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
                if c < 0x80 {
                    s.push(c as char);
                } else {
                    let need = if c >= 0xF0 {
                        3
                    } else if c >= 0xE0 {
                        2
                    } else {
                        1
                    };
                    let mut buf = vec![c];
                    for _ in 0..need {
                        if let Some(b) = self.peek() {
                            self.advance();
                            buf.push(b);
                        }
                    }
                    if let Ok(st) = std::str::from_utf8(&buf) {
                        s.push_str(st);
                    }
                }
            }
        }
        // closed the template literal with a backtick: return to normal scanning.
        self.template_state = 0;
        let mut tok = Token::new(TokenKind::TemplateString(s), line, col);
        tok.preceded_by_newline = preceded_by_newline;
        tok
    }

    #[allow(dead_code)]
    fn read_template_continue(
        &mut self,
        line: usize,
        col: usize,
        preceded_by_newline: bool,
    ) -> Token {
        self.read_template_segment(line, col, preceded_by_newline)
    }

    pub fn tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token();
            let is_eof = t.kind == TokenKind::Eof;
            out.push(t);
            if is_eof {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokens()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    #[allow(clippy::approx_constant)]
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
