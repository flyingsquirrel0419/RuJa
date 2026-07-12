use crate::token::{Token, TokenKind};

/// Decode one UTF-8 code point starting at the beginning of `bytes`,
/// returning `(char, byte_length)`. `(0, 0)` on invalid input.
fn decode_utf8_at(bytes: &[u8]) -> (char, usize) {
    if bytes.is_empty() {
        return ('\0', 0);
    }
    let b0 = bytes[0];
    let (len, init) = if b0 < 0x80 {
        (1, b0 as u32)
    } else if b0 < 0xC0 {
        return ('\0', 0); // continuation byte without lead
    } else if b0 < 0xE0 {
        (2, (b0 & 0x1F) as u32)
    } else if b0 < 0xF0 {
        (3, (b0 & 0x0F) as u32)
    } else {
        (4, (b0 & 0x07) as u32)
    };
    if bytes.len() < len {
        return ('\0', 0);
    }
    let mut cp = init;
    for &b in bytes.iter().take(len).skip(1) {
        if (b & 0xC0) != 0x80 {
            return ('\0', 0);
        }
        cp = (cp << 6) | (b & 0x3F) as u32;
    }
    match char::from_u32(cp) {
        Some(c) => (c, len),
        None => ('\0', 0),
    }
}

fn is_id_continue(c: char) -> bool {
    // ES IdentifierPart: ID_Continue plus `$`, ZWNJ, and ZWJ.
    c == '\u{200C}'
        || c == '\u{200D}'
        || c == '$'
        || unicode_ident::is_xid_continue(c)
        || is_other_id_start(c)
        || is_other_id_continue(c)
}

fn is_id_start(c: char) -> bool {
    // ES IdentifierStart: ID_Start plus `$` and `_`.
    c == '$' || c == '_' || unicode_ident::is_xid_start(c) || is_other_id_start(c)
}

fn is_other_id_start(c: char) -> bool {
    matches!(
        c,
        '\u{1885}' | '\u{1886}' | '\u{2118}' | '\u{212E}' | '\u{309B}' | '\u{309C}'
    )
}

fn is_other_id_continue(c: char) -> bool {
    matches!(
        c,
        '\u{00B7}' | '\u{0387}' | '\u{1369}'..='\u{1371}' | '\u{19DA}'
    )
}

fn is_unicode_space_separator(c: char) -> bool {
    matches!(
        c,
        '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// Read a Unicode escape that may appear inside an identifier: `\uXXXX` or
/// `\u{XXXX...}`. Returns the decoded char and the number of source bytes
/// consumed (including the leading backslash). `None` if not a valid escape.
fn read_ident_escape(src: &[u8]) -> Option<(char, usize)> {
    if src.len() < 2 || src[0] != b'\\' || src[1] != b'u' {
        return None;
    }
    if src.len() > 2 && src[2] == b'{' {
        // \u{XXXX...} form: up to 6 hex digits then `}`.
        let mut i = 3;
        let mut cp = 0u32;
        let mut count = 0;
        while i < src.len() {
            let b = src[i];
            if b == b'}' {
                if count == 0 || count > 6 {
                    return None;
                }
                return char::from_u32(cp).map(|c| (c, i + 1));
            }
            let d = (b as char).to_digit(16)?;
            cp = cp.checked_mul(16)?.checked_add(d)?;
            i += 1;
            count += 1;
        }
        None
    } else {
        // \uXXXX form: exactly 4 hex digits.
        if src.len() < 6 {
            return None;
        }
        let mut cp = 0u32;
        for &b in src.iter().take(6).skip(2) {
            let d = (b as char).to_digit(16)?;
            cp = cp * 16 + d;
        }
        char::from_u32(cp).map(|c| (c, 6))
    }
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    saw_newline: bool,
    /// Whether the previous significant token ended an expression (so a `/`
    /// means division rather than a regex literal).
    prev_value_ending: bool,
    /// Parenthesis depth after the previous significant token.
    paren_depth: usize,
    /// Set after `for` (and preserved across `await`) until the opening `(`.
    pending_for_head: bool,
    /// Parenthesis depths that correspond to active `for (...)` heads.
    for_head_depths: Vec<usize>,
    /// Template-literal scanner state.
    /// 0 = normal, 1 = emit TemplateExprStart next,
    /// 3 = read next segment after an interpolation closed.
    pub template_state: u8,
    /// Brace depth inside a template interpolation. When > 0, top-level `}`
    /// closes the interpolation and returns TemplateExprEnd; nested `{`/`}`
    /// pairs are tracked as normal braces.
    pub template_expr_depth: usize,
    /// Stack of outer template contexts, saved when a nested template literal
    /// appears inside an interpolation. Each entry is `(template_state,
    /// template_expr_depth)` to restore after the nested template closes.
    template_stack: Vec<(u8, usize)>,
    /// Whether the last identifier token had a Unicode escape.
    last_ident_had_escape: bool,
    /// Whether the last string literal token contained an escape sequence or
    /// line continuation.
    last_string_had_escape: bool,
    /// Whether the last string literal token contained a legacy octal or
    /// non-octal decimal escape.
    last_string_had_legacy_escape: bool,
    /// Whether the last string contained an unpaired surrogate escape.
    last_string_not_well_formed: bool,
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
            paren_depth: 0,
            pending_for_head: false,
            for_head_depths: Vec::new(),
            template_state: 0,
            template_expr_depth: 0,
            template_stack: Vec::new(),
            last_ident_had_escape: false,
            last_string_had_escape: false,
            last_string_had_legacy_escape: false,
            last_string_not_well_formed: false,
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
            } else if b == b'\r' {
                // CR is a line terminator. Skip a following LF so that
                // CRLF counts as a single line break.
                if self.peek() == Some(b'\n') {
                    self.pos += 1;
                }
                self.line += 1;
                self.col = 1;
                self.saw_newline = true;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn is_identifier_start_at_pos(&self, pos: usize) -> bool {
        match self.src.get(pos).copied() {
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => true,
            Some(b'\\') if self.src.get(pos + 1) == Some(&b'u') => true,
            Some(c) if c >= 0x80 => {
                let (ch, len) = decode_utf8_at(&self.src[pos..]);
                len > 0 && is_id_start(ch)
            }
            _ => false,
        }
    }

    fn invalid_numeric_tail(&self) -> bool {
        self.peek().is_some_and(|c| c.is_ascii_digit()) || self.is_identifier_start_at_pos(self.pos)
    }

    fn read_radix_number(
        &mut self,
        radix: u32,
        valid_digit: fn(u8) -> bool,
        parse_start: usize,
    ) -> TokenKind {
        let mut digits = Vec::new();
        let mut last_sep = false;
        let mut saw_digit = false;
        if self.peek() == Some(b'_') {
            return TokenKind::LexError("invalid numeric separator".to_string());
        }
        while let Some(c) = self.peek() {
            if valid_digit(c) {
                saw_digit = true;
                last_sep = false;
                digits.push(c);
                self.advance();
            } else if c == b'_' {
                if !saw_digit || last_sep {
                    return TokenKind::LexError("invalid numeric separator".to_string());
                }
                last_sep = true;
                self.advance();
            } else {
                break;
            }
        }
        if !saw_digit || last_sep {
            return TokenKind::LexError("invalid numeric literal".to_string());
        }
        let is_bigint = if self.peek() == Some(b'n') {
            self.advance();
            true
        } else {
            false
        };
        if self.invalid_numeric_tail() {
            return TokenKind::LexError("invalid numeric literal".to_string());
        }
        let raw = std::str::from_utf8(&digits).unwrap_or("0");
        let cleaned: String = raw.chars().filter(|&c| c != '_').collect();
        if is_bigint {
            let v = num_bigint::BigInt::parse_bytes(cleaned.as_bytes(), radix).unwrap_or_default();
            TokenKind::BigInt(v.to_string())
        } else {
            let value = i64::from_str_radix(&cleaned, radix).unwrap_or(0) as f64;
            // Keep `parse_start` observed so callers pass the source start for
            // symmetry with decimal scanning; it also documents the intended
            // source span for future diagnostics.
            let _ = parse_start;
            TokenKind::Number(value)
        }
    }

    fn skip_ws_and_comments(&mut self) -> Option<TokenKind> {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') => {
                    self.advance();
                }
                Some(0x0b) | Some(0x0c) => {
                    // vertical tab and form feed are whitespace per ES.
                    self.advance();
                }
                // NBSP (U+00A0) is whitespace per ES spec (WhiteSpace).
                // UTF-8: 0xC2 0xA0
                Some(0xC2) if self.peek_at(1) == Some(0xA0) => {
                    self.advance();
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                }
                // LS (U+2028) / PS (U+2029) line terminators: 0xE2 0x80 0xA8/0xA9
                Some(0xE2)
                    if self.peek_at(1) == Some(0x80)
                        && matches!(self.peek_at(2), Some(0xA8) | Some(0xA9)) =>
                {
                    self.read_line_terminator_sequence();
                }
                Some(c) if c >= 0x80 => {
                    let (ch, len) = decode_utf8_at(&self.src[self.pos..]);
                    if len > 0 && is_unicode_space_separator(ch) {
                        for _ in 0..len {
                            self.advance();
                        }
                    } else {
                        break;
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while self.peek().is_some() {
                        if self.is_line_terminator_start() {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.advance();
                    self.advance();
                    let mut closed = false;
                    while let Some(c) = self.peek() {
                        if c == b'*' && self.peek_at(1) == Some(b'/') {
                            self.advance();
                            self.advance();
                            closed = true;
                            break;
                        }
                        if self.is_line_terminator_start() {
                            self.read_line_terminator_sequence();
                            continue;
                        }
                        self.advance();
                    }
                    if !closed {
                        return Some(TokenKind::LexError(
                            "unterminated multiline comment".to_string(),
                        ));
                    }
                }
                _ => break,
            }
        }
        None
    }

    fn read_number(&mut self) -> TokenKind {
        let start = self.pos;
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'x') || self.peek_at(1) == Some(b'X'))
        {
            self.advance();
            self.advance();
            return self.read_radix_number(16, |c| c.is_ascii_hexdigit(), start);
        }
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'o') || self.peek_at(1) == Some(b'O'))
        {
            self.advance();
            self.advance();
            return self.read_radix_number(8, |c| (b'0'..=b'7').contains(&c), start);
        }
        if self.peek() == Some(b'0')
            && (self.peek_at(1) == Some(b'b') || self.peek_at(1) == Some(b'B'))
        {
            self.advance();
            self.advance();
            return self.read_radix_number(2, |c| c == b'0' || c == b'1', start);
        }

        let starts_with_dot = self.peek() == Some(b'.');
        let mut seen_dot = false;
        let mut seen_exp = false;
        let mut invalid_separator = false;
        let mut last_sep = false;
        let mut integer_digits = String::new();
        let mut digit_count = 0usize;
        let mut has_non_octal_digit = false;

        if starts_with_dot {
            seen_dot = true;
            self.advance();
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                if !seen_dot && !seen_exp {
                    integer_digits.push(c as char);
                    if matches!(c, b'8' | b'9') {
                        has_non_octal_digit = true;
                    }
                }
                digit_count += 1;
                last_sep = false;
                self.advance();
            } else if c == b'_' {
                if digit_count == 0 || last_sep {
                    invalid_separator = true;
                }
                last_sep = true;
                self.advance();
            } else {
                break;
            }
        }
        if last_sep {
            invalid_separator = true;
        }

        if self.peek() == Some(b'.') && !seen_dot && !seen_exp {
            seen_dot = true;
            last_sep = false;
            digit_count = 0;
            self.advance();
            if self.peek() == Some(b'_') {
                invalid_separator = true;
            }
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    digit_count += 1;
                    last_sep = false;
                    self.advance();
                } else if c == b'_' {
                    if digit_count == 0 || last_sep {
                        invalid_separator = true;
                    }
                    last_sep = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if last_sep {
                invalid_separator = true;
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            seen_exp = true;
            self.advance();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.advance();
            }
            let mut exp_digits = 0usize;
            last_sep = false;
            if self.peek() == Some(b'_') {
                invalid_separator = true;
            }
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    exp_digits += 1;
                    last_sep = false;
                    self.advance();
                } else if c == b'_' {
                    if exp_digits == 0 || last_sep {
                        invalid_separator = true;
                    }
                    last_sep = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if exp_digits == 0 || last_sep {
                invalid_separator = true;
            }
        }

        let raw = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let cleaned: String = raw.chars().filter(|&c| c != '_').collect();
        let legacy_integer = !starts_with_dot
            && !seen_dot
            && !seen_exp
            && integer_digits.len() > 1
            && integer_digits.starts_with('0');
        if invalid_separator || (legacy_integer && raw.contains('_')) {
            return TokenKind::LexError("invalid numeric separator".to_string());
        }

        if self.peek() == Some(b'n') {
            self.advance();
            if seen_dot || seen_exp || legacy_integer {
                return TokenKind::LexError("invalid BigInt literal".to_string());
            }
            if self.invalid_numeric_tail() {
                return TokenKind::LexError("invalid numeric literal".to_string());
            }
            return TokenKind::BigInt(cleaned);
        }
        if self.invalid_numeric_tail() {
            return TokenKind::LexError("invalid numeric literal".to_string());
        }
        if legacy_integer {
            if has_non_octal_digit {
                return TokenKind::LegacyNumber(cleaned.parse::<f64>().unwrap_or(f64::NAN));
            }
            let value = i64::from_str_radix(&integer_digits, 8).unwrap_or(0) as f64;
            return TokenKind::LegacyNumber(value);
        }
        TokenKind::Number(cleaned.parse::<f64>().unwrap_or(f64::NAN))
    }

    fn read_string(&mut self, quote: u8) -> TokenKind {
        self.last_string_had_escape = false;
        self.last_string_had_legacy_escape = false;
        self.last_string_not_well_formed = false;
        self.advance(); // opening quote
        let mut s = String::new();
        let mut closed = false;
        while let Some(c) = self.peek() {
            if c == quote {
                self.advance();
                closed = true;
                break;
            }
            if matches!(c, b'\n' | b'\r') {
                return TokenKind::LexError("unterminated string literal".to_string());
            }
            if c == b'\\' {
                self.last_string_had_escape = true;
                self.advance();
                if self.is_line_terminator_start() {
                    self.read_line_terminator_sequence();
                    continue;
                }
                match self.advance() {
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(b'r') => s.push('\r'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'\'') => s.push('\''),
                    Some(b'"') => s.push('"'),
                    Some(b'`') => s.push('`'),
                    Some(b'0') => {
                        if self.peek().is_some_and(|b| b.is_ascii_digit()) {
                            self.last_string_had_legacy_escape = true;
                            s.push(self.read_legacy_octal_escape(b'0'));
                        } else {
                            s.push('\0');
                        }
                    }
                    Some(c @ b'1'..=b'7') => {
                        self.last_string_had_legacy_escape = true;
                        s.push(self.read_legacy_octal_escape(c));
                    }
                    Some(c @ b'8'..=b'9') => {
                        self.last_string_had_legacy_escape = true;
                        s.push(c as char);
                    }
                    Some(b'b') => s.push('\u{0008}'),
                    Some(b'f') => s.push('\u{000C}'),
                    Some(b'v') => s.push('\u{000B}'),
                    Some(b'x') => {
                        // \xHH: exactly two hex digits -> one code unit.
                        match self.read_hex_digits(2) {
                            Some(n) => s.push(char::from_u32(n).unwrap_or('\u{FFFD}')),
                            None => {
                                return TokenKind::LexError(
                                    "invalid hex escape sequence".to_string(),
                                );
                            }
                        }
                    }
                    Some(b'u') => match self.read_unicode_escape_value() {
                        Some(cp) => {
                            if (0xD800..=0xDBFF).contains(&cp) {
                                let save = self.pos;
                                if self.peek() == Some(b'\\') && self.peek_at(1) == Some(b'u') {
                                    self.advance();
                                    self.advance();
                                    if let Some(low) = self.read_unicode_escape_value() {
                                        if (0xDC00..=0xDFFF).contains(&low) {
                                            let cp =
                                                0x10000 + (((cp - 0xD800) << 10) | (low - 0xDC00));
                                            if let Some(ch) = char::from_u32(cp) {
                                                s.push(ch);
                                                continue;
                                            }
                                        }
                                    }
                                }
                                self.pos = save;
                            }
                            if (0xD800..=0xDFFF).contains(&cp) {
                                self.last_string_not_well_formed = true;
                                s.push_str(&crate::value::utf16_to_string(&[cp as u16]));
                            } else if let Some(ch) = char::from_u32(cp) {
                                s.push(ch);
                            } else {
                                return TokenKind::LexError(
                                    "invalid unicode escape sequence".to_string(),
                                );
                            }
                        }
                        None => {
                            return TokenKind::LexError(
                                "invalid unicode escape sequence".to_string(),
                            );
                        }
                    },
                    Some(c) => match self.read_char_from_first_byte(c) {
                        Some(ch) => s.push(ch),
                        None => {
                            return TokenKind::LexError(
                                "invalid utf-8 in string literal".to_string(),
                            );
                        }
                    },
                    None => break,
                }
            } else {
                // Decode a UTF-8 multibyte sequence (non-ASCII byte). The
                // source is UTF-8; pushing each byte as a Latin-1 char would
                // corrupt supplementary characters (emoji etc.).
                self.advance();
                match self.read_char_from_first_byte(c) {
                    Some(ch) => s.push(ch),
                    None => {
                        return TokenKind::LexError("invalid utf-8 in string literal".to_string());
                    }
                }
            }
        }
        if !closed {
            return TokenKind::LexError("unterminated string literal".to_string());
        }
        TokenKind::String(s)
    }

    fn read_char_from_first_byte(&mut self, first: u8) -> Option<char> {
        if first < 0x80 {
            return Some(first as char);
        }
        let need = if first >= 0xF0 {
            3
        } else if first >= 0xE0 {
            2
        } else if first >= 0xC0 {
            1
        } else {
            return None;
        };
        let mut buf = vec![first];
        for _ in 0..need {
            buf.push(self.advance()?);
        }
        std::str::from_utf8(&buf).ok()?.chars().next()
    }

    fn read_legacy_octal_escape(&mut self, first: u8) -> char {
        let mut value = (first - b'0') as u32;
        let mut count = 1;
        let max_digits = if first <= b'3' { 3 } else { 2 };
        while count < max_digits {
            let Some(next @ b'0'..=b'7') = self.peek() else {
                break;
            };
            value = value * 8 + (next - b'0') as u32;
            self.advance();
            count += 1;
        }
        char::from_u32(value).unwrap_or('\u{FFFD}')
    }

    /// Read exactly `n` hex digits and return the parsed value, or None if
    /// any digit is missing/invalid. Assumes the `\x`/`\u` introducer was
    /// already consumed.
    fn read_hex_digits(&mut self, n: usize) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let b = self.advance()?;
            let d = (b as char).to_digit(16)?;
            v = v * 16 + d;
        }
        Some(v)
    }

    /// Read a `\uXXXX` or `\u{XXXX...}` escape (the `\u` already consumed),
    /// returning the numeric code point/code unit value or None on invalid input.
    fn read_unicode_escape_value(&mut self) -> Option<u32> {
        if self.peek() == Some(b'{') {
            self.advance();
            let mut v = 0u32;
            let mut count = 0;
            while let Some(b) = self.peek() {
                if b == b'}' {
                    self.advance();
                    if count == 0 || count > 6 {
                        return None;
                    }
                    return (v <= 0x10FFFF).then_some(v);
                }
                let d = (b as char).to_digit(16)?;
                v = v.checked_mul(16)?.checked_add(d)?;
                self.advance();
                count += 1;
            }
            // Unterminated \u{...}
            None
        } else {
            self.read_hex_digits(4)
        }
    }

    fn read_ident_or_keyword(&mut self) -> TokenKind {
        // Identifiers may contain Unicode escapes (`\uXXXX` / `\u{XXXX}`),
        // which decode to the corresponding character. Escapes fold into the
        // logical name so keyword matching uses the decoded form (e.g.
        // `\u{63}ase` -> `case`). The first char must satisfy IdentifierStart.
        let mut buf = String::new();
        let mut had_escape = false;
        let mut first = true;
        loop {
            if self.peek() == Some(b'\\') && self.peek_at(1) == Some(b'u') {
                let (ch, len) = match read_ident_escape(&self.src[self.pos..]) {
                    Some(v) => v,
                    None => {
                        // Invalid escape: if nothing consumed yet,
                        // advance past `\u` to avoid looping forever.
                        if buf.is_empty() {
                            self.advance();
                            self.advance();
                        }
                        break;
                    }
                };
                let ok = if first {
                    is_id_start(ch)
                } else {
                    is_id_continue(ch)
                };
                if !ok {
                    // Valid escape but not an identifier character
                    // (e.g. `\u007B` -> `{`) is a SyntaxError.
                    for _ in 0..len {
                        self.advance();
                    }
                    if first {
                        return TokenKind::LexError("invalid identifier start".to_string());
                    }
                    return TokenKind::LexError("invalid identifier continue".to_string());
                }
                buf.push(ch);
                for _ in 0..len {
                    self.advance();
                }
                had_escape = true;
                first = false;
                continue;
            }
            // If we got here with a leading `\u` that is not a valid escape
            // (e.g. `\u00` with too few hex digits) and nothing was consumed
            // yet, surface a parse error instead of looping forever.
            // If we got here with a leading `\u` that is not a valid escape
            // (e.g. `\u00` with too few hex digits) and nothing was consumed
            // yet, surface a parse error instead of looping forever.
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                buf.push(c as char);
                self.advance();
            } else if c >= 0x80 {
                let (ch, len) = decode_utf8_at(&self.src[self.pos..]);
                if len > 0 && is_id_continue(ch) {
                    buf.push(ch);
                    for _ in 0..len {
                        self.advance();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
            first = false;
        }
        self.last_ident_had_escape = had_escape;
        if had_escape {
            return TokenKind::Ident(buf);
        }
        let s = std::str::from_utf8(&self.src[self.pos - buf.len()..self.pos]).unwrap_or("");
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
            "debugger" => TokenKind::Debugger,
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

    fn read_private_name(&mut self) -> TokenKind {
        let mut buf = String::new();
        let mut first = true;
        loop {
            if self.peek() == Some(b'\\') && self.peek_at(1) == Some(b'u') {
                let (ch, len) = match read_ident_escape(&self.src[self.pos..]) {
                    Some(v) => v,
                    None => {
                        if buf.is_empty() {
                            self.advance();
                            self.advance();
                        }
                        break;
                    }
                };
                let ok = if first {
                    is_id_start(ch)
                } else {
                    is_id_continue(ch)
                };
                if !ok {
                    for _ in 0..len {
                        self.advance();
                    }
                    if first {
                        return TokenKind::LexError("invalid private name start".to_string());
                    }
                    return TokenKind::LexError("invalid private name continue".to_string());
                }
                buf.push(ch);
                for _ in 0..len {
                    self.advance();
                }
                first = false;
                continue;
            }

            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            let ascii_ok =
                c.is_ascii_alphabetic() || c == b'_' || c == b'$' || (!first && c.is_ascii_digit());
            if ascii_ok {
                buf.push(c as char);
                self.advance();
            } else if c >= 0x80 {
                let (ch, len) = decode_utf8_at(&self.src[self.pos..]);
                let ok = len > 0
                    && if first {
                        is_id_start(ch)
                    } else {
                        is_id_continue(ch)
                    };
                if !ok {
                    break;
                }
                buf.push(ch);
                for _ in 0..len {
                    self.advance();
                }
            } else {
                break;
            }
            first = false;
        }

        if buf.is_empty() {
            TokenKind::LexError("invalid private name start".to_string())
        } else {
            TokenKind::PrivateName(buf)
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
                Some(self.read_private_name())
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
                if self.template_state == 2 {
                    self.template_expr_depth += 1;
                }
                Some(TokenKind::LBrace)
            }
            b'}' => {
                self.advance();
                if self.template_state == 2 && self.template_expr_depth > 0 {
                    self.template_expr_depth -= 1;
                }
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
        // In template-literal mode (state 3), the next segment starts
        // right after `}` — do NOT skip whitespace, as it's part of the
        // template string content.
        if self.template_state != 3 {
            if let Some(kind) = self.skip_ws_and_comments() {
                let mut tok = Token::new(kind, self.line, self.col);
                tok.preceded_by_newline = self.saw_newline;
                return tok;
            }
        }
        let line = self.line;
        let col = self.col;
        let preceded_by_newline = self.saw_newline;
        self.saw_newline = false;
        self.last_ident_had_escape = false;
        self.last_string_had_escape = false;
        self.last_string_had_legacy_escape = false;
        self.last_string_not_well_formed = false;

        // Template-literal state machine.
        match self.template_state {
            1 => {
                self.template_state = 2;
                self.template_expr_depth = 0;
                return Token::new(TokenKind::TemplateExprStart, line, col);
            }
            2 => {
                // Inside an interpolation; a top-level `}` closes it, but
                // nested `{`/`}` pairs from object literals/blocks are tracked
                // by template_expr_depth.
                if self.peek() == Some(b'}') && self.template_expr_depth == 0 {
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
            Some(b'`') => {
                // Nested template literal inside an interpolation: save the
                // outer context so we can resume it after the inner template
                // closes.
                if self.template_state == 2 {
                    self.template_stack
                        .push((self.template_state, self.template_expr_depth));
                }
                return self.read_template_start(line, col, preceded_by_newline);
            }
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
            Some(c) if c >= 0x80 => {
                let (ch, len) = decode_utf8_at(&self.src[self.pos..]);
                if len > 0 && is_unicode_space_separator(ch) {
                    for _ in 0..len {
                        self.advance();
                    }
                    return self.next_token();
                }
                // Unicode identifier start (e.g. `π`, `café`, CJK names).
                if len > 0 && is_id_start(ch) {
                    self.read_ident_or_keyword()
                } else {
                    // Not a valid id start: advance past the byte(s) so the
                    // lexer does not loop, and surface as a parse error token.
                    let step = if len > 0 { len } else { 1 };
                    for _ in 0..step {
                        self.advance();
                    }
                    TokenKind::LexError(format!("invalid identifier start '{}'", ch))
                }
            }
            Some(b'\\') if self.peek_at(1) == Some(b'u') => {
                // `\uXXXX` / `\u{XXXX}` identifier start.
                self.read_ident_or_keyword()
            }
            Some(b'\\') => {
                // A backslash that is not a valid identifier escape here is a
                // stray character; consume it so the lexer does not loop and
                // surface it as a parse error token.
                self.advance();
                TokenKind::Ident(String::from("\\"))
            }
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => {
                return self.next_token();
            }
            Some(0x0b) | Some(0x0c) => {
                return self.next_token();
            }
            // NBSP (U+00A0) whitespace.
            Some(0xC2) if self.peek_at(1) == Some(0xA0) => {
                self.advance();
                self.advance();
                return self.next_token();
            }
            // LS (U+2028) / PS (U+2029) line terminators.
            Some(0xE2)
                if self.peek_at(1) == Some(0x80)
                    && matches!(self.peek_at(2), Some(0xA8) | Some(0xA9)) =>
            {
                self.read_line_terminator_sequence();
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

        let is_for_head_top_level = self
            .for_head_depths
            .last()
            .is_some_and(|depth| *depth == self.paren_depth);
        let is_for_of_delimiter = matches!(&kind, TokenKind::Of)
            && !self.last_ident_had_escape
            && self.prev_value_ending
            && is_for_head_top_level;

        // Update the regex/division disambiguator for the next token.
        self.prev_value_ending = matches!(
            &kind,
            TokenKind::Ident(_)
                | TokenKind::PrivateName(_)
                | TokenKind::Number(_)
                | TokenKind::LegacyNumber(_)
                | TokenKind::BigInt(_)
                | TokenKind::String(_)
                | TokenKind::TemplateString { .. }
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::Undefined
                | TokenKind::This
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Regex(_, _)
        ) || (matches!(&kind, TokenKind::Of) && !is_for_of_delimiter);

        match &kind {
            TokenKind::For => {
                self.pending_for_head = true;
            }
            TokenKind::Await if self.pending_for_head => {}
            TokenKind::LParen => {
                self.paren_depth += 1;
                if self.pending_for_head {
                    self.for_head_depths.push(self.paren_depth);
                    self.pending_for_head = false;
                }
            }
            TokenKind::RParen => {
                if self
                    .for_head_depths
                    .last()
                    .is_some_and(|depth| *depth == self.paren_depth)
                {
                    self.for_head_depths.pop();
                }
                self.paren_depth = self.paren_depth.saturating_sub(1);
                self.pending_for_head = false;
            }
            _ => {
                if self.pending_for_head {
                    self.pending_for_head = false;
                }
            }
        }
        let mut tok = Token::new(kind, line, col);
        tok.preceded_by_newline = preceded_by_newline;
        tok.had_escape = self.last_ident_had_escape;
        tok.string_had_escape = self.last_string_had_escape;
        tok.string_had_legacy_escape = self.last_string_had_legacy_escape;
        tok.string_not_well_formed = self.last_string_not_well_formed;
        tok
    }

    /// Read a regex literal `/pattern/flags`. The leading `/` is NOT yet consumed.
    fn read_regex(&mut self) -> TokenKind {
        self.advance(); // consume opening `/`
        let mut pattern = String::new();
        let mut in_class = false;
        let mut closed = false;
        while let Some(c) = self.peek() {
            if self.is_line_terminator_start() {
                return TokenKind::LexError("unterminated regular expression literal".to_string());
            }
            if c == b'\\' {
                // Escaped char: keep the backslash and the following char.
                self.advance(); // consume backslash
                if self.is_line_terminator_start() {
                    return TokenKind::LexError(
                        "unterminated regular expression literal".to_string(),
                    );
                }
                pattern.push('\\');
                if let Some(ch) = self.read_regex_pattern_char() {
                    pattern.push(ch);
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
                closed = true;
                break;
            }
            if let Some(ch) = self.read_regex_pattern_char() {
                pattern.push(ch);
            }
        }
        if !closed {
            return TokenKind::LexError("unterminated regular expression literal".to_string());
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
        match validate_regex_literal(&pattern, &flags) {
            Ok(()) => TokenKind::Regex(pattern, flags),
            Err(msg) => TokenKind::LexError(msg),
        }
    }

    fn read_regex_pattern_char(&mut self) -> Option<char> {
        let b = self.peek()?;
        if b < 0x80 {
            self.advance();
            Some(b as char)
        } else {
            let (ch, len) = decode_utf8_at(&self.src[self.pos..]);
            if len == 0 {
                self.advance();
                Some(b as char)
            } else {
                for _ in 0..len {
                    self.advance();
                }
                Some(ch)
            }
        }
    }

    fn read_template_start(&mut self, line: usize, col: usize, preceded_by_newline: bool) -> Token {
        self.advance(); // consume backtick
        self.read_template_segment(line, col, preceded_by_newline)
    }

    /// True if the current position starts a LineTerminatorSequence.
    fn is_line_terminator_start(&self) -> bool {
        match self.peek() {
            Some(b'\n') | Some(b'\r') => true,
            Some(0xE2) => {
                self.peek_at(1) == Some(0x80) && matches!(self.peek_at(2), Some(0xA8) | Some(0xA9))
            }
            _ => false,
        }
    }

    /// Consume a LineTerminatorSequence and return its source bytes as a UTF-8
    /// string. Updates line/column tracking like `advance`.
    fn read_line_terminator_sequence(&mut self) -> String {
        let mut buf = Vec::new();
        match self.peek() {
            Some(b'\r') => {
                if self.peek_at(1) == Some(b'\n') {
                    // CRLF: one advance() consumes both bytes.
                    self.advance();
                    buf.extend_from_slice(b"\r\n");
                } else {
                    buf.push(self.advance().unwrap());
                }
            }
            Some(b'\n') => {
                buf.push(self.advance().unwrap());
            }
            Some(0xE2)
                if self.peek_at(1) == Some(0x80)
                    && matches!(self.peek_at(2), Some(0xA8) | Some(0xA9)) =>
            {
                buf.push(self.advance().unwrap());
                buf.push(self.advance().unwrap());
                buf.push(self.advance().unwrap());
                self.line += 1;
                self.col = 1;
                self.saw_newline = true;
            }
            _ => {}
        }
        String::from_utf8(buf).unwrap_or_default()
    }

    /// Read a hex escape (\xHH) for a template literal. Only advances over valid
    /// hex digits; returns None if fewer than two hex digits are available.
    fn read_template_hex_escape(&mut self) -> Option<char> {
        let mut value = 0u32;
        for _ in 0..2 {
            let b = self.peek()?;
            let d = (b as char).to_digit(16)?;
            value = value * 16 + d;
            self.advance();
        }
        char::from_u32(value)
    }

    /// Read a unicode escape (\uXXXX or \u{X...}) for a template literal.
    /// Returns None for malformed escapes; only valid hex digits and a closing
    /// brace (for the braced form) are consumed.
    fn read_template_unicode_escape(&mut self) -> Option<char> {
        if self.peek() == Some(b'{') {
            self.advance(); // consume {
            let mut value = 0u32;
            let mut count = 0;
            loop {
                match self.peek() {
                    Some(b'}') => {
                        self.advance();
                        if count == 0 || count > 6 {
                            return None;
                        }
                        return char::from_u32(value);
                    }
                    Some(b) => {
                        let d = (b as char).to_digit(16)?;
                        value = value.checked_mul(16)?.checked_add(d)?;
                        self.advance();
                        count += 1;
                        if count > 6 {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
        } else {
            let mut value = 0u32;
            for _ in 0..4 {
                let b = self.peek()?;
                let d = (b as char).to_digit(16)?;
                value = value * 16 + d;
                self.advance();
            }
            char::from_u32(value)
        }
    }

    /// Read an escape sequence inside a template literal segment. Valid escapes
    /// are decoded into `cooked` and recorded in `raw`. Invalid escapes (legacy
    /// octal, malformed hex/unicode, stray trailing backslash) set `valid` to
    /// false and append the malformed source text to `raw` without updating
    /// `cooked`. Line continuations append `\\` + the terminator to `raw` and
    /// contribute nothing to `cooked`.
    fn read_template_escape(&mut self, raw: &mut String, cooked: &mut String, valid: &mut bool) {
        let raw_start = self.pos; // position of the backslash
        self.advance(); // consume '\\'

        // LineContinuation: \\ LineTerminatorSequence.
        if self.is_line_terminator_start() {
            raw.push('\\');
            raw.push_str(&self.read_line_terminator_sequence());
            return;
        }

        match self.peek() {
            Some(b'n') => {
                self.advance();
                cooked.push('\n');
                raw.push_str("\\n");
            }
            Some(b't') => {
                self.advance();
                cooked.push('\t');
                raw.push_str("\\t");
            }
            Some(b'r') => {
                self.advance();
                cooked.push('\r');
                raw.push_str("\\r");
            }
            Some(b'\\') => {
                self.advance();
                cooked.push('\\');
                raw.push_str("\\\\");
            }
            Some(b'\'') => {
                self.advance();
                cooked.push('\'');
                raw.push_str("\\'");
            }
            Some(b'"') => {
                self.advance();
                cooked.push('"');
                raw.push_str("\\\"");
            }
            Some(b'`') => {
                self.advance();
                cooked.push('`');
                raw.push_str("\\`");
            }
            Some(b'$') => {
                self.advance();
                cooked.push('$');
                raw.push_str("\\$");
            }
            Some(b'b') => {
                self.advance();
                cooked.push('\u{0008}');
                raw.push_str("\\b");
            }
            Some(b'f') => {
                self.advance();
                cooked.push('\u{000C}');
                raw.push_str("\\f");
            }
            Some(b'v') => {
                self.advance();
                cooked.push('\u{000B}');
                raw.push_str("\\v");
            }
            Some(b'0') => {
                self.advance();
                if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let d = self.advance().unwrap();
                    raw.push('\\');
                    raw.push('0');
                    raw.push(d as char);
                    *valid = false;
                } else {
                    cooked.push('\0');
                    raw.push_str("\\0");
                }
            }
            Some(c) if c.is_ascii_digit() => {
                // Legacy octal / 8 / 9 are not allowed in template literals.
                self.advance();
                raw.push('\\');
                raw.push(c as char);
                *valid = false;
            }
            Some(b'x') => {
                self.advance();
                match self.read_template_hex_escape() {
                    Some(ch) => {
                        cooked.push(ch);
                        raw.push_str(
                            std::str::from_utf8(&self.src[raw_start..self.pos]).unwrap_or(""),
                        );
                    }
                    None => {
                        raw.push_str(
                            std::str::from_utf8(&self.src[raw_start..self.pos]).unwrap_or(""),
                        );
                        *valid = false;
                    }
                }
            }
            Some(b'u') => {
                self.advance();
                match self.read_template_unicode_escape() {
                    Some(ch) => {
                        cooked.push(ch);
                        raw.push_str(
                            std::str::from_utf8(&self.src[raw_start..self.pos]).unwrap_or(""),
                        );
                    }
                    None => {
                        raw.push_str(
                            std::str::from_utf8(&self.src[raw_start..self.pos]).unwrap_or(""),
                        );
                        *valid = false;
                    }
                }
            }
            Some(c) => {
                // NonEscapeCharacter (e.g. \\z): represents the char itself.
                self.advance();
                cooked.push(c as char);
                raw.push('\\');
                raw.push(c as char);
            }
            None => {
                raw.push('\\');
                *valid = false;
            }
        }
    }

    /// Read the next segment of a template literal, starting at the current position
    /// (after the opening backtick or after a `}` that closed an interpolation).
    /// Read the next segment of a template literal, starting at the current position
    /// (after the opening backtick or after a `}` that closed an interpolation).
    fn read_template_segment(
        &mut self,
        line: usize,
        col: usize,
        preceded_by_newline: bool,
    ) -> Token {
        let mut cooked = String::new();
        let mut raw = String::new();
        let mut valid = true;
        while let Some(c) = self.peek() {
            if c == b'`' {
                self.advance();
                // Restore an outer template context when a nested template
                // literal inside an interpolation closes.
                if let Some((state, depth)) = self.template_stack.pop() {
                    self.template_state = state;
                    self.template_expr_depth = depth;
                } else {
                    self.template_state = 0;
                }
                break;
            }
            if c == b'$' && self.peek_at(1) == Some(b'{') {
                self.advance();
                self.advance();
                self.template_state = 1;
                let cooked = if valid { Some(cooked) } else { None };
                let mut tok = Token::new(TokenKind::TemplateString { cooked, raw }, line, col);
                tok.preceded_by_newline = preceded_by_newline;
                return tok;
            }
            if c == b'\\' {
                self.read_template_escape(&mut raw, &mut cooked, &mut valid);
            } else {
                self.advance();
                if c < 0x80 {
                    cooked.push(c as char);
                    raw.push(c as char);
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
                        cooked.push_str(st);
                        raw.push_str(st);
                    }
                }
            }
        }
        // closed the template literal with a backtick: return to normal scanning.
        // (State was already set when the closing backtick was consumed.)
        let cooked = if valid { Some(cooked) } else { None };
        let mut tok = Token::new(TokenKind::TemplateString { cooked, raw }, line, col);
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

pub(crate) fn validate_regex_literal(pattern: &str, flags: &str) -> Result<(), String> {
    validate_regex_flags(flags)?;
    validate_regex_unicode_mode_syntax(pattern, flags)?;
    validate_regex_quantifier_positions(pattern)?;
    validate_regex_assertion_quantifiers(pattern, flags)?;
    validate_regex_modifier_groups(pattern)
}

fn validate_regex_flags(flags: &str) -> Result<(), String> {
    let mut seen = Vec::new();
    for ch in flags.chars() {
        if !matches!(ch, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
            return Err(format!("invalid regular expression flag '{}'", ch));
        }
        if seen.contains(&ch) {
            return Err(format!("duplicate regular expression flag '{}'", ch));
        }
        seen.push(ch);
    }
    Ok(())
}

fn validate_regex_unicode_mode_syntax(pattern: &str, flags: &str) -> Result<(), String> {
    if !(flags.contains('u') || flags.contains('v')) {
        return Ok(());
    }

    let chars: Vec<char> = pattern.chars().collect();
    let capture_count = count_regex_captures(&chars);
    let mut i = 0usize;
    let mut in_class = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            let Some(next) = chars.get(i + 1).copied() else {
                return Err("invalid regular expression escape".to_string());
            };
            match next {
                'u' => {
                    i = validate_regex_unicode_escape_at(&chars, i + 1)?;
                    continue;
                }
                'c' => {
                    if !chars.get(i + 2).is_some_and(|ch| ch.is_ascii_alphabetic()) {
                        return Err("invalid regular expression escape".to_string());
                    }
                    i += 3;
                    continue;
                }
                '0' => {
                    if chars.get(i + 2).is_some_and(|ch| ch.is_ascii_digit()) {
                        return Err("invalid regular expression decimal escape".to_string());
                    }
                    i += 2;
                    continue;
                }
                'p' | 'P' => {
                    i = validate_regex_unicode_property_escape_at(&chars, i + 1)?;
                    continue;
                }
                '1'..='9' => {
                    let (value, end) = read_regex_decimal_escape(&chars, i + 1);
                    if value == 0 || value > capture_count {
                        return Err("invalid regular expression decimal escape".to_string());
                    }
                    i = end;
                    continue;
                }
                'f' | 'n' | 'r' | 't' | 'v' | 'b' | 'B' | 'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                    i += 2;
                    continue;
                }
                '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}'
                | '|' | '/' => {
                    i += 2;
                    continue;
                }
                '-' if in_class => {
                    i += 2;
                    continue;
                }
                _ => return Err("invalid regular expression identity escape".to_string()),
            }
        }

        if in_class {
            if ch == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }

        match ch {
            '[' => in_class = true,
            '{' if braced_quantifier_end(&chars, i).is_none() => {
                return Err("invalid regular expression pattern character".to_string());
            }
            _ => {}
        }

        i += 1;
    }

    validate_regex_unicode_class_ranges(&chars)
}

fn count_regex_captures(chars: &[char]) -> u32 {
    let mut count = 0u32;
    let mut i = 0usize;
    let mut in_class = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if in_class {
            if ch == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }

        match ch {
            '[' => in_class = true,
            '(' if regex_group_is_capturing(chars, i) => count += 1,
            _ => {}
        }

        i += 1;
    }

    count
}

fn regex_group_is_capturing(chars: &[char], idx: usize) -> bool {
    if chars.get(idx + 1) != Some(&'?') {
        return true;
    }
    match chars.get(idx + 2).copied() {
        Some(':') | Some('=') | Some('!') => false,
        Some('<') if matches!(chars.get(idx + 3), Some('=') | Some('!')) => false,
        _ => true,
    }
}

fn read_regex_decimal_escape(chars: &[char], start: usize) -> (u32, usize) {
    let mut idx = start;
    let mut value = 0u32;
    while let Some(ch) = chars.get(idx).copied() {
        let Some(digit) = ch.to_digit(10) else {
            break;
        };
        value = value.saturating_mul(10).saturating_add(digit);
        idx += 1;
    }
    (value, idx)
}

fn validate_regex_unicode_escape_at(chars: &[char], idx: usize) -> Result<usize, String> {
    debug_assert_eq!(chars.get(idx), Some(&'u'));
    if chars.get(idx + 1) == Some(&'{') {
        let mut scan = idx + 2;
        let mut value = 0u32;
        let mut saw_digit = false;
        while let Some(ch) = chars.get(scan).copied() {
            if ch == '}' {
                if !saw_digit || value > 0x10FFFF {
                    return Err("invalid regular expression unicode escape".to_string());
                }
                return Ok(scan + 1);
            }
            let Some(digit) = ch.to_digit(16) else {
                return Err("invalid regular expression unicode escape".to_string());
            };
            saw_digit = true;
            value = value.saturating_mul(16).saturating_add(digit);
            if value > 0x10FFFF {
                return Err("invalid regular expression unicode escape".to_string());
            }
            scan += 1;
        }
        return Err("invalid regular expression unicode escape".to_string());
    }

    for offset in 1..=4 {
        if !chars
            .get(idx + offset)
            .is_some_and(|ch| ch.is_ascii_hexdigit())
        {
            return Err("invalid regular expression unicode escape".to_string());
        }
    }
    Ok(idx + 5)
}

#[derive(Clone, Copy)]
struct RegexClassAtom {
    end: usize,
    is_character_set: bool,
}

fn validate_regex_unicode_class_ranges(chars: &[char]) -> Result<(), String> {
    let mut i = 0usize;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch != '[' {
            i += 1;
            continue;
        }

        i += 1;
        let class_start = i;
        while i < chars.len() && chars[i] != ']' {
            let Some(left) = regex_class_atom_at(chars, i) else {
                break;
            };
            if chars.get(left.end) == Some(&'-')
                && chars.get(left.end + 1).is_some()
                && chars.get(left.end + 1) != Some(&']')
            {
                if let Some(right) = regex_class_atom_at(chars, left.end + 1) {
                    if left.is_character_set || right.is_character_set {
                        return Err("invalid regular expression character class range".to_string());
                    }
                }
            }
            i = left.end.max(i + 1);
        }

        if i == class_start {
            continue;
        }
    }

    Ok(())
}

fn regex_class_atom_at(chars: &[char], idx: usize) -> Option<RegexClassAtom> {
    match chars.get(idx).copied()? {
        '\\' => {
            let escaped = chars.get(idx + 1).copied()?;
            let end = if escaped == 'u' {
                validate_regex_unicode_escape_at(chars, idx + 1).unwrap_or(idx + 2)
            } else if escaped == 'c' && chars.get(idx + 2).is_some() {
                idx + 3
            } else {
                idx + 2
            };
            Some(RegexClassAtom {
                end,
                is_character_set: matches!(escaped, 'd' | 'D' | 's' | 'S' | 'w' | 'W'),
            })
        }
        ']' => None,
        _ => Some(RegexClassAtom {
            end: idx + 1,
            is_character_set: false,
        }),
    }
}

fn validate_regex_quantifier_positions(pattern: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;
    let mut in_class = false;
    let mut escaped = false;
    let mut needs_atom = true;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            needs_atom = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if in_class {
            if ch == ']' {
                in_class = false;
                needs_atom = false;
            }
            i += 1;
            continue;
        }

        if ch == '[' {
            in_class = true;
            i += 1;
            continue;
        }

        if matches!(ch, '*' | '+' | '?') {
            if needs_atom {
                return Err("invalid regular expression quantifier".to_string());
            }
            i += 1;
            continue;
        }

        if ch == '{' {
            if let Some(end) = braced_quantifier_end(&chars, i) {
                if needs_atom {
                    return Err("invalid regular expression quantifier".to_string());
                }
                i = end + 1;
                continue;
            }
        }

        if ch == '|' {
            needs_atom = true;
            i += 1;
            continue;
        }

        needs_atom = false;
        i += 1;
    }

    Ok(())
}

fn braced_quantifier_end(chars: &[char], start: usize) -> Option<usize> {
    debug_assert_eq!(chars.get(start), Some(&'{'));
    let mut idx = start + 1;
    let first_digits_start = idx;

    while chars.get(idx).is_some_and(|ch| ch.is_ascii_digit()) {
        idx += 1;
    }
    if idx == first_digits_start {
        return None;
    }

    match chars.get(idx).copied() {
        Some('}') => Some(idx),
        Some(',') => {
            idx += 1;
            while chars.get(idx).is_some_and(|ch| ch.is_ascii_digit()) {
                idx += 1;
            }
            if chars.get(idx) == Some(&'}') {
                Some(idx)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn validate_regex_assertion_quantifiers(pattern: &str, flags: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let mut i = 0usize;
    let mut in_class = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if in_class {
            if ch == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }

        if ch == '[' {
            in_class = true;
            i += 1;
            continue;
        }

        if ch == '(' && chars.get(i + 1) == Some(&'?') {
            let assertion = match chars.get(i + 2).copied() {
                Some('=') | Some('!') => Some(RegexAssertionKind::Lookahead),
                Some('<') if matches!(chars.get(i + 3), Some('=') | Some('!')) => {
                    Some(RegexAssertionKind::Lookbehind)
                }
                _ => None,
            };

            if let Some(kind) = assertion {
                if let Some(end) = regex_group_end(&chars, i) {
                    if regex_quantifier_starts_at(&chars, end + 1)
                        && (kind == RegexAssertionKind::Lookbehind || unicode_mode)
                    {
                        return Err("invalid regular expression quantifier".to_string());
                    }
                    i = end + 1;
                    continue;
                }
            }
        }

        i += 1;
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RegexAssertionKind {
    Lookahead,
    Lookbehind,
}

fn regex_quantifier_starts_at(chars: &[char], idx: usize) -> bool {
    match chars.get(idx).copied() {
        Some('*' | '+' | '?') => true,
        Some('{') => braced_quantifier_end(chars, idx).is_some(),
        _ => false,
    }
}

fn regex_group_end(chars: &[char], start: usize) -> Option<usize> {
    debug_assert_eq!(chars.get(start), Some(&'('));
    let mut i = start + 1;
    let mut depth = 1usize;
    let mut in_class = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if in_class {
            if ch == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }

        match ch {
            '[' => in_class = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }

        i += 1;
    }

    None
}

fn validate_regex_unicode_property_escape_at(chars: &[char], idx: usize) -> Result<usize, String> {
    if chars.get(idx + 1) != Some(&'{') {
        return Err("invalid regular expression property escape".to_string());
    }
    let mut i = idx + 2;
    let mut property = String::new();
    while let Some(ch) = chars.get(i).copied() {
        if ch == '}' {
            if is_valid_regex_unicode_property_escape(&property) {
                return Ok(i + 1);
            }
            return Err("invalid regular expression property escape".to_string());
        }
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '=') {
            return Err("invalid regular expression property escape".to_string());
        }
        property.push(ch);
        i += 1;
    }
    Err("invalid regular expression property escape".to_string())
}

fn is_valid_regex_unicode_property_escape(property: &str) -> bool {
    if property.is_empty() {
        return false;
    }

    if let Some((name, value)) = property.split_once('=') {
        if value.is_empty() || value.contains('=') {
            return false;
        }
        return match name {
            "General_Category" | "gc" => is_regex_general_category_value(value),
            "Script" | "sc" | "Script_Extensions" | "scx" => is_regex_script_value(value),
            _ => false,
        };
    }

    is_regex_binary_property(property) || is_regex_general_category_value(property)
}

fn is_regex_general_category_value(value: &str) -> bool {
    matches!(
        value,
        "C" | "Other"
            | "Cc"
            | "Control"
            | "cntrl"
            | "Cf"
            | "Format"
            | "Cn"
            | "Unassigned"
            | "Co"
            | "Private_Use"
            | "Cs"
            | "Surrogate"
            | "L"
            | "Letter"
            | "LC"
            | "Cased_Letter"
            | "Ll"
            | "Lowercase_Letter"
            | "Lm"
            | "Modifier_Letter"
            | "Lo"
            | "Other_Letter"
            | "Lt"
            | "Titlecase_Letter"
            | "Lu"
            | "Uppercase_Letter"
            | "M"
            | "Mark"
            | "Combining_Mark"
            | "Mc"
            | "Spacing_Mark"
            | "Me"
            | "Enclosing_Mark"
            | "Mn"
            | "Nonspacing_Mark"
            | "N"
            | "Number"
            | "Nd"
            | "Decimal_Number"
            | "digit"
            | "Nl"
            | "Letter_Number"
            | "No"
            | "Other_Number"
            | "P"
            | "Punctuation"
            | "punct"
            | "Pc"
            | "Connector_Punctuation"
            | "Pd"
            | "Dash_Punctuation"
            | "Pe"
            | "Close_Punctuation"
            | "Pf"
            | "Final_Punctuation"
            | "Pi"
            | "Initial_Punctuation"
            | "Po"
            | "Other_Punctuation"
            | "Ps"
            | "Open_Punctuation"
            | "S"
            | "Symbol"
            | "Sc"
            | "Currency_Symbol"
            | "Sk"
            | "Modifier_Symbol"
            | "Sm"
            | "Math_Symbol"
            | "So"
            | "Other_Symbol"
            | "Z"
            | "Separator"
            | "Zl"
            | "Line_Separator"
            | "Zp"
            | "Paragraph_Separator"
            | "Zs"
            | "Space_Separator"
    )
}

fn is_regex_binary_property(property: &str) -> bool {
    matches!(
        property,
        "ASCII"
            | "Any"
            | "Assigned"
            | "Alphabetic"
            | "Alpha"
            | "ASCII_Hex_Digit"
            | "AHex"
            | "Bidi_Control"
            | "Bidi_C"
            | "Bidi_Mirrored"
            | "Bidi_M"
            | "Case_Ignorable"
            | "CI"
            | "Cased"
            | "Changes_When_Casefolded"
            | "CWCF"
            | "Changes_When_Casemapped"
            | "CWCM"
            | "Changes_When_Lowercased"
            | "CWL"
            | "Changes_When_NFKC_Casefolded"
            | "CWKCF"
            | "Changes_When_Titlecased"
            | "CWT"
            | "Changes_When_Uppercased"
            | "CWU"
            | "Dash"
            | "Default_Ignorable_Code_Point"
            | "DI"
            | "Deprecated"
            | "Dep"
            | "Diacritic"
            | "Dia"
            | "Emoji"
            | "Emoji_Component"
            | "EComp"
            | "Emoji_Modifier"
            | "EMod"
            | "Emoji_Modifier_Base"
            | "EBase"
            | "Emoji_Presentation"
            | "EPres"
            | "Extended_Pictographic"
            | "ExtPict"
            | "Extender"
            | "Ext"
            | "Grapheme_Base"
            | "Gr_Base"
            | "Grapheme_Extend"
            | "Gr_Ext"
            | "Hex_Digit"
            | "Hex"
            | "IDS_Binary_Operator"
            | "IDSB"
            | "IDS_Trinary_Operator"
            | "IDST"
            | "ID_Continue"
            | "IDC"
            | "ID_Start"
            | "IDS"
            | "Ideographic"
            | "Ideo"
            | "Join_Control"
            | "Join_C"
            | "Logical_Order_Exception"
            | "LOE"
            | "Lowercase"
            | "Lower"
            | "Math"
            | "Noncharacter_Code_Point"
            | "NChar"
            | "Pattern_Syntax"
            | "Pat_Syn"
            | "Pattern_White_Space"
            | "Pat_WS"
            | "Quotation_Mark"
            | "QMark"
            | "Radical"
            | "Regional_Indicator"
            | "RI"
            | "Sentence_Terminal"
            | "STerm"
            | "Soft_Dotted"
            | "SD"
            | "Terminal_Punctuation"
            | "Term"
            | "Unified_Ideograph"
            | "UIdeo"
            | "Uppercase"
            | "Upper"
            | "Variation_Selector"
            | "VS"
            | "White_Space"
            | "space"
            | "WSpace"
            | "XID_Continue"
            | "XIDC"
            | "XID_Start"
            | "XIDS"
    )
}

fn is_regex_script_value(value: &str) -> bool {
    regex::Regex::new(&format!(r"\p{{Script={value}}}")).is_ok()
}

fn validate_regex_modifier_groups(pattern: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;
    let mut in_class = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if ch == ']' && in_class {
            in_class = false;
            i += 1;
            continue;
        }

        if !in_class && ch == '(' && chars.get(i + 1) == Some(&'?') {
            match chars.get(i + 2).copied() {
                Some(':') | Some('=') | Some('!') | Some('<') => {
                    i += 2;
                    continue;
                }
                Some(_) => validate_regex_modifier_group_at(&chars, i + 2)?,
                None => {}
            }
        }
        i += 1;
    }

    Ok(())
}

fn validate_regex_modifier_group_at(chars: &[char], mut idx: usize) -> Result<(), String> {
    let mut add = Vec::new();
    let mut remove = Vec::new();
    let mut removing = false;

    while let Some(ch) = chars.get(idx).copied() {
        if ch == ':' {
            if add.is_empty() && remove.is_empty() {
                return Err("invalid regular expression modifiers".to_string());
            }
            if add.iter().any(|ch| remove.contains(ch)) {
                return Err("invalid regular expression modifiers".to_string());
            }
            return Ok(());
        }
        if ch == '-' && !removing {
            removing = true;
            idx += 1;
            continue;
        }
        if !matches!(ch, 'i' | 'm' | 's') {
            return Err("invalid regular expression modifiers".to_string());
        }

        let group = if removing { &mut remove } else { &mut add };
        if group.contains(&ch) {
            return Err("invalid regular expression modifiers".to_string());
        }
        group.push(ch);
        idx += 1;
    }

    Err("invalid regular expression modifiers".to_string())
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
        assert_eq!(kinds("'\\А'"), vec![String("А".into()), Eof]);
        assert_eq!(
            kinds("'\\1\\40\\377'"),
            vec![String("\u{1} \u{ff}".into()), Eof]
        );
        assert_eq!(kinds("\" \""), vec![String("\u{2028}".into()), Eof]);
        assert_eq!(kinds("\" \""), vec![String("\u{2029}".into()), Eof]);
        assert_eq!(
            kinds(concat!("'line\\", "\n", "Continuation'")),
            vec![String("lineContinuation".into()), Eof]
        );
        assert_eq!(
            kinds(concat!("'line\\", "\r\n", "Continuation'")),
            vec![String("lineContinuation".into()), Eof]
        );
    }

    #[test]
    fn string_escape_metadata() {
        let plain = Lexer::new("\"use strict\"").tokens();
        assert_eq!(plain[0].kind, String("use strict".into()));
        assert!(!plain[0].string_had_escape);

        let escaped = Lexer::new("'use\\u0020strict'").tokens();
        assert_eq!(escaped[0].kind, String("use strict".into()));
        assert!(escaped[0].string_had_escape);

        let continued = Lexer::new(concat!("'use\\", "\n", " strict'")).tokens();
        assert_eq!(continued[0].kind, String("use strict".into()));
        assert!(continued[0].string_had_escape);

        let legacy = Lexer::new("'\\1'").tokens();
        assert_eq!(legacy[0].kind, String("\u{1}".into()));
        assert!(legacy[0].string_had_escape);
        assert!(legacy[0].string_had_legacy_escape);

        let non_octal_decimal = Lexer::new("'\\8'").tokens();
        assert_eq!(non_octal_decimal[0].kind, String("8".into()));
        assert!(non_octal_decimal[0].string_had_legacy_escape);
    }

    #[test]
    fn keywords() {
        assert_eq!(kinds("var let const"), vec![Var, Let, Const, Eof]);
        assert_eq!(kinds("function return"), vec![Function, Return, Eof]);
        assert_eq!(
            kinds("v\\u0061r f\\u{61}lse undef\\u0069ned"),
            vec![
                Ident("var".into()),
                Ident("false".into()),
                Ident("undefined".into()),
                Eof,
            ]
        );
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
    fn slash_after_contextual_of() {
        assert_eq!(
            kinds("instance/of/g"),
            vec![
                Ident("instance".into()),
                Slash,
                Of,
                Slash,
                Ident("g".into()),
                Eof,
            ]
        );
        assert_eq!(
            kinds("for (x of /a/) ;"),
            vec![
                For,
                LParen,
                Ident("x".into()),
                Of,
                Regex("a".into(), "".into()),
                RParen,
                Semicolon,
                Eof,
            ]
        );
    }

    #[test]
    fn comments() {
        assert_eq!(kinds("1 // hi\n2"), vec![Number(1.0), Number(2.0), Eof]);
        assert_eq!(kinds("1 /* x */ 2"), vec![Number(1.0), Number(2.0), Eof]);
    }

    #[test]
    fn unicode_space_separators_are_whitespace() {
        assert_eq!(
            kinds("/x/g\u{2000}; /x/g\u{200A}; /x/g\u{202F}; /x/g\u{205F}; /x/g\u{3000}; /x/g\u{FEFF};"),
            vec![
                Regex("x".into(), "g".into()),
                Semicolon,
                Regex("x".into(), "g".into()),
                Semicolon,
                Regex("x".into(), "g".into()),
                Semicolon,
                Regex("x".into(), "g".into()),
                Semicolon,
                Regex("x".into(), "g".into()),
                Semicolon,
                Regex("x".into(), "g".into()),
                Semicolon,
                Eof,
            ]
        );
    }

    #[test]
    fn regex_flags_and_modifiers_report_early_errors() {
        assert!(matches!(
            Lexer::new("/./G").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression flag")
        ));
        assert!(matches!(
            Lexer::new("/./gig").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression flag")
        ));
        assert!(matches!(
            Lexer::new("/(?i-i:a)/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression modifiers")
        ));
        assert!(matches!(
            Lexer::new("/(?u:a)/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression modifiers")
        ));
        assert!(matches!(
            Lexer::new("/(?ii:a)/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression modifiers")
        ));
        assert!(matches!(
            Lexer::new("/(?i)/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("regular expression modifiers")
        ));
        for source in ["/?/", "/+/", "/{2}/", "/{2,}/", "/{2,3}/"] {
            assert!(matches!(
                Lexer::new(source).tokens()[0].kind,
                LexError(ref msg) if msg.contains("regular expression quantifier")
            ));
        }
        for source in [
            "/(?<=a)?/",
            "/(?<!a){2,3}/",
            "/(?=a)?/u",
            "/(?!a){2,3}/u",
            "/(?<=a)?/u",
            "/(?<!a){2,3}/u",
        ] {
            assert!(matches!(
                Lexer::new(source).tokens()[0].kind,
                LexError(ref msg) if msg.contains("regular expression quantifier")
            ));
        }
        for source in [
            "/\\c0/u",
            "/{/u",
            "/\\M/u",
            "/\\1/u",
            "/[\\d-a]/u",
            "/[\\s-\\d]/u",
            "/[%-\\d]/u",
            "/[--\\d]/u",
            "/\\8/u",
            "/\\u{110000}/u",
            "/\\u{1,}/u",
            "/\\u{1F_639}/u",
        ] {
            assert!(matches!(
                Lexer::new(source).tokens()[0].kind,
                LexError(ref msg) if msg.contains("regular expression")
            ));
        }
        assert_eq!(
            kinds("/(?i:a)/; /(?im-s:a)/; /(?:a)/; /(?=a)/; /(?!a)/; /(?=a)?/; /a?/; /a{2}/; /\\?/; /\\{2\\}/; /[?]/; /\\u{41}/u; /(a)\\1/u; /[a-\\-]/u;"),
            vec![
                Regex("(?i:a)".into(), "".into()),
                Semicolon,
                Regex("(?im-s:a)".into(), "".into()),
                Semicolon,
                Regex("(?:a)".into(), "".into()),
                Semicolon,
                Regex("(?=a)".into(), "".into()),
                Semicolon,
                Regex("(?!a)".into(), "".into()),
                Semicolon,
                Regex("(?=a)?".into(), "".into()),
                Semicolon,
                Regex("a?".into(), "".into()),
                Semicolon,
                Regex("a{2}".into(), "".into()),
                Semicolon,
                Regex("\\?".into(), "".into()),
                Semicolon,
                Regex("\\{2\\}".into(), "".into()),
                Semicolon,
                Regex("[?]".into(), "".into()),
                Semicolon,
                Regex("\\u{41}".into(), "u".into()),
                Semicolon,
                Regex("(a)\\1".into(), "u".into()),
                Semicolon,
                Regex("[a-\\-]".into(), "u".into()),
                Semicolon,
                Eof,
            ]
        );
    }

    #[test]
    fn regex_literals_preserve_utf8_pattern_source() {
        assert_eq!(
            kinds("/\\0②/u; /\u{80}/; /[፬]/u;"),
            vec![
                Regex("\\0②".into(), "u".into()),
                Semicolon,
                Regex("\u{80}".into(), "".into()),
                Semicolon,
                Regex("[፬]".into(), "u".into()),
                Semicolon,
                Eof,
            ]
        );
    }

    #[test]
    fn unicode_identifier_tables_follow_es_identifier_properties() {
        assert_eq!(
            kinds("var ℘; var ゛; var ᢅ; var _\u{200C}\u{200D}\u{30FB}\u{FF65};"),
            vec![
                Var,
                Ident("℘".into()),
                Semicolon,
                Var,
                Ident("゛".into()),
                Semicolon,
                Var,
                Ident("ᢅ".into()),
                Semicolon,
                Var,
                Ident("_\u{200C}\u{200D}\u{30FB}\u{FF65}".into()),
                Semicolon,
                Eof,
            ]
        );
        assert!(matches!(
            kinds("var \u{2E2F};")[1],
            LexError(ref msg) if msg.contains("invalid identifier start")
        ));
        assert!(matches!(
            kinds("var a\\u2E2F;")[1],
            LexError(ref msg) if msg.contains("invalid identifier continue")
        ));
    }

    #[test]
    fn private_names_follow_identifier_name_grammar() {
        assert_eq!(
            kinds("class C { #\\u{6F}; #℘; #ZW_\u{200C}_NJ; #ZW_\u{200D}_J; }"),
            vec![
                Class,
                Ident("C".into()),
                LBrace,
                PrivateName("o".into()),
                Semicolon,
                PrivateName("℘".into()),
                Semicolon,
                PrivateName("ZW_\u{200C}_NJ".into()),
                Semicolon,
                PrivateName("ZW_\u{200D}_J".into()),
                Semicolon,
                RBrace,
                Eof,
            ]
        );
        assert!(matches!(
            kinds("class C { #0; }")[3],
            LexError(ref msg) if msg.contains("invalid private name start")
        ));
    }

    #[test]
    fn comments_respect_line_terminators_and_errors() {
        assert_eq!(kinds("// hi\u{2028}42"), vec![Number(42.0), Eof]);
        assert_eq!(kinds("// hi\u{2029}42"), vec![Number(42.0), Eof]);
        assert_eq!(kinds("// hi\u{0085}42"), vec![Eof]);
        assert!(matches!(
            kinds("/* unterminated")[0],
            LexError(ref msg) if msg.contains("unterminated multiline comment")
        ));
        assert!(matches!(
            kinds("x*/")[2],
            LexError(ref msg) if msg.contains("unterminated regular expression")
        ));
        assert!(matches!(
            Lexer::new(concat!("/\\", "\n", "/")).tokens()[0].kind,
            LexError(ref msg) if msg.contains("unterminated regular expression")
        ));
        assert!(matches!(
            Lexer::new(concat!("/a\\", "\r", "/")).tokens()[0].kind,
            LexError(ref msg) if msg.contains("unterminated regular expression")
        ));
        assert!(matches!(
            Lexer::new("/\\\u{2028}/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("unterminated regular expression")
        ));
        assert!(matches!(
            Lexer::new("/a\\\u{2029}/").tokens()[0].kind,
            LexError(ref msg) if msg.contains("unterminated regular expression")
        ));

        let tokens = Lexer::new("a/*\u{2028}*/b").tokens();
        assert!(tokens[1].preceded_by_newline);
    }
}
