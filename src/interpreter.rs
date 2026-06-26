use crate::ast::*;
use crate::environment::{BindingKind, Env};
use crate::error::{self, Completion, Error};
use crate::value::{FunctionKind, FunctionValue, InternalData, Obj, PropertyDescriptor, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Interpreter {
    pub global: Env,
    pub object_proto: Value,
    pub function_proto: Value,
    pub array_proto: Value,
    pub string_proto: Value,
    pub number_proto: Value,
    pub boolean_proto: Value,
    pub error_proto: Value,
}

impl Default for Interpreter {
    fn default() -> Self { Self::new() }
}

impl Interpreter {
    pub fn new() -> Self {
        let global = Env::new();
        // proto placeholders; filled after creation
       let mut interp = Interpreter {
            global,
            object_proto: Value::Undefined,
            function_proto: Value::Undefined,
            array_proto: Value::Undefined,
            string_proto: Value::Undefined,
            number_proto: Value::Undefined,
            boolean_proto: Value::Undefined,
            error_proto: Value::Undefined,
       };
       crate::builtins::setup(&mut interp);
       interp
    }

    pub fn run(&mut self, src: &str) -> error::Result<Value> {
        let program = crate::parser::Parser::parse(src)?;
        self.hoist(&program.body, &self.global.clone());
        let mut last = Value::Undefined;
        for stmt in &program.body {
            if let Stmt::ExprStmt(e) = stmt {
                last = self.eval(e, &self.global.clone())?;
            } else {
                self.exec_stmt(stmt, &self.global.clone())?;
            }
        }
        Ok(last)
    }

    pub fn exec_program(&mut self, program: &Program) -> error::Result<Value> {
        // hoist function declarations and var declarations
        self.hoist(&program.body, &self.global.clone());
        let mut result = Value::Undefined;
        for stmt in &program.body {
            match self.exec_stmt(stmt, &self.global.clone())? {
                Completion::Normal => {}
                Completion::Return(v) => { result = v; break; }
                _ => {}
            }
        }
        Ok(result)
    }

    fn hoist(&mut self, body: &[Stmt], env: &Env) {
        for stmt in body {
            match stmt {
                Stmt::FunctionDecl(f) => {
                    if let Some(name) = &f.name {
                        let func = self.make_function(f.clone(), env.clone());
                        env.declare_var(name, func);
                    }
                }
                Stmt::VarDecl { kind: VarKind::Var, decls } => {
                    for (name, _) in decls {
                        env.declare_var(name, Value::Undefined);
                    }
                }
                Stmt::Block(b) => self.hoist(b, env),
                _ => {}
            }
        }
    }

    fn make_function(&self, f: FunctionExpr, env: Env) -> Value {
        let fv = FunctionValue {
            name: f.name.clone(),
            kind: FunctionKind::Interpreted { func: f.clone() },
            closure: env,
            prototype: None,
            properties: RefCell::new(HashMap::new()),
        };
        let func = Value::Function(Rc::new(fv));
        // give it a .prototype object
        if !f.is_arrow {
            if let Value::Function(fr) = &func {
                let proto_rc = Rc::new(RefCell::new(Obj::new()));
                {
                    let mut proto_obj = proto_rc.borrow_mut();
                    proto_obj.proto = Some(self.object_proto.clone());
                    proto_obj.props.insert(Rc::from("constructor"), PropertyDescriptor::data(func.clone()));
                }
                fr.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(Value::Object(proto_rc)));
            }
        }
        func
    }

    pub fn exec_stmt(&mut self, stmt: &Stmt, env: &Env) -> error::Result<Completion> {
        match stmt {
            Stmt::Empty => Ok(Completion::Normal),
            Stmt::Block(body) => {
                let block_env = env.child();
                self.hoist(body, &block_env);
                for s in body {
                    match self.exec_stmt(s, &block_env)? {
                        Completion::Normal => {}
                        c => return Ok(c),
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::VarDecl { kind, decls } => {
                for (name, init) in decls {
                    let val = if let Some(e) = init {
                        self.eval(e, env)?
                    } else { Value::Undefined };
                    match kind {
                        VarKind::Var => env.declare_var(name, val),
                        VarKind::Let | VarKind::Const => env.declare(name, val, if *kind == VarKind::Const { BindingKind::Const } else { BindingKind::Let }),
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::ExprStmt(e) => { self.eval(e, env)?; Ok(Completion::Normal) }
            Stmt::If { cond, then, else_ } => {
                if self.eval(cond, env)?.is_truthy() {
                    self.exec_stmt(then, env)
                } else if let Some(e) = else_ {
                    self.exec_stmt(e, env)
                } else { Ok(Completion::Normal) }
            }
            Stmt::While { cond, body } => {
                while self.eval(cond, env)?.is_truthy() {
                    match self.exec_stmt(body, env)? {
                        Completion::Normal => {}
                        Completion::Break(_) => break,
                        Completion::Continue(_) => continue,
                        c => return Ok(c),
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::DoWhile { body, cond } => {
                loop {
                    match self.exec_stmt(body, env)? {
                        Completion::Normal => {}
                        Completion::Break(_) => break,
                        Completion::Continue(_) => {}
                        c => return Ok(c),
                    }
                    if !self.eval(cond, env)?.is_truthy() { break; }
                }
                Ok(Completion::Normal)
            }
            Stmt::For { init, cond, update, body } => {
                let for_env = env.child();
                if let Some(init_stmt) = init {
                    self.exec_stmt(init_stmt, &for_env)?;
                }
                loop {
                    if let Some(c) = cond {
                        if !self.eval(c, &for_env)?.is_truthy() { break; }
                    }
                    match self.exec_stmt(body, &for_env)? {
                        Completion::Normal => {}
                        Completion::Break(_) => break,
                        Completion::Continue(_) => {}
                        c => return Ok(c),
                    }
                    if let Some(u) = update {
                        self.eval(u, &for_env)?;
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::ForOf { left, right, body } => {
                let iterable = self.eval(right, env)?;
                let items = self.iter_to_values(&iterable)?;
                for item in items {
                    self.assign_for_left(left, item, env)?;
                    match self.exec_stmt(body, env)? {
                        Completion::Normal => {}
                        Completion::Break(_) => break,
                        Completion::Continue(_) => continue,
                        c => return Ok(c),
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::ForIn { left, right, body } => {
                let obj = self.eval(right, env)?;
                let keys = self.own_enum_keys(&obj);
                for key in keys {
                    self.assign_for_left(left, Value::from_str(&key), env)?;
                    match self.exec_stmt(body, env)? {
                        Completion::Normal => {}
                        Completion::Break(_) => break,
                        Completion::Continue(_) => continue,
                        c => return Ok(c),
                    }
                }
                Ok(Completion::Normal)
            }
            Stmt::Break(label) => Ok(Completion::Break(label.as_ref().map(|s| s.to_string()))),
            Stmt::Continue(label) => Ok(Completion::Continue(label.as_ref().map(|s| s.to_string()))),
            Stmt::Return(e) => {
                let v = if let Some(e) = e { self.eval(e, env)? } else { Value::Undefined };
                Ok(Completion::Return(v))
            }
            Stmt::Throw(e) => {
                let v = self.eval(e, env)?;
                Err(Error::thrown(v))
            }
            Stmt::TryCatch { try_body, catch_param, catch_body, finally_body } => {
                let result = self.exec_stmt(try_body, env);
                let result = match result {
                    Err(err) => {
                        let err_val = err.thrown_value.clone().unwrap_or_else(|| self.error_to_value(err));
                        let catch_env = env.child();
                        if let Some(param) = catch_param {
                            catch_env.declare(param, err_val, BindingKind::Let);
                        }
                        match self.exec_stmt(catch_body, &catch_env) {
                            Ok(Completion::Normal) => Ok(Completion::Normal),
                            Ok(c) => Ok(c),
                            Err(e) => Err(e),
                        }
                    }
                    Ok(c) => Ok(c),
                };
                if let Some(fin) = finally_body {
                    let _ = self.exec_stmt(fin, env)?;
                }
                result
            }
            Stmt::FunctionDecl(_) => Ok(Completion::Normal), // hoisted
            Stmt::Labeled(_, s) => self.exec_stmt(s, env),
            Stmt::Switch { disc, cases } => {
                let d = self.eval(disc, env)?;
                let mut matched = false;
                let _exec_rest = false;
                for case in cases {
                    if !matched {
                        if let Some(test) = &case.test {
                            let test_val = self.eval(test, env)?;
                            if self.strict_equals(&d, &test_val)? {
                                matched = true;
                            }
                        } else {
                            matched = true; // default
                        }
                    }
                    if matched {
                        for s in &case.body {
                            match self.exec_stmt(s, env)? {
                                Completion::Normal => {}
                                Completion::Break(None) => return Ok(Completion::Normal),
                                c => return Ok(c),
                            }
                        }
                    }
                }
                Ok(Completion::Normal)
            }
        }
    }

    fn assign_for_left(&mut self, left: &Stmt, value: Value, env: &Env) -> error::Result<()> {
        match left {
            Stmt::VarDecl { decls, .. } => {
                if let Some((name, _)) = decls.first() {
                    env.declare(name, value, BindingKind::Let);
                }
            }
            _ => return Err(Error::internal("unsupported for-in/of left side".to_string())),
        }
        Ok(())
    }

    fn iter_to_values(&mut self, v: &Value) -> error::Result<Vec<Value>> {
        match v {
            Value::Object(o) => {
                let o = o.borrow();
                match &o.internal {
                    InternalData::Array(items) => Ok(items.clone()),
                    _ => Err(Error::type_err("not iterable".to_string())),
                }
            }
            Value::String(s) => Ok(s.chars().map(|c| Value::from_str(&c.to_string())).collect()),
            _ => Err(Error::type_err(format!("{} is not iterable", v.type_of()))),
        }
    }

    fn own_enum_keys(&self, v: &Value) -> Vec<String> {
        match v {
            Value::Object(o) => {
                let o = o.borrow();
                if let InternalData::Array(items) = &o.internal {
                    (0..items.len()).map(|i| i.to_string()).collect()
                } else {
                    o.props.iter()
                        .filter(|(_, d)| d.enumerable)
                        .map(|(k, _)| k.to_string())
                        .collect()
                }
            }
            Value::String(s) => (0..s.chars().count()).map(|i| i.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn eval(&mut self, expr: &Expr, env: &Env) -> error::Result<Value> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::String(s) => Ok(Value::String(s.clone())),
            Expr::TemplateStr(s) => Ok(Value::String(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Undefined => Ok(Value::Undefined),
            Expr::This => env.get("this").or(Ok(Value::Undefined)),
            Expr::Ident(name) => env.get(name),
            Expr::Array(elements) => {
                let mut items = Vec::new();
                for e in elements {
                    if let Expr::Spread(inner) = e {
                        let v = self.eval(inner, env)?;
                        items.extend(self.iter_to_values(&v)?);
                    } else {
                        items.push(self.eval(e, env)?);
                    }
                }
                let mut obj = Obj::new_array();
                obj.proto = Some(self.array_proto.clone());
                obj.internal = InternalData::Array(items);
                Ok(Value::Object(Rc::new(RefCell::new(obj))))
            }
            Expr::Object(props) => {
                let mut obj = Obj::new();
                obj.proto = Some(self.object_proto.clone());
                for p in props {
                    let key = self.prop_key_str(&p.key, env)?;
                    let val = self.eval(&p.value, env)?;
                    obj.props.insert(Rc::from(key.as_str()), PropertyDescriptor::data(val));
                }
                Ok(Value::Object(Rc::new(RefCell::new(obj))))
            }
            Expr::Function(f) => Ok(self.make_function(f.clone(), env.clone())),
            Expr::Arrow(f) => Ok(self.make_function(f.clone(), env.clone())),
            Expr::Unary(op, e) => self.eval_unary(op, e, env),
            Expr::Update(op, prefix, e) => self.eval_update(op, *prefix, e, env),
            Expr::Binary(op, l, r) => {
                let lv = self.eval(l, env)?;
                let rv = self.eval(r, env)?;
                self.eval_binary(op, lv, rv)
            }
            Expr::Logical(op, l, r) => {
                let lv = self.eval(l, env)?;
                match op {
                    LogicalOp::And => if !lv.is_truthy() { Ok(lv) } else { self.eval(r, env) },
                    LogicalOp::Or => if lv.is_truthy() { Ok(lv) } else { self.eval(r, env) },
                    LogicalOp::Nullish => if lv.is_nullish() { self.eval(r, env) } else { Ok(lv) },
                }
            }
            Expr::Assign(op, target, value) => self.eval_assign(op, target, value, env),
            Expr::Conditional(c, t, f) => {
                if self.eval(c, env)?.is_truthy() { self.eval(t, env) } else { self.eval(f, env) }
            }
            Expr::Call { callee, args } => self.eval_call(callee, args, env),
            Expr::New { callee, args } => self.eval_new(callee, args, env),
            Expr::Member { object, property, computed } => self.eval_member(object, property, *computed, env),
            Expr::Spread(_) => Err(Error::internal("spread outside context".to_string())),
            Expr::Sequence(exprs) => {
                let mut last = Value::Undefined;
                for e in exprs { last = self.eval(e, env)?; }
                Ok(last)
            }
            Expr::TemplateTagged(_, _) => Err(Error::internal("tagged templates not supported".to_string())),
        }
    }

    fn prop_key_str(&mut self, key: &PropertyKey, _env: &Env) -> error::Result<String> {
        Ok(match key {
            PropertyKey::Ident(s) => s.to_string(),
            PropertyKey::String(s) => s.to_string(),
            PropertyKey::Number(n) => crate::value::num_to_string(*n),
        })
    }

    fn eval_unary(&mut self, op: &UnOp, e: &Expr, env: &Env) -> error::Result<Value> {
        match op {
            UnOp::Typeof => {
                // typeof undefined variable should not throw
                let v = if let Expr::Ident(name) = e {
                    if env.has(name) { self.eval(e, env)? } else { Value::Undefined }
                } else { self.eval(e, env)? };
                Ok(Value::from_str(v.type_of()))
            }
            UnOp::Void => { self.eval(e, env)?; Ok(Value::Undefined) }
            UnOp::Delete => {
                if let Expr::Member { object, property, computed } = e {
                    let obj = self.eval(object, env)?;
                    let key = if *computed {
                        let k = self.eval(property, env)?;
                        self.to_property_key(&k)?
                    } else if let Expr::String(s) = &**property {
                        s.to_string()
                    } else { String::new() };
                if let Value::Object(o) = obj {
                    let key_rc: Rc<str> = Rc::from(key.as_str());
                    o.borrow_mut().props.remove(&*key_rc);
                }
                Ok(Value::Bool(true))
                } else { Ok(Value::Bool(true)) }
            }
            _ => {
                let v = self.eval(e, env)?;
                match op {
                    UnOp::Neg => Ok(Value::Number(-self.to_number(&v)?)),
                    UnOp::Not => Ok(Value::Bool(!v.is_truthy())),
                    UnOp::BitNot => {
                        let n = self.to_number(&v)? as i32;
                        Ok(Value::Number(!n as f64))
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    fn eval_update(&mut self, op: &UpdateOp, prefix: bool, e: &Expr, env: &Env) -> error::Result<Value> {
        let old = self.eval(e, env)?;
        let old_n = self.to_number(&old)?;
        let new_n = match op { UpdateOp::Inc => old_n + 1.0, UpdateOp::Dec => old_n - 1.0 };
        self.assign_to_target(e, Value::Number(new_n), env)?;
        Ok(if prefix { Value::Number(new_n) } else { Value::Number(old_n) })
    }

    fn assign_to_target(&mut self, target: &Expr, value: Value, env: &Env) -> error::Result<()> {
        match target {
            Expr::Ident(name) => { env.set(name, value)?; Ok(()) }
            Expr::Member { object, property, computed } => {
                let obj = self.eval(object, env)?;
                let key = if *computed {
                    let k = self.eval(property, env)?;
                    self.to_property_key(&k)?
                } else if let Expr::String(s) = &**property {
                    s.to_string()
                } else { String::new() };
                self.set_property(&obj, &key, value)
            }
            _ => Err(Error::reference("invalid assignment target".to_string())),
        }
    }

    fn eval_assign(&mut self, op: &AssignOp, target: &Expr, value: &Expr, env: &Env) -> error::Result<Value> {
        if matches!(op, AssignOp::Assign) {
            let v = self.eval(value, env)?;
            self.assign_to_target(target, v.clone(), env)?;
            return Ok(v);
        }
        // compound assignment
        let cur = self.eval(target, env)?;
        let rhs = self.eval(value, env)?;
        let bin_op = match op {
            AssignOp::AddAssign => BinOp::Add,
            AssignOp::SubAssign => BinOp::Sub,
            AssignOp::MulAssign => BinOp::Mul,
            AssignOp::DivAssign => BinOp::Div,
            AssignOp::ModAssign => BinOp::Mod,
            AssignOp::PowAssign => BinOp::Pow,
            AssignOp::BitAndAssign => BinOp::BitAnd,
            AssignOp::BitOrAssign => BinOp::BitOr,
            AssignOp::BitXorAssign => BinOp::BitXor,
            AssignOp::ShlAssign => BinOp::Shl,
            AssignOp::ShrAssign => BinOp::Shr,
            AssignOp::UshrAssign => BinOp::Ushr,
            AssignOp::AndAssign => {
                let v = if cur.is_truthy() { rhs } else { cur };
                self.assign_to_target(target, v.clone(), env)?;
                return Ok(v);
            }
            AssignOp::OrAssign => {
                let v = if cur.is_truthy() { cur } else { rhs };
                self.assign_to_target(target, v.clone(), env)?;
                return Ok(v);
            }
            AssignOp::NullishAssign => {
                let v = if cur.is_nullish() { rhs } else { cur };
                self.assign_to_target(target, v.clone(), env)?;
                return Ok(v);
            }
            AssignOp::Assign => unreachable!(),
        };
        let v = self.eval_binary(&bin_op, cur, rhs)?;
        self.assign_to_target(target, v.clone(), env)?;
        Ok(v)
    }

    pub fn eval_binary(&mut self, op: &BinOp, l: Value, r: Value) -> error::Result<Value> {
        match op {
            BinOp::Add => {
                let lp = self.to_primitive(&l)?;
                let rp = self.to_primitive(&r)?;
                match (&lp, &rp) {
                    (Value::String(_), _) | (_, Value::String(_)) => {
                        Ok(Value::String(Rc::from(format!("{}{}", self.to_string_val(&lp)?, self.to_string_val(&rp)?))))
                    }
                    _ => Ok(Value::Number(self.to_number(&lp)? + self.to_number(&rp)?)),
                }
            }
            BinOp::Sub => Ok(Value::Number(self.to_number(&l)? - self.to_number(&r)?)),
            BinOp::Mul => Ok(Value::Number(self.to_number(&l)? * self.to_number(&r)?)),
            BinOp::Div => Ok(Value::Number(self.to_number(&l)? / self.to_number(&r)?)),
            BinOp::Mod => {
                let a = self.to_number(&l)?;
                let b = self.to_number(&r)?;
                Ok(Value::Number(a % b))
            }
            BinOp::Pow => Ok(Value::Number(self.to_number(&l)?.powf(self.to_number(&r)?))),
            BinOp::Eq => Ok(Value::Bool(self.loose_equals(&l, &r)?)),
            BinOp::NotEq => Ok(Value::Bool(!self.loose_equals(&l, &r)?)),
            BinOp::StrictEq => Ok(Value::Bool(self.strict_equals(&l, &r)?)),
            BinOp::StrictNotEq => Ok(Value::Bool(!self.strict_equals(&l, &r)?)),
            BinOp::Lt => self.compare_lt(&l, &r, false),
            BinOp::Gt => self.compare_lt(&r, &l, false),
            BinOp::Lte => self.compare_lt(&l, &r, true),
            BinOp::Gte => self.compare_lt(&r, &l, true),
            BinOp::BitAnd => Ok(Value::Number((self.to_number(&l)? as i32 & self.to_number(&r)? as i32) as f64)),
            BinOp::BitOr => Ok(Value::Number((self.to_number(&l)? as i32 | self.to_number(&r)? as i32) as f64)),
            BinOp::BitXor => Ok(Value::Number((self.to_number(&l)? as i32 ^ self.to_number(&r)? as i32) as f64)),
            BinOp::Shl => Ok(Value::Number(((self.to_number(&l)? as i32) << (self.to_number(&r)? as u32 & 31)) as f64)),
            BinOp::Shr => Ok(Value::Number(((self.to_number(&l)? as i32) >> (self.to_number(&r)? as u32 & 31)) as f64)),
            BinOp::Ushr => Ok(Value::Number((((self.to_number(&l)? as i32) as u32) >> (self.to_number(&r)? as u32 & 31)) as f64)),
            BinOp::In => {
                match &r {
                    Value::Object(o) => {
                        let key: String = self.to_property_key(&l)?;
                        let key_rc: Rc<str> = Rc::from(key.as_str());
                        Ok(Value::Bool(o.borrow().props.contains_key(&*key_rc)))
                    }
                    _ => Err(Error::type_err("Cannot use 'in' on non-object".to_string())),
                }
            }
            BinOp::Instanceof => {
                self.instance_of(&l, &r)
            }
        }
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr], env: &Env) -> error::Result<Value> {
        // method call: need `this`
        let (func, this_val) = match callee {
            Expr::Member { object, property, computed } => {
                let obj = self.eval(object, env)?;
                let key = if *computed {
                    let k = self.eval(property, env)?;
                    self.to_property_key(&k)?
                } else if let Expr::String(s) = &**property {
                    s.to_string()
                } else { String::new() };
                let f = self.get_property(&obj, &key)?;
                (f, obj)
            }
            _ => {
                let f = self.eval(callee, env)?;
                (f, Value::Undefined)
            }
        };
        // evaluate args
        let mut arg_vals = Vec::new();
        for a in args {
            if let Expr::Spread(inner) = a {
                let v = self.eval(inner, env)?;
                arg_vals.extend(self.iter_to_values(&v)?);
            } else {
                arg_vals.push(self.eval(a, env)?);
            }
        }
        self.call_function(&func, &arg_vals, Some(this_val))
    }

    pub fn call_function(&mut self, func: &Value, args: &[Value], this_val: Option<Value>) -> error::Result<Value> {
        let fv = match func {
            Value::Function(f) => f.clone(),
            _ => return Err(Error::type_err(format!("{} is not a function", func.type_of()))),
        };
        match &fv.kind {
            FunctionKind::Native { func, .. } => func(self, args, this_val),
            FunctionKind::Interpreted { func } => {
                let func_env = fv.closure.child();
                // bind arguments
                for (i, param) in func.params.iter().enumerate() {
                    let v = args.get(i).cloned().unwrap_or(Value::Undefined);
                    func_env.declare(param, v, BindingKind::Let);
                }
                // arguments object (simplified as array)
                let mut arr = Obj::new_array();
                arr.internal = InternalData::Array(args.to_vec());
                func_env.declare("arguments", Value::Object(Rc::new(RefCell::new(arr))), BindingKind::Const);
                // this binding (arrows inherit `this` from their closure scope)
                if !func.is_arrow {
                    func_env.declare("this", this_val.unwrap_or(Value::Undefined), BindingKind::Const);
                }
                self.hoist(&func.body, &func_env);
                for stmt in &func.body {
                    match self.exec_stmt(stmt, &func_env)? {
                        Completion::Return(v) => return Ok(v),
                        Completion::Normal => {}
                        _ => {}
                    }
                }
                Ok(Value::Undefined)
            }
            FunctionKind::Bound { target, this_val, bound_args } => {
                let mut all = bound_args.clone();
                all.extend_from_slice(args);
                self.call_function(&Value::Function(target.clone()), &all, Some(this_val.clone()))
            }
        }
    }

    fn eval_new(&mut self, callee: &Expr, args: &[Expr], env: &Env) -> error::Result<Value> {
        let constructor = self.eval(callee, env)?;
        let mut arg_vals = Vec::new();
        for a in args {
            if let Expr::Spread(inner) = a {
                let v = self.eval(inner, env)?;
                arg_vals.extend(self.iter_to_values(&v)?);
            } else {
                arg_vals.push(self.eval(a, env)?);
            }
        }
        self.construct(&constructor, &arg_vals)
    }

    fn construct(&mut self, constructor: &Value, args: &[Value]) -> error::Result<Value> {
        let fv = match constructor {
            Value::Function(f) => f.clone(),
            _ => return Err(Error::type_err("not a constructor".to_string())),
        };
        // get prototype
        let proto = fv.properties.borrow().get("prototype").map(|d| d.value.clone()).unwrap_or(self.object_proto.clone());
        let mut obj = Obj::new();
        obj.proto = match &proto { Value::Object(_) => Some(proto.clone()), _ => Some(self.object_proto.clone()) };
        let this_obj = Value::Object(Rc::new(RefCell::new(obj)));
        let result = self.call_function(constructor, args, Some(this_obj.clone()))?;
        if matches!(result, Value::Object(_) | Value::Function(_)) {
            Ok(result)
        } else {
            Ok(this_obj)
        }
    }

    fn eval_member(&mut self, object: &Expr, property: &Expr, computed: bool, env: &Env) -> error::Result<Value> {
        let obj = self.eval(object, env)?;
        let key = if computed {
            let k = self.eval(property, env)?;
            self.to_property_key(&k)?
        } else if let Expr::String(s) = property {
            s.to_string()
        } else { String::new() };
        self.get_property(&obj, &key)
    }

    // ---- type conversions ----

    pub fn to_number(&mut self, v: &Value) -> error::Result<f64> {
        Ok(match v {
            Value::Undefined => f64::NAN,
            Value::Null => 0.0,
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::Number(n) => *n,
            Value::String(s) => {
                let t = s.trim();
                if t.is_empty() { 0.0 }
                else { t.parse::<f64>().unwrap_or(f64::NAN) }
            }
            Value::Object(o) => {
                let o = o.borrow();
                match &o.internal {
                    InternalData::None => f64::NAN,
                    InternalData::Array(items) => {
                        if items.is_empty() { 0.0 }
                        else if items.len() == 1 { self.to_number(&items[0])? }
                        else { f64::NAN }
                    }
                    InternalData::String(s) => s.parse::<f64>().unwrap_or(f64::NAN),
                    InternalData::Number(n) => *n,
                    InternalData::Boolean(b) => if *b { 1.0 } else { 0.0 },
                }
            }
            Value::Function(_) => f64::NAN,
        })
    }

    pub fn to_string_val(&mut self, v: &Value) -> error::Result<Rc<str>> {
        Ok(match v {
            Value::Undefined => Rc::from("undefined"),
            Value::Null => Rc::from("null"),
            Value::Bool(b) => Rc::from(b.to_string().as_str()),
            Value::Number(n) => Rc::from(crate::value::num_to_string(*n).as_str()),
            Value::String(s) => s.clone(),
            Value::Object(o) => {
                let o = o.borrow();
                match &o.internal {
                    InternalData::None => Rc::from("[object Object]"),
                    InternalData::Array(items) => {
                        let parts: Vec<String> = items.iter()
                            .map(|i| if i.is_nullish() { String::new() } else { self.to_string_val(i).map(|s| s.to_string()).unwrap_or_default() })
                            .collect();
                        Rc::from(parts.join(",").as_str())
                    }
                    InternalData::String(s) => s.clone(),
                    InternalData::Number(n) => Rc::from(crate::value::num_to_string(*n).as_str()),
                    InternalData::Boolean(b) => Rc::from(b.to_string().as_str()),
                }
            }
            Value::Function(f) => {
                match &f.name {
                    Some(n) => Rc::from(format!("function {}() {{ [native code] }}", n).as_str()),
                    None => Rc::from("function () { [native code] }"),
                }
            }
        })
    }

    pub fn to_primitive(&mut self, v: &Value) -> error::Result<Value> {
        match v {
            Value::Object(o) => {
                let o = o.borrow();
                match &o.internal {
                    InternalData::None => Ok(Value::from_str("[object Object]")),
                    InternalData::Array(items) => {
                        let parts: Vec<String> = items.iter()
                            .map(|i| if i.is_nullish() { String::new() } else { self.to_string_val(i).map(|s| s.to_string()).unwrap_or_default() })
                            .collect();
                        Ok(Value::String(Rc::from(parts.join(",").as_str())))
                    }
                    InternalData::String(s) => Ok(Value::String(s.clone())),
                    InternalData::Number(n) => Ok(Value::Number(*n)),
                    InternalData::Boolean(b) => Ok(Value::Bool(*b)),
                }
            }
            _ => Ok(v.clone()),
        }
    }

    pub fn to_property_key(&mut self, v: &Value) -> error::Result<String> {
        match v {
            Value::String(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(crate::value::num_to_string(*n)),
            _ => Ok(self.to_string_val(v)?.to_string()),
        }
    }

    pub fn to_boolean(&self, v: &Value) -> bool { v.is_truthy() }

    pub fn strict_equals(&mut self, a: &Value, b: &Value) -> error::Result<bool> {
        Ok(match (a, b) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(x), Value::Number(y)) => {
                if x.is_nan() || y.is_nan() { false } else { x == y }
            }
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => Rc::ptr_eq(x, y),
            (Value::Function(x), Value::Function(y)) => Rc::ptr_eq(x, y),
            _ => false,
        })
    }

    pub fn loose_equals(&mut self, a: &Value, b: &Value) -> error::Result<bool> {
        if std::mem::discriminant(a) == std::mem::discriminant(b) {
            return self.strict_equals(a, b);
        }
        Ok(match (a, b) {
            (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => true,
            (Value::Number(_), Value::String(_)) => {
                let bn = Value::Number(self.to_number(b)?);
                self.strict_equals(a, &bn)?
            }
            (Value::String(_), Value::Number(_)) => {
                let an = Value::Number(self.to_number(a)?);
                self.strict_equals(&an, b)?
            }
            (Value::Bool(_), _) => {
                let an = Value::Number(self.to_number(a)?);
                self.loose_equals(&an, b)?
            }
            (_, Value::Bool(_)) => {
                let bn = Value::Number(self.to_number(b)?);
                self.loose_equals(a, &bn)?
            }
            (Value::Object(_), Value::Number(_)) | (Value::Object(_), Value::String(_)) => {
                let pa = self.to_primitive(a)?;
                self.loose_equals(&pa, b)?
            }
            (Value::Number(_), Value::Object(_)) | (Value::String(_), Value::Object(_)) => {
                let pb = self.to_primitive(b)?;
                self.loose_equals(a, &pb)?
            }
            _ => false,
        })
    }

    fn compare_lt(&mut self, a: &Value, b: &Value, allow_eq: bool) -> error::Result<Value> {
        let pa = self.to_primitive(a)?;
        let pb = self.to_primitive(b)?;
        if let (Value::String(x), Value::String(y)) = (&pa, &pb) {
            return Ok(Value::Bool(if allow_eq { x <= y } else { x < y }));
        }
        let na = self.to_number(&pa)?;
        let nb = self.to_number(&pb)?;
        if na.is_nan() || nb.is_nan() { return Ok(Value::Bool(false)); }
        Ok(Value::Bool(if allow_eq { na <= nb } else { na < nb }))
    }

    pub fn instance_of(&mut self, v: &Value, constructor: &Value) -> error::Result<Value> {
        let proto = match constructor {
            Value::Function(f) => f.properties.borrow().get("prototype").map(|d| d.value.clone()).unwrap_or(Value::Undefined),
            _ => return Err(Error::type_err("Right-hand side of instanceof is not callable".to_string())),
        };
        let obj_proto = match v {
            Value::Object(o) => o.borrow().proto.clone(),
            _ => return Ok(Value::Bool(false)),
        };
        let mut cur = obj_proto;
        while let Some(p) = cur {
            if self.strict_equals(&p, &proto)? { return Ok(Value::Bool(true)); }
            cur = match &p { Value::Object(o) => o.borrow().proto.clone(), _ => None };
        }
        Ok(Value::Bool(false))
    }

    // ---- property access ----

    pub fn get_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        // array index / length
        match obj {
            Value::Number(_) => {
                self.get_property(&self.number_proto.clone(), key)
            }
            Value::Bool(_) => {
                self.get_property(&self.boolean_proto.clone(), key)
            }
            Value::Function(f) => {
                if let Some(d) = f.properties.borrow().get(key) {
                    return Ok(d.value.clone());
                }
                // fall through to function prototype
                let proto = self.function_proto.clone();
                self.get_property(&proto, key)
            }
            Value::String(s) => {
                if key == "length" {
                    return Ok(Value::Number(s.chars().count() as f64));
                }
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(c) = s.chars().nth(idx) {
                        return Ok(Value::from_str(&c.to_string()));
                    }
                    return Ok(Value::Undefined);
                }
                // fall through to string prototype
                self.get_proto_property(obj, key)
            }
            Value::Object(o) => {
                let o = o.borrow();
                if let InternalData::Array(items) = &o.internal {
                    if key == "length" {
                        return Ok(Value::Number(items.len() as f64));
                    }
                    if let Ok(idx) = key.parse::<usize>() {
                        if idx < items.len() {
                            return Ok(items[idx].clone());
                        }
                        return Ok(Value::Undefined);
                    }
                }
                if let Some(d) = o.props.get(key) {
                    if d.is_accessor {
                        if let Some(get) = d.get.clone() {
                            drop(o);
                            return self.call_function(&get, &[], Some(obj.clone()));
                        }
                        return Ok(Value::Undefined);
                    }
                    return Ok(d.value.clone());
                }
                drop(o);
                self.get_proto_property(obj, key)
            }
            _ => Ok(Value::Undefined),
        }
    }

    fn get_proto_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        let proto = match obj {
            Value::Object(o) => o.borrow().proto.clone(),
            Value::String(_) => Some(self.string_proto.clone()),
            _ => return Ok(Value::Undefined),
        };
        if let Some(p) = proto {
            return self.get_property(&p, key);
        }
        Ok(Value::Undefined)
    }

    pub fn set_property(&mut self, obj: &Value, key: &str, value: Value) -> error::Result<()> {
        match obj {
            Value::Function(f) => {
                f.properties.borrow_mut().insert(Rc::from(key), PropertyDescriptor::data(value));
                Ok(())
            }
            Value::Object(o) => {
                let mut o_ref = o.borrow_mut();
                if let InternalData::Array(items) = &mut o_ref.internal {
                    if key == "length" {
                        let n = self.to_number(&value)? as usize;
                        items.truncate(n);
                        while items.len() < n { items.push(Value::Undefined); }
                        return Ok(());
                    }
                    if let Ok(idx) = key.parse::<usize>() {
                        while items.len() <= idx { items.push(Value::Undefined); }
                        items[idx] = value;
                        return Ok(());
                    }
                }
                o_ref.props.insert(Rc::from(key), PropertyDescriptor::data(value));
                Ok(())
            }
            _ => Err(Error::type_err("Cannot set property of primitive".to_string())),
        }
    }

    // ---- error conversion ----

    fn _value_to_error(&self, v: Value) -> Rc<Error> {
        let msg = if let Value::Object(o) = &v {
            let o = o.borrow();
            if let Some(d) = o.props.get("message") {
                match &d.value {
                    Value::String(s) => s.to_string(),
                    other => format!("{:?}", other),
                }
            } else { format!("{:?}", v) }
        } else { format!("{:?}", v) };
        Error::user(msg)
    }

    fn error_to_value(&self, err: Rc<Error>) -> Value {
        let mut obj = Obj::new();
        obj.proto = Some(self.error_proto.clone());
        let msg = err.message.clone();
        let name = match err.kind {
            crate::error::ErrorKind::Syntax => "SyntaxError",
            crate::error::ErrorKind::Reference => "ReferenceError",
            crate::error::ErrorKind::Type => "TypeError",
            crate::error::ErrorKind::Range => "RangeError",
            crate::error::ErrorKind::Eval => "EvalError",
            crate::error::ErrorKind::Uri => "URIError",
            crate::error::ErrorKind::User => "Error",
            crate::error::ErrorKind::Internal => "InternalError",
        };
        obj.props.insert(Rc::from("message"), PropertyDescriptor::data(Value::from_str(&msg)));
        obj.props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str(name)));
        Value::Object(Rc::new(RefCell::new(obj)))
    }
}

// ---- public helpers for builtins ----
impl Interpreter {
    pub fn iter_to_values_pub(&mut self, v: &Value) -> error::Result<Vec<Value>> {
        self.iter_to_values(v)
    }

    pub fn array_proto_pub(&self) -> Value {
        self.array_proto.clone()
    }

    pub fn new_array(&mut self, items: Vec<Value>) -> Value {
        let mut o = Obj::new_array();
        o.proto = Some(self.array_proto.clone());
        o.internal = InternalData::Array(items);
        Value::Object(Rc::new(RefCell::new(o)))
    }
    pub fn own_enum_keys_pub(&mut self, v: &Value) -> Vec<String> {
        self.own_enum_keys(v)
    }

    // ---- JSON ----
    pub fn parse_json(&mut self, s: &str) -> error::Result<Value> {
        let mut p = crate::json::JsonParser { chars: s.chars().collect(), pos: 0 };
        p.skip_ws();
        let v = p.parse_value(self)?;
        Ok(v)
    }

    pub fn stringify_json(&mut self, v: &Value, indent: Option<&str>) -> Option<String> {
        match v {
            Value::Undefined => None,
            Value::Null => Some("null".to_string()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(n) => {
                if n.is_nan() || n.is_infinite() { None }
                else { Some(crate::value::num_to_string(*n)) }
            }
            Value::String(s) => Some(self.json_quote(s)),
            Value::Function(_) => None,
            Value::Object(o) => {
                let o = o.borrow();
                match &o.internal {
                    InternalData::Array(items) => {
                        let parts: Vec<String> = items.iter()
                            .filter_map(|i| self.stringify_json(i, indent))
                            .collect();
                        Some(format!("[{}]", parts.join(",")))
                    }
                    _ => {
                        let mut pairs = Vec::new();
                        for (k, d) in o.props.iter() {
                            if !d.enumerable { continue; }
                            if let Some(vs) = self.stringify_json(&d.value, indent) {
                                pairs.push(format!("{}:{}", self.json_quote(k), vs));
                            }
                        }
                        Some(format!("{{{}}}", pairs.join(",")))
                    }
                }
            }
        }
    }

    fn json_quote(&self, s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

impl Interpreter {
    pub fn to_string_pub(&mut self, v: &Value) -> error::Result<String> {
        Ok(self.to_string_val(v)?.to_string())
    }
}
