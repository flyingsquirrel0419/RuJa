use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    String(String),
    TemplateString(String),
    Ident(String),
    TemplateExprStart,  // ${
    TemplateExprEnd,    // } inside template

    // Keywords
    Var,
    Let,
    Const,
    Function,
    Return,
    If,
    Else,
    While,
    For,
    Do,
    Break,
    Continue,
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
    Case,
    Default,
    BreakLabel(String),

    // Operators
    Plus,
    Minus,
    Inc,        // ++
    Dec,        // --
    Star,
    Slash,
    Percent,
    StarStar,        // **
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
    Nullish,         // ??
    Question,
    Colon,
    Dot,
    Spread,          // ...
    Arrow,           // =>
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

    // Special
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
    /// True when a newline appeared immediately before this token.
    pub preceded_by_newline: bool,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token { kind, line, col, preceded_by_newline: false }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number(n) => write!(f, "{}", n),
            TokenKind::String(s) => write!(f, "\"{}\"", s),
            TokenKind::Ident(s) => write!(f, "{}", s),
            _ => write!(f, "{:?}", self),
        }
    }
}
