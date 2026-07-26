use super::*;
use num_traits::{Signed, ToPrimitive};

#[derive(Clone, Copy)]
enum CompareOp {
    Lt,
    Gt,
    Lte,
    Gte,
}

impl Vm {
    fn private_name_binding_name(name: &str) -> String {
        format!("#private_name:{}", name)
    }

    fn function_name_from_property_key(
        &self,
        key: &crate::value::PropertyKey,
        prefix: Option<&str>,
    ) -> Arc<str> {
        let base = match key {
            crate::value::PropertyKey::Str(name) => name.to_string(),
            crate::value::PropertyKey::Symbol(id) => self
                .symbol_descriptions
                .get(id)
                .and_then(|description| description.as_ref())
                .map(|description| format!("[{}]", description))
                .unwrap_or_default(),
        };
        match prefix {
            Some(prefix) => Arc::from(format!("{}{}", prefix, base).as_str()),
            None => Arc::from(base.as_str()),
        }
    }

    fn set_empty_function_name_from_property_key(
        &self,
        value: &Value,
        key: &crate::value::PropertyKey,
        prefix: Option<&str>,
    ) {
        let name = self.function_name_from_property_key(key, prefix);
        self.set_empty_function_name(value, name);
    }

    fn set_empty_function_name(&self, value: &Value, name: Arc<str>) {
        let Value::Object(idx) = value else {
            return;
        };
        self.heap.with_obj(idx.0, |obj| {
            let HeapObj::Function(function) = obj else {
                return;
            };
            let mut props = function.props.lock();
            let Some(desc) = props.get_mut(&crate::value::PropertyKey::from("name")) else {
                return;
            };
            if desc.is_accessor {
                return;
            }
            if matches!(&desc.value, Value::String(current) if current.is_empty()) {
                desc.value = Value::String(name);
            }
        });
    }

    fn set_method_home_object(&self, value: &Value, home_object: &Value) {
        let Value::Object(idx) = value else {
            return;
        };
        self.heap.with_obj(idx.0, |obj| {
            let HeapObj::Function(function) = obj else {
                return;
            };
            let is_method = matches!(
                &function.kind,
                crate::value::FunctionKind::Interpreted { func } if func.is_method
            );
            if is_method {
                let mut slot = function.home_object.lock();
                if slot.is_none() {
                    *slot = Some(home_object.clone());
                }
            }
        });
    }

    fn property_key_from_value(&mut self, key: &Value) -> error::Result<crate::value::PropertyKey> {
        Ok(match key {
            Value::Symbol(id) => crate::value::PropertyKey::Symbol(*id),
            _ => crate::value::PropertyKey::from(self.to_property_key(key)?),
        })
    }

    fn private_slot_key_from_name(
        &self,
        name_idx: usize,
    ) -> error::Result<crate::value::PrivateSlotKey> {
        let (name, env) = {
            let frame = self.current_frame()?;
            let name = match &frame.chunk.constants[name_idx] {
                Value::String(s) => s.to_string(),
                _ => String::new(),
            };
            (name, frame.env)
        };
        let binding_name = Self::private_name_binding_name(&name);
        match crate::environment::get_checked(&self.heap, env, &binding_name) {
            Ok(Some(Value::PrivateName(key))) => Ok(crate::value::PrivateSlotKey::Private(key)),
            Ok(Some(_)) => Err(Error::internal(
                "private name binding is not a private name",
            )),
            Ok(None) | Err(false) => Err(Error::reference(format!(
                "Private name #{} is not defined",
                name
            ))),
            Err(true) => Err(Error::reference(format!(
                "Cannot access private name #{} before initialization",
                name
            ))),
        }
    }

    fn discard_catches_inside_finally(frame: &mut CallFrame, finally_seq: u32) {
        while frame
            .catch_stack
            .last()
            .is_some_and(|(_, cseq, _, _)| *cseq > finally_seq)
        {
            frame.catch_stack.pop();
        }
    }

    fn prepare_super_constructor_call(&self) -> error::Result<(GcIdx, Value, Value)> {
        let frame = self.current_frame()?;
        let current_env = frame.env;
        let current_this = frame.this_val.clone();
        let current_new_target = frame.new_target.clone();
        let this_env = crate::environment::find_binding_env(&self.heap, current_env, "this")
            .ok_or_else(|| Error::reference("super() called outside derived constructor"))?;
        let this_val = match crate::environment::binding_initialized(&self.heap, this_env, "this") {
            Some(true) => crate::environment::get_checked(&self.heap, this_env, "this")
                .map_err(|_| Error::reference("Cannot access 'this' before initialization"))?
                .unwrap_or(Value::Undefined),
            Some(false) => self
                .frames
                .iter()
                .rev()
                .find(|f| f.env == this_env)
                .map(|f| f.this_val.clone())
                .unwrap_or(current_this),
            None => {
                return Err(Error::reference(
                    "super() called outside derived constructor",
                ))
            }
        };
        Ok((this_env, this_val, current_new_target))
    }

    fn bind_super_constructor_result(
        &mut self,
        this_env: GcIdx,
        new_this: Value,
    ) -> error::Result<()> {
        crate::environment::bind_this_value(&self.heap, this_env, new_this.clone())?;
        if let Some(frame) = self.frames.iter_mut().rev().find(|f| f.env == this_env) {
            frame.this_val = new_this;
        }
        Ok(())
    }

    pub(crate) fn interpret_inner_raw(
        &mut self,
        return_depth: Option<usize>,
        stop_at: Option<(usize, usize)>,
    ) -> error::Result<Value> {
        loop {
            // Execution fuel: bound untrusted code. Checked before each
            // opcode so a tight loop cannot run forever. None = unbounded.
            self.consume_fuel()?;
            // Generator `throw(e)` resume: if the current frame has a pending
            // forced throw (set by resume_generator on a Throw resume), raise
            // it now at the suspended `yield` point. This lets the generator
            // body's own try/catch handle the injected exception.
            if self.frames.is_empty() {
                return Err(crate::error::Error::internal(
                    "interpret loop with no call frame",
                ));
            }
            if let Some(exc) = self.frames.last().and_then(|f| f.force_throw.lock().take()) {
                return Err(Error::thrown(exc, &self.heap));
            }
            if let Some(ret) = self
                .frames
                .last()
                .and_then(|f| f.force_return.lock().take())
            {
                let stack_base = self.current_frame()?.stack_base;
                if let Some(frame) = self.frames.last_mut() {
                    if let Some(&(target, fseq)) = frame.finally_stack.last() {
                        Self::discard_catches_inside_finally(frame, fseq);
                        frame.finally_completion_tag.store(1, Ordering::Relaxed);
                        *frame.finally_completion_val.lock() = ret;
                        frame.ip = target;
                        continue;
                    }
                }
                self.stack.truncate(stack_base);
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(ret);
                }
                if let Some(d) = return_depth {
                    if self.frames.len() <= d {
                        return Ok(ret);
                    }
                }
                self.stack.push(ret);
                continue;
            }
            let frame = self.current_frame()?;
            let ip = frame.ip;
            if stop_at.is_some_and(|(depth, stop_ip)| {
                self.frames.len().saturating_sub(1) == depth && ip == stop_ip
            }) {
                return Ok(Value::Undefined);
            }
            if ip >= frame.chunk.code.len() {
                return Ok(Value::Undefined);
            }
            let op = frame.chunk.code[ip].clone();
            self.current_frame_mut()?.ip += 1;
            match op {
                Op::Halt => {
                    let stack_base = self.current_frame()?.stack_base;
                    let v = if self.stack.len() > stack_base {
                        self.stack.pop().unwrap_or(Value::Undefined)
                    } else {
                        Value::Undefined
                    };
                    self.stack.truncate(stack_base);
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
                Op::ToPropertyKey => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.to_property_key_value(&v)?;
                    self.stack.push(key);
                }
                Op::CheckNullBase => {
                    // Stack: [obj, key]. Check obj (second from top) for
                    // null/undefined per spec ToObject, which throws TypeError
                    // before ToPropertyKey is called on the key.
                    let len = self.stack.len();
                    if len >= 2 {
                        let obj = self.stack[len - 2].clone();
                        match obj {
                            Value::Null | Value::Undefined => {
                                return Err(Error::type_err(format!(
                                    "Cannot read properties of {}",
                                    obj.type_of()
                                )));
                            }
                            _ => {}
                        }
                    }
                }
                Op::RequireObjectCoercible => {
                    let v = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    if matches!(v, Value::Null | Value::Undefined) {
                        return Err(Error::type_err(format!(
                            "Cannot destructure {}",
                            v.type_of()
                        )));
                    }
                }
                Op::Const(idx) => {
                    let v = {
                        let frame = self.current_frame()?;
                        frame.chunk.constants[idx].clone()
                    };
                    self.stack.push(v);
                }
                Op::NewRegExpLiteral(pattern_idx, flags_idx) => {
                    let (pattern, flags) = {
                        let frame = self.current_frame()?;
                        let pattern = match frame.chunk.constants.get(pattern_idx) {
                            Some(Value::String(value)) => value.clone(),
                            _ => {
                                return Err(Error::internal(
                                    "RegExp literal pattern constant is not a string",
                                ));
                            }
                        };
                        let flags = match frame.chunk.constants.get(flags_idx) {
                            Some(Value::String(value)) => value.clone(),
                            _ => {
                                return Err(Error::internal(
                                    "RegExp literal flags constant is not a string",
                                ));
                            }
                        };
                        (pattern, flags)
                    };
                    let value =
                        crate::builtins::regexp::regexp_create_literal(self, &pattern, &flags)?;
                    self.stack.push(value);
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
                    if cur_env == self.global && self.global_property_is_non_writable_data(&name) {
                        let global_this = self.global_this.clone();
                        self.set_property(&global_this, &name, value)?;
                        self.stack.push(Value::Undefined);
                        continue;
                    }
                    match crate::environment::set_checked(&self.heap, cur_env, &name, value.clone())
                    {
                        crate::environment::SetOutcome::Set => {}
                        crate::environment::SetOutcome::Const
                        | crate::environment::SetOutcome::Import => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::FunctionName => {
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Assignment to constant variable '{}'",
                                    name
                                )));
                            }
                        }
                        crate::environment::SetOutcome::Tdz => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::NotFound => {
                            // Strict mode: assigning to an undeclared variable
                            // throws ReferenceError (not auto-global).
                            if self.current_strict() {
                                return Err(Error::reference(format!("{} is not defined", name)));
                            }
                            let global_this = self.global_this.clone();
                            self.set_property(&global_this, &name, value)?;
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::DeclareEnv(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                Op::HoistVar(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                    // Hoist the var binding as undefined in the function
                    // scope root, without touching with-object properties.
                    let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let root = crate::environment::function_scope_root(&self.heap, env);
                    if root == self.global {
                        let configurable = self
                            .frames
                            .last()
                            .is_some_and(|frame| frame.eval_global_bindings);
                        self.create_global_var_binding_with_configurable(&name, configurable)?;
                    } else {
                        let deletable = self
                            .frames
                            .last()
                            .is_some_and(|frame| frame.eval_deletable_bindings);
                        crate::environment::ensure_var_with_deletable(
                            &self.heap, env, &name, deletable,
                        );
                    }
                }
                Op::DeclareVar(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                    let root = crate::environment::function_scope_root(&self.heap, cur_env);
                    // Per spec (ES5 12.2): `var x = expr` is equivalent to
                    // `var x; x = expr`. The `var x` hoisting creates a binding
                    // in the variable environment (function scope) initialized
                    // to undefined. The `x = expr` assignment uses PutValue
                    // with the reference resolved via identifier resolution,
                    // which walks the environment chain: at each environment,
                    // var bindings take precedence, then with-object properties.
                    //
                    // First, ensure the var binding exists in the function
                    // scope root (hoisting). This creates it as undefined if
                    // not already present, without touching with-objects.
                    let eval_global_bindings = self
                        .frames
                        .last()
                        .is_some_and(|frame| frame.eval_global_bindings);
                    if root == self.global {
                        self.create_global_var_binding_with_configurable(
                            &name,
                            eval_global_bindings,
                        )?;
                    } else {
                        let deletable = self
                            .frames
                            .last()
                            .is_some_and(|frame| frame.eval_deletable_bindings);
                        crate::environment::ensure_var_with_deletable(
                            &self.heap, cur_env, &name, deletable,
                        );
                    }
                    // Now set the value via identifier resolution (set_checked),
                    // which respects the env chain: a var binding in the
                    // function scope takes precedence over a with-object
                    // property at a parent scope. If no binding is found at
                    // all, declare as a new var (auto-global at top level).
                    if root == self.global && self.global_property_is_non_writable_data(&name) {
                        let global_this = self.global_this.clone();
                        self.set_property(&global_this, &name, value)?;
                        continue;
                    }
                    match crate::environment::set_checked(&self.heap, cur_env, &name, value.clone())
                    {
                        crate::environment::SetOutcome::Set => {}
                        crate::environment::SetOutcome::Const
                        | crate::environment::SetOutcome::Import => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::FunctionName => {
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Assignment to constant variable '{}'",
                                    name
                                )));
                            }
                        }
                        crate::environment::SetOutcome::Tdz => {
                            return Err(Error::reference(format!(
                                "Cannot access '{}' before initialization",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::NotFound => {
                            crate::environment::declare_var(
                                &self.heap,
                                cur_env,
                                &name,
                                value.clone(),
                            );
                        }
                    }
                    if root == self.global {
                        if eval_global_bindings {
                            self.set_global_eval_var_property(&name, value);
                        } else {
                            self.set_global_var_property(&name, value);
                        }
                    }
                }
                Op::DeclareGlobalFunction(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                    let root = crate::environment::function_scope_root(&self.heap, cur_env);
                    if root == self.global {
                        let configurable = self
                            .frames
                            .last()
                            .is_some_and(|frame| frame.eval_global_bindings);
                        self.create_global_function_binding_with_configurable(
                            &name,
                            value,
                            configurable,
                        )?;
                    } else {
                        let deletable = self
                            .frames
                            .last()
                            .is_some_and(|frame| frame.eval_deletable_bindings);
                        crate::environment::declare_var_with_deletable(
                            &self.heap, cur_env, &name, value, deletable,
                        );
                    }
                }
                Op::DeclareLet(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        crate::environment::SetOutcome::Const
                        | crate::environment::SetOutcome::Import => {
                            return Err(Error::type_err(format!(
                                "Assignment to constant variable '{}'",
                                name
                            )));
                        }
                        crate::environment::SetOutcome::FunctionName => {
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Assignment to constant variable '{}'",
                                    name
                                )));
                            }
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
                                if self.current_strict() {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )));
                                }
                                crate::environment::declare(
                                    &self.heap,
                                    self.global,
                                    &name,
                                    value,
                                    crate::value::BindingKind::Var,
                                );
                            }
                        }
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::StoreEnvName(name_idx) => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let name = {
                        let frame = self.current_frame()?;
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
                    if name.starts_with('#') {
                        match crate::environment::set_checked(&self.heap, env, &name, value.clone())
                        {
                            crate::environment::SetOutcome::Set => {}
                            crate::environment::SetOutcome::Const
                            | crate::environment::SetOutcome::Import => {
                                return Err(Error::type_err(format!(
                                    "Assignment to constant variable '{}'",
                                    name
                                )));
                            }
                            crate::environment::SetOutcome::FunctionName => {
                                if self.current_strict() {
                                    return Err(Error::type_err(format!(
                                        "Assignment to constant variable '{}'",
                                        name
                                    )));
                                }
                            }
                            crate::environment::SetOutcome::Tdz => {
                                return Err(Error::reference(format!(
                                    "Cannot access '{}' before initialization",
                                    name
                                )));
                            }
                            crate::environment::SetOutcome::NotFound => {
                                if self.current_strict() {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )));
                                }
                                let global_this = self.global_this.clone();
                                self.set_property(&global_this, &name, value)?;
                            }
                        }
                    } else {
                        let strict = self.current_strict();
                        let r#ref = self.resolve_identifier_reference(
                            crate::value::PropertyKey::from(name.as_str()),
                            strict,
                        )?;
                        let r#ref = Value::Reference(Box::new(r#ref));
                        self.put_value(&r#ref, value)?;
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::LoadRef(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => crate::value::PropertyKey::from_rc(s),
                            _ => crate::value::PropertyKey::from(""),
                        }
                    };
                    let strict = self.current_strict();
                    let r#ref = self.resolve_identifier_reference(name, strict)?;
                    self.stack.push(Value::Reference(Box::new(r#ref)));
                }
                Op::MakePropertyRef => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let base = self.stack.pop().unwrap_or(Value::Undefined);
                    if base.is_nullish() {
                        return Err(Error::type_err(format!(
                            "Cannot read properties of {}",
                            base.type_of()
                        )));
                    }
                    let pin_count = self.pin_many(&[base.clone(), key.clone()]);
                    let name_result = self.coerce_property_key_record(&key);
                    self.unpin_many(pin_count);
                    let name = name_result?;
                    let strict = self.current_strict();
                    self.stack
                        .push(Value::Reference(Box::new(crate::value::ReferenceRecord {
                            base: crate::value::ReferenceBase::Value(Box::new(base)),
                            name: name.into(),
                            strict,
                            this_value: None,
                        })));
                }
                Op::MakeRawPropertyRef => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let base = self.stack.pop().unwrap_or(Value::Undefined);
                    let strict = self.current_strict();
                    self.stack
                        .push(Value::Reference(Box::new(crate::value::ReferenceRecord {
                            base: crate::value::ReferenceBase::Value(Box::new(base)),
                            name: crate::value::ReferencedName::UncoercedProperty(Box::new(key)),
                            strict,
                            this_value: None,
                        })));
                }
                Op::MakeSuperPropertyRef => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let base = self.stack.pop().unwrap_or(Value::Undefined);
                    let this_value = self.stack.pop().unwrap_or(Value::Undefined);
                    let strict = self.current_strict();
                    self.stack
                        .push(Value::Reference(Box::new(crate::value::ReferenceRecord {
                            base: crate::value::ReferenceBase::Value(Box::new(base)),
                            name: crate::value::ReferencedName::UncoercedProperty(Box::new(key)),
                            strict,
                            this_value: Some(Box::new(this_value)),
                        })));
                }
                Op::ResolvePropertyRef => {
                    let reference = self.stack.pop().unwrap_or(Value::Undefined);
                    let Value::Reference(mut record) = reference else {
                        return Err(Error::internal("expected property reference"));
                    };
                    if matches!(&record.base, crate::value::ReferenceBase::Value(base) if base.is_nullish())
                    {
                        return Err(Error::type_err("Cannot access null super base"));
                    }
                    if let crate::value::ReferencedName::UncoercedProperty(name) = &record.name {
                        let rooted = Value::Reference(record.clone());
                        let pin_count = self.pin(&rooted);
                        let name_result = self.coerce_property_key_record(name);
                        self.unpin_many(pin_count);
                        record.name = name_result?.into();
                    }
                    self.stack.push(Value::Reference(record));
                }
                Op::MakePrivateRef(name_idx) => {
                    let name = match self.private_slot_key_from_name(name_idx)? {
                        crate::value::PrivateSlotKey::Private(name) => name,
                        crate::value::PrivateSlotKey::Internal(_) => {
                            return Err(Error::internal(
                                "class private reference resolved to an internal slot",
                            ));
                        }
                    };
                    let base = self.stack.pop().unwrap_or(Value::Undefined);
                    let strict = self.current_strict();
                    self.stack
                        .push(Value::Reference(Box::new(crate::value::ReferenceRecord {
                            base: crate::value::ReferenceBase::Value(Box::new(base)),
                            name: crate::value::ReferencedName::Private(name),
                            strict,
                            this_value: None,
                        })));
                }
                Op::GetValue => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let resolved = self.get_value(&v)?;
                    self.stack.push(resolved);
                }
                Op::PutValue => {
                    // Stack: [value, ref] (ref on top). Pop ref, then value.
                    let r#ref = self.stack.pop().unwrap_or(Value::Undefined);
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    self.put_value(&r#ref, value.clone())?;
                    // Push the stored value back as the expression result.
                    self.stack.push(value);
                }
                Op::LoadLocal(idx) => {
                    let v = self.current_frame()?.locals[idx].clone();
                    self.stack.push(v);
                }
                Op::StoreLocal(idx) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    self.current_frame_mut()?.locals[idx] = v;
                }
                Op::Null => self.stack.push(Value::Null),
                Op::Undefined => self.stack.push(Value::Undefined),
                Op::True => self.stack.push(Value::Bool(true)),
                Op::False => self.stack.push(Value::Bool(false)),
                Op::Pop => {
                    let stack_base = self.current_frame()?.stack_base;
                    if self.stack.len() > stack_base {
                        self.stack.pop();
                    }
                }
                Op::PushScope => {
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let new_env = env::new_env(&self.heap, Some(cur_env), false)?;
                    self.current_frame_mut()?.env = new_env;
                }
                Op::PushFunctionScope => {
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let new_env = env::new_env(&self.heap, Some(cur_env), true)?;
                    self.current_frame_mut()?.env = new_env;
                    self.current_frame_mut()?.in_parameter_initializers = false;
                }
                Op::PopScope => {
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.current_frame_mut()?.env = p;
                    }
                }
                Op::PushWithEnv => {
                    let object = self.stack.pop().unwrap_or(Value::Undefined);
                    // Per spec, with(null) and with(undefined) throw TypeError.
                    if matches!(object, Value::Null | Value::Undefined) {
                        return Err(Error::type_err(
                            "with statement requires an object".to_string(),
                        ));
                    }
                    let object = self.to_object(&object)?;
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let new_env = env::new_with_env(&self.heap, cur_env, object)?;
                    self.current_frame_mut()?.env = new_env;
                }
                Op::PopWithEnv => {
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.current_frame_mut()?.env = p;
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
                    let child = env::clone_loop_vars(&self.heap, cur_env, &names)?;
                    self.current_frame_mut()?.env = child;
                }
                Op::RecloneLetNames(idx) => {
                    // Between a `for (let ...)` body and its update
                    // expression, create the next iteration env as a sibling
                    // of the current one. The update mutates the sibling, so
                    // body closures keep the pre-update binding.
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let names = self
                        .frames
                        .last()
                        .map(|f| f.chunk.let_names.get(idx).cloned().unwrap_or_default())
                        .unwrap_or_default();
                    let sibling = env::clone_loop_vars_to_sibling(&self.heap, cur_env, &names)?;
                    self.current_frame_mut()?.env = sibling;
                }
                Op::RestoreParentEnv => {
                    // After the loop body (which ran in a CloneLetEnv child),
                    // restore the frame env to the child's parent (the loop
                    // scope env) so the update/cond/next iteration run in the
                    // original env and the chain does not grow per iteration.
                    let parent = self.frames.last().and_then(|f| {
                        self.heap.with_obj(f.env.0, |o| {
                            if let HeapObj::Environment(e) = o {
                                *e.parent.lock()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(p) = parent {
                        self.current_frame_mut()?.env = p;
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
                Op::Dup2 => {
                    let len = self.stack.len();
                    if len >= 2 {
                        let b = self.stack[len - 1].clone();
                        let a = self.stack[len - 2].clone();
                        self.stack.push(a);
                        self.stack.push(b);
                    }
                }
                Op::Add => self.bin_op(
                    |a, b| Value::Number(a + b),
                    |a, b| Value::String(Arc::from(format!("{}{}", a, b).as_str())),
                )?,
                Op::Sub => self.num_bin_bigint(|a, b| a - b, |x, y| Ok(x - y))?,
                Op::Mul => self.num_bin_bigint(|a, b| a * b, |x, y| Ok(x * y))?,
                Op::Div => self.num_bin_bigint(
                    |a, b| a / b,
                    |x, y| {
                        if y.is_zero() {
                            Err(Error::range("Division by zero".to_string()))
                        } else {
                            Ok(x / y)
                        }
                    },
                )?,
                Op::Mod => self.num_bin_bigint(
                    |a, b| a % b,
                    |x, y| {
                        if y.is_zero() {
                            Err(Error::range("Division by zero".to_string()))
                        } else {
                            Ok(x % y)
                        }
                    },
                )?,
                Op::Pow => self.num_bin_bigint(
                    |a, b| {
                        // ES spec: abs(base)==1 and exponent is ±Infinity → NaN
                        if a.abs() == 1.0 && b.is_infinite() {
                            f64::NAN
                        } else {
                            a.powf(b)
                        }
                    },
                    |x, y| {
                        if y.is_negative() {
                            Err(Error::range("BigInt exponent must be positive".to_string()))
                        } else {
                            // Use BigInt's own pow (exponent is a u32).
                            let exp = num_traits::ToPrimitive::to_u32(&y).ok_or_else(|| {
                                Error::range("BigInt exponent is too large".to_string())
                            })?;
                            Ok(x.pow(exp))
                        }
                    },
                )?,
                Op::Neg => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    match self.to_numeric(&v)? {
                        Value::BigInt(n) => self.stack.push(Value::BigInt(-n)),
                        Value::Number(n) => self.stack.push(Value::Number(-n)),
                        _ => unreachable!("ToNumeric returns Number or BigInt"),
                    }
                }
                Op::Not => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let b = v.is_truthy();
                    self.stack.push(Value::Bool(!b));
                }
                Op::BitNot => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    match self.to_numeric(&v)? {
                        Value::BigInt(n) => self.stack.push(Value::BigInt(!n)),
                        Value::Number(n) => self.stack.push(Value::Number(!to_int32(n) as f64)),
                        _ => unreachable!("ToNumeric returns Number or BigInt"),
                    }
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
                Op::Lt => self.compare(CompareOp::Lt)?,
                Op::Gt => self.compare(CompareOp::Gt)?,
                Op::Lte => self.compare(CompareOp::Lte)?,
                Op::Gte => self.compare(CompareOp::Gte)?,
                Op::In => {
                    // stack: [key, obj]; true if obj has the property (own or inherited).
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    if !matches!(obj, Value::Object(_)) {
                        return Err(Error::type_err(
                            "Right-hand side of 'in' is not an object".to_string(),
                        ));
                    }
                    let property_key = match self.to_property_key_value(&key)? {
                        Value::String(name) => crate::value::PropertyKey::from(name),
                        Value::Symbol(id) => crate::value::PropertyKey::Symbol(id),
                        _ => unreachable!("ToPropertyKey returns string or symbol"),
                    };
                    let has = self.has_property_key(&obj, &property_key)?;
                    self.stack.push(Value::Bool(has));
                }
                Op::InstanceOf => {
                    // stack: [obj, ctor]; first honor @@hasInstance, then fall
                    // back to OrdinaryHasInstance.
                    let ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let result = self.instanceof_operator(&obj, &ctor)?;
                    self.stack.push(Value::Bool(result));
                }
                Op::BitAnd => self.bitwise_bin(|a, b| a & b, |a, b| Ok(a & b))?,
                Op::BitOr => self.bitwise_bin(|a, b| a | b, |a, b| Ok(a | b))?,
                Op::BitXor => self.bitwise_bin(|a, b| a ^ b, |a, b| Ok(a ^ b))?,
                Op::Shl => self.shift_bin(false)?,
                Op::Shr => self.shift_bin(true)?,
                Op::Ushr => {
                    // Unsigned right shift: result is a uint32 promoted to Number,
                    // so -1 >>> 0 === 4294967295 (not -1).
                    let (a, b) = self.pop2();
                    let av = self.to_numeric(&a)?;
                    let bv = self.to_numeric(&b)?;
                    match (&av, &bv) {
                        (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                            return Err(Error::type_err(
                                "BigInt does not support unsigned right shift".to_string(),
                            ));
                        }
                        (Value::Number(a), Value::Number(b)) => {
                            let av = to_uint32(*a);
                            let bv = to_uint32(*b);
                            self.stack.push(Value::Number((av >> (bv & 31)) as f64));
                        }
                        _ => unreachable!("ToNumeric returns Number or BigInt"),
                    }
                }
                Op::Jump(target) => {
                    self.current_frame_mut()?.ip = target;
                }
                Op::JumpIfFalse(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if !v.is_truthy() {
                        self.current_frame_mut()?.ip = target;
                    }
                }
                Op::JumpIfTrue(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if v.is_truthy() {
                        self.current_frame_mut()?.ip = target;
                    }
                }
                Op::JumpIfNullish(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if v.is_nullish() {
                        self.current_frame_mut()?.ip = target;
                    }
                }
                Op::JumpIfNotNullish(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if !v.is_nullish() {
                        self.current_frame_mut()?.ip = target;
                    }
                }
                Op::Return => {
                    let stack_base = self.current_frame()?.stack_base;
                    let v = if self.stack.len() > stack_base {
                        self.stack.pop().unwrap_or(Value::Undefined)
                    } else {
                        Value::Undefined
                    };
                    // If a `finally` is active, suspend the return across it:
                    // record the completion (tag 1) and divert to the finally
                    // target, popping the finally entry so the finally body's
                    // own transfers aren't re-intercepted by this finally.
                    if let Some(frame) = self.frames.last_mut() {
                        if let Some(&(target, fseq)) = frame.finally_stack.last() {
                            Self::discard_catches_inside_finally(frame, fseq);
                            frame.finally_completion_tag.store(1, Ordering::Relaxed);
                            *frame.finally_completion_val.lock() = v;
                            frame.ip = target;
                            continue;
                        }
                    }
                    self.stack.truncate(stack_base);
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
                    let stack_base = self.current_frame()?.stack_base;
                    self.stack.truncate(stack_base);
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
                    let idx = self.alloc(obj)?;
                    self.stack.push(Value::Object(idx));
                }
                Op::NewArray(count) => {
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    items.reverse();
                    let obj = HeapObj::Array(crate::value::ArrayData::new(
                        items,
                        Some(self.array_proto.clone()),
                    ));
                    let idx = self.alloc(obj)?;
                    self.stack.push(Value::Object(idx));
                }
                Op::ArrayPush => {
                    // stack: [array, value]; append value to the array's items.
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let arr = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                a.items.lock().push(value.clone());
                                a.present.lock().push(true);
                            }
                        });
                    }
                    self.stack.push(arr);
                }
                Op::ArrayHolePush => {
                    // stack: [array]; append an elision hole.
                    let arr = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                a.items.lock().push(Value::Undefined);
                                a.present.lock().push(false);
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
                                    a.items.lock().push(v.clone());
                                    a.present.lock().push(true);
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
                    if matches!((&dest, &src), (Value::Object(_), Value::Object(_))) {
                        let keys = crate::builtins::own_property_keys_or_throw(
                            self, &src, false, true, true,
                        )?;
                        for key in keys {
                            if !crate::builtins::own_property_descriptor_for_key_or_throw(
                                self, &src, &key,
                            )?
                            .is_some_and(|desc| desc.enumerable)
                            {
                                continue;
                            }
                            let v = self.get_property_by_key(&src, &key)?;
                            self.define_data_property(&dest, key, v)?;
                        }
                    }
                    self.stack.push(dest);
                }
                Op::ObjRest(count) => {
                    // stack: [src, k1..kN]; new obj with src's own enum props except k1..kN
                    let mut excluded: Vec<crate::value::PropertyKey> = Vec::with_capacity(count);
                    for _ in 0..count {
                        if let Some(v) = self.stack.pop() {
                            excluded.push(self.coerce_property_key_record(&v)?);
                        }
                    }
                    let src = self.stack.pop().unwrap_or(Value::Undefined);
                    let new_obj = Value::Object(self.new_object()?);
                    if matches!(src, Value::Null | Value::Undefined) {
                        return Err(Error::type_err(format!(
                            "Cannot destructure {}",
                            src.type_of()
                        )));
                    }
                    let src_obj = self.to_object(&src)?;
                    if matches!(src_obj, Value::Object(_)) {
                        let keys = crate::builtins::own_property_keys_or_throw(
                            self, &src_obj, false, true, true,
                        )?;
                        for key in keys {
                            if excluded.contains(&key) {
                                continue;
                            }
                            if !crate::builtins::own_property_descriptor_for_key_or_throw(
                                self, &src_obj, &key,
                            )?
                            .is_some_and(|desc| desc.enumerable)
                            {
                                continue;
                            }
                            let v = self.get_property_by_key(&src_obj, &key)?;
                            self.define_data_property(&new_obj, key, v)?;
                        }
                    }
                    self.stack.push(new_obj);
                }
                Op::SetFunctionNameFromKey(prefix_kind) => {
                    let Some(value) = self.stack.last().cloned() else {
                        continue;
                    };
                    let Some(key) = self.stack.get(self.stack.len().saturating_sub(2)).cloned()
                    else {
                        continue;
                    };
                    let pkey = self.property_key_from_value(&key)?;
                    let prefix = match prefix_kind {
                        1 => Some("get "),
                        2 => Some("set "),
                        _ => None,
                    };
                    self.set_empty_function_name_from_property_key(&value, &pkey, prefix);
                }
                Op::SetFunctionNameConst(name_idx) => {
                    let Some(value) = self.stack.last().cloned() else {
                        continue;
                    };
                    let name = {
                        let frame = self.current_frame()?;
                        match &frame.chunk.constants[name_idx] {
                            Value::String(s) => s.clone(),
                            _ => Arc::from(""),
                        }
                    };
                    self.set_empty_function_name(&value, name);
                }
                Op::DefineAccessor(kind) => {
                    // stack: [obj, key, fn]; define getter(0) or setter(1).
                    let func = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_method_home_object(&func, &obj);
                    if let Value::Object(idx) = &obj {
                        let pkey = self.property_key_from_value(&key_val)?;
                        self.heap.with_obj(idx.0, |o| {
                            let props = o.props();
                            let mut props = props.lock();
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
                Op::DefineClassAccessor(kind) => {
                    // Same as DefineAccessor but enumerable=false (class methods).
                    let func = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_val = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_method_home_object(&func, &obj);
                    if let Value::Object(idx) = &obj {
                        let pkey = self.property_key_from_value(&key_val)?;
                        let current = self
                            .heap
                            .with_obj(idx.0, |o| o.props().lock().get(&pkey).cloned());
                        let mut desc = current.unwrap_or(crate::value::PropertyDescriptor {
                            value: Value::Undefined,
                            writable: false,
                            enumerable: false,
                            configurable: true,
                            get: None,
                            set: None,
                            is_accessor: true,
                        });
                        desc.value = Value::Undefined;
                        desc.is_accessor = true;
                        desc.writable = false;
                        desc.enumerable = false;
                        desc.configurable = true;
                        if kind == 0 {
                            desc.get = Some(func.clone());
                        } else {
                            desc.set = Some(func.clone());
                        }
                        self.define_own_property_or_throw(&obj, pkey, desc)?;
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
                    // Inline cache fast-path: if the object is the same heap
                    // index as the last GetProp on this key, skip the proto
                    // chain walk. The cache is per-property-name, stored in
                    // a small VM-level HashMap. This helps tight loops like
                    // `for (let i = 0; i < arr.length; i++)` where the same
                    // property is read repeatedly.
                    let v = if let Value::Object(idx) = &obj {
                        let cacheable_own_data = self.heap.with_obj(idx.0, |o| {
                            o.props()
                                .lock()
                                .get(&crate::value::PropertyKey::from(key_str.as_str()))
                                .is_some_and(|desc| !desc.is_accessor)
                        });
                        if cacheable_own_data {
                            if let Some(cached) = self.ic_get(idx.0, &key_str) {
                                cached
                            } else {
                                let val = self.get_property(&obj, &key_str)?;
                                self.ic_put(idx.0, &key_str, val.clone());
                                val
                            }
                        } else {
                            self.get_property(&obj, &key_str)?
                        }
                    } else {
                        self.get_property(&obj, &key_str)?
                    };
                    self.stack.push(v);
                }
                Op::GetElem => {
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let v = self.get_property_key(&obj, &key)?;
                    self.stack.push(v);
                }
                Op::DefineDataProperty => {
                    // stack (bottom->top): [obj, key, value]
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_method_home_object(&value, &obj);
                    let (pkey, cache_key) = match &key {
                        Value::Symbol(id) => (crate::value::PropertyKey::Symbol(*id), None),
                        _ => {
                            let key_str = self.to_property_key(&key)?;
                            (
                                crate::value::PropertyKey::from(key_str.as_str()),
                                Some(key_str),
                            )
                        }
                    };
                    self.define_data_property(&obj, pkey, value.clone())?;
                    if let (Value::Object(idx), Some(key_str)) = (&obj, cache_key) {
                        self.ic_invalidate(idx.0, &key_str);
                    }
                    self.stack.push(value);
                }
                Op::DefineMethod => {
                    // stack (bottom->top): [obj, key, value]
                    // Define a non-enumerable, configurable, writable data property.
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_method_home_object(&value, &obj);
                    let pkey = match &key {
                        Value::Symbol(id) => crate::value::PropertyKey::Symbol(*id),
                        _ => crate::value::PropertyKey::from(self.to_property_key(&key)?),
                    };
                    if let Value::Object(idx) = &obj {
                        let mut desc = crate::value::PropertyDescriptor::data(value.clone());
                        desc.enumerable = false;
                        desc.configurable = true;
                        desc.writable = true;
                        self.define_own_property_or_throw(&obj, pkey.clone(), desc)?;
                        if let crate::value::PropertyKey::Str(key_str) = &pkey {
                            self.ic_invalidate(idx.0, key_str.as_ref());
                        }
                    }
                    self.stack.push(value);
                }
                Op::DeleteValue => {
                    let reference = self.stack.pop().unwrap_or(Value::Undefined);
                    let deleted = self.delete_value(&reference)?;
                    self.stack.push(Value::Bool(deleted));
                }
                Op::ValidateExtends => {
                    // Pop the superclass value and validate it. `extends null`
                    // is special: the instance prototype parent is null, while
                    // the constructor inherits from %Function.prototype%.
                    // Otherwise the superclass must be a constructor whose
                    // .prototype is an object or null. Leave both constructor
                    // parent and already-read prototype parent on the stack so
                    // class definition invokes prototype getters once.
                    let parent = self.stack.pop().unwrap_or(Value::Undefined);
                    if matches!(parent, Value::Null) {
                        self.stack.push(self.function_proto.clone());
                        self.stack.push(Value::Null);
                        continue;
                    }
                    if !self.is_constructor_value(&parent) {
                        self.stack.push(parent);
                        return Err(Error::type_err("Class extends value is not a constructor"));
                    }
                    // Check parent.prototype is an object or null.
                    let proto = self.get_property(&parent, "prototype")?;
                    if !matches!(proto, Value::Object(_) | Value::Null) {
                        self.stack.push(parent);
                        return Err(Error::type_err(
                            "Class extends value's prototype is not an object or null",
                        ));
                    }
                    self.stack.push(parent);
                    self.stack.push(proto);
                }
                Op::Inc => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    match v {
                        Value::BigInt(n) => self.stack.push(Value::BigInt(n + 1)),
                        _ => {
                            let n = self.to_number(&v)?;
                            self.stack.push(Value::Number(n + 1.0));
                        }
                    }
                }
                Op::Dec => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    match v {
                        Value::BigInt(n) => self.stack.push(Value::BigInt(n - 1)),
                        _ => {
                            let n = self.to_number(&v)?;
                            self.stack.push(Value::Number(n - 1.0));
                        }
                    }
                }
                Op::SetProto => {
                    // stack (top->bottom): [proto, obj]; set obj's [[Prototype]] to proto.
                    let proto = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    if let Value::Object(idx) = &obj {
                        match proto {
                            Value::Object(_) => {
                                self.heap.with_obj(idx.0, |o| {
                                    *o.proto().lock() = Some(proto);
                                });
                            }
                            Value::Null => {
                                self.heap.with_obj(idx.0, |o| {
                                    *o.proto().lock() = None;
                                });
                            }
                            _ => {}
                        }
                    }
                }
                Op::GetProto => {
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let proto = match &obj {
                        Value::Object(idx) => self
                            .heap
                            .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)),
                        _ => Value::Null,
                    };
                    self.stack.push(proto);
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
                    let mut catch_stack_target = None;
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
                                (Some(&(_, fseq)), Some(&(_, cseq, _, _))) => fseq > cseq,
                                _ => false,
                            };
                        if divert_to_finally {
                            let target =
                                frame
                                    .finally_stack
                                    .last()
                                    .map(|(ip, _)| *ip)
                                    .ok_or_else(|| {
                                        crate::error::Error::internal(
                                            "finally stack empty during throw diversion",
                                        )
                                    })?;
                            frame.finally_completion_tag.store(4, Ordering::Relaxed);
                            *frame.finally_completion_val.lock() = v;
                            frame.ip = target;
                            continue;
                        }
                        if let Some((handler, _, saved_env, saved_stack_depth)) =
                            frame.catch_stack.pop()
                        {
                            // A throw from inside a running finally replaces
                            // the completion that originally entered that
                            // finally. If an outer catch handles this new
                            // throw, no stale pending completion may remain.
                            frame.finally_completion_tag.store(0, Ordering::Relaxed);
                            *frame.finally_completion_val.lock() = Value::Undefined;
                            // Restore env to try-entry point, unwinding any
                            // scopes/with-envs opened in the try body.
                            frame.env = saved_env;
                            frame.ip = handler;
                            catch_stack_target = Some(frame.stack_base + saved_stack_depth);
                        }
                    }
                    if let Some(stack_target) = catch_stack_target {
                        self.stack.truncate(stack_target);
                        self.stack.push(v);
                        continue;
                    }
                    return Err(Error::thrown(v, &self.heap));
                }
                Op::ThrowReference(msg_idx) => {
                    let message = {
                        let frame = self.current_frame()?;
                        let v = frame
                            .chunk
                            .constants
                            .get(msg_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => s.to_string(),
                            _ => "ReferenceError".to_string(),
                        }
                    };
                    return Err(Error::reference(message));
                }
                Op::PushTry(handler) => {
                    let stack_depth = {
                        let frame = self.current_frame()?;
                        self.stack.len().saturating_sub(frame.stack_base)
                    };
                    let f = self.current_frame_mut()?;
                    let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                    f.guard_seq.store(seq, Ordering::Relaxed);
                    let saved_env = f.env;
                    f.catch_stack.push((handler, seq, saved_env, stack_depth));
                }
                Op::PopTry => {
                    let f = self.current_frame_mut()?;
                    f.catch_stack.pop();
                }
                Op::PushFinally(target) => {
                    // Begin guarding try/catch with a finally: record the
                    // finally entry so non-local transfers divert to it.
                    let f = self.current_frame_mut()?;
                    let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                    f.guard_seq.store(seq, Ordering::Relaxed);
                    f.finally_stack.push((target, seq));
                }
                Op::PopFinally => {
                    // The guarded region completed normally; drop the finally
                    // guard. A pending completion from inside the region was
                    // already popped when the transfer diverted to finally.
                    self.current_frame_mut()?.finally_stack.pop();
                }
                Op::DivertBreak(finally_start) => {
                    let resume_ip = ip + 1;
                    let f = self.current_frame_mut()?;
                    if let Some(&(_, fseq)) = f.finally_stack.last() {
                        Self::discard_catches_inside_finally(f, fseq);
                    }
                    f.finally_completion_tag.store(2, Ordering::Relaxed);
                    *f.finally_completion_val.lock() = Value::Number(resume_ip as f64);
                    f.ip = finally_start;
                    continue;
                }
                Op::DivertContinue(finally_start, cont) => {
                    // A `continue` inside an active try/finally: record the
                    // completion as a continue with the loop's continue target,
                    // and divert to the finally body.
                    let f = self.current_frame_mut()?;
                    if let Some(&(_, fseq)) = f.finally_stack.last() {
                        Self::discard_catches_inside_finally(f, fseq);
                    }
                    f.finally_completion_tag.store(3, Ordering::Relaxed);
                    *f.finally_completion_val.lock() = Value::Number(cont as f64);
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
                Op::CallThisSpread => {
                    // stack: [..., this, fn, argsArray]
                    let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
                    let func = self.stack.pop().unwrap_or(Value::Undefined);
                    let this = self.stack.pop().unwrap_or(Value::Undefined);
                    let mut args = Vec::new();
                    if let Value::Object(idx) = &args_arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                args = a.items.lock().clone();
                            }
                        });
                    }
                    let result = self.call_function(&func, &args, Some(this))?;
                    self.stack.push(result);
                }
                Op::CreatePrivateName(name_idx) => {
                    let description = {
                        let frame = self.current_frame()?;
                        match &frame.chunk.constants[name_idx] {
                            Value::String(s) => s.clone(),
                            _ => Arc::from(""),
                        }
                    };
                    let id = self.next_private_name_id;
                    self.next_private_name_id = self.next_private_name_id.saturating_add(1);
                    self.stack
                        .push(Value::PrivateName(crate::value::PrivateNameKey {
                            id,
                            description,
                        }));
                }
                Op::InitPrivate(name_idx) => {
                    let key = self.private_slot_key_from_name(name_idx)?;
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.add_private_element(
                        &obj,
                        key,
                        crate::value::PrivateSlot::Value(value.clone()),
                        "Cannot initialize private field twice",
                    )?;
                    self.stack.push(value);
                }
                Op::InitPrivateMethod(name_idx) => {
                    let key = self.private_slot_key_from_name(name_idx)?;
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.add_private_element(
                        &obj,
                        key,
                        crate::value::PrivateSlot::Method(value.clone()),
                        "Cannot initialize private method twice",
                    )?;
                    self.stack.push(value);
                }
                Op::DefinePrivateAccessor(name_idx) => {
                    let key = self.private_slot_key_from_name(name_idx)?;
                    let setter = self.stack.pop().unwrap_or(Value::Undefined);
                    let getter = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.define_private_accessor_element(&obj, key, getter, setter)?;
                    self.stack.push(Value::Undefined);
                }
                Op::PopFinallyRethrow => {
                    // The finally body has run. Re-raise the pending
                    // completion (return/break/continue/throw) that diverted
                    // here, if any. A normal completion (tag 0) falls through.
                    let (tag, val) = {
                        let f = self.current_frame()?;
                        (
                            f.finally_completion_tag.load(Ordering::Relaxed),
                            f.finally_completion_val.lock().clone(),
                        )
                    };
                    {
                        let f = self.current_frame_mut()?;
                        f.finally_completion_tag.store(0, Ordering::Relaxed);
                        *f.finally_completion_val.lock() = Value::Undefined;
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
                                    *frame.finally_completion_val.lock() = val.clone();
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
                            let catch_stack_target = {
                                let frame = self.current_frame_mut()?;
                                // If an outer finally still guards this scope,
                                // divert the throw through it first.
                                let divert_to_outer_finally =
                                    match (frame.finally_stack.last(), frame.catch_stack.last()) {
                                        (Some(&(_, _)), None) => true,
                                        (Some(&(_, fseq)), Some(&(_, cseq, _, _))) => fseq > cseq,
                                        _ => false,
                                    };
                                if divert_to_outer_finally {
                                    let outer =
                                        frame.finally_stack.last().map(|(ip, _)| *ip).ok_or_else(
                                            || {
                                                crate::error::Error::internal(
                                                    "finally stack empty during throw diversion",
                                                )
                                            },
                                        )?;
                                    frame.finally_completion_tag.store(4, Ordering::Relaxed);
                                    *frame.finally_completion_val.lock() = val.clone();
                                    frame.ip = outer;
                                    None
                                } else if let Some((handler, _, saved_env, saved_stack_depth)) =
                                    frame.catch_stack.pop()
                                {
                                    frame.finally_completion_tag.store(0, Ordering::Relaxed);
                                    *frame.finally_completion_val.lock() = Value::Undefined;
                                    frame.env = saved_env;
                                    frame.ip = handler;
                                    Some(frame.stack_base + saved_stack_depth)
                                } else {
                                    return Err(Error::thrown(val, &self.heap));
                                }
                            };
                            if let Some(stack_target) = catch_stack_target {
                                self.stack.truncate(stack_target);
                                self.stack.push(val);
                                continue;
                            }
                            continue;
                        }
                        // 2 (break) / 3 (continue): re-issue the recorded
                        // transfer by jumping to its saved target. These are
                        // recorded as the loop's break/continue ip.
                        2 | 3 => {
                            let frame = self.current_frame_mut()?;
                            // If an outer finally still guards this scope,
                            // divert the break/continue through it first.
                            if let Some(&(outer, _)) = frame.finally_stack.last() {
                                frame.finally_completion_tag.store(tag, Ordering::Relaxed);
                                *frame.finally_completion_val.lock() = val.clone();
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
                Op::ApplyDecoratorResult(kind) => {
                    let result = self.stack.pop().unwrap_or(Value::Undefined);
                    let original = self.stack.pop().unwrap_or(Value::Undefined);
                    if matches!(result, Value::Undefined) {
                        self.stack.push(original);
                    } else {
                        let valid = match kind {
                            0 => self.is_constructor_value(&result),
                            1 | 2 => crate::builtins::is_callable(&result, &self.heap),
                            _ => false,
                        };
                        if !valid {
                            return Err(Error::type_err(match kind {
                                0 => "Class decorator must return a constructor or undefined",
                                2 => "Field decorator must return a function or undefined",
                                _ => "Element decorator must return a function or undefined",
                            }));
                        }
                        self.stack.push(result);
                    }
                }
                Op::DecoratorAddInitializer => {
                    let initializer = self.stack.pop().unwrap_or(Value::Undefined);
                    let queue = self.stack.pop().unwrap_or(Value::Undefined);
                    let active = self.stack.pop().unwrap_or(Value::Undefined);
                    if !matches!(active, Value::Bool(true)) {
                        return Err(Error::type_err(
                            "addInitializer cannot be called after decoration has completed",
                        ));
                    }
                    if !crate::builtins::is_callable(&initializer, &self.heap) {
                        return Err(Error::type_err(
                            "addInitializer requires a callable initializer",
                        ));
                    }
                    let appended = if let Value::Object(idx) = &queue {
                        self.heap.with_obj(idx.0, |object| {
                            if let HeapObj::Array(array) = object {
                                array.items.lock().push(initializer.clone());
                                array.present.lock().push(true);
                                true
                            } else {
                                false
                            }
                        })
                    } else {
                        false
                    };
                    if !appended {
                        return Err(Error::internal(
                            "decorator initializer queue is not an array",
                        ));
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::DecoratorAccess(kind) => {
                    let value = if kind == 2 {
                        Some(self.stack.pop().unwrap_or(Value::Undefined))
                    } else {
                        None
                    };
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let receiver = self.stack.pop().unwrap_or(Value::Undefined);
                    if !matches!(receiver, Value::Object(_)) {
                        return Err(Error::type_err(
                            "Decorator access receiver must be an object",
                        ));
                    }
                    if let Value::PrivateName(private_name) = &key {
                        match kind {
                            0 => {
                                let found = if let Value::Object(idx) = &receiver {
                                    let slot_key =
                                        crate::value::PrivateSlotKey::Private(private_name.clone());
                                    self.heap.get_private_element(idx.0, &slot_key).is_some()
                                } else {
                                    false
                                };
                                self.stack.push(Value::Bool(found));
                            }
                            1 => {
                                let result = self.get_private_value(&receiver, private_name)?;
                                self.stack.push(result);
                            }
                            2 => {
                                self.set_private_value(
                                    &receiver,
                                    private_name,
                                    value.unwrap_or(Value::Undefined),
                                )?;
                                self.stack.push(Value::Undefined);
                            }
                            _ => return Err(Error::internal("invalid decorator access kind")),
                        }
                        continue;
                    }
                    match kind {
                        0 => {
                            let property_key = match &key {
                                Value::Symbol(id) => crate::value::PropertyKey::Symbol(*id),
                                _ => {
                                    let key_string = self.to_property_key(&key)?;
                                    crate::value::PropertyKey::from(key_string.as_str())
                                }
                            };
                            let found = self.has_property_key(&receiver, &property_key)?;
                            self.stack.push(Value::Bool(found));
                        }
                        1 => {
                            let result = self.get_property_key(&receiver, &key)?;
                            self.stack.push(result);
                        }
                        2 => {
                            self.set_property_key(
                                &receiver,
                                &key,
                                value.unwrap_or(Value::Undefined),
                            )?;
                            self.stack.push(Value::Undefined);
                        }
                        _ => return Err(Error::internal("invalid decorator access kind")),
                    }
                }
                Op::ExtractAccessorDecoratorResult => {
                    let result = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    if result.is_undefined() {
                        self.stack.pop();
                        self.stack
                            .extend([Value::Undefined, Value::Undefined, Value::Undefined]);
                        continue;
                    }
                    if !matches!(result, Value::Object(_)) {
                        return Err(Error::type_err(
                            "Accessor decorator must return an object or undefined",
                        ));
                    }
                    for name in ["get", "set", "init"] {
                        let replacement = self.get_property(&result, name)?;
                        if !replacement.is_undefined()
                            && !crate::builtins::is_callable(&replacement, &self.heap)
                        {
                            return Err(Error::type_err(format!(
                                "Accessor decorator '{}' replacement must be callable or undefined",
                                name
                            )));
                        }
                        self.stack.push(replacement);
                    }
                    let result_index = self.stack.len().saturating_sub(4);
                    self.stack.remove(result_index);
                }
                Op::CallRef(arg_count) => self.op_call_ref(arg_count)?,
                Op::CallMethod(arg_count) => self.op_call_method(arg_count)?,
                Op::CallEval(arg_count) => self.op_call_eval(arg_count)?,
                Op::CallEvalRef(arg_count) => self.op_call_eval_ref(arg_count)?,
                Op::CallEvalClassField(arg_count) => {
                    self.op_call_eval_with_context(arg_count, true)?
                }
                Op::CallEvalRefClassField(arg_count) => {
                    self.op_call_eval_ref_with_context(arg_count, true)?
                }
                Op::ImportCall { has_options } => self.op_import_call(has_options)?,
                Op::ImportMeta => {
                    let path = self.current_frame()?.chunk.source_path.clone();
                    let meta =
                        if let Some(path) = path {
                            self.import_meta_object(&path)?
                        } else {
                            Value::Object(self.current_frame()?.chunk.import_meta.ok_or_else(
                                || Error::syntax("import.meta requires module source"),
                            )?)
                        };
                    self.stack.push(meta);
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
                        .map(|f| f.gen_mode.load(Ordering::Relaxed))
                        .unwrap_or(false);
                    if in_gen {
                        let frame = self.current_frame()?;
                        *frame.gen_yield.lock() = Some(v);
                        frame
                            .gen_yield_is_iterator_result
                            .store(false, Ordering::Relaxed);
                        frame.gen_delegating.store(false, Ordering::Relaxed);
                        frame.gen_suspended.store(true, Ordering::Relaxed);
                        return Ok(Value::Undefined);
                    } else {
                        // Not in a generator context (shouldn't happen): behave eagerly.
                        self.current_yields.push(v);
                        self.stack.push(Value::Undefined);
                    }
                }
                Op::CallSuperCtor(arg_count) => {
                    // stack: [this, homeCtor, args...]; call homeCtor.[[Prototype]] with this.
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let home_ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let _placeholder = self.stack.pop().unwrap_or(Value::Undefined);
                    let super_ctor = match &home_ctor {
                        Value::Object(idx) => self
                            .heap
                            .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)),
                        _ => Value::Undefined,
                    };
                    let is_function_prototype = match (&super_ctor, &self.function_proto) {
                        (Value::Object(a), Value::Object(b)) => a == b,
                        _ => false,
                    };
                    if is_function_prototype || !self.is_constructor_value(&super_ctor) {
                        return Err(Error::type_err("not a constructor"));
                    }
                    let (this_env, _placeholder, new_target) =
                        self.prepare_super_constructor_call()?;
                    // `super()` performs [[Construct]], so BoundFunction and
                    // Proxy superclasses must use their construct semantics.
                    // It forwards the active constructor's new.target.
                    let forwarded_new_target = if matches!(new_target, Value::Undefined) {
                        super_ctor.clone()
                    } else {
                        new_target
                    };
                    let new_this =
                        self.construct_with_new_target(&super_ctor, &args, &forwarded_new_target)?;
                    // BindThisValue happens after Construct. If `this` was
                    // already initialized, the superclass constructor has
                    // still run and this step throws ReferenceError.
                    self.bind_super_constructor_result(this_env, new_this.clone())?;
                    self.stack.push(new_this);
                }
                Op::CallSuperCtorSpread => {
                    // stack: [this, homeCtor, argsArray]
                    let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
                    let home_ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let _placeholder = self.stack.pop().unwrap_or(Value::Undefined);
                    let super_ctor = match &home_ctor {
                        Value::Object(idx) => self
                            .heap
                            .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)),
                        _ => Value::Undefined,
                    };
                    // Expand the array into individual args.
                    let args = if let Value::Object(idx) = &args_arr {
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                a.items.lock().clone()
                            } else {
                                Vec::new()
                            }
                        })
                    } else {
                        Vec::new()
                    };
                    let is_function_prototype = match (&super_ctor, &self.function_proto) {
                        (Value::Object(a), Value::Object(b)) => a == b,
                        _ => false,
                    };
                    if is_function_prototype || !self.is_constructor_value(&super_ctor) {
                        return Err(Error::type_err("not a constructor"));
                    }
                    let (this_env, _placeholder, new_target) =
                        self.prepare_super_constructor_call()?;
                    let forwarded_new_target = if matches!(new_target, Value::Undefined) {
                        super_ctor.clone()
                    } else {
                        new_target
                    };
                    let new_this =
                        self.construct_with_new_target(&super_ctor, &args, &forwarded_new_target)?;
                    self.bind_super_constructor_result(this_env, new_this.clone())?;
                    self.stack.push(new_this);
                }
                Op::CallSpread => self.op_call_spread()?,
                Op::CallRefSpread => self.op_call_ref_spread()?,
                Op::CallEvalSpread => self.op_call_eval_spread()?,
                Op::CallEvalRefSpread => self.op_call_eval_ref_spread()?,
                Op::CallEvalSpreadClassField => self.op_call_eval_spread_with_context(true)?,
                Op::CallEvalRefSpreadClassField => {
                    self.op_call_eval_ref_spread_with_context(true)?
                }
                Op::New(arg_count) => self.op_new(arg_count)?,
                Op::NewSpread => self.op_new_spread()?,
                Op::MakeClosure(func_idx) => self.op_make_closure(func_idx)?,
                Op::MakeClass(func_idx) => {
                    self.op_make_closure(func_idx)?;
                    // Mark the function on top of the stack as a class constructor
                    // and make its .prototype non-writable (per spec: class
                    // constructors have a non-writable prototype).
                    if let Some(&Value::Object(idx)) = self.stack.last() {
                        self.heap.with_obj(idx.0, |obj| {
                            if let HeapObj::Function(f) = obj {
                                f.is_class_ctor
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                                // Class prototype: non-writable, non-enumerable, non-configurable.
                                if let Some(pd) = f
                                    .props
                                    .lock()
                                    .get_mut(&crate::value::PropertyKey::from("prototype"))
                                {
                                    pd.writable = false;
                                }
                            }
                        });
                    }
                }
                Op::TypeOf => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let t = if let Value::Object(idx) = &v {
                        if crate::builtins::is_callable(&Value::Object(*idx), &self.heap) {
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
                Op::ToNumeric => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    let n = self.to_numeric(&v)?;
                    self.stack.push(n);
                }
                Op::Await => {
                    if self.op_await()? {
                        return Ok(Value::Undefined);
                    }
                }
                Op::TypeofVar(name_idx) => {
                    // `typeof name`: "undefined" if the name is unbound, but
                    // TDZ bindings still throw ReferenceError.
                    let name_key = {
                        let frame = self.current_frame()?;
                        let v = frame
                            .chunk
                            .constants
                            .get(name_idx)
                            .cloned()
                            .unwrap_or(Value::Undefined);
                        match v {
                            Value::String(s) => crate::value::PropertyKey::from_rc(s),
                            _ => crate::value::PropertyKey::from(""),
                        }
                    };
                    let r#ref =
                        self.resolve_identifier_reference(name_key, self.current_strict())?;
                    let val = if matches!(r#ref.base, crate::value::ReferenceBase::Unresolvable) {
                        None
                    } else {
                        Some(self.get_value(&Value::Reference(Box::new(r#ref)))?)
                    };
                    let t = if let Some(v) = val {
                        if let Value::Object(idx) = &v {
                            if crate::builtins::is_callable(&Value::Object(*idx), &self.heap) {
                                "function"
                            } else {
                                "object"
                            }
                        } else {
                            v.type_of()
                        }
                    } else {
                        "undefined"
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
                op @ (Op::YieldDelegate | Op::YieldDelegateAsync) => {
                    let await_result = matches!(op, Op::YieldDelegateAsync);
                    let completion = self.current_frame()?.gen_delegate_resume.lock().take();
                    let completion = completion.unwrap_or(ResumeKind::Next(Value::Undefined));
                    let iterator = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    let outcome =
                        match self.iterator_delegate_step(&iterator, completion, await_result) {
                            Ok(outcome) => outcome,
                            Err(error) => {
                                let frame = self.current_frame()?;
                                frame.gen_delegating.store(false, Ordering::Relaxed);
                                frame
                                    .gen_yield_is_iterator_result
                                    .store(false, Ordering::Relaxed);
                                frame.gen_delegate_await_kind.store(0, Ordering::Relaxed);
                                return Err(error);
                            }
                        };
                    match outcome {
                        DelegateOutcome::Yield(result) => {
                            let resume_ip = self.current_frame()?.ip.saturating_sub(1);
                            self.current_frame_mut()?.ip = resume_ip;
                            let frame = self.current_frame()?;
                            *frame.gen_yield.lock() = Some(result);
                            frame
                                .gen_yield_is_iterator_result
                                .store(true, Ordering::Relaxed);
                            frame.gen_delegating.store(true, Ordering::Relaxed);
                            frame.gen_delegate_await_kind.store(0, Ordering::Relaxed);
                            frame.gen_suspended.store(true, Ordering::Relaxed);
                            return Ok(Value::Undefined);
                        }
                        DelegateOutcome::Complete(value) => {
                            self.stack.pop();
                            self.stack.push(value);
                            let frame = self.current_frame()?;
                            frame.gen_delegating.store(false, Ordering::Relaxed);
                            frame
                                .gen_yield_is_iterator_result
                                .store(false, Ordering::Relaxed);
                            frame.gen_delegate_await_kind.store(0, Ordering::Relaxed);
                        }
                        DelegateOutcome::Return(value) => {
                            self.stack.pop();
                            let frame = self.current_frame()?;
                            frame.gen_delegating.store(false, Ordering::Relaxed);
                            frame
                                .gen_yield_is_iterator_result
                                .store(false, Ordering::Relaxed);
                            frame.gen_delegate_await_kind.store(0, Ordering::Relaxed);
                            *frame.force_return.lock() = Some(value);
                            continue;
                        }
                        DelegateOutcome::Await(value, kind) => {
                            let resume_ip = self.current_frame()?.ip.saturating_sub(1);
                            self.current_frame_mut()?.ip = resume_ip;
                            let frame = self.current_frame()?;
                            *frame.gen_yield.lock() = Some(value);
                            frame
                                .gen_yield_is_iterator_result
                                .store(false, Ordering::Relaxed);
                            frame.gen_delegating.store(true, Ordering::Relaxed);
                            frame.gen_delegate_await_kind.store(
                                match kind {
                                    DelegateAwaitKind::Result => 1,
                                    DelegateAwaitKind::ReturnResult => 2,
                                    DelegateAwaitKind::MissingThrow => 3,
                                },
                                Ordering::Relaxed,
                            );
                            frame.gen_awaiting.store(true, Ordering::Relaxed);
                            frame.gen_suspended.store(true, Ordering::Relaxed);
                            return Ok(Value::Undefined);
                        }
                    }
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
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let result = self.iterator_next_await_start(&it)?;
                    self.stack.push(it);
                    self.stack.push(result);
                }
                Op::IteratorUnpackAwait => {
                    let result = self.stack.pop().unwrap_or(Value::Undefined);
                    let it = self.stack.pop().unwrap_or(Value::Undefined);
                    let (value, done) = self.iterator_unpack_await_result(&it, result)?;
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
                    let arr = HeapObj::Array(crate::value::ArrayData::new(
                        items,
                        Some(self.array_proto.clone()),
                    ));
                    let val = Value::Object(self.alloc(arr)?);
                    self.stack.push(val);
                }
                Op::IteratorCloseIfAbrupt {
                    iter,
                    done,
                    inner_continue,
                    ignore_close_errors,
                } => {
                    let (completion_tag, completion_val) = {
                        let frame = self.current_frame()?;
                        (
                            frame.finally_completion_tag.load(Ordering::Relaxed),
                            frame.finally_completion_val.lock().clone(),
                        )
                    };
                    let continue_target = match (completion_tag, completion_val) {
                        (3, Value::Number(n)) => {
                            let frame = self.current_frame()?;
                            let mut target = n as usize;
                            loop {
                                match frame.chunk.code.get(target) {
                                    Some(Op::PopScope | Op::PopWithEnv) => target += 1,
                                    Some(Op::Jump(next)) => break *next,
                                    _ => break target,
                                }
                            }
                        }
                        _ => usize::MAX,
                    };
                    let stays_in_loop = inner_continue == Some(continue_target);
                    let should_close = completion_tag != 0 && !stays_in_loop;
                    if should_close {
                        let (iter_name, done_name) = {
                            let frame = self.current_frame()?;
                            let iter_name = match frame.chunk.constants.get(iter) {
                                Some(Value::String(s)) => s.to_string(),
                                _ => String::new(),
                            };
                            let done_name = match frame.chunk.constants.get(done) {
                                Some(Value::String(s)) => s.to_string(),
                                _ => String::new(),
                            };
                            (iter_name, done_name)
                        };
                        let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                        let done_value =
                            crate::environment::get_checked(&self.heap, env, &done_name)
                                .ok()
                                .flatten()
                                .unwrap_or(Value::Bool(true));
                        if !matches!(done_value, Value::Bool(true)) {
                            if let Ok(Some(iterator)) =
                                crate::environment::get_checked(&self.heap, env, &iter_name)
                            {
                                let result = self.iterator_close(&iterator);
                                if !ignore_close_errors && completion_tag != 4 {
                                    result?;
                                }
                            }
                        }
                    }
                }
                Op::IteratorClose {
                    iter,
                    done,
                    ignore_close_errors,
                } => {
                    let (iter_name, done_name) = {
                        let frame = self.current_frame()?;
                        let iter_name = match frame.chunk.constants.get(iter) {
                            Some(Value::String(s)) => s.to_string(),
                            _ => String::new(),
                        };
                        let done_name = match frame.chunk.constants.get(done) {
                            Some(Value::String(s)) => s.to_string(),
                            _ => String::new(),
                        };
                        (iter_name, done_name)
                    };
                    let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let done_value = crate::environment::get_checked(&self.heap, env, &done_name)
                        .ok()
                        .flatten()
                        .unwrap_or(Value::Bool(true));
                    if !matches!(done_value, Value::Bool(true)) {
                        if let Ok(Some(iterator)) =
                            crate::environment::get_checked(&self.heap, env, &iter_name)
                        {
                            let result = self.iterator_close(&iterator);
                            if !ignore_close_errors {
                                result?;
                            }
                        }
                    }
                }
                Op::GetTemplateObject(quasi_ids, raw_ids) => {
                    let frame = self.current_frame()?;
                    let chunk_ptr = Arc::as_ptr(&frame.chunk) as usize;
                    let key = (chunk_ptr, frame.ip.saturating_sub(1));
                    if let Some(v) = self.template_cache.get(&key) {
                        self.stack.push(v.clone());
                    } else {
                        let obj =
                            self.make_template_object(quasi_ids.as_slice(), raw_ids.as_slice())?;
                        self.template_cache.insert(key, obj.clone());
                        self.stack.push(obj);
                    }
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

    fn this_value_from_reference(&self, r#ref: &Value) -> Value {
        if let Value::Reference(record) = r#ref {
            if let Some(this_value) = &record.this_value {
                return *this_value.clone();
            }
            match &record.base {
                crate::value::ReferenceBase::ObjectEnvironment(base)
                | crate::value::ReferenceBase::Value(base) => return *base.clone(),
                crate::value::ReferenceBase::Unresolvable
                | crate::value::ReferenceBase::Environment(_) => {}
            }
        }
        Value::Undefined
    }

    fn reject_dynamic_import(
        &mut self,
        reject: &Value,
        error: &Arc<Error>,
    ) -> error::Result<Value> {
        let realm = self.current_realm_global_env();
        let reason = self.promise_rejection_reason_in_realm(error, realm)?;
        let reason_pin = self.pin(&reason);
        let result = self.call_function(
            reject,
            std::slice::from_ref(&reason),
            Some(Value::Undefined),
        );
        self.unpin(reason_pin);
        result
    }

    fn dynamic_import_type_from_options(
        &mut self,
        options: &Value,
    ) -> error::Result<Option<Arc<str>>> {
        if options.is_undefined() {
            return Ok(None);
        }
        if !matches!(options, Value::Object(_)) {
            return Err(Error::type_err("Dynamic import options must be an object"));
        }
        let attributes = self.get_property(options, "with")?;
        if attributes.is_undefined() {
            return Ok(None);
        }
        if !matches!(attributes, Value::Object(_)) {
            return Err(Error::type_err(
                "Dynamic import 'with' attributes must be an object",
            ));
        }
        let attributes_pin = self.pin(&attributes);
        let result = (|| {
            let keys =
                crate::builtins::own_property_keys_or_throw(self, &attributes, false, true, false)?;
            let mut import_type = None;
            for key in keys {
                if !crate::builtins::own_property_descriptor_for_key_or_throw(
                    self,
                    &attributes,
                    &key,
                )?
                .is_some_and(|descriptor| descriptor.enumerable)
                {
                    continue;
                }
                let value = self.get_property_by_key(&attributes, &key)?;
                let Value::String(value) = value else {
                    return Err(Error::type_err(
                        "Dynamic import attribute values must be strings",
                    ));
                };
                if key.as_str() != Some("type") {
                    return Err(Error::type_err("Unsupported dynamic import attribute"));
                }
                import_type = Some(value);
            }
            if import_type
                .as_deref()
                .is_some_and(|value| value != "json" && value != "text")
            {
                return Err(Error::type_err("Unsupported dynamic import type"));
            }
            Ok(import_type)
        })();
        self.unpin(attributes_pin);
        result
    }

    fn op_import_call(&mut self, has_options: bool) -> error::Result<()> {
        let options = if has_options {
            self.stack.pop().unwrap_or(Value::Undefined)
        } else {
            Value::Undefined
        };
        let specifier = self.stack.pop().unwrap_or(Value::Undefined);
        let realm = self.current_realm_global_env();
        let constructor = self.promise_constructor_for_env(realm);
        let capability = crate::builtins::new_promise_capability(self, constructor)?;
        let promise = match capability.promise.clone() {
            Value::Object(promise) => promise,
            _ => {
                return Err(Error::internal(
                    "Promise capability did not create an object",
                ))
            }
        };
        let pins = self.pin_many(&[
            Value::Object(promise),
            capability.resolve.clone(),
            capability.reject.clone(),
            specifier.clone(),
            options.clone(),
        ]);
        let referrer = self
            .current_frame()
            .ok()
            .and_then(|frame| frame.chunk.source_path.clone());
        let settlement = match self.to_string_pub(&specifier) {
            Ok(specifier) => match self.dynamic_import_type_from_options(&options) {
                Ok(import_type) => {
                    if let Some(referrer) = referrer {
                        self.microtask_queue.push_back(Microtask::DynamicImport {
                            promise,
                            resolve: capability.resolve.clone(),
                            reject: capability.reject.clone(),
                            realm,
                            referrer,
                            specifier: specifier.into(),
                            import_type,
                        });
                        Ok(Value::Undefined)
                    } else {
                        let error =
                            Error::type_err("Dynamic import requires a source-file referrer");
                        self.reject_dynamic_import(&capability.reject, &error)
                    }
                }
                Err(error) => self.reject_dynamic_import(&capability.reject, &error),
            },
            Err(error) => self.reject_dynamic_import(&capability.reject, &error),
        };
        self.stack.push(Value::Object(promise));
        self.unpin_many(pins);
        settlement?;
        Ok(())
    }

    /// `Op::Call(arg_count)`: pop callee + args and call with an unbound
    /// `this`. Direct IdentifierReference calls use `CallRef` instead.
    fn op_call(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let result = self.call_function(&callee, &args, Some(Value::Undefined))?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::CallRef(arg_count)`: call a previously resolved Reference. The
    /// retained Reference supplies the spec this-value without reading the
    /// callee a second time.
    fn op_call_ref(&mut self, arg_count: usize) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let r#ref = self.stack.pop().unwrap_or(Value::Undefined);
        let this = self.this_value_from_reference(&r#ref);
        let result = self.call_function(&callee, &args, Some(this))?;
        self.stack.push(result);
        Ok(())
    }

    fn realm_eval_function_for_env(&self, env: GcIdx) -> Option<Value> {
        let mut cur_env = Some(env);
        while let Some(e_idx) = cur_env {
            if let Some(eval) = self.realm_eval_functions.get(&e_idx.0) {
                return Some(eval.clone());
            }
            cur_env = self.heap.with_obj(e_idx.0, |obj| {
                if let HeapObj::Environment(e) = obj {
                    *e.parent.lock()
                } else {
                    None
                }
            });
        }
        None
    }

    fn is_current_realm_eval(&self, callee: &Value, caller_env: GcIdx) -> bool {
        match (callee, self.realm_eval_function_for_env(caller_env)) {
            (Value::Object(a), Some(Value::Object(b))) => a == &b,
            _ => false,
        }
    }

    fn call_direct_eval_from_args(
        &mut self,
        args: &[Value],
        in_class_field_initializer: bool,
    ) -> error::Result<Value> {
        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        let src = match arg {
            Value::String(s) => s.to_string(),
            _ => return Ok(arg),
        };
        let (caller_env, this_val, caller_strict, caller_new_target, new_target_allowed) = self
            .frames
            .last()
            .map(|f| {
                (
                    f.env,
                    f.this_val.clone(),
                    f.chunk.is_strict,
                    f.new_target.clone(),
                    f.direct_eval_new_target_allowed,
                )
            })
            .unwrap_or((
                self.global,
                Value::Undefined,
                false,
                Value::Undefined,
                false,
            ));
        self.eval_direct(
            &src,
            DirectEvalContext {
                caller_env,
                this_val,
                caller_strict,
                caller_new_target,
                new_target_allowed,
                in_class_field_initializer,
            },
        )
    }

    /// `Op::CallEval(arg_count)`: unqualified `eval(...)`. The parser/compiler
    /// can identify the syntactic shape, but only runtime resolution can tell
    /// whether `eval` was shadowed by `with` or a mutable binding.
    fn op_call_eval_with_context(
        &mut self,
        arg_count: usize,
        in_class_field_initializer: bool,
    ) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let caller_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
        let result = if self.is_current_realm_eval(&callee, caller_env) {
            self.call_direct_eval_from_args(&args, in_class_field_initializer)?
        } else {
            self.call_function(&callee, &args, Some(Value::Undefined))?
        };
        self.stack.push(result);
        Ok(())
    }

    fn op_call_eval(&mut self, arg_count: usize) -> error::Result<()> {
        self.op_call_eval_with_context(arg_count, false)
    }

    /// `Op::CallEvalRef(arg_count)`: direct `eval(...)` syntactic form with
    /// its IdentifierReference retained. Intrinsic eval stays direct; a
    /// shadowing callable reached through `with` receives the with object as
    /// `this`.
    fn op_call_eval_ref_with_context(
        &mut self,
        arg_count: usize,
        in_class_field_initializer: bool,
    ) -> error::Result<()> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.stack.pop().unwrap_or(Value::Undefined));
        }
        args.reverse();
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let r#ref = self.stack.pop().unwrap_or(Value::Undefined);
        let caller_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
        let result = if self.is_current_realm_eval(&callee, caller_env) {
            self.call_direct_eval_from_args(&args, in_class_field_initializer)?
        } else {
            let this = self.this_value_from_reference(&r#ref);
            self.call_function(&callee, &args, Some(this))?
        };
        self.stack.push(result);
        Ok(())
    }

    fn op_call_eval_ref(&mut self, arg_count: usize) -> error::Result<()> {
        self.op_call_eval_ref_with_context(arg_count, false)
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

    /// `Op::CallSpread`: spread an array's items as call arguments.
    fn op_call_spread(&mut self) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let mut args = Vec::new();
        if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    args = a.items.lock().clone();
                }
            });
        }
        let result = self.call_function(&callee, &args, Some(Value::Undefined))?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::CallRefSpread`: spread form of a Reference call.
    fn op_call_ref_spread(&mut self) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let r#ref = self.stack.pop().unwrap_or(Value::Undefined);
        let mut args = Vec::new();
        if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    args = a.items.lock().clone();
                }
            });
        }
        let this = self.this_value_from_reference(&r#ref);
        let result = self.call_function(&callee, &args, Some(this))?;
        self.stack.push(result);
        Ok(())
    }

    /// `Op::CallEvalSpread`: spread form of unqualified `eval(...)`.
    fn op_call_eval_spread_with_context(
        &mut self,
        in_class_field_initializer: bool,
    ) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let mut args = Vec::new();
        if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    args = a.items.lock().clone();
                }
            });
        }
        let caller_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
        let result = if self.is_current_realm_eval(&callee, caller_env) {
            self.call_direct_eval_from_args(&args, in_class_field_initializer)?
        } else {
            self.call_function(&callee, &args, Some(Value::Undefined))?
        };
        self.stack.push(result);
        Ok(())
    }

    fn op_call_eval_spread(&mut self) -> error::Result<()> {
        self.op_call_eval_spread_with_context(false)
    }

    /// `Op::CallEvalRefSpread`: spread form of Reference-preserving
    /// unqualified `eval(...)`.
    fn op_call_eval_ref_spread_with_context(
        &mut self,
        in_class_field_initializer: bool,
    ) -> error::Result<()> {
        let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
        let callee = self.stack.pop().unwrap_or(Value::Undefined);
        let r#ref = self.stack.pop().unwrap_or(Value::Undefined);
        let mut args = Vec::new();
        if let Value::Object(idx) = &args_arr {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    args = a.items.lock().clone();
                }
            });
        }
        let caller_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
        let result = if self.is_current_realm_eval(&callee, caller_env) {
            self.call_direct_eval_from_args(&args, in_class_field_initializer)?
        } else {
            let this = self.this_value_from_reference(&r#ref);
            self.call_function(&callee, &args, Some(this))?
        };
        self.stack.push(result);
        Ok(())
    }

    fn op_call_eval_ref_spread(&mut self) -> error::Result<()> {
        self.op_call_eval_ref_spread_with_context(false)
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
                    a.items.lock().clone()
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

    /// `Op::Await`: resolve Promises and generic thenables, then push the
    /// fulfilled value or rethrow the rejection.
    fn op_await(&mut self) -> error::Result<bool> {
        let v = self.stack.pop().unwrap_or(Value::Undefined);
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.gen_mode.load(Ordering::Relaxed))
        {
            let frame = self.current_frame()?;
            *frame.gen_yield.lock() = Some(v);
            frame.gen_awaiting.store(true, Ordering::Relaxed);
            frame.gen_suspended.store(true, Ordering::Relaxed);
            return Ok(true);
        }
        if self.frames.last().is_some_and(|frame| frame.async_mode) {
            let frame = self.current_frame_mut()?;
            frame.async_await_value = Some(v);
            frame.async_awaiting = true;
            return Ok(true);
        }
        let result = self.await_value(v)?;
        self.stack.push(result);
        Ok(false)
    }

    /// `Op::MakeClosure(func_idx)`: build a function object capturing the
    /// current environment, with a `.prototype` for non-arrow functions.
    fn op_make_closure(&mut self, func_idx: usize) -> error::Result<()> {
        if let Some(fdef) = self.functions.get(func_idx).cloned() {
            let env_idx = self.frames.last().map(|f| f.env).unwrap_or(self.global);
            let realm = crate::environment::global_env_root(&self.heap, env_idx);
            let is_arrow = fdef.is_arrow;
            let is_method = fdef.is_method;
            let is_generator = fdef.is_generator;
            let is_async = fdef.is_async;
            let fn_length = fdef.length;
            let fn_name = fdef.name.clone();
            let has_name_binding = fdef.has_name_binding && fn_name.is_some() && !is_arrow;
            // Generator functions/methods have an own prototype even though
            // they are not constructors. Async functions and other concise
            // methods have no own prototype.
            let has_prototype = !is_arrow && (is_generator || (!is_method && !fdef.is_async));
            let proto_val = if has_prototype {
                let proto = HeapObj::Object(crate::value::ObjectData {
                    props: Mutex::new(IndexMap::new()),
                    proto: Mutex::new(Some(if is_generator {
                        if is_async {
                            self.async_generator_prototype_for_env(realm)
                        } else {
                            self.generator_prototype_for_env(realm)
                        }
                    } else {
                        self.object_proto.clone()
                    })),
                    extensible: std::sync::atomic::AtomicBool::new(true),
                    class_name: None,
                    private_fields: Mutex::new(std::collections::HashMap::new()),
                    primitive: Mutex::new(None),
                });
                Value::Object(self.alloc(proto)?)
            } else {
                Value::Undefined
            };
            // The fresh prototype is not reachable from JavaScript until the
            // function allocation succeeds, so it must survive any collection
            // triggered while creating the name environment or function.
            let proto_pin_count = self.pin(&proto_val);
            let closure_env = if has_name_binding {
                let name_env = match crate::environment::new_env(&self.heap, Some(env_idx), false) {
                    Ok(name_env) => name_env,
                    Err(error) => {
                        self.unpin_many(proto_pin_count);
                        return Err(error.into());
                    }
                };
                self.gc_pins.push(name_env.0);
                name_env
            } else {
                env_idx
            };
            let function_object_proto = if is_generator {
                if is_async {
                    self.async_generator_function_prototype_for_env(realm)
                } else {
                    self.generator_function_prototype_for_env(realm)
                }
            } else if is_async {
                self.realm_async_function_prototypes
                    .get(&realm.0)
                    .cloned()
                    .unwrap_or_else(|| self.function_proto.clone())
            } else {
                self.realm_function_prototypes
                    .get(&realm.0)
                    .cloned()
                    .unwrap_or_else(|| self.function_proto.clone())
            };
            let lexical_new_target = if is_arrow {
                self.frames
                    .last()
                    .map(|frame| frame.new_target.clone())
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            };
            let fd = crate::value::FunctionData {
                name: fdef.name.clone(),
                kind: crate::value::FunctionKind::Interpreted { func: fdef },
                closure: closure_env,
                lexical_new_target,
                home_object: Mutex::new(None),
                is_class_ctor: std::sync::atomic::AtomicBool::new(false),
                prototype: Mutex::new(if has_prototype {
                    Some(proto_val.clone())
                } else {
                    None
                }),
                proto: Mutex::new(match function_object_proto {
                    Value::Object(_) => Some(function_object_proto),
                    _ => None,
                }),
                props: Mutex::new(IndexMap::new()),
                extensible: std::sync::atomic::AtomicBool::new(true),
                private_fields: Mutex::new(std::collections::HashMap::new()),
            };
            let idx_result = self.alloc(HeapObj::Function(fd));
            if has_name_binding {
                self.gc_pins.pop();
            }
            self.unpin_many(proto_pin_count);
            let idx = idx_result?;
            if has_name_binding {
                if let Some(name) = fn_name.as_ref() {
                    crate::environment::declare(
                        &self.heap,
                        closure_env,
                        name,
                        Value::Object(idx),
                        crate::value::BindingKind::FunctionName,
                    );
                }
            }
            // Set function.length, function.name, and function.prototype as own properties.
            self.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Function(f) = obj {
                    let mut props = f.props.lock();
                    let mut len_desc =
                        crate::value::PropertyDescriptor::data(Value::Number(fn_length as f64));
                    len_desc.writable = false;
                    len_desc.enumerable = false;
                    len_desc.configurable = true;
                    props.insert(crate::value::PropertyKey::from("length"), len_desc);
                    let mut name_desc = crate::value::PropertyDescriptor::data(Value::String(
                        fn_name.clone().unwrap_or_else(|| Arc::from("")),
                    ));
                    name_desc.writable = false;
                    name_desc.enumerable = false;
                    name_desc.configurable = true;
                    props.insert(crate::value::PropertyKey::from("name"), name_desc);
                    // prototype: writable, non-enumerable, non-configurable
                    if has_prototype {
                        let mut proto_desc =
                            crate::value::PropertyDescriptor::data(proto_val.clone());
                        proto_desc.writable = true;
                        proto_desc.enumerable = false;
                        proto_desc.configurable = false;
                        props.insert(crate::value::PropertyKey::from("prototype"), proto_desc);
                    }
                }
            });
            // link prototype.constructor back to the function
            if has_prototype && !is_generator {
                if let Value::Object(pidx) = &proto_val {
                    self.heap.with_obj(pidx.0, |obj| {
                        let mut desc = crate::value::PropertyDescriptor::data(Value::Object(idx));
                        desc.enumerable = false;
                        obj.props()
                            .lock()
                            .insert(crate::value::PropertyKey::from("constructor"), desc);
                    });
                }
            }
            self.stack.push(Value::Object(idx));
        } else {
            self.stack.push(Value::Undefined);
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn num_bin<F: Fn(f64, f64) -> f64>(&mut self, f: F) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = self.to_number(&a)?;
        let bv = self.to_number(&b)?;
        self.stack.push(Value::Number(f(av, bv)));
        Ok(())
    }

    fn bitwise_bin<
        F: Fn(i32, i32) -> i32,
        B: Fn(num_bigint::BigInt, num_bigint::BigInt) -> error::Result<num_bigint::BigInt>,
    >(
        &mut self,
        numf: F,
        bigf: B,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = self.to_numeric(&a)?;
        let bv = self.to_numeric(&b)?;
        match (av, bv) {
            (Value::BigInt(x), Value::BigInt(y)) => self.stack.push(Value::BigInt(bigf(x, y)?)),
            (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                return Err(Error::type_err(
                    "Cannot mix BigInt and other types, use explicit conversions".to_string(),
                ));
            }
            (Value::Number(x), Value::Number(y)) => {
                self.stack
                    .push(Value::Number(numf(to_int32(x), to_int32(y)) as f64));
            }
            _ => unreachable!("ToNumeric returns Number or BigInt"),
        }
        Ok(())
    }

    fn shift_bin(&mut self, right: bool) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = self.to_numeric(&a)?;
        let bv = self.to_numeric(&b)?;
        match (av, bv) {
            (Value::BigInt(x), Value::BigInt(y)) => {
                let shifted = if right {
                    Self::bigint_signed_right_shift(x, y)?
                } else {
                    Self::bigint_left_shift(x, y)?
                };
                self.stack.push(Value::BigInt(shifted));
            }
            (Value::BigInt(_), _) | (_, Value::BigInt(_)) => {
                return Err(Error::type_err(
                    "Cannot mix BigInt and other types, use explicit conversions".to_string(),
                ));
            }
            (Value::Number(x), Value::Number(y)) => {
                let left = to_int32(x);
                let shift = to_uint32(y) & 31;
                let result = if right {
                    left >> shift
                } else {
                    left.wrapping_shl(shift)
                };
                self.stack.push(Value::Number(result as f64));
            }
            _ => unreachable!("ToNumeric returns Number or BigInt"),
        }
        Ok(())
    }

    fn bigint_left_shift(
        x: num_bigint::BigInt,
        y: num_bigint::BigInt,
    ) -> error::Result<num_bigint::BigInt> {
        if y.is_negative() {
            return Self::bigint_signed_right_shift(x, -y);
        }
        let shift = y
            .to_usize()
            .ok_or_else(|| Error::range("BigInt shift count is too large".to_string()))?;
        Ok(x << shift)
    }

    fn bigint_signed_right_shift(
        x: num_bigint::BigInt,
        y: num_bigint::BigInt,
    ) -> error::Result<num_bigint::BigInt> {
        if y.is_negative() {
            return Self::bigint_left_shift(x, -y);
        }
        let shift = y
            .to_usize()
            .ok_or_else(|| Error::range("BigInt shift count is too large".to_string()))?;
        Ok(x >> shift)
    }

    fn number_bigint_type_error() -> std::sync::Arc<Error> {
        Error::type_err("Cannot mix BigInt and other types, use explicit conversions".to_string())
    }

    fn numeric_pair(&mut self) -> error::Result<(Value, Value)> {
        let (a, b) = self.pop2();
        let av = self.to_numeric(&a)?;
        let bv = self.to_numeric(&b)?;
        if matches!(
            (&av, &bv),
            (Value::BigInt(_), Value::Number(_)) | (Value::Number(_), Value::BigInt(_))
        ) {
            return Err(Self::number_bigint_type_error());
        }
        Ok((av, bv))
    }

    fn push_numeric_bin<
        F: Fn(f64, f64) -> f64,
        B: Fn(num_bigint::BigInt, num_bigint::BigInt) -> error::Result<num_bigint::BigInt>,
    >(
        &mut self,
        numf: F,
        bigf: B,
    ) -> error::Result<()> {
        let (av, bv) = self.numeric_pair()?;
        match (av, bv) {
            (Value::BigInt(x), Value::BigInt(y)) => self.stack.push(Value::BigInt(bigf(x, y)?)),
            (Value::Number(x), Value::Number(y)) => self.stack.push(Value::Number(numf(x, y))),
            _ => unreachable!("numeric_pair returns matching Number or BigInt operands"),
        }
        Ok(())
    }

    /// Like `num_bin`, but if both operands are `BigInt`, keep the result a
    /// `BigInt` (arbitrary precision via num-bigint). The BigInt closure may
    /// return an error so that operations like division by zero can throw a
    /// `RangeError` per spec.
    fn num_bin_bigint<
        F: Fn(f64, f64) -> f64,
        B: Fn(num_bigint::BigInt, num_bigint::BigInt) -> error::Result<num_bigint::BigInt>,
    >(
        &mut self,
        numf: F,
        bigf: B,
    ) -> error::Result<()> {
        self.push_numeric_bin(numf, bigf)
    }

    fn bin_op<F: Fn(f64, f64) -> Value, G: Fn(&str, &str) -> Value>(
        &mut self,
        numf: F,
        _strf: G,
    ) -> error::Result<()> {
        let (a, b) = self.pop2();
        let ap = self.to_primitive(&a)?;
        let bp = self.to_primitive(&b)?;
        match (&ap, &bp) {
            (Value::String(_), _) | (_, Value::String(_)) => {
                let sa = self.to_string(&ap)?;
                let sb = self.to_string(&bp)?;
                self.stack
                    .push(Value::String(Arc::from(format!("{}{}", sa, sb).as_str())));
            }
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
            _ => {
                let av = self.to_number(&ap)?;
                let bv = self.to_number(&bp)?;
                self.stack.push(numf(av, bv));
            }
        }
        Ok(())
    }

    fn compare(&mut self, op: CompareOp) -> error::Result<()> {
        let (a, b) = self.pop2();
        let pa = self.to_primitive(&a)?;
        let pb = self.to_primitive(&b)?;
        let result = match (&pa, &pb) {
            (Value::String(sa), Value::String(sb)) => {
                let au = crate::value::utf16_from_str(sa);
                let bu = crate::value::utf16_from_str(sb);
                Self::apply_compare_order(op, au.cmp(&bu))
            }
            (Value::BigInt(x), Value::BigInt(y)) => Self::apply_compare_order(op, x.cmp(y)),
            (Value::BigInt(x), Value::String(s)) => Self::string_to_bigint(s)
                .map(|y| Self::apply_compare_order(op, x.cmp(&y)))
                .unwrap_or(false),
            (Value::String(s), Value::BigInt(y)) => Self::string_to_bigint(s)
                .map(|x| Self::apply_compare_order(op, x.cmp(y)))
                .unwrap_or(false),
            (Value::BigInt(x), Value::Number(y)) => Self::compare_bigint_number(x, *y, op),
            (Value::Number(x), Value::BigInt(y)) => {
                let reversed = match op {
                    CompareOp::Lt => CompareOp::Gt,
                    CompareOp::Gt => CompareOp::Lt,
                    CompareOp::Lte => CompareOp::Gte,
                    CompareOp::Gte => CompareOp::Lte,
                };
                Self::compare_bigint_number(y, *x, reversed)
            }
            (Value::BigInt(x), other) => match self.to_numeric(other)? {
                Value::BigInt(y) => Self::apply_compare_order(op, x.cmp(&y)),
                Value::Number(y) => Self::compare_bigint_number(x, y, op),
                _ => unreachable!("ToNumeric returns Number or BigInt"),
            },
            (other, Value::BigInt(y)) => {
                let reversed = match op {
                    CompareOp::Lt => CompareOp::Gt,
                    CompareOp::Gt => CompareOp::Lt,
                    CompareOp::Lte => CompareOp::Gte,
                    CompareOp::Gte => CompareOp::Lte,
                };
                match self.to_numeric(other)? {
                    Value::BigInt(x) => Self::apply_compare_order(reversed, y.cmp(&x)),
                    Value::Number(x) => Self::compare_bigint_number(y, x, reversed),
                    _ => unreachable!("ToNumeric returns Number or BigInt"),
                }
            }
            _ => {
                let av = self.to_number(&pa)?;
                let bv = self.to_number(&pb)?;
                if av.is_nan() || bv.is_nan() {
                    false
                } else {
                    match op {
                        CompareOp::Lt => av < bv,
                        CompareOp::Gt => av > bv,
                        CompareOp::Lte => av <= bv,
                        CompareOp::Gte => av >= bv,
                    }
                }
            }
        };
        self.stack.push(Value::Bool(result));
        Ok(())
    }

    fn apply_compare_order(op: CompareOp, ordering: std::cmp::Ordering) -> bool {
        match op {
            CompareOp::Lt => ordering == std::cmp::Ordering::Less,
            CompareOp::Gt => ordering == std::cmp::Ordering::Greater,
            CompareOp::Lte => ordering != std::cmp::Ordering::Greater,
            CompareOp::Gte => ordering != std::cmp::Ordering::Less,
        }
    }

    fn compare_bigint_number(x: &num_bigint::BigInt, y: f64, op: CompareOp) -> bool {
        if y.is_nan() {
            return false;
        }
        if y == f64::INFINITY {
            return matches!(op, CompareOp::Lt | CompareOp::Lte);
        }
        if y == f64::NEG_INFINITY {
            return matches!(op, CompareOp::Gt | CompareOp::Gte);
        }
        if let Some(y_int) = Self::number_to_bigint_exact(y) {
            return Self::apply_compare_order(op, x.cmp(&y_int));
        }
        let bound = match op {
            CompareOp::Lt | CompareOp::Lte => y.floor(),
            CompareOp::Gt | CompareOp::Gte => y.ceil(),
        };
        Self::number_to_bigint_exact(bound)
            .map(|bound| match op {
                CompareOp::Lt | CompareOp::Lte => x <= &bound,
                CompareOp::Gt | CompareOp::Gte => x >= &bound,
            })
            .unwrap_or(false)
    }

    // ---- type conversions ----
}
