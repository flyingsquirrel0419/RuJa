use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassExpr {
    pub name: Option<Rc<str>>,
    pub superclass: Option<Box<Expr>>,
    pub methods: Vec<ClassMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMethod {
    pub name: Rc<str>,
    pub params: Vec<Rc<str>>,
    pub body: Vec<Stmt>,
    pub is_static: bool,
    pub is_constructor: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(Rc<str>),
    TemplateStr(Rc<str>),
    TemplateTagged(Box<Expr>, Vec<Expr>),
    Bool(bool),
    Null,
    Undefined,
    Ident(Rc<str>),
    This,
    Super,
    Array(Vec<Expr>),
    Object(Vec<Property>),
    Function(FunctionExpr),
    Arrow(FunctionExpr),
    Class(ClassExpr),
    Unary(UnOp, Box<Expr>),
    Update(UpdateOp, bool, Box<Expr>), // op, prefix, expr
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Logical(LogicalOp, Box<Expr>, Box<Expr>),
    Assign(AssignOp, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>), // cond ? then : else
    Call { callee: Box<Expr>, args: Vec<Expr> },
    New { callee: Box<Expr>, args: Vec<Expr> },
    Member { object: Box<Expr>, property: Box<Expr>, computed: bool },
    Spread(Box<Expr>),
    Sequence(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub key: PropertyKey,
    pub value: Expr,
    pub computed: bool,
    pub method: bool,
    pub shorthand: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKey {
    Ident(Rc<str>),
    String(Rc<str>),
    Number(f64),
    Computed(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionExpr {
    pub name: Option<Rc<str>>,
    pub params: Vec<Rc<str>>,
    pub body: Vec<Stmt>,
    pub is_arrow: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub param_decls: Vec<Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Ident(Rc<str>),
    Array(Vec<Pattern>),
    Object(Vec<(Rc<str>, Pattern)>),
    Assign(Box<Pattern>, Expr),
    Rest(Box<Pattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateOp {
    Inc,
    Dec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Gt,
    Lte,
    Gte,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Ushr,
    In,
    Instanceof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
    Nullish,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    PowAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    ShlAssign,
    ShrAssign,
    UshrAssign,
    AndAssign,
    OrAssign,
    NullishAssign,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl { kind: VarKind, decls: Vec<(Rc<str>, Option<Expr>)> },
    ExprStmt(Expr),
    Block(Vec<Stmt>),
    If { cond: Expr, then: Box<Stmt>, else_: Option<Box<Stmt>> },
    While { cond: Expr, body: Box<Stmt> },
    DoWhile { body: Box<Stmt>, cond: Expr },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    ForIn { left: Box<Stmt>, right: Expr, body: Box<Stmt> },
    ForOf { left: Box<Stmt>, right: Expr, body: Box<Stmt> },
    Break(Option<Rc<str>>),
    Continue(Option<Rc<str>>),
    Return(Option<Expr>),
    Throw(Expr),
    TryCatch { try_body: Box<Stmt>, catch_param: Option<Rc<str>>, catch_body: Box<Stmt>, finally_body: Option<Box<Stmt>> },
    FunctionDecl(FunctionExpr),
    Labeled(Rc<str>, Box<Stmt>),
    Empty,
    Switch { disc: Expr, cases: Vec<SwitchCase> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expr>, // None = default
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VarKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Vec<Stmt>,
}
