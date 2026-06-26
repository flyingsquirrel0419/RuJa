//! Bytecode instruction set for the RuJa VM.
//!
//! The VM is a stack machine: operands are pushed/popped on a value
//! stack, and operations consume from the top.

use crate::value::Value;

/// A compiled function's bytecode.
#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    /// Source spans for error reporting (ip -> line).
    pub lines: Vec<(usize, usize)>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk { code: Vec::new(), constants: Vec::new(), lines: Vec::new() }
    }

    pub fn emit(&mut self, op: Op, line: usize) {
        self.code.push(op);
        self.lines.push((self.code.len() - 1, line));
    }

    pub fn add_constant(&mut self, v: Value) -> usize {
        self.constants.push(v);
        self.constants.len() - 1
    }

    /// Patch a jump target after the destination is known.
    pub fn patch_jump(&mut self, op_idx: usize, target: usize) {
        if let Op::Jump(ref mut dst) | Op::JumpIfFalse(ref mut dst) | Op::JumpIfTrue(ref mut dst)
            = self.code[op_idx] {
            *dst = target;
        }
    }
}

impl Default for Chunk {
    fn default() -> Self { Chunk::new() }
}

#[derive(Clone, Debug)]
pub enum Op {
    // Constants & locals
    Const(usize),        // push constants[idx]
    LoadLocal(usize),    // push locals[idx]
    StoreLocal(usize),   // pop into locals[idx]
    LoadGlobal,          // pop name string, push global[name]
    StoreGlobal,         // pop value + name string, store into global[name]
    LoadEnv(usize),      // push from environment slot
    StoreEnv(usize),     // store to environment slot
    LoadUpvalue(usize),  // captured variable from closure
    StoreUpvalue(usize),

    // Stack ops
    Pop,
    Dup,
    Swap,
    Rot3,

    // Literals
    Null,
    Undefined,
    True,
    False,

    // Arithmetic
    Add, Sub, Mul, Div, Mod, Pow,
    Neg,         // unary minus
    BitNot,      // ~
    Shl, Shr, Ushr,
    BitAnd, BitOr, BitXor,

    // Comparison
    Eq, NotEq, StrictEq, StrictNotEq,
    Lt, Gt, Lte, Gte,

    // Logical
    Not,
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),

    // Objects/arrays
    NewObject,
    NewArray(usize),     // count of elements already on stack
    GetProp,
    SetProp,
    GetElem,             // computed member
    SetElem,
    DeleteProp,

    // Functions
    MakeFunction(usize), // function index in a function table
    Call(usize),         // arg count
    CallMethod(usize),   // arg count (method call: this is on stack)
    New(usize),          // constructor call, arg count
    Return,
    ReturnUndefined,

    // Control flow (non-local)
    Throw,
    PushTry(usize),      // catch handler ip
    PopTry,
    EnterCatch,
    PushFinally(usize),
    PopFinally,

    // Closures
    MakeClosure(usize),  // function index, captures current env
    MakeClass(usize),    // class definition index in function table

    // Iteration
    GetIterator,
    IteratorNext,
    IteratorDone,

    // Spread
    Spread,

    // Type
    TypeOf,

    // Misc
    InstanceOf,
    In,
    TypeCoerce,          // ToNumber for unary +
    Void,
    DeleteVar(usize),
    TypeofVar(usize),

    // Environment
    PushScope,
    PopScope,
    DeclareVar(usize),   // name index
    DeclareLet(usize),
    DeclareConst(usize),
    DeclareEnv(usize),   // declare name in env with value from stack
    LoadEnvName(usize),   // push name const then load from env
    StoreEnvName(usize), // push name const then store to env

    // Halt
    Halt,
}
