//! Stack-based bytecode VM.

#[cfg(test)]
mod allocation_tests;
mod async_runtime;
mod conversions;
#[cfg(test)]
mod execution_context_tests;
pub(crate) mod ops;
mod property;

pub(crate) use conversions::{to_int32, to_uint32};
pub(crate) use property::TypedArrayDefineDescriptor;

use crate::bytecode::{Chunk, Op};
use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{GcIdx, HeapObj, PropertyKey, Value};
use indexmap::IndexMap;
use num_traits::Zero;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

pub type NativeFn = fn(&mut Vm, &[Value], Option<Value>) -> error::Result<Value>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PrimitivePrototypeKind {
    String,
    Number,
    BigInt,
    Boolean,
    Symbol,
}

#[derive(Clone)]
pub(crate) struct AgentBroadcast {
    pub bytes: Arc<Mutex<Vec<u8>>>,
    pub waiters: Arc<
        Mutex<
            std::collections::HashMap<
                usize,
                std::collections::VecDeque<Arc<crate::value::AtomicsWaiter>>,
            >,
        >,
    >,
    pub max_byte_length: Option<usize>,
}

#[derive(Default)]
pub(crate) struct AgentCluster {
    pub broadcasts: Mutex<Vec<std::sync::mpsc::Sender<AgentBroadcast>>>,
    pub reports: Mutex<std::collections::VecDeque<String>>,
}

pub(crate) struct ExternalPromiseJob {
    pub resolve: Value,
    pub value: Value,
}

#[derive(Default)]
pub(crate) struct ExternalJobState {
    pub jobs: std::collections::VecDeque<ExternalPromiseJob>,
    pub wait_roots: HashMap<u64, Value>,
    pub next_wait_id: u64,
}

#[derive(Clone)]
pub(crate) enum ExecutionContextKind {
    Interpreted {
        callee: Value,
    },
    Native {
        callee: Value,
        new_target: Option<Value>,
        new_target_prototype: Option<Value>,
    },
}

#[derive(Clone)]
pub(crate) struct ExecutionContext {
    /// The lexical environment whose global root owns this call's intrinsics.
    pub(crate) realm_env: GcIdx,
    pub(crate) kind: ExecutionContextKind,
}

#[allow(dead_code)]
pub struct Vm {
    pub(crate) heap: Heap,
    pub(crate) global: GcIdx,
    pub(crate) global_this: Value,
    /// `new.target` to set on the next pushed frame (used by `construct`).
    pub(crate) pending_new_target: Option<Value>,
    /// Observable `newTarget.prototype` value already read by `construct`.
    pub(crate) pending_new_target_prototype: Option<Value>,
    /// Calls and resumptions are ordered independently from bytecode frames:
    /// a native builtin can re-enter interpreted code before another frame is
    /// available, and generators later resume beneath a different native call.
    pub(crate) execution_contexts: Vec<ExecutionContext>,
    pub(crate) stack: Vec<Value>,
    pub(crate) frames: Vec<CallFrame>,
    pub(crate) object_proto: Value,
    pub(crate) array_proto: Value,
    pub(crate) array_to_string_fn: Value,
    pub(crate) function_proto: Value,
    pub(crate) string_proto: Value,
    pub(crate) number_proto: Value,
    pub(crate) bigint_proto: Value,
    pub(crate) boolean_proto: Value,
    pub(crate) error_proto: Value,
    pub(crate) symbol_proto: Value,
    pub(crate) regexp_proto: Value,
    pub(crate) array_buffer_proto: Value,
    pub(crate) promise_ctor: Value,
    pub(crate) promise_proto: Value,
    /// `%Iterator.prototype%`, shared by synchronous iterator prototypes.
    pub(crate) iterator_base_proto: Value,
    /// `%ArrayIteratorPrototype%`.
    pub(crate) iterator_proto: Value,
    pub(crate) string_iterator_proto: Value,
    pub(crate) map_iterator_proto: Value,
    pub(crate) set_iterator_proto: Value,
    pub(crate) regexp_string_iterator_proto: Value,
    pub(crate) generator_proto: Value,
    pub(crate) generator_function_proto: Value,
    pub(crate) async_iterator_proto: Value,
    pub(crate) async_generator_proto: Value,
    pub(crate) async_generator_function_proto: Value,
    pub(crate) map_proto: Value,
    pub(crate) set_proto: Value,
    pub(crate) date_proto: Value,
    pub(crate) microtask_queue: std::collections::VecDeque<Microtask>,
    /// Monomorphic inline cache: (heap_idx, property_name) -> cached Value.
    /// Caches own data-property reads on that object for that key.
    pub(crate) ic: std::collections::HashMap<(usize, String), Value>,
    /// Temporary GC roots pinned across operations that hold heap values in
    /// Rust locals (e.g. a Promise handler while `call_function` runs, which
    /// may itself trigger a GC). Push indices on entry, pop on exit.
    pub(crate) gc_pins: Vec<usize>,
    /// WeakRef targets kept alive until the current ECMAScript job finishes.
    pub(crate) kept_objects: Vec<usize>,
    /// Collected yield values while running a generator function body (eager,
    /// legacy fallback path). Lazy generators use per-frame gen-state instead.
    pub(crate) current_yields: Vec<Value>,
    pub(crate) next_symbol_id: u32,
    pub(crate) next_private_name_id: u64,
    pub(crate) symbol_registry: HashMap<Arc<str>, u32>,
    pub(crate) symbol_descriptions: HashMap<u32, Option<Arc<str>>>,
    pub(crate) well_known_symbols: WellKnownSymbols,
    pub(crate) global_names: HashMap<Arc<str>, usize>,
    pub(crate) global_constants: Vec<Value>,
    /// Realm global environment index -> that Realm's global object.
    pub(crate) realm_globals: HashMap<usize, Value>,
    /// Realm global environment index -> original `%Object.prototype%`.
    pub(crate) realm_object_prototypes: HashMap<usize, Value>,
    /// Realm global environment index -> original `%Array.prototype%`.
    pub(crate) realm_array_prototypes: HashMap<usize, Value>,
    /// Realm global environment -> `%Promise%` and `%Promise.prototype%`.
    /// Async execution and Promise species defaults must use intrinsic
    /// identities rather than mutable global bindings.
    pub(crate) realm_promise_constructors: HashMap<usize, Value>,
    pub(crate) realm_promise_prototypes: HashMap<usize, Value>,
    /// Realm global environment -> synchronous generator intrinsics.
    pub(crate) realm_generator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_generator_function_constructors: HashMap<usize, Value>,
    pub(crate) realm_generator_function_prototypes: HashMap<usize, Value>,
    /// Realm global environment -> asynchronous generator intrinsics.
    /// All four identities are direct roots because their configurable graph
    /// links can be deleted independently.
    pub(crate) realm_async_iterator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_async_generator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_async_generator_function_constructors: HashMap<usize, Value>,
    pub(crate) realm_async_generator_function_prototypes: HashMap<usize, Value>,
    /// Realm global environment index + primitive kind -> that Realm's
    /// intrinsic wrapper prototype used by ToObject and primitive references.
    pub(crate) realm_primitive_prototypes: HashMap<(usize, PrimitivePrototypeKind), Value>,
    /// Realm global environment index -> that Realm's original intrinsic
    /// `%eval%` function object. Direct eval detection must not consult the
    /// mutable global `eval` property because scripts may replace it.
    pub(crate) realm_eval_functions: HashMap<usize, Value>,
    /// Realm global environment index -> that Realm's intrinsic
    /// `%ThrowTypeError%` function object.
    pub(crate) realm_throw_type_errors: HashMap<usize, Value>,
    /// Realm global environment index -> that Realm's intrinsic
    /// `%Function.prototype%` object. Dynamic `Function(...)` calls use this
    /// when there is no explicit `new.target` prototype.
    pub(crate) realm_function_prototypes: HashMap<usize, Value>,
    /// Realm global environment index -> that Realm's intrinsic
    /// `%AsyncFunction.prototype%` object.
    pub(crate) realm_async_function_prototypes: HashMap<usize, Value>,
    /// Realm global environment -> `%Iterator%` and `%Iterator.prototype%`.
    pub(crate) realm_iterator_constructors: HashMap<usize, Value>,
    pub(crate) realm_iterator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_array_iterator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_wrap_for_valid_iterator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_string_iterator_prototypes: HashMap<usize, Value>,
    pub(crate) realm_iterator_helper_prototypes: HashMap<usize, Value>,
    /// Realm global environment index + native error constructor name -> that
    /// Realm's original intrinsic Error prototype. Native errors must not
    /// consult mutable global bindings such as `TypeError`.
    pub(crate) realm_error_prototypes: HashMap<(usize, Arc<str>), Value>,
    /// Realm global environment index -> original `%RegExp.prototype%`.
    /// RegExp literals must not consult a mutable `RegExp` binding.
    pub(crate) realm_regexp_prototypes: HashMap<usize, Value>,
    /// Realm global environment index -> that Realm's original intrinsic
    /// `%ArrayBuffer.prototype%` object. Internal buffer allocation must not
    /// use the main Realm prototype or consult a mutable global binding.
    pub(crate) realm_array_buffer_prototypes: HashMap<usize, Value>,
    /// Realm global environment + element kind -> original TypedArray
    /// constructor. Same-type copy methods must not consult mutable globals.
    pub(crate) realm_typed_array_constructors:
        HashMap<(usize, crate::value::TypedArrayKind), Value>,
    pub(crate) module_records: HashMap<std::path::PathBuf, crate::module::ModuleRecord>,
    pub(crate) functions: Vec<Arc<crate::function::FunctionDef>>,
    /// Optional execution fuel: when set, each dispatched opcode decrements
    /// this; reaching zero throws a "fuel exhausted" RangeError. `None` means
    /// unbounded (the default). Embedders call `set_fuel` to bound untrusted
    /// code. Coarse and non-preemptive: a single native call (e.g. a long
    /// regex) is not subdivided.
    pub(crate) fuel: Option<i64>,
    /// Maximum number of live heap objects. `0` means unlimited.
    pub(crate) max_heap_objects: usize,
    /// Tagged-template object cache keyed by (chunk ptr, ip). Per spec the
    /// same template-literal site returns the same frozen template object.
    pub(crate) template_cache: std::collections::HashMap<(usize, usize), Value>,
    pub(crate) agent_cluster: Arc<AgentCluster>,
    pub(crate) agent_broadcast_rx: Option<std::sync::mpsc::Receiver<AgentBroadcast>>,
    pub(crate) agent_can_block: bool,
    pub(crate) external_jobs: Arc<Mutex<ExternalJobState>>,
}

pub struct WellKnownSymbols {
    pub iterator: u32,
    pub to_primitive: u32,
    pub has_instance: u32,
    pub to_string_tag: u32,
    pub async_iterator: u32,
    pub is_concat_spreadable: u32,
    pub r#match: u32,
    pub match_all: u32,
    pub replace: u32,
    pub search: u32,
    pub split: u32,
    pub unscopables: u32,
    pub species: u32,
    pub dispose: u32,
    pub async_dispose: u32,
}

pub struct CallFrame {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
    pub stack_base: usize,
    pub locals: Vec<Value>,
    pub callee: Value,
    pub env: GcIdx,
    /// Catch handler, guard sequence, saved environment, and operand-stack
    /// depth relative to this frame's `stack_base` at try entry.
    pub catch_stack: Vec<(usize, u32, GcIdx, usize)>,
    /// Monotonic push counter for ordering catch vs finally guards by depth.
    pub guard_seq: AtomicU32,
    pub this_val: Value,
    /// `new.target` for this frame: the constructor function when invoked via
    /// `new`, otherwise `undefined`.
    pub new_target: Value,
    /// Per-frame generator run-state. Non-zero only on a generator's own frame,
    /// so a generator body that calls `next()` on *another* generator is fully
    /// isolated (each has its own frame with its own gen-state).
    pub gen_mode: AtomicBool,
    pub gen_yield: Mutex<Option<Value>>,
    pub gen_suspended: AtomicBool,
    /// Distinguishes an Await suspension from a user-visible yield.
    pub gen_awaiting: AtomicBool,
    pub gen_resume_value: Mutex<Value>,
    /// Whether this frame is suspended in the `yield*` state machine.
    pub gen_delegating: AtomicBool,
    /// A delegated resume is consumed by `Op::YieldDelegate` instead of being
    /// injected as an ordinary yield completion.
    pub gen_delegate_resume: Mutex<Option<ResumeKind>>,
    /// Internal async `yield*` phase persisted across the adapter Promise job.
    pub gen_delegate_await_kind: AtomicU8,
    /// `yield*` forwards the delegated iterator result object unchanged.
    pub gen_yield_is_iterator_result: AtomicBool,
    /// Ordinary async functions set this while their frame is executing.
    pub async_mode: bool,
    /// Module evaluation resumes like an async function but discards its
    /// completion value when settling the evaluation Promise.
    pub module_evaluation: bool,
    /// Set by `Op::Await` so the async wrapper captures this frame instead of
    /// treating the interpreter return as function completion.
    pub async_awaiting: bool,
    pub async_await_value: Option<Value>,
    /// When set, the generator was resumed via `throw(e)`: the next dispatch
    /// in this frame throws `e` at the suspended `yield` point instead of
    /// pushing a resume value. Consumed on first use.
    pub force_throw: Mutex<Option<Value>>,
    /// When set, the generator was resumed via `return(v)`: the next dispatch
    /// in this frame injects a return completion at the suspended `yield`
    /// point, so any active `finally` blocks run before the generator closes.
    pub force_return: Mutex<Option<Value>>,
    /// Pending completion to re-raise after a `finally` block runs.
    /// Tag: 0 normal, 1 return, 2 break, 3 continue, 4 throw.
    pub finally_completion_tag: AtomicU8,
    pub finally_completion_val: Mutex<Value>,
    /// Stack of finally-target-ips for nested active `try/finally`. A
    /// non-local transfer (return/break/continue/throw) that hits an active
    /// finally diverts to the finally target after recording its completion.
    pub finally_stack: Vec<(usize, u32)>,
    /// True while executing non-strict eval code whose variable environment is
    /// the global environment. Global var/function bindings created from this
    /// frame use EvalDeclarationInstantiation's configurable=true argument.
    pub eval_global_bindings: bool,
    /// True while executing non-strict direct eval code whose variable
    /// environment is a local declarative environment. Newly-created
    /// var/function bindings are deletable per EvalDeclarationInstantiation.
    pub eval_deletable_bindings: bool,
    /// True while a function frame is evaluating parameter initializers before
    /// entering the body variable environment.
    pub in_parameter_initializers: bool,
    /// True only for non-arrow function frames. Direct eval code may contain
    /// `new.target` only when the eval call is contained in non-arrow function
    /// code; eval frames themselves keep this false so nested eval code must
    /// satisfy the same syntactic condition again.
    pub direct_eval_new_target_allowed: bool,
    /// True when this frame is a derived class constructor (extends ...).
    /// In derived constructors, returning a non-object value after super()
    /// is a TypeError.
    pub is_derived_ctor: bool,
}

pub(crate) struct DirectEvalContext {
    pub caller_env: GcIdx,
    pub this_val: Value,
    pub caller_strict: bool,
    pub caller_new_target: Value,
    pub new_target_allowed: bool,
    pub in_class_field_initializer: bool,
}

impl CallFrame {
    fn new(
        chunk: Arc<Chunk>,
        ip: usize,
        stack_base: usize,
        locals: Vec<Value>,
        env: GcIdx,
        this_val: Value,
    ) -> Self {
        CallFrame {
            chunk,
            ip,
            stack_base,
            locals,
            callee: Value::Undefined,
            env,
            new_target: Value::Undefined,
            catch_stack: Vec::new(),
            guard_seq: AtomicU32::new(0),
            this_val,
            gen_mode: AtomicBool::new(false),
            gen_yield: Mutex::new(None),
            gen_suspended: AtomicBool::new(false),
            gen_awaiting: AtomicBool::new(false),
            gen_resume_value: Mutex::new(Value::Undefined),
            gen_delegating: AtomicBool::new(false),
            gen_delegate_resume: Mutex::new(None),
            gen_delegate_await_kind: AtomicU8::new(0),
            gen_yield_is_iterator_result: AtomicBool::new(false),
            async_mode: false,
            module_evaluation: false,
            async_awaiting: false,
            async_await_value: None,
            force_throw: Mutex::new(None),
            force_return: Mutex::new(None),
            finally_completion_tag: AtomicU8::new(0),
            finally_completion_val: Mutex::new(Value::Undefined),
            finally_stack: Vec::new(),
            eval_global_bindings: false,
            eval_deletable_bindings: false,
            in_parameter_initializers: false,
            direct_eval_new_target_allowed: false,
            is_derived_ctor: false,
        }
    }
}

/// How a suspended generator is resumed: normal `next(v)`, `throw(e)` (inject an
/// exception at the yield point), or `return(v)` (force-complete the generator).
#[derive(Clone)]
pub enum ResumeKind {
    Next(Value),
    Throw(Value),
    Return(Value),
    DelegateResult {
        value: Value,
        return_completion: bool,
    },
    DelegateThrow(Value),
    DelegateMissingThrow,
}

#[derive(Clone, Copy)]
pub(crate) enum DelegateAwaitKind {
    Result,
    ReturnResult,
    MissingThrow,
}

pub(crate) enum DelegateOutcome {
    Yield(Value),
    Complete(Value),
    Return(Value),
    Await(Value, DelegateAwaitKind),
}

/// Outcome of executing a single bytecode instruction.
#[allow(dead_code)]
enum Flow {
    /// Keep dispatching the next instruction.
    Continue,
    /// A Halt/Return ended execution with a value.
    Value(Value),
}

pub(crate) struct GeneratorPrologueState {
    pub env: GcIdx,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub locals: Vec<Value>,
    pub catch_stack: Vec<(usize, u32, GcIdx, usize)>,
    pub finally_stack: Vec<(usize, u32)>,
    pub guard_seq: u32,
    pub finally_completion_tag: u8,
    pub finally_completion_val: Value,
}

pub enum Microtask {
    Then {
        promise: GcIdx,
        on_fulfilled: Value,
        on_rejected: Value,
        derived: Option<crate::value::PromiseReactionCapability>,
        continuation: Option<crate::value::PromiseContinuation>,
        realm: Option<GcIdx>,
    },
    Thenable {
        thenable: Value,
        then: Value,
        resolve: Value,
        reject: Value,
        realm: GcIdx,
    },
    Resolve {
        promise: GcIdx,
        value: Value,
    },
    Reject {
        promise: GcIdx,
        reason: Value,
    },
    AsyncGeneratorDrain {
        generator: GcIdx,
    },
    DynamicImport {
        promise: GcIdx,
        resolve: Value,
        reject: Value,
        realm: GcIdx,
        referrer: Arc<std::path::PathBuf>,
        specifier: Arc<str>,
        import_type: Option<Arc<str>>,
    },
    FinalizationCleanup {
        registry: GcIdx,
    },
}

impl Default for Vm {
    fn default() -> Self {
        Self::new().expect("failed to initialize VM")
    }
}

impl Vm {
    fn offset_function_indices(chunk: &mut Chunk, base: usize) {
        if base == 0 {
            return;
        }
        for op in &mut chunk.code {
            match op {
                Op::MakeFunction(idx) | Op::MakeClosure(idx) | Op::MakeClass(idx) => {
                    *idx += base;
                }
                _ => {}
            }
        }
    }

    pub(crate) fn append_compiled_functions(
        &mut self,
        mut chunk: Chunk,
        funcs: Vec<Arc<crate::function::FunctionDef>>,
    ) -> Chunk {
        let base = self.functions.len();
        Self::offset_function_indices(&mut chunk, base);
        let adjusted = funcs.into_iter().map(|func| {
            let mut func = (*func).clone();
            let mut func_chunk = (*func.chunk).clone();
            Self::offset_function_indices(&mut func_chunk, base);
            func.chunk = Arc::new(func_chunk);
            Arc::new(func)
        });
        self.functions.extend(adjusted);
        chunk
    }

    pub(crate) fn append_compiled_functions_with_source(
        &mut self,
        mut chunk: Chunk,
        funcs: Vec<Arc<crate::function::FunctionDef>>,
        source_path: Arc<std::path::PathBuf>,
    ) -> Chunk {
        chunk.source_path = Some(source_path.clone());
        let funcs = funcs
            .into_iter()
            .map(|func| {
                let mut func = (*func).clone();
                let mut func_chunk = (*func.chunk).clone();
                func_chunk.source_path = Some(source_path.clone());
                func.chunk = Arc::new(func_chunk);
                Arc::new(func)
            })
            .collect();
        self.append_compiled_functions(chunk, funcs)
    }

    fn append_compiled_functions_with_active_source(
        &mut self,
        chunk: Chunk,
        funcs: Vec<Arc<crate::function::FunctionDef>>,
    ) -> Chunk {
        let source_path = self
            .frames
            .iter()
            .rev()
            .find_map(|frame| frame.chunk.source_path.clone());
        if let Some(source_path) = source_path {
            self.append_compiled_functions_with_source(chunk, funcs, source_path)
        } else {
            self.append_compiled_functions(chunk, funcs)
        }
    }

    fn current_frame(&self) -> error::Result<&CallFrame> {
        self.frames
            .last()
            .ok_or_else(|| crate::error::Error::internal("no active call frame"))
    }

    fn current_frame_mut(&mut self) -> error::Result<&mut CallFrame> {
        self.frames
            .last_mut()
            .ok_or_else(|| crate::error::Error::internal("no active call frame"))
    }

    pub fn new() -> error::Result<Self> {
        let heap = Heap::new();
        let global = env::new_env(&heap, None, true)?;
        let mut vm = Vm {
            heap,
            global,
            global_this: Value::Undefined,
            pending_new_target: None,
            pending_new_target_prototype: None,
            execution_contexts: Vec::new(),
            stack: Vec::new(),
            frames: Vec::new(),
            object_proto: Value::Undefined,
            array_proto: Value::Undefined,
            array_to_string_fn: Value::Undefined,
            function_proto: Value::Undefined,
            string_proto: Value::Undefined,
            number_proto: Value::Undefined,
            bigint_proto: Value::Undefined,
            boolean_proto: Value::Undefined,
            error_proto: Value::Undefined,
            symbol_proto: Value::Undefined,
            regexp_proto: Value::Undefined,
            array_buffer_proto: Value::Undefined,
            promise_ctor: Value::Undefined,
            promise_proto: Value::Undefined,
            iterator_base_proto: Value::Undefined,
            iterator_proto: Value::Undefined,
            string_iterator_proto: Value::Undefined,
            map_iterator_proto: Value::Undefined,
            set_iterator_proto: Value::Undefined,
            regexp_string_iterator_proto: Value::Undefined,
            generator_proto: Value::Undefined,
            generator_function_proto: Value::Undefined,
            async_iterator_proto: Value::Undefined,
            async_generator_proto: Value::Undefined,
            async_generator_function_proto: Value::Undefined,
            map_proto: Value::Undefined,
            set_proto: Value::Undefined,
            date_proto: Value::Undefined,
            microtask_queue: std::collections::VecDeque::new(),
            ic: std::collections::HashMap::new(),
            gc_pins: Vec::new(),
            kept_objects: Vec::new(),
            current_yields: Vec::new(),
            next_symbol_id: 16,
            next_private_name_id: 1,
            symbol_registry: HashMap::new(),
            symbol_descriptions: HashMap::new(),
            well_known_symbols: WellKnownSymbols {
                iterator: 1,
                to_primitive: 2,
                has_instance: 3,
                to_string_tag: 4,
                async_iterator: 5,
                is_concat_spreadable: 6,
                r#match: 7,
                match_all: 8,
                replace: 9,
                search: 10,
                split: 11,
                unscopables: 12,
                species: 13,
                dispose: 14,
                async_dispose: 15,
            },
            global_names: HashMap::new(),
            global_constants: Vec::new(),
            realm_globals: HashMap::new(),
            realm_object_prototypes: HashMap::new(),
            realm_array_prototypes: HashMap::new(),
            realm_promise_constructors: HashMap::new(),
            realm_promise_prototypes: HashMap::new(),
            realm_generator_prototypes: HashMap::new(),
            realm_generator_function_constructors: HashMap::new(),
            realm_generator_function_prototypes: HashMap::new(),
            realm_async_iterator_prototypes: HashMap::new(),
            realm_async_generator_prototypes: HashMap::new(),
            realm_async_generator_function_constructors: HashMap::new(),
            realm_async_generator_function_prototypes: HashMap::new(),
            realm_primitive_prototypes: HashMap::new(),
            realm_eval_functions: HashMap::new(),
            realm_throw_type_errors: HashMap::new(),
            realm_function_prototypes: HashMap::new(),
            realm_async_function_prototypes: HashMap::new(),
            realm_iterator_constructors: HashMap::new(),
            realm_iterator_prototypes: HashMap::new(),
            realm_array_iterator_prototypes: HashMap::new(),
            realm_wrap_for_valid_iterator_prototypes: HashMap::new(),
            realm_string_iterator_prototypes: HashMap::new(),
            realm_iterator_helper_prototypes: HashMap::new(),
            realm_error_prototypes: HashMap::new(),
            realm_regexp_prototypes: HashMap::new(),
            realm_array_buffer_prototypes: HashMap::new(),
            realm_typed_array_constructors: HashMap::new(),
            module_records: HashMap::new(),
            functions: Vec::new(),
            fuel: None,
            max_heap_objects: 0,
            template_cache: std::collections::HashMap::new(),
            agent_cluster: Arc::new(AgentCluster::default()),
            agent_broadcast_rx: None,
            agent_can_block: false,
            external_jobs: Arc::new(Mutex::new(ExternalJobState::default())),
        };
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.iterator,
            Some(Arc::from("Symbol.iterator")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.to_primitive,
            Some(Arc::from("Symbol.toPrimitive")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.has_instance,
            Some(Arc::from("Symbol.hasInstance")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.to_string_tag,
            Some(Arc::from("Symbol.toStringTag")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.async_iterator,
            Some(Arc::from("Symbol.asyncIterator")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.is_concat_spreadable,
            Some(Arc::from("Symbol.isConcatSpreadable")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.r#match,
            Some(Arc::from("Symbol.match")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.match_all,
            Some(Arc::from("Symbol.matchAll")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.replace,
            Some(Arc::from("Symbol.replace")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.search,
            Some(Arc::from("Symbol.search")),
        );
        vm.symbol_descriptions
            .insert(vm.well_known_symbols.split, Some(Arc::from("Symbol.split")));
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.unscopables,
            Some(Arc::from("Symbol.unscopables")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.species,
            Some(Arc::from("Symbol.species")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.dispose,
            Some(Arc::from("Symbol.dispose")),
        );
        vm.symbol_descriptions.insert(
            vm.well_known_symbols.async_dispose,
            Some(Arc::from("Symbol.asyncDispose")),
        );
        crate::builtins::setup_full(&mut vm)?;
        Ok(vm)
    }

    /// Set an execution-fuel budget. While set, each dispatched opcode
    /// decrements the budget; reaching zero throws a `RangeError("fuel
    /// exhausted")`. Pass `None` to disable the limit (the default). The
    /// budget persists across `run` calls, so an embedder can refill it
    /// between ticks. Coarse and cooperative, not preemption.
    pub fn set_fuel(&mut self, fuel: Option<i64>) {
        self.fuel = fuel;
    }

    pub(crate) fn consume_fuel(&mut self) -> error::Result<()> {
        if let Some(fuel) = self.fuel.as_mut() {
            if *fuel <= 0 {
                return Err(crate::error::Error::fuel("fuel exhausted"));
            }
            *fuel -= 1;
        }
        Ok(())
    }

    /// Configure whether the current agent may synchronously suspend in
    /// `Atomics.wait`. Browser main agents normally disable this while worker
    /// agents enable it.
    pub fn set_agent_can_block(&mut self, can_block: bool) {
        self.agent_can_block = can_block;
    }

    pub fn run_external_jobs_until_idle(&mut self) -> error::Result<()> {
        loop {
            self.run_microtasks()?;
            let pending_external = {
                let external = self.external_jobs.lock();
                !external.wait_roots.is_empty() || !external.jobs.is_empty()
            };
            if !pending_external && !self.has_pending_microtasks() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Set the maximum number of live heap objects. When this limit is
    /// exceeded, allocation throws a `RangeError("heap limit exceeded")`.
    /// `Some(0)` or `None` means unlimited.
    pub fn set_max_heap_objects(&mut self, max: Option<usize>) {
        let limit = max.unwrap_or(0);
        self.max_heap_objects = limit;
        self.heap.set_max_objects(limit);
    }

    /// Remaining fuel, or `None` if unbounded.
    pub fn fuel_remaining(&self) -> Option<i64> {
        self.fuel
    }

    /// Run a source string and return the value of the last top-level expression.
    pub fn run(&mut self, src: &str) -> error::Result<Value> {
        self.run_with_source_path(src, None)
    }

    /// Run a Script source file while preserving its canonical host referrer.
    pub fn run_file(&mut self, path: impl AsRef<std::path::Path>) -> error::Result<Value> {
        let canonical = path.as_ref().canonicalize().map_err(|err| {
            Error::syntax(format!(
                "Cannot resolve script '{}': {}",
                path.as_ref().display(),
                err
            ))
        })?;
        let src = std::fs::read_to_string(&canonical).map_err(|err| {
            Error::syntax(format!(
                "Cannot read script '{}': {}",
                canonical.display(),
                err
            ))
        })?;
        self.run_with_source_path(&src, Some(Arc::new(canonical)))
    }

    fn run_with_source_path(
        &mut self,
        src: &str,
        source_path: Option<Arc<std::path::PathBuf>>,
    ) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        self.check_global_declaration_instantiation(&program, self.global, &self.global_this)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = if let Some(source_path) = source_path {
            self.append_compiled_functions_with_source(chunk, funcs, source_path)
        } else {
            self.append_compiled_functions(chunk, funcs)
        };
        // Script top-level `this` is the global object even for strict scripts.
        crate::environment::declare(
            &self.heap,
            self.global,
            "this",
            self.global_this.clone(),
            crate::value::BindingKind::Const,
        );
        let result = self.execute_chunk(chunk, self.global, Value::Undefined);
        let result_roots: Vec<Value> = match &result {
            Ok(value) => vec![value.clone()],
            Err(err) => err.thrown_value.iter().cloned().collect(),
        };
        let pinned_result = self.pin_many(&result_roots);
        self.clear_kept_objects();
        // Drain microtasks (Promise callbacks) after the synchronous run.
        let mut microtask_result = if !self.microtask_queue.is_empty() {
            self.run_microtasks()
        } else {
            Ok(())
        };
        // Collect at a safe point: all frames are settled and no Rust local
        // except `result` holds a heap value across this boundary.
        if microtask_result.is_ok() && self.heap.live_count() > 0 {
            let roots = self.collect_roots();
            if self.heap.maybe_collect(&roots) {
                self.ic.clear();
            }
            self.schedule_finalization_cleanup_jobs();
        }
        if microtask_result.is_ok() && !self.microtask_queue.is_empty() {
            microtask_result = self.run_microtasks();
        }
        self.unpin_many(pinned_result);
        microtask_result?;
        result
    }

    /// Evaluate source text with the ECMAScript Module source goal.
    /// A later module-graph layer will add import/export linking; this entry
    /// point establishes the strict, declarative module execution context.
    pub fn run_module(&mut self, src: &str) -> error::Result<Value> {
        let program = crate::parser::Parser::parse_module(src)?;
        if !program.module_requests.is_empty() {
            return Err(Error::syntax(
                "Module requests require run_module_file with a resolvable origin",
            ));
        }
        let module_env = crate::environment::new_env(&self.heap, Some(self.global), true)?;
        crate::environment::declare(
            &self.heap,
            module_env,
            "this",
            Value::Undefined,
            crate::value::BindingKind::Const,
        );
        let mut compiler = crate::compiler::Compiler::new();
        let (mut chunk, funcs) = compiler.compile_program(&program)?;
        let import_meta = self.allocate_import_meta()?;
        chunk.import_meta = Some(import_meta);
        let funcs = funcs
            .into_iter()
            .map(|func| {
                let mut func = (*func).clone();
                let mut func_chunk = (*func.chunk).clone();
                func_chunk.import_meta = Some(import_meta);
                func.chunk = Arc::new(func_chunk);
                Arc::new(func)
            })
            .collect();
        let chunk = self.append_compiled_functions_with_active_source(chunk, funcs);
        let result = self.execute_chunk(chunk, module_env, Value::Undefined);
        let result_roots: Vec<Value> = match &result {
            Ok(value) => vec![value.clone()],
            Err(err) => err.thrown_value.iter().cloned().collect(),
        };
        let pinned_result = self.pin_many(&result_roots);
        self.clear_kept_objects();
        let mut microtask_result = if !self.microtask_queue.is_empty() {
            self.run_microtasks()
        } else {
            Ok(())
        };
        if microtask_result.is_ok() && self.heap.live_count() > 0 {
            let roots = self.collect_roots();
            if self.heap.maybe_collect(&roots) {
                self.ic.clear();
            }
            self.schedule_finalization_cleanup_jobs();
        }
        if microtask_result.is_ok() && !self.microtask_queue.is_empty() {
            microtask_result = self.run_microtasks();
        }
        self.unpin_many(pinned_result);
        microtask_result?;
        result
    }

    pub(crate) fn execute_chunk(
        &mut self,
        chunk: Chunk,
        env: GcIdx,
        this_val: Value,
    ) -> error::Result<Value> {
        let chunk = Arc::new(chunk);
        let stack_base = self.stack.len();
        self.frames.push(CallFrame::new(
            chunk.clone(),
            0,
            stack_base,
            vec![Value::Undefined; 256],
            env,
            this_val,
        ));
        self.interpret()
    }

    pub(crate) fn instantiate_module_chunk(
        &mut self,
        chunk: Arc<Chunk>,
        env: GcIdx,
    ) -> error::Result<()> {
        let stack_base = self.stack.len();
        self.frames.push(CallFrame::new(
            chunk.clone(),
            0,
            stack_base,
            vec![Value::Undefined; 256],
            env,
            Value::Undefined,
        ));
        let target_depth = self.frames.len() - 1;
        if let Err(error) = self.interpret_to_depth_until_ip(target_depth, chunk.body_start_ip) {
            self.frames.truncate(target_depth);
            self.stack.truncate(stack_base);
            return Err(error);
        }
        self.frames.truncate(target_depth);
        self.stack.truncate(stack_base);
        Ok(())
    }

    pub(crate) fn evaluate_module_chunk(
        &mut self,
        chunk: Arc<Chunk>,
        env: GcIdx,
    ) -> error::Result<Value> {
        let stack_base = self.stack.len();
        let frame_depth = self.frames.len();
        self.frames.push(CallFrame::new(
            chunk.clone(),
            chunk.body_start_ip,
            stack_base,
            vec![Value::Undefined; 256],
            env,
            Value::Undefined,
        ));
        let result = self.interpret();
        self.frames.truncate(frame_depth);
        self.stack.truncate(stack_base);
        result
    }

    /// Like execute_chunk but guarantees the pushed frame is popped on return,
    /// so eval (which reuses the VM afterwards) leaves the caller's frame stack
    /// intact. Used by eval paths only.
    fn execute_chunk_scoped(
        &mut self,
        chunk: Chunk,
        env: GcIdx,
        this_val: Value,
        eval_global_bindings: bool,
        eval_deletable_bindings: bool,
        new_target: Value,
    ) -> error::Result<Value> {
        let chunk = Arc::new(chunk);
        // eval runs on the shared stack. Push a sentinel Undefined so that
        // Halt has something to pop even if the eval body never pushes a
        // value (e.g. break/continue jumping directly to Halt).
        let stack_depth = self.stack.len();
        self.stack.push(Value::Undefined);
        self.frames.push(CallFrame::new(
            chunk.clone(),
            0,
            stack_depth,
            vec![Value::Undefined; 256],
            env,
            this_val,
        ));
        if let Some(frame) = self.frames.last_mut() {
            frame.eval_global_bindings = eval_global_bindings;
            frame.eval_deletable_bindings = eval_deletable_bindings;
            frame.new_target = new_target;
        }
        let depth_before = self.frames.len();
        let result = self.interpret();
        // Restore caller stack to its pre-eval state, then push the result.
        self.stack.truncate(stack_depth);
        // Pop any frames we pushed for the eval (Halt leaves it; Return popped it).
        while self.frames.len() >= depth_before && self.frames.len() > 1 {
            let top_is_ours = self
                .frames
                .last()
                .map(|f| Arc::ptr_eq(&f.chunk, &chunk))
                .unwrap_or(false);
            if top_is_ours {
                self.frames.pop();
            } else {
                break;
            }
        }
        result
    }

    /// Evaluate a source string as an *indirect* eval in the current Realm's
    /// global scope. Non-string inputs are handled by the eval builtin before
    /// this method is called.
    pub fn eval_indirect(&mut self, src: &str) -> error::Result<Value> {
        self.eval_indirect_in(self.global, self.global_this.clone(), src)
    }

    /// Evaluate a test262 host `evalScript` string as script code in the
    /// current Realm's global scope. Unlike indirect eval, script global
    /// declarations use non-configurable global bindings.
    pub(crate) fn eval_script_global(&mut self, src: &str) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        self.check_global_declaration_instantiation(&program, self.global, &self.global_this)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions_with_active_source(chunk, funcs);
        crate::environment::declare(
            &self.heap,
            self.global,
            "this",
            self.global_this.clone(),
            crate::value::BindingKind::Const,
        );
        let result = self.execute_chunk_scoped(
            chunk,
            self.global,
            self.global_this.clone(),
            false,
            false,
            Value::Undefined,
        );
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    /// Evaluate a source string as an *indirect* eval in a specific Realm
    /// global environment. This is used for cross-Realm eval functions whose
    /// [[Realm]] is represented by their closure environment.
    pub(crate) fn eval_indirect_in(
        &mut self,
        global_env: GcIdx,
        global_this: Value,
        src: &str,
    ) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        let is_strict = program.is_strict;
        let (_, var_names, _) = crate::compiler::Compiler::collect_global_declaration_names(
            &program.body,
            program.is_strict,
        );
        if !is_strict && global_env == self.global {
            self.check_eval_global_declaration_instantiation(&program, global_env, &global_this)?;
        }
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions_with_active_source(chunk, funcs);
        let eval_env = if global_env == self.global {
            crate::environment::new_env(&self.heap, Some(global_env), is_strict)?
        } else {
            global_env
        };
        crate::environment::declare(
            &self.heap,
            eval_env,
            "this",
            global_this.clone(),
            crate::value::BindingKind::Const,
        );
        let result = self.execute_chunk_scoped(
            chunk,
            eval_env,
            global_this.clone(),
            !is_strict && global_env == self.global,
            false,
            Value::Undefined,
        );
        if !is_strict && global_env != self.global {
            for name in &var_names {
                if let Some(value) = crate::environment::get(&self.heap, global_env, name) {
                    self.set_property(&global_this, name, value)?;
                }
            }
        }
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    fn check_global_declaration_instantiation(
        &self,
        program: &crate::ast::Program,
        global_env: GcIdx,
        global_this: &Value,
    ) -> error::Result<()> {
        let (lexical_names, var_names, function_names) =
            crate::compiler::Compiler::collect_global_declaration_names(
                &program.body,
                program.is_strict,
            );

        for name in &lexical_names {
            if self.has_global_lexical_declaration(global_env, name)
                || self.has_restricted_global_property(global_this, name)
            {
                return Err(Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }

        for name in &var_names {
            if self.has_global_lexical_declaration(global_env, name) {
                return Err(Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }

        let mut declared_functions = std::collections::HashSet::new();
        for name in function_names.iter().rev() {
            if declared_functions.insert(name.clone())
                && !self.can_declare_global_function(global_this, name)
            {
                return Err(Error::type_err(format!(
                    "Cannot declare global function '{}'",
                    name
                )));
            }
        }

        let function_set: std::collections::HashSet<Arc<str>> =
            function_names.into_iter().collect();
        let mut declared_vars = std::collections::HashSet::new();
        for name in &var_names {
            if function_set.contains(name) {
                continue;
            }
            if declared_vars.insert(name.clone()) && !self.can_declare_global_var(global_this, name)
            {
                return Err(Error::type_err(format!(
                    "Cannot declare global variable '{}'",
                    name
                )));
            }
        }
        Ok(())
    }

    fn check_eval_global_declaration_instantiation(
        &self,
        program: &crate::ast::Program,
        global_env: GcIdx,
        global_this: &Value,
    ) -> error::Result<()> {
        let (_, var_names, function_names) =
            crate::compiler::Compiler::collect_global_declaration_names(
                &program.body,
                program.is_strict,
            );

        for name in &var_names {
            if self.has_global_lexical_declaration(global_env, name) {
                return Err(Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }

        let mut declared_functions = std::collections::HashSet::new();
        for name in function_names.iter().rev() {
            if declared_functions.insert(name.clone())
                && !self.can_declare_global_function(global_this, name)
            {
                return Err(Error::type_err(format!(
                    "Cannot declare global function '{}'",
                    name
                )));
            }
        }

        let function_set: std::collections::HashSet<Arc<str>> =
            function_names.into_iter().collect();
        let mut declared_vars = std::collections::HashSet::new();
        for name in &var_names {
            if function_set.contains(name) {
                continue;
            }
            if declared_vars.insert(name.clone()) && !self.can_declare_global_var(global_this, name)
            {
                return Err(Error::type_err(format!(
                    "Cannot declare global variable '{}'",
                    name
                )));
            }
        }
        Ok(())
    }

    /// Evaluate a source string as a *direct* eval: run it in a child of the
    /// caller's current environment, so it can read/assign the caller's
    /// variables. `var`/function declarations leak to the caller's function
    /// scope root (sloppy mode). `this_val` is the caller's `this`.
    pub(crate) fn eval_direct(
        &mut self,
        src: &str,
        ctx: DirectEvalContext,
    ) -> error::Result<Value> {
        let super_allowed = crate::environment::has(&self.heap, ctx.caller_env, "#super");
        let super_call_allowed = !ctx.in_class_field_initializer
            && crate::environment::has(&self.heap, ctx.caller_env, "#superctor");
        let inherited_private_names =
            crate::environment::private_names_in_scope(&self.heap, ctx.caller_env);
        let program = crate::parser::Parser::parse_direct_eval_inherited(
            src,
            ctx.caller_strict,
            super_allowed,
            super_call_allowed,
            ctx.new_target_allowed,
            &inherited_private_names,
        )?;
        if ctx.in_class_field_initializer {
            crate::parser::Parser::reject_class_field_initializer_program_contains_arguments(
                &program,
            )?;
        }
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions_with_active_source(chunk, funcs);
        // Per spec, direct eval runs in a dedicated lexical environment whose
        // parent is the caller's environment. `let`/`const`/`class` declared in
        // eval stay local to that environment (they do NOT leak to the caller),
        // while `var` and function declarations leak to the caller's function
        // scope — UNLESS the eval code is strict, in which case nothing leaks
        // (the eval has its own scope and all bindings stay local). Pre-declare
        // the var/function names in the caller's variable environment (sloppy
        // only) so later name resolution can see the hoisted bindings; then run
        // the eval body in the child environment.
        let is_strict = ctx.caller_strict || program.is_strict;
        let var_names = if is_strict {
            Vec::new()
        } else {
            crate::compiler::Compiler::collect_var_names(&program.body)
        };
        let (_, _, function_names) = crate::compiler::Compiler::collect_global_declaration_names(
            &program.body,
            program.is_strict,
        );
        let var_env = crate::environment::function_scope_root(&self.heap, ctx.caller_env);
        if !is_strict && var_env == self.global {
            self.check_eval_global_declaration_instantiation(&program, var_env, &self.global_this)?;
        }
        if !is_strict {
            for name in &var_names {
                let declaring_arguments = name.as_ref() == "arguments";
                let in_parameter_initializers = self
                    .frames
                    .last()
                    .is_some_and(|frame| frame.in_parameter_initializers);
                let conflicts_with_parameter = in_parameter_initializers
                    && crate::environment::binding_env_and_kind(&self.heap, ctx.caller_env, name)
                        .is_some_and(|(env, kind)| {
                            env == ctx.caller_env && kind == crate::value::BindingKind::Param
                        });
                if declaring_arguments
                    && in_parameter_initializers
                    && crate::environment::has_own_binding(&self.heap, ctx.caller_env, name)
                {
                    return Err(Error::syntax(
                        "Cannot declare 'arguments' from direct eval in parameter initializer",
                    ));
                }
                if conflicts_with_parameter {
                    return Err(Error::syntax(format!(
                        "Identifier '{}' conflicts with a parameter binding",
                        name
                    )));
                }
                if var_env != self.global
                    && function_names
                        .iter()
                        .any(|function_name| function_name == name)
                {
                    continue;
                }
                if crate::environment::has_lexical_declaration_between(
                    &self.heap,
                    ctx.caller_env,
                    var_env,
                    name,
                ) {
                    return Err(Error::syntax(format!(
                        "Identifier '{}' has already been declared",
                        name
                    )));
                }
            }
        }
        let eval_env = crate::environment::new_env(&self.heap, Some(ctx.caller_env), is_strict)?;
        let result = self.execute_chunk_scoped(
            chunk,
            eval_env,
            ctx.this_val,
            !is_strict && var_env == self.global,
            !is_strict && var_env != self.global,
            if ctx.in_class_field_initializer {
                Value::Undefined
            } else {
                ctx.caller_new_target
            },
        );
        // After running, copy the var/function bindings that the eval body
        // established back into the caller's variable environment (they leak per
        // spec). `let`/`const`/`class` stay in eval_env and are discarded with
        // it. Strict eval does not leak anything.
        if is_strict {
            if !self.microtask_queue.is_empty() {
                self.run_microtasks()?;
            }
            return result;
        }
        let leaked: Vec<(Arc<str>, Value)> = self.heap.with_obj(eval_env.0, |o| {
            if let HeapObj::Environment(e) = o {
                e.vars
                    .lock()
                    .iter()
                    .filter(|(name, _)| var_names.contains(&**name))
                    .map(|(name, b)| (name.clone(), b.value.lock().clone()))
                    .collect()
            } else {
                Vec::new()
            }
        });
        for (name, value) in leaked {
            // Do not clobber an existing lexical (let/const) binding in the
            // caller: a same-named eval `var` is a no-op there per spec.
            if crate::environment::has_lexical_declaration_between(
                &self.heap,
                ctx.caller_env,
                var_env,
                &name,
            ) {
                continue;
            }
            if var_env == self.global {
                if function_names
                    .iter()
                    .any(|function_name| function_name == &name)
                {
                    self.create_global_function_binding_with_configurable(&name, value, true)?;
                } else {
                    crate::environment::declare_var(&self.heap, var_env, &name, value.clone());
                    self.create_global_var_binding_with_configurable(&name, true)?;
                    self.set_global_eval_var_property(&name, value);
                }
            } else {
                crate::environment::declare_var(&self.heap, var_env, &name, value);
            }
        }
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    /// Execute a compiled function's chunk in a new frame.
    fn execute_chunk_func(
        &mut self,
        callee: Value,
        fdef: Arc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
        new_target: Value,
    ) -> error::Result<Value> {
        let mut locals = vec![Value::Undefined; fdef.num_locals.max(256)];
        for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
            // Use the compiled slot map so duplicate parameter names (allowed
            // in non-strict functions) share a slot, with the last value winning.
            let slot = fdef.param_slots.get(i).copied().unwrap_or(i);
            if slot < locals.len() {
                locals[slot] = a.clone();
            }
        }
        self.frames.push(CallFrame::new(
            fdef.chunk.clone(),
            0,
            self.stack.len(),
            locals,
            env,
            this_val,
        ));
        if let Some(frame) = self.frames.last_mut() {
            frame.callee = callee;
            frame.in_parameter_initializers = fdef.has_parameter_expressions;
            frame.new_target = new_target;
            frame.direct_eval_new_target_allowed = !fdef.is_arrow;
        }
        // Run only this function's frame. interpret returns when its frame pops.
        let target_depth = self.frames.len() - 1;
        let result = self.interpret_to_depth(target_depth);
        // On error, the function frame is still on the stack; pop it so the
        // caller's catch handler can be found by the enclosing interpret_catch.
        if result.is_err() {
            self.frames.pop();
        }
        // Periodic GC at a frame-boundary safe point (no Rust local holds a
        // heap value here). Throttled to keep collection cost low.
        // Use `%` rather than `is_multiple_of`, which was only stabilized in
        // Rust 1.87 — older toolchains (and some CI images) lack it.
        let live = self.heap.live_count();
        #[allow(clippy::manual_is_multiple_of)]
        if live > 0 && live % 2048 == 0 {
            let result_roots: Vec<Value> = match &result {
                Ok(value) => vec![value.clone()],
                Err(err) => err.thrown_value.iter().cloned().collect(),
            };
            let pinned_result = self.pin_many(&result_roots);
            let roots = self.collect_roots();
            if self.heap.maybe_collect(&roots) {
                self.ic.clear();
            }
            self.schedule_finalization_cleanup_jobs();
            self.unpin_many(pinned_result);
        }
        result
    }

    pub(crate) fn execute_generator_prologue(
        &mut self,
        fdef: Arc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
    ) -> error::Result<GeneratorPrologueState> {
        let mut locals = vec![Value::Undefined; fdef.num_locals.max(256)];
        for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
            let slot = fdef.param_slots.get(i).copied().unwrap_or(i);
            if slot < locals.len() {
                locals[slot] = a.clone();
            }
        }

        let stack_base = self.stack.len();
        self.frames.push(CallFrame::new(
            fdef.chunk.clone(),
            0,
            stack_base,
            locals,
            env,
            this_val,
        ));
        if let Some(frame) = self.frames.last_mut() {
            frame.in_parameter_initializers = fdef.has_parameter_expressions;
        }

        let target_depth = self.frames.len() - 1;
        let result = self
            .interpret_to_depth_until_ip(target_depth, fdef.chunk.body_start_ip)
            .map_err(|error| self.materialize_current_interpreted_error(error));
        if let Err(err) = result {
            if self.frames.len() > target_depth {
                self.frames.truncate(target_depth);
            }
            self.stack.truncate(stack_base);
            return Err(err);
        }

        let frame = self
            .frames
            .pop()
            .ok_or_else(|| Error::internal("generator prologue frame missing"))?;
        let saved_stack = self.stack.split_off(stack_base);
        let guard_seq = frame.guard_seq.load(Ordering::Relaxed);
        let finally_completion_tag = frame.finally_completion_tag.load(Ordering::Relaxed);
        let finally_completion_val = frame.finally_completion_val.lock().clone();
        Ok(GeneratorPrologueState {
            env: frame.env,
            ip: frame.ip,
            stack: saved_stack,
            locals: frame.locals,
            catch_stack: frame.catch_stack,
            finally_stack: frame.finally_stack,
            guard_seq,
            finally_completion_tag,
            finally_completion_val,
        })
    }

    pub(crate) fn instanceof_operator(
        &mut self,
        object: &Value,
        constructor: &Value,
    ) -> error::Result<bool> {
        if !matches!(constructor, Value::Object(_)) {
            return Err(Error::type_err(
                "Right-hand side of 'instanceof' is not an object".to_string(),
            ));
        }

        let has_instance_key = PropertyKey::Symbol(self.well_known_symbols.has_instance);
        let has_instance = self.get_property_by_key(constructor, &has_instance_key)?;
        if !has_instance.is_undefined() && !has_instance.is_null() {
            if !crate::builtins::is_callable(&has_instance, &self.heap) {
                return Err(Error::type_err(
                    "Symbol.hasInstance method is not callable".to_string(),
                ));
            }
            let result = self.call_function(
                &has_instance,
                std::slice::from_ref(object),
                Some(constructor.clone()),
            )?;
            return Ok(self.to_boolean(&result));
        }

        if !crate::builtins::is_callable(constructor, &self.heap) {
            return Err(Error::type_err(
                "Right-hand side of 'instanceof' is not callable".to_string(),
            ));
        }

        self.ordinary_has_instance(constructor, object)
    }

    pub(crate) fn ordinary_has_instance(
        &mut self,
        constructor: &Value,
        object: &Value,
    ) -> error::Result<bool> {
        if !crate::builtins::is_callable(constructor, &self.heap) {
            return Ok(false);
        }

        if let Value::Object(idx) = constructor {
            let bound_target = self.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Function(function) = obj {
                    if let crate::value::FunctionKind::Bound { target, .. } = &function.kind {
                        return Some(*target);
                    }
                }
                None
            });
            if let Some(target) = bound_target {
                return self.instanceof_operator(object, &Value::Object(target));
            }
        }

        if !matches!(object, Value::Object(_)) {
            return Ok(false);
        }

        let constructor_proto =
            self.get_property_by_key(constructor, &PropertyKey::from("prototype"))?;
        if !matches!(constructor_proto, Value::Object(_)) {
            return Err(Error::type_err(
                "Function has non-object prototype 'undefined' in instanceof check".to_string(),
            ));
        }

        let mut current = object.clone();
        while let Value::Object(object_idx) = &current {
            if Value::Object(*object_idx) == constructor_proto {
                return Ok(true);
            }
            current = self.get_prototype_of(&current)?.unwrap_or(Value::Undefined);
            if current.is_undefined() {
                break;
            }
        }
        Ok(false)
    }

    /// Resume (or start) a lazy generator, running until the next `yield` or
    /// until the body completes. The third result flag means `value` is an
    /// iterator result object forwarded unchanged by `yield*`.
    pub fn resume_generator(
        &mut self,
        g_idx: GcIdx,
        kind: ResumeKind,
    ) -> error::Result<(Value, bool, bool, bool)> {
        // Pull the saved execution state out of the generator object.
        let (
            fdef,
            env,
            this_val,
            args,
            mut ip,
            mut locals,
            mut stack,
            mut catch_stack,
            mut finally_stack,
            guard_seq,
            finally_completion_tag,
            finally_completion_val,
            started,
            done,
            delegating,
            delegate_await_kind,
        ) = self.heap.with_obj(g_idx.0, |o| {
            if let HeapObj::LazyGenerator(g) = o {
                (
                    g.fdef.clone(),
                    *g.env.lock(),
                    g.this_val.lock().clone(),
                    g.args.lock().clone(),
                    g.ip.load(Ordering::Relaxed),
                    g.locals.lock().clone(),
                    g.stack.lock().clone(),
                    g.catch_stack.lock().clone(),
                    g.finally_stack.lock().clone(),
                    g.guard_seq.load(Ordering::Relaxed),
                    g.finally_completion_tag.load(Ordering::Relaxed),
                    g.finally_completion_val.lock().clone(),
                    g.started.load(Ordering::Relaxed),
                    g.done.load(Ordering::Relaxed),
                    g.delegating.load(Ordering::Relaxed),
                    g.async_delegate_await_kind.load(Ordering::Relaxed),
                )
            } else {
                panic!("resume_generator on non-lazy-generator");
            }
        });

        if done {
            return match &kind {
                ResumeKind::Return(v) => Ok((v.clone(), true, false, false)),
                _ => Ok((Value::Undefined, true, false, false)),
            };
        }

        // Per spec, an unstarted generator's return() completes without
        // evaluating the body. A suspended generator's return() must instead
        // resume at the yield point with a return completion so finally blocks
        // can run.
        if !started {
            if let ResumeKind::Return(v) = &kind {
                self.heap.with_obj(g_idx.0, |o| {
                    if let HeapObj::LazyGenerator(g) = o {
                        g.done.store(true, Ordering::Relaxed);
                        g.started.store(true, Ordering::Relaxed);
                    }
                });
                return Ok((v.clone(), true, false, false));
            }
        }

        let resume_val = match &kind {
            ResumeKind::Next(v) => v.clone(),
            ResumeKind::Throw(e) => e.clone(),
            ResumeKind::Return(v) => v.clone(),
            ResumeKind::DelegateResult { value, .. } => value.clone(),
            ResumeKind::DelegateThrow(value) => value.clone(),
            ResumeKind::DelegateMissingThrow => Value::Undefined,
        };

        // On the first resume, either continue after the call-time generator
        // prologue or initialize legacy generator state created before that
        // prologue existed.
        if !started {
            if locals.is_empty() {
                locals = vec![Value::Undefined; fdef.num_locals.max(256)];
                for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
                    let slot = fdef.param_slots.get(i).copied().unwrap_or(i);
                    if slot < locals.len() {
                        locals[slot] = a.clone();
                    }
                }
                ip = 0;
                stack.clear();
                catch_stack.clear();
                finally_stack.clear();
            }
        } else if delegating {
            // `yield*` consumes the complete resume record itself.
        } else if matches!(kind, ResumeKind::Throw(_) | ResumeKind::Return(_)) {
            // Abrupt resumes do NOT push a resume value. They set a per-frame
            // flag below so dispatch injects the completion at the suspended
            // yield point.
        } else {
            // Resuming after a `yield`: the value sent via `next(v)` becomes the
            // result of the suspended `yield` expression.
            stack.push(resume_val.clone());
        }

        // Push the generator's frame.
        self.frames.push(CallFrame::new(
            fdef.chunk.clone(),
            ip,
            0,
            locals,
            env,
            this_val.clone(),
        ));
        if let Some(frame) = self.frames.last_mut() {
            frame.in_parameter_initializers = fdef.has_parameter_expressions;
        }
        // Restore the saved catch_stack onto the new frame.
        {
            let frame = self.current_frame_mut()?;
            frame.catch_stack = catch_stack;
            frame.finally_stack = finally_stack;
            frame.guard_seq.store(guard_seq, Ordering::Relaxed);
            frame
                .finally_completion_tag
                .store(finally_completion_tag, Ordering::Relaxed);
            *frame.finally_completion_val.lock() = finally_completion_val;
        }
        // Swap in a dedicated operand stack for the generator run, preserving
        // the caller's stack untouched. This keeps generator execution fully
        // isolated from the caller's operand values.
        let caller_stack = std::mem::replace(&mut self.stack, stack);

        // Set up the generator's own frame run-state. The gen-state lives on
        // the frame so a generator body that resumes *another* generator is
        // fully isolated (each frame carries its own state).
        let target_depth = self.frames.len() - 1;
        {
            let frame = &self.frames[target_depth];
            *frame.gen_resume_value.lock() = resume_val.clone();
            frame.gen_mode.store(true, Ordering::Relaxed);
            frame.gen_suspended.store(false, Ordering::Relaxed);
            frame.gen_awaiting.store(false, Ordering::Relaxed);
            frame.gen_delegating.store(delegating, Ordering::Relaxed);
            *frame.gen_yield.lock() = None;
            frame
                .gen_yield_is_iterator_result
                .store(false, Ordering::Relaxed);
            *frame.gen_delegate_resume.lock() = delegating.then(|| kind.clone());
            frame
                .gen_delegate_await_kind
                .store(delegate_await_kind, Ordering::Relaxed);
            // `throw(e)`: arrange for the next dispatch to raise `e`.
            if !delegating {
                if let ResumeKind::Throw(e) = &kind {
                    *frame.force_throw.lock() = Some(e.clone());
                }
            }
            // `return(v)`: arrange for the next dispatch to inject a return
            // completion at the suspended yield point.
            if !delegating {
                if let ResumeKind::Return(v) = &kind {
                    *frame.force_return.lock() = Some(v.clone());
                }
            }
        }

        let result = self.interpret_to_depth(target_depth);

        // Clear the resume value on the frame so a subsequent resume (or a
        // GC pass between resumes) does not observe a stale value.
        if self.frames.len() > target_depth {
            *self.frames[target_depth].gen_resume_value.lock() = Value::Undefined;
        }

        // Reclaim the generator's (possibly modified) operand stack and restore
        // the caller's stack.
        let gen_stack = std::mem::replace(&mut self.stack, caller_stack);

        // The generator frame is now either suspended (still on the stack at
        // target_depth) or completed (popped by Return/Halt).
        let suspended = if self.frames.len() > target_depth {
            self.frames[target_depth]
                .gen_suspended
                .load(Ordering::Relaxed)
        } else {
            false
        };

        // If the run ended in an uncaught exception (e.g. a `throw(e)` resume
        // whose exception was not caught by the generator body), propagate it.
        // The generator is marked done; its frame (if still on the stack) is
        // popped so the caller's catch routing can find the right handler.
        if let Err(e) = &result {
            if self.frames.len() > target_depth {
                self.frames.truncate(target_depth);
            }
            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.done.store(true, Ordering::Relaxed);
                    g.started.store(true, Ordering::Relaxed);
                    g.delegating.store(false, Ordering::Relaxed);
                    g.async_delegate_await_kind.store(0, Ordering::Relaxed);
                }
            });
            return Err(e.clone());
        }

        if suspended {
            // Capture the yielded value from the frame *before* popping it
            // (gen-state now lives on the frame, not the VM).
            let yielded = self.frames[target_depth]
                .gen_yield
                .lock()
                .take()
                .unwrap_or(Value::Undefined);
            let yielded_iterator_result = self.frames[target_depth]
                .gen_yield_is_iterator_result
                .load(Ordering::Relaxed);
            let awaiting = self.frames[target_depth]
                .gen_awaiting
                .load(Ordering::Relaxed);
            // Pop the generator frame and save its state for the next resume.
            let frame = self.frames.pop().ok_or_else(|| {
                crate::error::Error::internal("generator frame missing during resume")
            })?;
            // The generator's leftover operands are its private stack.
            let saved_stack = gen_stack;

            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.ip.store(frame.ip, Ordering::Relaxed);
                    *g.env.lock() = frame.env;
                    *g.locals.lock() = frame.locals;
                    *g.stack.lock() = saved_stack;
                    *g.catch_stack.lock() = frame.catch_stack;
                    *g.finally_stack.lock() = frame.finally_stack;
                    g.guard_seq
                        .store(frame.guard_seq.load(Ordering::Relaxed), Ordering::Relaxed);
                    g.finally_completion_tag.store(
                        frame.finally_completion_tag.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    *g.finally_completion_val.lock() = frame.finally_completion_val.lock().clone();
                    g.started.store(true, Ordering::Relaxed);
                    g.delegating.store(
                        frame.gen_delegating.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    g.async_delegate_await_kind.store(
                        frame.gen_delegate_await_kind.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                }
            });

            Ok((yielded, false, yielded_iterator_result, awaiting))
        } else {
            // Completed: the body returned or ran off the end. `result` holds
            // the return value; mark the generator done.
            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.done.store(true, Ordering::Relaxed);
                    g.started.store(true, Ordering::Relaxed);
                    g.delegating.store(false, Ordering::Relaxed);
                    g.async_delegate_await_kind.store(0, Ordering::Relaxed);
                }
            });
            let ret = result.unwrap_or(Value::Undefined);
            Ok((ret, true, false, false))
        }
    }

    fn interpret(&mut self) -> error::Result<Value> {
        self.with_current_frame_execution_context(|vm| vm.interpret_catch(None, None))
    }

    fn interpret_to_depth(&mut self, target_depth: usize) -> error::Result<Value> {
        self.with_current_frame_execution_context(|vm| {
            vm.interpret_catch(Some(target_depth), None)
                .map_err(|error| vm.materialize_current_interpreted_error(error))
        })
    }

    fn interpret_to_depth_until_ip(
        &mut self,
        target_depth: usize,
        stop_ip: usize,
    ) -> error::Result<Value> {
        self.with_current_frame_execution_context(|vm| {
            vm.interpret_catch(Some(target_depth), Some((target_depth, stop_ip)))
        })
    }

    /// Build a catchable `Error` object for a native (non-thrown) error, so
    /// `try/catch` receives a real object with `message` and `name`.
    pub(crate) fn make_error_value(&mut self, e: &Error) -> error::Result<Value> {
        let error_env = self.current_realm_global_env();
        self.make_error_value_in_realm(e, error_env)
    }

    pub(crate) fn make_error_value_in_realm(
        &mut self,
        e: &Error,
        error_env: GcIdx,
    ) -> error::Result<Value> {
        use crate::value::{ObjectData, PropertyDescriptor};
        let ctor_name = match e.kind {
            crate::error::ErrorKind::Type => "TypeError",
            crate::error::ErrorKind::Range => "RangeError",
            crate::error::ErrorKind::Reference => "ReferenceError",
            crate::error::ErrorKind::Syntax => "SyntaxError",
            crate::error::ErrorKind::Eval => "EvalError",
            crate::error::ErrorKind::Uri => "URIError",
            _ => "Error",
        };
        let proto = self
            .realm_error_prototypes
            .get(&(error_env.0, Arc::from(ctor_name)))
            .cloned()
            .or_else(|| {
                self.realm_error_prototypes
                    .get(&(error_env.0, Arc::from("Error")))
                    .cloned()
            })
            .or_else(
                || match crate::environment::get(&self.heap, error_env, ctor_name) {
                    Some(Value::Object(ci)) => self.heap.with_obj(ci.0, |o| {
                        o.props()
                            .lock()
                            .get(&crate::value::PropertyKey::from("prototype"))
                            .map(|d| d.value.clone())
                    }),
                    _ => None,
                },
            )
            .unwrap_or(self.error_proto.clone());
        let mut props = IndexMap::new();
        props.insert(
            crate::value::PropertyKey::from("name"),
            PropertyDescriptor::data(Value::String(Arc::from(ctor_name))),
        );
        props.insert(
            crate::value::PropertyKey::from("message"),
            PropertyDescriptor::data(Value::String(Arc::from(e.message.as_str()))),
        );
        props.insert(
            crate::value::PropertyKey::from("stack"),
            PropertyDescriptor::data(Value::String(Arc::from(e.stack.join("\n").as_str()))),
        );
        let obj = HeapObj::Object(ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Error")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(obj)?)))
    }

    /// Preserve JavaScript throws, materialize native errors in the operation
    /// Realm, and keep host aborts outside the Promise rejection channel.
    pub(crate) fn promise_rejection_reason_in_realm(
        &mut self,
        error: &Arc<Error>,
        realm: GcIdx,
    ) -> error::Result<Value> {
        if !error.catchable() {
            return Err(error.clone());
        }
        match error.thrown_value.clone() {
            Some(reason) => Ok(reason),
            None => self.make_error_value_in_realm(error, realm),
        }
    }

    pub(crate) fn materialize_error_in_realm(
        &mut self,
        error: Arc<Error>,
        error_env: GcIdx,
    ) -> Arc<Error> {
        if !error.catchable() || error.thrown_value.is_some() {
            return error;
        }
        match self.make_error_value_in_realm(&error, error_env) {
            Ok(thrown) => Error::thrown(thrown, &self.heap),
            Err(materialization_error) => materialization_error,
        }
    }

    /// Run the dispatch loop, routing runtime errors to an active try/catch
    /// handler when one is present on the current frame's catch stack. A JS
    /// `throw` already routes through `Op::Throw`; this wrapper additionally
    /// converts errors raised by builtins/operators (TypeError, ReferenceError,
    /// ...) into catchable exceptions so that `try { f() } catch(e)` works for
    /// native errors too.
    fn interpret_catch(
        &mut self,
        return_depth: Option<usize>,
        stop_at: Option<(usize, usize)>,
    ) -> error::Result<Value> {
        loop {
            match self.interpret_inner(return_depth, stop_at) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if !e.catchable() {
                        return Err(e);
                    }

                    let has_guard = self
                        .frames
                        .last()
                        .is_some_and(|f| !f.catch_stack.is_empty() || !f.finally_stack.is_empty());
                    if !has_guard {
                        return Err(e);
                    }

                    let thrown = match e.thrown_value.clone() {
                        Some(v) => v,
                        None => {
                            let error_env = self.current_interpreted_realm_global_env();
                            self.make_error_value_in_realm(&e, error_env)?
                        }
                    };

                    let divert_to_finally = self.frames.last().is_some_and(|frame| {
                        match (frame.finally_stack.last(), frame.catch_stack.last()) {
                            (Some(&(_, _)), None) => true,
                            (Some(&(_, fseq)), Some(&(_, cseq, _, _))) => fseq > cseq,
                            _ => false,
                        }
                    });
                    if divert_to_finally {
                        let target = self
                            .frames
                            .last()
                            .and_then(|f| f.finally_stack.last().map(|(ip, _)| *ip))
                            .ok_or_else(|| {
                                crate::error::Error::internal(
                                    "finally stack empty during native error diversion",
                                )
                            })?;
                        let frame = self.current_frame_mut()?;
                        frame.finally_completion_tag.store(4, Ordering::Relaxed);
                        *frame.finally_completion_val.lock() = thrown;
                        frame.ip = target;
                        continue;
                    }

                    // If a catch handler is active, convert the error to a
                    // thrown value and resume at the handler.
                    let handler = self
                        .frames
                        .last()
                        .and_then(|f| f.catch_stack.last().map(|(ip, _, _, _)| *ip));
                    if let Some(handler) = handler {
                        // Pop the handler so we don't loop, push the thrown value
                        // for the catch binding, and jump to the handler ip.
                        let (_, _, saved_env, saved_stack_depth) =
                            self.current_frame_mut()?.catch_stack.pop().unwrap();
                        {
                            let frame = self.current_frame_mut()?;
                            frame.finally_completion_tag.store(0, Ordering::Relaxed);
                            *frame.finally_completion_val.lock() = Value::Undefined;
                        }
                        // Unwind scopes opened in the try body.
                        let stack_target = {
                            let frame = self.current_frame_mut()?;
                            frame.env = saved_env;
                            frame.stack_base + saved_stack_depth
                        };
                        self.stack.truncate(stack_target);
                        self.stack.push(thrown);
                        self.current_frame_mut()?.ip = handler;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }

    fn interpret_inner(
        &mut self,
        return_depth: Option<usize>,
        stop_at: Option<(usize, usize)>,
    ) -> error::Result<Value> {
        match self.interpret_inner_raw(return_depth, stop_at) {
            Ok(v) => Ok(v),
            Err(e) => {
                // Stamp the source line of the faulting instruction (the
                // current frame's ip, stepped back one to point at the op that
                // raised). Only the first occurrence is kept.
                let line = self.frames.last().and_then(|f| {
                    let ip = f.ip.saturating_sub(1);
                    f.chunk.line_for_ip(ip)
                });
                Err(e.with_line(line))
            }
        }
    }
}

enum FuncCallInfo {
    Native {
        func: NativeFn,
        closure: GcIdx,
    },
    Interpreted {
        func: std::sync::Arc<crate::function::FunctionDef>,
        closure: GcIdx,
        lexical_new_target: Value,
        home_object: Option<Value>,
        is_arrow: bool,
        is_async: bool,
        is_class_ctor: bool,
    },
    Bound {
        target: GcIdx,
        this_val: Value,
        bound_args: Vec<Value>,
    },
}

impl Vm {
    pub fn to_string_pub(&mut self, v: &Value) -> error::Result<String> {
        Ok(self.to_string(v)?.to_string())
    }

    pub(crate) fn with_execution_context<T>(
        &mut self,
        context: ExecutionContext,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let depth = self.execution_contexts.len();
        self.execution_contexts.push(context);
        let result = operation(self);
        debug_assert_eq!(self.execution_contexts.len(), depth + 1);
        self.execution_contexts.truncate(depth);
        result
    }

    fn with_current_frame_execution_context<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        // Call setup already owns an interpreted context, but a frame context
        // must sit above it while bytecode runs and when a suspended frame is
        // later resumed beneath an unrelated native caller.
        let Some((realm_env, callee)) = self
            .frames
            .last()
            .map(|frame| (frame.env, frame.callee.clone()))
        else {
            return operation(self);
        };
        self.with_execution_context(
            ExecutionContext {
                realm_env,
                kind: ExecutionContextKind::Interpreted { callee },
            },
            operation,
        )
    }

    pub(crate) fn current_native_callee(&self) -> Option<&Value> {
        match &self.execution_contexts.last()?.kind {
            ExecutionContextKind::Native { callee, .. } => Some(callee),
            ExecutionContextKind::Interpreted { .. } => None,
        }
    }

    pub(crate) fn current_native_new_target(&self) -> Option<&Value> {
        match &self.execution_contexts.last()?.kind {
            ExecutionContextKind::Native { new_target, .. } => new_target.as_ref(),
            ExecutionContextKind::Interpreted { .. } => None,
        }
    }

    pub(crate) fn current_native_new_target_prototype(&self) -> Option<&Value> {
        match &self.execution_contexts.last()?.kind {
            ExecutionContextKind::Native {
                new_target_prototype,
                ..
            } => new_target_prototype.as_ref(),
            ExecutionContextKind::Interpreted { .. } => None,
        }
    }

    pub(crate) fn current_realm_global_env(&self) -> GcIdx {
        let env = self
            .execution_contexts
            .last()
            .map(|context| context.realm_env)
            .or_else(|| self.frames.last().map(|frame| frame.env))
            .unwrap_or(self.global);
        crate::environment::global_env_root(&self.heap, env)
    }

    pub(crate) fn current_interpreted_realm_global_env(&self) -> GcIdx {
        let context_env = self
            .execution_contexts
            .last()
            .and_then(|context| match &context.kind {
                ExecutionContextKind::Interpreted { .. } => Some(context.realm_env),
                ExecutionContextKind::Native { .. } => None,
            });
        let env = context_env
            .or_else(|| self.frames.last().map(|frame| frame.env))
            .unwrap_or(self.global);
        crate::environment::global_env_root(&self.heap, env)
    }

    fn materialize_current_interpreted_error(&mut self, error: Arc<Error>) -> Arc<Error> {
        if !error.catchable() || error.thrown_value.is_some() {
            return error;
        }
        let error_env = self.current_interpreted_realm_global_env();
        match self.make_error_value_in_realm(&error, error_env) {
            Ok(thrown) => error.with_thrown_value(thrown),
            Err(materialization_error) => materialization_error,
        }
    }

    pub(crate) fn realm_global_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_globals
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.global_this.clone())
    }

    pub(crate) fn object_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_object_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.object_proto.clone())
    }

    pub(crate) fn array_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_array_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.array_proto.clone())
    }

    pub(crate) fn error_prototype_for_env(&self, name: &str, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_error_prototypes
            .get(&(realm.0, Arc::from(name)))
            .cloned()
            .or_else(|| {
                self.realm_error_prototypes
                    .get(&(realm.0, Arc::from("Error")))
                    .cloned()
            })
            .unwrap_or_else(|| self.error_proto.clone())
    }

    pub(crate) fn primitive_prototype_for_env(&self, value: &Value, env: GcIdx) -> Value {
        let kind = match value {
            Value::String(_) => PrimitivePrototypeKind::String,
            Value::Number(_) => PrimitivePrototypeKind::Number,
            Value::BigInt(_) => PrimitivePrototypeKind::BigInt,
            Value::Bool(_) => PrimitivePrototypeKind::Boolean,
            Value::Symbol(_) => PrimitivePrototypeKind::Symbol,
            _ => return Value::Undefined,
        };
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_primitive_prototypes
            .get(&(realm.0, kind))
            .cloned()
            .unwrap_or_else(|| match kind {
                PrimitivePrototypeKind::String => self.string_proto.clone(),
                PrimitivePrototypeKind::Number => self.number_proto.clone(),
                PrimitivePrototypeKind::BigInt => self.bigint_proto.clone(),
                PrimitivePrototypeKind::Boolean => self.boolean_proto.clone(),
                PrimitivePrototypeKind::Symbol => self.symbol_proto.clone(),
            })
    }

    pub(crate) fn current_realm_primitive_prototype(&self, value: &Value) -> Value {
        self.primitive_prototype_for_env(value, self.current_realm_global_env())
    }

    pub(crate) fn promise_constructor_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_promise_constructors
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.promise_ctor.clone())
    }

    pub(crate) fn promise_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_promise_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.promise_proto.clone())
    }

    pub(crate) fn current_realm_promise_constructor(&self) -> Value {
        self.promise_constructor_for_env(self.current_realm_global_env())
    }

    pub(crate) fn current_realm_promise_prototype(&self) -> Value {
        self.promise_prototype_for_env(self.current_realm_global_env())
    }

    pub(crate) fn generator_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_generator_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.generator_proto.clone())
    }

    pub(crate) fn generator_function_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_generator_function_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.generator_function_proto.clone())
    }

    pub(crate) fn async_generator_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_async_generator_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.async_generator_proto.clone())
    }

    pub(crate) fn async_generator_function_prototype_for_env(&self, env: GcIdx) -> Value {
        let realm = crate::environment::global_env_root(&self.heap, env);
        self.realm_async_generator_function_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.async_generator_function_proto.clone())
    }

    /// Resolve an identifier to a spec Reference record, using the same
    /// environment/with/global search order for reads, writes, calls, and
    /// `typeof`. This is the Reference-creation half of IdentifierReference
    /// evaluation; callers still decide whether to apply GetValue.
    pub(crate) fn resolve_identifier_reference(
        &mut self,
        name: crate::value::PropertyKey,
        strict: bool,
    ) -> error::Result<crate::value::ReferenceRecord> {
        let name_str = name.as_str().unwrap_or_default().to_string();
        let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
        let mut base = crate::value::ReferenceBase::Unresolvable;
        let mut cur_env = Some(env);
        while let Some(e_idx) = cur_env {
            let (has_binding, has_with, with_obj_val, parent) =
                self.heap.with_obj(e_idx.0, |obj| {
                    if let HeapObj::Environment(e) = obj {
                        if e.vars.lock().contains_key(name_str.as_str()) {
                            return (true, false, None, None);
                        }
                        if let Some(with_obj) = e.with_object.lock().clone() {
                            return (false, true, Some(with_obj), *e.parent.lock());
                        }
                        return (false, false, None, *e.parent.lock());
                    }
                    (false, false, None, None)
                });
            if has_binding {
                base = crate::value::ReferenceBase::Environment(e_idx);
                break;
            }
            if has_with {
                if let Some(with_obj) = with_obj_val {
                    if self.with_object_has_binding(&with_obj, &name_str)? {
                        base = crate::value::ReferenceBase::ObjectEnvironment(Box::new(with_obj));
                        break;
                    }
                }
            }
            cur_env = parent;
        }
        if cur_env.is_none() {
            let realm = crate::environment::global_env_root(&self.heap, env);
            let global_this = self.realm_global_for_env(realm);
            if self.has_property(&global_this, &name_str)? {
                base = crate::value::ReferenceBase::Environment(realm);
            }
        }
        Ok(crate::value::ReferenceRecord {
            base,
            name: name.into(),
            strict,
            this_value: None,
        })
    }

    pub(crate) fn get_private_value(
        &mut self,
        obj: &Value,
        name: &crate::value::PrivateNameKey,
    ) -> error::Result<Value> {
        let key = crate::value::PrivateSlotKey::Private(name.clone());
        let slot = if let Value::Object(idx) = obj {
            self.heap.get_private_element(idx.0, &key)
        } else {
            None
        };
        match slot {
            Some(crate::value::PrivateSlot::Value(value))
            | Some(crate::value::PrivateSlot::Method(value)) => Ok(value),
            Some(crate::value::PrivateSlot::Accessor { get: Some(get), .. }) => {
                self.call_function(&get, &[], Some(obj.clone()))
            }
            Some(crate::value::PrivateSlot::Accessor { get: None, .. }) => {
                Err(Error::type_err("Private accessor has no getter"))
            }
            None => Err(Error::type_err("Private field is not present")),
        }
    }

    pub(crate) fn set_private_value(
        &mut self,
        obj: &Value,
        name: &crate::value::PrivateNameKey,
        value: Value,
    ) -> error::Result<()> {
        let key = crate::value::PrivateSlotKey::Private(name.clone());
        let setter = if let Value::Object(idx) = obj {
            self.heap
                .with_private_elements(idx.0, |fields| match fields.get(&key) {
                    Some(crate::value::PrivateSlot::Accessor {
                        set: Some(setter), ..
                    }) => Ok(Some(setter.clone())),
                    Some(crate::value::PrivateSlot::Accessor { set: None, .. }) => {
                        Err(Error::type_err("Private accessor has no setter"))
                    }
                    Some(crate::value::PrivateSlot::Value(_)) => {
                        fields.insert(key, crate::value::PrivateSlot::Value(value.clone()));
                        Ok(None)
                    }
                    Some(crate::value::PrivateSlot::Method(_)) => {
                        Err(Error::type_err("Cannot assign to private method"))
                    }
                    None => Err(Error::type_err("Private field is not present")),
                })?
        } else {
            return Err(Error::type_err("Private receiver is not an object"));
        };
        if let Some(setter) = setter {
            self.call_function(&setter, std::slice::from_ref(&value), Some(obj.clone()))?;
        }
        Ok(())
    }

    pub(crate) fn add_private_element(
        &self,
        obj: &Value,
        key: crate::value::PrivateSlotKey,
        slot: crate::value::PrivateSlot,
        duplicate_message: &'static str,
    ) -> error::Result<()> {
        let Value::Object(idx) = obj else {
            return Err(Error::type_err("Private receiver is not an object"));
        };
        if !self.heap.with_obj(idx.0, |object| object.is_extensible()) {
            return Err(Error::type_err(
                "Cannot add private field to non-extensible object",
            ));
        }
        self.heap.with_private_elements(idx.0, |fields| {
            if fields.contains_key(&key) {
                return Err(Error::type_err(duplicate_message));
            }
            fields.insert(key, slot);
            Ok(())
        })
    }

    pub(crate) fn define_private_accessor_element(
        &self,
        obj: &Value,
        key: crate::value::PrivateSlotKey,
        getter: Value,
        setter: Value,
    ) -> error::Result<()> {
        let Value::Object(idx) = obj else {
            return Err(Error::type_err("Private receiver is not an object"));
        };
        let extensible = self.heap.with_obj(idx.0, |object| object.is_extensible());
        self.heap.with_private_elements(idx.0, |fields| {
            if !fields.contains_key(&key) && !extensible {
                return Err(Error::type_err(
                    "Cannot add private field to non-extensible object",
                ));
            }
            match fields.get_mut(&key) {
                None => {
                    fields.insert(
                        key,
                        crate::value::PrivateSlot::Accessor {
                            get: (!getter.is_undefined()).then_some(getter),
                            set: (!setter.is_undefined()).then_some(setter),
                        },
                    );
                }
                Some(crate::value::PrivateSlot::Accessor { get, set }) => {
                    if !getter.is_undefined() {
                        if get.is_some() {
                            return Err(Error::type_err(
                                "Cannot initialize private accessor twice",
                            ));
                        }
                        *get = Some(getter);
                    }
                    if !setter.is_undefined() {
                        if set.is_some() {
                            return Err(Error::type_err(
                                "Cannot initialize private accessor twice",
                            ));
                        }
                        *set = Some(setter);
                    }
                }
                Some(
                    crate::value::PrivateSlot::Value(_) | crate::value::PrivateSlot::Method(_),
                ) => {
                    return Err(Error::type_err("Cannot initialize private accessor twice"));
                }
            }
            Ok(())
        })
    }

    /// Spec GetValue (6.2.4.2): if `v` is a Reference, resolve it to its
    /// value; otherwise return `v` unchanged. For environment-record-based
    /// references (identifier lookups), this walks the env chain. For
    /// property references, this calls [[Get]] on the base object.
    pub(crate) fn get_value(&mut self, v: &Value) -> error::Result<Value> {
        match v {
            Value::Reference(r) => {
                let r = r.clone();
                match &r.base {
                    crate::value::ReferenceBase::Unresolvable => {
                        let name = r.name.as_str().map(|s| s.to_string()).unwrap_or_default();
                        Err(Error::reference(format!("{} is not defined", name)))
                    }
                    crate::value::ReferenceBase::Environment(env_idx) => {
                        let name = r.name.as_str().map(|s| s.to_string()).unwrap_or_default();
                        let global_env = crate::environment::global_env_root(&self.heap, *env_idx);
                        if *env_idx == global_env {
                            let is_global_var = self.heap.with_obj(env_idx.0, |obj| {
                                matches!(
                                    obj,
                                    HeapObj::Environment(environment)
                                        if environment.vars.lock().get(name.as_str()).is_some_and(
                                            |binding| binding.kind
                                                == crate::value::BindingKind::Var
                                        )
                                )
                            });
                            if is_global_var {
                                let global_this = self.realm_global_for_env(*env_idx);
                                if self.has_property(&global_this, &name)? {
                                    return self.get_property(&global_this, &name);
                                }
                            }
                        }
                        // Walk the environment chain in order: at each env,
                        // check var/let/const bindings first, then the
                        // with-object (if any). This ensures a var binding in
                        // a child scope shadows a with-object property on a
                        // parent scope, and a with-object property shadows an
                        // outer var binding.
                        let mut cur_env = Some(*env_idx);
                        while let Some(e_idx) = cur_env {
                            let (binding_val, indirect, in_tdz, has_with, with_obj_val, parent) =
                                self.heap.with_obj(e_idx.0, |obj| {
                                    if let HeapObj::Environment(e) = obj {
                                        if let Some(b) = e.vars.lock().get(name.as_str()) {
                                            if !b.initialized.load(Ordering::Relaxed) {
                                                return (None, None, true, false, None, None);
                                            }
                                            return (
                                                Some(b.value.lock().clone()),
                                                b.indirect.clone(),
                                                false,
                                                false,
                                                None,
                                                None,
                                            );
                                        }
                                        if let Some(with_obj) = e.with_object.lock().clone() {
                                            return (
                                                None,
                                                None,
                                                false,
                                                true,
                                                Some(with_obj),
                                                *e.parent.lock(),
                                            );
                                        }
                                        return (None, None, false, false, None, *e.parent.lock());
                                    }
                                    (None, None, false, false, None, None)
                                });
                            if in_tdz {
                                return Err(Error::reference(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )));
                            }
                            if indirect.is_some() {
                                return match crate::environment::get_checked(
                                    &self.heap, e_idx, &name,
                                ) {
                                    Ok(Some(value)) => Ok(value),
                                    Ok(None) | Err(false) => {
                                        Err(Error::reference(format!("{} is not defined", name)))
                                    }
                                    Err(true) => Err(Error::reference(format!(
                                        "Cannot access '{}' before initialization",
                                        name
                                    ))),
                                };
                            }
                            if let Some(v) = binding_val {
                                return Ok(v);
                            }
                            if has_with {
                                if let Some(with_obj) = with_obj_val {
                                    if self.with_object_has_binding(&with_obj, &name)? {
                                        if !self.has_property(&with_obj, &name)? {
                                            if r.strict {
                                                return Err(Error::reference(format!(
                                                    "{} is not defined",
                                                    name
                                                )));
                                            }
                                            return Ok(Value::Undefined);
                                        }
                                        return self.get_property(&with_obj, &name);
                                    }
                                }
                            }
                            cur_env = parent;
                        }
                        // Last resort: check global (this).
                        let global_this = self.realm_global_for_env(*env_idx);
                        let has = self.has_property(&global_this, &name)?;
                        if has {
                            self.get_property(&global_this, &name)
                        } else {
                            Err(Error::reference(format!("{} is not defined", name)))
                        }
                    }
                    crate::value::ReferenceBase::Value(base) => match &r.name {
                        crate::value::ReferencedName::Property(name) => {
                            if let Some(receiver) = &r.this_value {
                                if base.is_nullish() {
                                    return Err(Error::type_err(
                                        "Cannot read property from null super base",
                                    ));
                                }
                                self.get_property_key_rx(base, name, *receiver.clone(), 0)
                            } else {
                                self.get_property_reference_value(base, name)
                            }
                        }
                        crate::value::ReferencedName::UncoercedProperty(name) => {
                            if base.is_nullish() {
                                return Err(Error::type_err(
                                    "Cannot read property from null super base",
                                ));
                            }
                            let pin_count = self.pin(v);
                            let name_result = self.coerce_property_key_record(name);
                            self.unpin_many(pin_count);
                            let name = name_result?;
                            if let Some(receiver) = &r.this_value {
                                self.get_property_key_rx(base, &name, *receiver.clone(), 0)
                            } else {
                                self.get_property_reference_value(base, &name)
                            }
                        }
                        crate::value::ReferencedName::Private(name) => {
                            self.get_private_value(base, name)
                        }
                    },
                    crate::value::ReferenceBase::ObjectEnvironment(base) => match &r.name {
                        crate::value::ReferencedName::Property(crate::value::PropertyKey::Str(
                            s,
                        )) => {
                            if !self.has_property(base, s)? {
                                if r.strict {
                                    return Err(Error::reference(format!("{} is not defined", s)));
                                }
                                return Ok(Value::Undefined);
                            }
                            self.get_property(base, s)
                        }
                        crate::value::ReferencedName::Property(
                            crate::value::PropertyKey::Symbol(id),
                        ) => self.get_property(base, &format!("[Symbol {}]", id)),
                        crate::value::ReferencedName::UncoercedProperty(_)
                        | crate::value::ReferencedName::Private(_) => Err(Error::internal(
                            "invalid reference name for an object environment base",
                        )),
                    },
                }
            }
            _ => Ok(v.clone()),
        }
    }

    fn get_property_reference_value(
        &mut self,
        base: &Value,
        name: &crate::value::PropertyKey,
    ) -> error::Result<Value> {
        let crate::value::PropertyKey::Str(key) = name else {
            let key = Self::property_key_to_value(name);
            return self.get_property_key(base, &key);
        };

        let Value::Object(idx) = base else {
            return self.get_property(base, key);
        };
        let cacheable_own_data = self.heap.with_obj(idx.0, |object| {
            if matches!(
                object,
                crate::value::HeapObj::Array(array)
                    if array
                        .is_arguments
                        .load(std::sync::atomic::Ordering::Relaxed)
            ) {
                return false;
            }
            object
                .props()
                .lock()
                .get(name)
                .is_some_and(|desc| !desc.is_accessor)
        });
        if !cacheable_own_data {
            return self.get_property(base, key);
        }
        if let Some(cached) = self.ic_get(idx.0, key) {
            return Ok(cached);
        }
        let value = self.get_property(base, key)?;
        self.ic_put(idx.0, key.to_string(), value.clone());
        Ok(value)
    }

    fn delete_environment_reference(&mut self, env_idx: GcIdx, name: &str) -> error::Result<bool> {
        let global_env = crate::environment::global_env_root(&self.heap, env_idx);
        let is_global = env_idx == global_env;
        let binding = self.heap.with_obj(env_idx.0, |obj| {
            if let HeapObj::Environment(env) = obj {
                env.vars
                    .lock()
                    .get(name)
                    .map(|binding| (binding.kind, binding.deletable))
            } else {
                None
            }
        });

        if let Some((kind, deletable)) = binding {
            if !is_global || kind != crate::value::BindingKind::Var {
                if deletable {
                    crate::environment::delete_var_binding(&self.heap, env_idx, name);
                    return Ok(true);
                }
                return Ok(false);
            }

            let global_this = self.realm_global_for_env(global_env);
            let deleted =
                self.delete_property_key(&global_this, &crate::value::PropertyKey::from(name))?;
            if deleted {
                crate::environment::delete_var_binding(&self.heap, env_idx, name);
            }
            return Ok(deleted);
        }

        if is_global {
            let global_this = self.realm_global_for_env(global_env);
            return self.delete_property_key(&global_this, &crate::value::PropertyKey::from(name));
        }
        Ok(true)
    }

    /// Delete through an evaluated Reference. Property bases are boxed before
    /// a raw referenced name is coerced, matching DeleteExpression.
    pub(crate) fn delete_value(&mut self, v: &Value) -> error::Result<bool> {
        let Value::Reference(reference) = v else {
            return Ok(true);
        };
        let reference = reference.clone();
        if reference.this_value.is_some() {
            return Err(Error::reference("Cannot delete super property"));
        }
        let base = match &reference.base {
            crate::value::ReferenceBase::Unresolvable => return Ok(true),
            crate::value::ReferenceBase::Environment(env_idx) => {
                let name = reference
                    .name
                    .as_str()
                    .ok_or_else(|| Error::internal("invalid environment reference name"))?;
                return self.delete_environment_reference(*env_idx, name);
            }
            crate::value::ReferenceBase::ObjectEnvironment(base) => {
                let key = match &reference.name {
                    crate::value::ReferencedName::Property(key) => key.clone(),
                    crate::value::ReferencedName::UncoercedProperty(_)
                    | crate::value::ReferencedName::Private(_) => {
                        return Err(Error::internal("invalid object environment reference name"));
                    }
                };
                let pin_count = self.pin(v);
                let result = self.delete_property_key(base, &key);
                self.unpin_many(pin_count);
                let deleted = result?;
                if !deleted && reference.strict {
                    return Err(Error::type_err("Cannot delete non-configurable property"));
                }
                return Ok(deleted);
            }
            crate::value::ReferenceBase::Value(base) => base,
        };
        if base.is_nullish() {
            return Err(Error::type_err(
                "Cannot convert undefined or null to object",
            ));
        }
        if matches!(reference.name, crate::value::ReferencedName::Private(_)) {
            return Err(Error::internal("cannot delete a private reference"));
        }

        let mut pin_count = self.pin(v);
        let object = match self.to_object(base) {
            Ok(object) => object,
            Err(error) => {
                self.unpin_many(pin_count);
                return Err(error);
            }
        };
        pin_count += self.pin(&object);
        let key_result = match &reference.name {
            crate::value::ReferencedName::Property(key) => Ok(key.clone()),
            crate::value::ReferencedName::UncoercedProperty(name) => {
                self.coerce_property_key_record(name)
            }
            crate::value::ReferencedName::Private(_) => unreachable!(),
        };
        let result = match key_result {
            Ok(key) => self.delete_property_key(&object, &key),
            Err(error) => Err(error),
        };
        self.unpin_many(pin_count);

        let deleted = result?;
        if !deleted && reference.strict {
            return Err(Error::type_err("Cannot delete non-configurable property"));
        }
        Ok(deleted)
    }

    /// Spec PutValue (6.2.4.3): if `v` is a Reference, store `value` into
    /// the referenced binding/property. For environment-record-based
    /// references, this uses SetMutableBinding on the reference's env (not
    /// the current env), preserving the original binding even if it was
    /// deleted between GetValue and PutValue (the `with`+compound-assign case).
    /// For property references, this calls [[Set]] on the base object.
    pub(crate) fn put_value(&mut self, v: &Value, value: Value) -> error::Result<()> {
        match v {
            Value::Reference(r) => {
                let r = r.clone();
                match &r.base {
                    crate::value::ReferenceBase::Unresolvable => {
                        let name = r.name.as_str().map(|s| s.to_string()).unwrap_or_default();
                        if r.strict {
                            return Err(Error::reference(format!("{} is not defined", name)));
                        }
                        let global_this =
                            self.realm_global_for_env(self.current_realm_global_env());
                        self.set_property(&global_this, &name, value)?;
                    }
                    crate::value::ReferenceBase::Environment(env_idx) => {
                        let name = r.name.as_str().map(|s| s.to_string()).unwrap_or_default();
                        let global_env = crate::environment::global_env_root(&self.heap, *env_idx);
                        if *env_idx != global_env {
                            match crate::environment::set_checked_exact(
                                &self.heap,
                                *env_idx,
                                &name,
                                value.clone(),
                            ) {
                                crate::environment::SetOutcome::Set => return Ok(()),
                                crate::environment::SetOutcome::Tdz => {
                                    return Err(Error::reference(format!(
                                        "Cannot access '{}' before initialization",
                                        name
                                    )));
                                }
                                crate::environment::SetOutcome::Const
                                | crate::environment::SetOutcome::Import => {
                                    return Err(Error::type_err(format!(
                                        "Assignment to constant variable '{}'",
                                        name
                                    )));
                                }
                                crate::environment::SetOutcome::FunctionName => {
                                    if r.strict {
                                        return Err(Error::type_err(format!(
                                            "Assignment to constant variable '{}'",
                                            name
                                        )));
                                    }
                                    return Ok(());
                                }
                                crate::environment::SetOutcome::NotFound => {
                                    if r.strict {
                                        return Err(Error::reference(format!(
                                            "{} is not defined",
                                            name
                                        )));
                                    }
                                    crate::environment::create_mutable_binding_exact(
                                        &self.heap, *env_idx, &name, value,
                                    );
                                    return Ok(());
                                }
                            }
                        }
                        // Global Environment Records mirror `var` bindings to the
                        // Realm's global object. Route those writes through the
                        // actual property so accessors and descriptor failures are
                        // observable before synchronizing the declarative mirror.
                        let global_readonly =
                            self.realm_global_property_is_non_writable_data(*env_idx, &name);
                        let mut cur_env = Some(*env_idx);
                        while let Some(e_idx) = cur_env {
                            let (
                                has_binding,
                                is_const,
                                is_function_name,
                                in_tdz,
                                global_readonly_binding,
                                global_var_binding,
                                has_with,
                                with_obj_val,
                                parent,
                            ) = self.heap.with_obj(e_idx.0, |obj| {
                                if let HeapObj::Environment(e) = obj {
                                    // Check var/let/const bindings.
                                    if let Some(b) = e.vars.lock().get(name.as_str()) {
                                        if !b.initialized.load(Ordering::Relaxed) {
                                            return (
                                                false, false, false, true, false, false, false,
                                                None, None,
                                            );
                                        }
                                        if e_idx == global_env && global_readonly {
                                            return (
                                                false, false, false, false, true, false, false,
                                                None, None,
                                            );
                                        }
                                        if b.kind == crate::value::BindingKind::FunctionName {
                                            return (
                                                false, false, true, false, false, false, false,
                                                None, None,
                                            );
                                        }
                                        if matches!(
                                            b.kind,
                                            crate::value::BindingKind::Const
                                                | crate::value::BindingKind::Import
                                        ) {
                                            return (
                                                false, true, false, false, false, false, false,
                                                None, None,
                                            );
                                        }
                                        let is_global_var = e_idx == global_env
                                            && b.kind == crate::value::BindingKind::Var;
                                        if !is_global_var {
                                            *b.value.lock() = value.clone();
                                        }
                                        return (
                                            true,
                                            false,
                                            false,
                                            false,
                                            false,
                                            is_global_var,
                                            false,
                                            None,
                                            None,
                                        );
                                    }
                                    // Check with-object.
                                    if let Some(with_obj) = e.with_object.lock().clone() {
                                        return (
                                            false,
                                            false,
                                            false,
                                            false,
                                            false,
                                            false,
                                            true,
                                            Some(with_obj),
                                            *e.parent.lock(),
                                        );
                                    }
                                    return (
                                        false,
                                        false,
                                        false,
                                        false,
                                        false,
                                        false,
                                        false,
                                        None,
                                        *e.parent.lock(),
                                    );
                                }
                                (false, false, false, false, false, false, false, None, None)
                            });
                            if in_tdz {
                                return Err(Error::reference(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )));
                            }
                            if is_const {
                                return Err(Error::type_err(format!(
                                    "Assignment to constant variable '{}'",
                                    name
                                )));
                            }
                            if is_function_name {
                                if r.strict {
                                    return Err(Error::type_err(format!(
                                        "Assignment to constant variable '{}'",
                                        name
                                    )));
                                }
                                return Ok(());
                            }
                            if global_readonly_binding {
                                let global_this = self.realm_global_for_env(*env_idx);
                                self.set_property(&global_this, &name, value)?;
                                return Ok(());
                            }
                            if has_binding {
                                if global_var_binding {
                                    let global_this = self.realm_global_for_env(*env_idx);
                                    self.set_property(&global_this, &name, value.clone())?;
                                    let data_value = match &global_this {
                                        Value::Object(idx) => self.heap.with_obj(idx.0, |obj| {
                                            obj.props()
                                                .lock()
                                                .get(&crate::value::PropertyKey::from(
                                                    name.as_str(),
                                                ))
                                                .filter(|descriptor| !descriptor.is_accessor)
                                                .map(|descriptor| descriptor.value.clone())
                                        }),
                                        _ => None,
                                    };
                                    if let Some(data_value) = data_value {
                                        let _ = crate::environment::set_checked_exact(
                                            &self.heap, *env_idx, &name, data_value,
                                        );
                                    }
                                }
                                return Ok(());
                            }
                            if has_with {
                                if let Some(with_obj) = with_obj_val {
                                    if self.with_object_has_binding(&with_obj, &name)? {
                                        if !self.has_property(&with_obj, &name)? {
                                            if r.strict {
                                                return Err(Error::reference(format!(
                                                    "{} is not defined",
                                                    name
                                                )));
                                            }
                                            if let Value::Object(idx) = &with_obj {
                                                let pkey =
                                                    crate::value::PropertyKey::from(name.as_str());
                                                let desc = crate::value::PropertyDescriptor {
                                                    value,
                                                    writable: true,
                                                    enumerable: true,
                                                    configurable: true,
                                                    get: None,
                                                    set: None,
                                                    is_accessor: false,
                                                };
                                                self.heap.with_obj(idx.0, |o| {
                                                    o.props().lock().insert(pkey, desc);
                                                });
                                                self.ic_invalidate(idx.0, &name);
                                            }
                                            return Ok(());
                                        }
                                        self.set_property(&with_obj, &name, value)?;
                                        return Ok(());
                                    }
                                    // Property not found on this with-object
                                    // (may have been deleted). For non-strict,
                                    // recreate it here; for strict, throw.
                                    if r.strict {
                                        return Err(Error::reference(format!(
                                            "{} is not defined",
                                            name
                                        )));
                                    }
                                    if let Value::Object(idx) = &with_obj {
                                        let pkey = crate::value::PropertyKey::from(name.as_str());
                                        let desc = crate::value::PropertyDescriptor {
                                            value,
                                            writable: true,
                                            enumerable: true,
                                            configurable: true,
                                            get: None,
                                            set: None,
                                            is_accessor: false,
                                        };
                                        self.heap.with_obj(idx.0, |o| {
                                            o.props().lock().insert(pkey, desc);
                                        });
                                        self.ic_invalidate(idx.0, &name);
                                    }
                                    return Ok(());
                                }
                            }
                            cur_env = parent;
                        }
                        // Not found in env chain: check global, or declare.
                        let global_this = self.realm_global_for_env(*env_idx);
                        let has_global = self.has_property(&global_this, &name)?;
                        if has_global {
                            self.set_property(&global_this, &name, value)?;
                            return Ok(());
                        }
                        if r.strict {
                            return Err(Error::reference(format!("{} is not defined", name)));
                        }
                        // Non-strict: create a configurable property on the
                        // global object (spec implicit global).
                        let global_this = self.realm_global_for_env(*env_idx);
                        self.set_property(&global_this, &name, value)?;
                    }
                    crate::value::ReferenceBase::Value(base) => {
                        if let crate::value::ReferencedName::Private(name) = &r.name {
                            return self.set_private_value(base, name, value);
                        }
                        if base.is_nullish() {
                            return Err(Error::type_err(
                                "Cannot set property of primitive".to_string(),
                            ));
                        }
                        let name = match &r.name {
                            crate::value::ReferencedName::Property(name) => name.clone(),
                            crate::value::ReferencedName::UncoercedProperty(name) => {
                                let pin_count = self.pin_many(&[v.clone(), value.clone()]);
                                let name_result = self.coerce_property_key_record(name);
                                self.unpin_many(pin_count);
                                name_result?
                            }
                            crate::value::ReferencedName::Private(_) => unreachable!(),
                        };
                        if let Some(receiver) = &r.this_value {
                            let base_obj = if matches!(base.as_ref(), Value::Object(_)) {
                                *base.clone()
                            } else {
                                self.to_object(base)?
                            };
                            let success = self.try_set_property_key_with_receiver(
                                &base_obj, &name, value, receiver,
                            )?;
                            if let (true, Value::Object(idx), crate::value::PropertyKey::Str(key)) =
                                (success, receiver.as_ref(), &name)
                            {
                                self.ic_invalidate(idx.0, key);
                            }
                            if !success && r.strict {
                                return Err(Error::type_err("Cannot assign to super property"));
                            }
                            return Ok(());
                        }
                        let success = if matches!(base.as_ref(), Value::Object(_)) {
                            self.try_set_property_key_with_receiver(base, &name, value, base)?
                        } else {
                            let boxed = self.to_object(base)?;
                            self.try_set_property_key_with_receiver(&boxed, &name, value, base)?
                        };
                        if success {
                            if let (Value::Object(idx), crate::value::PropertyKey::Str(s)) =
                                (base.as_ref(), &name)
                            {
                                let is_global_this = self.heap.with_obj(idx.0, |o| {
                                    matches!(
                                        o,
                                        HeapObj::Object(od)
                                            if od.class_name.as_deref() == Some("global")
                                    )
                                });
                                self.mirror_global_property_to_binding(
                                    *idx,
                                    s,
                                    true,
                                    is_global_this,
                                );
                            }
                        }
                        if !success && r.strict {
                            match &name {
                                crate::value::PropertyKey::Str(s) => {
                                    return Err(Error::type_err(format!(
                                        "Cannot assign to read only property '{}' of object",
                                        s
                                    )));
                                }
                                crate::value::PropertyKey::Symbol(_) => {
                                    return Err(Error::type_err(
                                        "Cannot assign to read only Symbol property",
                                    ));
                                }
                            }
                        }
                    }
                    crate::value::ReferenceBase::ObjectEnvironment(base) => match &r.name {
                        crate::value::ReferencedName::Property(crate::value::PropertyKey::Str(
                            s,
                        )) => {
                            if !self.has_property(base, s)? {
                                if r.strict {
                                    return Err(Error::reference(format!("{} is not defined", s)));
                                }
                            }
                            self.set_object_environment_property(base, s, value)?
                        }
                        crate::value::ReferencedName::Property(
                            crate::value::PropertyKey::Symbol(id),
                        ) => self.set_property_key(base, &Value::Symbol(*id), value)?,
                        crate::value::ReferencedName::UncoercedProperty(_)
                        | crate::value::ReferencedName::Private(_) => {
                            return Err(Error::internal(
                                "invalid reference name for an object environment base",
                            ));
                        }
                    },
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Build a frozen tagged-template object and its frozen `raw` array per
    /// GetTemplateObject. Both objects are ordinary objects with
    /// Array.prototype and class_name "Array" so `Array.isArray` recognizes
    /// them. The template object is cached by (chunk ptr, ip) so each source
    /// site returns the same instance.
    pub(crate) fn make_template_object(
        &mut self,
        quasi_ids: &[usize],
        raw_ids: &[usize],
    ) -> error::Result<Value> {
        let frame = self.current_frame()?;
        let raw_strings: Vec<Value> = raw_ids
            .iter()
            .map(|i| {
                frame
                    .chunk
                    .constants
                    .get(*i)
                    .cloned()
                    .unwrap_or(Value::Undefined)
            })
            .collect();
        let cooked_strings: Vec<Value> = quasi_ids
            .iter()
            .map(|i| {
                frame
                    .chunk
                    .constants
                    .get(*i)
                    .cloned()
                    .unwrap_or(Value::Undefined)
            })
            .collect();

        let raw_obj = self.new_frozen_arraylike(raw_strings)?;
        let mut tmpl = crate::value::ObjectData {
            props: parking_lot::Mutex::new(indexmap::IndexMap::new()),
            proto: parking_lot::Mutex::new(Some(self.array_proto.clone())),
            extensible: std::sync::atomic::AtomicBool::new(false),
            class_name: Some(std::sync::Arc::from("Array")),
            private_fields: parking_lot::Mutex::new(std::collections::HashMap::new()),
            primitive: parking_lot::Mutex::new(None),
        };
        let len = cooked_strings.len();
        for (i, v) in cooked_strings.into_iter().enumerate() {
            let mut desc = crate::value::PropertyDescriptor::data(v);
            desc.writable = false;
            desc.configurable = false;
            // enumerable = true (default)
            tmpl.props.lock().insert(
                crate::value::PropertyKey::from(i.to_string().as_str()),
                desc,
            );
        }
        let mut len_desc = crate::value::PropertyDescriptor::data(Value::Number(len as f64));
        len_desc.writable = false;
        len_desc.enumerable = false;
        len_desc.configurable = false;
        tmpl.props
            .lock()
            .insert(crate::value::PropertyKey::from("length"), len_desc);
        let mut raw_desc = crate::value::PropertyDescriptor::data(raw_obj);
        raw_desc.writable = false;
        raw_desc.enumerable = false;
        raw_desc.configurable = false;
        tmpl.props
            .lock()
            .insert(crate::value::PropertyKey::from("raw"), raw_desc);
        let idx = self.heap.allocate(crate::value::HeapObj::Object(tmpl))?;
        Ok(Value::Object(GcIdx(idx)))
    }

    fn new_frozen_arraylike(&mut self, items: Vec<Value>) -> error::Result<Value> {
        let mut obj = crate::value::ObjectData {
            props: parking_lot::Mutex::new(indexmap::IndexMap::new()),
            proto: parking_lot::Mutex::new(Some(self.array_proto.clone())),
            extensible: std::sync::atomic::AtomicBool::new(false),
            class_name: Some(std::sync::Arc::from("Array")),
            private_fields: parking_lot::Mutex::new(std::collections::HashMap::new()),
            primitive: parking_lot::Mutex::new(None),
        };
        for (i, v) in items.into_iter().enumerate() {
            let mut desc = crate::value::PropertyDescriptor::data(v);
            desc.writable = false;
            desc.configurable = false;
            obj.props.lock().insert(
                crate::value::PropertyKey::from(i.to_string().as_str()),
                desc,
            );
        }
        let len = obj.props.lock().len();
        let mut len_desc = crate::value::PropertyDescriptor::data(Value::Number(len as f64));
        len_desc.writable = false;
        len_desc.enumerable = false;
        len_desc.configurable = false;
        obj.props
            .lock()
            .insert(crate::value::PropertyKey::from("length"), len_desc);
        let idx = self.heap.allocate(crate::value::HeapObj::Object(obj))?;
        Ok(Value::Object(GcIdx(idx)))
    }
}
