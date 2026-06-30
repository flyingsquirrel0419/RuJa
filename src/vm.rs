//! Stack-based bytecode VM.

use crate::bytecode::{Chunk, Op};
use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{GcIdx, HeapObj, PromiseStatus, Value};
use indexmap::IndexMap;
use num_traits::{Signed, Zero};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

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
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        let heap = Heap::new();
        let global = env::new_env(&heap, None, true);
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
        };
        crate::builtins::setup_full(&mut vm);
        vm
    }

    /// Set an execution-fuel budget. While set, each dispatched opcode
    /// decrements the budget; reaching zero throws a `RangeError("fuel
    /// exhausted")`. Pass `None` to disable the limit (the default). The
    /// budget persists across `run` calls, so an embedder can refill it
    /// between ticks. Coarse and cooperative, not preemption.
    pub fn set_fuel(&mut self, fuel: Option<i64>) {
        self.fuel = fuel;
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
        self.frames.push(CallFrame::new(
            chunk.clone(),
            0,
            vec![Value::Undefined; 256],
            env,
            this_val,
        ));
        let depth_before = self.frames.len();
        let result = self.interpret();
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
        let eval_env = crate::environment::new_env(&self.heap, Some(caller_env), true);
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
                    .unwrap()
                    .iter()
                    .filter(|(name, _)| var_names.contains(&**name))
                    .map(|(name, b)| (name.clone(), b.value.lock().unwrap().clone()))
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
                    *g.env.lock().unwrap(),
                    g.this_val.lock().unwrap().clone(),
                    g.args.lock().unwrap().clone(),
                    g.ip.load(Ordering::Relaxed),
                    g.locals.lock().unwrap().clone(),
                    g.stack.lock().unwrap().clone(),
                    g.catch_stack.lock().unwrap().clone(),
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
        self.frames.last_mut().unwrap().catch_stack = catch_stack;
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
            *frame.gen_resume_value.lock().unwrap() = resume_val.clone();
            frame.gen_mode.store(true, Ordering::Relaxed);
            frame.gen_suspended.store(false, Ordering::Relaxed);
            *frame.gen_yield.lock().unwrap() = None;
            // `throw(e)`: arrange for the next dispatch to raise `e`.
            if let ResumeKind::Throw(e) = &kind {
                *frame.force_throw.lock().unwrap() = Some(e.clone());
            }
        }

        let result = self.interpret_to_depth(target_depth);

        // Clear the resume value on the frame so a subsequent resume (or a
        // GC pass between resumes) does not observe a stale value.
        if self.frames.len() > target_depth {
            *self.frames[target_depth].gen_resume_value.lock().unwrap() = Value::Undefined;
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
                .unwrap()
                .take()
                .unwrap_or(Value::Undefined);
            // Pop the generator frame and save its state for the next resume.
            let frame = self.frames.pop().expect("generator frame present");
            // The generator's leftover operands are its private stack.
            let saved_stack = gen_stack;

            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.ip.store(frame.ip, Ordering::Relaxed);
                    *g.env.lock().unwrap() = frame.env;
                    *g.locals.lock().unwrap() = frame.locals;
                    *g.stack.lock().unwrap() = saved_stack;
                    *g.catch_stack.lock().unwrap() = frame.catch_stack;
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
    fn make_error_value(&mut self, e: &Error) -> Value {
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
                    .unwrap()
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
        Value::Object(GcIdx(self.heap.allocate(obj)))
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
                        Some(handler) => {
                            let thrown = match e.thrown_value.clone() {
                                Some(v) => v,
                                None => {
                                    // Synthesize an Error object for native errors.
                                    self.make_error_value(&e)
                                }
                            };
                            // Pop the handler so we don't loop, push the thrown value
                            // for the catch binding, and jump to the handler ip.
                            self.frames.last_mut().unwrap().catch_stack.pop();
                            self.stack.push(thrown);
                            self.frames.last_mut().unwrap().ip = handler;
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

    fn interpret_inner_raw(&mut self, return_depth: Option<usize>) -> error::Result<Value> {
        loop {
            // Execution fuel: bound untrusted code. Checked before each
            // opcode so a tight loop cannot run forever. None = unbounded.
            if let Some(f) = self.fuel.as_mut() {
                if *f <= 0 {
                    return Err(Error::range("fuel exhausted".to_string()));
                }
                *f -= 1;
            }
            // Generator `throw(e)` resume: if the current frame has a pending
            // forced throw (set by resume_generator on a Throw resume), raise
            // it now at the suspended `yield` point. This lets the generator
            // body's own try/catch handle the injected exception.
            if let Some(exc) = self
                .frames
                .last()
                .and_then(|f| f.force_throw.lock().unwrap().take())
            {
                return Err(Error::thrown(exc, &self.heap));
            }
            let frame = self.frames.last().unwrap();
            let ip = frame.ip;
            if ip >= frame.chunk.code.len() {
                return Ok(Value::Undefined);
            }
            let op = frame.chunk.code[ip].clone();
            self.frames.last_mut().unwrap().ip += 1;
            match op {
                Op::Halt => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    return Ok(v);
                }
                Op::ToString => {
                    // Template-literal interpolation: ToPrimitive(string)
                    // then ToString.
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let prim = self.to_primitive_hint(&v, true)?;
                    let s = self.to_string(&prim)?;
                    self.stack.push(Value::String(s));
                }
                Op::Const(idx) => {
                    let v = {
                        let frame = self.frames.last().unwrap();
                        frame.chunk.constants[idx].clone()
                    };
                    self.stack.push(v);
                }
                Op::LoadGlobal => {
                    let name_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let name = match &name_val {
                        Value::String(s) => s.to_string(),
                        _ => self.to_string(&name_val)?.to_string(),
                    };
                    // search the current frame's env first, then global
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::get_checked(&self.heap, cur_env, &name) {
                        Ok(Some(v)) => self.stack.push(v),
                        Ok(None) => {
                            match crate::environment::get_checked(&self.heap, self.global, &name) {
                                Ok(Some(v)) => self.stack.push(v),
                                Ok(None) => {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )))
                                }
                                Err(true) => {
                                    return Err(Error::reference(format!(
                                        "Cannot access '{}' before initialization",
                                        name
                                    )))
                                }
                                Err(false) => {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )))
                                }
                            }
                        }
                        Err(true) => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )))
                        }
                        Err(false) => {
                            return Err(Error::reference(format!("{} is not defined", name)))
                        }
                    }
                }
                Op::StoreGlobal => {
                    let name_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let name = match &name_val {
                        Value::String(s) => s.to_string(),
                        _ => self.to_string(&name_val)?.to_string(),
                    };
                    // try to set in current scope chain first, else declare in global
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::set_checked(&self.heap, cur_env, &name, value.clone())
                    {
                        crate::environment::SetOutcome::Set => {}
                        crate::environment::SetOutcome::Const => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::Tdz => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::NotFound => {
                            crate::environment::declare(
                                &self.heap,
                                self.global,
                                &name,
                                value,
                                crate::value::BindingKind::Var,
                            );
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::DeclareEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare(
                        &self.heap,
                        cur_env,
                        &name,
                        value,
                        crate::value::BindingKind::Let,
                    );
                }
                Op::DeclareVar(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare_var(&self.heap, cur_env, &name, value);
                }
                Op::DeclareLet(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare(
                        &self.heap,
                        cur_env,
                        &name,
                        value,
                        crate::value::BindingKind::Let,
                    );
                }
                Op::DeclareConst(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare(
                        &self.heap,
                        cur_env,
                        &name,
                        value,
                        crate::value::BindingKind::Const,
                    );
                }
                Op::DeclareEnvConst(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare_typed(
                        &self.heap,
                        cur_env,
                        &name,
                        value,
                        crate::value::BindingKind::Const,
                    );
                }
                Op::DeclareLetUninit(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare_uninit(
                        &self.heap,
                        cur_env,
                        &name,
                        crate::value::BindingKind::Let,
                    );
                }
                Op::DeclareConstUninit(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare_uninit(
                        &self.heap,
                        cur_env,
                        &name,
                        crate::value::BindingKind::Const,
                    );
                }
                Op::InitEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    if !crate::environment::initialize_local(
                        &self.heap,
                        cur_env,
                        &name,
                        value.clone(),
                    ) {
                        crate::environment::declare_typed(
                            &self.heap,
                            cur_env,
                            &name,
                            value,
                            crate::value::BindingKind::Let,
                        );
                    }
                }
                Op::InitEnvConst(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    if !crate::environment::initialize_local(
                        &self.heap,
                        cur_env,
                        &name,
                        value.clone(),
                    ) {
                        crate::environment::declare_typed(
                            &self.heap,
                            cur_env,
                            &name,
                            value,
                            crate::value::BindingKind::Const,
                        );
                    }
                }
                Op::InitLet(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    if !crate::environment::initialize_local(
                        &self.heap,
                        cur_env,
                        &name,
                        value.clone(),
                    ) {
                        crate::environment::declare_typed(
                            &self.heap,
                            cur_env,
                            &name,
                            value,
                            crate::value::BindingKind::Let,
                        );
                    }
                }
                Op::InitConst(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    if !crate::environment::initialize_local(
                        &self.heap,
                        cur_env,
                        &name,
                        value.clone(),
                    ) {
                        crate::environment::declare_typed(
                            &self.heap,
                            cur_env,
                            &name,
                            value,
                            crate::value::BindingKind::Const,
                        );
                    }
                }
                Op::LoadEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::get_checked(&self.heap, cur_env, &name) {
                        Ok(Some(v)) => self.stack.push(v),
                        Ok(None) => {
                            match crate::environment::get_checked(&self.heap, self.global, &name) {
                                Ok(Some(v)) => self.stack.push(v),
                                Ok(None) => {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )))
                                }
                                Err(true) => {
                                    return Err(Error::reference(format!(
                                        "Cannot access '{}' before initialization",
                                        name
                                    )))
                                }
                                Err(false) => {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )))
                                }
                            }
                        }
                        Err(true) => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )))
                        }
                        Err(false) => {
                            return Err(Error::reference(format!("{} is not defined", name)))
                        }
                    }
                }
                Op::StoreEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::set_checked(&self.heap, cur_env, &name, value.clone())
                    {
                        crate::environment::SetOutcome::Set => {}
                        crate::environment::SetOutcome::Const => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::Tdz => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::NotFound => {
                            // `with`-statement: assign to the closest object env
                            // record that has the property, else declare as var.
                            let with_objs = crate::environment::with_objects(&self.heap, cur_env);
                            let mut set_on_with = false;
                            for obj in &with_objs {
                                if self.has_property(obj, &name)? {
                                    self.set_property(obj, &name, value.clone())?;
                                    set_on_with = true;
                                    break;
                                }
                            }
                            if !set_on_with {
                                crate::environment::declare(
                                    &self.heap,
                                    cur_env,
                                    &name,
                                    value,
                                    crate::value::BindingKind::Var,
                                );
                            }
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::LoadEnvName(name_idx) => {
                    // Reset any stale `with`-this from a previous name load that
                    // was not immediately followed by a `Call`. Only a name found
                    // on a `with` object *and* used as a call callee should rebind
                    // `this`; clearing here prevents leftover values from leaking
                    // into a later, unrelated call.
                    if let Some(f) = self.frames.last() {
                        *f.pending_with_this.lock().unwrap() = None;
                    }
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    // `with`-statement object environment records take precedence over
                    // the lexical scope chain (closest first), per spec.
                    let with_objs = crate::environment::with_objects(&self.heap, env);
                    let mut found_in_with: Option<(Value, Value)> = None;
                    for obj in &with_objs {
                        let v = self.get_property(obj, &name)?;
                        if !v.is_undefined() {
                            // Remember which `with` object supplied the value so
                            // that, if the callee is called as `foo()` (not
                            // `obj.foo()`), the next `Call` binds `this` to it.
                            found_in_with = Some((v, obj.clone()));
                            break;
                        }
                    }
                    if let Some((v, with_obj)) = found_in_with {
                        // Only function-valued lookups rebind `this`; a plain
                        // value read does not affect the next call. We defer the
                        // is-function check to `Call` by stashing the candidate
                        // `this` here unconditionally, and `Call` clears it on
                        // any use (function or not) so it never leaks past one
                        // opcode.
                        if matches!(v, Value::Object(_)) {
                            *self
                                .frames
                                .last_mut()
                                .unwrap()
                                .pending_with_this
                                .lock()
                                .unwrap() = Some(with_obj);
                        }
                        self.stack.push(v);
                    } else {
                        match crate::environment::get_checked(&self.heap, env, &name) {
                            Ok(Some(v)) => self.stack.push(v),
                            Err(true) => {
                                return Err(Error::reference(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )))
                            }
                            Ok(None) | Err(false) => {
                                match crate::environment::get_checked(
                                    &self.heap,
                                    self.global,
                                    &name,
                                ) {
                                    Ok(Some(v)) => self.stack.push(v),
                                    Ok(None) | Err(false) => {
                                        return Err(Error::reference(format!(
                                            "{} is not defined",
                                            name
                                        )))
                                    }
                                    Err(true) => {
                                        return Err(Error::reference(format!(
                                            "Cannot access '{}' before initialization",
                                            name
                                        )))
                                    }
                                }
                            }
                        }
                    }
                }
                Op::StoreEnvName(name_idx) => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::set_checked(&self.heap, env, &name, value.clone()) {
                        crate::environment::SetOutcome::Set => {}
                        crate::environment::SetOutcome::Const => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::Tdz => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::NotFound => {
                            // `with`-statement: assign to the closest object env
                            // record that has the property, else declare as var.
                            let with_objs = crate::environment::with_objects(&self.heap, env);
                            let mut set_on_with = false;
                            for obj in &with_objs {
                                let has = self.has_property(obj, &name)?;
                                if has {
                                    self.set_property(obj, &name, value.clone())?;
                                    set_on_with = true;
                                    break;
                                }
                            }
                            if !set_on_with {
                                crate::environment::declare(
                                    &self.heap,
                                    env,
                                    &name,
                                    value,
                                    crate::value::BindingKind::Var,
                                );
                            }
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::LoadLocal(idx) => {
                    let v = self.frames.last().unwrap().locals[idx].clone();
                    self.stack.push(v);
                }
                Op::StoreLocal(idx) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    self.frames.last_mut().unwrap().locals[idx] = v;
                }
                Op::Null => self.stack.push(Value::Null),
                Op::Undefined => self.stack.push(Value::Undefined),
                Op::True => self.stack.push(Value::Bool(true)),
                Op::False => self.stack.push(Value::Bool(false)),
                Op::Pop => {
                    self.stack.pop();
                }
                Op::PushScope => {
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let new_env = env::new_env(&self.heap, Some(cur_env), false);
                    self.frames.last_mut().unwrap().env = new_env;
                }
                Op::PopScope => {
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock().unwrap()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.frames.last_mut().unwrap().env = p;
                    }
                }
                Op::PushWithEnv => {
                    let object = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let new_env = env::new_with_env(&self.heap, cur_env, object);
                    self.frames.last_mut().unwrap().env = new_env;
                }
                Op::PopWithEnv => {
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock().unwrap()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.frames.last_mut().unwrap().env = p;
                    }
                }
                Op::CloneLetNames(idx) => {
                    // Per-iteration environment for `for (let ...)`: clone
                    // ONLY the loop's declared variables into a child env so
                    // each iteration's closures capture a distinct binding for
                    // the loop variable while sharing the rest of the scope.
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let names = self
                        .frames
                        .last()
                        .map(|f| f.chunk.let_names.get(idx).cloned().unwrap_or_default())
                        .unwrap_or_default();
                    let child = env::clone_loop_vars(&self.heap, cur_env, &names);
                    self.frames.last_mut().unwrap().env = child;
                }
                Op::RestoreParentEnv => {
                    // After the loop body (which ran in a CloneLetEnv child),
                    // restore the frame env to the child's parent (the loop
                    // scope env) so the update/cond/next iteration run in the
                    // original env and the chain does not grow per iteration.
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock().unwrap()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.frames.last_mut().unwrap().env = p;
                    }
                }
                Op::Dup => {
                    let v = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    self.stack.push(v);
                }
                Op::Swap => {
                    let len = self.stack.len();
                    if len >= 2 {
                        self.stack.swap(len - 1, len - 2);
                    }
                }
                Op::Rot3 => {
                    let len = self.stack.len();
                    if len >= 3 {
                        let c = self.stack.remove(len - 3);
                        self.stack.push(c);
                    }
                }
                Op::Add => self.bin_op(
                    |a, b| Value::Number(a + b),
                    |a, b| Value::String(Arc::from(format!("{}{}", a, b).as_str())),
                )?,
                Op::Sub => self.num_bin_bigint(|a, b| a - b, |x, y| x - y)?,
                Op::Mul => self.num_bin_bigint(|a, b| a * b, |x, y| x * y)?,
                Op::Div => self.num_bin_bigint(
                    |a, b| a / b,
                    |x, y| {
                        if y.is_zero() {
                            num_bigint::BigInt::from(0)
                        } else {
                            x / y
                        }
                    },
                )?,
                Op::Mod => self.num_bin_bigint(
                    |a, b| a % b,
                    |x, y| {
                        if y.is_zero() {
                            num_bigint::BigInt::from(0)
                        } else {
                            x % y
                        }
                    },
                )?,
                Op::Pow => self.num_bin_bigint(
                    |a, b| a.powf(b),
                    |x, y| {
                        if y.is_negative() {
                            num_bigint::BigInt::from(0)
                        } else {
                            // Use BigInt's own pow (exponent is a u64).
                            let exp = num_traits::ToPrimitive::to_u32(&y).unwrap_or(0);
                            x.pow(exp)
                        }
                    },
                )?,
                Op::Neg => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::BigInt(n) = v {
                        self.stack.push(Value::BigInt(-n));
                    } else {
                        let n = self.to_number(&v)?;
                        self.stack.push(Value::Number(-n));
                    }
                }
                Op::Not => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let b = v.is_truthy();
                    self.stack.push(Value::Bool(!b));
                }
                Op::BitNot => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let n = self.to_number(&v)? as i32;
                    self.stack.push(Value::Number(!n as f64));
                }
                Op::Eq => {
                    let (a, b) = self.pop2();
                    let r = self.loose_eq(&a, &b)?;
                    self.stack.push(Value::Bool(r));
                }
                Op::NotEq => {
                    let (a, b) = self.pop2();
                    let r = self.loose_eq(&a, &b)?;
                    self.stack.push(Value::Bool(!r));
                }
                Op::StrictEq => {
                    let (a, b) = self.pop2();
                    let r = self.strict_eq(&a, &b);
                    self.stack.push(Value::Bool(r));
                }
                Op::StrictNotEq => {
                    let (a, b) = self.pop2();
                    let r = self.strict_eq(&a, &b);
                    self.stack.push(Value::Bool(!r));
                }
                Op::Lt => self.compare(|a, b| a < b, |a: &str, b: &str| a < b)?,
                Op::Gt => self.compare(|a, b| a > b, |a: &str, b: &str| a > b)?,
                Op::Lte => self.compare(|a, b| a <= b, |a: &str, b: &str| a <= b)?,
                Op::Gte => self.compare(|a, b| a >= b, |a: &str, b: &str| a >= b)?,
                Op::In => {
                    // stack: [key, obj]; true if obj has the property (own or inherited).
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    let v = self.get_property(&obj, &key_str)?;
                    self.stack.push(Value::Bool(!v.is_undefined()));
                }
                Op::InstanceOf => {
                    // stack: [obj, ctor]; walk obj's proto chain for ctor.prototype.
                    let ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let ctor_proto = if let Value::Object(ci) = &ctor {
                        self.heap.with_obj(ci.0, |o| {
                            if let HeapObj::Function(f) = o {
                                f.prototype
                                    .lock()
                                    .unwrap()
                                    .clone()
                                    .unwrap_or(Value::Undefined)
                            } else {
                                Value::Undefined
                            }
                        })
                    } else {
                        Value::Undefined
                    };
                    let mut cur = obj;
                    let mut result = false;
                    while let Value::Object(oi) = &cur {
                        if Value::Object(*oi) == ctor_proto {
                            result = true;
                            break;
                        }
                        cur = self.heap.with_obj(oi.0, |o| {
                            o.proto()
                                .lock()
                                .unwrap()
                                .clone()
                                .unwrap_or(Value::Undefined)
                        });
                        if cur.is_undefined() {
                            break;
                        }
                    }
                    let _ = ctor;
                    self.stack.push(Value::Bool(result));
                }
                Op::BitAnd => self.int_bin(|a, b| a & b)?,
                Op::BitOr => self.int_bin(|a, b| a | b)?,
                Op::BitXor => self.int_bin(|a, b| a ^ b)?,
                Op::Shl => self.int_bin(|a, b| a << (b as u32 & 31))?,
                Op::Shr => self.int_bin(|a, b| a >> (b as u32 & 31))?,
                Op::Ushr => {
                    // Unsigned right shift: result is a uint32 promoted to Number,
                    // so -1 >>> 0 === 4294967295 (not -1).
                    let (a, b) = self.pop2();
                    let av = self.to_number(&a)? as i32 as u32;
                    let bv = self.to_number(&b)? as i32 as u32;
                    self.stack.push(Value::Number((av >> (bv & 31)) as f64));
                }
                Op::Jump(target) => {
                    self.frames.last_mut().unwrap().ip = target;
                }
                Op::JumpIfFalse(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if !v.is_truthy() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }
                Op::JumpIfTrue(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if v.is_truthy() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }
                Op::JumpIfNullish(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if v.is_nullish() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }
                Op::JumpIfNotNullish(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if !v.is_nullish() {
                        self.frames.last_mut().unwrap().ip = target;
                    }
                }
                Op::Return => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    // If a `finally` is active, suspend the return across it:
                    // record the completion (tag 1) and divert to the finally
                    // target, popping the finally entry so the finally body's
                    // own transfers aren't re-intercepted by this finally.
                    if let Some(frame) = self.frames.last_mut() {
                        if let Some(&(target, _)) = frame.finally_stack.last() {
                            frame.finally_completion_tag.store(1, Ordering::Relaxed);
                            *frame.finally_completion_val.lock().unwrap() = v;
                            frame.ip = target;
                            continue;
                        }
                    }
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(v);
                    }
                    if let Some(d) = return_depth {
                        if self.frames.len() <= d {
                            return Ok(v);
                        }
                    }
                    self.stack.push(v);
                }
                Op::ReturnUndefined => {
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(Value::Undefined);
                    }
                    if let Some(d) = return_depth {
                        if self.frames.len() <= d {
                            return Ok(Value::Undefined);
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::NewObject => {
                    let obj = HeapObj::Object(crate::value::ObjectData {
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(self.object_proto.clone())),
                        extensible: std::sync::atomic::AtomicBool::new(true),
                        class_name: None,
                        private_fields: Mutex::new(std::collections::HashMap::new()),
                        primitive: Mutex::new(None),
                    });
                    let idx = self.heap.allocate(obj);
                    self.stack.push(Value::Object(GcIdx(idx)));
                }
                Op::NewArray(count) => {
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    items.reverse();
                    let obj = HeapObj::Array(crate::value::ArrayData {
                        items: Mutex::new(items),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(self.array_proto.clone())),
                    });
                    let idx = self.heap.allocate(obj);
                    self.stack.push(Value::Object(GcIdx(idx)));
                }
                Op::ArrayPush => {
                    // stack: [array, value]; append value to the array's items.
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let arr = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                a.items.lock().unwrap().push(value.clone());
                            }
                        });
                    }
                    self.stack.push(arr);
                }
                Op::SpreadPush => {
                    // stack: [array, iterable]; spread iterable's values into the array.
                    let iterable = self.stack.pop().unwrap_or(Value::Undefined);
                    let arr = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(arr_idx) = &arr {
                        let it = self.make_iterator(&iterable)?;
                        // drain the iterator into the array
                        loop {
                            let (v, done) = self.iterator_next(&it)?;
                            if done {
                                break;
                            }
                            self.heap.with_obj(arr_idx.0, |o| {
                                if let HeapObj::Array(a) = o {
                                    a.items.lock().unwrap().push(v.clone());
                                }
                            });
                        }
                    }
                    self.stack.push(arr);
                }
                Op::ObjSpread => {
                    // stack: [dest, src]; copy src's enumerable own props into dest.
                    let src = self.stack.pop().unwrap_or(Value::Undefined);
                    let dest = self.stack.pop().unwrap_or(Value::Undefined);
                    if let (Value::Object(dest_idx), Value::Object(src_idx)) = (&dest, &src) {
                        let _ = dest_idx;
                        // Collect (key, value) pairs from src's own enumerable props.
                        let pairs: Vec<(Arc<str>, Value)> = self.heap.with_obj(src_idx.0, |o| {
                            let mut out = Vec::new();
                            if let HeapObj::Array(a) = o {
                                for (i, v) in a.items.lock().unwrap().iter().enumerate() {
                                    out.push((Arc::from(i.to_string().as_str()), v.clone()));
                                }
                            }
                            for (k, desc) in o.props().lock().unwrap().iter() {
                                if desc.enumerable {
                                    if let crate::value::PropertyKey::Str(s) = k {
                                        out.push((s.clone(), Value::Undefined));
                                    }
                                }
                            }
                            out
                        });
                        for (k, mut v) in pairs {
                            if v.is_undefined() {
                                v = self.get_property(&src, &k)?;
                            }
                            self.set_property(&dest, &k, v)?;
                        }
                    }
                    self.stack.push(dest);
                }
                Op::ObjRest(count) => {
                    // stack: [src, k1..kN]; new obj with src's own enum props except k1..kN
                    let mut excluded: Vec<Arc<str>> = Vec::with_capacity(count);
                    for _ in 0..count {
                        if let Some(Value::String(s)) = self.stack.pop() {
                            excluded.push(s);
                        }
                    }
                    let src = self.stack.pop().unwrap_or(Value::Undefined);
                    let new_obj = Value::Object(self.new_object());
                    if let (Value::Object(dest_idx), Value::Object(src_idx)) = (&new_obj, &src) {
                        let pairs: Vec<(Arc<str>, Value)> = self.heap.with_obj(src_idx.0, |o| {
                            let mut out = Vec::new();
                            for (k, desc) in o.props().lock().unwrap().iter() {
                                if desc.enumerable {
                                    if let crate::value::PropertyKey::Str(s) = k {
                                        out.push((s.clone(), Value::Undefined));
                                    }
                                }
                            }
                            out
                        });
                        for (k, mut v) in pairs {
                            if excluded.contains(&k) {
                                continue;
                            }
                            if v.is_undefined() {
                                v = self.get_property(&src, &k)?;
                            }
                            self.set_property(&new_obj, &k, v)?;
                        }
                        let _ = dest_idx;
                    }
                    self.stack.push(new_obj);
                }
                Op::DefineAccessor(kind) => {
                    // stack: [obj, key, fn]; define getter(0) or setter(1).
                    let func = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &obj {
                        let pkey = match &key_val {
                            Value::String(s) => crate::value::PropertyKey::Str(s.clone()),
                            Value::Number(n) => crate::value::PropertyKey::Str(Arc::from(
                                crate::value::num_to_string(*n).as_str(),
                            )),
                            Value::Symbol(s) => crate::value::PropertyKey::Symbol(*s),
                            _ => crate::value::PropertyKey::Str(Arc::from("undefined")),
                        };
                        self.heap.with_obj(idx.0, |o| {
                            let props = o.props();
                            let mut props = props.lock().unwrap();
                            let entry = props.entry(pkey).or_insert_with(|| {
                                crate::value::PropertyDescriptor {
                                    value: Value::Undefined,
                                    writable: false,
                                    enumerable: true,
                                    configurable: true,
                                    get: None,
                                    set: None,
                                    is_accessor: true,
                                }
                            });
                            entry.is_accessor = true;
                            entry.writable = false;
                            if kind == 0 {
                                entry.get = Some(func.clone());
                            } else {
                                entry.set = Some(func.clone());
                            }
                        });
                    }
                    self.stack.push(obj);
                }
                Op::NewTarget => {
                    let nt = self
                        .frames
                        .last()
                        .map(|f| f.new_target.clone())
                        .unwrap_or(Value::Undefined);
                    self.stack.push(nt);
                }
                Op::GetProp => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    let v = self.get_property(&obj, &key_str)?;
                    self.stack.push(v);
                }
                Op::GetElem => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let v = self.get_property_key(&obj, &key)?;
                    self.stack.push(v);
                }
                Op::SetProp => {
                    // stack (bottom->top): [obj, key, value]
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    self.set_property(&obj, &key_str, value.clone())?;
                    self.stack.push(value);
                }
                Op::SetElem => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_property_key(&obj, &key, value.clone())?;
                    self.stack.push(value);
                }
                Op::DeleteProp => {
                    // stack: [obj, key]; remove the own property, push boolean.
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let pkey = match &key {
                        Value::Symbol(id) => crate::value::PropertyKey::Symbol(*id),
                        _ => crate::value::PropertyKey::from(self.to_property_key(&key)?),
                    };
                    let result = if let Value::Object(idx) = &obj {
                        // Check configurability first: deleting a
                        // non-configurable own property must fail (`false`,
                        // or a TypeError in strict mode), not actually remove
                        // the property.
                        let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                            o.props()
                                .lock()
                                .unwrap()
                                .get(&pkey)
                                .map_or((false, true), |d| (true, d.configurable))
                        });
                        if exists && !configurable {
                            if self.current_strict() {
                                return Err(Error::type_err(
                                    "Cannot delete non-configurable property",
                                ));
                            }
                            Value::Bool(false)
                        } else if exists {
                            self.heap.with_obj(idx.0, |o| {
                                o.props().lock().unwrap().shift_remove(&pkey);
                            });
                            Value::Bool(true)
                        } else {
                            // Non-existent own property: delete returns true.
                            Value::Bool(true)
                        }
                    } else {
                        // Primitive receiver: delete is a no-op that returns true.
                        Value::Bool(true)
                    };
                    self.stack.push(result);
                }
                Op::SetProto => {
                    // stack (top->bottom): [proto, obj]; set obj's [[Prototype]] to proto.
                    let proto = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            *o.proto().lock().unwrap() = Some(proto);
                        });
                    }
                }
                Op::Throw => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    // If a finally guards this region, divert to it with a
                    // `throw` completion (tag 4) so the finally body runs before
                    // the exception propagates. Otherwise route to a catch
                    // handler, or propagate the throw out of the frame.
                    //
                    // Spec model: when both a catch and a finally are active,
                    // the catch handles the throw first; the finally runs only
                    // after the try/catch region as a whole completes. So divert
                    // to finally only when there is no catch handler on top of
                    // the finally guard (i.e. try/finally without catch, or a
                    // throw escaping from a catch body that a finally guards).
                    if let Some(frame) = self.frames.last_mut() {
                        // A throw must pass through any finally that is *more
                        // deeply nested* than the nearest catch. Compare the
                        // finally's entry ip against the catch handler ip: a
                        // finally pushed after (greater ip) its enclosing catch
                        // guard sits inside it, so the throw diverts there first.
                        // Divert to finally iff it was pushed after (deeper
                        // than) the nearest catch guard. Uses push sequence
                        // numbers so nesting order is tracked correctly even
                        // when finally/catch ips are interleaved.
                        let divert_to_finally =
                            match (frame.finally_stack.last(), frame.catch_stack.last()) {
                                (Some(&(_, _)), None) => true,
                                (Some(&(_, fseq)), Some(&(_, cseq))) => fseq > cseq,
                                _ => false,
                            };
                        if divert_to_finally {
                            let target = frame.finally_stack.last().unwrap().0;
                            frame.finally_completion_tag.store(4, Ordering::Relaxed);
                            *frame.finally_completion_val.lock().unwrap() = v;
                            frame.ip = target;
                            continue;
                        }
                        if let Some((handler, _)) = frame.catch_stack.pop() {
                            frame.ip = handler;
                            self.stack.push(v);
                            continue;
                        }
                    }
                    return Err(Error::thrown(v, &self.heap));
                }
                Op::PushTry(handler) => {
                    let f = self.frames.last_mut().unwrap();
                    let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                    f.guard_seq.store(seq, Ordering::Relaxed);
                    f.catch_stack.push((handler, seq));
                }
                Op::PopTry => {
                    let f = self.frames.last_mut().unwrap();
                    f.catch_stack.pop();
                }
                Op::PushFinally(target) => {
                    // Begin guarding try/catch with a finally: record the
                    // finally entry so non-local transfers divert to it.
                    let f = self.frames.last_mut().unwrap();
                    let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                    f.guard_seq.store(seq, Ordering::Relaxed);
                    f.finally_stack.push((target, seq));
                }
                Op::PopFinally => {
                    // The guarded region completed normally; drop the finally
                    // guard. A pending completion from inside the region was
                    // already popped when the transfer diverted to finally.
                    self.frames.last_mut().unwrap().finally_stack.pop();
                }
                Op::DivertBreak(finally_start) => {
                    let resume_ip = ip + 1;
                    let f = self.frames.last_mut().unwrap();
                    f.finally_completion_tag.store(2, Ordering::Relaxed);
                    *f.finally_completion_val.lock().unwrap() = Value::Number(resume_ip as f64);
                    f.ip = finally_start;
                    continue;
                }
                Op::DivertContinue(finally_start, cont) => {
                    // A `continue` inside an active try/finally: record the
                    // completion as a continue with the loop's continue target,
                    // and divert to the finally body.
                    let f = self.frames.last_mut().unwrap();
                    f.finally_completion_tag.store(3, Ordering::Relaxed);
                    *f.finally_completion_val.lock().unwrap() = Value::Number(cont as f64);
                    f.ip = finally_start;
                    continue;
                }
                Op::CallThis(arg_count) => {
                    // stack: [..., this, fn, args...]
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let func = self.stack.pop().unwrap_or(Value::Undefined);
                    let this = self.stack.pop().unwrap_or(Value::Undefined);
                    let result = self.call_function(&func, &args, Some(this))?;
                    self.stack.push(result);
                }
                Op::GetPrivate(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        match &frame.chunk.constants[name_idx] {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let v = if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Object(od) = o {
                                od.private_fields
                                    .lock()
                                    .unwrap()
                                    .get(name.as_str())
                                    .cloned()
                                    .unwrap_or(Value::Undefined)
                            } else {
                                Value::Undefined
                            }
                        })
                    } else {
                        Value::Undefined
                    };
                    self.stack.push(v);
                }
                Op::SetPrivate(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        match &frame.chunk.constants[name_idx] {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Object(od) = o {
                                od.private_fields
                                    .lock()
                                    .unwrap()
                                    .insert(Arc::from(name.as_str()), value.clone());
                            }
                        });
                    }
                    self.stack.push(value);
                }
                Op::CallPrivateMethod(name_idx, arg_count) => {
                    // stack: [..., obj, args...]
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let name = {
                        let frame = self.frames.last().unwrap();
                        match &frame.chunk.constants[name_idx] {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let method = if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Object(od) = o {
                                od.private_fields
                                    .lock()
                                    .unwrap()
                                    .get(name.as_str())
                                    .cloned()
                                    .unwrap_or(Value::Undefined)
                            } else {
                                Value::Undefined
                            }
                        })
                    } else {
                        Value::Undefined
                    };
                    let result = self.call_function(&method, &args, Some(obj))?;
                    self.stack.push(result);
                }
                Op::PopFinallyRethrow => {
                    // The finally body has run. Re-raise the pending
                    // completion (return/break/continue/throw) that diverted
                    // here, if any. A normal completion (tag 0) falls through.
                    let (tag, val) = {
                        let f = self.frames.last().unwrap();
                        (
                            f.finally_completion_tag.load(Ordering::Relaxed),
                            f.finally_completion_val.lock().unwrap().clone(),
                        )
                    };
                    {
                        let f = self.frames.last_mut().unwrap();
                        f.finally_completion_tag.store(0, Ordering::Relaxed);
                        *f.finally_completion_val.lock().unwrap() = Value::Undefined;
                    }
                    match tag {
                        0 => {} // normal: continue
                        1 => {
                            // return
                            // If an outer finally still guards this scope,
                            // divert the return through it before unwinding.
                            if let Some(frame) = self.frames.last_mut() {
                                if let Some(&(outer, _)) = frame.finally_stack.last() {
                                    frame.finally_completion_tag.store(1, Ordering::Relaxed);
                                    *frame.finally_completion_val.lock().unwrap() = val.clone();
                                    frame.ip = outer;
                                    continue;
                                }
                            }
                            // Re-run the return semantics now that no finally
                            // guards it.
                            self.frames.pop();
                            if self.frames.is_empty() {
                                return Ok(val);
                            }
                            if let Some(d) = return_depth {
                                if self.frames.len() <= d {
                                    return Ok(val);
                                }
                            }
                            self.stack.push(val);
                        }
                        4 => {
                            // throw
                            let frame = self.frames.last_mut().unwrap();
                            // If an outer finally still guards this scope,
                            // divert the throw through it first.
                            // Divert only if the outer finally is more deeply
                            // nested than the nearest catch (per spec, a throw
                            // is caught by the innermost matching handler, but
                            // must still run any finally nested inside it).
                            let divert_to_outer_finally =
                                match (frame.finally_stack.last(), frame.catch_stack.last()) {
                                    (Some(&(_, _)), None) => true,
                                    (Some(&(_, fseq)), Some(&(_, cseq))) => fseq > cseq,
                                    _ => false,
                                };
                            if divert_to_outer_finally {
                                let outer = frame.finally_stack.last().unwrap().0;
                                frame.finally_completion_tag.store(4, Ordering::Relaxed);
                                *frame.finally_completion_val.lock().unwrap() = val.clone();
                                frame.ip = outer;
                                continue;
                            }
                            // If an outer try catches, route there; else propagate.
                            if let Some(&(handler, _)) = frame.catch_stack.last() {
                                frame.catch_stack.pop();
                                frame.ip = handler;
                                self.stack.push(val);
                                continue;
                            }
                            return Err(Error::thrown(val, &self.heap));
                        }
                        // 2 (break) / 3 (continue): re-issue the recorded
                        // transfer by jumping to its saved target. These are
                        // recorded as the loop's break/continue ip.
                        2 | 3 => {
                            let frame = self.frames.last_mut().unwrap();
                            // If an outer finally still guards this scope,
                            // divert the break/continue through it first.
                            if let Some(&(outer, _)) = frame.finally_stack.last() {
                                frame.finally_completion_tag.store(tag, Ordering::Relaxed);
                                *frame.finally_completion_val.lock().unwrap() = val.clone();
                                frame.ip = outer;
                                continue;
                            }
                            let target = match val {
                                Value::Number(n) => n as usize,
                                _ => usize::MAX,
                            };
                            frame.ip = target;
                            continue;
                        }
                        _ => {}
                    }
                }
                Op::EnterCatch => {
                    // pop the thrown value and bind it; the compiler already
                    // emitted a StoreLocal for the catch param.
                }
                Op::Call(arg_count) => self.op_call(arg_count)?,
                Op::CallMethod(arg_count) => self.op_call_method(arg_count)?,
                Op::CallMethodOpt(arg_count) => self.op_call_method_opt(arg_count)?,
                Op::YieldValue => {
                    // Lazy generator: pop the yielded value and suspend execution.
                    // The `yield` expression's *result* (the value sent in by the
                    // next `next(v)`) is pushed onto the stack on resume, not here.
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    // Read the *current* frame's gen-state (per-frame isolation):
                    // a generator body that calls `next()` on another generator
                    // only suspends its own frame, not the nested one.
                    let in_gen = self
                        .frames
                        .last()
                        .map(|f| f.gen_mode.load(Ordering::Relaxed))
                        .unwrap_or(false);
                    if in_gen {
                        let frame = self.frames.last().unwrap();
                        *frame.gen_yield.lock().unwrap() = Some(v);
                        frame.gen_suspended.store(true, Ordering::Relaxed);
                        return Ok(Value::Undefined);
                    } else {
                        // Not in a generator context (shouldn't happen): behave eagerly.
                        self.current_yields.push(v);
                        self.stack.push(Value::Undefined);
                    }
                }
                Op::CallSuperCtor(arg_count) => {
                    // stack: [this, superCtor, args...]; call superCtor with this.
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let super_ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let this_val = self.stack.pop().unwrap_or(Value::Undefined);
                    // Call the parent constructor with `this` (not `new`, just call).
                    let result = self.call_function(&super_ctor, &args, Some(this_val.clone()))?;
                    // If the parent constructor returned an object, use it as the new `this`.
                    let new_this = if matches!(result, Value::Object(_)) {
                        result
                    } else {
                        this_val
                    };
                    // Rebind `this` in the current environment to the (possibly updated) value.
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::set(&self.heap, cur_env, "this", new_this.clone());
                    self.frames.last_mut().unwrap().this_val = new_this.clone();
                    self.stack.push(new_this);
                }
                Op::CallSuper(arg_count) => {
                    // stack (bottom->top): [this, superProto, key, args...]
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let super_proto = self.stack.pop().unwrap_or(Value::Undefined);
                    let this_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    // Look up the method on the parent prototype (and its chain).
                    let method = self.get_property(&super_proto, &key_str)?;
                    let result = self.call_function(&method, &args, Some(this_val))?;
                    self.stack.push(result);
                }
                Op::CallSpread => self.op_call_spread()?,
                Op::CallDirectEval(arg_count) => {
                    // Direct `eval(src, ...)`: per spec only the first argument
                    // is the source string; extras are ignored. Compile and run
                    // it in the caller's scope (current frame env + this).
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let src = match args.first() {
                        Some(Value::String(s)) => s.to_string(),
                        // Non-string first arg: return it as-is.
                        Some(v) => {
                            self.stack.push(v.clone());
                            continue;
                        }
                        None => {
                            self.stack.push(Value::Undefined);
                            continue;
                        }
                    };
                    let (caller_env, this_val, caller_strict) = self
                        .frames
                        .last()
                        .map(|f| (f.env, f.this_val.clone(), f.chunk.is_strict))
                        .unwrap_or((self.global, Value::Undefined, false));
                    let result = self.eval_direct(&src, caller_env, this_val, caller_strict)?;
                    self.stack.push(result);
                }
                Op::New(arg_count) => self.op_new(arg_count)?,
                Op::NewSpread => self.op_new_spread()?,
                Op::MakeClosure(func_idx) => self.op_make_closure(func_idx),
                Op::TypeOf => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let t = if let Value::Object(idx) = &v {
                        if self.heap.with_obj(idx.0, |o| o.is_function()) {
                            "function"
                        } else {
                            "object"
                        }
                    } else {
                        match &v {
                            Value::Object(_) => "object",
                            _ => v.type_of(),
                        }
                    };
                    self.stack.push(Value::String(Arc::from(t)));
                }
                Op::TypeCoerce => {
                    // unary +: ToNumber coercion.
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let n = self.to_number(&v)?;
                    self.stack.push(Value::Number(n));
                }
                Op::Await => self.op_await()?,
                Op::TypeofVar(name_idx) => {
                    // `typeof name`: "undefined" if the name is not bound (must not throw).
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => String::new(),
                        }
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let val = crate::environment::get(&self.heap, cur_env, &name)
                        .or_else(|| crate::environment::get(&self.heap, self.global, &name));
                    let t = match val {
                        Some(v) => {
                            if let Value::Object(idx) = &v {
                                if self.heap.with_obj(idx.0, |o| o.is_function()) {
                                    "function"
                                } else {
                                    "object"
                                }
                            } else {
                                v.type_of()
                            }
                        }
                        None => "undefined",
                    };
                    self.stack.push(Value::String(Arc::from(t)));
                }
                Op::GetIterator => {
                    let iterable = self.stack.pop().unwrap_or(Value::Undefined);
                    let it = self.make_iterator(&iterable)?;
                    self.stack.push(it);
                }
                Op::GetForInKeys => {
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let it = self.make_for_in_keys(&obj)?;
                    self.stack.push(it);
                }
                Op::IteratorNext => {
                    // pop iterator, push [value, done]
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let (value, done) = self.iterator_next(&it)?;
                    self.stack.push(value);
                    self.stack.push(Value::Bool(done));
                }
                Op::IteratorNextResume => {
                    // stack (bottom->top): [iterator, resume] -> pop both, push [value, done]
                    let resume = self.stack.pop().unwrap_or(Value::Undefined);
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let (value, done) = self.iterator_next_resume(&it, resume)?;
                    self.stack.push(value);
                    self.stack.push(Value::Bool(done));
                }
                Op::IteratorDone => {
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let done = self.iterator_done(&it);
                    self.stack.push(Value::Bool(done));
                }
                Op::GetAsyncIterator => {
                    let iterable = self.stack.pop().unwrap_or(Value::Undefined);
                    let it = self.make_async_iterator(&iterable)?;
                    self.stack.push(it);
                }
                Op::IteratorNextAwait => {
                    // Pop the iterator, call its `next()`, await the result,
                    // and push [value, done] (already awaited).
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let (value, done) = self.iterator_next_await(&it)?;
                    self.stack.push(value);
                    self.stack.push(Value::Bool(done));
                }
                Op::IteratorCollectRest => {
                    // Pop the iterator, drain its remaining values into a new
                    // array, and push the array. Used by rest in array patterns.
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let mut items = Vec::new();
                    loop {
                        let (value, done) = self.iterator_next(&it)?;
                        if done {
                            break;
                        }
                        items.push(value);
                    }
                    let arr = HeapObj::Array(crate::value::ArrayData {
                        items: Mutex::new(items),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(self.array_proto.clone())),
                    });
                    self.stack
                        .push(Value::Object(GcIdx(self.heap.allocate(arr))));
                }
                _ => {
                    panic!("unimplemented bytecode op: {:?}", op);
                }
            }
        }
    }

    fn pop2(&mut self) -> (Value, Value) {
        let b = self.stack.pop().unwrap_or(Value::Undefined);
        let a = self.stack.pop().unwrap_or(Value::Undefined);
        (a, b)
    }

    /// `Op::Call(arg_count)`: pop callee + args, apply `with`-this binding if
    /// the callee was resolved through a `with` object, and push the result.
    fn op_call(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        // If the callee was resolved through a `with`-statement object
        // environment record, bind `this` to that object (ES spec). Otherwise
        // use `undefined` (strict-mode-style). Take and clear the pending value
        // so it never leaks past this call.
        let with_this = self
            .frames
            .last()
            .map(|f| f.pending_with_this.lock().unwrap().take())
            .unwrap_or(None);
        let this = with_this.or(Some(Value::Undefined));
        let result = self.call_function(&callee, &args, this)?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::CallMethod(arg_count)`: `obj.key(...args)` (computed member call).
    fn op_call_method(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let key = self.stack.pop().unwrap_or(Value::Undefined);
        let obj = self.stack.pop().unwrap_or(Value::Undefined);
        let key_str = self.to_property_key(&key)?;
        let method = self.get_property(&obj, &key_str)?;
        let result = self.call_function(&method, &args, Some(obj))?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::CallMethodOpt(arg_count)`: optional chaining member call.
    fn op_call_method_opt(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let key = self.stack.pop().unwrap_or(Value::Undefined);
        let obj = self.stack.pop().unwrap_or(Value::Undefined);
        let key_str = self.to_property_key(&key)?;
        let method = self.get_property(&obj, &key_str)?;
        if method.is_nullish() {
            self.stack.push(Value::Undefined);
        } else {
            let result = self.call_function(&method, &args, Some(obj))?;
            self.stack.push(result);
        }
        Ok(())
    }

    /// `Op::CallSpread`: spread an array's items as call arguments.
    fn op_call_spread(&mut self) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let mut args = Vec::new();
        if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    args = a.items.lock().unwrap().clone();
                }
            });
        }
        let result = self.call_function(&callee, &args, Some(Value::Undefined))?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::New(arg_count)`: constructor call.
    fn op_new(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let constructor = self.stack.pop().unwrap_or(Value::Undefined);
        let result = self.construct(&constructor, &args)?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::NewSpread`: constructor call with spread args. Stack: [ctor, argsArr].
    fn op_new_spread(&mut self) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let constructor = self.stack.pop().unwrap_or(Value::Undefined);
        let args = if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    a.items.lock().unwrap().clone()
                } else {
                    Vec::new()
                }
            })
        } else {
            Vec::new()
        };
        let result = self.construct(&constructor, &args)?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::Await`: synchronous await. If the value is a pending Promise, drain
    /// microtasks until it settles, then push its result (or rethrow rejection).
    fn op_await(&mut self) -> error::Result<()> {
        let v = self.stack.pop().unwrap_or(Value::Undefined);
        if let Value::Object(idx) = &v {
            let is_promise = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
            if is_promise {
                self.run_microtasks()?;
                let (state, result) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Promise(p) = o {
                        (*p.state.lock().unwrap(), p.result.lock().unwrap().clone())
                    } else {
                        (PromiseStatus::Fulfilled, Value::Undefined)
                    }
                });
                if state == PromiseStatus::Rejected {
                    return Err(Error::thrown(result, &self.heap));
                }
                self.stack.push(result);
                return Ok(());
            }
        }
        self.stack.push(v);
        Ok(())
    }

    /// `Op::MakeClosure(func_idx)`: build a function object capturing the
    /// current environment, with a `.prototype` for non-arrow functions.
    fn op_make_closure(&mut self, func_idx: usize) {
        if let Some(fdef) = self.functions.get(func_idx).cloned() {
            let env_idx = self.frames.last().map(|f| f.env).unwrap_or(self.global);
            let is_arrow = fdef.is_arrow;
            // create a .prototype object for non-arrow functions
            let proto_val = if !fdef.is_arrow {
                let proto = HeapObj::Object(crate::value::ObjectData {
                    props: Mutex::new(IndexMap::new()),
                    proto: Mutex::new(Some(self.object_proto.clone())),
                    extensible: std::sync::atomic::AtomicBool::new(true),
                    class_name: None,
                    private_fields: Mutex::new(std::collections::HashMap::new()),
                    primitive: Mutex::new(None),
                });
                Value::Object(GcIdx(self.heap.allocate(proto)))
            } else {
                Value::Undefined
            };
            let fd = crate::value::FunctionData {
                name: fdef.name.clone(),
                kind: crate::value::FunctionKind::Interpreted { func: fdef },
                closure: env_idx,
                prototype: Mutex::new(if !is_arrow {
                    Some(proto_val.clone())
                } else {
                    None
                }),
                proto: Mutex::new(match self.function_proto {
                    Value::Object(_) => Some(self.function_proto.clone()),
                    _ => None,
                }),
                props: Mutex::new(IndexMap::new()),
            };
            let idx = self.heap.allocate(HeapObj::Function(fd));
            // link prototype.constructor back to the function
            if let Value::Object(pidx) = &proto_val {
                self.heap.with_obj(pidx.0, |obj| {
                    let mut desc =
                        crate::value::PropertyDescriptor::data(Value::Object(GcIdx(idx)));
                    desc.enumerable = false;
                    obj.props()
                        .lock()
                        .unwrap()
                        .insert(crate::value::PropertyKey::from("constructor"), desc);
                });
            }
            self.stack.push(Value::Object(GcIdx(idx)));
        } else {
            self.stack.push(Value::Undefined);
        }
    }

    #[allow(dead_code)]
    fn num_bin<F: Fn(f64, f64) -> f64>(&mut self, f: F) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = self.to_number(&a)?;
        let bv = self.to_number(&b)?;
        self.stack.push(Value::Number(f(av, bv)));
        Ok(())
    }

    fn int_bin<F: Fn(i32, i32) -> i32>(&mut self, f: F) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = self.to_number(&a)? as i32;
        let bv = self.to_number(&b)? as i32;
        self.stack.push(Value::Number(f(av, bv) as f64));
        Ok(())
    }

    /// Like `num_bin`, but if both operands are `BigInt`, keep the result a
    /// `BigInt` (arbitrary precision via num-bigint).
    fn num_bin_bigint<
        F: Fn(f64, f64) -> f64,
        B: Fn(num_bigint::BigInt, num_bigint::BigInt) -> num_bigint::BigInt,
    >(
        &mut self,
        numf: F,
        bigf: B,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        match (&a, &b) {
            (Value::BigInt(x), Value::BigInt(y)) => {
                self.stack.push(Value::BigInt(bigf(x.clone(), y.clone())));
            }
            (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                // Mixing BigInt with non-bigint numbers is a TypeError per spec.
                return Err(Error::type_err(
                    "Cannot mix BigInt and other types, use explicit conversions".to_string(),
                ));
            }
            _ => {
                let av = self.to_number(&a)?;
                let bv = self.to_number(&b)?;
                self.stack.push(Value::Number(numf(av, bv)));
            }
        }
        Ok(())
    }

    fn bin_op<F: Fn(f64, f64) -> Value, G: Fn(&str, &str) -> Value>(
        &mut self,
        numf: F,
        _strf: G,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        // BigInt + BigInt stays BigInt; mixing with other types is a TypeError.
        match (&a, &b) {
            (Value::BigInt(x), Value::BigInt(y)) => {
                self.stack.push(Value::BigInt(x + y));
                return Ok(());
            }
            (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                return Err(Error::type_err(
                    "Cannot mix BigInt and other types, use explicit conversions".to_string(),
                ));
            }
            _ => {}
        }
        // string concatenation
        let ap = self.to_primitive(&a)?;
        let bp = self.to_primitive(&b)?;
        match (&ap, &bp) {
            // BigInt + BigInt stays BigInt; mixing with other types is a TypeError.
            (Value::BigInt(x), Value::BigInt(y)) => {
                self.stack.push(Value::BigInt(x + y));
                return Ok(());
            }
            (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                return Err(Error::type_err(
                    "Cannot mix BigInt and other types, use explicit conversions".to_string(),
                ));
            }
            (Value::String(_), _) | (_, Value::String(_)) => {
                let sa = self.to_string(&ap)?;
                let sb = self.to_string(&bp)?;
                self.stack
                    .push(Value::String(Arc::from(format!("{}{}", sa, sb).as_str())));
            }
            _ => {
                let av = self.to_number(&ap)?;
                let bv = self.to_number(&bp)?;
                self.stack.push(numf(av, bv));
            }
        }
        Ok(())
    }

    fn compare<F: Fn(f64, f64) -> bool, S: Fn(&str, &str) -> bool>(
        &mut self,
        f: F,
        sf: S,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        let pa = self.to_primitive(&a)?;
        let pb = self.to_primitive(&b)?;
        // BigInt vs BigInt: compare exactly without f64 rounding.
        if let (Value::BigInt(x), Value::BigInt(y)) = (&pa, &pb) {
            let xf = num_traits::ToPrimitive::to_f64(x).unwrap_or(f64::NAN);
            let yf = num_traits::ToPrimitive::to_f64(y).unwrap_or(f64::NAN);
            self.stack.push(Value::Bool(f(xf, yf)));
            return Ok(());
        }
        if let (Value::String(sa), Value::String(sb)) = (&pa, &pb) {
            self.stack.push(Value::Bool(sf(sa, sb)));
        } else {
            let av = self.to_number(&pa)?;
            let bv = self.to_number(&pb)?;
            if av.is_nan() || bv.is_nan() {
                self.stack.push(Value::Bool(false));
            } else {
                self.stack.push(Value::Bool(f(av, bv)));
            }
        }
        Ok(())
    }

    // ---- type conversions ----

    /// StringNumericLiteral -> Number (ES ToNumber applied to a string).
    /// Handles leading/trailing whitespace, `Infinity`/`-Infinity`, and the
    /// `0x`/`0b`/`0o` integer-literal prefixes; anything else is a decimal
    /// float, or NaN if it does not parse.
    fn string_to_number(s: &str) -> f64 {
        let t = s.trim();
        if t.is_empty() {
            return 0.0;
        }
        if t == "Infinity" || t == "+Infinity" {
            return f64::INFINITY;
        }
        if t == "-Infinity" {
            return f64::NEG_INFINITY;
        }
        // Hex/binary/octal integer literals (no sign, no fraction).
        let (radix, digits) = if let Some(d) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))
        {
            (16, d)
        } else if let Some(d) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            (2, d)
        } else if let Some(d) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            (8, d)
        } else {
            return t.parse::<f64>().unwrap_or(f64::NAN);
        };
        if digits.is_empty() {
            return f64::NAN;
        }
        match u64::from_str_radix(digits, radix) {
            Ok(n) => n as f64,
            Err(_) => f64::NAN,
        }
    }

    pub fn to_number(&mut self, v: &Value) -> error::Result<f64> {
        Ok(match v {
            Value::Undefined => f64::NAN,
            Value::Null => 0.0,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(n) => *n,
            Value::BigInt(n) => num_traits::ToPrimitive::to_f64(n).unwrap_or(f64::NAN),
            Value::String(s) => Self::string_to_number(s),
            Value::Object(_) => {
                // Per ES ToNumber on objects: run ToPrimitive(number hint)
                // (valueOf then toString), then convert the primitive result.
                let prim = self.to_primitive_number(v)?;
                self.to_number(&prim)?
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to number".to_string(),
                ));
            }
        })
    }

    pub fn to_string(&mut self, v: &Value) -> error::Result<Arc<str>> {
        Ok(match v {
            Value::Undefined => Arc::from("undefined"),
            Value::Null => Arc::from("null"),
            Value::Bool(b) => Arc::from(b.to_string().as_str()),
            Value::Number(n) => Arc::from(crate::value::num_to_string(*n).as_str()),
            Value::String(s) => s.clone(),
            Value::BigInt(n) => Arc::from(n.to_string().as_str()),
            Value::Object(idx) => {
                let is_array = self
                    .heap
                    .with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_)));
                if is_array {
                    // join items outside the borrow
                    let items = self.heap.with_obj(idx.0, |obj| {
                        if let HeapObj::Array(a) = obj {
                            a.items.lock().unwrap().clone()
                        } else {
                            Vec::new()
                        }
                    });
                    let parts: Vec<String> = items
                        .iter()
                        .map(|i| {
                            if i.is_nullish() {
                                String::new()
                            } else {
                                self.to_string(i).map(|s| s.to_string()).unwrap_or_default()
                            }
                        })
                        .collect();
                    Arc::from(parts.join(",").as_str())
                } else {
                    // Honor a user-defined `toString` method (it returns a
                    // primitive that we then stringify). This is evaluated
                    // outside the heap borrow so it can call back into the VM.
                    let ts = self.get_property(v, "toString")?;
                    if matches!(ts, Value::Object(_)) {
                        let r = self.call_function(&ts, &[], Some(v.clone()))?;
                        if !matches!(r, Value::Object(_)) {
                            return self.to_string(&r);
                        }
                    }
                    // No usable toString: use the default class tag.
                    self.heap.with_obj(idx.0, |obj| match obj {
                        HeapObj::Object(o) => {
                            if let Some(cn) = &o.class_name {
                                cn.clone()
                            } else {
                                Arc::from("[object Object]")
                            }
                        }
                        _ => Arc::from("[object Object]"),
                    })
                }
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to string".to_string(),
                ));
            }
        })
    }

    /// Default-hint ToPrimitive (used by binary `+` and `==`): valueOf then
    /// toString, with "default" passed to @@toPrimitive.
    pub fn to_primitive(&mut self, v: &Value) -> error::Result<Value> {
        self.to_primitive_ex(v, false, "default")
    }
    /// Number-hint ToPrimitive (used by unary `+`, arithmetic, Number()):
    /// valueOf then toString, with "number" passed to @@toPrimitive.
    pub fn to_primitive_number(&mut self, v: &Value) -> error::Result<Value> {
        self.to_primitive_ex(v, false, "number")
    }
    /// Convert a value to a primitive per the ES OrdinaryToPrimitive
    /// abstract operation. For objects, invoke `valueOf` then `toString`
    /// (default/number hint) or `toString` then `valueOf` (string hint),
    /// returning the first non-object result. Arrays/objects without custom
    /// methods fall back to their default string form.
    pub fn to_primitive_hint(&mut self, v: &Value, string_hint: bool) -> error::Result<Value> {
        let hint = if string_hint { "string" } else { "number" };
        self.to_primitive_ex(v, string_hint, hint)
    }
    /// Shared ToPrimitive body. `string_hint` controls the valueOf/toString
    /// order; `hint` is the string passed to @@toPrimitive.
    #[allow(clippy::wrong_self_convention)]
    fn to_primitive_ex(
        &mut self,
        v: &Value,
        string_hint: bool,
        hint: &'static str,
    ) -> error::Result<Value> {
        match v {
            Value::Object(_) => {
                // ES ToPrimitive: an object may define @@toPrimitive, which
                // takes precedence over valueOf/toString and receives the hint.
                {
                    let tp_key =
                        crate::value::PropertyKey::Symbol(self.well_known_symbols.to_primitive);
                    let method = self.get_property_by_key(v, &tp_key)?;
                    if matches!(method, Value::Object(_)) {
                        let hint_str = Arc::from(hint);
                        let result = self.call_function(
                            &method,
                            &[Value::String(hint_str)],
                            Some(v.clone()),
                        )?;
                        if matches!(result, Value::Object(_)) {
                            return Err(Error::type_err(
                                "Cannot convert object to primitive value".to_string(),
                            ));
                        }
                        return Ok(result);
                    }
                }
                // Boxed primitives (`new Number(5)`, `Object("x")`):
                // ToPrimitive returns the wrapped primitive via valueOf,
                // unless a string hint asks for toString (e.g. `${...}`).
                if !string_hint {
                    if let Value::Object(idx) = v {
                        let prim = self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Object(od) = o {
                                od.primitive.lock().unwrap().clone()
                            } else {
                                None
                            }
                        });
                        if let Some(p) = prim {
                            return Ok(p);
                        }
                    }
                }
                // Arrays have a well-defined default toString (join with ",");
                // honor it directly rather than looking up a method that may
                // not be installed on Array.prototype yet.
                let is_array = match v {
                    Value::Object(idx) => self
                        .heap
                        .with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_))),
                    _ => false,
                };
                let methods: [&str; 2] = if string_hint {
                    ["toString", "valueOf"]
                } else {
                    ["valueOf", "toString"]
                };
                if is_array && !string_hint {
                    // valueOf on an array returns the array (object), so skip
                    // straight to toString to avoid a pointless call.
                    return Ok(Value::String(self.to_string(v)?));
                }
                for name in methods {
                    let method = self.get_property(v, name)?;
                    if matches!(method, Value::Object(_)) {
                        let result = self.call_function(&method, &[], Some(v.clone()))?;
                        if !matches!(result, Value::Object(_)) {
                            return Ok(result);
                        }
                    }
                }
                // Both returned objects (or were missing): fall back to a
                // best-effort string form.
                // Both returned objects (or were missing): per spec
                // OrdinaryToPrimitive throws a TypeError when neither yields
                // a primitive.
                Err(Error::type_err(
                    "Cannot convert object to primitive value".to_string(),
                ))
            }
            _ => Ok(v.clone()),
        }
    }

    /// Coerce a `Value` to a property key as a `String`.
    ///
    /// Symbols cannot be converted to a string key and return `Err` (a Symbol
    /// must be looked up via [`get_property_key`] / [`set_property_key`] using
    /// the `Value::Symbol` directly).
    pub fn to_property_key(&mut self, v: &Value) -> error::Result<String> {
        match v {
            Value::String(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(crate::value::num_to_string(*n)),
            Value::Symbol(_) => Err(Error::type_err(
                "Cannot convert a Symbol value to a string key".to_string(),
            )),
            _ => Ok(self.to_string(v)?.to_string()),
        }
    }

    /// Get a property by a `Value` key, supporting string keys (via the
    /// existing `get_property(&str)` path) and Symbol keys (looked up directly
    /// in the object's `props` map as `PropertyKey::Symbol`).
    pub fn get_property_key(&mut self, obj: &Value, key: &Value) -> error::Result<Value> {
        match key {
            Value::Symbol(id) => {
                let pkey = crate::value::PropertyKey::Symbol(*id);
                self.get_property_by_key(obj, &pkey)
            }
            other => {
                let s = self.to_property_key(other)?;
                self.get_property(obj, &s)
            }
        }
    }

    /// Set a property by a `Value` key, supporting string and Symbol keys.
    pub fn set_property_key(
        &mut self,
        obj: &Value,
        key: &Value,
        value: Value,
    ) -> error::Result<()> {
        match key {
            Value::Symbol(id) => {
                if let Value::Object(idx) = obj {
                    let pkey = crate::value::PropertyKey::Symbol(*id);
                    self.heap.with_obj(idx.0, |o| {
                        o.props()
                            .lock()
                            .unwrap()
                            .insert(pkey, crate::value::PropertyDescriptor::data(value.clone()));
                    });
                    Ok(())
                } else {
                    Err(Error::type_err(
                        "Cannot set property of primitive".to_string(),
                    ))
                }
            }
            other => {
                let s = self.to_property_key(other)?;
                self.set_property(obj, &s, value)
            }
        }
    }

    /// Look up a property by a `PropertyKey` (string or Symbol), walking the
    /// prototype chain. Used internally by [`get_property_key`] for Symbol
    /// keys and by the iterator protocol for `Symbol.iterator`.
    pub fn get_property_by_key(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> error::Result<Value> {
        let mut cur = obj.clone();
        let mut depth = 0;
        while let Value::Object(idx) = &cur {
            if depth > 1024 {
                break;
            }
            depth += 1;
            let (found, proto) = self.heap.with_obj(idx.0, |o| {
                let props = o.props();
                let v = props.lock().unwrap().get(key).map(|d| d.value.clone());
                let proto = o.proto().lock().unwrap().clone();
                (v, proto)
            });
            if let Some(v) = found {
                return Ok(v);
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        Ok(Value::Undefined)
    }

    /// Does `obj` (or its prototype chain) have an own/inherited property for
    /// the given `PropertyKey`? Used by the iterator protocol to detect a
    /// user-defined `Symbol.iterator`.
    pub fn has_property_key(&self, obj: &Value, key: &crate::value::PropertyKey) -> bool {
        let mut cur = obj.clone();
        let mut depth = 0;
        while let Value::Object(idx) = &cur {
            if depth > 1024 {
                break;
            }
            depth += 1;
            let (has, proto) = self.heap.with_obj(idx.0, |o| {
                (
                    o.props().lock().unwrap().contains_key(key),
                    o.proto().lock().unwrap().clone(),
                )
            });
            if has {
                return true;
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        false
    }

    /// Does `obj` (or its prototype chain) have a named property? Used by the
    /// `with` statement to decide whether to assign to a `with` object.
    /// Does `obj` (or its prototype chain) have a named property? Unlike the
    /// previous undefined-sentinel check, this walks the own-property maps so
    /// a property whose value is `undefined` is still "present" (per spec
    /// `[[HasProperty]]`). Used by the `with` statement.
    pub fn has_property(&mut self, obj: &Value, name: &str) -> error::Result<bool> {
        // Fast path: objects with a props map walk own + proto for the key.
        let pkey = crate::value::PropertyKey::from(name);
        if self.has_property_key(obj, &pkey) {
            return Ok(true);
        }
        // Arrays expose indexed "properties" and `length`; strings expose
        // indexed chars and `length`. Treat those as present.
        match obj {
            Value::Object(idx) => {
                let (is_arr, len) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        (true, a.items.lock().unwrap().len())
                    } else {
                        (false, 0)
                    }
                });
                if is_arr && (name == "length" || name.parse::<usize>().is_ok_and(|i| i < len)) {
                    return Ok(true);
                }
                Ok(false)
            }
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                Ok(name == "length" || name.parse::<usize>().is_ok_and(|i| i < len))
            }
            _ => Ok(false),
        }
    }

    /// Does `obj` have an OWN property named `name` (not inherited)?
    /// Used by ToPropertyDescriptor (Object.defineProperty) to tell a field
    /// that was explicitly set to `undefined` from a field that is simply
    /// absent on the descriptor object.
    pub fn has_own(&self, obj: &Value, name: &str) -> bool {
        let pkey = crate::value::PropertyKey::from(name);
        match obj {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    if name == "length" {
                        return true;
                    }
                    if let Ok(i) = name.parse::<usize>() {
                        return i < a.items.lock().unwrap().len();
                    }
                    // array extra props live in props()
                }
                o.props().lock().unwrap().contains_key(&pkey)
            }),
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                name == "length" || name.parse::<usize>().is_ok_and(|i| i < len)
            }
            _ => false,
        }
    }

    pub fn to_boolean(&self, v: &Value) -> bool {
        v.is_truthy()
    }

    pub fn strict_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(x), Value::Number(y)) => {
                if x.is_nan() || y.is_nan() {
                    false
                } else {
                    x == y
                }
            }
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => x == y,
            (Value::Symbol(x), Value::Symbol(y)) => x == y,
            (Value::BigInt(x), Value::BigInt(y)) => x == y,
            _ => false,
        }
    }

    pub fn loose_eq(&mut self, a: &Value, b: &Value) -> error::Result<bool> {
        if std::mem::discriminant(a) == std::mem::discriminant(b) {
            return Ok(self.strict_eq(a, b));
        }
        Ok(match (a, b) {
            (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => true,
            (Value::Number(_), Value::String(_)) => {
                let bn = self.to_number(b)?;
                self.strict_eq(a, &Value::Number(bn))
            }
            (Value::String(_), Value::Number(_)) => {
                let an = self.to_number(a)?;
                self.strict_eq(&Value::Number(an), b)
            }
            (Value::Bool(_), _) => {
                let an = self.to_number(a)?;
                self.loose_eq(&Value::Number(an), b)?
            }
            (_, Value::Bool(_)) => {
                let bn = self.to_number(b)?;
                self.loose_eq(a, &Value::Number(bn))?
            }
            // Object vs primitive: ToPrimitive the object, then compare.
            (Value::Object(_), _) if !b.is_object() => {
                let ap = self.to_primitive(a)?;
                self.loose_eq(&ap, b)?
            }
            (_, Value::Object(_)) if !a.is_object() => {
                let bp = self.to_primitive(b)?;
                self.loose_eq(a, &bp)?
            }
            // BigInt vs Number: compare numerically.
            (Value::BigInt(x), Value::Number(y)) => {
                num_traits::ToPrimitive::to_f64(x).unwrap_or(f64::NAN) == *y
            }
            (Value::Number(x), Value::BigInt(y)) => {
                *x == num_traits::ToPrimitive::to_f64(y).unwrap_or(f64::NAN)
            }
            // BigInt vs String: parse the string, then compare.
            (Value::BigInt(x), Value::String(s)) => {
                num_bigint::BigInt::parse_bytes(s.trim().as_bytes(), 10)
                    .map(|v| v == *x)
                    .unwrap_or(false)
            }
            (Value::String(s), Value::BigInt(y)) => {
                num_bigint::BigInt::parse_bytes(s.trim().as_bytes(), 10)
                    .map(|v| v == *y)
                    .unwrap_or(false)
            }
            _ => false,
        })
    }

    // ---- property access ----

    pub fn get_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        match obj {
            Value::String(s) => {
                if key == "length" {
                    return Ok(Value::Number(crate::value::utf16_len(s) as f64));
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(unit) = crate::value::utf16_get(s, idx) {
                        return Ok(Value::String(Arc::from(
                            String::from_utf16_lossy(&[unit]).as_str(),
                        )));
                    }
                    return Ok(Value::Undefined);
                }
                self.get_proto_property(obj, key)
            }
            Value::Number(_) => self.get_proto_property(obj, key),
            Value::BigInt(_) => self.get_proto_property(obj, key),
            Value::Bool(_) => self.get_proto_property(obj, key),
            Value::Symbol(_) => self.get_proto_property(obj, key),
            Value::Undefined | Value::Null => Err(Error::type_err(format!(
                "Cannot read properties of {} (reading '{}')",
                obj.type_of(),
                key
            ))),
            Value::Object(idx) => {
                // Honor an accessor getter on this object (own property).
                // Inherited accessors are handled by the recursive proto-chain
                // walk below, since `get_property` is called again on the
                // prototype. The getter must be invoked outside the
                // `with_obj` borrow, so we look it up first.
                let pkey = crate::value::PropertyKey::from(key);
                if let Some(getter) = self.heap.with_obj(idx.0, |o| {
                    o.props().lock().unwrap().get(&pkey).and_then(|d| {
                        if d.is_accessor {
                            d.get.clone()
                        } else {
                            None
                        }
                    })
                }) {
                    if !getter.is_undefined() {
                        return self.call_function(&getter, &[], Some(obj.clone()));
                    }
                    return Ok(Value::Undefined);
                }
                // __proto__ getter returns the object's [[Prototype]].
                if key == "__proto__" {
                    return Ok(self.heap.with_obj(idx.0, |o| {
                        o.proto().lock().unwrap().clone().unwrap_or(Value::Null)
                    }));
                }
                // globalThis routes property reads to the global environment.
                let is_global_this = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("global"))
                });
                if is_global_this {
                    if let Some(v) = crate::environment::get(&self.heap, self.global, key) {
                        return Ok(v);
                    }
                }
                // array
                let proto = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            return Ok::<Value, Error>(Value::Number(
                                a.items.lock().unwrap().len() as f64,
                            ));
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            let items = a.items.lock().unwrap();
                            return Ok(items.get(i).cloned().unwrap_or(Value::Undefined));
                        }
                    }
                    if let HeapObj::Map(m) = o {
                        if key == "size" {
                            return Ok(Value::Number(m.entries.lock().unwrap().len() as f64));
                        }
                    }
                    if let HeapObj::Set(s) = o {
                        if key == "size" {
                            return Ok(Value::Number(s.items.lock().unwrap().len() as f64));
                        }
                    }
                    let props = o.props();
                    if let Some(desc) = props.lock().unwrap().get(&pkey) {
                        return Ok(desc.value.clone());
                    }
                    // function-specific: .prototype lives in a dedicated field
                    if let HeapObj::Function(f) = o {
                        if key == "prototype" {
                            if let Some(p) = f.prototype.lock().unwrap().as_ref() {
                                return Ok(p.clone());
                            }
                        }
                        if key == "name" {
                            if let Some(n) = &f.name {
                                return Ok(Value::String(n.clone()));
                            }
                            return Ok(Value::String(Arc::from("")));
                        }
                        if key == "length" {
                            if let crate::value::FunctionKind::Native { length, .. } = &f.kind {
                                return Ok(Value::Number(*length as f64));
                            }
                            if let crate::value::FunctionKind::Interpreted { func } = &f.kind {
                                return Ok(Value::Number(func.length as f64));
                            }
                        }
                    }
                    Ok(Value::Undefined)
                });
                let val = proto?;
                if !val.is_undefined() {
                    return Ok(val);
                }
                // walk proto chain, preserving the original receiver so that
                // getters inherited from a prototype bind `this` to the receiver.
                let p = self
                    .heap
                    .with_obj(idx.0, |o| o.proto().lock().unwrap().clone());
                if let Some(proto) = p {
                    if !proto.is_undefined() {
                        return self.get_property_rx(&proto, key, obj.clone());
                    }
                }
                Ok(Value::Undefined)
            }
            #[allow(unreachable_patterns)]
            _ => Ok(Value::Undefined),
        }
    }

    /// Like `get_property` but tracks the receiver (the original object the
    /// property access started from) so inherited accessors bind `this` to it.
    fn get_property_rx(&mut self, obj: &Value, key: &str, receiver: Value) -> error::Result<Value> {
        match obj {
            Value::Object(idx) => {
                let pkey = crate::value::PropertyKey::from(key);
                // Own accessor on this object?
                if let Some(getter) = self.heap.with_obj(idx.0, |o| {
                    o.props().lock().unwrap().get(&pkey).and_then(|d| {
                        if d.is_accessor {
                            d.get.clone()
                        } else {
                            None
                        }
                    })
                }) {
                    if !getter.is_undefined() {
                        return self.call_function(&getter, &[], Some(receiver));
                    }
                    return Ok(Value::Undefined);
                }
                // Own data property?
                let val = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            return Some(Value::Number(a.items.lock().unwrap().len() as f64));
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            return Some(
                                a.items
                                    .lock()
                                    .unwrap()
                                    .get(i)
                                    .cloned()
                                    .unwrap_or(Value::Undefined),
                            );
                        }
                    }
                    o.props()
                        .lock()
                        .unwrap()
                        .get(&pkey)
                        .map(|d| d.value.clone())
                });
                if let Some(v) = val {
                    return Ok(v);
                }
                // Walk up.
                let p = self
                    .heap
                    .with_obj(idx.0, |o| o.proto().lock().unwrap().clone());
                if let Some(proto) = p {
                    if !proto.is_undefined() {
                        return self.get_property_rx(&proto, key, receiver);
                    }
                }
                Ok(Value::Undefined)
            }
            _ => self.get_property(obj, key),
        }
    }

    fn get_proto_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        let proto = match obj {
            Value::String(_) => self.string_proto.clone(),
            Value::Number(_) => self.number_proto.clone(),
            Value::BigInt(_) => self.bigint_proto.clone(),
            Value::Bool(_) => self.boolean_proto.clone(),
            Value::Symbol(_) => self.symbol_proto.clone(),
            _ => return Ok(Value::Undefined),
        };
        if !proto.is_undefined() {
            return self.get_property(&proto, key);
        }
        Ok(Value::Undefined)
    }

    /// Delete an own property. Returns true if removed (or didn't exist).
    pub fn delete_property(&mut self, obj: &Value, key: &str) -> error::Result<bool> {
        if let Value::Object(idx) = obj {
            let pkey = crate::value::PropertyKey::from(key);
            let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .unwrap()
                    .get(&pkey)
                    .map_or((false, true), |d| (true, d.configurable))
            });
            if exists && !configurable {
                return Ok(false);
            }
            self.heap.with_obj(idx.0, |o| {
                o.props().lock().unwrap().shift_remove(&pkey);
            });
        }
        Ok(true)
    }

    pub fn set_property(&mut self, obj: &Value, key: &str, value: Value) -> error::Result<()> {
        // ES [[Set]] semantics, simplified:
        //  1. Walk the prototype chain for an accessor descriptor with a
        //     `set` function; if found, call it and return.
        //  2. Otherwise, if `obj` has its OWN data descriptor that is
        //     non-writable, the assignment fails: in strict mode throw a
        //     TypeError; otherwise silently ignore.
        //  3. Otherwise define/overwrite an own writable data property.
        // Arrays route `length` and integer-index writes through dedicated
        // logic below before falling back to ordinary object semantics.
        match obj {
            Value::Object(idx) => {
                // __proto__ assignment sets the object's [[Prototype]].
                if key == "__proto__" {
                    match &value {
                        Value::Object(_) | Value::Null => {
                            let proto = if value.is_null() {
                                None
                            } else {
                                Some(value.clone())
                            };
                            self.heap.with_obj(idx.0, |o| {
                                *o.proto().lock().unwrap() = proto;
                            });
                            return Ok(());
                        }
                        // non-object, non-null: ignore (spec: no-op in sloppy mode)
                        _ => return Ok(()),
                    }
                }
                // globalThis routes property writes to the global environment.
                let is_global_this = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("global"))
                });
                if is_global_this {
                    // Set on the global environment: update if it exists,
                    // otherwise declare a new global binding.
                    if !crate::environment::set(&self.heap, self.global, key, value.clone()) {
                        crate::environment::declare(
                            &self.heap,
                            self.global,
                            key,
                            value.clone(),
                            crate::value::BindingKind::Var,
                        );
                    }
                    return Ok(());
                }
                // --- Array fast paths ---
                let is_array_length = self
                    .heap
                    .with_obj(idx.0, |o| matches!(o, HeapObj::Array(_) if key == "length"));
                if is_array_length {
                    return self.set_array_length(idx.0, value);
                }
                let array_index = self.heap.with_obj(idx.0, |o| {
                    if matches!(o, HeapObj::Array(_)) {
                        key.parse::<usize>().ok()
                    } else {
                        None
                    }
                });
                if let Some(i) = array_index {
                    self.set_array_index(idx.0, i, value)?;
                    return Ok(());
                }

                // --- Ordinary object [[Set]] ---
                let pkey = crate::value::PropertyKey::from(key);

                // 1. Look for an accessor `set` up the prototype chain.
                if let Some(setter) = self.find_setter(*idx, &pkey) {
                    self.call_function(&setter, std::slice::from_ref(&value), Some(obj.clone()))?;
                    return Ok(());
                }

                // 2. Reject writes to a non-writable own data property.
                let non_writable_own = self.heap.with_obj(idx.0, |o| {
                    o.props()
                        .lock()
                        .unwrap()
                        .get(&pkey)
                        .is_some_and(|d| !d.is_accessor && !d.writable)
                });
                if non_writable_own {
                    if self.current_strict() {
                        return Err(Error::type_err(format!(
                            "Cannot assign to read only property '{}' of object",
                            key
                        )));
                    }
                    // non-strict: silently ignore
                    return Ok(());
                }

                // 3. Define/overwrite an own writable data property.
                self.heap.with_obj(idx.0, |o| {
                    let props = o.props();
                    let mut props = props.lock().unwrap();
                    if let Some(existing) = props.get_mut(&pkey) {
                        existing.value = value;
                    } else {
                        props.insert(pkey, crate::value::PropertyDescriptor::data(value));
                    }
                });
                Ok(())
            }
            _ => Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            )),
        }
    }

    /// Strictness of the currently-executing frame, used by ordinary
    /// [[Set]]/[[DefineOwnProperty]] to decide whether a failed assignment
    /// throws a TypeError or is silently ignored. The top-level program has
    /// no frame; its strictness comes from the compiled top-level chunk.
    fn current_strict(&self) -> bool {
        self.frames
            .last()
            .map(|f| f.chunk.is_strict)
            .unwrap_or(false)
    }

    /// Walk the prototype chain starting at `idx` looking for an accessor
    /// descriptor for `key` with a non-empty `set` function. Returns the
    /// setter to invoke, if any. Used by ordinary [[Set]] to honor inherited
    /// setters.
    fn find_setter(&mut self, mut idx: GcIdx, key: &crate::value::PropertyKey) -> Option<Value> {
        let mut depth = 0;
        while depth < 1024 {
            depth += 1;
            let (found, proto) = self.heap.with_obj(idx.0, |o| {
                let props = o.props();
                let setter = props.lock().unwrap().get(key).and_then(|d| {
                    if d.is_accessor {
                        d.set.clone()
                    } else {
                        None
                    }
                });
                let proto = o.proto().lock().unwrap().clone();
                (setter, proto)
            });
            if let Some(s) = found {
                return Some(s);
            }
            match proto {
                Some(Value::Object(pidx)) => idx = pidx,
                _ => break,
            }
        }
        None
    }

    /// Set an integer-indexed element of an array, extending with
    /// `undefined` holes as needed.
    fn set_array_index(&mut self, idx: usize, i: usize, value: Value) -> error::Result<()> {
        self.heap.with_obj(idx, |o| {
            if let HeapObj::Array(a) = o {
                let mut items = a.items.lock().unwrap();
                while items.len() <= i {
                    items.push(Value::Undefined);
                }
                items[i] = value;
            }
        });
        Ok(())
    }

    /// ES [[Set]] for `Array.prototype.length`. Validates the value per
    /// `ArraySetLength`: must be a non-negative integer in the 32-bit range,
    /// else a RangeError ("Invalid array length"); then truncate or extend.
    fn set_array_length(&mut self, idx: usize, value: Value) -> error::Result<()> {
        let new_len = match value {
            Value::Number(n) => {
                // Must be a non-negative integer that fits in u32, and equal
                // to its uint32 truncation (i.e. no fractional part).
                if n.is_nan() || n < 0.0 || n.is_infinite() {
                    return Err(Error::range("Invalid array length"));
                }
                if n.fract() != 0.0 {
                    return Err(Error::range("Invalid array length"));
                }
                let as_u32 = n as u32;
                if (as_u32 as f64) != n {
                    return Err(Error::range("Invalid array length"));
                }
                if n >= (1u64 << 32) as f64 {
                    return Err(Error::range("Invalid array length"));
                }
                as_u32 as usize
            }
            _ => {
                // Non-numeric assignment to length: ToUint32 semantics would
                // require conversion; for explicit non-numbers we throw as
                // V8 does for clearly-invalid values like "abc".
                return Err(Error::range("Invalid array length"));
            }
        };
        self.heap.with_obj(idx, |o| {
            if let HeapObj::Array(a) = o {
                let mut items = a.items.lock().unwrap();
                if new_len < items.len() {
                    items.truncate(new_len);
                } else {
                    while items.len() < new_len {
                        items.push(Value::Undefined);
                    }
                }
            }
        });
        Ok(())
    }

    // ---- GC roots ----
    pub fn collect_roots(&self) -> Vec<usize> {
        let mut roots = vec![self.global.0];
        if let Value::Object(idx) = &self.global_this {
            roots.push(idx.0);
        }
        if let Some(Value::Object(idx)) = &self.pending_new_target {
            roots.push(idx.0);
        }
        for v in &self.stack {
            if let Value::Object(idx) = v {
                roots.push(idx.0);
            }
        }
        for f in &self.frames {
            roots.push(f.env.0);
            if let Value::Object(idx) = &f.this_val {
                roots.push(idx.0);
            }
            for l in &f.locals {
                if let Value::Object(idx) = l {
                    roots.push(idx.0);
                }
            }
            // Per-frame generator run-state can hold live heap values
            // (resume value sent via next(obj), and the yielded value before
            // it is moved into the LazyGenerator). Root them so a GC during
            // resume_generator does not collect them.
            if let Value::Object(idx) = &*f.gen_resume_value.lock().unwrap() {
                roots.push(idx.0);
            }
            // gen_yield is Mutex<Option<Value>>; peek without consuming via take+set.
            let y = f.gen_yield.lock().unwrap().take();
            if let Some(Value::Object(idx)) = &y {
                roots.push(idx.0);
            }
            *f.gen_yield.lock().unwrap() = y;
        }
        for proto in [
            &self.object_proto,
            &self.array_proto,
            &self.function_proto,
            &self.string_proto,
            &self.number_proto,
            &self.bigint_proto,
            &self.boolean_proto,
            &self.error_proto,
            &self.symbol_proto,
            &self.promise_proto,
            &self.iterator_proto,
            &self.map_proto,
            &self.set_proto,
            &self.generator_proto,
        ] {
            if let Value::Object(idx) = proto {
                roots.push(idx.0);
            }
        }
        // Pending microtasks hold live heap values (Promise handlers, resolve/
        // reject reasons). Root them so a GC between scheduling and running a
        // microtask does not collect them.
        for mt in &self.microtask_queue {
            match mt {
                Microtask::Then {
                    on_fulfilled,
                    on_rejected,
                    derived,
                    ..
                } => {
                    if let Value::Object(idx) = on_fulfilled {
                        roots.push(idx.0);
                    }
                    if let Value::Object(idx) = on_rejected {
                        roots.push(idx.0);
                    }
                    if let Some(idx) = derived {
                        roots.push(idx.0);
                    }
                }
                Microtask::Resolve { value, .. } => {
                    if let Value::Object(idx) = value {
                        roots.push(idx.0);
                    }
                }
                Microtask::Reject { reason, .. } => {
                    if let Value::Object(idx) = reason {
                        roots.push(idx.0);
                    }
                }
            }
        }
        // Global constants are reachable for the program lifetime.
        for v in &self.global_constants {
            if let Value::Object(idx) = v {
                roots.push(idx.0);
            }
        }
        // Pinned temporary roots (e.g. Promise handlers held across call_function).
        roots.extend_from_slice(&self.gc_pins);
        roots
    }

    pub fn gc(&self) {
        let roots = self.collect_roots();
        self.heap.collect(&roots);
    }

    /// Pin a heap object as a temporary GC root. Returns a guard token to pass
    /// to `unpin` when the value is no longer held in a Rust local.
    pub fn pin(&mut self, v: &Value) -> usize {
        if let Value::Object(idx) = v {
            self.gc_pins.push(idx.0);
            1
        } else {
            0
        }
    }

    /// Release the temporary root pinned at `token`.
    pub fn unpin(&mut self, token: usize) {
        if token != 0 {
            // Swap-remove is unsafe here (would move another live pin's index),
            // so just clear by setting to an invalid/no-op slot. We truncate
            // trailing sentinels lazily; pins are short-lived (single call).
            // Simplest correct approach: only the most-recent pin is popped.
            if token + 1 == self.gc_pins.len() {
                self.gc_pins.pop();
            } else {
                // Overwritten with a stale slot; collect_roots tolerates dupes.
                self.gc_pins[token] = usize::MAX;
            }
        }
    }

    /// Pin multiple values at once; returns the count to unpin later.
    pub fn pin_many(&mut self, vals: &[Value]) -> usize {
        let mut n = 0;
        for v in vals {
            if let Value::Object(idx) = v {
                self.gc_pins.push(idx.0);
                n += 1;
            }
        }
        n
    }

    /// Release `n` most-recently pinned temporary roots.
    pub fn unpin_many(&mut self, n: usize) {
        for _ in 0..n {
            self.gc_pins.pop();
        }
    }

    /// Allocate a plain object and return its handle.
    /// Resolve a promise: set state to Fulfilled and schedule its handlers.
    pub fn promise_resolve(&mut self, promise_idx: usize, value: Value) {
        let handlers: Vec<crate::value::PromiseHandler> = self.heap.with_obj(promise_idx, |o| {
            if let HeapObj::Promise(p) = o {
                if *p.state.lock().unwrap() != PromiseStatus::Pending {
                    return Vec::new();
                }
                *p.state.lock().unwrap() = PromiseStatus::Fulfilled;
                *p.result.lock().unwrap() = value.clone();
                p.handlers.lock().unwrap().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push_back(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: h.on_fulfilled,
                on_rejected: h.on_rejected,
                derived: h.derived,
            });
        }
    }

    /// Reject a promise: set state to Rejected and schedule its handlers.
    pub fn promise_reject(&mut self, promise_idx: usize, reason: Value) {
        let handlers: Vec<crate::value::PromiseHandler> = self.heap.with_obj(promise_idx, |o| {
            if let HeapObj::Promise(p) = o {
                if *p.state.lock().unwrap() != PromiseStatus::Pending {
                    return Vec::new();
                }
                *p.state.lock().unwrap() = PromiseStatus::Rejected;
                *p.result.lock().unwrap() = reason.clone();
                p.handlers.lock().unwrap().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push_back(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: h.on_fulfilled,
                on_rejected: h.on_rejected,
                derived: h.derived,
            });
        }
    }

    /// Drain the microtask queue, running scheduled then/catch callbacks.
    pub fn run_microtasks(&mut self) -> error::Result<()> {
        // Drain in enqueue order (FIFO): Promise microtasks must fire in the
        // order they were scheduled, so pop from the front. (Vec::remove(0) is
        // O(n), but microtask queues are typically small per drain cycle.)
        while let Some(task) = self.microtask_queue.pop_front() {
            match task {
                Microtask::Then {
                    promise,
                    on_fulfilled,
                    on_rejected,
                    derived,
                } => self.run_then(promise, on_fulfilled, on_rejected, derived)?,
                Microtask::Resolve { promise, value } => {
                    self.promise_resolve(promise.0, value);
                }
                Microtask::Reject { promise, reason } => {
                    self.promise_reject(promise.0, reason);
                }
            }
        }
        Ok(())
    }

    /// Run a single then handler for a settled promise, chaining into the
    /// derived promise (if any). The derived promise is stored in the handler
    /// via a side-table keyed by the handler function index.
    fn run_then(
        &mut self,
        promise: GcIdx,
        on_fulfilled: Value,
        on_rejected: Value,
        derived: Option<GcIdx>,
    ) -> error::Result<()> {
        let (state, result) = self.heap.with_obj(promise.0, |o| {
            if let HeapObj::Promise(p) = o {
                (*p.state.lock().unwrap(), p.result.lock().unwrap().clone())
            } else {
                (PromiseStatus::Fulfilled, Value::Undefined)
            }
        });
        let handler = if state == PromiseStatus::Rejected {
            on_rejected
        } else {
            on_fulfilled
        };
        if matches!(handler, Value::Undefined) {
            // pass-through: settle the derived promise with the same outcome
            if let Some(d) = derived {
                if state == PromiseStatus::Rejected {
                    self.promise_reject(d.0, result);
                } else {
                    self.promise_resolve(d.0, result);
                }
            }
            return Ok(());
        }
        // Pin the handler, result, and derived promise as GC roots while the
        // handler call runs: call_function may allocate enough to trigger a GC,
        // which would otherwise collect these values held only in Rust locals.
        let pinned = self.pin_many(&[handler.clone(), result.clone()]);
        if let Some(d) = derived {
            self.gc_pins.push(d.0);
        }
        // call the handler with the result
        let call_ret = self.call_function(&handler, std::slice::from_ref(&result), None);
        // Unpin everything (handler + result + derived) regardless of outcome.
        let mut to_unpin = pinned;
        if derived.is_some() {
            to_unpin += 1;
        }
        self.unpin_many(to_unpin);
        match call_ret {
            Ok(ret) => {
                if let Some(d) = derived {
                    // if the return is itself a promise, adopt its state
                    if let Value::Object(ret_idx) = ret {
                        let is_promise = self
                            .heap
                            .with_obj(ret_idx.0, |o| matches!(o, HeapObj::Promise(_)));
                        if is_promise {
                            // chain: when ret settles, settle derived
                            self.heap.with_obj(ret_idx.0, |o| {
                                if let HeapObj::Promise(p) = o {
                                    p.handlers
                                        .lock()
                                        .unwrap()
                                        .push(crate::value::PromiseHandler {
                                            on_fulfilled: Value::Undefined,
                                            on_rejected: Value::Undefined,
                                            derived: Some(d),
                                        });
                                }
                            });
                            // If `ret` is already settled, the handler we just
                            // registered will never be drained (promise_resolve/
                            // reject only drain handlers while Pending). Run it
                            // now so the derived promise settles.
                            let (settled, state) = self.heap.with_obj(ret_idx.0, |o| {
                                if let HeapObj::Promise(p) = o {
                                    (
                                        *p.state.lock().unwrap() != PromiseStatus::Pending,
                                        *p.state.lock().unwrap(),
                                    )
                                } else {
                                    (false, PromiseStatus::Pending)
                                }
                            });
                            if settled {
                                self.microtask_queue.push_back(Microtask::Then {
                                    promise: ret_idx,
                                    on_fulfilled: Value::Undefined,
                                    on_rejected: Value::Undefined,
                                    derived: Some(d),
                                });
                                let _ = state;
                            }
                            // Do NOT also resolve `derived` now: the adoption
                            // handler registered above settles `derived` when
                            // `ret` settles. Resolving here immediately would
                            // wrap the Promise as `[object Promise]` instead of
                            // adopting its eventual value.
                        } else {
                            self.promise_resolve(d.0, ret);
                        }
                    } else {
                        self.promise_resolve(d.0, ret);
                    }
                }
            }
            Err(e) => {
                if let Some(d) = derived {
                    let reason: Value = e
                        .thrown_value
                        .clone()
                        .unwrap_or_else(|| Value::String(Arc::from(e.message.as_str())));
                    self.promise_reject(d.0, reason);
                }
            }
        }
        Ok(())
    }

    pub fn new_object(&mut self) -> GcIdx {
        let obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(self.object_proto.clone())),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        GcIdx(self.heap.allocate(obj))
    }

    /// Set the wrapped primitive on an object (for `new Number(5)`,
    /// `new Boolean(true)`, `new String("x")`, `Object(1n)`). `valueOf()` and
    /// `ToPrimitive` consult this so `new Number(5) + 1 === 6`.
    pub fn set_primitive(&mut self, obj: &Value, prim: Value) {
        if let Value::Object(idx) = obj {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    *od.primitive.lock().unwrap() = Some(prim);
                }
            });
        }
    }

    /// Allocate a function with native impl.
    pub fn new_native_function(&mut self, name: &str, func: NativeFn, length: usize) -> GcIdx {
        let fdef = crate::value::FunctionData {
            name: Some(Arc::from(name)),
            kind: crate::value::FunctionKind::Native { func, length },
            closure: self.global,
            // Native functions have no `prototype` property (they are not
            // constructors). Their [[Prototype]] (`__proto__`) is
            // `Function.prototype` once it has been allocated.
            prototype: Mutex::new(None),
            proto: Mutex::new(match self.function_proto {
                Value::Object(_) => Some(self.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(IndexMap::new()),
        };
        GcIdx(self.heap.allocate(HeapObj::Function(fdef)))
    }

    /// Define a global binding (visible to JS as a top-level variable).
    /// This is the embedding API for exposing host values to script code.
    pub fn define_global(&mut self, name: &str, value: Value) {
        crate::environment::declare(
            &self.heap,
            self.global,
            name,
            value,
            crate::value::BindingKind::Var,
        );
    }

    /// Get a global binding by name, or `undefined` if not present.
    pub fn get_global(&self, name: &str) -> Value {
        crate::environment::get(&self.heap, self.global, name).unwrap_or(Value::Undefined)
    }

    /// Minimal stub for `Object(value)` coercion.
    pub fn to_object(&mut self, value: &Value) -> error::Result<Value> {
        Ok(match value {
            Value::Object(idx) => Value::Object(*idx),
            _ => {
                let idx = self.new_object();
                self.set_primitive(&Value::Object(idx), value.clone());
                Value::Object(idx)
            }
        })
    }
}

impl Vm {
    /// Call a function value with the given arguments and `this` binding.
    pub fn call_function(
        &mut self,
        func: &Value,
        args: &[Value],
        this: Option<Value>,
    ) -> error::Result<Value> {
        // Pin the callee and args as GC roots for the duration of this call:
        // reading the function kind and building the call frame involve heap
        // allocations that can trigger a GC, which would otherwise collect
        // values held only in the caller's Rust locals / args slice.
        let pin_count = {
            let mut n = self.pin(func);
            for a in args {
                n += self.pin(a);
            }
            n
        };
        let result = self.call_function_inner(func, args, this);
        self.unpin_many(pin_count);
        result
    }

    fn call_function_inner(
        &mut self,
        func: &Value,
        args: &[Value],
        this: Option<Value>,
    ) -> error::Result<Value> {
        // Cap the call-stack depth before pushing another frame. Without this
        // an unbounded JS recursion would overflow the Rust stack (each JS
        // call recurses through `call_function` -> `execute_chunk_func` ->
        // `interpret_to_depth`), killing the process with a hard stack
        // overflow instead of a catchable RangeError. The limit is generous
        // (well below the Rust default 8 MiB stack) and matches the spirit of
        // V8's "Maximum call stack size exceeded".
        const MAX_CALL_STACK_DEPTH: usize = 1000;
        if self.frames.len() >= MAX_CALL_STACK_DEPTH {
            return Err(Error::range("Maximum call stack size exceeded"));
        }
        let idx = match func {
            Value::Object(idx) => *idx,
            _ => {
                return Err(Error::type_err(format!(
                    "{} is not a function",
                    func.type_of()
                )))
            }
        };
        // read function kind without holding borrow
        let kind_info = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                match &f.kind {
                    crate::value::FunctionKind::Native { func, .. } => {
                        Some(FuncCallInfo::Native(*func))
                    }
                    crate::value::FunctionKind::Interpreted { func } => {
                        Some(FuncCallInfo::Interpreted {
                            func: func.clone(),
                            closure: f.closure,
                            is_arrow: func.is_arrow,
                            is_async: func.is_async,
                        })
                    }
                    crate::value::FunctionKind::Bound {
                        target,
                        this_val,
                        bound_args,
                    } => Some(FuncCallInfo::Bound {
                        target: *target,
                        this_val: this_val.clone(),
                        bound_args: bound_args.clone(),
                    }),
                }
            } else {
                None
            }
        });
        match kind_info {
            Some(FuncCallInfo::Native(f)) => f(self, args, this),
            Some(FuncCallInfo::Interpreted {
                func,
                closure,
                is_arrow,
                is_async,
            }) => {
                let call_env = env::new_env(&self.heap, Some(closure), true);
                // Declare every parameter binding as *uninitialized* (TDZ). The raw
                // argument still lives in `locals[i]`, which the compiled
                // parameter prologue reads via `LoadLocal`; the binding is only
                // lifted by `InitLet` once the prologue applies the raw value or
                // the default, left-to-right. This makes
                // `function f(a = b, b = 2)` a ReferenceError: when `a`'s
                // default evaluates, `b` is still in the TDZ.
                for param in func.params.iter() {
                    env::declare_uninit(
                        &self.heap,
                        call_env,
                        param,
                        crate::value::BindingKind::Let,
                    );
                }
                // rest parameter: collect remaining args into an array.
                if let Some(rest_name) = &func.rest_param {
                    let rest: Vec<Value> = if func.params.len() <= args.len() {
                        args[func.params.len()..].to_vec()
                    } else {
                        Vec::new()
                    };
                    let arr = HeapObj::Array(crate::value::ArrayData {
                        items: Mutex::new(rest),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(self.array_proto.clone())),
                    });
                    env::declare(
                        &self.heap,
                        call_env,
                        rest_name,
                        Value::Object(GcIdx(self.heap.allocate(arr))),
                        crate::value::BindingKind::Const,
                    );
                }
                // make the function visible to itself by its name (for recursion)
                if let Some(name) = &func.name {
                    env::declare(
                        &self.heap,
                        call_env,
                        name,
                        Value::Object(idx),
                        crate::value::BindingKind::Const,
                    );
                }
                let arr = HeapObj::Array(crate::value::ArrayData {
                    items: Mutex::new(args.to_vec()),
                    props: Mutex::new(IndexMap::new()),
                    proto: Mutex::new(Some(self.array_proto.clone())),
                });
                env::declare(
                    &self.heap,
                    call_env,
                    "arguments",
                    Value::Object(GcIdx(self.heap.allocate(arr))),
                    crate::value::BindingKind::Const,
                );
                // In sloppy (non-strict) mode, an unbound `this` (plain
                // function call with no receiver) defaults to the global
                // object. In strict mode it stays `undefined`. Arrow functions
                // ignore `this` entirely (lexical capture).
                let this_val = if is_arrow {
                    this.unwrap_or(Value::Undefined)
                } else {
                    let raw = this.unwrap_or(Value::Undefined);
                    if raw.is_undefined() && !func.chunk.is_strict {
                        self.global_this.clone()
                    } else {
                        raw
                    }
                };
                // Arrow functions capture `this` lexically from their
                // enclosing scope, so they must NOT redeclare `this` in
                // their own call environment (which would shadow the
                // captured binding). Non-arrow functions bind `this` to the
                // caller-supplied value (or `undefined`).
                if !is_arrow {
                    env::declare(
                        &self.heap,
                        call_env,
                        "this",
                        this_val.clone(),
                        crate::value::BindingKind::Const,
                    );
                }
                let _ = &this_val;
                let is_gen = func.is_generator;
                if is_gen {
                    // Lazy generator: don't run the body yet. Create a suspended
                    // generator object; the body runs incrementally via next().
                    let g_idx = self.heap.allocate(HeapObj::LazyGenerator(
                        crate::value::LazyGeneratorData {
                            fdef: func.clone(),
                            closure: call_env,
                            env: Mutex::new(call_env),
                            this_val: Mutex::new(this_val.clone()),
                            args: Mutex::new(args.to_vec()),
                            ip: AtomicUsize::new(0),
                            stack: Mutex::new(Vec::new()),
                            locals: Mutex::new(Vec::new()),
                            catch_stack: Mutex::new(Vec::new()),
                            started: AtomicBool::new(false),
                            done: AtomicBool::new(false),
                            resume_value: Mutex::new(Value::Undefined),
                            is_async,
                            props: Mutex::new(IndexMap::new()),
                            proto: Mutex::new(Some(self.generator_proto.clone())),
                        },
                    ));
                    Ok(Value::Object(GcIdx(g_idx)))
                } else {
                    // execute the compiled function chunk
                    let result = self.execute_chunk_func(func.clone(), call_env, this_val, args);
                    if is_async {
                        // async functions return a Promise. An uncaught throw
                        // inside the async body must settle the returned
                        // Promise to Rejected (with the thrown value as the
                        // reason) rather than propagating as a hard error.
                        match result {
                            Ok(value) => {
                                let p_idx = self.heap.allocate(HeapObj::Promise(
                                    crate::value::PromiseData {
                                        state: Mutex::new(PromiseStatus::Fulfilled),
                                        result: Mutex::new(value),
                                        handlers: Mutex::new(Vec::new()),
                                        props: Mutex::new(IndexMap::new()),
                                        proto: Mutex::new(Some(self.promise_proto.clone())),
                                    },
                                ));
                                Ok(Value::Object(GcIdx(p_idx)))
                            }
                            Err(err) => {
                                // Extract the thrown JS value if present;
                                // otherwise stringify the engine error.
                                let reason = match &err.thrown_value {
                                    Some(v) => v.clone(),
                                    None => Value::String(Arc::from(err.to_string().as_str())),
                                };
                                let p_idx = self.heap.allocate(HeapObj::Promise(
                                    crate::value::PromiseData {
                                        state: Mutex::new(PromiseStatus::Rejected),
                                        result: Mutex::new(reason),
                                        handlers: Mutex::new(Vec::new()),
                                        props: Mutex::new(IndexMap::new()),
                                        proto: Mutex::new(Some(self.promise_proto.clone())),
                                    },
                                ));
                                Ok(Value::Object(GcIdx(p_idx)))
                            }
                        }
                    } else {
                        result
                    }
                }
            }
            Some(FuncCallInfo::Bound {
                target,
                this_val,
                bound_args,
            }) => {
                let mut all = bound_args;
                all.extend_from_slice(args);
                self.call_function(&Value::Object(target), &all, Some(this_val))
            }
            None => Err(Error::type_err("not a function".to_string())),
        }
    }

    pub fn construct(&mut self, constructor: &Value, args: &[Value]) -> error::Result<Value> {
        let idx = match constructor {
            Value::Object(idx) => *idx,
            _ => return Err(Error::type_err("not a constructor".to_string())),
        };
        let proto = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                f.prototype
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or(self.object_proto.clone())
            } else {
                self.object_proto.clone()
            }
        });
        let new_obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        let this_obj = Value::Object(GcIdx(self.heap.allocate(new_obj)));
        self.pending_new_target = Some(constructor.clone());
        let result = self.call_function(constructor, args, Some(this_obj.clone()))?;
        if matches!(result, Value::Object(_)) {
            Ok(result)
        } else {
            Ok(this_obj)
        }
    }

    // ---- iteration ----

    /// Build a heap iterator object that yields the values of `iterable`.
    pub fn make_iterator(&mut self, iterable: &Value) -> error::Result<Value> {
        // Built-in iterables (Array/String/Map/Set/Generator) have fast paths
        // below. Only plain objects honor a user-defined `Symbol.iterator`
        // method, to avoid calling Map/Set's iterator method and getting a
        // non-iterator array back.
        // Built-in iterables (String/Map/Set/Generator) always use their fast
        // paths. Arrays use the fast path UNLESS an own (overriding)
        // Symbol.iterator was installed on the array or its prototype chain;
        // in that case honor the custom iterator method. Plain objects honor
        // a user-defined Symbol.iterator.
        let (is_map, is_set, is_gen, is_arr) = match iterable {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                (
                    matches!(o, HeapObj::Map(_)),
                    matches!(o, HeapObj::Set(_)),
                    matches!(o, HeapObj::Generator(_) | HeapObj::LazyGenerator(_)),
                    matches!(o, HeapObj::Array(_)),
                )
            }),
            _ => (false, false, false, false),
        };
        let is_builtin_iterable =
            matches!(iterable, Value::String(_)) || is_map || is_set || is_gen;
        // For arrays, honor an overriding Symbol.iterator only if it is an own
        // property of this array (not the inherited default, which RuJa does
        // not install, so the fast path applies).
        let arr_has_inherited_iterator = is_arr && {
            let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
            self.has_property_key(iterable, &sym_key)
        };
        if !is_builtin_iterable || arr_has_inherited_iterator {
            if let Value::Object(_) = iterable {
                let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
                if self.has_property_key(iterable, &sym_key) {
                    let iter_method = self.get_property_by_key(iterable, &sym_key)?;
                    let iter_obj = self.call_function(&iter_method, &[], Some(iterable.clone()))?;
                    return Ok(self.new_lazy_iterator(iter_obj));
                }
            }
        }
        let items: Vec<Value> = match iterable {
            Value::String(s) => crate::value::utf16_from_str(s)
                .into_iter()
                .map(|unit| Value::String(Arc::from(String::from_utf16_lossy(&[unit]).as_str())))
                .collect(),
            Value::Object(idx) => {
                let (is_array, is_map, is_set, is_generator) = self.heap.with_obj(idx.0, |o| {
                    (
                        matches!(o, HeapObj::Array(_)),
                        matches!(o, HeapObj::Map(_)),
                        matches!(o, HeapObj::Set(_)),
                        matches!(o, HeapObj::Generator(_) | HeapObj::LazyGenerator(_)),
                    )
                });
                if is_generator {
                    // Wrap the generator in a lazy iterator that resumes it per
                    // pull. This preserves the generator's return value (needed
                    // by `yield*`) and avoids eagerly draining infinite
                    // generators before the loop even starts.
                    return Ok(self.new_generator_iterator(iterable.clone()));
                } else if is_array {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            a.items.lock().unwrap().clone()
                        } else {
                            Vec::new()
                        }
                    })
                } else if is_map {
                    // Extract (k, v) pairs out of the borrow first; allocate the
                    // pair arrays afterwards so we never call heap.allocate while
                    // with_obj holds an immutable borrow of the heap cells (which
                    // would panic on RefCell reborrow).
                    let pairs: Vec<(Value, Value)> = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Map(m) = o {
                            m.entries.lock().unwrap().iter().cloned().collect()
                        } else {
                            Vec::new()
                        }
                    });
                    let array_proto = self.array_proto.clone();
                    pairs
                        .into_iter()
                        .map(|(k, v)| {
                            let pair = HeapObj::Array(crate::value::ArrayData {
                                items: Mutex::new(vec![k, v]),
                                props: Mutex::new(IndexMap::new()),
                                proto: Mutex::new(Some(array_proto.clone())),
                            });
                            Value::Object(GcIdx(self.heap.allocate(pair)))
                        })
                        .collect::<Vec<_>>()
                } else if is_set {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Set(s) = o {
                            s.items.lock().unwrap().clone()
                        } else {
                            Vec::new()
                        }
                    })
                } else {
                    return Err(Error::type_err("value is not iterable".to_string()));
                }
            }
            _ => {
                return Err(Error::type_err(format!(
                    "{} is not iterable",
                    iterable.type_of()
                )))
            }
        };
        Ok(self.new_iterator(items))
    }

    /// Obtain an async iterator for `for await...of`. Prefers a user-defined
    /// `Symbol.asyncIterator` method; falls back to the sync iterator protocol
    /// (`Symbol.iterator`) as an async-from-sync iterator. Async generators are
    /// wrapped directly (their `next()` already returns a Promise).
    pub fn make_async_iterator(&mut self, iterable: &Value) -> error::Result<Value> {
        if let Value::Object(_) = iterable {
            let akey = crate::value::PropertyKey::Symbol(self.well_known_symbols.async_iterator);
            if self.has_property_key(iterable, &akey) {
                let m = self.get_property_by_key(iterable, &akey)?;
                let iter_obj = self.call_function(&m, &[], Some(iterable.clone()))?;
                return Ok(self.new_lazy_iterator(iter_obj));
            }
            // No async iterator: fall back to the sync iterator protocol. Each
            // `next()` is awaited (a non-Promise value awaits to itself).
            let it = self.make_iterator(iterable)?;
            return Ok(it);
        }
        // Primitives (strings etc.): use the sync iterator, awaited per step.
        let it = self.make_iterator(iterable)?;
        Ok(it)
    }

    /// Build an iterator over an object's enumerable string keys (for `for...in`).
    pub fn make_for_in_keys(&mut self, obj: &Value) -> error::Result<Value> {
        let mut keys: Vec<Value> = Vec::new();
        let mut cur = obj.clone();
        while let Value::Object(idx) = &cur {
            let (own, proto) = self.heap.with_obj(idx.0, |o| {
                let mut own = Vec::new();
                if let HeapObj::Array(a) = o {
                    for i in 0..a.items.lock().unwrap().len() {
                        own.push(Value::String(Arc::from(i.to_string().as_str())));
                    }
                }
                if let HeapObj::Map(m) = o {
                    for (k, _) in m.entries.lock().unwrap().iter() {
                        if let Value::String(s) = k {
                            own.push(Value::String(s.clone()));
                        }
                    }
                }
                for (k, desc) in o.props().lock().unwrap().iter() {
                    if desc.enumerable {
                        if let crate::value::PropertyKey::Str(s) = k {
                            own.push(Value::String(s.clone()));
                        }
                    }
                }
                (own, o.proto().lock().unwrap().clone())
            });
            for k in own {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        Ok(self.new_iterator(keys))
    }

    fn new_iterator(&mut self, items: Vec<Value>) -> Value {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(items),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(None),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Value::Object(GcIdx(self.heap.allocate(it)))
    }

    /// Build a *lazy* iterator wrapping a JS iterator object (one returned by a
    /// user-defined `Symbol.iterator` method). Each `next()` call invokes the
    /// JS object's `next()` method and reads its `value`/`done` properties.
    fn new_lazy_iterator(&mut self, iter_obj: Value) -> Value {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(Some(iter_obj)),
            generator: Mutex::new(None),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Value::Object(GcIdx(self.heap.allocate(it)))
    }

    /// Build a lazy iterator wrapping a generator object. Each `next()` resumes
    /// the generator via `resume_generator`, preserving its return value (so
    /// `yield* gen()` yields the generator's return value as the result).
    fn new_generator_iterator(&mut self, gen: Value) -> Value {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(Some(gen)),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Value::Object(GcIdx(self.heap.allocate(it)))
    }

    pub fn iterator_next(&mut self, it: &Value) -> error::Result<(Value, bool)> {
        self.iterator_next_resume(it, Value::Undefined)
    }

    /// Like [`iterator_next`] but passes `resume` to a lazy iterator's JS
    /// `next()` method (used by `yield*` to forward the outer resume value to
    /// the delegated iterator). Eager (Vec-backed) iterators ignore `resume`.
    pub fn iterator_next_resume(
        &mut self,
        it: &Value,
        resume: Value,
    ) -> error::Result<(Value, bool)> {
        let (lazy, is_gen, already_done) = match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    (
                        it.lazy_iter.lock().unwrap().is_some(),
                        it.generator.lock().unwrap().is_some(),
                        it.done.load(Ordering::Relaxed),
                    )
                } else {
                    (false, false, true)
                }
            }),
            _ => return Err(Error::type_err("not an iterator".to_string())),
        };
        if already_done {
            return Ok((Value::Undefined, true));
        }
        if is_gen {
            // Resume the wrapped generator with `resume`. The generator's
            // return value (when done) is preserved as the iterator value.
            let gen = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        it.generator.lock().unwrap().clone()
                    } else {
                        None
                    }
                },
            );
            let gen = gen.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let g_idx = match &gen {
                Value::Object(idx) => *idx,
                _ => return Err(Error::type_err("not a generator".to_string())),
            };
            let (value, done) = self.resume_generator(g_idx, ResumeKind::Next(resume))?;
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
            }
            return Ok((value, done));
        }
        if lazy {
            // Call the JS iterator object's next() method and read {value, done}.
            let iter_obj = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        it.lazy_iter.lock().unwrap().clone()
                    } else {
                        None
                    }
                },
            );
            let iter_obj =
                iter_obj.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let next_fn = self.get_property(&iter_obj, "next")?;
            let result = self.call_function(&next_fn, &[resume], Some(iter_obj))?;
            let value = self.get_property(&result, "value")?;
            let done = match self.get_property(&result, "done")? {
                Value::Bool(b) => b,
                _ => false,
            };
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
            }
            Ok((value, done))
        } else {
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            self.heap.with_obj(idx, |o| {
                if let HeapObj::Iterator(it) = o {
                    let items = it.items.lock().unwrap();
                    let i = it.index.load(Ordering::Relaxed);
                    if i < items.len() {
                        let v = items[i].clone();
                        it.index.store(i + 1, Ordering::Relaxed);
                        Ok((v, false))
                    } else {
                        Ok((Value::Undefined, true))
                    }
                } else {
                    Err(Error::type_err("not an iterator".to_string()))
                }
            })
        }
    }

    /// `for await` step: call the async iterator's `next()` (or the sync
    /// iterator's `next()`), await the returned Promise, and return
    /// `(value, done)`. Eager (Vec-backed) iterators are stepped directly.
    pub fn iterator_next_await(&mut self, it: &Value) -> error::Result<(Value, bool)> {
        // Generator-backed lazy iterators and custom async iterators both go
        // through the JS `next()` method; awaiting its result (a Promise for
        // async iterators, a plain object for sync ones) yields {value, done}.
        let lazy_or_gen = if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(i) = o {
                    i.lazy_iter.lock().unwrap().is_some() || i.generator.lock().unwrap().is_some()
                } else {
                    false
                }
            })
        } else {
            false
        };
        if lazy_or_gen {
            // Resolve the iterator object whose `next()` we call: either the
            // wrapped JS async iterator or the generator itself.
            let iter_obj = if let Value::Object(idx) = it {
                self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Iterator(i) = o {
                        i.lazy_iter
                            .lock()
                            .unwrap()
                            .clone()
                            .or_else(|| i.generator.lock().unwrap().clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            let iter_obj =
                iter_obj.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let next_fn = self.get_property(&iter_obj, "next")?;
            let result = self.call_function(&next_fn, &[], Some(iter_obj))?;
            // Await: if it's a Promise, drain microtasks and read the settled value.
            let result = self.await_value(result)?;
            let value = self.get_property(&result, "value")?;
            let done = match self.get_property(&result, "done")? {
                Value::Bool(b) => b,
                _ => false,
            };
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(i) = o {
                            i.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
            }
            return Ok((value, done));
        }
        // Eager (Vec-backed) iterator: step directly, no awaiting needed.
        self.iterator_next(it)
    }

    /// Await a value: if it is a pending Promise, drain microtasks until it
    /// settles and return the settled value (rejecting on rejection); otherwise
    /// return the value as-is.
    fn await_value(&mut self, v: Value) -> error::Result<Value> {
        if let Value::Object(idx) = &v {
            let is_promise = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
            if is_promise {
                self.run_microtasks()?;
                let (state, result) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Promise(p) = o {
                        (*p.state.lock().unwrap(), p.result.lock().unwrap().clone())
                    } else {
                        (PromiseStatus::Fulfilled, Value::Undefined)
                    }
                });
                if state == PromiseStatus::Rejected {
                    return Err(Error::thrown(result, &self.heap));
                }
                return Ok(result);
            }
        }
        Ok(v)
    }

    pub fn iterator_done(&self, it: &Value) -> bool {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    return it.index.load(Ordering::Relaxed) >= it.items.lock().unwrap().len();
                }
                false
            })
        } else {
            false
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
