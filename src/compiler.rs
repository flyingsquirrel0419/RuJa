//! AST → bytecode compiler.

use crate::ast::*;
use crate::bytecode::{Chunk, Op};
use crate::error;
use crate::value::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Compiler {
    chunk: Chunk,
    scopes: Vec<Scope>,
    /// Function table: compiled nested functions.
    funcs: Vec<Rc<crate::function::FunctionDef>>,
    /// String constant pool for names.
    names: Vec<String>,
    name_map: HashMap<String, usize>,
}

struct Scope {
    /// name -> (slot, kind)
    bindings: HashMap<String, (usize, VarKind)>,
    is_function: bool,
    /// Starting offset; locals in this scope are numbered from `base` upward.
    base: usize,
}



impl Compiler {
    pub fn new() -> Self {
        Compiler {
            chunk: Chunk::new(),
            scopes: vec![Scope { bindings: HashMap::new(), is_function: true, base: 0 }],
            funcs: Vec::new(),
            names: Vec::new(),
            name_map: HashMap::new(),
        }
    }

    fn intern(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.name_map.get(name) {
            return idx;
        }
        let idx = self.names.len();
        self.names.push(name.to_string());
        self.name_map.insert(name.to_string(), idx);
        idx
    }

    pub fn compile_program(&mut self, program: &Program) -> error::Result<(Chunk, Vec<Rc<crate::function::FunctionDef>>)> {
        let n = program.body.len();
        for (i, stmt) in program.body.iter().enumerate() {
            if i + 1 == n {
                // last statement: if it's an expression, keep its value as the result
                if let Stmt::ExprStmt(e) = stmt {
                    self.compile_expr(e)?;
                } else {
                    self.compile_stmt(stmt)?;
                }
            } else {
                self.compile_stmt(stmt)?;
            }
        }
        self.chunk.emit(Op::Halt, 0);
        let chunk = std::mem::take(&mut self.chunk);
        let funcs = std::mem::take(&mut self.funcs);
        Ok((chunk, funcs))
    }

    fn push_scope(&mut self, is_function: bool) {
        let base = self.scopes.last().map(|s| s.base + s.bindings.len()).unwrap_or(0);
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            is_function,
            base,
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, kind: VarKind) {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.bindings.contains_key(name) {
                return;
            }
            let slot = scope.base + scope.bindings.len();
            scope.bindings.insert(name.to_string(), (slot, kind));
        }
    }

    fn resolve(&self, name: &str) -> Option<(usize, VarKind)> {
        // At top level, all names resolve via LoadGlobal (declared with StoreGlobal).
        if self.scopes.len() <= 1 {
            return None;
        }
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            // Skip the global scope (index 0); its bindings are accessed via LoadGlobal.
            if self.scopes.len() > 1 && i == 0 {
                continue;
            }
            if let Some(&(slot, ref kind)) = scope.bindings.get(name) {
                return Some((slot, *kind));
            }
        }
        None
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> error::Result<()> {
        match stmt {
            Stmt::Empty => {}
            Stmt::ExprStmt(e) => {
                self.compile_expr(e)?;
                self.chunk.emit(Op::Pop, 0);
            }
            Stmt::VarDecl { kind, decls } => {
                for (name, init) in decls {
                    if let Some(e) = init {
                        self.compile_expr(e)?;
                    } else {
                        self.chunk.emit(Op::Undefined, 0);
                    }
                    // store: at top level go to global, otherwise local slot
                    if self.scopes.len() == 1 {
                        // top-level: declare in global env, don't use local slots
                        self.declare(name, *kind);
                        let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                        self.chunk.emit(Op::Const(name_idx), 0);
                        self.chunk.emit(Op::StoreGlobal, 0);
                    } else if let Some((slot, _)) = self.resolve(name) {
                        self.declare(name, *kind);
                        self.chunk.emit(Op::StoreLocal(slot), 0);
                    }
                }
            }
            Stmt::Return(e) => {
                if let Some(e) = e {
                    self.compile_expr(e)?;
                } else {
                    self.chunk.emit(Op::Undefined, 0);
                }
                self.chunk.emit(Op::Return, 0);
            }
            Stmt::Block(body) => {
                self.push_scope(false);
                for s in body {
                    self.compile_stmt(s)?;
                }
                self.pop_scope();
            }
            Stmt::If { cond, then, else_ } => {
                self.compile_expr(cond)?;
                let jump_false = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), 0);
                self.compile_stmt(then)?;
                if let Some(el) = else_ {
                    let jump_end = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), 0);
                    let else_start = self.chunk.code.len();
                    self.chunk.patch_jump(jump_false, else_start);
                    self.compile_stmt(el)?;
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jump_end, end);
                } else {
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jump_false, end);
                }
            }
            Stmt::While { cond, body } => {
                let loop_start = self.chunk.code.len();
                self.compile_expr(cond)?;
                let jump_false = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), 0);
                self.compile_stmt(body)?;
                self.chunk.emit(Op::Jump(loop_start), 0);
                let end = self.chunk.code.len();
                self.chunk.patch_jump(jump_false, end);
            }
            Stmt::DoWhile { body, cond } => {
                let loop_start = self.chunk.code.len();
                self.compile_stmt(body)?;
                self.compile_expr(cond)?;
                self.chunk.emit(Op::JumpIfTrue(loop_start), 0);
            }
            Stmt::For { init, cond, update, body } => {
                self.push_scope(false);
                if let Some(init_stmt) = init {
                    self.compile_stmt(init_stmt)?;
                }
                let loop_start = self.chunk.code.len();
                let jump_false = if let Some(c) = cond {
                    self.compile_expr(c)?;
                    let jf = self.chunk.code.len();
                    self.chunk.emit(Op::JumpIfFalse(0), 0);
                    Some(jf)
                } else { None };
                self.compile_stmt(body)?;
                if let Some(u) = update {
                    self.compile_expr(u)?;
                    self.chunk.emit(Op::Pop, 0);
                }
                self.chunk.emit(Op::Jump(loop_start), 0);
                if let Some(jf) = jump_false {
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jf, end);
                }
                self.pop_scope();
            }
            Stmt::Throw(e) => {
                self.compile_expr(e)?;
                self.chunk.emit(Op::Throw, 0);
            }
            Stmt::TryCatch { try_body, catch_param, catch_body, finally_body } => {
                let try_start = self.chunk.code.len();
                self.chunk.emit(Op::PushTry(0), 0); // placeholder
                self.compile_stmt(try_body)?;
                self.chunk.emit(Op::PopTry, 0);
                let jump_end = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), 0);
                let catch_start = self.chunk.code.len();
                self.chunk.patch_jump(try_start + 0, catch_start); // patch PushTry handler
                // patch the PushTry's handler ip
                {
                    let try_ip = try_start;
                    if let Op::PushTry(ref mut h) = self.chunk.code[try_ip] {
                        *h = catch_start;
                    }
                }
                self.push_scope(true);
                if let Some(param) = catch_param {
                    self.declare(param, VarKind::Let);
                    if let Some((slot, _)) = self.resolve(param) {
                        self.chunk.emit(Op::StoreLocal(slot), 0);
                    }
                }
                self.compile_stmt(catch_body)?;
                self.pop_scope();
                if let Some(fin) = finally_body {
                    self.compile_stmt(fin)?;
                }
                let end = self.chunk.code.len();
                self.chunk.patch_jump(jump_end, end);
            }
            Stmt::FunctionDecl(f) => {
                // compile function body into a separate chunk
                let func_chunk = self.compile_function(f)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    chunk: Rc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                };
                self.funcs.push(Rc::new(fdef));
                self.chunk.emit(Op::MakeClosure(func_idx), 0);
                if let Some(name) = &f.name {
                    if let Some((slot, _)) = self.resolve(name) {
                        self.chunk.emit(Op::StoreLocal(slot), 0);
                    } else {
                        // store as global so recursive calls can find it
                        let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                        self.chunk.emit(Op::Const(name_idx), 0);
                        self.chunk.emit(Op::StoreGlobal, 0);
                    }
                }
            }
            Stmt::Break(_) => {
                // simplified: jump to end (proper impl needs a jump stack)
                self.chunk.emit(Op::Pop, 0); // placeholder
            }
            Stmt::Continue(_) => {
                self.chunk.emit(Op::Pop, 0); // placeholder
            }
            Stmt::Switch { disc, cases } => {
                self.compile_expr(disc)?;
                let mut end_jumps = Vec::new();
                let mut default_jump = None;
                for (i, case) in cases.iter().enumerate() {
                    if let Some(test) = &case.test {
                        self.chunk.emit(Op::Dup, 0);
                        self.compile_expr(test)?;
                        self.chunk.emit(Op::StrictEq, 0);
                        let jf = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfFalse(0), 0);
                        // matched: pop disc
                        self.chunk.emit(Op::Pop, 0);
                        for s in &case.body {
                            self.compile_stmt(s)?;
                        }
                        let jend = self.chunk.code.len();
                        self.chunk.emit(Op::Jump(0), 0);
                        end_jumps.push(jend);
                        self.chunk.patch_jump(jf, self.chunk.code.len());
                    } else {
                        default_jump = Some((i, self.chunk.code.len()));
                    }
                }
                // if no case matched, jump to default
                self.chunk.emit(Op::Pop, 0); // discard disc
                if let Some((_, pos)) = default_jump {
                    self.chunk.patch_jump(pos, self.chunk.code.len());
                }
                let end = self.chunk.code.len();
                for j in end_jumps {
                    self.chunk.patch_jump(j, end);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_function(&mut self, f: &FunctionExpr) -> error::Result<Chunk> {
        let saved_chunk = std::mem::take(&mut self.chunk);
        self.scopes.push(Scope { bindings: HashMap::new(), is_function: true, base: 0 });
        for (i, param) in f.params.iter().enumerate() {
            self.declare(param, VarKind::Let);
            // param is in slot i (VM stores args to locals[0..n])
        }
        for stmt in &f.body {
            self.compile_stmt(stmt)?;
        }
        self.chunk.emit(Op::ReturnUndefined, 0);
        self.pop_scope();
        let func_chunk = std::mem::take(&mut self.chunk);
        self.chunk = saved_chunk;
        Ok(func_chunk)
    }

    fn compile_expr(&mut self, expr: &Expr) -> error::Result<()> {
        match expr {
            Expr::Number(n) => {
                let idx = self.chunk.add_constant(Value::Number(*n));
                self.chunk.emit(Op::Const(idx), 0);
            }
            Expr::String(s) => {
                let idx = self.chunk.add_constant(Value::String(s.clone()));
                self.chunk.emit(Op::Const(idx), 0);
            }
            Expr::Bool(b) => {
                self.chunk.emit(if *b { Op::True } else { Op::False }, 0);
            }
            Expr::Null => self.chunk.emit(Op::Null, 0),
            Expr::Undefined => self.chunk.emit(Op::Undefined, 0),
            Expr::Ident(name) => {
                if let Some((slot, _)) = self.resolve(name) {
                    self.chunk.emit(Op::LoadLocal(slot), 0);
                } else {
                    // global variable: push name string constant, then LoadGlobal
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(name.as_ref())));
                    self.chunk.emit(Op::Const(name_idx), 0);
                    self.chunk.emit(Op::LoadGlobal, 0);
                }
            }
            Expr::This => {
                let name_idx = self.intern("this");
                self.chunk.emit(Op::LoadEnv(name_idx), 0);
            }
            Expr::Update(op, prefix, target) => {
                // load current value
                self.compile_expr(target)?;
                self.chunk.emit(Op::Dup, 0); // keep a copy for storing
                let delta = match op { UpdateOp::Inc => 1.0, UpdateOp::Dec => -1.0 };
                let c = self.chunk.add_constant(Value::Number(delta));
                self.chunk.emit(Op::Const(c), 0);
                self.chunk.emit(Op::Add, 0); // new value on stack
                // store new value back
                self.chunk.emit(Op::Dup, 0); // duplicate for result
                self.compile_assign_target(target)?;
                // stack now has the result; if postfix, we need old value
                // simplified: return new value (prefix semantics) for now
                let _ = prefix;
            }
            Expr::Binary(op, l, r) => {
                self.compile_expr(l)?;
                self.compile_expr(r)?;
                self.chunk.emit(self.bin_op(op), 0);
            }
            Expr::Unary(op, e) => {
                match op {
                    UnOp::Neg => { self.compile_expr(e)?; self.chunk.emit(Op::Neg, 0); }
                    UnOp::Not => { self.compile_expr(e)?; self.chunk.emit(Op::Not, 0); }
                    UnOp::BitNot => { self.compile_expr(e)?; self.chunk.emit(Op::BitNot, 0); }
                    UnOp::Typeof => { self.compile_expr(e)?; self.chunk.emit(Op::TypeOf, 0); }
                    _ => { self.compile_expr(e)?; }
                }
            }
            Expr::Logical(op, l, r) => {
                self.compile_expr(l)?;
                match op {
                    LogicalOp::And => {
                        let jf = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfFalse(0), 0);
                        self.chunk.emit(Op::Pop, 0);
                        self.compile_expr(r)?;
                        self.chunk.patch_jump(jf, self.chunk.code.len());
                    }
                    LogicalOp::Or => {
                        let jt = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfTrue(0), 0);
                        self.chunk.emit(Op::Pop, 0);
                        self.compile_expr(r)?;
                        self.chunk.patch_jump(jt, self.chunk.code.len());
                    }
                    LogicalOp::Nullish => {
                        let jf = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfFalse(0), 0); // simplified
                        self.chunk.emit(Op::Pop, 0);
                        self.compile_expr(r)?;
                        self.chunk.patch_jump(jf, self.chunk.code.len());
                    }
                }
            }
            Expr::Assign(op, target, value) => {
                if matches!(op, AssignOp::Assign) {
                    self.compile_expr(value)?;
                    self.chunk.emit(Op::Dup, 0);
                    self.compile_assign_target(target)?;
                } else {
                    // compound: load, op, store
                    self.compile_expr(target)?;
                    self.compile_expr(value)?;
                    let bin = self.assign_bin_op(op);
                    self.chunk.emit(bin, 0);
                    self.chunk.emit(Op::Dup, 0);
                    self.compile_assign_target(target)?;
                }
            }
            Expr::Conditional(c, t, f) => {
                self.compile_expr(c)?;
                let jf = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), 0);
                self.compile_expr(t)?;
                let jend = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), 0);
                self.chunk.patch_jump(jf, self.chunk.code.len());
                self.compile_expr(f)?;
                self.chunk.patch_jump(jend, self.chunk.code.len());
            }
            Expr::Object(props) => {
                self.chunk.emit(Op::NewObject, 0);
                for p in props {
                    self.chunk.emit(Op::Dup, 0);
                    // key
                    let key = match &p.key {
                        PropertyKey::Ident(s) => s.to_string(),
                        PropertyKey::String(s) => s.to_string(),
                        PropertyKey::Number(n) => crate::value::num_to_string(*n),
                        PropertyKey::Computed(_) => String::new(),
                    };
                    let key_idx = self.chunk.add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), 0);
                    self.compile_expr(&p.value)?;
                    self.chunk.emit(Op::SetProp, 0);
                    // SetProp pops obj+key+value, pushes undefined; we dup'd obj so it's still under the undefined
                    self.chunk.emit(Op::Pop, 0); // remove the undefined
                }
            }
            Expr::Array(elements) => {
                for e in elements {
                    if let Expr::Spread(_) = e {
                        // simplified: skip spread
                    } else {
                        self.compile_expr(e)?;
                    }
                }
                self.chunk.emit(Op::NewArray(elements.len()), 0);
            }
            Expr::Call { callee, args } => {
                // check if method call
                match callee.as_ref() {
                    Expr::Member { object, property, computed } => {
                        self.compile_expr(object)?;
                        let key = if !*computed {
                            if let Expr::String(s) = property.as_ref() { s.to_string() } else { String::new() }
                        } else { String::new() };
                        let key_idx = self.chunk.add_constant(Value::String(Rc::from(key.as_str())));
                        self.chunk.emit(Op::Const(key_idx), 0);
                        // push args
                        for a in args {
                            if let Expr::Spread(_) = a {} else { self.compile_expr(a)?; }
                        }
                        self.chunk.emit(Op::CallMethod(args.len()), 0);
                    }
                    _ => {
                        self.compile_expr(callee)?;
                        for a in args {
                            if let Expr::Spread(_) = a {} else { self.compile_expr(a)?; }
                        }
                        self.chunk.emit(Op::Call(args.len()), 0);
                    }
                }
            }
            Expr::New { callee, args } => {
                self.compile_expr(callee)?;
                for a in args {
                    if let Expr::Spread(_) = a {} else { self.compile_expr(a)?; }
                }
                self.chunk.emit(Op::New(args.len()), 0);
            }
            Expr::Member { object, property, computed } => {
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    self.chunk.emit(Op::GetElem, 0);
                } else {
                    let key = if let Expr::String(s) = property.as_ref() { s.to_string() } else { String::new() };
                    let key_idx = self.chunk.add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), 0);
                    self.chunk.emit(Op::GetProp, 0);
                }
            }
            Expr::Function(f) | Expr::Arrow(f) => {
                let func_chunk = self.compile_function(f)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    chunk: Rc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                };
                self.funcs.push(Rc::new(fdef));
                self.chunk.emit(Op::MakeClosure(func_idx), 0);
            }
            Expr::Sequence(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.compile_expr(e)?;
                    if i + 1 < exprs.len() {
                        self.chunk.emit(Op::Pop, 0);
                    }
                }
            }
            _ => {
                self.chunk.emit(Op::Undefined, 0);
            }
        }
        Ok(())
    }

    fn compile_assign_target(&mut self, target: &Expr) -> error::Result<()> {
        match target {
            Expr::Ident(name) => {
                if let Some((slot, _)) = self.resolve(name) {
                    self.chunk.emit(Op::StoreLocal(slot), 0);
                } else {
                    // global variable assignment
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk.emit(Op::Const(name_idx), 0);
                    self.chunk.emit(Op::StoreGlobal, 0);
                }
            }
            Expr::Member { object, property, computed } => {
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    self.chunk.emit(Op::SetElem, 0);
                } else {
                    let key = if let Expr::String(s) = property.as_ref() { s.to_string() } else { String::new() };
                    let key_idx = self.chunk.add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), 0);
                    self.chunk.emit(Op::SetProp, 0);
                }
            }
            _ => {
                self.chunk.emit(Op::Pop, 0);
            }
        }
        Ok(())
    }

    fn bin_op(&self, op: &BinOp) -> Op {
        match op {
            BinOp::Add => Op::Add,
            BinOp::Sub => Op::Sub,
            BinOp::Mul => Op::Mul,
            BinOp::Div => Op::Div,
            BinOp::Mod => Op::Mod,
            BinOp::Pow => Op::Pow,
            BinOp::Eq => Op::Eq,
            BinOp::NotEq => Op::NotEq,
            BinOp::StrictEq => Op::StrictEq,
            BinOp::StrictNotEq => Op::StrictNotEq,
            BinOp::Lt => Op::Lt,
            BinOp::Gt => Op::Gt,
            BinOp::Lte => Op::Lte,
            BinOp::Gte => Op::Gte,
            BinOp::BitAnd => Op::BitAnd,
            BinOp::BitOr => Op::BitOr,
            BinOp::BitXor => Op::BitXor,
            BinOp::Shl => Op::Shl,
            BinOp::Shr => Op::Shr,
            BinOp::Ushr => Op::Ushr,
            _ => Op::Pop,
        }
    }

    fn assign_bin_op(&self, op: &AssignOp) -> Op {
        match op {
            AssignOp::AddAssign => Op::Add,
            AssignOp::SubAssign => Op::Sub,
            AssignOp::MulAssign => Op::Mul,
            AssignOp::DivAssign => Op::Div,
            AssignOp::ModAssign => Op::Mod,
            AssignOp::PowAssign => Op::Pow,
            AssignOp::BitAndAssign => Op::BitAnd,
            AssignOp::BitOrAssign => Op::BitOr,
            AssignOp::BitXorAssign => Op::BitXor,
            AssignOp::ShlAssign => Op::Shl,
            AssignOp::ShrAssign => Op::Shr,
            AssignOp::UshrAssign => Op::Ushr,
            _ => Op::Add,
        }
    }
}
