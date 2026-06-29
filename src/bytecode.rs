//! Bytecode instruction set for the RuJa VM.
//!
//! The VM is a stack machine: operands are pushed/popped on a value
//! stack, and operations consume from the top.

use crate::value::Value;
use std::rc::Rc;

/// A compiled function's bytecode.
#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    /// Per-iteration `for (let ...)` loop variable name lists, referenced by
    /// `Op::CloneLetNames(idx)`. Each entry is the set of names declared in the
    /// loop's `let`/`const` init that must be rebound per iteration.
    pub let_names: Vec<Vec<Rc<str>>>,
    /// Source spans for error reporting (ip -> line).
    pub lines: Vec<(usize, usize)>,
    /// Whether this chunk was compiled under strict-mode rules. The VM uses
    /// this to apply strict-direct-eval semantics (no var leak).
    pub is_strict: bool,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            let_names: Vec::new(),
            lines: Vec::new(),
            is_strict: false,
        }
    }

    pub fn emit(&mut self, op: Op, line: usize) {
        self.code.push(op);
        self.lines.push((self.code.len() - 1, line));
    }

    /// Resolve the source line for a given instruction pointer. Returns the
    /// line of the last recorded span at or before `ip`.
    pub fn line_for_ip(&self, ip: usize) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let mut lo = 0usize;
        let mut hi = self.lines.len();
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.lines[mid].0 <= ip {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // lines may have gaps (some ips share a span entry); find the closest
        // entry whose ip <= the target.
        let mut best = self.lines[lo].1;
        for (entry_ip, line) in &self.lines {
            if *entry_ip <= ip {
                best = *line;
            } else {
                break;
            }
        }
        Some(best)
    }

    pub fn add_constant(&mut self, v: Value) -> usize {
        self.constants.push(v);
        self.constants.len() - 1
    }

    /// Patch a jump target after the destination is known.
    pub fn patch_jump(&mut self, op_idx: usize, target: usize) {
        if let Op::Jump(ref mut dst)
        | Op::JumpIfFalse(ref mut dst)
        | Op::JumpIfTrue(ref mut dst)
        | Op::JumpIfNullish(ref mut dst)
        | Op::JumpIfNotNullish(ref mut dst) = self.code[op_idx]
        {
            *dst = target;
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk::new()
    }
}

#[derive(Clone, Debug)]
pub enum Op {
    // Constants & locals
    Const(usize),       // push constants[idx]
    LoadLocal(usize),   // push locals[idx]
    StoreLocal(usize),  // pop into locals[idx]
    LoadGlobal,         // pop name string, push global[name]
    StoreGlobal,        // pop value + name string, store into global[name]
    LoadEnv(usize),     // push from environment slot
    StoreEnv(usize),    // store to environment slot
    LoadUpvalue(usize), // captured variable from closure
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
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,    // unary minus
    BitNot, // ~
    Shl,
    Shr,
    Ushr,
    BitAnd,
    BitOr,
    BitXor,

    // Comparison
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Gt,
    Lte,
    Gte,

    // Logical
    Not,
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    JumpIfNullish(usize),    // pop; jump if null or undefined (for ?? operator)
    JumpIfNotNullish(usize), // pop; jump if NOT (null or undefined)

    // Objects/arrays
    NewObject,
    NewArray(usize),    // count of elements already on stack
    ArrayPush,          // pop [value, array]; append value to the array's items
    SpreadPush,         // pop [iterable, array]; spread iterable's values into the array
    ObjSpread,          // pop [src, dest]; copy src's enumerable own props into dest
    ObjRest(usize),     // pop [src, k1..kN]; new obj with src's own enum props except k1..kN
    DefineAccessor(u8), // pop [fn, key, obj]; define getter(0)/setter(1)
    GetProp,
    SetProp,
    GetElem, // computed member
    SetElem,
    DeleteProp,
    SetProto, // pop [proto, obj]; set obj's [[Prototype]] to proto

    // Functions
    MakeFunction(usize),  // function index in a function table
    Call(usize),          // arg count
    CallMethod(usize),    // arg count (method call: this is on stack)
    CallMethodOpt(usize), // arg count (optional method call: skip if method is nullish)
    CallSpread,           // callee + args-array on stack; spread array into call args
    /// Direct `eval(src)`: compile `src` and run it in the caller's scope.
    /// Stack: [src, argCount-extras...]; uses the current frame's env + this.
    CallDirectEval(usize), // arg count
    CallSuperCtor(usize), // super(args): stack [this, superCtor, args...]
    CallSuper(usize),     // arg count: stack [this, superProto, key, args...]
    New(usize),           // constructor call, arg count
    NewSpread,            // constructor call with spread args (argsArr on stack)
    Return,
    ReturnUndefined,

    // Control flow (non-local)
    Throw,
    PushTry(usize), // catch handler ip
    PopTry,
    EnterCatch,
    PushFinally(usize),
    PopFinally,
    /// After a `finally` body runs, re-raise the pending non-local completion
    /// (return/break/continue/throw) that diverted into the finally, if any.
    /// A normal completion (tag 0) falls through.
    PopFinallyRethrow,
    /// Divert a `break` through an active finally: set tag=2, val=next ip
    /// (the break jump that runs after the finally body), jump to finally.
    DivertBreak(usize),
    /// Divert a `continue` through an active finally: set tag=3, val=cont,
    /// jump to finally.
    DivertContinue(usize, usize),

    // Closures
    MakeClosure(usize), // function index, captures current env
    NewTarget,          // push the current frame's new.target (ctor or undefined)
    MakeClass(usize),   // class definition index in function table

    // Iteration
    GetIterator,
    /// `for await`: obtain an async iterator. Pops the iterable; prefers
    /// `Symbol.asyncIterator`, falling back to `Symbol.iterator`
    /// (async-from-sync). Pushes the iterator object.
    GetAsyncIterator,
    GetForInKeys, // pop object, push iterator over enumerable string keys
    IteratorNext,
    /// Like IteratorNext but pops a resume value and forwards it to a lazy
    /// iterator's `next()` (used by `yield*` delegation).
    IteratorNextResume,
    IteratorDone,
    /// `for await`: call the async iterator's `next()` and await the result,
    /// pushing `{value, done}` (already awaited). Pops the iterator.
    IteratorNextAwait,
    /// Collect the remaining values from an iterator (already on the stack)
    /// into a fresh array. Used by rest elements in array destructuring
    /// patterns: `[a, ...rest] = iterable`. Pops the iterator, pushes the array.
    IteratorCollectRest,

    // Spread
    Spread,

    // Type
    TypeOf,
    Await,      // pop promise/value, push settled value (sync)
    YieldValue, // pop value, push to generator's collected yields (eager)

    // Misc
    InstanceOf,
    In,
    TypeCoerce, // ToNumber for unary +
    Void,
    DeleteVar(usize),
    TypeofVar(usize),

    // Environment
    PushScope,
    PopScope,
    /// `with` statement: pop an object from the stack and push a new
    /// environment record whose `with_object` is it, as a child of the current
    /// frame env. Name lookups fall back to the object's properties.
    PushWithEnv,
    /// Pop a `with` environment record pushed by `PushWithEnv`.
    PopWithEnv,
    /// Per-iteration environment for `for (let ...)`: copy the current
    /// frame env's lexical bindings into a fresh child env and make that
    /// child the active frame env. Each iteration's closures capture a
    /// distinct binding (the classic `for (let i) out.push(()=>i)` case).
    /// Per-iteration environment for `for (let ...)`: clone ONLY the loop's
    /// declared `let`/`const` variables (referenced by `let_names[idx]`) into a
    /// fresh child env whose parent is the current env. Other bindings stay
    /// reachable through the chain so mutations to outer `let`s persist.
    CloneLetNames(usize),
    /// Restore the frame env to the current env's parent (undo a CloneLetEnv
    /// after the loop body so the update and next iteration's cond run in the
    /// original loop-scope env, and the env chain does not grow per iteration).
    RestoreParentEnv,
    DeclareVar(usize), // name index
    DeclareLet(usize),
    DeclareConst(usize),
    DeclareEnv(usize),         // declare name in env with value from stack
    DeclareEnvConst(usize),    // declare const name in env with value from stack
    DeclareLetUninit(usize),   // TDZ: declare let binding uninitialized at scope entry
    DeclareConstUninit(usize), // TDZ: declare const binding uninitialized at scope entry
    InitLet(usize),            // pop value, initialize an existing (hoisted) let binding (TDZ lift)
    InitConst(usize), // pop value, initialize an existing (hoisted) const binding (TDZ lift)
    InitEnv(usize),   // pop value, init-or-declare a let binding in current env (pattern/loop)
    InitEnvConst(usize), // pop value, init-or-declare a const binding in current env (pattern/loop)
    LoadEnvName(usize), // push name const then load from env
    StoreEnvName(usize), // push name const then store to env

    // Halt
    Halt,
}
