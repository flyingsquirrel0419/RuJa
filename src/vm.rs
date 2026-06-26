//! Stack-based bytecode VM.

use crate::bytecode::{Chunk, Op};
use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{GcIdx, HeapObj, Value};
use std::cell::RefCell;
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
    pub map_proto: Value,
    pub set_proto: Value,
    pub microtask_queue: Vec<Microtask>,
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
}

pub enum Microtask {
    Then { promise: GcIdx, on_fulfilled: Value, on_rejected: Value },
    Resolve { promise: GcIdx, value: Value },
    Reject { promise: GcIdx, reason: Value },
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
            map_proto: Value::Undefined,
            set_proto: Value::Undefined,
            microtask_queue: Vec::new(),
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
        let base = self.functions.len();
        self.functions.extend(funcs);
        self.execute_chunk(chunk, self.global, Value::Undefined)
    }

    fn execute_chunk(&mut self, chunk: Chunk, env: GcIdx, this_val: Value) -> error::Result<Value> {
        let chunk = Rc::new(chunk);
        self.frames.push(CallFrame {
            chunk: chunk.clone(),
            ip: 0,
            locals: vec![Value::Undefined; 256],
            env,
            catch_stack: Vec::new(),
            this_val,
        });
        self.interpret()
    }

    /// Execute a compiled function's chunk in a new frame.
    fn execute_chunk_func(&mut self, fdef: Rc<crate::function::FunctionDef>, env: GcIdx, this_val: Value, args: &[Value]) -> error::Result<Value> {
        let mut locals = vec![Value::Undefined; fdef.num_locals.max(256)];
        for (i, a) in args.iter().enumerate().take(fdef.params.len()) {
            if i < locals.len() { locals[i] = a.clone(); }
        }
        self.frames.push(CallFrame {
            chunk: fdef.chunk.clone(),
            ip: 0,
            locals,
            env,
            catch_stack: Vec::new(),
            this_val,
        });
        // Run only this function's frame. interpret returns when its frame pops.
        let target_depth = self.frames.len() - 1;
        self.interpret_to_depth(target_depth)
    }

    fn interpret(&mut self) -> error::Result<Value> {
        self.interpret_inner(None)
    }

    fn interpret_to_depth(&mut self, target_depth: usize) -> error::Result<Value> {
        self.interpret_inner(Some(target_depth))
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
                    let val = crate::environment::get(&self.heap, cur_env, &name)
                        .or_else(|| crate::environment::get(&self.heap, self.global, &name));
                    match val {
                        Some(v) => self.stack.push(v),
                        None => return Err(Error::reference(format!("{} is not defined", name))),
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
                    if !crate::environment::set(&self.heap, cur_env, &name, value.clone()) {
                        crate::environment::declare(&self.heap, self.global, &name, value, crate::value::BindingKind::Var);
                    }
                    self.stack.push(Value::Undefined);
                }
                Op::DeclareEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame.chunk.constants.get(name_idx).cloned().unwrap_or(Value::Undefined);
                        match v { Value::String(s) => s.to_string(), _ => String::new() }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    crate::environment::declare(&self.heap, cur_env, &name, value, crate::value::BindingKind::Let);
                }
                Op::LoadEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame.chunk.constants.get(name_idx).cloned().unwrap_or(Value::Undefined);
                        match v { Value::String(s) => s.to_string(), _ => String::new() }
                    };
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    match crate::environment::get(&self.heap, cur_env, &name) {
                        Some(v) => self.stack.push(v),
                        None => match crate::environment::get(&self.heap, self.global, &name) {
                            Some(v) => self.stack.push(v),
                            None => return Err(Error::reference(format!("{} is not defined", name))),
                        }
                    }
                }
                Op::StoreEnv(name_idx) => {
                    let name = {
                        let frame = self.frames.last().unwrap();
                        let v = frame.chunk.constants.get(name_idx).cloned().unwrap_or(Value::Undefined);
                        match v { Value::String(s) => s.to_string(), _ => String::new() }
                    };
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let cur_env = self.frames.last().map(|f| f.env).unwrap_or(self.global);
                    if !crate::environment::set(&self.heap, cur_env, &name, value.clone()) {
                        crate::environment::declare(&self.heap, cur_env, &name, value, crate::value::BindingKind::Var);
                    }
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
                Op::Pop => { self.stack.pop(); }
                Op::Dup => { let v = self.stack.last().cloned().unwrap_or(Value::Undefined); self.stack.push(v); }
                Op::Swap => {
                    let len = self.stack.len();
                    if len >= 2 { self.stack.swap(len-1, len-2); }
                }
                Op::Rot3 => {
                    let len = self.stack.len();
                    if len >= 3 {
                        let c = self.stack.remove(len-3);
                        self.stack.push(c);
                    }
                }
                Op::Add => self.bin_op(|a, b| Value::Number(a + b), |a, b| Value::String(Rc::from(format!("{}{}", a, b).as_str())))?,
                Op::Sub => self.num_bin(|a, b| a - b)?,
                Op::Mul => self.num_bin(|a, b| a * b)?,
                Op::Div => self.num_bin(|a, b| a / b)?,
                Op::Mod => self.num_bin(|a, b| a % b)?,
                Op::Pow => self.num_bin(|a, b| a.powf(b))?,
                Op::Neg => { let v = self.stack.pop().unwrap_or(Value::Undefined); let n = self.to_number(&v)?; self.stack.push(Value::Number(-n)); }
                Op::Not => { let v = self.stack.pop().unwrap_or(Value::Undefined); let b = v.is_truthy(); self.stack.push(Value::Bool(!b)); }
                Op::BitNot => { let v = self.stack.pop().unwrap_or(Value::Undefined); let n = self.to_number(&v)? as i32; self.stack.push(Value::Number(!n as f64)); }
                Op::Eq => { let (a, b) = self.pop2(); let r = self.loose_eq(&a, &b)?; self.stack.push(Value::Bool(r)); }
                Op::NotEq => { let (a, b) = self.pop2(); let r = self.loose_eq(&a, &b)?; self.stack.push(Value::Bool(!r)); }
                Op::StrictEq => { let (a, b) = self.pop2(); let r = self.strict_eq(&a, &b); self.stack.push(Value::Bool(r)); }
                Op::StrictNotEq => { let (a, b) = self.pop2(); let r = self.strict_eq(&a, &b); self.stack.push(Value::Bool(!r)); }
                Op::Lt => self.compare(|a, b| a < b, true)?,
                Op::Gt => self.compare(|a, b| a > b, false)?,
                Op::Lte => self.compare(|a, b| a <= b, true)?,
                Op::Gte => self.compare(|a, b| a >= b, false)?,
                Op::BitAnd => self.int_bin(|a, b| a & b)?,
                Op::BitOr => self.int_bin(|a, b| a | b)?,
                Op::BitXor => self.int_bin(|a, b| a ^ b)?,
                Op::Shl => self.int_bin(|a, b| a << (b as u32 & 31))?,
                Op::Shr => self.int_bin(|a, b| a >> (b as u32 & 31))?,
                Op::Ushr => self.int_bin(|a, b| ((a as u32) >> (b as u32 & 31)) as i32)?,
                Op::Jump(target) => { self.frames.last_mut().unwrap().ip = target; }
                Op::JumpIfFalse(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if !v.is_truthy() { self.frames.last_mut().unwrap().ip = target; }
                }
                Op::JumpIfTrue(target) => {
                    let v = self.stack.pop().unwrap_or(Value::Undefined);
                    if v.is_truthy() { self.frames.last_mut().unwrap().ip = target; }
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
                        props: RefCell::new(HashMap::new()),
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
                        props: RefCell::new(HashMap::new()),
                        proto: RefCell::new(Some(self.array_proto.clone())),
                    });
                    let idx = self.heap.allocate(obj);
                    self.stack.push(Value::Object(GcIdx(idx)));
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
                    let key_str = self.to_property_key(&key)?;
                    let v = self.get_property(&obj, &key_str)?;
                    self.stack.push(v);
                }
                Op::SetProp => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    self.set_property(&obj, &key_str, value)?;
                    self.stack.push(Value::Undefined);
                }
                Op::SetElem => {
                    let value = self.stack.pop().unwrap_or(Value::Undefined);
                    let key = self.stack.pop().unwrap_or(Value::Undefined);
                    let obj = self.stack.pop().unwrap_or(Value::Undefined);
                    let key_str = self.to_property_key(&key)?;
                    self.set_property(&obj, &key_str, value)?;
                    self.stack.push(Value::Undefined);
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
                                props: RefCell::new(HashMap::new()),
                                proto: RefCell::new(Some(self.object_proto.clone())),
                                extensible: std::cell::Cell::new(true),
                                class_name: None,
                            });
                            Value::Object(GcIdx(self.heap.allocate(proto)))
                        } else { Value::Undefined };
                        let fd = crate::value::FunctionData {
                            name: fdef.name.clone(),
                            kind: crate::value::FunctionKind::Interpreted { func: fdef },
                            closure: env_idx,
                            prototype: RefCell::new(if !is_arrow { Some(proto_val.clone()) } else { None }),
                            props: RefCell::new(HashMap::new()),
                        };
                        let idx = self.heap.allocate(HeapObj::Function(fd));
                        // link prototype.constructor back to the function
                        if let Value::Object(pidx) = &proto_val {
                            self.heap.with_obj(pidx.0, |obj| {
                                obj.props().borrow_mut().insert(Rc::from("constructor"), crate::value::PropertyDescriptor::data(Value::Object(GcIdx(idx))));
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
                        if self.heap.with_obj(idx.0, |o| o.is_function()) { "function" } else { "object" }
                    } else {
                        match &v { Value::Object(_) => "object", _ => v.type_of() }
                    };
                    self.stack.push(Value::String(Rc::from(t)));
                }
                _ => {
                    // Unimplemented op: skip for now
                }
            }
            // GC check
            if self.heap.live_count() > 0 && self.heap.live_count() % 4096 == 0 {
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

    fn bin_op<F: Fn(f64, f64) -> Value, G: Fn(&str, &str) -> Value>(&mut self, numf: F, strf: G) -> error::Result<()> {
        let (a, b) = self.pop2();
        // string concatenation
        let ap = self.to_primitive(&a)?;
        let bp = self.to_primitive(&b)?;
        match (&ap, &bp) {
            (Value::String(_), _) | (_, Value::String(_)) => {
                let sa = self.to_string(&ap)?;
                let sb = self.to_string(&bp)?;
                self.stack.push(Value::String(Rc::from(format!("{}{}", sa, sb).as_str())));
            }
            _ => {
                let av = self.to_number(&ap)?;
                let bv = self.to_number(&bp)?;
                self.stack.push(numf(av, bv));
            }
        }
        Ok(())
    }

    fn compare<F: Fn(f64, f64) -> bool>(&mut self, f: F, _eq: bool) -> error::Result<()> {
        let (a, b) = self.pop2();
        let pa = self.to_primitive(&a)?;
        let pb = self.to_primitive(&b)?;
        if let (Value::String(sa), Value::String(sb)) = (&pa, &pb) {
            self.stack.push(Value::Bool(f(0.0, 0.0) && sa == sb || sa < sb)); // simplified
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
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::Number(n) => *n,
            Value::String(s) => {
                let t = s.trim();
                if t.is_empty() { 0.0 } else { t.parse::<f64>().unwrap_or(f64::NAN) }
            }
            Value::Object(idx) => {
                self.heap.with_obj(idx.0, |obj| {
                    match obj {
                        HeapObj::Array(a) => {
                            let items = a.items.borrow();
                            if items.is_empty() { 0.0 }
                            else if items.len() == 1 { 0.0 } // recurse needed
                            else { f64::NAN }
                        }
                        _ => f64::NAN,
                    }
                })
            }
            Value::Symbol(_) => { return Err(Error::type_err("Cannot convert Symbol to number".to_string())); }
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
                let is_array = self.heap.with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_)));
                if is_array {
                    // join items outside the borrow
                    let items = self.heap.with_obj(idx.0, |obj| {
                        if let HeapObj::Array(a) = obj { a.items.borrow().clone() } else { Vec::new() }
                    });
                    let parts: Vec<String> = items.iter()
                        .map(|i| if i.is_nullish() { String::new() } else { self.to_string(i).map(|s| s.to_string()).unwrap_or_default() })
                        .collect();
                    Rc::from(parts.join(",").as_str())
                } else {
                    self.heap.with_obj(idx.0, |obj| {
                        match obj {
                            HeapObj::Object(o) => {
                                if let Some(cn) = &o.class_name { cn.clone() } else { Rc::from("[object Object]") }
                            }
                            _ => Rc::from("[object Object]"),
                        }
                    })
                }
            }
            Value::Symbol(_) => { return Err(Error::type_err("Cannot convert Symbol to string".to_string())); }
        })
    }

    pub fn to_primitive(&mut self, v: &Value) -> error::Result<Value> {
        match v {
            Value::Object(idx) => {
                // simplified: arrays/objects -> toString-ish
                let s = self.to_string(v)?;
                Ok(Value::String(s))
            }
            _ => Ok(v.clone()),
        }
    }

    pub fn to_property_key(&mut self, v: &Value) -> error::Result<String> {
        match v {
            Value::String(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(crate::value::num_to_string(*n)),
            Value::Symbol(_) => Err(Error::type_err("Symbol keys not yet supported in property access".to_string())),
            _ => Ok(self.to_string(v)?.to_string()),
        }
    }

    pub fn to_boolean(&self, v: &Value) -> bool { v.is_truthy() }

    pub fn strict_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(x), Value::Number(y)) => {
                if x.is_nan() || y.is_nan() { false } else { x == y }
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
            Value::Object(idx) => {
                // array
                let proto = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            return Ok::<Value, Error>(Value::Number(a.items.borrow().len() as f64));
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            let items = a.items.borrow();
                            return Ok(items.get(i).cloned().unwrap_or(Value::Undefined));
                        }
                    }
                let props = o.props();
                if let Some(desc) = props.borrow().get(key) {
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
                if !val.is_undefined() { return Ok(val); }
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
                                while items.len() < new_len { items.push(Value::Undefined); }
                            }
                            return;
                        }
                        if let Ok(i) = key.parse::<usize>() {
                            let mut items = a.items.borrow_mut();
                            while items.len() <= i { items.push(Value::Undefined); }
                            items[i] = value;
                            return;
                        }
                    }
                    let props = o.props();
                    props.borrow_mut().insert(Rc::from(key), crate::value::PropertyDescriptor::data(value));
                });
                Ok(())
            }
            _ => Err(Error::type_err("Cannot set property of primitive".to_string())),
        }
    }

    // ---- GC roots ----
    pub fn collect_roots(&self) -> Vec<usize> {
        let mut roots = vec![self.global.0];
        for v in &self.stack {
            if let Value::Object(idx) = v { roots.push(idx.0); }
        }
        for f in &self.frames {
            roots.push(f.env.0);
            if let Value::Object(idx) = &f.this_val { roots.push(idx.0); }
            for l in &f.locals {
                if let Value::Object(idx) = l { roots.push(idx.0); }
            }
        }
        for proto in [&self.object_proto, &self.array_proto, &self.function_proto,
                      &self.string_proto, &self.number_proto, &self.boolean_proto,
                      &self.error_proto, &self.symbol_proto, &self.promise_proto,
                      &self.iterator_proto, &self.map_proto, &self.set_proto] {
            if let Value::Object(idx) = proto { roots.push(idx.0); }
        }
        roots
    }

    pub fn gc(&self) {
        let roots = self.collect_roots();
        self.heap.collect(&roots);
    }

    /// Allocate a plain object and return its handle.
    pub fn new_object(&mut self) -> GcIdx {
        let obj = HeapObj::Object(crate::value::ObjectData {
            props: RefCell::new(HashMap::new()),
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
            props: RefCell::new(HashMap::new()),
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
    pub fn call_function(&mut self, func: &Value, args: &[Value], this: Option<Value>) -> error::Result<Value> {
        let idx = match func {
            Value::Object(idx) => *idx,
            _ => return Err(Error::type_err(format!("{} is not a function", func.type_of()))),
        };
        // read function kind without holding borrow
        let kind_info = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                match &f.kind {
                    crate::value::FunctionKind::Native { func, .. } => Some(FuncCallInfo::Native(*func)),
                    crate::value::FunctionKind::Interpreted { func } => Some(FuncCallInfo::Interpreted {
                        func: func.clone(),
                        closure: f.closure,
                        is_arrow: func.is_arrow,
                    }),
                    crate::value::FunctionKind::Bound { target, this_val, bound_args } => Some(FuncCallInfo::Bound {
                        target: *target, this_val: this_val.clone(), bound_args: bound_args.clone(),
                    }),
                }
            } else { None }
        });
        match kind_info {
            Some(FuncCallInfo::Native(f)) => f(self, args, this),
            Some(FuncCallInfo::Interpreted { func, closure, is_arrow }) => {
                let call_env = env::new_env(&self.heap, Some(closure), true);
                // args are stored into locals[0..n] by execute_chunk_func
                // make the function visible to itself by its name (for recursion)
                if let Some(name) = &func.name {
                    env::declare(&self.heap, call_env, name, Value::Object(idx), crate::value::BindingKind::Const);
                }
                let arr = HeapObj::Array(crate::value::ArrayData {
                    items: RefCell::new(args.to_vec()),
                    props: RefCell::new(HashMap::new()),
                    proto: RefCell::new(Some(self.array_proto.clone())),
                });
                env::declare(&self.heap, call_env, "arguments", Value::Object(GcIdx(self.heap.allocate(arr))), crate::value::BindingKind::Const);
                let this_val = this.unwrap_or(Value::Undefined);
                env::declare(&self.heap, call_env, "this", this_val.clone(), crate::value::BindingKind::Const);
                let _ = is_arrow;
                // execute the compiled function chunk
                self.execute_chunk_func(func.clone(), call_env, this_val, args)
            }
            Some(FuncCallInfo::Bound { target, this_val, bound_args }) => {
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
                f.prototype.borrow().clone().unwrap_or(self.object_proto.clone())
            } else { self.object_proto.clone() }
        });
        let new_obj = HeapObj::Object(crate::value::ObjectData {
            props: RefCell::new(HashMap::new()),
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
}

enum FuncCallInfo {
    Native(NativeFn),
    Interpreted { func: std::rc::Rc<crate::function::FunctionDef>, closure: GcIdx, is_arrow: bool },
    Bound { target: GcIdx, this_val: Value, bound_args: Vec<Value> },
}
