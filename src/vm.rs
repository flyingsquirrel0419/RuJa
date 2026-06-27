//! Stack-based bytecode VM.

use crate::bytecode::{Chunk, Op};
use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{GcIdx, HeapObj, PromiseStatus, Value};
use indexmap::IndexMap;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub type NativeFn = fn(&mut Vm, &[Value], Option<Value>) -> error::Result<Value>;

pub struct Vm {
    pub heap: Heap,
    pub global: GcIdx,
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub object_proto: Value,
    pub array_proto: Value,
    pub function_proto: Value,
    pub string_proto: Value,
    pub number_proto: Value,
    pub boolean_proto: Value,
    pub error_proto: Value,
    pub symbol_proto: Value,
    pub promise_proto: Value,
    pub iterator_proto: Value,
    pub generator_proto: Value,
    pub map_proto: Value,
    pub set_proto: Value,
    pub microtask_queue: Vec<Microtask>,
    /// Collected yield values while running a generator function body (eager,
    /// legacy fallback path). Lazy generators use per-frame gen-state instead.
    pub current_yields: Vec<Value>,
    pub next_symbol_id: u32,
    pub well_known_symbols: WellKnownSymbols,
    pub global_names: HashMap<Rc<str>, usize>,
    pub global_constants: Vec<Value>,
    pub functions: Vec<Rc<crate::function::FunctionDef>>,
}

pub struct WellKnownSymbols {
    pub iterator: u32,
    pub to_primitive: u32,
    pub has_instance: u32,
    pub to_string_tag: u32,
    pub async_iterator: u32,
}

pub struct CallFrame {
    pub chunk: Rc<Chunk>,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub env: GcIdx,
    pub catch_stack: Vec<usize>,
    pub this_val: Value,
    /// Per-frame generator run-state. Non-zero only on a generator's own frame,
    /// so a generator body that calls `next()` on *another* generator is fully
    /// isolated (each has its own frame with its own gen-state).
    pub gen_mode: Cell<bool>,
    pub gen_yield: Cell<Option<Value>>,
    pub gen_suspended: Cell<bool>,
    pub gen_resume_value: RefCell<Value>,
}

impl CallFrame {
    fn new(chunk: Rc<Chunk>, ip: usize, locals: Vec<Value>, env: GcIdx, this_val: Value) -> Self {
        CallFrame {
            chunk,
            ip,
            locals,
            env,
            catch_stack: Vec::new(),
            this_val,
            gen_mode: Cell::new(false),
            gen_yield: Cell::new(None),
            gen_suspended: Cell::new(false),
            gen_resume_value: RefCell::new(Value::Undefined),
        }
    }
}

/// Outcome of executing a single bytecode instruction.
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
            stack: Vec::new(),
            frames: Vec::new(),
            object_proto: Value::Undefined,
            array_proto: Value::Undefined,
            function_proto: Value::Undefined,
            string_proto: Value::Undefined,
            number_proto: Value::Undefined,
            boolean_proto: Value::Undefined,
            error_proto: Value::Undefined,
            symbol_proto: Value::Undefined,
            promise_proto: Value::Undefined,
            iterator_proto: Value::Undefined,
            generator_proto: Value::Undefined,
            map_proto: Value::Undefined,
            set_proto: Value::Undefined,
            microtask_queue: Vec::new(),
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
        };
        crate::builtins::setup_full(&mut vm);
        vm
    }

    /// Run a source string and return the value of the last top-level expression.
    pub fn run(&mut self, src: &str) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        let _base = self.functions.len();
        self.functions.extend(funcs);
        let result = self.execute_chunk(chunk, self.global, Value::Undefined);
        // Drain microtasks (Promise callbacks) after the synchronous run.
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    fn execute_chunk(&mut self, chunk: Chunk, env: GcIdx, this_val: Value) -> error::Result<Value> {
        let chunk = Rc::new(chunk);
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
        let chunk = Rc::new(chunk);
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
                .map(|f| Rc::ptr_eq(&f.chunk, &chunk))
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
    ) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        let mut compiler = crate::compiler::Compiler::new();
        let (chunk, funcs) = compiler.compile_program(&program)?;
        self.functions.extend(funcs);
        // Run directly in the caller's environment so `var`/function
        // declarations and name lookups behave as sloppy-mode direct eval
        // (declarations leak into the caller's scope). This is a simplification
        // of the spec's separate var-environment vs lexical-environment model.
        let result = self.execute_chunk_scoped(chunk, caller_env, this_val);
        if !self.microtask_queue.is_empty() {
            self.run_microtasks()?;
        }
        result
    }

    /// Execute a compiled function's chunk in a new frame.
    fn execute_chunk_func(
        &mut self,
        fdef: Rc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
    ) -> error::Result<Value> {
        let mut locals = vec![Value::Undefined; fdef.num_locals.max(256)];
        for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
            if i < locals.len() {
                locals[i] = a.clone();
            }
        }
        self.frames
            .push(CallFrame::new(fdef.chunk.clone(), 0, locals, env, this_val));
        // Run only this function's frame. interpret returns when its frame pops.
        let target_depth = self.frames.len() - 1;
        let result = self.interpret_to_depth(target_depth);
        // On error, the function frame is still on the stack; pop it so the
        // caller's catch handler can be found by the enclosing interpret_catch.
        if result.is_err() {
            self.frames.pop();
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
        resume_val: Value,
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
                    *g.env.borrow(),
                    g.this_val.borrow().clone(),
                    g.args.borrow().clone(),
                    g.ip.get(),
                    g.locals.borrow().clone(),
                    g.stack.borrow().clone(),
                    g.catch_stack.borrow().clone(),
                    g.started.get(),
                    g.done.get(),
                )
            } else {
                panic!("resume_generator on non-lazy-generator");
            }
        });

        if done {
            return Ok((Value::Undefined, true));
        }

        // On the first resume, initialize the locals table with the arguments.
        if !started {
            locals = vec![Value::Undefined; fdef.num_locals.max(256)];
            for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
                if i < locals.len() {
                    locals[i] = a.clone();
                }
            }
            ip = 0;
            stack.clear();
            catch_stack.clear();
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
            *frame.gen_resume_value.borrow_mut() = resume_val.clone();
            frame.gen_mode.set(true);
            frame.gen_suspended.set(false);
            frame.gen_yield.set(None);
        }

        let result = self.interpret_to_depth(target_depth);

        // Reclaim the generator's (possibly modified) operand stack and restore
        // the caller's stack.
        let gen_stack = std::mem::replace(&mut self.stack, caller_stack);

        // The generator frame is now either suspended (still on the stack at
        // target_depth) or completed (popped by Return/Halt).
        let suspended = if self.frames.len() > target_depth {
            self.frames[target_depth].gen_suspended.get()
        } else {
            false
        };

        if suspended {
            // Capture the yielded value from the frame *before* popping it
            // (gen-state now lives on the frame, not the VM).
            let yielded = self.frames[target_depth]
                .gen_yield
                .take()
                .unwrap_or(Value::Undefined);
            // Pop the generator frame and save its state for the next resume.
            let frame = self.frames.pop().expect("generator frame present");
            // The generator's leftover operands are its private stack.
            let saved_stack = gen_stack;

            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.ip.set(frame.ip);
                    *g.env.borrow_mut() = frame.env;
                    *g.locals.borrow_mut() = frame.locals;
                    *g.stack.borrow_mut() = saved_stack;
                    *g.catch_stack.borrow_mut() = frame.catch_stack;
                    g.started.set(true);
                }
            });

            Ok((yielded, false))
        } else {
            // Completed: the body returned or ran off the end. `result` holds
            // the return value; mark the generator done.
            self.heap.with_obj(g_idx.0, |o| {
                if let HeapObj::LazyGenerator(g) = o {
                    g.done.set(true);
                    g.started.set(true);
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
                    .borrow()
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
            PropertyDescriptor::data(Value::String(Rc::from(ctor_name))),
        );
        props.insert(
            crate::value::PropertyKey::from("message"),
            PropertyDescriptor::data(Value::String(Rc::from(e.message.as_str()))),
        );
        props.insert(
            crate::value::PropertyKey::from("stack"),
            PropertyDescriptor::data(Value::String(Rc::from(e.stack.join("\n").as_str()))),
        );
        let obj = HeapObj::Object(ObjectData {
            props: RefCell::new(props),
            proto: RefCell::new(Some(proto)),
            extensible: Cell::new(true),
            class_name: Some(Rc::from(ctor_name)),
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
                        .and_then(|f| f.catch_stack.last().copied());
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
        loop {
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
                    let mut found_in_with: Option<Value> = None;
                    for obj in &with_objs {
                        let v = self.get_property(obj, &name)?;
                        if !v.is_undefined() {
                            found_in_with = Some(v);
                            break;
                        }
                    }
                    if let Some(v) = found_in_with {
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
                                *e.parent.borrow()
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
                                *e.parent.borrow()
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
                    |a, b| Value::String(Rc::from(format!("{}{}", a, b).as_str())),
                )?,
                Op::Sub => self.num_bin(|a, b| a - b)?,
                Op::Mul => self.num_bin(|a, b| a * b)?,
                Op::Div => self.num_bin(|a, b| a / b)?,
                Op::Mod => self.num_bin(|a, b| a % b)?,
                Op::Pow => self.num_bin(|a, b| a.powf(b))?,
                Op::Neg => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let n = self.to_number(&v)?;
                    self.stack.push(Value::Number(-n));
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
                                f.prototype.borrow().clone().unwrap_or(Value::Undefined)
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
                            o.proto().borrow().clone().unwrap_or(Value::Undefined)
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
                Op::Ushr => self.int_bin(|a, b| ((a as u32) >> (b as u32 & 31)) as i32)?,
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
                        props: RefCell::new(IndexMap::new()),
                        proto: RefCell::new(Some(self.object_proto.clone())),
                        extensible: std::cell::Cell::new(true),
                        class_name: None,
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
                        items: RefCell::new(items),
                        props: RefCell::new(IndexMap::new()),
                        proto: RefCell::new(Some(self.array_proto.clone())),
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
                                a.items.borrow_mut().push(value.clone());
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
                                    a.items.borrow_mut().push(v.clone());
                                }
                            });
                        }
                    }
                    self.stack.push(arr);
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
                    let pkey = crate::value::PropertyKey::from(self.to_property_key(&key)?);
                    let removed = if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            o.props().borrow_mut().shift_remove(&pkey).is_some()
                        })
                    } else {
                        false
                    };
                    self.stack.push(Value::Bool(removed || obj.is_object()));
                }
                Op::SetProto => {
                    // stack (top->bottom): [proto, obj]; set obj's [[Prototype]] to proto.
                    let proto = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            *o.proto().borrow_mut() = Some(proto);
                        });
                    }
                }
                Op::Throw => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    // if there's an active try, jump to the catch handler
                    if let Some(frame) = self.frames.last_mut() {
                        if let Some(handler) = frame.catch_stack.pop() {
                            frame.ip = handler;
                            self.stack.push(v);
                            continue;
                        }
                    }
                    return Err(Error::thrown(v, &self.heap));
                }
                Op::PushTry(handler) => {
                    self.frames.last_mut().unwrap().catch_stack.push(handler);
                }
                Op::PopTry => {
                    self.frames.last_mut().unwrap().catch_stack.pop();
                }
                Op::EnterCatch => {
                    // pop the thrown value and bind it; the compiler already
                    // emitted a StoreLocal for the catch param.
                }
                Op::Call(arg_count) => {
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let callee = self.stack.pop().unwrap_or(Value::Undefined);
                    let result = self.call_function(&callee, &args, Some(Value::Undefined))?;
                    self.stack.push(result);
                }
                Op::CallMethod(arg_count) => {
                    // stack: [obj, key, arg1, arg2, ...]
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
                }
                Op::CallMethodOpt(arg_count) => {
                    // Optional method call: like CallMethod but if the resolved
                    // method is null/undefined, short-circuit to undefined.
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
                }
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
                        .map(|f| f.gen_mode.get())
                        .unwrap_or(false);
                    if in_gen {
                        let frame = self.frames.last().unwrap();
                        frame.gen_yield.set(Some(v));
                        frame.gen_suspended.set(true);
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
                Op::CallSpread => {
                    // stack: [callee, argsArray]; spread the array's items as call args.
                    let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
                    let callee = self.stack.pop().unwrap_or(Value::Undefined);
                    let mut args = Vec::new();
                    if let Value::Object(idx) = &args_arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                args = a.items.borrow().clone();
                            }
                        });
                    }
                    let result = self.call_function(&callee, &args, Some(Value::Undefined))?;
                    self.stack.push(result);
                }
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
                    let (caller_env, this_val) = self
                        .frames
                        .last()
                        .map(|f| (f.env, f.this_val.clone()))
                        .unwrap_or((self.global, Value::Undefined));
                    let result = self.eval_direct(&src, caller_env, this_val)?;
                    self.stack.push(result);
                }
                Op::New(arg_count) => {
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let constructor = self.stack.pop().unwrap_or(Value::Undefined);
                    let result = self.construct(&constructor, &args)?;
                    self.stack.push(result);
                }
                Op::MakeClosure(func_idx) => {
                    if let Some(fdef) = self.functions.get(func_idx).cloned() {
                        let env_idx = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                        let is_arrow = fdef.is_arrow;
                        // create a .prototype object for non-arrow functions
                        let proto_val = if !fdef.is_arrow {
                            let proto = HeapObj::Object(crate::value::ObjectData {
                                props: RefCell::new(IndexMap::new()),
                                proto: RefCell::new(Some(self.object_proto.clone())),
                                extensible: std::cell::Cell::new(true),
                                class_name: None,
                            });
                            Value::Object(GcIdx(self.heap.allocate(proto)))
                        } else {
                            Value::Undefined
                        };
                        let fd = crate::value::FunctionData {
                            name: fdef.name.clone(),
                            kind: crate::value::FunctionKind::Interpreted { func: fdef },
                            closure: env_idx,
                            prototype: RefCell::new(if !is_arrow {
                                Some(proto_val.clone())
                            } else {
                                None
                            }),
                            props: RefCell::new(IndexMap::new()),
                        };
                        let idx = self.heap.allocate(HeapObj::Function(fd));
                        // link prototype.constructor back to the function
                        if let Value::Object(pidx) = &proto_val {
                            self.heap.with_obj(pidx.0, |obj| {
                                let mut desc = crate::value::PropertyDescriptor::data(
                                    Value::Object(GcIdx(idx)),
                                );
                                desc.enumerable = false;
                                obj.props()
                                    .borrow_mut()
                                    .insert(crate::value::PropertyKey::from("constructor"), desc);
                            });
                        }
                        self.stack.push(Value::Object(GcIdx(idx)));
                    } else {
                        self.stack.push(Value::Undefined);
                    }
                }
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
                    self.stack.push(Value::String(Rc::from(t)));
                }
                Op::TypeCoerce => {
                    // unary +: ToNumber coercion.
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let n = self.to_number(&v)?;
                    self.stack.push(Value::Number(n));
                }
                Op::Await => {
                    // Synchronous await: pop a value; if it is a pending promise,
                    // drain microtasks until it settles, then push its result.
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &v {
                        let is_promise = self
                            .heap
                            .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
                        if is_promise {
                            self.run_microtasks()?;
                            let (state, result) = self.heap.with_obj(idx.0, |o| {
                                if let HeapObj::Promise(p) = o {
                                    (p.state.get(), p.result.borrow().clone())
                                } else {
                                    (PromiseStatus::Fulfilled, Value::Undefined)
                                }
                            });
                            if state == PromiseStatus::Rejected {
                                return Err(Error::thrown(result, &self.heap));
                            }
                            self.stack.push(result);
                        } else {
                            self.stack.push(v);
                        }
                    } else {
                        self.stack.push(v);
                    }
                }
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
                    self.stack.push(Value::String(Rc::from(t)));
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
                _ => {
                    // Unimplemented op: skip for now
                }
            }
            // GC check
            if self.heap.live_count() > 0 && self.heap.live_count().is_multiple_of(4096) {
                let roots = self.collect_roots();
                self.heap.maybe_collect(&roots);
            }
        }
    }

    fn pop2(&mut self) -> (Value, Value) {
        let b = self.stack.pop().unwrap_or(Value::Undefined);
        let a = self.stack.pop().unwrap_or(Value::Undefined);
        (a, b)
    }

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

    fn bin_op<F: Fn(f64, f64) -> Value, G: Fn(&str, &str) -> Value>(
        &mut self,
        numf: F,
        _strf: G,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        // string concatenation
        let ap = self.to_primitive(&a)?;
        let bp = self.to_primitive(&b)?;
        match (&ap, &bp) {
            (Value::String(_), _) | (_, Value::String(_)) => {
                let sa = self.to_string(&ap)?;
                let sb = self.to_string(&bp)?;
                self.stack
                    .push(Value::String(Rc::from(format!("{}{}", sa, sb).as_str())));
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
            Value::String(s) => {
                let t = s.trim();
                if t.is_empty() {
                    0.0
                } else {
                    t.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            Value::Object(idx) => {
                self.heap.with_obj(idx.0, |obj| {
                    match obj {
                        HeapObj::Array(a) => {
                            let items = a.items.borrow();
                            if items.len() <= 1 {
                                0.0
                            } else {
                                // recurse needed
                                f64::NAN
                            }
                        }
                        _ => f64::NAN,
                    }
                })
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to number".to_string(),
                ));
            }
        })
    }

    pub fn to_string(&mut self, v: &Value) -> error::Result<Rc<str>> {
        Ok(match v {
            Value::Undefined => Rc::from("undefined"),
            Value::Null => Rc::from("null"),
            Value::Bool(b) => Rc::from(b.to_string().as_str()),
            Value::Number(n) => Rc::from(crate::value::num_to_string(*n).as_str()),
            Value::String(s) => s.clone(),
            Value::Object(idx) => {
                let is_array = self
                    .heap
                    .with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_)));
                if is_array {
                    // join items outside the borrow
                    let items = self.heap.with_obj(idx.0, |obj| {
                        if let HeapObj::Array(a) = obj {
                            a.items.borrow().clone()
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
                    Rc::from(parts.join(",").as_str())
                } else {
                    self.heap.with_obj(idx.0, |obj| match obj {
                        HeapObj::Object(o) => {
                            if let Some(cn) = &o.class_name {
                                cn.clone()
                            } else {
                                Rc::from("[object Object]")
                            }
                        }
                        _ => Rc::from("[object Object]"),
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

    pub fn to_primitive(&mut self, v: &Value) -> error::Result<Value> {
        match v {
            Value::Object(_idx) => {
                // simplified: arrays/objects -> toString-ish
                let s = self.to_string(v)?;
                Ok(Value::String(s))
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
                            .borrow_mut()
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
        match obj {
            Value::Object(idx) => {
                let (found, proto) = self.heap.with_obj(idx.0, |o| {
                    let props = o.props();
                    let v = props.borrow().get(key).map(|d| d.value.clone());
                    let proto = o.proto().borrow().clone();
                    (v, proto)
                });
                if let Some(v) = found {
                    return Ok(v);
                }
                if let Some(proto) = proto {
                    if !proto.is_undefined() {
                        return self.get_property_by_key(&proto, key);
                    }
                }
                Ok(Value::Undefined)
            }
            _ => Ok(Value::Undefined),
        }
    }

    /// Does `obj` (or its prototype chain) have an own/inherited property for
    /// the given `PropertyKey`? Used by the iterator protocol to detect a
    /// user-defined `Symbol.iterator`.
    pub fn has_property_key(&self, obj: &Value, key: &crate::value::PropertyKey) -> bool {
        let mut cur = obj.clone();
        while let Value::Object(idx) = &cur {
            let (has, proto) = self.heap.with_obj(idx.0, |o| {
                (
                    o.props().borrow().contains_key(key),
                    o.proto().borrow().clone(),
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
    pub fn has_property(&mut self, obj: &Value, name: &str) -> error::Result<bool> {
        let v = self.get_property(obj, name)?;
        Ok(!v.is_undefined())
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
            _ => false,
        })
    }

    // ---- property access ----

    pub fn get_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        match obj {
            Value::String(s) => {
                if key == "length" {
                    return Ok(Value::Number(s.chars().count() as f64));
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(c) = s.chars().nth(idx) {
                        return Ok(Value::String(Rc::from(c.to_string().as_str())));
                    }
                    return Ok(Value::Undefined);
                }
                self.get_proto_property(obj, key)
            }
            Value::Number(_) => self.get_proto_property(obj, key),
            Value::Bool(_) => self.get_proto_property(obj, key),
            Value::Symbol(_) => self.get_proto_property(obj, key),
            Value::Undefined | Value::Null => Err(Error::type_err(format!(
                "Cannot read properties of {} (reading '{}')",
                obj.type_of(),
                key
            ))),
            Value::Object(idx) => {
                // array
                let proto = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            return Ok::<Value, Error>(
                                Value::Number(a.items.borrow().len() as f64),
                            );
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            let items = a.items.borrow();
                            return Ok(items.get(i).cloned().unwrap_or(Value::Undefined));
                        }
                    }
                    if let HeapObj::Map(m) = o {
                        if key == "size" {
                            return Ok(Value::Number(m.entries.borrow().len() as f64));
                        }
                    }
                    if let HeapObj::Set(s) = o {
                        if key == "size" {
                            return Ok(Value::Number(s.items.borrow().len() as f64));
                        }
                    }
                    let props = o.props();
                    if let Some(desc) = props.borrow().get(&crate::value::PropertyKey::from(key)) {
                        return Ok(desc.value.clone());
                    }
                    // function-specific: .prototype lives in a dedicated field
                    if let HeapObj::Function(f) = o {
                        if key == "prototype" {
                            if let Some(p) = f.prototype.borrow().as_ref() {
                                return Ok(p.clone());
                            }
                        }
                        if key == "name" {
                            if let Some(n) = &f.name {
                                return Ok(Value::String(n.clone()));
                            }
                            return Ok(Value::String(Rc::from("")));
                        }
                        if key == "length" {
                            if let crate::value::FunctionKind::Native { length, .. } = &f.kind {
                                return Ok(Value::Number(*length as f64));
                            }
                            if let crate::value::FunctionKind::Interpreted { func } = &f.kind {
                                return Ok(Value::Number(func.params.len() as f64));
                            }
                        }
                    }
                    Ok(Value::Undefined)
                });
                let val = proto?;
                if !val.is_undefined() {
                    return Ok(val);
                }
                // walk proto chain
                let p = self.heap.with_obj(idx.0, |o| o.proto().borrow().clone());
                if let Some(proto) = p {
                    if !proto.is_undefined() {
                        return self.get_property(&proto, key);
                    }
                }
                Ok(Value::Undefined)
            }
            _ => Ok(Value::Undefined),
        }
    }

    fn get_proto_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        let proto = match obj {
            Value::String(_) => self.string_proto.clone(),
            Value::Number(_) => self.number_proto.clone(),
            Value::Bool(_) => self.boolean_proto.clone(),
            Value::Symbol(_) => self.symbol_proto.clone(),
            _ => return Ok(Value::Undefined),
        };
        if !proto.is_undefined() {
            return self.get_property(&proto, key);
        }
        Ok(Value::Undefined)
    }

    pub fn set_property(&mut self, obj: &Value, key: &str, value: Value) -> error::Result<()> {
        match obj {
            Value::Object(idx) => {
                self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            let n = value.clone();
                            // simplified: truncate/extend
                            if let Value::Number(len) = n {
                                let mut items = a.items.borrow_mut();
                                let new_len = len as usize;
                                items.truncate(new_len);
                                while items.len() < new_len {
                                    items.push(Value::Undefined);
                                }
                            }
                            return;
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            let mut items = a.items.borrow_mut();
                            while items.len() <= i {
                                items.push(Value::Undefined);
                            }
                            items[i] = value;
                            return;
                        }
                    }
                    let props = o.props();
                    props.borrow_mut().insert(
                        crate::value::PropertyKey::from(key),
                        crate::value::PropertyDescriptor::data(value),
                    );
                });
                Ok(())
            }
            _ => Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            )),
        }
    }

    // ---- GC roots ----
    pub fn collect_roots(&self) -> Vec<usize> {
        let mut roots = vec![self.global.0];
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
        }
        for proto in [
            &self.object_proto,
            &self.array_proto,
            &self.function_proto,
            &self.string_proto,
            &self.number_proto,
            &self.boolean_proto,
            &self.error_proto,
            &self.symbol_proto,
            &self.promise_proto,
            &self.iterator_proto,
            &self.map_proto,
            &self.set_proto,
        ] {
            if let Value::Object(idx) = proto {
                roots.push(idx.0);
            }
        }
        roots
    }

    pub fn gc(&self) {
        let roots = self.collect_roots();
        self.heap.collect(&roots);
    }

    /// Allocate a plain object and return its handle.
    /// Resolve a promise: set state to Fulfilled and schedule its handlers.
    pub fn promise_resolve(&mut self, promise_idx: usize, value: Value) {
        let handlers: Vec<crate::value::PromiseHandler> = self.heap.with_obj(promise_idx, |o| {
            if let HeapObj::Promise(p) = o {
                if p.state.get() != PromiseStatus::Pending {
                    return Vec::new();
                }
                p.state.set(PromiseStatus::Fulfilled);
                *p.result.borrow_mut() = value.clone();
                p.handlers.borrow_mut().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push(Microtask::Then {
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
                if p.state.get() != PromiseStatus::Pending {
                    return Vec::new();
                }
                p.state.set(PromiseStatus::Rejected);
                *p.result.borrow_mut() = reason.clone();
                p.handlers.borrow_mut().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: h.on_fulfilled,
                on_rejected: h.on_rejected,
                derived: h.derived,
            });
        }
    }

    /// Drain the microtask queue, running scheduled then/catch callbacks.
    pub fn run_microtasks(&mut self) -> error::Result<()> {
        while let Some(task) = self.microtask_queue.pop() {
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
                (p.state.get(), p.result.borrow().clone())
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
        // call the handler with the result
        match self.call_function(&handler, &[result], None) {
            Ok(ret) => {
                if let Some(d) = derived {
                    // if the return is itself a promise, adopt its state
                    if let Value::Object(ret_idx) = ret {
                        let is_promise = self
                            .heap
                            .with_obj(ret_idx.0, |o| matches!(o, HeapObj::Promise(_)));
                        if is_promise {
                            // chain: when ret settles, settle derived
                            let d2 = d;
                            self.heap.with_obj(ret_idx.0, |o| {
                                if let HeapObj::Promise(p) = o {
                                    p.handlers.borrow_mut().push(crate::value::PromiseHandler {
                                        on_fulfilled: Value::Undefined,
                                        on_rejected: Value::Undefined,
                                        derived: Some(d),
                                    });
                                    let _ = d2;
                                }
                            });
                            // simpler: register a microtask that adopts
                            // (basic: just resolve derived with the promise as value)
                            self.promise_resolve(d.0, ret);
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
                        .unwrap_or_else(|| Value::String(Rc::from(e.message.as_str())));
                    self.promise_reject(d.0, reason);
                }
            }
        }
        Ok(())
    }

    pub fn new_object(&mut self) -> GcIdx {
        let obj = HeapObj::Object(crate::value::ObjectData {
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(self.object_proto.clone())),
            extensible: std::cell::Cell::new(true),
            class_name: None,
        });
        GcIdx(self.heap.allocate(obj))
    }

    /// Allocate a function with native impl.
    pub fn new_native_function(&mut self, name: &str, func: NativeFn, length: usize) -> GcIdx {
        let fdef = crate::value::FunctionData {
            name: Some(Rc::from(name)),
            kind: crate::value::FunctionKind::Native { func, length },
            closure: self.global,
            prototype: RefCell::new(None),
            props: RefCell::new(IndexMap::new()),
        };
        GcIdx(self.heap.allocate(HeapObj::Function(fdef)))
    }

    /// Minimal stub for `Object(value)` coercion.
    pub fn to_object(&mut self, value: &Value) -> error::Result<Value> {
        Ok(match value {
            Value::Object(idx) => Value::Object(*idx),
            _ => Value::Object(self.new_object()),
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
                // store parameters into the call environment (enables closures + recursion)
                for (i, param) in func.params.iter().enumerate() {
                    let v = args.get(i).cloned().unwrap_or(Value::Undefined);
                    env::declare(
                        &self.heap,
                        call_env,
                        param,
                        v,
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
                        items: RefCell::new(rest),
                        props: RefCell::new(IndexMap::new()),
                        proto: RefCell::new(Some(self.array_proto.clone())),
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
                    items: RefCell::new(args.to_vec()),
                    props: RefCell::new(IndexMap::new()),
                    proto: RefCell::new(Some(self.array_proto.clone())),
                });
                env::declare(
                    &self.heap,
                    call_env,
                    "arguments",
                    Value::Object(GcIdx(self.heap.allocate(arr))),
                    crate::value::BindingKind::Const,
                );
                let this_val = this.unwrap_or(Value::Undefined);
                env::declare(
                    &self.heap,
                    call_env,
                    "this",
                    this_val.clone(),
                    crate::value::BindingKind::Const,
                );
                let _ = is_arrow;
                let is_gen = func.is_generator;
                if is_gen {
                    // Lazy generator: don't run the body yet. Create a suspended
                    // generator object; the body runs incrementally via next().
                    let g_idx = self.heap.allocate(HeapObj::LazyGenerator(
                        crate::value::LazyGeneratorData {
                            fdef: func.clone(),
                            closure: call_env,
                            env: RefCell::new(call_env),
                            this_val: RefCell::new(this_val.clone()),
                            args: RefCell::new(args.to_vec()),
                            ip: Cell::new(0),
                            stack: RefCell::new(Vec::new()),
                            locals: RefCell::new(Vec::new()),
                            catch_stack: RefCell::new(Vec::new()),
                            started: Cell::new(false),
                            done: Cell::new(false),
                            resume_value: RefCell::new(Value::Undefined),
                            is_async,
                            props: RefCell::new(IndexMap::new()),
                            proto: RefCell::new(Some(self.generator_proto.clone())),
                        },
                    ));
                    Ok(Value::Object(GcIdx(g_idx)))
                } else {
                    // execute the compiled function chunk
                    let result = self.execute_chunk_func(func.clone(), call_env, this_val, args)?;
                    if is_async {
                        // async functions return a Promise resolved with the result.
                        let p_idx =
                            self.heap
                                .allocate(HeapObj::Promise(crate::value::PromiseData {
                                    state: Cell::new(PromiseStatus::Fulfilled),
                                    result: RefCell::new(result.clone()),
                                    handlers: RefCell::new(Vec::new()),
                                    props: RefCell::new(IndexMap::new()),
                                    proto: RefCell::new(Some(self.promise_proto.clone())),
                                }));
                        Ok(Value::Object(GcIdx(p_idx)))
                    } else {
                        Ok(result)
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
                    .borrow()
                    .clone()
                    .unwrap_or(self.object_proto.clone())
            } else {
                self.object_proto.clone()
            }
        });
        let new_obj = HeapObj::Object(crate::value::ObjectData {
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(proto)),
            extensible: std::cell::Cell::new(true),
            class_name: None,
        });
        let this_obj = Value::Object(GcIdx(self.heap.allocate(new_obj)));
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
        // First, honor a user-defined `Symbol.iterator` method on objects.
        // Built-in iterables (Array/String/Map/Set/Generator) define their own
        // fast paths below, but a plain object with `[Symbol.iterator]` is a
        // custom iterable: call the method and wrap the returned iterator
        // object in a lazy IteratorData.
        if let Value::Object(_) = iterable {
            let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
            if self.has_property_key(iterable, &sym_key) {
                let iter_method = self.get_property_by_key(iterable, &sym_key)?;
                let iter_obj = self.call_function(&iter_method, &[], Some(iterable.clone()))?;
                return Ok(self.new_lazy_iterator(iter_obj));
            }
        }
        let items: Vec<Value> = match iterable {
            Value::String(s) => s
                .chars()
                .map(|c| Value::String(Rc::from(c.to_string().as_str())))
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
                            a.items.borrow().clone()
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
                            m.entries.borrow().iter().cloned().collect()
                        } else {
                            Vec::new()
                        }
                    });
                    let array_proto = self.array_proto.clone();
                    pairs
                        .into_iter()
                        .map(|(k, v)| {
                            let pair = HeapObj::Array(crate::value::ArrayData {
                                items: RefCell::new(vec![k, v]),
                                props: RefCell::new(IndexMap::new()),
                                proto: RefCell::new(Some(array_proto.clone())),
                            });
                            Value::Object(GcIdx(self.heap.allocate(pair)))
                        })
                        .collect::<Vec<_>>()
                } else if is_set {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Set(s) = o {
                            s.items.borrow().clone()
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

    /// Build an iterator over an object's enumerable string keys (for `for...in`).
    pub fn make_for_in_keys(&mut self, obj: &Value) -> error::Result<Value> {
        let mut keys: Vec<Value> = Vec::new();
        let mut cur = obj.clone();
        while let Value::Object(idx) = &cur {
            let (own, proto) = self.heap.with_obj(idx.0, |o| {
                let mut own = Vec::new();
                if let HeapObj::Array(a) = o {
                    for i in 0..a.items.borrow().len() {
                        own.push(Value::String(Rc::from(i.to_string().as_str())));
                    }
                }
                if let HeapObj::Map(m) = o {
                    for (k, _) in m.entries.borrow().iter() {
                        if let Value::String(s) = k {
                            own.push(Value::String(s.clone()));
                        }
                    }
                }
                for (k, desc) in o.props().borrow().iter() {
                    if desc.enumerable {
                        if let crate::value::PropertyKey::Str(s) = k {
                            own.push(Value::String(s.clone()));
                        }
                    }
                }
                (own, o.proto().borrow().clone())
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
            items: RefCell::new(items),
            index: std::cell::Cell::new(0),
            lazy_iter: RefCell::new(None),
            generator: RefCell::new(None),
            done: std::cell::Cell::new(false),
        });
        Value::Object(GcIdx(self.heap.allocate(it)))
    }

    /// Build a *lazy* iterator wrapping a JS iterator object (one returned by a
    /// user-defined `Symbol.iterator` method). Each `next()` call invokes the
    /// JS object's `next()` method and reads its `value`/`done` properties.
    fn new_lazy_iterator(&mut self, iter_obj: Value) -> Value {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: RefCell::new(Vec::new()),
            index: std::cell::Cell::new(0),
            lazy_iter: RefCell::new(Some(iter_obj)),
            generator: RefCell::new(None),
            done: std::cell::Cell::new(false),
        });
        Value::Object(GcIdx(self.heap.allocate(it)))
    }

    /// Build a lazy iterator wrapping a generator object. Each `next()` resumes
    /// the generator via `resume_generator`, preserving its return value (so
    /// `yield* gen()` yields the generator's return value as the result).
    fn new_generator_iterator(&mut self, gen: Value) -> Value {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: RefCell::new(Vec::new()),
            index: std::cell::Cell::new(0),
            lazy_iter: RefCell::new(None),
            generator: RefCell::new(Some(gen)),
            done: std::cell::Cell::new(false),
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
                        it.lazy_iter.borrow().is_some(),
                        it.generator.borrow().is_some(),
                        it.done.get(),
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
                        it.generator.borrow().clone()
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
            let (value, done) = self.resume_generator(g_idx, resume)?;
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.set(true);
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
                        it.lazy_iter.borrow().clone()
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
                            it.done.set(true);
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
                    let items = it.items.borrow();
                    let i = it.index.get();
                    if i < items.len() {
                        let v = items[i].clone();
                        it.index.set(i + 1);
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

    pub fn iterator_done(&self, it: &Value) -> bool {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    return it.index.get() >= it.items.borrow().len();
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
        func: std::rc::Rc<crate::function::FunctionDef>,
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
