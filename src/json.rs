use crate::error::{self, Error};
use crate::interpreter::Interpreter;
use crate::value::{InternalData, Obj, PropertyDescriptor, Value};
use std::cell::RefCell;
use std::rc::Rc;

pub struct JsonParser {
    pub chars: Vec<char>,
    pub pos: usize,
}

impl JsonParser {
    pub fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    pub fn parse_value(&mut self, interp: &mut Interpreter) -> error::Result<Value> {
        self.skip_ws();
        if self.pos >= self.chars.len() {
            return Err(Error::syntax("Unexpected end of JSON input".to_string()));
        }
        let c = self.chars[self.pos];
        match c {
            '{' => self.parse_object(interp),
            '[' => self.parse_array(interp),
            '"' => self.parse_string(),
            't' | 'f' => self.parse_bool(),
            'n' => self.parse_null(),
            c if c == '-' || c.is_ascii_digit() => self.parse_number(),
            _ => Err(Error::syntax(format!("Unexpected character '{}' in JSON", c))),
        }
    }

    fn parse_object(&mut self, interp: &mut Interpreter) -> error::Result<Value> {
        self.pos += 1; // {
        let mut obj = Obj::new();
        self.skip_ws();
        if self.pos < self.chars.len() && self.chars[self.pos] == '}' {
            self.pos += 1;
            return Ok(Value::Object(Rc::new(RefCell::new(obj))));
        }
        loop {
            self.skip_ws();
            let key_val = self.parse_string()?;
            let key = if let Value::String(s) = &key_val { s.to_string() } else { unreachable!() };
            self.skip_ws();
            if self.pos >= self.chars.len() || self.chars[self.pos] != ':' {
                return Err(Error::syntax("Expected ':' in JSON object".to_string()));
            }
            self.pos += 1;
            let val = self.parse_value(interp)?;
            obj.props.insert(Rc::from(key.as_str()), PropertyDescriptor::data(val));
            self.skip_ws();
            if self.pos >= self.chars.len() { return Err(Error::syntax("Unterminated JSON object".to_string())); }
            match self.chars[self.pos] {
                ',' => { self.pos += 1; continue; }
                '}' => { self.pos += 1; break; }
                c => return Err(Error::syntax(format!("Expected ',' or '}}' got '{}'", c))),
            }
        }
        Ok(Value::Object(Rc::new(RefCell::new(obj))))
    }

    fn parse_array(&mut self, interp: &mut Interpreter) -> error::Result<Value> {
        self.pos += 1; // [
        let mut items = Vec::new();
        self.skip_ws();
        if self.pos < self.chars.len() && self.chars[self.pos] == ']' {
            self.pos += 1;
            let mut o = Obj::new_array();
            o.internal = InternalData::Array(items);
            return Ok(Value::Object(Rc::new(RefCell::new(o))));
        }
        loop {
            let v = self.parse_value(interp)?;
            items.push(v);
            self.skip_ws();
            if self.pos >= self.chars.len() { return Err(Error::syntax("Unterminated JSON array".to_string())); }
            match self.chars[self.pos] {
                ',' => { self.pos += 1; continue; }
                ']' => { self.pos += 1; break; }
                c => return Err(Error::syntax(format!("Expected ',' or ']' got '{}'", c))),
            }
        }
        let mut o = Obj::new_array();
        o.internal = InternalData::Array(items);
        Ok(Value::Object(Rc::new(RefCell::new(o))))
    }

    fn parse_string(&mut self) -> error::Result<Value> {
        self.pos += 1; // "
        let mut s = String::new();
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            match c {
                '"' => return Ok(Value::String(Rc::from(s.as_str()))),
                '\\' => {
                    if self.pos >= self.chars.len() { break; }
                    let e = self.chars[self.pos];
                    self.pos += 1;
                    match e {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '/' => s.push('/'),
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        'b' => s.push('\u{0008}'),
                        'f' => s.push('\u{000C}'),
                        'u' => {
                            let hex: String = (0..4).filter_map(|_| {
                                if self.pos < self.chars.len() {
                                    let c = self.chars[self.pos];
                                    self.pos += 1;
                                    Some(c)
                                } else { None }
                            }).collect();
                            if let Ok(n) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(n) { s.push(ch); }
                            }
                        }
                        c => s.push(c),
                    }
                }
                c => s.push(c),
            }
        }
        Err(Error::syntax("Unterminated string in JSON".to_string()))
    }

    fn parse_bool(&mut self) -> error::Result<Value> {
        if self.chars[self.pos..].starts_with(&['t','r','u','e']) {
            self.pos += 4;
            Ok(Value::Bool(true))
        } else if self.chars[self.pos..].starts_with(&['f','a','l','s','e']) {
            self.pos += 5;
            Ok(Value::Bool(false))
        } else {
            Err(Error::syntax("Invalid JSON boolean".to_string()))
        }
    }

    fn parse_null(&mut self) -> error::Result<Value> {
        if self.chars[self.pos..].starts_with(&['n','u','l','l']) {
            self.pos += 4;
            Ok(Value::Null)
        } else {
            Err(Error::syntax("Invalid JSON null".to_string()))
        }
    }

    fn parse_number(&mut self) -> error::Result<Value> {
        let start = self.pos;
        if self.chars[self.pos] == '-' { self.pos += 1; }
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-' {
                self.pos += 1;
            } else { break; }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        Ok(Value::Number(s.parse::<f64>().map_err(|_| Error::syntax("Invalid JSON number".to_string()))?))
    }
}
