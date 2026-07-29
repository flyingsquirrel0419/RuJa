use num_bigint::BigInt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ClassExpr {
    /// Decorator expressions in source order.
    pub decorators: Vec<Expr>,
    pub name: Option<Arc<str>>,
    pub inferred_name: Option<Arc<str>>,
    pub is_declaration: bool,
    pub superclass: Option<Box<Expr>>,
    pub elements: Vec<ClassElement>,
    pub methods: Vec<ClassMethod>,
    /// Static initialization blocks: `static { ... }`. Each runs with `this`
    /// bound to the class (constructor), in source order, at class definition
    /// time.
    pub static_blocks: Vec<Vec<Stmt>>,
    /// Private instance field declarations: `#name = init`.
    pub private_fields: Vec<PrivateFieldDecl>,
    /// Public field declarations: `name = init`, `static name = init`, `[key] = init`.
    pub public_fields: Vec<PublicFieldDecl>,
    /// Auto-accessor declarations: `accessor name = init`.
    pub auto_accessors: Vec<AutoAccessorDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassElement {
    Method(usize),
    StaticBlock(usize),
    PrivateField(usize),
    PublicField(usize),
    AutoAccessor(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMethod {
    /// Decorator expressions in source order.
    pub decorators: Vec<Expr>,
    pub name: Arc<str>,
    /// Computed property name expression (when `name` is from `[expr]`).
    /// None means the name is a static string.
    pub computed_name: Option<Box<Expr>>,
    pub params: Vec<Arc<str>>,
    pub param_defaults: Vec<Option<Expr>>,
    pub rest_param: Option<Arc<str>>,
    pub body: Vec<Stmt>,
    pub is_static: bool,
    pub is_constructor: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub kind: PropKind,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    BigInt(BigInt),
    String(Arc<str>),
    TemplateStr(Arc<str>),
    /// Tagged template: `tag`...`` — calls tag(strings, raw, ...exprs).
    TaggedTemplate {
        tag: Box<Expr>,
        /// Cooked template segments. A segment is `None` when it contains an
        /// invalid escape sequence, which is allowed in tagged templates and
        /// yields `undefined` for that element.
        quasis: Vec<Option<Arc<str>>>,
        raw: Vec<Arc<str>>,
        exprs: Vec<Expr>,
    },
    TemplateInterp {
        quasis: Vec<Arc<str>>,
        exprs: Vec<Expr>,
    },
    Bool(bool),
    Null,
    Undefined,
    Ident(Arc<str>),
    This,
    Super,
    ArrayHole,
    Array(Vec<Expr>),
    Object(Vec<Property>),
    Function(FunctionExpr),
    Arrow(FunctionExpr),
    Class(ClassExpr),
    /// Private field read: `obj.#name`.
    PrivateGet {
        object: Box<Expr>,
        name: Arc<str>,
        optional: bool,
    },
    /// Private field write: `obj.#name = value`.
    PrivateSet {
        object: Box<Expr>,
        name: Arc<str>,
        value: Box<Expr>,
    },
    /// Private field/method initialization performed by class element setup.
    PrivateInit {
        object: Box<Expr>,
        name: Arc<str>,
        value: Box<Expr>,
        kind: PropKind,
    },
    /// Private accessor declaration lowered into constructor/static init code.
    PrivateDefineAccessor {
        object: Box<Expr>,
        name: Arc<str>,
        get: Option<Box<Expr>>,
        set: Option<Box<Expr>>,
    },
    /// Private field declaration: `#name = init` in a class body.
    PrivateFieldDecl {
        name: Arc<str>,
        init: Option<Box<Expr>>,
    },
    /// Public field initialization performed by class construction/evaluation.
    PublicFieldInit {
        object: Box<Expr>,
        name: Arc<str>,
        computed_name: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    Unary(UnOp, Box<Expr>),
    Update(UpdateOp, bool, Box<Expr>), // op, prefix, expr
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Logical(LogicalOp, Box<Expr>, Box<Expr>),
    Assign(AssignOp, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>), // cond ? then : else
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        optional: bool,
        optional_chain: bool,
    },
    /// Compiler-generated direct call with an explicit `this` value.
    CallWithThis {
        callee: Box<Expr>,
        this_value: Box<Expr>,
        args: Vec<Expr>,
    },
    /// Compiler-generated body of a decorator context's `addInitializer`.
    DecoratorAddInitializer {
        initializer: Box<Expr>,
        active_binding: Arc<str>,
        queue_binding: Arc<str>,
    },
    /// Compiler-generated decorator access operation. The key may be a public
    /// property key or a private-name identity. Kind: 0 has, 1 get, 2 set.
    DecoratorAccess {
        receiver: Box<Expr>,
        key: Box<Expr>,
        value: Option<Box<Expr>>,
        kind: u8,
    },
    ImportCall {
        specifier: Box<Expr>,
        options: Option<Box<Expr>>,
    },
    ImportMeta,
    New {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    NewTarget,
    Member {
        object: Box<Expr>,
        property: Box<Expr>,
        computed: bool,
        optional: bool,
        optional_chain: bool,
    },
    /// A complete optional-chain boundary. Inner Member/Call/PrivateGet nodes
    /// share one short-circuit target; nested chains in keys/arguments get
    /// their own wrapper.
    OptionalChain(Box<Expr>),
    Spread(Box<Expr>),
    Sequence(Vec<Expr>),
    Regex(Arc<str>, Arc<str>),
    Await(Box<Expr>),
    Yield(Option<Box<Expr>>),
    /// `yield* expr` - delegate to another iterable/generator, forwarding each
    /// yielded value to the outer generator.
    YieldDelegate(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrivateFieldDecl {
    pub decorators: Vec<Expr>,
    pub name: Arc<str>,
    pub init: Option<Box<Expr>>,
    pub is_static: bool,
    pub kind: PropKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublicFieldDecl {
    pub decorators: Vec<Expr>,
    pub name: Arc<str>,
    pub computed_name: Option<Box<Expr>>,
    pub init: Option<Box<Expr>>,
    pub is_static: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoAccessorDecl {
    pub decorators: Vec<Expr>,
    pub name: Arc<str>,
    pub computed_name: Option<Box<Expr>>,
    pub init: Option<Box<Expr>>,
    pub is_static: bool,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PropKind {
    Normal,
    Method,
    Get,
    Set,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub key: PropertyKey,
    pub value: Expr,
    pub computed: bool,
    pub method: bool,
    pub shorthand: bool,
    pub kind: PropKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKey {
    Ident(Arc<str>),
    String(Arc<str>),
    Number(f64),
    Computed(Box<Expr>),
    Spread(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionExpr {
    pub name: Option<Arc<str>>,
    pub params: Vec<Arc<str>>,
    /// Optional default expression for each parameter (None = no default).
    pub param_defaults: Vec<Option<Expr>>,
    /// Name of the rest parameter (`...rest`), if any.
    pub rest_param: Option<Arc<str>>,
    pub body: Vec<Stmt>,
    pub is_arrow: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub param_decls: Vec<Pattern>,
    /// Whether this function was parsed with a `"use strict"` directive (or
    /// inherited strictness from an enclosing strict context). Drives
    /// strict-mode enforcement: `with` rejection, duplicate params, etc.
    pub is_strict: bool,
    /// True for object literal methods and class methods (enables super).
    pub is_method: bool,
    /// True only for explicit named function expressions. This creates the
    /// immutable inner name binding used for recursion; inferred display names
    /// and declarations do not get this binding.
    pub has_name_binding: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Ident(Arc<str>),
    /// An elision hole in an array pattern: `[a, , b]` consumes the element
    /// at that index but binds nothing.
    Hole,
    Array(Vec<Pattern>),
    Object(Vec<(PropertyKey, Pattern)>, Option<Box<Pattern>>),
    Assign(Box<Pattern>, Expr),
    Rest(Box<Pattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Plus, // unary + (ToNumber coercion)
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
pub struct Stmt {
    /// 1-based source line where the statement begins (0 if unknown).
    pub line: u32,
    pub node: StmtNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtNode {
    VarDecl {
        kind: VarKind,
        decls: Vec<(Arc<str>, Option<Expr>)>,
    },
    ExprStmt(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then: Box<Stmt>,
        else_: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    ForIn {
        left: Box<Stmt>,
        right: Expr,
        body: Box<Stmt>,
    },
    ForOf {
        left: Box<Stmt>,
        right: Expr,
        body: Box<Stmt>,
        /// True for `for await (x of asyncIterable)`. Requires the enclosing
        /// function to be async.
        is_await: bool,
    },
    /// `with (object) body` - injects `object`'s properties into the scope chain
    /// for dynamic name lookup within `body`.
    With {
        object: Expr,
        body: Box<Stmt>,
    },
    Break(Option<Arc<str>>),
    Continue(Option<Arc<str>>),
    Return(Option<Expr>),
    Throw(Expr),
    TryCatch {
        try_body: Box<Stmt>,
        catch_param: Option<Pattern>,
        catch_body: Option<Box<Stmt>>,
        finally_body: Option<Box<Stmt>>,
    },
    FunctionDecl(FunctionExpr),
    Labeled(Arc<str>, Box<Stmt>),
    Empty,
    Switch {
        disc: Expr,
        cases: Vec<SwitchCase>,
    },
    /// Destructuring declaration: `let [a,b] = expr` / `const {x,y} = expr`.
    Destructure {
        kind: VarKind,
        pattern: Pattern,
        init: Option<Expr>,
    },
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
    /// True when the program is parsed in strict mode, either from a
    /// `"use strict"` directive prologue or an inherited strict context.
    pub is_strict: bool,
    pub source_type: SourceType,
    /// Module specifiers requested by top-level import/re-export declarations.
    /// Empty for Script source text.
    pub module_requests: Vec<ModuleRequest>,
    /// Import bindings declared by a Module source text. Side-effect-only
    /// imports contribute a ModuleRequest but no ImportEntry.
    pub import_entries: Vec<ImportEntry>,
    /// Export bindings and re-exports declared by a Module source text.
    pub export_entries: Vec<ExportEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Script,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAttribute {
    pub key: Arc<str>,
    pub value: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRequest {
    pub specifier: Arc<str>,
    /// Sorted by UTF-16 code units, as required by ModuleRequest records.
    pub attributes: Vec<ImportAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportEntry {
    pub module_request: ModuleRequest,
    pub import_name: Arc<str>,
    pub local_name: Arc<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportEntry {
    Local {
        local_name: Arc<str>,
        export_name: Arc<str>,
    },
    ReExport {
        module_request: ModuleRequest,
        import_name: Arc<str>,
        export_name: Arc<str>,
    },
    Star {
        module_request: ModuleRequest,
    },
    NamespaceReExport {
        module_request: ModuleRequest,
        export_name: Arc<str>,
    },
}
