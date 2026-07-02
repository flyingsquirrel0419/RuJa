//! Stack-based bytecode VM.

mod async_runtime;
mod conversions;
pub(crate) mod ops;
mod property;

pub(crate) use conversions::{to_int32, to_uint32};

use crate::bytecode::{Chunk, Op};
use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{GcIdx, HeapObj, PromiseStatus, Value};
use indexmap::IndexMap;
use num_traits::{Signed, Zero};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

pub type NativeFn = fn(&mut Vm, &[Value], Option<Value>) -> error::Result<Value>;

#[allow(dead_code)]
pub struct Vm {
    pub(crate) heap: Heap,
    pub(crate) global: GcIdx,
    pub(crate) global_this: Value,
    /// `new.target` to set on the next pushed frame (used by `construct`).
    pub(crate) pending_new_target: Option<Value>,
    pub(crate) stack: Vec<Value>,
    pub(crate) frames: Vec<CallFrame>,
    pub(crate) object_proto: Value,
    pub(crate) array_proto: Value,
    pub(crate) function_proto: Value,
    pub(crate) string_proto: Value,
    pub(crate) number_proto: Value,
    pub(crate) bigint_proto: Value,
    pub(crate) boolean_proto: Value,
    pub(crate) error_proto: Value,
    pub(crate) symbol_proto: Value,
    pub(crate) promise_proto: Value,
    pub(crate) iterator_proto: Value,
    pub(crate) generator_proto: Value,
    pub(crate) map_proto: Value,
    pub(crate) set_proto: Value,
    pub(crate) date_proto: Value,
    pub(crate) microtask_queue: std::collections::VecDeque<Microtask>,
    /// Monomorphic inline cache: (heap_idx, property_name) -> cached Value.
    /// Caches the result of the last GetProp on that object for that key.
    pub(crate) ic: std::collections::HashMap<(usize, String), Value>,
    /// Temporary GC roots pinned across operations that hold heap values in
    /// Rust locals (e.g. a Promise handler while `call_function` runs, which
    /// may itself trigger a GC). Push indices on entry, pop on exit.
    pub(crate) gc_pins: Vec<usize>,
    /// Collected yield values while running a generator function body (eager,
    /// legacy fallback path). Lazy generators use per-frame gen-state instead.
    pub(crate) current_yields: Vec<Value>,
    pub(crate) next_symbol_id: u32,
    pub(crate) well_known_symbols: WellKnownSymbols,
    pub(crate) global_names: HashMap<Arc<str>, usize>,
    pub(crate) global_constants: Vec<Value>,
    pub(crate) functions: Vec<Arc<crate::function::FunctionDef>>,
    /// Optional execution fuel: when set, each dispatched opcode decrements
    /// this; reaching zero throws a "fuel exhausted" RangeError. `None` means
    /// unbounded (the default). Embedders call `set_fuel` to bound untrusted
    /// code. Coarse and non-preemptive: a single native call (e.g. a long
    /// regex) is not subdivided.
    pub(crate) fuel: Option<i64>,
    /// Maximum number of live heap objects. `0` means unlimited.
    pub(crate) max_heap_objects: usize,
}

pub struct WellKnownSymbols {
    pub iterator: u32,
    pub to_primitive: u32,
    pub has_instance: u32,
    pub to_string_tag: u32,
    pub async_iterator: u32,
}

pub struct CallFrame {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub env: GcIdx,
    pub catch_stack: Vec<(usize, u32)>,
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
    pub gen_resume_value: Mutex<Value>,
    /// `this` binding to use for the next `Call` when the callee was resolved
    /// through a `with`-statement object environment record. Per ES spec,
    /// `with(o){ foo() }` binds `this` to `o` inside `foo` when `foo` is found
    /// as a property of `o`. Cleared after each `Call`.
    pub pending_with_this: Mutex<Option<Value>>,
    /// When set, the generator was resumed via `throw(e)`: the next dispatch
    /// in this frame throws `e` at the suspended `yield` point instead of
    /// pushing a resume value. Consumed on first use.
    pub force_throw: Mutex<Option<Value>>,
    /// Pending completion to re-raise after a `finally` block runs.
    /// Tag: 0 normal, 1 return, 2 break, 3 continue, 4 throw.
    pub finally_completion_tag: AtomicU8,
    pub finally_completion_val: Mutex<Value>,
    /// Stack of finally-target-ips for nested active `try/finally`. A
    /// non-local transfer (return/break/continue/throw) that hits an active
    /// finally diverts to the finally target after recording its completion.
    pub finally_stack: Vec<(usize, u32)>,
}

impl CallFrame {
    fn new(chunk: Arc<Chunk>, ip: usize, locals: Vec<Value>, env: GcIdx, this_val: Value) -> Self {
        CallFrame {
            chunk,
            ip,
            locals,
            env,
            new_target: Value::Undefined,
            catch_stack: Vec::new(),
            guard_seq: AtomicU32::new(0),
            this_val,
            gen_mode: AtomicBool::new(false),
            gen_yield: Mutex::new(None),
            gen_suspended: AtomicBool::new(false),
            gen_resume_value: Mutex::new(Value::Undefined),
            pending_with_this: Mutex::new(None),
            force_throw: Mutex::new(None),
            finally_completion_tag: AtomicU8::new(0),
            finally_completion_val: Mutex::new(Value::Undefined),
            finally_stack: Vec::new(),
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
}

/// Outcome of executing a single bytecode instruction.
#[allow(dead_code)]
enum Flow {
    /// Keep dispatching the next instruction.
    Continue,
    /// A Halt/Return ended execution with a value.
    Value(Value),
}

pub enum Microtask {
    Then {
        promise: GcIdx,
        on_fulfilled: Value,
        on_rejected: Value,
        derived: Option<GcIdx>,
    },
    Resolve {
        promise: GcIdx,
        value: Value,
    },
    Reject {
        promise: GcIdx,
        reason: Value,
    },
}

impl Default for Vm {
    fn default() -> Self {
        Self::new().expect("failed to initialize VM")
    }
}

impl Vm {
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
            stack: Vec::new(),
            frames: Vec::new(),
            object_proto: Value::Undefined,
            array_proto: Value::Undefined,
            function_proto: Value::Undefined,
            string_proto: Value::Undefined,
            number_proto: Value::Undefined,
            bigint_proto: Value::Undefined,
            boolean_proto: Value::Undefined,
            error_proto: Value::Undefined,
            symbol_proto: Value::Undefined,
            promise_proto: Value::Undefined,
            iterator_proto: Value::Undefined,
            generator_proto: Value::Undefined,
            map_proto: Value::Undefined,
            set_proto: Value::Undefined,
            date_proto: Value::Undefined,
            microtask_queue: std::collections::VecDeque::new(),
            ic: std::collections::HashMap::new(),
            gc_pins: Vec::new(),
            current_yields: Vec::new(),
            next_symbol_id: 1,
            well_known_symbols: WellKnownSymbols {
                iterator: 1,
                to_primitive: 2,
                has_instance: 3,
                to_string_tag: 4,
                async_iterator: 5,
            },
            global_names: HashMap::new(),
            global_constants: Vec::new(),
            functions: Vec::new(),
            fuel: None,
            max_heap_objects: 0,
        };
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
        let program = crate::parser::Parser::parse(src)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let _base = self.functions.len();
        self.functions.extend(funcs);
        // In sloppy (non-strict) script mode, top-level `this` is the global
        // object. Bind it on the global environment so `LoadEnv("this")` finds it.
        if !program.is_strict {
            crate::environment::declare(
                &self.heap,
                self.global,
                "this",
                self.global_this.clone(),
                crate::value::BindingKind::Const,
            );
        }
        let result = self.execute_chunk(chunk, self.global, Value::Undefined);
        // Drain microtasks (Promise callbacks) after the synchronous run.
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        // Collect at a safe point: all frames are settled and no Rust local
        // holds a heap value across this boundary. (Per-instruction GC was
        // unsafe because call_function/run_then hold handler values in Rust
        // locals that collect_roots could not see.)
        if self.heap.live_count() > 0 {
            let roots = self.collect_roots();
            self.heap.maybe_collect(&roots);
        }
        result
    }

    fn execute_chunk(&mut self, chunk: Chunk, env: GcIdx, this_val: Value) -> error::Result<Value> {
        let chunk = Arc::new(chunk);
        self.frames.push(CallFrame::new(
            chunk.clone(),
            0,
            vec![Value::Undefined; 256],
            env,
            this_val,
        ));
        self.interpret()
    }

    /// Like execute_chunk but guarantees the pushed frame is popped on return,
    /// so eval (which reuses the VM afterwards) leaves the caller's frame stack
    /// intact. Used by eval paths only.
    fn execute_chunk_scoped(
        &mut self,
        chunk: Chunk,
        env: GcIdx,
        this_val: Value,
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
            vec![Value::Undefined; 256],
            env,
            this_val,
        ));
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

    /// Evaluate a source string as an *indirect* eval: parse and compile it,
    /// then run it in the global scope (var/function declarations leak to
    /// global). Non-string inputs are returned as-is.
    pub fn eval_indirect(&mut self, src: &str) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        self.functions.extend(funcs);
        let result = self.execute_chunk_scoped(chunk, self.global, Value::Undefined);
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    /// Evaluate a source string as a *direct* eval: run it in a child of the
    /// caller's current environment, so it can read/assign the caller's
    /// variables. `var`/function declarations leak to the caller's function
    /// scope root (sloppy mode). `this_val` is the caller's `this`.
    pub fn eval_direct(
        &mut self,
        src: &str,
        caller_env: GcIdx,
        this_val: Value,
        caller_strict: bool,
    ) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        self.functions.extend(funcs);
        // Per spec, direct eval runs in a dedicated lexical environment whose
        // parent is the caller's environment. `let`/`const`/`class` declared in
        // eval stay local to that environment (they do NOT leak to the caller),
        // while `var` and function declarations leak to the caller's function
        // scope — UNLESS the eval code is strict, in which case nothing leaks
        // (the eval has its own scope and all bindings stay local). Pre-declare
        // the var/function names in the caller env (sloppy only) so the eval
        // body's `DeclareVar` writes land in the right place; then run the eval
        // body in the child environment.
        let is_strict = caller_strict || program.is_strict;
        let var_names = if is_strict {
            Vec::new()
        } else {
            crate::compiler::Compiler::collect_var_names(&program.body)
        };
        if !is_strict {
            for name in &var_names {
                crate::environment::declare_var(&self.heap, caller_env, name, Value::Undefined);
            }
        }
        let eval_env = crate::environment::new_env(&self.heap, Some(caller_env), true)?;
        let result = self.execute_chunk_scoped(chunk, eval_env, this_val);
        // After running, copy the var/function bindings that the eval body
        // established back into the caller's environment (they leak per spec).
        // `let`/`const`/`class` stay in eval_env and are discarded with it.
        // Strict eval does not leak anything.
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
            if crate::environment::has_lexical_binding(&self.heap, caller_env, &name) {
                continue;
            }
            crate::environment::declare(
                &self.heap,
                caller_env,
                &name,
                value,
                crate::value::BindingKind::Var,
            );
        }
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    /// Execute a compiled function's chunk in a new frame.
    fn execute_chunk_func(
        &mut self,
        fdef: Arc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
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
        self.frames
            .push(CallFrame::new(fdef.chunk.clone(), 0, locals, env, this_val));
        // Apply `new.target` if this call was a Construct.
        if let Some(nt) = self.pending_new_target.take() {
            if let Some(frame) = self.frames.last_mut() {
                frame.new_target = nt;
            }
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
            let roots = self.collect_roots();
            self.heap.maybe_collect(&roots);
        }
        result
    }

    /// Resume (or start) a lazy generator, running until the next `yield` or
    /// until the body completes. Returns `(value, done)` where `value` is the
    /// yielded value (or the return value when done) and `done` indicates
    /// whether the generator has finished.
    pub fn resume_generator(
        &mut self,
        g_idx: GcIdx,
        kind: ResumeKind,
    ) -> error::Result<(Value, bool)> {
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
            started,
            done,
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
                    g.started.load(Ordering::Relaxed),
                    g.done.load(Ordering::Relaxed),
                )
            } else {
                panic!("resume_generator on non-lazy-generator");
            }
        });

        if done {
            return Ok((Value::Undefined, true));
        }

        // `return(v)` on a suspended generator forces completion: the value is
        // the generator's return value and the generator is marked done.
        // Per spec, an unstarted generator's return() also just completes.
        if let ResumeKind::Return(v) = &kind {
            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.done.store(true, Ordering::Relaxed);
                    g.started.store(true, Ordering::Relaxed);
                }
            });
            return Ok((v.clone(), true));
        }

        let resume_val = match &kind {
            ResumeKind::Next(v) => v.clone(),
            ResumeKind::Throw(e) => e.clone(),
            ResumeKind::Return(_) => Value::Undefined, // handled above
        };

        // On the first resume, initialize the locals table with the arguments.
        if !started {
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
        } else if let ResumeKind::Throw(_e) = &kind {
            // `throw(e)`: do NOT push a resume value; instead, set a flag so
            // the next dispatch in this frame throws `e` at the yield point.
            // (The force_throw is stashed on the frame after it is pushed
            // below; we remember it in a local for now.)
        } else {
            // Resuming after a `yield`: the value sent via `next(v)` becomes the
            // result of the suspended `yield` expression.
            stack.push(resume_val.clone());
        }

        // Push the generator's frame.
        self.frames.push(CallFrame::new(
            fdef.chunk.clone(),
            ip,
            locals,
            env,
            this_val.clone(),
        ));
        // Restore the saved catch_stack onto the new frame.
        self.current_frame_mut()?.catch_stack = catch_stack;
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
            *frame.gen_yield.lock() = None;
            // `throw(e)`: arrange for the next dispatch to raise `e`.
            if let ResumeKind::Throw(e) = &kind {
                *frame.force_throw.lock() = Some(e.clone());
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
                    g.started.store(true, Ordering::Relaxed);
                }
            });

            Ok((yielded, false))
        } else {
            // Completed: the body returned or ran off the end. `result` holds
            // the return value; mark the generator done.
            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.done.store(true, Ordering::Relaxed);
                    g.started.store(true, Ordering::Relaxed);
                }
            });
            let ret = result.unwrap_or(Value::Undefined);
            Ok((ret, true))
        }
    }

    fn interpret(&mut self) -> error::Result<Value> {
        self.interpret_catch(None)
    }

    fn interpret_to_depth(&mut self, target_depth: usize) -> error::Result<Value> {
        self.interpret_catch(Some(target_depth))
    }

    /// Build a catchable `Error` object for a native (non-thrown) error, so
    /// `try/catch` receives a real object with `message` and `name`.
    fn make_error_value(&mut self, e: &Error) -> error::Result<Value> {
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
        // Look up the constructor (e.g. TypeError) and its prototype.
        let proto = match crate::environment::get(&self.heap, self.global, ctor_name) {
            Some(Value::Object(ci)) => self.heap.with_obj(ci.0, |o| {
                o.props()
                    .lock()
                    .get(&crate::value::PropertyKey::from("prototype"))
                    .map(|d| d.value.clone())
            }),
            _ => None,
        }
        .or_else(|| crate::environment::get(&self.heap, self.global, "Error"))
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
            class_name: Some(Arc::from(ctor_name)),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(obj)?)))
    }

    /// Run the dispatch loop, routing runtime errors to an active try/catch
    /// handler when one is present on the current frame's catch stack. A JS
    /// `throw` already routes through `Op::Throw`; this wrapper additionally
    /// converts errors raised by builtins/operators (TypeError, ReferenceError,
    /// ...) into catchable exceptions so that `try { f() } catch(e)` works for
    /// native errors too.
    fn interpret_catch(&mut self, return_depth: Option<usize>) -> error::Result<Value> {
        loop {
            match self.interpret_inner(return_depth) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    // If a catch handler is active, convert the error to a thrown
                    // value and resume at the handler.
                    let handler = self
                        .frames
                        .last()
                        .and_then(|f| f.catch_stack.last().map(|(ip, _)| *ip));
                    match handler {
                        _ if !e.catchable() => return Err(e),
                        Some(handler) => {
                            let thrown = match e.thrown_value.clone() {
                                Some(v) => v,
                                None => {
                                    // Synthesize an Error object for native errors.
                                    self.make_error_value(&e)?
                                }
                            };
                            // Pop the handler so we don't loop, push the thrown value
                            // for the catch binding, and jump to the handler ip.
                            self.current_frame_mut()?.catch_stack.pop();
                            self.stack.push(thrown);
                            self.current_frame_mut()?.ip = handler;
                            continue;
                        }
                        None => return Err(e),
                    }
                }
            }
        }
    }

    fn interpret_inner(&mut self, return_depth: Option<usize>) -> error::Result<Value> {
        match self.interpret_inner_raw(return_depth) {
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
    Native(NativeFn),
    Interpreted {
        func: std::sync::Arc<crate::function::FunctionDef>,
        closure: GcIdx,
        is_arrow: bool,
        is_async: bool,
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
}
