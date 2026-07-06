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
use num_traits::Zero;
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
    /// Native functions sometimes need the active callee object, for example
    /// Error subclass constructors called without `new`.
    pub(crate) current_native_callee: Option<Value>,
    /// Native constructors need the active `new.target` for
    /// OrdinaryCreateFromConstructor-style allocation.
    pub(crate) current_native_new_target: Option<Value>,
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
    pub(crate) generator_function_proto: Value,
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
    /// Tagged-template object cache keyed by (chunk ptr, ip). Per spec the
    /// same template-literal site returns the same frozen template object.
    pub(crate) template_cache: std::collections::HashMap<(usize, usize), Value>,
}

pub struct WellKnownSymbols {
    pub iterator: u32,
    pub to_primitive: u32,
    pub has_instance: u32,
    pub to_string_tag: u32,
    pub async_iterator: u32,
    pub r#match: u32,
}

pub struct CallFrame {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
    pub stack_base: usize,
    pub locals: Vec<Value>,
    pub env: GcIdx,
    pub catch_stack: Vec<(usize, u32, GcIdx)>,
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
    /// True while executing non-strict eval code whose variable environment is
    /// the global environment. Global var/function bindings created from this
    /// frame use EvalDeclarationInstantiation's configurable=true argument.
    pub eval_global_bindings: bool,
    /// True when this frame is a derived class constructor (extends ...).
    /// In derived constructors, returning a non-object value after super()
    /// is a TypeError.
    pub is_derived_ctor: bool,
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
            eval_global_bindings: false,
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

    fn append_compiled_functions(
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
            current_native_callee: None,
            current_native_new_target: None,
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
            generator_function_proto: Value::Undefined,
            map_proto: Value::Undefined,
            set_proto: Value::Undefined,
            date_proto: Value::Undefined,
            microtask_queue: std::collections::VecDeque::new(),
            ic: std::collections::HashMap::new(),
            gc_pins: Vec::new(),
            current_yields: Vec::new(),
            next_symbol_id: 7,
            well_known_symbols: WellKnownSymbols {
                iterator: 1,
                to_primitive: 2,
                has_instance: 3,
                to_string_tag: 4,
                async_iterator: 5,
                r#match: 6,
            },
            global_names: HashMap::new(),
            global_constants: Vec::new(),
            functions: Vec::new(),
            fuel: None,
            max_heap_objects: 0,
            template_cache: std::collections::HashMap::new(),
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
        self.check_global_declaration_instantiation(&program, self.global, &self.global_this)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions(chunk, funcs);
        // Script top-level `this` is the global object even for strict scripts.
        crate::environment::declare(
            &self.heap,
            self.global,
            "this",
            self.global_this.clone(),
            crate::value::BindingKind::Const,
        );
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

    /// Like execute_chunk but guarantees the pushed frame is popped on return,
    /// so eval (which reuses the VM afterwards) leaves the caller's frame stack
    /// intact. Used by eval paths only.
    fn execute_chunk_scoped(
        &mut self,
        chunk: Chunk,
        env: GcIdx,
        this_val: Value,
        eval_global_bindings: bool,
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
        let chunk = self.append_compiled_functions(chunk, funcs);
        crate::environment::declare(
            &self.heap,
            self.global,
            "this",
            self.global_this.clone(),
            crate::value::BindingKind::Const,
        );
        let result = self.execute_chunk_scoped(chunk, self.global, self.global_this.clone(), false);
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
        if !is_strict && global_env == self.global {
            self.check_eval_global_declaration_instantiation(&program, global_env, &global_this)?;
        }
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions(chunk, funcs);
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
            global_this,
            !is_strict && global_env == self.global,
        );
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
    pub fn eval_direct(
        &mut self,
        src: &str,
        caller_env: GcIdx,
        this_val: Value,
        caller_strict: bool,
    ) -> error::Result<Value> {
        let super_allowed = crate::environment::has(&self.heap, caller_env, "#super");
        let super_call_allowed = crate::environment::has(&self.heap, caller_env, "#superctor");
        let program = crate::parser::Parser::parse_direct_eval_inherited(
            src,
            caller_strict,
            super_allowed,
            super_call_allowed,
        )?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let chunk = self.append_compiled_functions(chunk, funcs);
        // Per spec, direct eval runs in a dedicated lexical environment whose
        // parent is the caller's environment. `let`/`const`/`class` declared in
        // eval stay local to that environment (they do NOT leak to the caller),
        // while `var` and function declarations leak to the caller's function
        // scope — UNLESS the eval code is strict, in which case nothing leaks
        // (the eval has its own scope and all bindings stay local). Pre-declare
        // the var/function names in the caller's variable environment (sloppy
        // only) so later name resolution can see the hoisted bindings; then run
        // the eval body in the child environment.
        let is_strict = caller_strict || program.is_strict;
        let var_names = if is_strict {
            Vec::new()
        } else {
            crate::compiler::Compiler::collect_var_names(&program.body)
        };
        let (_, _, function_names) = crate::compiler::Compiler::collect_global_declaration_names(
            &program.body,
            program.is_strict,
        );
        let var_env = crate::environment::function_scope_root(&self.heap, caller_env);
        if !is_strict && var_env == self.global {
            self.check_eval_global_declaration_instantiation(&program, var_env, &self.global_this)?;
        }
        if !is_strict {
            for name in &var_names {
                if crate::environment::has_lexical_declaration_between(
                    &self.heap, caller_env, var_env, name,
                ) {
                    return Err(Error::syntax(format!(
                        "Identifier '{}' has already been declared",
                        name
                    )));
                }
                crate::environment::declare_var(&self.heap, var_env, name, Value::Undefined);
            }
        }
        let eval_env = crate::environment::new_env(&self.heap, Some(caller_env), true)?;
        let result = self.execute_chunk_scoped(chunk, eval_env, this_val, false);
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
                &self.heap, caller_env, var_env, &name,
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
        self.frames.push(CallFrame::new(
            fdef.chunk.clone(),
            0,
            self.stack.len(),
            locals,
            env,
            this_val,
        ));
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
            0,
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
            class_name: Some(Arc::from("Error")),
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
                            // Synthesize an Error object for native errors.
                            self.make_error_value(&e)?
                        }
                    };

                    let divert_to_finally = self.frames.last().is_some_and(|frame| {
                        match (frame.finally_stack.last(), frame.catch_stack.last()) {
                            (Some(&(_, _)), None) => true,
                            (Some(&(_, fseq)), Some(&(_, cseq, _))) => fseq > cseq,
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
                        .and_then(|f| f.catch_stack.last().map(|(ip, _, _)| *ip));
                    if let Some(handler) = handler {
                        // Pop the handler so we don't loop, push the thrown value
                        // for the catch binding, and jump to the handler ip.
                        let (_, _, saved_env) =
                            self.current_frame_mut()?.catch_stack.pop().unwrap();
                        {
                            let frame = self.current_frame_mut()?;
                            frame.finally_completion_tag.store(0, Ordering::Relaxed);
                            *frame.finally_completion_val.lock() = Value::Undefined;
                        }
                        // Unwind scopes opened in the try body.
                        self.current_frame_mut()?.env = saved_env;
                        self.stack.push(thrown);
                        self.current_frame_mut()?.ip = handler;
                        continue;
                    }

                    return Err(e);
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
                        // Walk the environment chain in order: at each env,
                        // check var/let/const bindings first, then the
                        // with-object (if any). This ensures a var binding in
                        // a child scope shadows a with-object property on a
                        // parent scope, and a with-object property shadows an
                        // outer var binding.
                        let mut cur_env = Some(*env_idx);
                        while let Some(e_idx) = cur_env {
                            let (binding_val, in_tdz, has_with, with_obj_val, parent) =
                                self.heap.with_obj(e_idx.0, |obj| {
                                    if let HeapObj::Environment(e) = obj {
                                        if let Some(b) = e.vars.lock().get(name.as_str()) {
                                            if !b.initialized.load(Ordering::Relaxed) {
                                                return (None, true, false, None, None);
                                            }
                                            return (
                                                Some(b.value.lock().clone()),
                                                false,
                                                false,
                                                None,
                                                None,
                                            );
                                        }
                                        if let Some(with_obj) = e.with_object.lock().clone() {
                                            return (
                                                None,
                                                false,
                                                true,
                                                Some(with_obj),
                                                *e.parent.lock(),
                                            );
                                        }
                                        return (None, false, false, None, *e.parent.lock());
                                    }
                                    (None, false, false, None, None)
                                });
                            if in_tdz {
                                return Err(Error::reference(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )));
                            }
                            if let Some(v) = binding_val {
                                return Ok(v);
                            }
                            if has_with {
                                if let Some(with_obj) = with_obj_val {
                                    if self.has_own_property(&with_obj, &name) {
                                        return self.get_property(&with_obj, &name);
                                    }
                                }
                            }
                            cur_env = parent;
                        }
                        // Last resort: check global (this).
                        let global_this = self.global_this.clone();
                        let has = self.has_property(&global_this, &name)?;
                        if has {
                            self.get_property(&global_this, &name)
                        } else {
                            Err(Error::reference(format!("{} is not defined", name)))
                        }
                    }
                    crate::value::ReferenceBase::Value(base) => match &r.name {
                        crate::value::PropertyKey::Str(s) => self.get_property(base, s),
                        crate::value::PropertyKey::Symbol(id) => {
                            self.get_property(base, &format!("[Symbol {}]", id))
                        }
                    },
                    crate::value::ReferenceBase::ObjectEnvironment(base) => match &r.name {
                        crate::value::PropertyKey::Str(s) => self.get_property(base, s),
                        crate::value::PropertyKey::Symbol(id) => {
                            self.get_property(base, &format!("[Symbol {}]", id))
                        }
                    },
                }
            }
            _ => Ok(v.clone()),
        }
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
                        let global_this = self.global_this.clone();
                        self.set_property(&global_this, &name, value)?;
                    }
                    crate::value::ReferenceBase::Environment(env_idx) => {
                        let name = r.name.as_str().map(|s| s.to_string()).unwrap_or_default();
                        // Walk the env chain from the reference's env, checking
                        // at each level: (1) var/let/const binding, (2) with-object
                        // property. This matches the spec's SetMutableBinding
                        // semantics where the reference's base env is used, not
                        // the current env. If a with-object property was deleted
                        // between GetValue and PutValue, we recreate it on the
                        // closest with-object (non-strict) or throw (strict).
                        let global_readonly = self.global_property_is_non_writable_data(&name);
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
                                        if e_idx == self.global && global_readonly {
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
                                        if b.kind == crate::value::BindingKind::Const {
                                            return (
                                                false, true, false, false, false, false, false,
                                                None, None,
                                            );
                                        }
                                        let is_global_var = e_idx == self.global
                                            && b.kind == crate::value::BindingKind::Var;
                                        *b.value.lock() = value.clone();
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
                                let global_this = self.global_this.clone();
                                self.set_property(&global_this, &name, value)?;
                                return Ok(());
                            }
                            if has_binding {
                                if global_var_binding {
                                    self.set_global_var_property(&name, value.clone());
                                }
                                return Ok(());
                            }
                            if has_with {
                                if let Some(with_obj) = with_obj_val {
                                    if self.has_own_property(&with_obj, &name) {
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
                        let global_this = self.global_this.clone();
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
                        let global_this = self.global_this.clone();
                        self.set_property(&global_this, &name, value)?;
                    }
                    crate::value::ReferenceBase::Value(base) => match &r.name {
                        crate::value::PropertyKey::Str(s) => self.set_property(base, s, value)?,
                        crate::value::PropertyKey::Symbol(id) => {
                            let key = crate::value::PropertyKey::Symbol(*id);
                            self.set_property_key(base, &Value::Symbol(*id), value)?
                        }
                    },
                    crate::value::ReferenceBase::ObjectEnvironment(base) => match &r.name {
                        crate::value::PropertyKey::Str(s) => {
                            if !self.has_property(base, s)? {
                                if r.strict {
                                    return Err(Error::reference(format!("{} is not defined", s)));
                                }
                            }
                            self.set_object_environment_property(base, s, value)?
                        }
                        crate::value::PropertyKey::Symbol(id) => {
                            self.set_property_key(base, &Value::Symbol(*id), value)?
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
