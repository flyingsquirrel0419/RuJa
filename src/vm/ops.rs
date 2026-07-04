use super::*;

#[derive(Clone, Copy)]
enum CompareOp {
    Lt,
    Gt,
    Lte,
    Gte,
}

impl Vm {
    pub(crate) fn interpret_inner_raw(
        &mut self,
        return_depth: Option<usize>,
    ) -> error::Result<Value> {
        loop {
            // Execution fuel: bound untrusted code. Checked before each
            // opcode so a tight loop cannot run forever. None = unbounded.
            if let Some(f) = self.fuel.as_mut() {
                if *f <= 0 {
                    return Err(Error::fuel("fuel exhausted".to_string()));
                }
                *f -= 1;
            }
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
            let frame = self.current_frame()?;
            let ip = frame.ip;
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
                Op::Const(idx) => {
                    let v = {
                        let frame = self.current_frame()?;
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
                                    let global_this = self.global_this.clone();
                                    if self.has_property(&global_this, &name)? {
                                        let v = self.get_property(&global_this, &name)?;
                                        self.stack.push(v);
                                    } else {
                                        return Err(Error::reference(format!(
                                            "{} is not defined",
                                            name
                                        )));
                                    }
                                }
                                Err(true) => {
                                    return Err(Error::reference(format!(
                                        "Cannot access '{}' before initialization",
                                        name
                                    )))
                                }
                                Err(false) => {
                                    let global_this = self.global_this.clone();
                                    if self.has_property(&global_this, &name)? {
                                        let v = self.get_property(&global_this, &name)?;
                                        self.stack.push(v);
                                    } else {
                                        return Err(Error::reference(format!(
                                            "{} is not defined",
                                            name
                                        )));
                                    }
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
                            let global_this = self.global_this.clone();
                            if self.has_property(&global_this, &name)? {
                                let v = self.get_property(&global_this, &name)?;
                                self.stack.push(v);
                            } else {
                                return Err(Error::reference(format!("{} is not defined", name)));
                            }
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
                    if cur_env == self.global && self.global_property_is_non_writable_data(&name) {
                        let global_this = self.global_this.clone();
                        self.set_property(&global_this, &name, value)?;
                        self.stack.push(Value::Undefined);
                        continue;
                    }
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
                    crate::environment::ensure_var(&self.heap, env, &name);
                    if root == self.global {
                        self.set_global_var_property(&name, Value::Undefined);
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
                    crate::environment::ensure_var(&self.heap, cur_env, &name);
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
                            crate::environment::declare_var(
                                &self.heap,
                                cur_env,
                                &name,
                                value.clone(),
                            );
                        }
                    }
                    if root == self.global {
                        self.set_global_var_property(&name, value);
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
                Op::LoadEnvName(name_idx) => {
                    // Reset any stale `with`-this from a previous name load that
                    // was not immediately followed by a `Call`. Only a name found
                    // on a `with` object *and* used as a call callee should rebind
                    // `this`; clearing here prevents leftover values from leaking
                    // into a later, unrelated call.
                    if let Some(f) = self.frames.last() {
                        *f.pending_with_this.lock() = None;
                    }
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
                    // Per spec, identifier resolution walks the environment chain
                    // from innermost to outermost. At each environment record:
                    //   1. Check for a binding (var/let/const) — found = use it.
                    //   2. If the environment has a `with`-object, check its
                    //      properties — found = use it (and set pending_with_this).
                    //   3. Neither = continue to parent.
                    //
                    // We walk the chain manually so that a var binding in a
                    // child scope shadows a with-object property on a parent
                    // scope (the function-inside-with case), while a
                    // with-object property shadows an outer var binding
                    // (the direct-with case).
                    let mut found = false;
                    let mut cur_env = Some(env);
                    while let Some(e_idx) = cur_env {
                        let (binding_val, in_tdz, has_with, with_obj_val, parent) =
                            self.heap.with_obj(e_idx.0, |obj| {
                                if let HeapObj::Environment(e) = obj {
                                    // 1. Check var/let/const bindings.
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
                                    // 2. Check with-object.
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
                            self.stack.push(v);
                            found = true;
                            break;
                        }
                        if has_with {
                            if let Some(with_obj) = with_obj_val {
                                let has_prop = self.has_own_property(&with_obj, &name);
                                if has_prop {
                                    let v = self.get_property(&with_obj, &name)?;
                                    if matches!(v, Value::Object(_)) {
                                        *self.current_frame_mut()?.pending_with_this.lock() =
                                            Some(with_obj);
                                    }
                                    self.stack.push(v);
                                    found = true;
                                    break;
                                }
                            }
                        }
                        cur_env = parent;
                    }
                    if !found {
                        // Last resort: check global (this).
                        let global_this = self.global_this.clone();
                        let has = self.has_property(&global_this, &name)?;
                        if has {
                            let v = self.get_property(&global_this, &name)?;
                            self.stack.push(v);
                        } else {
                            return Err(Error::reference(format!("{} is not defined", name)));
                        }
                    }
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
                                // Strict mode: assigning to an undeclared variable
                                // throws ReferenceError (not auto-global).
                                if self.current_strict() {
                                    return Err(Error::reference(format!(
                                        "{} is not defined",
                                        name
                                    )));
                                }
                                // Non-strict: create a configurable property
                                // on the global object (spec: implicit global
                                // assignment creates a writable, enumerable,
                                // configurable data property on the global
                                // object, NOT a var binding in the env record).
                                let global_this = self.global_this.clone();
                                self.set_property(&global_this, &name, value)?;
                            }
                        }
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
                    let name_str = name.as_str().unwrap_or_default().to_string();
                    let env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    let strict = self.current_strict();
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
                                if self.has_own_property(&with_obj, &name_str) {
                                    base = crate::value::ReferenceBase::ObjectEnvironment(
                                        Box::new(with_obj),
                                    );
                                    break;
                                }
                            }
                        }
                        cur_env = parent;
                    }
                    if cur_env.is_none() {
                        let global_this = self.global_this.clone();
                        if self.has_property(&global_this, &name_str)? {
                            base = crate::value::ReferenceBase::ObjectEnvironment(Box::new(
                                global_this,
                            ));
                        }
                    }
                    let r#ref = crate::value::ReferenceRecord { base, name, strict };
                    self.stack.push(Value::Reference(Box::new(r#ref)));
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
                            Ok(num_bigint::BigInt::from(0))
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
                    let n = to_int32(self.to_number(&v)?);
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
                Op::Lt => self.compare(CompareOp::Lt)?,
                Op::Gt => self.compare(CompareOp::Gt)?,
                Op::Lte => self.compare(CompareOp::Lte)?,
                Op::Gte => self.compare(CompareOp::Gte)?,
                Op::In => {
                    // stack: [key, obj]; true if obj has the property (own or inherited).
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    // Use has_property (existence check) instead of get_property
                    // to avoid triggering poisoned accessors (e.g. strict-mode
                    // function's 'caller'/'arguments' throw on [[Get]] but
                    // 'in' should return true).
                    let has = self.has_property(&obj, &key_str)?;
                    self.stack.push(Value::Bool(has));
                }
                Op::InstanceOf => {
                    // stack: [obj, ctor]; walk obj's proto chain for ctor.prototype.
                    let ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    // ES spec: if ctor is not a function, throw TypeError.
                    let is_function = if let Value::Object(ci) = &ctor {
                        self.heap.with_obj(ci.0, |o| o.is_function())
                    } else {
                        false
                    };
                    if !is_function {
                        return Err(Error::type_err(
                            "Right-hand side of 'instanceof' is not callable".to_string(),
                        ));
                    }
                    // ES spec: call [[Get]](ctor, "prototype") — this honors
                    // user-set .prototype and getters, not just the internal
                    // field.
                    let ctor_proto = self.get_property(&ctor, "prototype")?;
                    // ES spec: if F.prototype is not an object, throw TypeError.
                    if !matches!(ctor_proto, Value::Object(_) | Value::Null) {
                        return Err(Error::type_err(
                            "Function has non-object prototype 'undefined' in instanceof check"
                                .to_string(),
                        ));
                    }
                    let mut cur = obj;
                    let mut result = false;
                    while let Value::Object(oi) = &cur {
                        if Value::Object(*oi) == ctor_proto {
                            result = true;
                            break;
                        }
                        cur = self.heap.with_obj(oi.0, |o| {
                            o.proto().lock().clone().unwrap_or(Value::Undefined)
                        });
                        if cur.is_undefined() {
                            break;
                        }
                    }
                    // ES spec: if O is not an object, return false.
                    // (Already handled: the while loop only enters for Object.)
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
                    let av = to_uint32(self.to_number(&a)?);
                    let bv = to_uint32(self.to_number(&b)?);
                    self.stack.push(Value::Number((av >> (bv & 31)) as f64));
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
                        if let Some(&(target, _)) = frame.finally_stack.last() {
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
                    let obj = HeapObj::Array(crate::value::ArrayData {
                        items: Mutex::new(items),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(self.array_proto.clone())),
                        sparse_max: Mutex::new(None),
                    });
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
                                for (i, v) in a.items.lock().iter().enumerate() {
                                    out.push((Arc::from(i.to_string().as_str()), v.clone()));
                                }
                            }
                            for (k, desc) in o.props().lock().iter() {
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
                            self.define_data_property(
                                &dest,
                                crate::value::PropertyKey::Str(k.clone()),
                                v,
                            )?;
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
                    let new_obj = Value::Object(self.new_object()?);
                    if let (Value::Object(dest_idx), Value::Object(src_idx)) = (&new_obj, &src) {
                        let pairs: Vec<(Arc<str>, Value)> = self.heap.with_obj(src_idx.0, |o| {
                            let mut out = Vec::new();
                            for (k, desc) in o.props().lock().iter() {
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
                            let mut props = props.lock();
                            let entry = props.entry(pkey).or_insert_with(|| {
                                crate::value::PropertyDescriptor {
                                    value: Value::Undefined,
                                    writable: false,
                                    enumerable: false,
                                    configurable: true,
                                    get: None,
                                    set: None,
                                    is_accessor: true,
                                }
                            });
                            entry.is_accessor = true;
                            entry.writable = false;
                            entry.enumerable = false;
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
                    // Inline cache fast-path: if the object is the same heap
                    // index as the last GetProp on this key, skip the proto
                    // chain walk. The cache is per-property-name, stored in
                    // a small VM-level HashMap. This helps tight loops like
                    // `for (let i = 0; i < arr.length; i++)` where the same
                    // property is read repeatedly.
                    let v = if let Value::Object(idx) = &obj {
                        if let Some(cached) = self.ic_get(idx.0, &key_str) {
                            cached
                        } else {
                            let val = self.get_property(&obj, &key_str)?;
                            self.ic_put(idx.0, key_str.clone(), val.clone());
                            val
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
                Op::SetProp => {
                    // stack (bottom->top): [obj, key, value]
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    self.set_property(&obj, &key_str, value.clone())?;
                    // Invalidate IC entry for this object+key so stale
                    // cached values are not returned on next GetProp.
                    if let Value::Object(idx) = &obj {
                        self.ic_invalidate(idx.0, &key_str);
                    }
                    self.stack.push(value);
                }
                Op::DefineDataProperty => {
                    // stack (bottom->top): [obj, key, value]
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
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
                    let key_str = self.to_property_key(&key)?;
                    if let Value::Object(idx) = &obj {
                        self.heap.with_obj(idx.0, |o| {
                            let mut props = o.props().lock();
                            let mut desc = crate::value::PropertyDescriptor::data(value.clone());
                            desc.enumerable = false;
                            desc.configurable = true;
                            desc.writable = true;
                            props.insert(crate::value::PropertyKey::from(key_str.as_str()), desc);
                        });
                        self.ic_invalidate(idx.0, &key_str);
                    }
                    self.stack.push(value);
                }
                Op::SetElem => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    self.set_property_key(&obj, &key, value.clone())?;
                    // Invalidate IC entry for this object+key so that
                    // subsequent GetProp does not return a stale value.
                    // Symbol keys are not cached by the IC, so skip them.
                    if let (Value::Object(idx), Value::String(_)) = (&obj, &key) {
                        let key_str = self.to_property_key(&key)?;
                        self.ic_invalidate(idx.0, &key_str);
                    }
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
                        // Array element deletion: delete arr[i] sets the
                        // element to undefined (creates a hole), and
                        // delete arr.length returns false (non-configurable).
                        let is_array = self
                            .heap
                            .with_obj(idx.0, |o| matches!(o, HeapObj::Array(_)));
                        if is_array {
                            if let crate::value::PropertyKey::Str(ref s) = &pkey {
                                if s.as_ref() == "length" {
                                    Value::Bool(false)
                                } else if let Ok(i) = s.parse::<usize>() {
                                    let exists = self.heap.with_obj(idx.0, |o| {
                                        if let HeapObj::Array(a) = o {
                                            i < a.items.lock().len()
                                        } else {
                                            false
                                        }
                                    });
                                    if exists {
                                        self.heap.with_obj(idx.0, |o| {
                                            if let HeapObj::Array(a) = o {
                                                a.items.lock()[i] = Value::Undefined;
                                            }
                                        });
                                    }
                                    Value::Bool(true)
                                } else {
                                    // Non-index string key on array: use props
                                    let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                                        o.props()
                                            .lock()
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
                                            o.props().lock().shift_remove(&pkey);
                                        });
                                        Value::Bool(true)
                                    } else {
                                        Value::Bool(true)
                                    }
                                }
                            } else {
                                // Symbol key on array: use props
                                let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                                    o.props()
                                        .lock()
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
                                        o.props().lock().shift_remove(&pkey);
                                    });
                                    Value::Bool(true)
                                } else {
                                    Value::Bool(true)
                                }
                            }
                        } else {
                            // Check configurability first: deleting a
                            // non-configurable own property must fail (`false`,
                            // or a TypeError in strict mode), not actually remove
                            // the property.
                            let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                                o.props()
                                    .lock()
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
                                    o.props().lock().shift_remove(&pkey);
                                });
                                Value::Bool(true)
                            } else {
                                // Non-existent own property: delete returns true.
                                Value::Bool(true)
                            }
                        }
                    } else {
                        // null/undefined receiver: ToObject throws TypeError.
                        if matches!(obj, Value::Null | Value::Undefined) {
                            return Err(Error::type_err(
                                "Cannot convert undefined or null to object",
                            ));
                        }
                        // Other primitives (number, string, boolean): delete is
                        // a no-op that returns true (ToObject wraps in a wrapper
                        // object, which has no own configurable properties).
                        Value::Bool(true)
                    };
                    self.stack.push(result);
                }
                Op::DeleteVar(name_idx) => {
                    // `delete x` (identifier, non-strict mode): check if the
                    // binding is deletable. var/function = false, unbound = true.
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
                    let env_binding =
                        crate::environment::binding_env_and_kind(&self.heap, cur_env, &name);
                    let has_env_binding = env_binding.is_some();
                    let has_with_env =
                        !crate::environment::with_objects(&self.heap, cur_env).is_empty();
                    // First try normal delete_binding (handles lexical bindings
                    // and with-object properties).
                    let deleted = crate::environment::delete_binding(&self.heap, cur_env, &name);
                    if deleted {
                        if !has_env_binding && !has_with_env {
                            let global_this = self.global_this.clone();
                            if self.has_own_property(&global_this, &name) {
                                let deleted = self.delete_property(&global_this, &name)?;
                                self.stack.push(Value::Bool(deleted));
                            } else {
                                self.stack.push(Value::Bool(true));
                            }
                        } else {
                            self.stack.push(Value::Bool(true));
                        }
                    } else if env_binding
                        .as_ref()
                        .is_some_and(|(binding_env, _)| *binding_env == self.global)
                    {
                        let global_this = self.global_this.clone();
                        if self.has_own_property(&global_this, &name) {
                            let deleted = self.delete_property(&global_this, &name)?;
                            if deleted {
                                crate::environment::delete_var_binding(
                                    &self.heap,
                                    self.global,
                                    &name,
                                );
                            }
                            self.stack.push(Value::Bool(deleted));
                        } else {
                            self.stack.push(Value::Bool(false));
                        }
                    } else if !has_env_binding {
                        let global_this = self.global_this.clone();
                        if self.has_own_property(&global_this, &name) {
                            let deleted = self.delete_property(&global_this, &name)?;
                            self.stack.push(Value::Bool(deleted));
                        } else {
                            // Unresolvable reference: delete returns true.
                            self.stack.push(Value::Bool(true));
                        }
                    } else {
                        self.stack.push(Value::Bool(false));
                    }
                }
                Op::ValidateExtends => {
                    // Pop the superclass value and validate it's a constructor
                    // whose .prototype is an object or null.
                    let parent = self.stack.pop().unwrap_or(Value::Undefined);
                    let is_ctor = match &parent {
                        Value::Object(idx) => self.heap.with_obj(idx.0, |o| o.is_function()),
                        _ => false,
                    };
                    if !is_ctor {
                        self.stack.push(parent);
                        return Err(Error::type_err("Class extends value is not a constructor"));
                    }
                    // Check parent.prototype is an object or null.
                    let proto = self
                        .get_property_by_key(&parent, &crate::value::PropertyKey::from("prototype"))
                        .unwrap_or(Value::Undefined);
                    if !matches!(proto, Value::Object(_) | Value::Null) {
                        self.stack.push(parent);
                        return Err(Error::type_err(
                            "Class extends value's prototype is not an object or null",
                        ));
                    }
                    self.stack.push(parent);
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
                                (Some(&(_, fseq)), Some(&(_, cseq, _))) => fseq > cseq,
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
                        if let Some((handler, _, saved_env)) = frame.catch_stack.pop() {
                            // Restore env to try-entry point, unwinding any
                            // scopes/with-envs opened in the try body.
                            frame.env = saved_env;
                            frame.ip = handler;
                            self.stack.push(v);
                            continue;
                        }
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
                    let f = self.current_frame_mut()?;
                    let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                    f.guard_seq.store(seq, Ordering::Relaxed);
                    let saved_env = f.env;
                    f.catch_stack.push((handler, seq, saved_env));
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
                Op::GetPrivate(name_idx) => {
                    let name = {
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                        let frame = self.current_frame()?;
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
                            let frame = self.current_frame_mut()?;
                            // If an outer finally still guards this scope,
                            // divert the throw through it first.
                            // Divert only if the outer finally is more deeply
                            // nested than the nearest catch (per spec, a throw
                            // is caught by the innermost matching handler, but
                            // must still run any finally nested inside it).
                            let divert_to_outer_finally =
                                match (frame.finally_stack.last(), frame.catch_stack.last()) {
                                    (Some(&(_, _)), None) => true,
                                    (Some(&(_, fseq)), Some(&(_, cseq, _))) => fseq > cseq,
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
                                continue;
                            }
                            // If an outer try catches, route there; else propagate.
                            if let Some(&(handler, _, _)) = frame.catch_stack.last() {
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
                        let frame = self.current_frame()?;
                        *frame.gen_yield.lock() = Some(v);
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
                    // Calling super() twice is a ReferenceError.
                    if self
                        .current_frame()?
                        .super_called
                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        return Err(Error::reference("super() has already been called"));
                    }
                    let mut args = Vec::with_capacity(arg_count);
                    for _ in 0..arg_count {
                        args.push(self.stack.pop().unwrap_or(Value::Undefined));
                    }
                    args.reverse();
                    let super_ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let _placeholder = self.stack.pop().unwrap_or(Value::Undefined);
                    // Use the frame's this_val (the object created by the
                    // outer `construct` call), not the environment's `this`
                    // which is in the TDZ for derived constructors.
                    let this_val = self.current_frame()?.this_val.clone();
                    // Call the parent constructor. Set pending_new_target so
                    // that class constructors accept this as a [[Construct]] call.
                    self.pending_new_target = Some(super_ctor.clone());
                    let result = self.call_function(&super_ctor, &args, Some(this_val.clone()))?;
                    // If the parent constructor returned an object, use it as the new `this`.
                    let new_this = if matches!(result, Value::Object(_)) {
                        result
                    } else {
                        this_val
                    };
                    // Rebind `this` in the current environment to the (possibly updated) value.
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    // Use `initialize` (not `set`) so the TDZ flag is lifted
                    // for derived constructors where `this` was declared
                    // uninitialized until `super()` ran.
                    crate::environment::initialize(&self.heap, cur_env, "this", new_this.clone());
                    self.current_frame_mut()?.this_val = new_this.clone();
                    self.stack.push(new_this);
                }
                Op::CallSuperCtorSpread => {
                    // stack: [this, superCtor, argsArray]
                    if self
                        .current_frame()?
                        .super_called
                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        return Err(Error::reference("super() has already been called"));
                    }
                    let args_arr = self.stack.pop().unwrap_or(Value::Undefined);
                    let super_ctor = self.stack.pop().unwrap_or(Value::Undefined);
                    let _placeholder = self.stack.pop().unwrap_or(Value::Undefined);
                    let this_val = self.current_frame()?.this_val.clone();
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
                    self.pending_new_target = Some(super_ctor.clone());
                    let result = self.call_function(&super_ctor, &args, Some(this_val.clone()))?;
                    let new_this = if matches!(result, Value::Object(_)) {
                        result
                    } else {
                        this_val
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::initialize(&self.heap, cur_env, "this", new_this.clone());
                    self.current_frame_mut()?.this_val = new_this.clone();
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
                Op::ToNumeric => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    match v {
                        Value::BigInt(_) => self.stack.push(v),
                        _ => {
                            let n = self.to_number(&v)?;
                            self.stack.push(Value::Number(n));
                        }
                    }
                }
                Op::Await => self.op_await()?,
                Op::TypeofVar(name_idx) => {
                    // `typeof name`: "undefined" if the name is not bound (must not throw).
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
                    let val = crate::environment::get(&self.heap, cur_env, &name)
                        .or_else(|| crate::environment::get(&self.heap, self.global, &name));
                    let val = if val.is_none() {
                        let global_this = self.global_this.clone();
                        if self.has_property(&global_this, &name)? {
                            Some(self.get_property(&global_this, &name)?)
                        } else {
                            None
                        }
                    } else {
                        val
                    };
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
                        sparse_max: Mutex::new(None),
                    });
                    let val = Value::Object(self.alloc(arr)?);
                    self.stack.push(val);
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
            .map(|f| f.pending_with_this.lock().take())
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
                    args = a.items.lock().clone();
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
                        (*p.state.lock(), p.result.lock().clone())
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
    fn op_make_closure(&mut self, func_idx: usize) -> error::Result<()> {
        if let Some(fdef) = self.functions.get(func_idx).cloned() {
            let env_idx = self.frames.last().map(|f| f.env).unwrap_or(self.global);
            let is_arrow = fdef.is_arrow;
            let fn_length = fdef.length;
            let fn_name = fdef.name.clone();
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
                Value::Object(self.alloc(proto)?)
            } else {
                Value::Undefined
            };
            let fd = crate::value::FunctionData {
                name: fdef.name.clone(),
                kind: crate::value::FunctionKind::Interpreted { func: fdef },
                closure: env_idx,
                is_class_ctor: std::sync::atomic::AtomicBool::new(false),
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
            let idx = self.alloc(HeapObj::Function(fd))?;
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
                    if !is_arrow {
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
            if let Value::Object(pidx) = &proto_val {
                self.heap.with_obj(pidx.0, |obj| {
                    let mut desc = crate::value::PropertyDescriptor::data(Value::Object(idx));
                    desc.enumerable = false;
                    obj.props()
                        .lock()
                        .insert(crate::value::PropertyKey::from("constructor"), desc);
                });
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

    fn int_bin<F: Fn(i32, i32) -> i32>(&mut self, f: F) -> error::Result<()> {
        let (a, b) = self.pop2();
        let av = to_int32(self.to_number(&a)?);
        let bv = to_int32(self.to_number(&b)?);
        self.stack.push(Value::Number(f(av, bv) as f64));
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
        let (a, b) = self.pop2();
        match (&a, &b) {
            (Value::BigInt(x), Value::BigInt(y)) => {
                self.stack.push(Value::BigInt(bigf(x.clone(), y.clone())?));
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
