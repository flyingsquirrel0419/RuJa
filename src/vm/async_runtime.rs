//! Promise microtask, generator, and async runtime helpers split
//! from vm/mod.rs for readability.

use super::*;
use crate::error::{self, Error};
use crate::value::HeapObj;
use crate::value::{GcIdx, PromiseStatus, Value};
use indexmap::IndexMap;
use parking_lot::Mutex;
use std::sync::Arc;

impl Vm {
    fn push_for_in_own_key(
        index_keys: &mut Vec<(usize, Arc<str>, bool)>,
        string_keys: &mut Vec<(Arc<str>, bool)>,
        key: Arc<str>,
        enumerable: bool,
    ) {
        if let Some(index) = crate::value::parse_array_index(&key) {
            index_keys.push((index, key, enumerable));
        } else {
            string_keys.push((key, enumerable));
        }
    }

    pub(crate) fn run_then(
        &mut self,
        promise: GcIdx,
        on_fulfilled: Value,
        on_rejected: Value,
        derived: Option<GcIdx>,
    ) -> error::Result<()> {
        let (state, result) = self.heap.with_obj(promise.0, |o| {
            if let HeapObj::Promise(p) = o {
                (*p.state.lock(), p.result.lock().clone())
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
        // Pin the source promise, handler, result, and derived promise as GC roots while the
        // handler call runs: call_function may allocate enough to trigger a GC,
        // which would otherwise collect these values held only in Rust locals.
        let pinned = self.pin_many(&[handler.clone(), result.clone()]);
        self.gc_pins.push(promise.0);
        if let Some(d) = derived {
            self.gc_pins.push(d.0);
        }
        // call the handler with the result
        let call_ret = self.call_function(&handler, std::slice::from_ref(&result), None);
        // Unpin everything (handler + result + derived) regardless of outcome.
        let mut to_unpin = pinned + 1;
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
                                    p.handlers.lock().push(crate::value::PromiseHandler {
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
                                    (*p.state.lock() != PromiseStatus::Pending, *p.state.lock())
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

    pub fn new_object(&mut self) -> error::Result<GcIdx> {
        let obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(self.object_proto.clone())),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        self.alloc(obj)
    }

    /// Allocate a heap object, returning a catchable `RangeError` if the
    /// heap limit is exceeded. All heap allocations must go through this
    /// method so the limit is enforced uniformly.
    pub(crate) fn alloc(&mut self, obj: HeapObj) -> error::Result<GcIdx> {
        // If a heap limit is set, try collecting first to free up space.
        let max = self.max_heap_objects;
        if max > 0 && self.heap.live_count() >= max {
            self.heap.collect(&self.collect_roots());
        }
        Ok(GcIdx(self.heap.allocate(obj)?))
    }

    /// Set the wrapped primitive on an object (for `new Number(5)`,
    /// `new Boolean(true)`, `new String("x")`, `Object(1n)`). `valueOf()` and
    /// `ToPrimitive` consult this so `new Number(5) + 1 === 6`.
    pub fn set_primitive(&mut self, obj: &Value, prim: Value) {
        if let Value::Object(idx) = obj {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    *od.primitive.lock() = Some(prim);
                }
            });
        }
    }

    /// Allocate a function with native impl.
    /// Register a Rust function as a global JS function. The function is
    /// callable from JS by `name` with the given `length` (arity).
    ///
    /// ```no_run
    /// use ruja::{Vm, Value};
    ///
    /// let mut vm = Vm::new().expect("init");
    /// vm.register_fn("double", |vm, args, _| {
    ///     let n = vm.to_number(&args[0])?;
    ///     Ok(Value::Number(n * 2.0))
    /// }, 1).unwrap();
    /// vm.run("double(21);").unwrap(); // -> 42
    /// ```
    pub fn register_fn(&mut self, name: &str, func: NativeFn, length: usize) -> error::Result<()> {
        let idx = self.new_native_function(name, func, length)?;
        crate::builtins::define_global(self, name, Value::Object(idx));
        Ok(())
    }

    pub fn new_native_function(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
    ) -> error::Result<GcIdx> {
        self.new_native_function_in_env(name, func, length, self.global)
    }

    pub(crate) fn new_native_function_in_env(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
    ) -> error::Result<GcIdx> {
        let mut props = IndexMap::new();
        let mut len_desc = crate::value::PropertyDescriptor::data(Value::Number(length as f64));
        len_desc.writable = false;
        len_desc.enumerable = false;
        len_desc.configurable = true;
        props.insert(crate::value::PropertyKey::from("length"), len_desc);
        let mut name_desc = crate::value::PropertyDescriptor::data(Value::String(Arc::from(name)));
        name_desc.writable = false;
        name_desc.enumerable = false;
        name_desc.configurable = true;
        props.insert(crate::value::PropertyKey::from("name"), name_desc);

        let fdef = crate::value::FunctionData {
            name: Some(Arc::from(name)),
            kind: crate::value::FunctionKind::Native { func, length },
            closure,
            lexical_new_target: Value::Undefined,
            is_class_ctor: std::sync::atomic::AtomicBool::new(false),
            // Native functions have no `prototype` property (they are not
            // constructors). Their [[Prototype]] (`__proto__`) is
            // `Function.prototype` once it has been allocated.
            prototype: Mutex::new(None),
            proto: Mutex::new(match self.function_proto {
                Value::Object(_) => Some(self.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(props),
            extensible: std::sync::atomic::AtomicBool::new(true),
            private_fields: Mutex::new(std::collections::HashMap::new()),
        };
        Ok(GcIdx(self.heap.allocate(HeapObj::Function(fdef))?))
    }

    pub(crate) fn native_callee_closure(&self) -> Option<GcIdx> {
        let Value::Object(idx) = self.current_native_callee.as_ref()? else {
            return None;
        };
        self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                Some(f.closure)
            } else {
                None
            }
        })
    }

    pub(crate) fn is_constructor_value(&self, value: &Value) -> bool {
        let Value::Object(idx) = value else {
            return false;
        };
        self.heap.with_obj(idx.0, |obj| {
            let HeapObj::Function(f) = obj else {
                return false;
            };
            match &f.kind {
                crate::value::FunctionKind::Interpreted { func } => {
                    !func.is_arrow && !func.is_method
                }
                crate::value::FunctionKind::Native { .. } => f.prototype.lock().is_some(),
                crate::value::FunctionKind::Bound { target, .. } => {
                    self.is_constructor_value(&Value::Object(*target))
                }
            }
        })
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
                let idx = self.new_object()?;
                let obj = Value::Object(idx);
                self.set_primitive(&obj, value.clone());
                let proto = match value {
                    Value::String(_) => Some(self.string_proto.clone()),
                    Value::Number(_) => Some(self.number_proto.clone()),
                    Value::Bool(_) => Some(self.boolean_proto.clone()),
                    Value::BigInt(_) => Some(self.bigint_proto.clone()),
                    Value::Symbol(_) => Some(self.symbol_proto.clone()),
                    _ => None,
                };
                if let Some(proto) = proto {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(od) = o {
                            *od.proto.lock() = Some(proto);
                        }
                    });
                }
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

    pub(crate) fn call_function_inner(
        &mut self,
        func: &Value,
        args: &[Value],
        this: Option<Value>,
    ) -> error::Result<Value> {
        // Cap the call-stack depth before pushing another frame. Without this
        // an unbounded JS recursion would overflow the Rust stack (each JS
        // call recurses through `call_function` -> `execute_chunk_func` ->
        // `interpret_to_depth`), killing the process with a hard stack
        // overflow instead of a catchable RangeError. Keep the limit
        // conservative for debug builds on small CI stacks while still
        // allowing ordinary recursive code to run.
        const MAX_CALL_STACK_DEPTH: usize = 512;
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
                            lexical_new_target: f.lexical_new_target.clone(),
                            is_class_ctor: f
                                .is_class_ctor
                                .load(std::sync::atomic::Ordering::Relaxed),
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
            Some(FuncCallInfo::Native(f)) => {
                let saved_native_callee = self.current_native_callee.replace(Value::Object(idx));
                let saved_native_new_target = self.current_native_new_target.take();
                self.current_native_new_target = self.pending_new_target.take();
                let result = f(self, args, this);
                self.current_native_callee = saved_native_callee;
                self.current_native_new_target = saved_native_new_target;
                result
            }
            Some(FuncCallInfo::Interpreted {
                func,
                closure,
                is_arrow,
                is_async,
                is_class_ctor,
                lexical_new_target,
            }) => {
                // Class constructors cannot be called without `new`.
                // `construct()` sets `pending_new_target` before calling us;
                // it is consumed later by `execute_chunk_func`. Super()
                // calls also go through `call_function` but are valid.
                if is_class_ctor && self.pending_new_target.is_none() {
                    return Err(Error::type_err(
                        "Class constructor cannot be invoked without 'new'",
                    ));
                }
                let call_env = env::new_env(&self.heap, Some(closure), true)?;
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
                        crate::value::BindingKind::Param,
                    );
                }
                // rest parameter: collect remaining args into an array.
                if let Some(rest_name) = &func.rest_param {
                    let rest: Vec<Value> = if func.params.len() <= args.len() {
                        args[func.params.len()..].to_vec()
                    } else {
                        Vec::new()
                    };
                    let arr = HeapObj::Array(crate::value::ArrayData::new(
                        rest,
                        Some(self.array_proto.clone()),
                    ));
                    env::declare(
                        &self.heap,
                        call_env,
                        rest_name,
                        Value::Object(GcIdx(self.heap.allocate(arr)?)),
                        crate::value::BindingKind::Param,
                    );
                }
                if !is_arrow {
                    let mut arg_array = crate::value::ArrayData::new(
                        args.to_vec(),
                        Some(self.object_proto.clone()),
                    );
                    arg_array
                        .is_arguments
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    if !func.chunk.is_strict && !func.has_parameter_expressions {
                        let mut seen = std::collections::HashSet::new();
                        let mut names = vec![None; func.params.len()];
                        for (i, name) in func.params.iter().enumerate().rev() {
                            if i < args.len() && seen.insert(name.clone()) {
                                names[i] = Some(name.clone());
                            }
                        }
                        arg_array.arguments_map = Mutex::new(Some(crate::value::ArgumentsMap {
                            env: call_env,
                            names,
                        }));
                    }
                    let arr = HeapObj::Array(arg_array);
                    let arg_idx = GcIdx(self.heap.allocate(arr)?);
                    self.heap.with_obj(arg_idx.0, |obj| {
                        if let HeapObj::Array(a) = obj {
                            let mut props = a.props.lock();
                            let mut length_desc = crate::value::PropertyDescriptor::data(
                                Value::Number(args.len() as f64),
                            );
                            length_desc.writable = true;
                            length_desc.enumerable = false;
                            length_desc.configurable = true;
                            props.insert(crate::value::PropertyKey::from("length"), length_desc);
                        }
                    });
                    if func.chunk.is_strict {
                        let thrower = match &self.function_proto {
                            Value::Object(proto_idx) => self.heap.with_obj(proto_idx.0, |obj| {
                                obj.props()
                                    .lock()
                                    .get(&crate::value::PropertyKey::from("caller"))
                                    .and_then(|desc| {
                                        if desc.is_accessor {
                                            desc.get.clone()
                                        } else {
                                            None
                                        }
                                    })
                            }),
                            _ => None,
                        };
                        if let Some(thrower) = thrower {
                            self.heap.with_obj(arg_idx.0, |obj| {
                                if let HeapObj::Array(a) = obj {
                                    a.props.lock().insert(
                                        crate::value::PropertyKey::from("callee"),
                                        crate::value::PropertyDescriptor {
                                            value: Value::Undefined,
                                            writable: false,
                                            enumerable: false,
                                            configurable: false,
                                            get: Some(thrower.clone()),
                                            set: Some(thrower),
                                            is_accessor: true,
                                        },
                                    );
                                }
                            });
                        }
                    } else {
                        // In non-strict mode, arguments has a `callee` property
                        // pointing to the executing function.
                        self.heap.with_obj(arg_idx.0, |obj| {
                            if let HeapObj::Array(a) = obj {
                                let mut props = a.props.lock();
                                let mut callee_desc =
                                    crate::value::PropertyDescriptor::data(Value::Object(idx));
                                callee_desc.writable = true;
                                callee_desc.enumerable = false;
                                callee_desc.configurable = true;
                                props
                                    .insert(crate::value::PropertyKey::from("callee"), callee_desc);
                            }
                        });
                    }
                    env::declare(
                        &self.heap,
                        call_env,
                        "arguments",
                        Value::Object(arg_idx),
                        crate::value::BindingKind::Var,
                    );
                }
                // In sloppy (non-strict) mode, an unbound `this` (plain
                // function call with no receiver) defaults to the global
                // object. In strict mode it stays `undefined`. Arrow functions
                // ignore `this` entirely (lexical capture).
                let this_val = if is_arrow {
                    this.unwrap_or(Value::Undefined)
                } else {
                    let raw = this.unwrap_or(Value::Undefined);
                    if !func.chunk.is_strict {
                        if raw.is_nullish() {
                            self.global_this.clone()
                        } else {
                            self.to_object(&raw)?
                        }
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
                    // Derived class constructors leave `this` in the TDZ
                    // until `super()` initializes it.
                    if func.is_derived {
                        env::declare_uninit(
                            &self.heap,
                            call_env,
                            "this",
                            crate::value::BindingKind::Const,
                        );
                    } else {
                        env::declare(
                            &self.heap,
                            call_env,
                            "this",
                            this_val.clone(),
                            crate::value::BindingKind::Const,
                        );
                    }
                }
                // For object literal methods, bind #super to the HomeObject
                // approximation used by RuJa's method calls. Super property
                // code reads its prototype dynamically at each access. Class
                // methods already have #super bound by the compiler.
                if func.is_method && !is_arrow {
                    let has_super = crate::environment::has(&self.heap, call_env, "#super");
                    if !has_super {
                        env::declare(
                            &self.heap,
                            call_env,
                            "#super",
                            this_val.clone(),
                            crate::value::BindingKind::Const,
                        );
                    }
                }
                let _ = &this_val;
                let is_gen = func.is_generator;
                if is_gen {
                    let prologue = self.execute_generator_prologue(
                        func.clone(),
                        call_env,
                        this_val.clone(),
                        args,
                    )?;
                    let generator_instance_proto = {
                        let callee = Value::Object(idx);
                        let proto = self
                            .get_property_by_key(
                                &callee,
                                &crate::value::PropertyKey::from("prototype"),
                            )
                            .unwrap_or(Value::Undefined);
                        if matches!(proto, Value::Object(_)) {
                            proto
                        } else {
                            self.generator_proto.clone()
                        }
                    };
                    // Lazy generator: don't run the body yet. Create a suspended
                    // generator object; the body runs incrementally via next().
                    let g_idx = self.heap.allocate(HeapObj::LazyGenerator(
                        crate::value::LazyGeneratorData {
                            fdef: func.clone(),
                            closure: call_env,
                            env: Mutex::new(prologue.env),
                            this_val: Mutex::new(this_val.clone()),
                            args: Mutex::new(args.to_vec()),
                            ip: AtomicUsize::new(prologue.ip),
                            stack: Mutex::new(prologue.stack),
                            locals: Mutex::new(prologue.locals),
                            catch_stack: Mutex::new(prologue.catch_stack),
                            started: AtomicBool::new(false),
                            done: AtomicBool::new(false),
                            resume_value: Mutex::new(Value::Undefined),
                            is_async,
                            props: Mutex::new(IndexMap::new()),
                            proto: Mutex::new(Some(generator_instance_proto)),
                        },
                    ))?;
                    Ok(Value::Object(GcIdx(g_idx)))
                } else {
                    // execute the compiled function chunk
                    let frame_new_target = if is_arrow {
                        lexical_new_target
                    } else {
                        self.pending_new_target.take().unwrap_or(Value::Undefined)
                    };
                    let mut result = self.execute_chunk_func(
                        Value::Object(idx),
                        func.clone(),
                        call_env,
                        this_val,
                        args,
                        frame_new_target,
                    );
                    // For derived class constructors, check that `super()` was
                    // called (i.e. `this` is no longer in the TDZ). If the
                    // constructor returned without calling super, throw a
                    // ReferenceError per spec.
                    if func.is_derived {
                        if let Ok(ref rv) = result {
                            // Per spec [[Construct]] step 13: if a derived
                            // constructor returns a value, it must be an
                            // object (or undefined). Returning a primitive
                            // (number, string, boolean, null) is a TypeError.
                            let is_object = matches!(rv, Value::Object(_) | Value::Undefined);
                            if !is_object {
                                return Err(Error::type_err(
                                    "Derived constructor may only return an object or undefined",
                                ));
                            }
                            if !matches!(rv, Value::Object(_)) {
                                let bound_this = self.heap.with_obj(call_env.0, |obj| {
                                    if let HeapObj::Environment(e) = obj {
                                        let vars = e.vars.lock();
                                        vars.get("this").and_then(|b| {
                                            if b.initialized.load(Ordering::Relaxed) {
                                                Some(b.value.lock().clone())
                                            } else {
                                                None
                                            }
                                        })
                                    } else {
                                        None
                                    }
                                });
                                result = match bound_this {
                                    Some(this_val) => Ok(this_val),
                                    None => Err(Error::reference(
                                        "must call super constructor before accessing 'this' or returning"
                                    )),
                                };
                            }
                        }
                    }
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
                                ))?;
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
                                ))?;
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
        let bound_construct = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                if let crate::value::FunctionKind::Bound {
                    target, bound_args, ..
                } = &f.kind
                {
                    return Some((*target, bound_args.clone()));
                }
            }
            None
        });
        if let Some((target, bound_args)) = bound_construct {
            let mut all = bound_args;
            all.extend_from_slice(args);
            return self.construct(&Value::Object(target), &all);
        }
        if !self.is_constructor_value(constructor) {
            return Err(Error::type_err("not a constructor".to_string()));
        }
        // Read prototype from the function's own properties first (it may
        // have been reassigned via `fn.prototype = newProto`), falling back
        // to the internal FunctionData.prototype field.
        let proto = self
            .get_property_by_key(constructor, &crate::value::PropertyKey::from("prototype"))
            .unwrap_or(Value::Undefined);
        let proto = if matches!(proto, Value::Object(_) | Value::Null) {
            proto
        } else {
            // Fall back to internal field or default object proto.
            self.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Function(f) = obj {
                    f.prototype
                        .lock()
                        .clone()
                        .unwrap_or(self.object_proto.clone())
                } else {
                    self.object_proto.clone()
                }
            })
        };
        let class_name = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                match f.name.as_deref() {
                    Some("Error")
                    | Some("EvalError")
                    | Some("RangeError")
                    | Some("ReferenceError")
                    | Some("SyntaxError")
                    | Some("TypeError")
                    | Some("URIError") => Some(Arc::from("Error")),
                    Some("Date") => Some(Arc::from("Date")),
                    _ => None,
                }
            } else {
                None
            }
        });
        let new_obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        let this_obj = Value::Object(GcIdx(self.heap.allocate(new_obj)?));
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
                    return self.new_lazy_iterator(iter_obj);
                }
            }
        }
        if let Value::Object(idx) = iterable {
            let is_array = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Array(_)));
            if is_array {
                return self.new_array_like_iterator(iterable.clone());
            }
        }

        let items: Vec<Value> = match iterable {
            Value::String(s) => crate::value::utf16_code_point_strings(s)
                .into_iter()
                .map(|s| Value::String(Arc::from(s.as_str())))
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
                    return self.new_generator_iterator(iterable.clone());
                } else if is_array {
                    unreachable!("array iterators are handled lazily above")
                } else if is_map {
                    // Extract (k, v) pairs out of the borrow first; allocate the
                    // pair arrays afterwards so we never call heap.allocate while
                    // with_obj holds an immutable borrow of the heap cells (which
                    // would panic on RefCell reborrow).
                    let pairs: Vec<(Value, Value)> = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Map(m) = o {
                            m.entries
                                .lock()
                                .iter()
                                .map(|(k, v)| (k.0.clone(), v.clone()))
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    });
                    let array_proto = self.array_proto.clone();
                    let mut pair_vals = Vec::with_capacity(pairs.len());
                    for (k, v) in pairs {
                        let pair = HeapObj::Array(crate::value::ArrayData::new(
                            vec![k, v],
                            Some(array_proto.clone()),
                        ));
                        pair_vals.push(Value::Object(GcIdx(self.heap.allocate(pair)?)));
                    }
                    pair_vals
                } else if is_set {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Set(s) = o {
                            s.items
                                .lock()
                                .iter()
                                .map(|k| k.0.clone())
                                .collect::<Vec<_>>()
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
        self.new_iterator(items)
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
                return self.new_lazy_iterator(iter_obj);
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
        let mut keys: Vec<(Arc<str>, Value)> = Vec::new();
        let mut visited: Vec<Arc<str>> = Vec::new();
        let mut cur = obj.clone();
        while let Value::Object(idx) = &cur {
            let (own, proto) = self.heap.with_obj(idx.0, |o| {
                let mut index_keys: Vec<(usize, Arc<str>, bool)> = Vec::new();
                let mut string_keys: Vec<(Arc<str>, bool)> = Vec::new();
                if let HeapObj::Array(a) = o {
                    let props = a.props.lock();
                    for (i, present) in a.present.lock().iter().copied().enumerate() {
                        if !present {
                            continue;
                        }
                        let key = i.to_string();
                        let enumerable = props
                            .get(&crate::value::PropertyKey::from(key.as_str()))
                            .is_none_or(|desc| desc.enumerable);
                        Self::push_for_in_own_key(
                            &mut index_keys,
                            &mut string_keys,
                            Arc::from(key.as_str()),
                            enumerable,
                        );
                    }
                }
                if let HeapObj::Map(m) = o {
                    for (k, _) in m.entries.lock().iter().map(|(k, v)| (&k.0, v)) {
                        if let Value::String(s) = k {
                            Self::push_for_in_own_key(
                                &mut index_keys,
                                &mut string_keys,
                                s.clone(),
                                true,
                            );
                        }
                    }
                }
                if let HeapObj::Object(od) = o {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        for i in 0..crate::value::utf16_len(&s) {
                            Self::push_for_in_own_key(
                                &mut index_keys,
                                &mut string_keys,
                                Arc::from(i.to_string().as_str()),
                                true,
                            );
                        }
                    }
                }
                for (k, desc) in o.props().lock().iter() {
                    if let crate::value::PropertyKey::Str(s) = k {
                        Self::push_for_in_own_key(
                            &mut index_keys,
                            &mut string_keys,
                            s.clone(),
                            desc.enumerable,
                        );
                    }
                }
                index_keys.sort_by_key(|(index, _, _)| *index);
                let own: Vec<(Arc<str>, bool)> = index_keys
                    .into_iter()
                    .map(|(_, key, enumerable)| (key, enumerable))
                    .chain(string_keys)
                    .collect();
                (own, o.proto().lock().clone())
            });
            for (k, enumerable) in own {
                if visited.iter().any(|seen| **seen == *k) {
                    continue;
                }
                visited.push(k.clone());
                if enumerable {
                    keys.push((k, cur.clone()));
                }
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        self.new_for_in_iterator(obj.clone(), keys)
    }

    pub(crate) fn new_iterator(&mut self, items: Vec<Value>) -> error::Result<Value> {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(items),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(None),
            array_like: Mutex::new(None),
            for_in_source: Mutex::new(None),
            for_in_key_sources: Mutex::new(Vec::new()),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    /// Build a *lazy* iterator wrapping a JS iterator object (one returned by a
    /// user-defined `Symbol.iterator` method). Each `next()` call invokes the
    /// JS object's `next()` method and reads its `value`/`done` properties.
    pub(crate) fn new_lazy_iterator(&mut self, iter_obj: Value) -> error::Result<Value> {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(Some(iter_obj)),
            generator: Mutex::new(None),
            array_like: Mutex::new(None),
            for_in_source: Mutex::new(None),
            for_in_key_sources: Mutex::new(Vec::new()),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    /// Build a lazy iterator wrapping a generator object. Each `next()` resumes
    /// the generator via `resume_generator`, preserving its return value (so
    /// `yield* gen()` yields the generator's return value as the result).
    pub(crate) fn new_generator_iterator(&mut self, gen: Value) -> error::Result<Value> {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(Some(gen)),
            array_like: Mutex::new(None),
            for_in_source: Mutex::new(None),
            for_in_key_sources: Mutex::new(Vec::new()),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    pub(crate) fn new_array_like_iterator(&mut self, source: Value) -> error::Result<Value> {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(None),
            array_like: Mutex::new(Some(source)),
            for_in_source: Mutex::new(None),
            for_in_key_sources: Mutex::new(Vec::new()),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    pub(crate) fn new_for_in_iterator(
        &mut self,
        source: Value,
        keys: Vec<(Arc<str>, Value)>,
    ) -> error::Result<Value> {
        let (keys, key_sources): (Vec<_>, Vec<_>) = keys.into_iter().unzip();
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(keys.into_iter().map(Value::String).collect()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            generator: Mutex::new(None),
            array_like: Mutex::new(None),
            for_in_source: Mutex::new(Some(source)),
            for_in_key_sources: Mutex::new(key_sources),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    pub(crate) fn iterator_close(&mut self, it: &Value) -> error::Result<()> {
        let iter_obj = match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    it.lazy_iter
                        .lock()
                        .clone()
                        .or_else(|| it.generator.lock().clone())
                } else {
                    None
                }
            }),
            _ => None,
        };
        let Some(iter_obj) = iter_obj else {
            return Ok(());
        };

        let return_method = self.get_property(&iter_obj, "return")?;
        if return_method.is_undefined() || matches!(return_method, Value::Null) {
            self.mark_iterator_done(it);
            return Ok(());
        }
        if !crate::builtins::is_callable(&return_method, &self.heap) {
            return Err(Error::type_err("Iterator return is not callable"));
        }
        let _ = self.call_function(&return_method, &[], Some(iter_obj))?;
        self.mark_iterator_done(it);
        Ok(())
    }

    pub(crate) fn mark_iterator_done(&self, it: &Value) {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    it.done.store(true, Ordering::Relaxed);
                }
            });
        }
    }

    fn for_in_key_is_enumerable(&self, source: &Value, origin: &Value, key: &str) -> bool {
        let mut cur = if origin == source {
            source.clone()
        } else {
            origin.clone()
        };
        while let Value::Object(idx) = &cur {
            let (found, enumerable, proto) = self.heap.with_obj(idx.0, |o| {
                let pkey = crate::value::PropertyKey::from(key);
                if let HeapObj::Array(a) = o {
                    if crate::value::parse_array_index(key).is_some_and(|i| a.is_dense_present(i)) {
                        let enumerable =
                            a.props.lock().get(&pkey).is_none_or(|desc| desc.enumerable);
                        return (true, enumerable, o.proto().lock().clone());
                    }
                }
                if let HeapObj::Object(od) = o {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        if crate::value::parse_array_index(key)
                            .is_some_and(|i| i < crate::value::utf16_len(&s))
                        {
                            return (true, true, o.proto().lock().clone());
                        }
                    }
                }
                let desc = o.props().lock().get(&pkey).cloned();
                match desc {
                    Some(desc) => (true, desc.enumerable, o.proto().lock().clone()),
                    None => (false, false, o.proto().lock().clone()),
                }
            });
            if found {
                return enumerable;
            }
            if origin != source {
                break;
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        false
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
        let (lazy, is_gen, is_array_like, is_for_in, already_done) = match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    (
                        it.lazy_iter.lock().is_some(),
                        it.generator.lock().is_some(),
                        it.array_like.lock().is_some(),
                        it.for_in_source.lock().is_some(),
                        it.done.load(Ordering::Relaxed),
                    )
                } else {
                    (false, false, false, false, true)
                }
            }),
            _ => return Err(Error::type_err("not an iterator".to_string())),
        };
        if already_done {
            return Ok((Value::Undefined, true));
        }
        if is_array_like {
            let source = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        it.array_like.lock().clone()
                    } else {
                        None
                    }
                },
            );
            let source = source.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            let i = self.heap.with_obj(idx, |o| {
                if let HeapObj::Iterator(it) = o {
                    let i = it.index.load(Ordering::Relaxed);
                    it.index.store(i + 1, Ordering::Relaxed);
                    i
                } else {
                    0
                }
            });
            let len = match self.get_property(&source, "length")? {
                Value::Number(n) if n.is_finite() && n > 0.0 => n.floor() as usize,
                _ => 0,
            };
            if i >= len {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
                return Ok((Value::Undefined, true));
            }
            let value = self.get_property(&source, &i.to_string())?;
            return Ok((value, false));
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
                        it.generator.lock().clone()
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
        if is_for_in {
            let source = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        it.for_in_source.lock().clone()
                    } else {
                        None
                    }
                },
            );
            let source = source.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            loop {
                let next_key = self.heap.with_obj(idx, |o| {
                    if let HeapObj::Iterator(it) = o {
                        let i = it.index.load(Ordering::Relaxed);
                        let key = it.items.lock().get(i).cloned();
                        it.index.store(i + 1, Ordering::Relaxed);
                        let origin = it
                            .for_in_key_sources
                            .lock()
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| source.clone());
                        key.map(|key| (key, origin))
                    } else {
                        None
                    }
                });
                let Some((Value::String(key), origin)) = next_key else {
                    self.heap.with_obj(idx, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                    return Ok((Value::Undefined, true));
                };
                if self.for_in_key_is_enumerable(&source, &origin, &key) {
                    return Ok((Value::String(key), false));
                }
            }
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
                        it.lazy_iter.lock().clone()
                    } else {
                        None
                    }
                },
            );
            let iter_obj =
                iter_obj.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let next_fn = self.get_property(&iter_obj, "next")?;
            let result = self.call_function(&next_fn, &[resume], Some(iter_obj))?;
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
                return Ok((Value::Undefined, true));
            }
            let value = self.get_property(&result, "value")?;
            Ok((value, done))
        } else {
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            self.heap.with_obj(idx, |o| {
                if let HeapObj::Iterator(it) = o {
                    let items = it.items.lock();
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
                    i.lazy_iter.lock().is_some() || i.generator.lock().is_some()
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
                            .clone()
                            .or_else(|| i.generator.lock().clone())
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
                return Ok((Value::Undefined, true));
            }
            let value = self.get_property(&result, "value")?;
            return Ok((value, done));
        }
        // Eager (Vec-backed) iterator: step directly, no awaiting needed.
        self.iterator_next(it)
    }

    /// Await a value: if it is a pending Promise, drain microtasks until it
    /// settles and return the settled value (rejecting on rejection); otherwise
    /// return the value as-is.
    pub(crate) fn await_value(&mut self, v: Value) -> error::Result<Value> {
        if let Value::Object(idx) = &v {
            let is_promise = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
            if is_promise {
                self.run_microtasks()?;
                let (state, result) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Promise(p) = o {
                        (*p.state.lock(), p.result.lock().clone())
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
                    return it.index.load(Ordering::Relaxed) >= it.items.lock().len();
                }
                false
            })
        } else {
            false
        }
    }
}
