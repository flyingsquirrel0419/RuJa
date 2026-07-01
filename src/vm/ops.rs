use super::*;

impl Vm {
        pub(crate) fn interpret_inner_raw(&mut self, return_depth: Option<usize>) -> error::Result<Value> {
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
                if let Some(exc) = self
                    .frames
                    .last()
                    .and_then(|f| f.force_throw.lock().take())
                {
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
                        crate::environment::declare_var(&self.heap, cur_env, &name, value);
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
                                    .current_frame_mut()?
                                    .pending_with_this
                                    .lock() = Some(with_obj);
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
                        self.stack.pop();
                    }
                    Op::PushScope => {
                        let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                        let new_env = env::new_env(&self.heap, Some(cur_env), false);
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
                        let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                        let new_env = env::new_with_env(&self.heap, cur_env, object);
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
                        let child = env::clone_loop_vars(&self.heap, cur_env, &names);
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
                        let v = self.stack.pop().unwrap_or(Value::Undefined);
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
                            sparse_max: Mutex::new(None),
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
                                *o.proto().lock() = Some(proto);
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
                                let target = frame
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
                            if let Some((handler, _)) = frame.catch_stack.pop() {
                                frame.ip = handler;
                                self.stack.push(v);
                                continue;
                            }
                        }
                        return Err(Error::thrown(v, &self.heap));
                    }
                    Op::PushTry(handler) => {
                        let f = self.current_frame_mut()?;
                        let seq = f.guard_seq.load(Ordering::Relaxed) + 1;
                        f.guard_seq.store(seq, Ordering::Relaxed);
                        f.catch_stack.push((handler, seq));
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
                                        (Some(&(_, fseq)), Some(&(_, cseq))) => fseq > cseq,
                                        _ => false,
                                    };
                                if divert_to_outer_finally {
                                    let outer = frame
                                        .finally_stack
                                        .last()
                                        .map(|(ip, _)| *ip)
                                        .ok_or_else(|| {
                                            crate::error::Error::internal(
                                                "finally stack empty during throw diversion",
                                            )
                                        })?;
                                    frame.finally_completion_tag.store(4, Ordering::Relaxed);
                                    *frame.finally_completion_val.lock() = val.clone();
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
            let av = to_int32(self.to_number(&a)?);
            let bv = to_int32(self.to_number(&b)?);
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
    
}
