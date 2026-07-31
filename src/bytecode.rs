//! Bytecode instruction set for the RuJa VM.
//!
//! The VM is a stack machine: operands are pushed/popped on a value
//! stack, and operations consume from the top.

use crate::value::GcIdx;
use crate::value::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// A compiled function's bytecode.
#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    /// Per-iteration `for (let ...)` loop variable name lists, referenced by
    /// `Op::CloneLetNames(idx)`. Each entry is the set of names declared in the
    /// loop's `let`/`const` init that must be rebound per iteration.
    pub let_names: Vec<Vec<Arc<str>>>,
    /// Source spans for error reporting (ip -> line).
    pub lines: Vec<(usize, usize)>,
    /// Whether this chunk was compiled under strict-mode rules. The VM uses
    /// this to apply strict-direct-eval semantics (no var leak).
    pub is_strict: bool,
    /// For function chunks, the first instruction that belongs to the actual
    /// body after parameter initialization and declaration instantiation.
    pub body_start_ip: usize,
    /// Canonical source file that owns this script or module chunk.
    pub source_path: Option<Arc<PathBuf>>,
    /// Canonical import.meta object for an inline module source.
    pub import_meta: Option<GcIdx>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            let_names: Vec::new(),
            lines: Vec::new(),
            is_strict: false,
            body_start_ip: 0,
            source_path: None,
            import_meta: None,
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
    /// Duplicate the top two stack values: [a, b] -> [a, b, a, b]
    Dup2,

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
    NewArray(usize),                // count of elements already on stack
    NewRegExpLiteral(usize, usize), // pattern and flags constant indices
    ArrayPush,                      // pop [value, array]; append value to the array's items
    ArrayHolePush,                  // pop [array]; append an absent dense slot
    SpreadPush, // pop [iterable, array]; spread iterable's values into the array
    ObjSpread,  // pop [src, dest]; copy src's enumerable own props into dest
    /// Pop `[src, k1..kN]`; create object with source enumerable own props
    /// except the excluded keys.
    ObjRest(usize),
    /// Peek `[.., key, fn]` and apply SetFunctionName. Prefix: 0 none, 1 get,
    /// 2 set.
    SetFunctionNameFromKey(u8),
    /// Peek `[.., fn]` and apply SetFunctionName using a string constant.
    SetFunctionNameConst(usize),
    DefineAccessor(u8),      // pop [fn, key, obj]; define getter(0)/setter(1)
    DefineClassAccessor(u8), // same but enumerable=false (for class methods)
    GetProp,
    DefineDataProperty, // define enumerable own data property: stack [obj, key, value]
    DefineMethod,       // define non-enumerable method property: stack [obj, key, value]
    GetElem,            // computed member
    DeleteValue,        // pop a property Reference, push its [[Delete]] result
    SetProto,           // pop [proto, obj]; set obj's [[Prototype]] to proto
    GetProto,           // pop [obj]; push obj.[[Prototype]] or null
    ValidateExtends, // pop [parentCtor]; throw TypeError if not a constructor or prototype is invalid
    Inc,             // pop [val]; push val+1 (Number or BigInt)
    Dec,             // pop [val]; push val-1 (Number or BigInt)

    // Functions
    MakeFunction(usize), // function index in a function table
    Call(usize),         // arg count
    CallRef(usize),      // Reference call; stack [ref, callee, args...]
    CallMethod(usize),   // arg count (method call: this is on stack)
    CallSpread,          // callee + args-array on stack; spread array into call args
    CallRefSpread,       // Reference call; stack [ref, callee, args-array]
    /// Select a decorator replacement. Kind: 0 class constructor, 1 callable
    /// element value, 2 field initializer. Stack: [original, result].
    ApplyDecoratorResult(u8),
    /// Validate and append a decorator extra initializer. Stack:
    /// [active(bool), queue(array), initializer].
    DecoratorAddInitializer,
    /// Decorator context access for public or private keys. Kind: 0 has,
    /// 1 get, 2 set.
    DecoratorAccess(u8),
    /// Validate an auto-accessor decorator result and extract optional
    /// get/set/init replacements. Stack: [result] -> [get, set, init].
    ExtractAccessorDecoratorResult,
    /// Unqualified `eval(...)`: call directly only if the resolved callee is
    /// the current Realm's intrinsic eval function; otherwise call normally.
    /// Stack: [callee, args...].
    CallEval(usize), // arg count
    /// Reference-preserving unqualified `eval(...)`.
    /// Stack: [ref, callee, args...].
    CallEvalRef(usize), // arg count
    /// Direct eval from a class field initializer value.
    CallEvalClassField(usize),
    /// Reference-preserving direct eval from a class field initializer value.
    CallEvalRefClassField(usize),
    /// Spread form of unqualified `eval(...)`.
    /// Stack: [callee, argsArray].
    CallEvalSpread,
    /// Reference-preserving spread form of unqualified `eval(...)`.
    /// Stack: [ref, callee, argsArray].
    CallEvalRefSpread,
    /// Spread direct eval from a class field initializer value.
    CallEvalSpreadClassField,
    /// Reference-preserving spread direct eval from a class field initializer value.
    CallEvalRefSpreadClassField,
    CallSuperCtor(usize), // super(args): stack [this, superCtor, args...]
    CallSuperCtorSpread,  // super(...args): stack [this, superCtor, argsArray]
    New(usize),           // constructor call, arg count
    NewSpread,            // constructor call with spread args (argsArr on stack)
    Return,
    ReturnUndefined,

    // Control flow (non-local)
    Throw,
    ThrowReference(usize),
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
    /// Call a function with an explicit `this`: stack [this, fn, args...].
    CallThis(usize),
    /// Call a function with an explicit `this`: stack [this, fn, argsArray].
    CallThisSpread,
    /// Dynamic import: pop options (when present) and specifier, push Promise.
    ImportCall {
        has_options: bool,
    },
    /// Push the canonical import.meta object for the current module.
    ImportMeta,
    /// Create a fresh class private-name identity. arg = description constant idx.
    CreatePrivateName(usize),
    /// Initialize a private field/method slot. arg = name constant idx.
    /// Stack: [obj, value]. Only class element initialization should emit this.
    InitPrivate(usize),
    /// Initialize a private method slot. arg = name constant idx.
    /// Stack: [obj, method].
    InitPrivateMethod(usize),
    /// Define/merge a private accessor slot: stack [obj, get, set].
    DefinePrivateAccessor(usize),
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
    /// Execute the complete synchronous `yield*` delegation state machine.
    /// The iterator remains on the generator stack while delegation suspends.
    YieldDelegate,
    /// Async-generator `yield*`: await delegated next/return/throw results.
    YieldDelegateAsync,
    IteratorDone,
    /// `for await`: call the iterator's `next()`. Pops the iterator and pushes
    /// the iterator plus its raw result so `Await` can suspend the frame.
    IteratorNextAwait,
    /// Unpack an awaited iterator result. Pops `[iterator, result]` and pushes
    /// `[value, done]`.
    IteratorUnpackAwait,
    /// Collect the remaining values from an iterator (already on the stack)
    /// into a fresh array. Used by rest elements in array destructuring
    /// patterns: `[a, ...rest] = iterable`. Pops the iterator, pushes the array.
    IteratorCollectRest,
    /// In a synthetic finally cleanup, close the iterator when the guarded
    /// region completed abruptly and that iterator is not done. `inner_continue`
    /// identifies a continue target that stays inside the same loop and
    /// therefore must not close the iterator.
    IteratorCloseIfAbrupt {
        iter: usize,
        done: usize,
        inner_continue: Option<usize>,
        ignore_close_errors: bool,
    },
    /// Close an iterator on normal completion when its done binding is not
    /// true. Used by array destructuring assignment, whose iterator must be
    /// closed even after a successful partial pattern.
    IteratorClose {
        iter: usize,
        done: usize,
        ignore_close_errors: bool,
    },
    /// Build a tagged-template object: pop or look up the cached template
    /// object for this source site. Operands are indices into the chunk's
    /// constants for the cooked and raw string arrays. The resulting object
    /// and its `raw` property are frozen per spec.
    GetTemplateObject(Vec<usize>, Vec<usize>),

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
    ToNumeric,  // ToNumeric for update expressions
    Void,
    TypeofVar(usize),

    // Environment
    PushScope,
    /// Push the declarative environment for a catch parameter. Annex B eval
    /// admission distinguishes this record from ordinary lexical scopes.
    PushCatchScope(Option<usize>), // optional simple catch name index
    /// Push a child environment record that is itself a function-scope root.
    /// Used for sloppy functions with parameter expressions: parameter
    /// initializers run in the outer function environment, then body `var`
    /// hoists go into this body environment so they do not rewrite closures
    /// already created by parameter initializers.
    PushFunctionScope,
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
    /// Clone the loop variables from the current per-iteration env into a
    /// fresh sibling env with the same parent. Used between a `for (let ...)`
    /// body and update expression so body closures keep the pre-update
    /// binding while the update initializes the next iteration's binding.
    RecloneLetNames(usize),
    /// Restore the frame env to the current env's parent (undo a CloneLetEnv
    /// after the loop body so the update and next iteration's cond run in the
    /// original loop-scope env, and the env chain does not grow per iteration).
    RestoreParentEnv,
    DeclareVar(usize), // name index
    /// Create/update a function declaration binding. At the real global scope
    /// this applies CreateGlobalFunctionBinding descriptor rules; in eval or
    /// function scopes it falls back to an ordinary var binding.
    DeclareGlobalFunction(usize), // name index
    /// Hoist a `var` binding in the function scope root as `undefined`,
    /// without touching any `with`-object properties. Used at function/block
    /// entry to create the hoisted binding before the initializer runs.
    HoistVar(usize), // name index
    /// Copy the current block's own lexical function binding into the
    /// VariableEnvironment when its Annex B declaration is evaluated.
    AnnexBMirror(usize), // name index
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
    StoreEnvName(usize), // push name const then store to env
    LoadRef(usize),   // push a Reference record for the named binding
    MakePropertyRef,  // pop [base, propertyKey], push a resolved property Reference
    MakeRawPropertyRef, // pop [base, referencedName], push a raw property Reference
    MakeSuperPropertyRef, // pop [thisValue, base, name], defer ToPropertyKey
    ResolvePropertyRef, // resolve a retained raw property Reference exactly once
    MakePrivateRef(usize), // pop base, push private Reference; arg = name constant idx
    GetValue,         // pop a Reference, push its resolved value
    GetValueKeepReference, // [Reference] -> [Reference, resolved value] without cloning
    PutValue,         // pop [Reference, value], store value into the Reference

    // Coerce top-of-stack to a string via ToPrimitive(string) + ToString
    // (used by template-literal interpolation).
    ToString,
    // Coerce top-of-stack via ToPropertyKey, preserving Symbol keys.
    ToPropertyKey,
    CheckNullBase,          // pop obj; throw TypeError if null/undefined; push obj back
    RequireObjectCoercible, // throw TypeError if top-of-stack is null/undefined

    // Halt
    Halt,
}
