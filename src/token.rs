use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    /// A non-strict legacy decimal/octal literal such as `077` or `08`.
    /// Parsers must reject this token in strict-mode code.
    LegacyNumber(f64),
    BigInt(String),
    String(String),
    TemplateString {
        /// Cooked template value. `None` when the segment contains an invalid
        /// escape sequence (allowed in tagged templates; SyntaxError in
        /// untagged templates).
        cooked: Option<String>,
        /// Raw template text including backslashes and escape sequences as
        /// they appear in source.
        raw: String,
    },
    Ident(String),
    TemplateExprStart, // ${
    TemplateExprEnd,   // } inside template

    // Keywords
    Var,
    Let,
    Const,
    Function,
    Return,
    If,
    Else,
    While,
    With,
    For,
    Do,
    Break,
    Continue,
    Debugger,
    Null,
    True,
    False,
    Undefined,
    New,
    This,
    Typeof,
    Instanceof,
    In,
    Of,
    Delete,
    Void,
    Throw,
    Try,
    Catch,
    Finally,
    Switch,
    Class,
    Extends,
    Static,
    Get,
    Set,
    Super,
    Async,
    Await,
    Yield,
    Case,
    Default,
    BreakLabel(String),

    // Operators
    Plus,
    Minus,
    Inc, // ++
    Dec, // --
    Star,
    Slash,
    Percent,
    StarStar, // **
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    StarStarAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    ShlAssign,
    ShrAssign,
    UshrAssign,
    Eq,
    NotEq,
    EqEqEq,
    NotEqEqEq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
    Not,
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,
    Ushr,
    Nullish, // ??
    Question,
    QuestionDot,           // ?.
    Regex(String, String), // /pattern/flags
    Colon,
    Dot,
    Spread, // ...
    Arrow,  // =>
    NullishAssign,
    AndAssign,
    OrAssign,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    At,
    /// Private name: `#field`. The string excludes the `#`.
    PrivateName(String),

    // Special
    Eof,
    /// A lexer-level error (e.g. an invalid escape sequence in a
    /// string/template literal). Parsers must turn this into a SyntaxError.
    LexError(String),
    /// A valid construct exceeded a bounded lexer-owned resource policy.
    /// Parsers must preserve this as a RangeError.
    ResourceError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
    /// True when a newline appeared immediately before this token.
    pub preceded_by_newline: bool,
    /// True when the identifier contained a Unicode escape sequence.
    pub had_escape: bool,
    /// True when a string literal contained an escape sequence or line
    /// continuation. Such literals do not form `"use strict"` directives.
    pub string_had_escape: bool,
    /// True when a string literal contained a legacy octal or non-octal
    /// decimal escape. Strict mode code must reject these escapes.
    pub string_had_legacy_escape: bool,
    /// True when a string escape produced an unpaired UTF-16 surrogate.
    pub string_not_well_formed: bool,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token {
            kind,
            line,
            col,
            preceded_by_newline: false,
            had_escape: false,
            string_had_escape: false,
            string_had_legacy_escape: false,
            string_not_well_formed: false,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number(n) => write!(f, "{}", n),
            TokenKind::LegacyNumber(n) => write!(f, "{}", n),
            TokenKind::String(s) => write!(f, "\"{}\"", s),
            TokenKind::Ident(s) => write!(f, "{}", s),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl TokenKind {
    /// If this token is a reserved word usable as a property/identifier name
    /// (`return`, `class`, `default`, ...), return its source spelling.
    pub fn as_keyword_str(&self) -> Option<&'static str> {
        Some(match self {
            TokenKind::Var => "var",
            TokenKind::Let => "let",
            TokenKind::Const => "const",
            TokenKind::Function => "function",
            TokenKind::Class => "class",
            TokenKind::Extends => "extends",
            TokenKind::Static => "static",
            TokenKind::Return => "return",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::While => "while",
            TokenKind::With => "with",
            TokenKind::For => "for",
            TokenKind::Do => "do",
            TokenKind::Break => "break",
            TokenKind::Continue => "continue",
            TokenKind::Debugger => "debugger",
            TokenKind::Null => "null",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Undefined => "undefined",
            TokenKind::New => "new",
            TokenKind::This => "this",
            TokenKind::Super => "super",
            TokenKind::Typeof => "typeof",
            TokenKind::Instanceof => "instanceof",
            TokenKind::In => "in",
            TokenKind::Of => "of",
            TokenKind::Delete => "delete",
            TokenKind::Void => "void",
            TokenKind::Throw => "throw",
            TokenKind::Try => "try",
            TokenKind::Catch => "catch",
            TokenKind::Finally => "finally",
            TokenKind::Switch => "switch",
            TokenKind::Case => "case",
            TokenKind::Default => "default",
            TokenKind::Async => "async",
            TokenKind::Await => "await",
            TokenKind::Yield => "yield",
            TokenKind::Get => "get",
            TokenKind::Set => "set",
            _ => return None,
        })
    }
}
