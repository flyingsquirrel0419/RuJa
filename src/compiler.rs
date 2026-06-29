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
    /// Active loops: (continue target ip, pending break jumps, pending continue jumps).
    /// `continue_target == usize::MAX` means "patch me later" (C-style for, where the
    /// continue target is the update block, known only after the body is compiled).
    /// (continue_target, pending break jumps, pending continue jumps, label)
    /// (continue_target, pending break jumps, pending continue jumps, label)
    loop_stack: Vec<LoopFrame>,
    /// A label waiting to be attached to the next begin_loop call.
    pending_label: Option<Rc<str>>,
    /// Current source line being compiled; emitted onto each `Op` so runtime
    /// errors can report `(at line N)`. Updated as `compile_stmt` enters a stmt.
    current_line: usize,
}

struct Scope {
    /// name -> (slot, kind)
    bindings: HashMap<String, (usize, VarKind)>,
    is_function: bool,
    /// Starting offset; locals in this scope are numbered from `base` upward.
    base: usize,
    /// True when this scope corresponds to a `with` environment record; used
    /// to emit `PopWithEnv` (rather than `PopScope`) when unwinding on
    /// break/continue.
    is_with: bool,
    /// Whether strict-mode rules apply in this scope (inherited from the
    /// enclosing strict context or set by a `"use strict"` directive).
    is_strict: bool,
}

/// A step in the access path used while compiling destructuring patterns.
#[derive(Clone)]
enum PathStep {
    Index(usize),
    Prop(Rc<str>),
    RestFrom(usize),
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// A loop-stack frame: (continue_target, pending break jumps,
/// pending continue jumps, optional label).
type LoopFrame = (usize, Vec<usize>, Vec<usize>, Option<Rc<str>>);

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            chunk: Chunk::new(),
            scopes: vec![Scope {
                bindings: HashMap::new(),
                is_function: true,
                base: 0,
                is_with: false,
                is_strict: false,
            }],
            funcs: Vec::new(),
            names: Vec::new(),
            name_map: HashMap::new(),
            loop_stack: Vec::new(),
            pending_label: None,
            current_line: 0,
        }
    }

    fn intern(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.name_map.get(name) {
            return idx;
        }
        let idx = self.chunk.add_constant(Value::String(Rc::from(name)));
        self.name_map.insert(name.to_string(), idx);
        idx
    }

    /// Whether the current scope is strict (inherited from the enclosing
    /// strict context or set by a `"use strict"` directive).
    fn is_strict(&self) -> bool {
        self.scopes.last().map(|s| s.is_strict).unwrap_or(false)
    }

    pub fn compile_program(
        &mut self,
        program: &Program,
    ) -> error::Result<(Chunk, Vec<Rc<crate::function::FunctionDef>>)> {
        // The top-level scope inherits the program's strictness (from a leading
        // "use strict" directive prologue).
        if let Some(top) = self.scopes.last_mut() {
            top.is_strict = program.is_strict;
        }
        let n = program.body.len();
        // Hoist function declarations: compile them first so they're available
        // before any statement in the body runs.
        for stmt in &program.body {
            if let StmtNode::FunctionDecl(f) = &stmt.node {
                self.compile_stmt(stmt)?;
                let _ = f;
            }
        }
        // Hoist `var` declarations as undefined at the top level.
        for stmt in &program.body {
            if let StmtNode::VarDecl {
                kind: VarKind::Var,
                decls,
            } = &stmt.node
            {
                for (name, _) in decls {
                    self.declare(name, VarKind::Var)?;
                    let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                    self.chunk.emit(Op::Const(name_idx), self.current_line);
                    self.chunk.emit(Op::StoreGlobal, self.current_line);
                }
            }
        }
        // Hoist lexical (`let`/`const`) declarations into the TDZ at the top
        // level, so accessing them before the declaration throws ReferenceError.
        {
            let lex = Self::collect_lexical_names(&program.body);
            self.emit_lexical_hoist(&lex)?;
        }
        for (i, stmt) in program.body.iter().enumerate() {
            // Function declarations were hoisted above; skip them in the body pass.
            if matches!(&stmt.node, StmtNode::FunctionDecl(_)) {
                continue;
            }
            if i + 1 == n {
                // last statement: if it's an expression, keep its value as the result
                if let StmtNode::ExprStmt(e) = &stmt.node {
                    self.current_line = stmt.line as usize;
                    self.compile_expr(e)?;
                } else {
                    self.compile_stmt(stmt)?;
                }
            } else {
                self.compile_stmt(stmt)?;
            }
        }
        self.chunk.emit(Op::Halt, self.current_line);
        let mut chunk = std::mem::take(&mut self.chunk);
        chunk.is_strict = program.is_strict;
        let funcs = std::mem::take(&mut self.funcs);
        Ok((chunk, funcs))
    }

    fn push_scope(&mut self, is_function: bool) {
        let base = self
            .scopes
            .last()
            .map(|s| s.base + s.bindings.len())
            .unwrap_or(0);
        let is_strict = self.scopes.last().map(|s| s.is_strict).unwrap_or(false);
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            is_function,
            base,
            is_with: false,
            is_strict,
        });
    }

    /// Push a scope flagged as a `with` environment record.
    fn push_with_scope(&mut self) {
        let base = self
            .scopes
            .last()
            .map(|s| s.base + s.bindings.len())
            .unwrap_or(0);
        let is_strict = self.scopes.last().map(|s| s.is_strict).unwrap_or(false);
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            is_function: false,
            base,
            is_with: true,
            is_strict,
        });
    }

    /// Emit PopScope/PopWithEnv ops to unwind scopes opened since `loop_depth`,
    /// so `break`/`continue` don't leak `with` or block scopes past the loop.
    fn emit_scope_unwind(&mut self, loop_depth: usize) {
        for i in (loop_depth..self.scopes.len()).rev() {
            if self.scopes[i].is_with {
                self.chunk.emit(Op::PopWithEnv, self.current_line);
            } else {
                self.chunk.emit(Op::PopScope, self.current_line);
            }
        }
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Begin a loop: `continue_target` is where `continue` jumps (loop start/cond).
    fn begin_loop(&mut self, continue_target: usize) {
        // Attach a pending label (set by a wrapping Labeled statement) so
        // `break label` / `continue label` can target this loop.
        let label = self.pending_label.take();
        self.loop_stack
            .push((continue_target, Vec::new(), Vec::new(), label));
    }

    /// Like `begin_loop` but tags the loop with a label so `break label` /
    /// `continue label` can target it.
    fn begin_labeled_loop(&mut self, continue_target: usize, label: Rc<str>) {
        self.loop_stack
            .push((continue_target, Vec::new(), Vec::new(), Some(label)));
    }

    /// Patch the current loop's continue target (used when the continue target is
    /// only known after the body, e.g. the update block of a C-style for).
    fn set_continue_target(&mut self, target: usize) {
        if let Some((cont, _, cont_jumps, _)) = self.loop_stack.last_mut() {
            *cont = target;
            // patch already-emitted continue jumps to the real target
            for j in cont_jumps.drain(..) {
                self.chunk.patch_jump(j, target);
            }
        }
    }

    /// End a loop: patch all pending `break` jumps to `end`.
    fn end_loop(&mut self, end: usize) {
        if let Some((cont, breaks, _, _)) = self.loop_stack.pop() {
            // any un-patched continue jumps fall back to the loop start/cond.
            let _ = cont;
            for j in breaks {
                self.chunk.patch_jump(j, end);
            }
        }
    }

    fn declare(&mut self, name: &str, kind: VarKind) -> error::Result<()> {
        if let Some(scope) = self.scopes.last_mut() {
            if let Some((_, existing_kind)) = scope.bindings.get(name) {
                // `var` may redeclare `var` (spec allows it). Any other
                // redeclaration in the same lexical scope is a SyntaxError.
                let ok = *existing_kind == VarKind::Var && kind == VarKind::Var;
                if !ok {
                    return Err(error::Error::syntax(format!(
                        "Identifier '{}' has already been declared",
                        name
                    )));
                }
                // var-on-var: keep the existing slot/kind.
                return Ok(());
            }
            let slot = scope.base + scope.bindings.len();
            scope.bindings.insert(name.to_string(), (slot, kind));
        }
        Ok(())
    }

    /// Declare a function parameter. In non-strict mode duplicate parameter
    /// names are permitted (the last binding wins); in strict mode they are a
    /// SyntaxError (checked separately in `compile_function`).
    fn declare_param(&mut self, name: &str, is_strict: bool) -> error::Result<()> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.bindings.contains_key(name) {
                if is_strict {
                    return Err(error::Error::syntax(format!(
                        "Duplicate parameter '{}' is not allowed in strict mode",
                        name
                    )));
                }
                // Non-strict: keep the existing slot; the later parameter's
                // value overwrites it at runtime.
                return Ok(());
            }
            let slot = scope.base + scope.bindings.len();
            scope
                .bindings
                .insert(name.to_string(), (slot, VarKind::Let));
        }
        Ok(())
    }

    /// Collect all binding names introduced by a destructuring pattern.
    fn pattern_names(pattern: &Pattern, out: &mut Vec<Rc<str>>) {
        match pattern {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Array(elems) => {
                for el in elems {
                    Self::pattern_names(el, out);
                }
            }
            Pattern::Object(props, rest) => {
                for (_, target) in props {
                    Self::pattern_names(target, out);
                }
                if let Some(r) = rest {
                    Self::pattern_names(r, out);
                }
            }
            Pattern::Assign(inner, _) => Self::pattern_names(inner, out),
            Pattern::Rest(inner) => Self::pattern_names(inner, out),
        }
    }

    /// Collect lexical (`let`/`const`) names declared at the top level of a
    /// statement list. Does NOT descend into nested blocks/functions/loops:
    /// those introduce their own scopes and hoist their own lexicals.
    fn collect_lexical_names(body: &[Stmt]) -> Vec<(Rc<str>, VarKind)> {
        let mut out = Vec::new();
        for stmt in body {
            match &stmt.node {
                StmtNode::VarDecl { kind, decls } => {
                    if *kind != VarKind::Var {
                        for (name, _) in decls {
                            out.push((name.clone(), *kind));
                        }
                    }
                }
                // `var` destructuring (rare) is function-scoped, not lexical.
                StmtNode::Destructure { kind, pattern, .. } if *kind != VarKind::Var => {
                    let mut names = Vec::new();
                    Self::pattern_names(pattern, &mut names);
                    for n in names {
                        out.push((n, *kind));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Collect top-level `var` and function-declaration names from a statement
    /// list (for direct-eval leak into the caller's function scope).
    pub fn collect_var_names(body: &[Stmt]) -> Vec<Rc<str>> {
        let mut out = Vec::new();
        for stmt in body {
            match &stmt.node {
                StmtNode::VarDecl { kind, decls } if *kind == VarKind::Var => {
                    for (name, _) in decls {
                        out.push(name.clone());
                    }
                }
                StmtNode::FunctionDecl(f) => {
                    if let Some(name) = &f.name {
                        out.push(name.clone());
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Emit TDZ (uninitialized) declarations for lexical bindings at scope entry.
    /// Also registers them in the compiler's scope table so `resolve` works and
    /// later `declare` calls for the same name are no-ops (preventing slot reuse).
    fn emit_lexical_hoist(&mut self, names: &[(Rc<str>, VarKind)]) -> error::Result<()> {
        for (name, kind) in names {
            self.declare(name, *kind)?;
            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
            match kind {
                VarKind::Const => self
                    .chunk
                    .emit(Op::DeclareConstUninit(name_idx), self.current_line),
                _ => self
                    .chunk
                    .emit(Op::DeclareLetUninit(name_idx), self.current_line),
            }
        }
        Ok(())
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
        // Track the statement's source line so every Op emitted while
        // compiling it carries the line for runtime error reporting.
        self.current_line = stmt.line as usize;
        match &stmt.node {
            StmtNode::Empty => {}
            StmtNode::ExprStmt(e) => {
                self.compile_expr(e)?;
                self.chunk.emit(Op::Pop, self.current_line);
            }
            StmtNode::VarDecl { kind, decls } => {
                for (name, init) in decls {
                    if let Some(e) = init {
                        self.compile_expr(e)?;
                    } else {
                        self.chunk.emit(Op::Undefined, self.current_line);
                    }
                    if *kind == VarKind::Var {
                        // `var` is function-scoped: declare at the function-scope root
                        // (or global at top level), regardless of block nesting.
                        self.declare(name, *kind)?;
                        let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                        if self.scopes.len() == 1 {
                            // top-level var: store as global
                            self.chunk.emit(Op::Const(name_idx), self.current_line);
                            self.chunk.emit(Op::StoreGlobal, self.current_line);
                        } else {
                            // var was hoisted to function-scope root; just set the value.
                            self.chunk.emit(Op::DeclareVar(name_idx), self.current_line);
                        }
                    } else {
                        // Lexical (let/const): already declared uninitialized at scope
                        // entry by `emit_lexical_hoist`. Initialize the binding with the
                        // value now (this lifts the TDZ).
                        let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                        match kind {
                            VarKind::Const => {
                                self.chunk.emit(Op::InitConst(name_idx), self.current_line)
                            }
                            _ => self.chunk.emit(Op::InitLet(name_idx), self.current_line),
                        }
                    }
                }
            }
            StmtNode::Return(e) => {
                if let Some(e) = e {
                    self.compile_expr(e)?;
                } else {
                    self.chunk.emit(Op::Undefined, self.current_line);
                }
                self.chunk.emit(Op::Return, self.current_line);
            }
            StmtNode::Block(body) => {
                self.push_scope(false);
                self.chunk.emit(Op::PushScope, self.current_line);
                // Hoist function declarations within the block.
                for s in body {
                    if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                        self.compile_stmt(s)?;
                    }
                }
                // Hoist `var` declarations: declare them as undefined before the body runs.
                for s in body {
                    if let StmtNode::VarDecl {
                        kind: VarKind::Var,
                        decls,
                    } = &s.node
                    {
                        for (name, _) in decls {
                            self.declare(name, VarKind::Var)?;
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            if self.scopes.len() == 1 {
                                self.chunk.emit(Op::Const(name_idx), self.current_line);
                                self.chunk.emit(Op::StoreGlobal, self.current_line);
                            } else {
                                self.chunk.emit(Op::Undefined, self.current_line);
                                self.chunk.emit(Op::DeclareVar(name_idx), self.current_line);
                            }
                        }
                    }
                }
                // Hoist lexical (`let`/`const`) declarations into the TDZ at block
                // entry, so accessing them before the declaration throws ReferenceError.
                {
                    let lex = Self::collect_lexical_names(body);
                    self.emit_lexical_hoist(&lex)?;
                }
                for s in body {
                    if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                        continue;
                    }
                    self.compile_stmt(s)?;
                }
                self.chunk.emit(Op::PopScope, self.current_line);
                self.pop_scope();
            }
            StmtNode::If { cond, then, else_ } => {
                self.compile_expr(cond)?;
                let jump_false = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.compile_stmt(then)?;
                if let Some(el) = else_ {
                    let jump_end = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), self.current_line);
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
            StmtNode::While { cond, body } => {
                let loop_start = self.chunk.code.len();
                self.begin_loop(loop_start);
                self.compile_expr(cond)?;
                let jump_false = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.compile_stmt(body)?;
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                let end = self.chunk.code.len();
                self.chunk.patch_jump(jump_false, end);
                self.end_loop(end);
            }
            StmtNode::DoWhile { body, cond } => {
                let loop_start = self.chunk.code.len();
                // continue jumps to the condition test.
                let cond_ip_placeholder = loop_start;
                self.begin_loop(cond_ip_placeholder);
                self.compile_stmt(body)?;
                let cond_ip = self.chunk.code.len();
                self.set_continue_target(cond_ip);
                self.compile_expr(cond)?;
                self.chunk
                    .emit(Op::JumpIfTrue(loop_start), self.current_line);
                let end = self.chunk.code.len();
                self.end_loop(end);
            }
            StmtNode::For {
                init,
                cond,
                update,
                body,
            } => {
                self.push_scope(false);
                if let Some(init_stmt) = init {
                    self.compile_stmt(init_stmt)?;
                }
                let loop_start = self.chunk.code.len();
                // continue should re-run the update, then the condition: insert the
                // update block as the continue target after loop_start.
                let jump_false = if let Some(c) = cond {
                    self.compile_expr(c)?;
                    let jf = self.chunk.code.len();
                    self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                    Some(jf)
                } else {
                    None
                };
                // continue target is the update block (known after the body); mark unknown.
                self.begin_loop(usize::MAX);
                // Per-iteration environment for `for (let ...)`: clone the
                // lexical bindings into a fresh child env before each body so
                // closures created in the body capture a distinct binding per
                // iteration. (Spec: CreatePerIterationEnvironment; an
                // approximation: the update still mutates this iteration's
                // env, so closures see the post-update value for their
                // iteration -- but each iteration gets its own env.)
                let per_iteration_let = matches!(
                    init.as_ref().map(|s| &s.node),
                    Some(StmtNode::VarDecl {
                        kind: VarKind::Let,
                        ..
                    }) | Some(StmtNode::VarDecl {
                        kind: VarKind::Const,
                        ..
                    })
                );
                // Collect the loop's declared lexical names so only those are
                // rebound per iteration (outer lets are shared via the chain).
                let loop_names: Vec<Rc<str>> = match init.as_ref().map(|s| &s.node) {
                    Some(StmtNode::VarDecl {
                        kind: VarKind::Let | VarKind::Const,
                        decls,
                        ..
                    }) => decls.iter().map(|(n, _)| n.clone()).collect(),
                    _ => Vec::new(),
                };
                let loop_names_idx = if loop_names.is_empty() {
                    usize::MAX
                } else {
                    let idx = self.chunk.let_names.len();
                    self.chunk.let_names.push(loop_names);
                    idx
                };
                if per_iteration_let {
                    self.chunk
                        .emit(Op::CloneLetNames(loop_names_idx), self.current_line);
                }
                self.compile_stmt(body)?;
                // Restore the frame env to the loop-scope env (the CloneLetEnv
                // child's parent) so the update and next iteration run in the
                // original env and the chain does not grow per iteration.
                if per_iteration_let {
                    self.chunk.emit(Op::RestoreParentEnv, self.current_line);
                }
                let continue_target = self.chunk.code.len();
                if let Some(u) = update {
                    self.compile_expr(u)?;
                    self.chunk.emit(Op::Pop, self.current_line);
                }
                // if there's no update, continue jumps to the condition (loop_start).
                self.set_continue_target(continue_target);
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                if let Some(jf) = jump_false {
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jf, end);
                }
                self.end_loop(self.chunk.code.len());
                self.pop_scope();
            }
            StmtNode::ForOf {
                left,
                right,
                body,
                is_await,
            } => {
                // for (let x of iterable): iterate values. `for await` uses the
                // async iterator protocol (Symbol.asyncIterator) and awaits each
                // next() result.
                self.push_scope(false);
                self.compile_expr(right)?;
                if *is_await {
                    self.chunk.emit(Op::GetAsyncIterator, self.current_line);
                } else {
                    // GetIterator pops the iterable, pushes an iterator object.
                    self.chunk.emit(Op::GetIterator, self.current_line);
                }
                let it_name_idx = self.intern("#iter");
                self.chunk
                    .emit(Op::DeclareEnv(it_name_idx), self.current_line);
                let loop_start = self.chunk.code.len();
                self.begin_loop(loop_start);
                self.chunk.emit(Op::LoadEnv(it_name_idx), self.current_line);
                if *is_await {
                    self.chunk.emit(Op::IteratorNextAwait, self.current_line);
                } else {
                    // IteratorNext pops the iterator, pushes [value, done(bool)].
                    self.chunk.emit(Op::IteratorNext, self.current_line);
                }
                // JumpIfTrue pops `done`; when true (done==true), jump past the body.
                let done_jump = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfTrue(0), self.current_line);
                // Bind the value into the loop variable, then run the body.
                self.compile_for_var(left)?;
                self.compile_stmt(body)?;
                self.chunk.emit(Op::Pop, self.current_line); // discard body's expr result
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                let end = self.chunk.code.len();
                self.chunk.patch_jump(done_jump, end);
                self.end_loop(end);
                // When done, the stale value is still on the stack; drop it.
                self.chunk.emit(Op::Pop, self.current_line);
                self.pop_scope();
            }
            StmtNode::ForIn { left, right, body } => self.compile_for_in(left, right, body)?,
            StmtNode::With { object, body } => {
                if self.is_strict() {
                    return Err(error::Error::syntax(
                        "'with' statement is not allowed in strict mode".to_string(),
                    ));
                }
                self.push_with_scope();
                self.compile_expr(object)?;
                self.chunk.emit(Op::PushWithEnv, self.current_line);
                self.compile_stmt(body)?;
                self.chunk.emit(Op::PopWithEnv, self.current_line);
                self.pop_scope();
            }
            StmtNode::Throw(e) => {
                self.compile_expr(e)?;
                self.chunk.emit(Op::Throw, self.current_line);
            }
            StmtNode::TryCatch {
                try_body,
                catch_param,
                catch_body,
                finally_body,
            } => {
                let has_finally = finally_body.is_some();
                let has_catch = catch_body.is_some();
                // --- try body, guarded by the catch handler (and finally guard) ---
                let try_start = self.chunk.code.len();
                if has_catch {
                    self.chunk.emit(Op::PushTry(0), self.current_line); // catch handler placeholder
                }
                if has_finally {
                    // Push a finally guard whose target is patched to finally_start
                    // below; non-local transfers (return/break/continue/throw) inside
                    // try/catch divert to it with their completion recorded.
                    self.chunk.emit(Op::PushFinally(0), self.current_line);
                }
                let finally_guard_ip = if has_finally {
                    // finally guard is at try_start when there is no PushTry before it
                    if has_catch {
                        try_start + 1
                    } else {
                        try_start
                    }
                } else {
                    usize::MAX
                };
                self.compile_stmt(try_body)?;
                if has_catch {
                    self.chunk.emit(Op::PopTry, self.current_line);
                }
                if has_finally {
                    self.chunk.emit(Op::PopFinally, self.current_line); // drop finally guard
                }
                // Normal try completion -> jump to finally (or end).
                let jump_past_catch = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                // --- catch handler ---
                let catch_start = self.chunk.code.len();
                if has_catch {
                    if let Op::PushTry(ref mut h) = self.chunk.code[try_start] {
                        *h = catch_start;
                    }
                    self.push_scope(true);
                    if let Some(param) = catch_param {
                        self.declare(param, VarKind::Let)?;
                        let name_idx = self.intern(param);
                        self.chunk.emit(Op::DeclareEnv(name_idx), self.current_line);
                    }
                    self.compile_stmt(catch_body.as_ref().unwrap())?;
                    self.pop_scope();
                    if has_finally {
                        self.chunk.emit(Op::PopFinally, self.current_line); // drop finally guard
                    }
                    // Normal catch completion -> jump to finally (or end).
                    let jump_past_catch2 = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), self.current_line);
                    // --- finally entry ---
                    let finally_start = self.chunk.code.len();
                    if has_finally {
                        if let Op::PushFinally(ref mut t) = self.chunk.code[finally_guard_ip] {
                            *t = finally_start;
                        }
                    }
                    self.chunk.patch_jump(jump_past_catch, finally_start);
                    self.chunk.patch_jump(jump_past_catch2, finally_start);
                } else {
                    // No catch: patch the try-completion jump and finally guard
                    // straight to the finally entry.
                    let finally_start = self.chunk.code.len();
                    if has_finally {
                        if let Op::PushFinally(ref mut t) = self.chunk.code[finally_guard_ip] {
                            *t = finally_start;
                        }
                    }
                    self.chunk.patch_jump(jump_past_catch, finally_start);
                }
                if let Some(fin) = finally_body {
                    self.compile_stmt(fin)?;
                    // Re-raise the pending completion (return/break/continue/throw)
                    // that diverted here. A normal completion falls through.
                    self.chunk.emit(Op::PopFinallyRethrow, self.current_line);
                }
            }
            StmtNode::FunctionDecl(f) => {
                // compile function body into a separate chunk
                let (func_chunk, param_slots) = self.compile_function(f)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    param_slots,
                    rest_param: f.rest_param.clone(),
                    chunk: Rc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                };
                self.funcs.push(Rc::new(fdef));
                self.chunk
                    .emit(Op::MakeClosure(func_idx), self.current_line);
                if let Some(name) = &f.name {
                    if let Some((slot, _)) = self.resolve(name) {
                        self.chunk.emit(Op::StoreLocal(slot), self.current_line);
                    } else {
                        // store as global so recursive calls can find it
                        let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                        self.chunk.emit(Op::Const(name_idx), self.current_line);
                        self.chunk.emit(Op::StoreGlobal, self.current_line);
                    }
                }
            }
            StmtNode::Destructure {
                kind,
                pattern,
                init,
            } => {
                // Evaluate the source (if any), stash it in a temp env binding, then bind each
                // pattern element by indexing/property access on the temp. When `init` is None
                // (for-of/for-in) the value is already on the stack.
                if let Some(e) = init {
                    self.compile_expr(e)?;
                }
                let temp_idx = self.intern("#destr");
                self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                self.compile_pattern(pattern, temp_idx, &[], *kind)?;
            }
            StmtNode::Break(label) => {
                // Jump past the loop body; target patched when the loop ends.
                // With a label, target the matching labeled loop (searching
                // outward); otherwise the innermost loop.
                let target = if let Some(l) = label {
                    self.loop_stack
                        .iter()
                        .rposition(|(_, _, _, lbl)| lbl.as_ref().is_some_and(|x| x == l))
                } else {
                    if self.loop_stack.is_empty() {
                        None
                    } else {
                        Some(self.loop_stack.len() - 1)
                    }
                };
                if let Some(i) = target {
                    let (_, breaks, _, _) = &mut self.loop_stack[i];
                    let j = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), self.current_line);
                    breaks.push(j);
                }
            }
            StmtNode::Continue(label) => {
                // Jump back to the loop condition/next-iteration target.
                let target = if let Some(l) = label {
                    self.loop_stack
                        .iter()
                        .rposition(|(_, _, _, lbl)| lbl.as_ref().is_some_and(|x| x == l))
                } else {
                    if self.loop_stack.is_empty() {
                        None
                    } else {
                        Some(self.loop_stack.len() - 1)
                    }
                };
                if let Some(i) = target {
                    let (cont, _, cont_jumps, _) = &mut self.loop_stack[i];
                    if *cont != usize::MAX {
                        self.chunk.emit(Op::Jump(*cont), self.current_line);
                    } else {
                        // Target unknown yet (C-style for); record and patch later.
                        let j = self.chunk.code.len();
                        self.chunk.emit(Op::Jump(0), self.current_line);
                        cont_jumps.push(j);
                    }
                }
            }
            StmtNode::Labeled(label, body) => {
                // A labeled statement: compile the body with this label on the
                // loop stack so `break label` / `continue label` can target it.
                // For non-loop bodies, only `break label` is meaningful; we push
                // a synthetic loop frame whose continue target is unreachable.
                if matches!(
                    &body.node,
                    StmtNode::While { .. } | StmtNode::DoWhile { .. } | StmtNode::For { .. }
                ) {
                    // Hand the label to the inner loop's begin_loop by stashing
                    // it on a pending-label field that begin_loop consumes.
                    self.pending_label = Some(label.clone());
                    self.compile_stmt(body)?;
                } else {
                    // Non-loop labeled statement: push a frame that only honors
                    // `break label`. continue is invalid here; mark as MAX.
                    self.loop_stack
                        .push((usize::MAX, Vec::new(), Vec::new(), Some(label.clone())));
                    let result = self.compile_stmt(body);
                    // break jumps patch to here (after the body).
                    if let Some((_, breaks, _, _)) = self.loop_stack.pop() {
                        let end = self.chunk.code.len();
                        for j in breaks {
                            self.chunk.patch_jump(j, end);
                        }
                    }
                    result?;
                }
            }
            StmtNode::Switch { disc, cases } => {
                // Evaluate the discriminant once into a temp env binding, so tests can
                // re-load it without stack gymnastics. Supports fall-through and break.
                self.compile_expr(disc)?;
                let sw_idx = self.intern("#switch");
                self.chunk.emit(Op::DeclareEnv(sw_idx), self.current_line);
                self.begin_loop(usize::MAX);
                // Tests: for each case, load disc, compare, jump to body on match.
                let mut match_jumps: Vec<(usize, usize)> = Vec::new(); // (case_idx, jump_pos)
                let mut default_idx: Option<usize> = None;
                for (i, case) in cases.iter().enumerate() {
                    if let Some(test) = &case.test {
                        self.chunk.emit(Op::LoadEnv(sw_idx), self.current_line);
                        self.compile_expr(test)?;
                        self.chunk.emit(Op::StrictEq, self.current_line);
                        let j = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfTrue(0), self.current_line);
                        match_jumps.push((i, j));
                    } else {
                        default_idx = Some(i);
                    }
                }
                // No match: jump to default body (patched later) or end.
                let no_match = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                // Bodies compile sequentially; fall-through is automatic.
                let mut body_starts: Vec<Option<usize>> = vec![None; cases.len()];
                for (i, case) in cases.iter().enumerate() {
                    body_starts[i] = Some(self.chunk.code.len());
                    for s in &case.body {
                        self.compile_stmt(s)?;
                    }
                }
                let end = self.chunk.code.len();
                for (i, j) in &match_jumps {
                    if let Some(pos) = body_starts[*i] {
                        self.chunk.patch_jump(*j, pos);
                    }
                }
                if let Some(di) = default_idx {
                    if let Some(pos) = body_starts[di] {
                        self.chunk.patch_jump(no_match, pos);
                    }
                } else {
                    self.chunk.patch_jump(no_match, end);
                }
                self.end_loop(end);
            }
            _ => {}
        }
        Ok(())
    }

    /// Bind the value on top of the stack into the loop variable of a `for`/`for-in`/`for-of`.
    /// `left` is the statement produced by `parse_var_decl_no_semi` (a `VarDecl` with one name)
    /// or an expression (implicit assignment to an existing binding).
    fn compile_for_var(&mut self, left: &Stmt) -> error::Result<()> {
        match &left.node {
            StmtNode::VarDecl { kind, decls } => {
                // Single declarator: bind the on-stack value as a let/const in the loop scope.
                if let Some((name, _)) = decls.first() {
                    self.declare(name, *kind)?;
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk.emit(Op::DeclareEnv(name_idx), self.current_line);
                } else {
                    self.chunk.emit(Op::Pop, self.current_line);
                }
            }
            StmtNode::Destructure { kind, pattern, .. } => {
                // for-of/for-in with a destructuring pattern: the value is on the stack.
                let temp_idx = self.intern("#destr");
                self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                self.compile_pattern(pattern, temp_idx, &[], *kind)?;
            }
            _other => {
                // Non-declaration left side (e.g. `for (x of ...)`): treat as assignment target.
                // `other` is a &StmtNode; the full statement is `left`.
                self.compile_stmt(left)?;
            }
        }
        Ok(())
    }

    /// Compile `for (left in right)`: iterate enumerable own+inherited string keys.
    fn compile_for_in(&mut self, left: &Stmt, right: &Expr, body: &Stmt) -> error::Result<()> {
        self.push_scope(false);
        self.compile_expr(right)?;
        // GetForInKeys pops the object and pushes an iterator over its string keys.
        self.chunk.emit(Op::GetForInKeys, self.current_line);
        let it_name_idx = self.intern("#iter");
        self.chunk
            .emit(Op::DeclareEnv(it_name_idx), self.current_line);
        let loop_start = self.chunk.code.len();
        self.begin_loop(loop_start);
        self.chunk.emit(Op::LoadEnv(it_name_idx), self.current_line);
        self.chunk.emit(Op::IteratorNext, self.current_line);
        let done_jump = self.chunk.code.len();
        self.chunk.emit(Op::JumpIfTrue(0), self.current_line);
        self.compile_for_var(left)?;
        self.compile_stmt(body)?;
        self.chunk.emit(Op::Pop, self.current_line);
        self.chunk.emit(Op::Jump(loop_start), self.current_line);
        let end = self.chunk.code.len();
        self.chunk.patch_jump(done_jump, end);
        self.end_loop(end);
        self.chunk.emit(Op::Pop, self.current_line);
        self.pop_scope();
        Ok(())
    }

    pub fn compile_function(&mut self, f: &FunctionExpr) -> error::Result<(Chunk, Vec<usize>)> {
        let saved_chunk = std::mem::take(&mut self.chunk);
        let saved_names = std::mem::take(&mut self.name_map);
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            is_function: true,
            base: 0,
            is_with: false,
            is_strict: f.is_strict,
        });

        // Declare each parameter as a lexical binding and remember its local
        // slot. The VM stores argument values into `locals[slot]` before the
        // frame runs, so defaults can read the raw argument via `LoadLocal`
        // (bypassing the environment TDZ) while the *binding* stays in the TDZ
        // until `InitLet` -- this is what makes `function f(a = b, b = 2)` a
        // ReferenceError while `function f(a, b = a)` still works.
        let mut param_slots: Vec<usize> = Vec::with_capacity(f.params.len());
        for param in f.params.iter() {
            self.declare_param(param, f.is_strict)?;
            let slot = self
                .scopes
                .last()
                .and_then(|sc| sc.bindings.get(&param.to_string()))
                .map(|(slot, _)| *slot)
                .unwrap_or(param_slots.len());
            param_slots.push(slot);
        }
        // Initialize every parameter binding left-to-right. In the VM all
        // parameter bindings are declared *uninitialized* (TDZ), so a default
        // expression that references a parameter to its right throws
        // ReferenceError -- matching the ES spec rule that parameter default
        // initializers run in a scope where only earlier parameters are
        // initialized. The raw argument lives in `locals[slot]`, read via
        // `LoadLocal` to bypass the environment TDZ during the undefined check.
        for (i, param) in f.params.iter().enumerate() {
            let name_idx = self.chunk.add_constant(Value::String(param.clone()));
            let slot = param_slots[i];
            if let Some(default) = f.param_defaults.get(i).and_then(|d| d.as_ref()) {
                self.chunk.emit(Op::LoadLocal(slot), self.current_line);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk.emit(Op::StrictEq, self.current_line);
                // stack: [param, isUndefined]; JumpIfFalse pops isUndefined.
                // If defined (isUndefined == false), jump to the init path that
                // initializes the binding with the raw argument.
                let defined_jump = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                // Undefined path: the binding is still in the TDZ. Re-declare it
                // as uninitialized (no-op if already uninit) for clarity, then
                // evaluate the default and initialize.
                self.chunk.emit(Op::Pop, self.current_line);
                self.chunk
                    .emit(Op::DeclareLetUninit(name_idx), self.current_line);
                self.compile_expr(default)?;
                self.chunk.emit(Op::InitLet(name_idx), self.current_line);
                // Jump over the defined-path init (stack is empty here).
                let over_init = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                // Defined path lands here with [param] on the stack. Initialize
                // the binding with the raw argument value (lifts the TDZ).
                let init_param = self.chunk.code.len();
                self.chunk.emit(Op::InitLet(name_idx), self.current_line);
                self.chunk.patch_jump(defined_jump, init_param);
                let after = self.chunk.code.len();
                self.chunk.patch_jump(over_init, after);
            } else {
                // No default: initialize the binding with the raw argument
                // (which may be `undefined` if fewer args were supplied). This
                // lifts the TDZ for this parameter so later defaults may read it.
                self.chunk.emit(Op::LoadLocal(slot), self.current_line);
                self.chunk.emit(Op::InitLet(name_idx), self.current_line);
            }
        }
        // Hoist `var` declarations within the function body as undefined.
        for stmt in &f.body {
            if let StmtNode::VarDecl {
                kind: VarKind::Var,
                decls,
            } = &stmt.node
            {
                for (name, _) in decls {
                    self.declare(name, VarKind::Var)?;
                    let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                    self.chunk.emit(Op::Undefined, self.current_line);
                    self.chunk.emit(Op::DeclareVar(name_idx), self.current_line);
                }
            }
        }
        // Hoist lexical (`let`/`const`) declarations into the TDZ at function
        // entry, so accessing them before the declaration throws ReferenceError.
        {
            let lex = Self::collect_lexical_names(&f.body);
            self.emit_lexical_hoist(&lex)?;
        }
        for stmt in &f.body {
            self.compile_stmt(stmt)?;
        }
        self.chunk.emit(Op::ReturnUndefined, self.current_line);
        self.pop_scope();
        let mut func_chunk = std::mem::take(&mut self.chunk);
        func_chunk.is_strict = f.is_strict;
        self.name_map = saved_names;
        self.chunk = saved_chunk;
        Ok((func_chunk, param_slots))
    }

    /// A path step to reach a destructured value from the source temp.
    fn load_path(&mut self, temp_idx: usize, path: &[PathStep]) {
        self.chunk.emit(Op::LoadEnv(temp_idx), self.current_line);
        for step in path {
            match step {
                PathStep::Index(i) => {
                    let k = self.chunk.add_constant(Value::Number(*i as f64));
                    self.chunk.emit(Op::Const(k), self.current_line);
                    self.chunk.emit(Op::GetElem, self.current_line);
                }
                PathStep::Prop(name) => {
                    let k = self.chunk.add_constant(Value::String(name.clone()));
                    self.chunk.emit(Op::Const(k), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line);
                }
                PathStep::RestFrom(_) => {} // handled by bind_rest
            }
        }
    }

    /// Compile a destructuring pattern against the source held in env var `temp_idx`,
    /// reaching nested values via `path`.
    fn compile_pattern(
        &mut self,
        pattern: &Pattern,
        temp_idx: usize,
        path: &[PathStep],
        kind: VarKind,
    ) -> error::Result<()> {
        match pattern {
            Pattern::Ident(name) => {
                self.load_path(temp_idx, path);
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                // Try to initialize an already-hoisted (TDZ) binding; if none exists
                // (e.g. a per-iteration loop binding in for-of), declare it fresh.
                match kind {
                    VarKind::Const => self
                        .chunk
                        .emit(Op::InitEnvConst(name_idx), self.current_line),
                    _ => self.chunk.emit(Op::InitEnv(name_idx), self.current_line),
                }
            }
            Pattern::Array(elems) => {
                // Array destructuring uses the iterator protocol: obtain an
                // iterator from the value at `path`, then pull one value per
                // element. This matches `[Symbol.iterator]`-based iterables
                // (generators, custom iterables, sets) as well as arrays.
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::GetIterator, self.current_line);
                let iter_idx = self.intern("#arr-iter");
                self.chunk.emit(Op::DeclareEnv(iter_idx), self.current_line);
                for el in elems.iter() {
                    match el {
                        Pattern::Rest(inner) => {
                            // Collect the remaining iterator values into an array.
                            self.chunk.emit(Op::LoadEnv(iter_idx), self.current_line);
                            self.chunk.emit(Op::IteratorCollectRest, self.current_line);
                            let rest_idx = self.intern("#arr-rest");
                            self.chunk.emit(Op::DeclareEnv(rest_idx), self.current_line);
                            self.compile_pattern(inner, rest_idx, &[], kind)?;
                        }
                        _ => {
                            // Pull the next value (or undefined if exhausted).
                            self.chunk.emit(Op::LoadEnv(iter_idx), self.current_line);
                            self.chunk.emit(Op::IteratorNext, self.current_line);
                            // IteratorNext pushes [value, done]; we ignore done
                            // here (a missing element binds undefined, matching
                            // the spec where exhausted iterators yield undefined).
                            self.chunk.emit(Op::Pop, self.current_line); // discard `done`
                            let elem_idx = self.intern("#arr-elem");
                            self.chunk.emit(Op::DeclareEnv(elem_idx), self.current_line);
                            self.compile_pattern(el, elem_idx, &[], kind)?;
                        }
                    }
                }
            }
            Pattern::Object(props, rest) => {
                let mut bound_keys: Vec<Rc<str>> = Vec::new();
                for (key, target) in props {
                    // Static keys extend the access path; computed/numeric keys
                    // load the source via GetElem into a temp env binding.
                    match key {
                        PropertyKey::Ident(s) | PropertyKey::String(s) => {
                            bound_keys.push(s.clone());
                            let mut new_path = path.to_vec();
                            new_path.push(PathStep::Prop(s.clone()));
                            self.bind_destructure_target(target, temp_idx, &new_path, kind)?;
                        }
                        PropertyKey::Number(n) => {
                            let ks = crate::value::num_to_string(*n);
                            bound_keys.push(Rc::from(ks.as_str()));
                            self.load_path(temp_idx, path);
                            let key_idx = self.chunk.add_constant(Value::Number(*n));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.chunk.emit(Op::GetElem, self.current_line);
                            let t2 = self.intern("#d2");
                            self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                            self.bind_destructure_target_value(target, t2, kind)?;
                        }
                        PropertyKey::Computed(e) => {
                            // Can't statically exclude a computed key from rest.
                            self.load_path(temp_idx, path);
                            self.compile_expr(e)?;
                            self.chunk.emit(Op::GetElem, self.current_line);
                            let t2 = self.intern("#d2");
                            self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                            self.bind_destructure_target_value(target, t2, kind)?;
                        }
                        PropertyKey::Spread(_) => {
                            return Err(error::Error::syntax(
                                "unexpected spread in object pattern".to_string(),
                            ));
                        }
                    }
                }
                // Object rest: collect remaining own enumerable props into a new obj.
                if let Some(r) = rest {
                    self.load_path(temp_idx, path); // [src]
                    for k in &bound_keys {
                        let k_idx = self.chunk.add_constant(Value::String(k.clone()));
                        self.chunk.emit(Op::Const(k_idx), self.current_line);
                    }
                    self.chunk
                        .emit(Op::ObjRest(bound_keys.len()), self.current_line); // [restObj]
                    let t2 = self.intern("#drest");
                    self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                    self.bind_destructure_target_value(r, t2, kind)?;
                }
            }
            Pattern::Assign(inner, default) => {
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk.emit(Op::StrictEq, self.current_line);
                let skip = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(default)?;
                let after = self.chunk.code.len();
                self.chunk.patch_jump(skip, after);
                let t2 = self.intern("#d2");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_pattern(inner, t2, &[], kind)?;
            }
            Pattern::Rest(inner) => {
                self.load_path(temp_idx, path);
                let t2 = self.intern("#d2");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_pattern(inner, t2, &[], kind)?;
            }
        }
        Ok(())
    }

    /// Bind a destructuring target whose source value is reached via `path`
    /// (applies default if undefined, then recurses for nested patterns).
    fn bind_destructure_target(
        &mut self,
        target: &Pattern,
        temp_idx: usize,
        path: &[PathStep],
        kind: VarKind,
    ) -> error::Result<()> {
        match target {
            Pattern::Assign(inner, default) => {
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk.emit(Op::StrictEq, self.current_line);
                let skip = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(default)?;
                let after = self.chunk.code.len();
                self.chunk.patch_jump(skip, after);
                let t2 = self.intern("#d2");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_pattern(inner, t2, &[], kind)?;
            }
            other => {
                self.compile_pattern(other, temp_idx, path, kind)?;
            }
        }
        Ok(())
    }

    /// Bind a destructuring target whose source value is already loaded into
    /// env binding `temp_idx` (used for computed/numeric keys where the value
    /// was fetched via GetElem).
    fn bind_destructure_target_value(
        &mut self,
        target: &Pattern,
        temp_idx: usize,
        kind: VarKind,
    ) -> error::Result<()> {
        match target {
            Pattern::Assign(inner, default) => {
                self.load_path(temp_idx, &[]);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk.emit(Op::StrictEq, self.current_line);
                let skip = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(default)?;
                let after = self.chunk.code.len();
                self.chunk.patch_jump(skip, after);
                let t2 = self.intern("#d2");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_pattern(inner, t2, &[], kind)?;
            }
            other => {
                self.compile_pattern(other, temp_idx, &[], kind)?;
            }
        }
        Ok(())
    }

    /// Compile a destructuring *assignment* pattern (no declaration): each
    /// bound name is an existing variable that receives its value via
    /// `StoreEnvName`. `target` is an array/object literal expression.
    fn compile_assign_pattern(
        &mut self,
        target: &Expr,
        temp_idx: usize,
        path: &[PathStep],
    ) -> error::Result<()> {
        match target {
            Expr::Array(elems) => {
                for (i, el) in elems.iter().enumerate() {
                    match el {
                        Expr::Spread(inner) => {
                            self.bind_assign_rest(inner, temp_idx, path, i)?;
                        }
                        _ => {
                            let mut new_path = path.to_vec();
                            new_path.push(PathStep::Index(i));
                            self.compile_assign_pattern(el, temp_idx, &new_path)?;
                        }
                    }
                }
            }
            Expr::Object(props) => {
                for p in props {
                    let mut new_path = path.to_vec();
                    match &p.key {
                        PropertyKey::Ident(s) | PropertyKey::String(s) => {
                            new_path.push(PathStep::Prop(s.clone()));
                        }
                        PropertyKey::Number(n) => {
                            let key = self
                                .chunk
                                .add_constant(Value::String(Rc::from(n.to_string().as_str())));
                            // numeric key: load via computed element access
                            self.load_path(temp_idx, path);
                            self.chunk.emit(Op::Const(key), self.current_line);
                            self.chunk.emit(Op::GetElem, self.current_line);
                            let t2 = self.intern("#d2");
                            self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                            self.compile_assign_pattern(&p.value, t2, &[])?;
                            continue;
                        }
                        PropertyKey::Computed(e) => {
                            self.load_path(temp_idx, path);
                            self.compile_expr(e)?;
                            self.chunk.emit(Op::GetElem, self.current_line);
                            let t2 = self.intern("#d2");
                            self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                            self.compile_assign_pattern(&p.value, t2, &[])?;
                            continue;
                        }
                        PropertyKey::Spread(_) => {
                            return Err(error::Error::syntax(
                                "spread in assignment target object".to_string(),
                            ));
                        }
                    }
                    // shorthand `o.a` assigns to existing var named `a`;
                    // `o.a: b` assigns to `b` (p.value is the target).
                    if p.shorthand {
                        self.load_path(temp_idx, &new_path);
                        if let Expr::Ident(name) = &p.value {
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            self.chunk
                                .emit(Op::StoreEnvName(name_idx), self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line);
                        } else {
                            let t2 = self.intern("#d2");
                            self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                            self.compile_assign_pattern(&p.value, t2, &[])?;
                        }
                    } else {
                        self.compile_assign_pattern(&p.value, temp_idx, &new_path)?;
                    }
                }
            }
            Expr::Ident(name) => {
                self.load_path(temp_idx, path);
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk
                    .emit(Op::StoreEnvName(name_idx), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
            }
            Expr::Array(_) => {
                // nested array pattern as a direct element (rare); stash and recurse
                self.load_path(temp_idx, path);
                let t2 = self.intern("#d2");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_assign_pattern(target, t2, &[])?;
            }
            _ => {
                // Non-pattern element (e.g. a hole `[,`): just discard.
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::Pop, self.current_line);
            }
        }
        Ok(())
    }

    /// Rest binding for assignment patterns: `...rest` collects temp[i..].
    fn bind_assign_rest(
        &mut self,
        inner: &Expr,
        temp_idx: usize,
        path: &[PathStep],
        from: usize,
    ) -> error::Result<()> {
        self.load_path(temp_idx, path);
        let slice_key = self.chunk.add_constant(Value::String(Rc::from("slice")));
        self.chunk.emit(Op::Const(slice_key), self.current_line);
        let from_c = self.chunk.add_constant(Value::Number(from as f64));
        self.chunk.emit(Op::Const(from_c), self.current_line);
        self.chunk.emit(Op::CallMethod(1), self.current_line);
        let t2 = self.intern("#d2");
        self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
        self.compile_assign_pattern(inner, t2, &[])?;
        Ok(())
    }

    /// Bind a rest pattern: build an array from temp[i..] (i relative to current path end).
    fn bind_rest(
        &mut self,
        inner: &Pattern,
        temp_idx: usize,
        path: &[PathStep],
        from: usize,
        kind: VarKind,
    ) -> error::Result<()> {
        // Load the value at path (the array to slice), then call .slice(from).
        self.load_path(temp_idx, path);
        let slice_key = self.chunk.add_constant(Value::String(Rc::from("slice")));
        self.chunk.emit(Op::Const(slice_key), self.current_line);
        let from_c = self.chunk.add_constant(Value::Number(from as f64));
        self.chunk.emit(Op::Const(from_c), self.current_line);
        self.chunk.emit(Op::CallMethod(1), self.current_line); // value.slice(from)
        let t2 = self.intern("#d2");
        self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
        self.compile_pattern(inner, t2, &[], kind)?;
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> error::Result<()> {
        match expr {
            Expr::Number(n) => {
                let idx = self.chunk.add_constant(Value::Number(*n));
                self.chunk.emit(Op::Const(idx), self.current_line);
            }
            Expr::String(s) => {
                let idx = self.chunk.add_constant(Value::String(s.clone()));
                self.chunk.emit(Op::Const(idx), self.current_line);
            }
            Expr::TemplateInterp { quasis, exprs } => {
                // Build: quasis[0] + String(exprs[0]) + quasis[1] + ... + quasis[n]
                // Use repeated Add which concatenates when either side is a string.
                let first_idx = self.chunk.add_constant(Value::String(quasis[0].clone()));
                self.chunk.emit(Op::Const(first_idx), self.current_line);
                for (i, e) in exprs.iter().enumerate() {
                    self.compile_expr(e)?;
                    self.chunk.emit(Op::Add, self.current_line); // string + value -> string concat
                    let q_idx = self
                        .chunk
                        .add_constant(Value::String(quasis[i + 1].clone()));
                    self.chunk.emit(Op::Const(q_idx), self.current_line);
                    self.chunk.emit(Op::Add, self.current_line);
                }
            }
            Expr::Bool(b) => {
                self.chunk.emit(if *b { Op::True } else { Op::False }, 0);
            }
            Expr::Null => self.chunk.emit(Op::Null, self.current_line),
            Expr::Undefined => self.chunk.emit(Op::Undefined, self.current_line),
            Expr::This => {
                let name_idx = self.intern("this");
                self.chunk.emit(Op::LoadEnv(name_idx), self.current_line);
            }
            Expr::Super => {
                // `super` resolves to the parent prototype bound as `#super` in the
                // method's closure environment. Used as a callee in `super.m(...)`.
                let name_idx = self.intern("#super");
                self.chunk.emit(Op::LoadEnv(name_idx), self.current_line);
            }
            Expr::Ident(name) => {
                if self.scopes.len() > 1 {
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk
                        .emit(Op::LoadEnvName(name_idx), self.current_line);
                } else {
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk.emit(Op::Const(name_idx), self.current_line);
                    self.chunk.emit(Op::LoadGlobal, self.current_line);
                }
            }
            Expr::Update(op, prefix, target) => {
                // `x++`/`++x`/`x--`/`--x`. Stash the old value in a temp env binding
                // so the store can use a clean [obj, key, value] without fighting it.
                let delta = match op {
                    UpdateOp::Inc => 1.0,
                    UpdateOp::Dec => -1.0,
                };
                let c = self.chunk.add_constant(Value::Number(delta));
                match target.as_ref() {
                    Expr::Member {
                        object,
                        property,
                        computed,
                        ..
                    } => {
                        // Load the current value.
                        self.compile_expr(object)?; // [obj]
                        if *computed {
                            self.compile_expr(property)?; // [obj, key]
                            self.chunk.emit(Op::GetElem, self.current_line); // [oldVal]
                        } else {
                            let key = if let Expr::String(s) = property.as_ref() {
                                s.to_string()
                            } else {
                                String::new()
                            };
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Rc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line); // [obj, key]
                            self.chunk.emit(Op::GetProp, self.current_line); // [oldVal]
                        }
                        self.chunk.emit(Op::TypeCoerce, self.current_line); // [oldNum]
                                                                            // Stash oldNum; then store newNum back via a clean reload.
                        let tmp_idx = self.intern("#upd");
                        self.chunk.emit(Op::DeclareEnv(tmp_idx), self.current_line); // []
                                                                                     // Build [obj, key, newNum] and store.
                        self.compile_expr(object)?; // [obj]
                        if *computed {
                            self.compile_expr(property)?; // [obj, key]
                        } else {
                            let key = if let Expr::String(s) = property.as_ref() {
                                s.to_string()
                            } else {
                                String::new()
                            };
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Rc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            // [obj, key]
                        }
                        self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line); // [obj, key, oldNum]
                        self.chunk.emit(Op::Const(c), self.current_line); // [obj, key, oldNum, delta]
                        self.chunk.emit(Op::Add, self.current_line); // [obj, key, newNum]
                        if *computed {
                            self.chunk.emit(Op::SetElem, self.current_line);
                        } else {
                            self.chunk.emit(Op::SetProp, self.current_line);
                        }
                        self.chunk.emit(Op::Pop, self.current_line); // discard the value SetProp/SetElem leaves
                                                                     // Result: oldNum (postfix) or newNum (prefix).
                        if *prefix {
                            self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line); // [oldNum]
                            self.chunk.emit(Op::Const(c), self.current_line); // [oldNum, delta]
                            self.chunk.emit(Op::Add, self.current_line); // [newNum]
                        } else {
                            self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line);
                            // [oldNum]
                        }
                    }
                    _ => {
                        // Identifier target.
                        self.compile_expr(target)?; // [old]
                        self.chunk.emit(Op::TypeCoerce, self.current_line); // [oldNum]
                        self.chunk.emit(Op::Dup, self.current_line); // [oldNum, oldNum]
                        self.chunk.emit(Op::Const(c), self.current_line); // [oldNum, oldNum, delta]
                        self.chunk.emit(Op::Add, self.current_line); // [oldNum, newNum]
                        self.compile_assign_target(target)?;
                        self.chunk.emit(Op::Pop, self.current_line); // [oldNum]
                        if *prefix {
                            self.chunk.emit(Op::Dup, self.current_line); // [oldNum, oldNum]
                            self.chunk.emit(Op::Const(c), self.current_line); // [oldNum, oldNum, delta]
                            self.chunk.emit(Op::Add, self.current_line); // [oldNum, newNum]
                            self.chunk.emit(Op::Swap, self.current_line); // [newNum, oldNum]
                            self.chunk.emit(Op::Pop, self.current_line); // [newNum]
                        }
                    }
                }
            }
            Expr::Binary(op, l, r) => match op {
                BinOp::In => {
                    self.compile_expr(l)?;
                    self.compile_expr(r)?;
                    self.chunk.emit(Op::In, self.current_line);
                }
                BinOp::Instanceof => {
                    self.compile_expr(l)?;
                    self.compile_expr(r)?;
                    self.chunk.emit(Op::InstanceOf, self.current_line);
                }
                _ => {
                    self.compile_expr(l)?;
                    self.compile_expr(r)?;
                    self.chunk.emit(self.bin_op(op), 0);
                }
            },
            Expr::Unary(op, e) => {
                match op {
                    UnOp::Neg => {
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::Neg, self.current_line);
                    }
                    UnOp::Plus => {
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::TypeCoerce, self.current_line);
                    }
                    UnOp::Not => {
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::Not, self.current_line);
                    }
                    UnOp::BitNot => {
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::BitNot, self.current_line);
                    }
                    // unary `+` coerces its operand to a number
                    UnOp::Typeof => {
                        // `typeof undeclaredVar` must yield "undefined" instead of throwing.
                        if let Expr::Ident(name) = e.as_ref() {
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            self.chunk.emit(Op::TypeofVar(name_idx), self.current_line);
                        } else {
                            self.compile_expr(e)?;
                            self.chunk.emit(Op::TypeOf, self.current_line);
                        }
                    }
                    UnOp::Void => {
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::Pop, self.current_line);
                        self.chunk.emit(Op::Undefined, self.current_line);
                    }
                    UnOp::Delete => {
                        // `delete obj.prop` / `delete obj[expr]`
                        match e.as_ref() {
                            Expr::Member {
                                object,
                                property,
                                computed,
                                ..
                            } => {
                                self.compile_expr(object)?;
                                if *computed {
                                    self.compile_expr(property)?;
                                    self.chunk.emit(Op::DeleteProp, self.current_line);
                                } else {
                                    let key = if let Expr::String(s) = property.as_ref() {
                                        s.to_string()
                                    } else {
                                        String::new()
                                    };
                                    let key_idx = self
                                        .chunk
                                        .add_constant(Value::String(Rc::from(key.as_str())));
                                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                                    self.chunk.emit(Op::DeleteProp, self.current_line);
                                }
                            }
                            _ => {
                                // delete of a variable or other expression always succeeds.
                                self.compile_expr(e)?;
                                self.chunk.emit(Op::Pop, self.current_line);
                                self.chunk.emit(Op::True, self.current_line);
                            }
                        }
                    }
                    _ => {
                        self.compile_expr(e)?;
                    }
                }
            }
            Expr::Logical(op, l, r) => {
                self.compile_expr(l)?;
                match op {
                    LogicalOp::And => {
                        // `a && b`: if a is falsy, keep a as the result;
                        // otherwise drop a and evaluate b.
                        self.chunk.emit(Op::Dup, self.current_line);
                        let jf = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                        // a is truthy: drop the duplicate and evaluate b.
                        self.chunk.emit(Op::Pop, self.current_line);
                        self.compile_expr(r)?;
                        let end = self.chunk.code.len();
                        self.chunk.patch_jump(jf, end);
                    }
                    LogicalOp::Or => {
                        // `a || b`: if a is truthy, keep a as the result;
                        // otherwise drop a and evaluate b.
                        self.chunk.emit(Op::Dup, self.current_line);
                        let jt = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfTrue(0), self.current_line);
                        // a is falsy: drop the duplicate and evaluate b.
                        self.chunk.emit(Op::Pop, self.current_line);
                        self.compile_expr(r)?;
                        let end = self.chunk.code.len();
                        self.chunk.patch_jump(jt, end);
                    }
                    LogicalOp::Nullish => {
                        // `a ?? b`: if a is NOT null/undefined, keep a;
                        // otherwise drop a and evaluate b.
                        self.chunk.emit(Op::Dup, self.current_line);
                        let jn = self.chunk.code.len();
                        self.chunk.emit(Op::JumpIfNotNullish(0), self.current_line);
                        // a is nullish: drop the duplicate and evaluate b.
                        self.chunk.emit(Op::Pop, self.current_line);
                        self.compile_expr(r)?;
                        let end = self.chunk.code.len();
                        self.chunk.patch_jump(jn, end);
                    }
                }
            }
            Expr::Assign(op, target, value) => {
                if matches!(op, AssignOp::Assign) {
                    self.compile_assign_target_store(target, value)?;
                } else if matches!(
                    op,
                    AssignOp::AndAssign | AssignOp::OrAssign | AssignOp::NullishAssign
                ) {
                    self.compile_logical_assign(op, target, value)?;
                } else {
                    // numeric/bitwise compound assignment: load, op, store
                    self.compile_compound_assign(op, target, value)?;
                }
            }
            Expr::Conditional(c, t, f) => {
                self.compile_expr(c)?;
                let jf = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.compile_expr(t)?;
                let jend = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                self.chunk.patch_jump(jf, self.chunk.code.len());
                self.compile_expr(f)?;
                self.chunk.patch_jump(jend, self.chunk.code.len());
            }
            Expr::Object(props) => {
                self.chunk.emit(Op::NewObject, self.current_line);
                for p in props {
                    // Spread: {...expr} copies enumerable own props.
                    if let PropertyKey::Spread(e) = &p.key {
                        self.compile_expr(e)?; // [obj, src]
                        self.chunk.emit(Op::ObjSpread, self.current_line); // [obj]
                        continue;
                    }
                    self.chunk.emit(Op::Dup, self.current_line);
                    match &p.key {
                        PropertyKey::Computed(e) => {
                            // Computed key: evaluate the expression and set via SetElem
                            // (supports Symbol keys, e.g. `[Symbol.iterator]`).
                            self.compile_expr(e)?;
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::SetElem, self.current_line);
                        }
                        PropertyKey::Ident(s) => {
                            let key_idx = self.chunk.add_constant(Value::String(s.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::SetProp, self.current_line);
                        }
                        PropertyKey::String(s) => {
                            let key_idx = self.chunk.add_constant(Value::String(s.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::SetProp, self.current_line);
                        }
                        PropertyKey::Number(n) => {
                            let key = crate::value::num_to_string(*n);
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Rc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::SetProp, self.current_line);
                        }
                        PropertyKey::Spread(_) => unreachable!("spread handled above"),
                    }
                    // SetProp/SetElem leaves the assigned value on top; pop it so obj remains
                    self.chunk.emit(Op::Pop, self.current_line);
                }
            }
            Expr::Array(elements) => {
                // Build incrementally: start with an empty array, then push each element
                // (or spread each iterable). ArrayPush/SpreadPush pop [array, operand] and
                // leave the array back on the stack.
                self.chunk.emit(Op::NewArray(0), self.current_line); // [arr]
                for e in elements {
                    match e {
                        Expr::Spread(inner) => {
                            self.compile_expr(inner)?; // [arr, iterable]
                            self.chunk.emit(Op::SpreadPush, self.current_line); // [arr]
                        }
                        _ => {
                            self.compile_expr(e)?; // [arr, value]
                            self.chunk.emit(Op::ArrayPush, self.current_line); // [arr]
                        }
                    }
                }
            }
            Expr::Call {
                callee,
                args,
                optional: call_opt,
            } => {
                // check if method call
                // `super(args)`: call the parent constructor with `this`.
                if matches!(callee.as_ref(), Expr::Super) {
                    let this_idx = self.intern("this");
                    self.chunk.emit(Op::LoadEnv(this_idx), self.current_line); // [this]
                    let superctor_idx = self.intern("#superctor");
                    self.chunk
                        .emit(Op::LoadEnv(superctor_idx), self.current_line); // [this, superCtor]
                    for a in args {
                        if let Expr::Spread(_) = a {
                        } else {
                            self.compile_expr(a)?;
                        }
                    }
                    self.chunk
                        .emit(Op::CallSuperCtor(args.len()), self.current_line);
                    return Ok(());
                }
                match callee.as_ref() {
                    Expr::Member {
                        object,
                        property,
                        computed,
                        optional: m_opt,
                    } => {
                        if matches!(object.as_ref(), Expr::Super) {
                            // super.m(args): call parent proto's m with `this`.
                            let this_idx = self.intern("this");
                            self.chunk.emit(Op::LoadEnv(this_idx), self.current_line);
                            let super_idx = self.intern("#super");
                            self.chunk.emit(Op::LoadEnv(super_idx), self.current_line);
                            if *computed {
                                self.compile_expr(property)?;
                            } else {
                                let key = if let Expr::String(s) = property.as_ref() {
                                    s.to_string()
                                } else {
                                    String::new()
                                };
                                let key_idx = self
                                    .chunk
                                    .add_constant(Value::String(Rc::from(key.as_str())));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                            }
                            for a in args {
                                if let Expr::Spread(_) = a {
                                } else {
                                    self.compile_expr(a)?;
                                }
                            }
                            self.chunk
                                .emit(Op::CallSuper(args.len()), self.current_line);
                            return Ok(());
                        }
                        self.compile_expr(object)?;
                        let mut jend = 0usize;
                        if *m_opt {
                            // `o?.m(args)`: if `o` is null/undefined, short-circuit the
                            // whole method call to undefined.
                            self.chunk.emit(Op::Dup, self.current_line);
                            let jskip = self.chunk.code.len();
                            self.chunk.emit(Op::JumpIfNotNullish(0), self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line);
                            self.chunk.emit(Op::Undefined, self.current_line);
                            jend = self.chunk.code.len();
                            self.chunk.emit(Op::Jump(0), self.current_line);
                            self.chunk.patch_jump(jskip, self.chunk.code.len());
                        }
                        let key = if !*computed {
                            if let Expr::String(s) = property.as_ref() {
                                s.to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        let key_idx = self
                            .chunk
                            .add_constant(Value::String(Rc::from(key.as_str())));
                        self.chunk.emit(Op::Const(key_idx), self.current_line);
                        // push args
                        for a in args {
                            if let Expr::Spread(_) = a {
                            } else {
                                self.compile_expr(a)?;
                            }
                        }
                        self.chunk
                            .emit(Op::CallMethod(args.len()), self.current_line);
                        if *call_opt {
                            // `a?.b?.()`: the method value was fetched; if it was
                            // nullish the optional call short-circuits to undefined.
                            // Replace the just-emitted CallMethod with CallMethodOpt.
                            let pos = self.chunk.code.len() - 1;
                            self.chunk.code[pos] = Op::CallMethodOpt(args.len());
                        }
                        if *m_opt {
                            let end = self.chunk.code.len();
                            self.chunk.patch_jump(jend, end);
                        }
                    }
                    _ => {
                        // If any argument is a spread, build an args array and use CallSpread.
                        let has_spread = args.iter().any(|a| matches!(a, Expr::Spread(_)));
                        // Direct eval: a plain `eval(...)` call (callee is the
                        // unqualified identifier `eval`) runs in the caller's
                        // scope. Compile the first argument (the source) and
                        // emit CallDirectEval so the VM can compile+run it
                        // against the current frame's environment.
                        if !*call_opt
                            && matches!(callee.as_ref(), Expr::Ident(name) if &**name == "eval")
                        {
                            // Direct eval: only the first argument is the source
                            // string; extras (including spread) are ignored per
                            // spec. Suppressed only if the first arg itself is a
                            // spread (source not statically first) or `eval` is
                            // shadowed by a lexical binding.
                            let is_global_eval = self.resolve("eval").is_none();
                            let first_is_spread = args
                                .first()
                                .map(|a| matches!(a, Expr::Spread(_)))
                                .unwrap_or(false);
                            if is_global_eval && !first_is_spread {
                                // Compile only the source (first arg); arity is 1.
                                if let Some(a) = args.first() {
                                    self.compile_expr(a)?;
                                } else {
                                    self.chunk.emit(Op::Undefined, self.current_line);
                                }
                                self.chunk.emit(Op::CallDirectEval(1), self.current_line);
                                return Ok(());
                            }
                        }
                        let mut jend = 0usize;
                        self.compile_expr(callee)?; // [callee]
                        if *call_opt {
                            // `f?.(args)`: if `f` is null/undefined, short-circuit to
                            // undefined without evaluating the arguments or the call.
                            self.chunk.emit(Op::Dup, self.current_line);
                            let jskip = self.chunk.code.len();
                            self.chunk.emit(Op::JumpIfNotNullish(0), self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line);
                            self.chunk.emit(Op::Undefined, self.current_line);
                            jend = self.chunk.code.len();
                            self.chunk.emit(Op::Jump(0), self.current_line);
                            self.chunk.patch_jump(jskip, self.chunk.code.len());
                        }
                        if has_spread {
                            self.chunk.emit(Op::NewArray(0), self.current_line); // [callee, argsArr]
                            for a in args {
                                match a {
                                    Expr::Spread(inner) => {
                                        self.compile_expr(inner)?; // [callee, argsArr, iterable]
                                        self.chunk.emit(Op::SpreadPush, self.current_line);
                                        // [callee, argsArr]
                                    }
                                    _ => {
                                        self.compile_expr(a)?; // [callee, argsArr, value]
                                        self.chunk.emit(Op::ArrayPush, self.current_line);
                                        // [callee, argsArr]
                                    }
                                }
                            }
                            self.chunk.emit(Op::CallSpread, self.current_line); // pops argsArr then callee
                        } else {
                            for a in args {
                                if let Expr::Spread(_) = a {
                                } else {
                                    self.compile_expr(a)?;
                                }
                            }
                            self.chunk.emit(Op::Call(args.len()), self.current_line);
                        }
                        if *call_opt {
                            let end = self.chunk.code.len();
                            self.chunk.patch_jump(jend, end);
                        }
                    }
                }
            }
            Expr::New { callee, args } => {
                self.compile_expr(callee)?;
                for a in args {
                    if let Expr::Spread(_) = a {
                    } else {
                        self.compile_expr(a)?;
                    }
                }
                self.chunk.emit(Op::New(args.len()), self.current_line);
            }
            Expr::Member {
                object,
                property,
                computed,
                optional,
            } => {
                self.compile_expr(object)?;
                let mut jend = 0usize;
                if *optional {
                    // `a?.b` / `a?.[b]`: if `a` is null/undefined, short-circuit to
                    // undefined without evaluating the property access.
                    self.chunk.emit(Op::Dup, self.current_line);
                    let jskip = self.chunk.code.len();
                    self.chunk.emit(Op::JumpIfNotNullish(0), self.current_line);
                    // a is nullish: drop it, push undefined, jump to end.
                    self.chunk.emit(Op::Pop, self.current_line);
                    self.chunk.emit(Op::Undefined, self.current_line);
                    jend = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), self.current_line);
                    // a is not nullish: perform the property access on [a].
                    self.chunk.patch_jump(jskip, self.chunk.code.len());
                }
                if *computed {
                    self.compile_expr(property)?;
                    self.chunk.emit(Op::GetElem, self.current_line);
                } else {
                    let key = if let Expr::String(s) = property.as_ref() {
                        s.to_string()
                    } else {
                        String::new()
                    };
                    let key_idx = self
                        .chunk
                        .add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line);
                }
                if *optional {
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jend, end);
                }
            }
            Expr::Regex(pattern, flags) => {
                // Compile to `new RegExp(pattern, flags)`.
                let name_idx = self.chunk.add_constant(Value::String(Rc::from("RegExp")));
                let pat_idx = self.chunk.add_constant(Value::String(pattern.clone()));
                let flg_idx = self.chunk.add_constant(Value::String(flags.clone()));
                self.chunk.emit(Op::Const(name_idx), self.current_line);
                self.chunk.emit(Op::LoadGlobal, self.current_line);
                self.chunk.emit(Op::Const(pat_idx), self.current_line);
                self.chunk.emit(Op::Const(flg_idx), self.current_line);
                self.chunk.emit(Op::New(2), self.current_line);
            }
            Expr::Await(inner) => {
                self.compile_expr(inner)?;
                self.chunk.emit(Op::Await, self.current_line);
            }
            Expr::Yield(inner) => {
                // Eager generator: evaluate the yielded value and emit it.
                match inner {
                    Some(e) => self.compile_expr(e)?,
                    None => self.chunk.emit(Op::Undefined, self.current_line),
                }
                self.chunk.emit(Op::YieldValue, self.current_line);
            }
            Expr::YieldDelegate(inner) => {
                // `yield* expr`: obtain an iterator from `expr` and forward each
                // of its values to the outer generator via YieldValue, until the
                // iterator is done. The outer resume value (sent via next(v)) is
                // forwarded to the delegated iterator's next(v). The result of
                // the `yield*` expression is the iterator's final value.
                self.compile_expr(inner)?;
                self.chunk.emit(Op::GetIterator, self.current_line);
                let it_name_idx = self.intern("#yldel-iter");
                self.chunk
                    .emit(Op::DeclareEnv(it_name_idx), self.current_line);
                // Track the value to forward to the delegated iterator's next().
                // First pull uses no resume value (undefined).
                let resume_name_idx = self.intern("#yldel-resume");
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk
                    .emit(Op::DeclareEnv(resume_name_idx), self.current_line);
                let loop_start = self.chunk.code.len();
                // [iterator, resume] -> IteratorNextResume -> [value, done]
                self.chunk.emit(Op::LoadEnv(it_name_idx), self.current_line);
                self.chunk
                    .emit(Op::LoadEnv(resume_name_idx), self.current_line);
                self.chunk.emit(Op::IteratorNextResume, self.current_line); // [value, done]
                let done_jump = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfTrue(0), self.current_line); // if done, jump to end
                                                                       // value is on the stack; yield it to the outer generator.
                self.chunk.emit(Op::YieldValue, self.current_line); // yields `value`; leaves resume value
                                                                    // Save the resume value for the next delegated next(v).
                self.chunk
                    .emit(Op::StoreEnv(resume_name_idx), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line); // discard StoreEnv's return
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                let end = self.chunk.code.len();
                self.chunk.patch_jump(done_jump, end);
                // Iterator done: JumpIfTrue already popped `done`, leaving the
                // iterator's return value on the stack as the yield* result.
            }
            Expr::Function(f) | Expr::Arrow(f) => {
                let (func_chunk, param_slots) = self.compile_function(f)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    param_slots,
                    rest_param: f.rest_param.clone(),
                    chunk: Rc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                };
                self.funcs.push(Rc::new(fdef));
                self.chunk
                    .emit(Op::MakeClosure(func_idx), self.current_line);
            }
            Expr::Class(cls) => {
                // Build a constructor function from the class.
                // Methods become prototype properties (or static on the constructor).
                let has_ctor = cls.methods.iter().any(|m| m.is_constructor);
                // For derived classes without an explicit constructor, synthesize one
                // that forwards all arguments to `super(...)`.
                let synthetic_params: Vec<Rc<str>> = if cls.superclass.is_some() && !has_ctor {
                    (0..16)
                        .map(|i| Rc::from(format!("#a{}", i).as_str()))
                        .collect()
                } else {
                    Vec::new()
                };
                let ctor_fn = FunctionExpr {
                    name: cls.name.clone(),
                    params: cls
                        .methods
                        .iter()
                        .find(|m| m.is_constructor)
                        .map(|m| m.params.clone())
                        .or_else(|| {
                            if cls.superclass.is_some() {
                                Some(synthetic_params.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    param_defaults: cls
                        .methods
                        .iter()
                        .find(|m| m.is_constructor)
                        .map(|m| m.param_defaults.clone())
                        .unwrap_or_default(),
                    rest_param: cls
                        .methods
                        .iter()
                        .find(|m| m.is_constructor)
                        .and_then(|m| m.rest_param.clone()),
                    body: cls
                        .methods
                        .iter()
                        .find(|m| m.is_constructor)
                        .map(|m| m.body.clone())
                        .or_else(|| {
                            if cls.superclass.is_some() {
                                // super(#a0, #a1, ... #a15) — extra args are harmlessly undefined.
                                let args: Vec<Expr> = synthetic_params
                                    .iter()
                                    .map(|n| Expr::Ident(n.clone()))
                                    .collect();
                                Some(vec![Stmt {
                                    line: 0,
                                    node: StmtNode::ExprStmt(Expr::Call {
                                        callee: Box::new(Expr::Super),
                                        args,
                                        optional: false,
                                    }),
                                }])
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    is_arrow: false,
                    is_async: false,
                    is_generator: false,
                    param_decls: Vec::new(),
                    is_strict: true, // classes are always strict
                };
                let (func_chunk, param_slots) = self.compile_function(&ctor_fn)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: cls.name.clone(),
                    params: ctor_fn.params.clone(),
                    param_slots,
                    rest_param: ctor_fn.rest_param.clone(),
                    chunk: Rc::new(func_chunk),
                    num_locals: ctor_fn.params.len() + 16,
                    is_arrow: false,
                    is_async: false,
                    is_generator: false,
                };
                self.funcs.push(Rc::new(fdef));
                self.chunk
                    .emit(Op::MakeClosure(func_idx), self.current_line);
                // If there is a superclass, evaluate it and wire up the prototype chain.
                // The parent prototype is exposed to methods as the `#super` binding so that
                // `super.m(...)` can look up methods on the parent prototype.
                if let Some(super_expr) = &cls.superclass {
                    // stack: [ctor]
                    self.compile_expr(super_expr)?;
                    // stack: [ctor, parentCtor]
                    // Bind parentCtor as `#superctor` so `super(...)` calls can find it.
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, parentCtor, parentCtor]
                    let superctor_idx = self.intern("#superctor");
                    self.chunk
                        .emit(Op::DeclareEnv(superctor_idx), self.current_line); // [ctor, parentCtor]
                    let proto_key = self
                        .chunk
                        .add_constant(Value::String(Rc::from("prototype")));
                    self.chunk.emit(Op::Const(proto_key), self.current_line);
                    // stack: [ctor, parentCtor, "prototype"]; GetProp pops key then obj
                    self.chunk.emit(Op::GetProp, self.current_line); // -> [ctor, parentProto]
                                                                     // stack: [ctor, parentProto]
                                                                     // Bind parentProto as `#super` in the current env so method closures capture it.
                    let super_name_idx = self.intern("#super");
                    self.chunk
                        .emit(Op::DeclareEnv(super_name_idx), self.current_line);
                    // stack: [ctor]
                    // Set childCtor.prototype.__proto__ = parentProto (link prototype chain).
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    let cp_key = self
                        .chunk
                        .add_constant(Value::String(Rc::from("prototype")));
                    self.chunk.emit(Op::Const(cp_key), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line); // [ctor, childProto]
                    self.chunk
                        .emit(Op::LoadEnv(super_name_idx), self.current_line); // [ctor, childProto, parentProto]
                    self.chunk.emit(Op::SetProto, self.current_line); // pop parentProto,childProto; set childProto.__proto__
                                                                      // stack: [ctor]
                                                                      // Also link the constructors: childCtor.__proto__ = parentCtor (static inheritance).
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    self.chunk
                        .emit(Op::LoadEnv(super_name_idx), self.current_line); // [ctor, ctor, parentProto]
                                                                               // We need parentCtor, not parentProto, for static chain; re-derive by getting
                                                                               // constructor from parentProto. Simpler: parentCtor is the superclass expr;
                                                                               // but it's already consumed. Use parentProto.constructor.
                    let ctor_key = self
                        .chunk
                        .add_constant(Value::String(Rc::from("constructor")));
                    self.chunk.emit(Op::Const(ctor_key), self.current_line); // [ctor, ctor, parentProto, "constructor"]
                    self.chunk.emit(Op::GetProp, self.current_line); // pop "constructor",parentProto -> [ctor, ctor, parentCtor]
                    self.chunk.emit(Op::SetProto, self.current_line); // set ctor.__proto__ = parentCtor
                                                                      // stack: [ctor]
                } else {
                    // No superclass: clear any stale #super binding so methods don't capture one.
                    let super_name_idx = self.intern("#super");
                    self.chunk.emit(Op::Undefined, self.current_line);
                    self.chunk
                        .emit(Op::DeclareEnv(super_name_idx), self.current_line);
                }
                // assign each non-constructor method to prototype (or constructor if static)
                for method in &cls.methods {
                    if method.is_constructor {
                        continue;
                    }
                    let m_fn = FunctionExpr {
                        name: Some(method.name.clone()),
                        params: method.params.clone(),
                        param_defaults: method.param_defaults.clone(),
                        rest_param: method.rest_param.clone(),
                        body: method.body.clone(),
                        is_arrow: false,
                        is_async: false,
                        is_generator: false,
                        param_decls: Vec::new(),
                        is_strict: true, // class methods are always strict
                    };
                    let (m_chunk, m_slots) = self.compile_function(&m_fn)?;
                    let m_idx = self.funcs.len();
                    let mdef = crate::function::FunctionDef {
                        name: Some(method.name.clone()),
                        params: method.params.clone(),
                        param_slots: m_slots,
                        rest_param: method.rest_param.clone(),
                        chunk: Rc::new(m_chunk),
                        num_locals: method.params.len() + 16,
                        is_arrow: false,
                        is_async: false,
                        is_generator: false,
                    };
                    self.funcs.push(Rc::new(mdef));
                    if method.is_static {
                        // Constructor.method = fn
                        self.chunk.emit(Op::Dup, self.current_line); // dup constructor
                        let key_idx = self.chunk.add_constant(Value::String(method.name.clone()));
                        self.chunk.emit(Op::Const(key_idx), self.current_line);
                        self.chunk.emit(Op::MakeClosure(m_idx), self.current_line);
                        self.chunk.emit(Op::SetProp, self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line);
                    } else {
                        // Constructor.prototype.method = fn
                        self.chunk.emit(Op::Dup, self.current_line); // dup constructor
                        let proto_key = self
                            .chunk
                            .add_constant(Value::String(Rc::from("prototype")));
                        self.chunk.emit(Op::Const(proto_key), self.current_line);
                        self.chunk.emit(Op::GetProp, self.current_line);
                        // stack: [ctor, proto_obj] — push key then value then SetProp
                        let key_idx = self.chunk.add_constant(Value::String(method.name.clone()));
                        self.chunk.emit(Op::Const(key_idx), self.current_line);
                        self.chunk.emit(Op::MakeClosure(m_idx), self.current_line);
                        self.chunk.emit(Op::SetProp, self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line);
                    }
                }
                // store the constructor under the class name
                if let Some(name) = &cls.name {
                    let name_idx = self.intern(name);
                    self.chunk.emit(Op::StoreEnv(name_idx), self.current_line);
                }
            }
            Expr::Sequence(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.compile_expr(e)?;
                    if i + 1 < exprs.len() {
                        self.chunk.emit(Op::Pop, self.current_line);
                    }
                }
            }
            _ => {
                self.chunk.emit(Op::Undefined, self.current_line);
            }
        }
        Ok(())
    }

    fn compile_assign_target_store(&mut self, target: &Expr, value: &Expr) -> error::Result<()> {
        match target {
            // Destructuring assignment: `[a, b] = expr` / `{a, b} = expr`.
            Expr::Array(_) | Expr::Object(_) => {
                self.compile_expr(value)?;
                let temp_idx = self.intern("#destr");
                self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                self.compile_assign_pattern(target, temp_idx, &[])?;
            }
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    self.compile_expr(value)?;
                    self.chunk.emit(Op::SetElem, self.current_line);
                } else {
                    let key = if let Expr::String(s) = &**property {
                        s.to_string()
                    } else {
                        String::new()
                    };
                    let key_idx = self
                        .chunk
                        .add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                    self.compile_expr(value)?;
                    self.chunk.emit(Op::SetProp, self.current_line);
                }
            }
            Expr::Ident(name) => {
                self.compile_expr(value)?;
                self.chunk.emit(Op::Dup, self.current_line);
                let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                self.chunk.emit(Op::StoreEnv(name_idx), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
            }
            _ => {
                self.compile_expr(value)?;
            }
        }
        Ok(())
    }

    fn compile_assign_target(&mut self, target: &Expr) -> error::Result<()> {
        match target {
            Expr::Ident(name) => {
                if self.scopes.len() > 1 {
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk
                        .emit(Op::StoreEnvName(name_idx), self.current_line);
                } else {
                    let name_idx = self.chunk.add_constant(Value::String(Rc::from(&**name)));
                    self.chunk.emit(Op::Const(name_idx), self.current_line);
                    self.chunk.emit(Op::StoreGlobal, self.current_line);
                }
            }
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    self.chunk.emit(Op::SetElem, self.current_line);
                } else {
                    let key = if let Expr::String(s) = property.as_ref() {
                        s.to_string()
                    } else {
                        String::new()
                    };
                    let key_idx = self
                        .chunk
                        .add_constant(Value::String(Rc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                    self.chunk.emit(Op::SetProp, self.current_line);
                }
            }
            _ => {
                self.chunk.emit(Op::Pop, self.current_line);
            }
        }
        Ok(())
    }

    /// Compile a numeric/bitwise compound assignment (`+=`, `-=`, `<<=`, ...).
    /// Handles both identifier and member targets. For member targets the
    /// object/key pair is re-evaluated for the store (consistent with the
    /// simple-assignment codegen), since RuJa has no pair-duplication opcode.
    fn compile_compound_assign(
        &mut self,
        op: &AssignOp,
        target: &Expr,
        value: &Expr,
    ) -> error::Result<()> {
        let bin = self.assign_bin_op(op);
        match target {
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                // Load current value via GetProp/GetElem.
                self.compile_member_load(object, property, *computed)?;
                // value, bin -> result
                self.compile_expr(value)?;
                self.chunk.emit(bin, 0);
                // Store: re-push object/key, then SetProp consumes [obj, key, result].
                // result is currently on top; rotate so obj,key land below it.
                self.compile_member_key(object, property, *computed)?;
                // stack: [result, obj, key] -> Rot3 -> [obj, key, result] for SetProp.
                self.chunk.emit(Op::Rot3, self.current_line);
                self.chunk.emit(Op::SetProp, self.current_line);
            }
            _ => {
                self.compile_expr(target)?;
                self.compile_expr(value)?;
                self.chunk.emit(bin, 0);
                self.chunk.emit(Op::Dup, self.current_line);
                self.compile_assign_target(target)?;
                // `compile_assign_target` for an ident emits StoreEnvName
                // (which pushes Undefined on top of the duplicated result) or
                // Const+StoreGlobal (net -1). Drop the leftover Undefined so
                // the stack ends with exactly the assignment's result value,
                // matching the simple-assignment codegen (StoreEnv + Pop).
                self.chunk.emit(Op::Pop, self.current_line);
            }
        }
        Ok(())
    }

    /// Compile a logical compound assignment (`&&=`, `||=`, `??=`) with
    /// short-circuit semantics.
    fn compile_logical_assign(
        &mut self,
        op: &AssignOp,
        target: &Expr,
        value: &Expr,
    ) -> error::Result<()> {
        match target {
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                // Load current value.
                self.compile_member_load(object, property, *computed)?;
                self.chunk.emit(Op::Dup, self.current_line);
                let (cond_jump, fires_when) = match op {
                    AssignOp::AndAssign => (Op::JumpIfFalse(0), "falsy"),
                    AssignOp::OrAssign => (Op::JumpIfTrue(0), "truthy"),
                    AssignOp::NullishAssign => (Op::JumpIfNotNullish(0), "not-nullish"),
                    _ => unreachable!(),
                };
                let _ = fires_when;
                let jskip = self.chunk.code.len();
                self.chunk.emit(cond_jump, 0);
                // Short-circuit fired: drop the old value, evaluate the RHS, store it.
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(value)?;
                // stack: [result]; re-push object/key and store via SetProp.
                self.compile_member_key(object, property, *computed)?;
                // stack: [result, obj, key] -> Rot3 -> [obj, key, result] for SetProp.
                self.chunk.emit(Op::Rot3, self.current_line);
                self.chunk.emit(Op::SetProp, self.current_line);
                self.chunk.patch_jump(jskip, self.chunk.code.len());
            }
            _ => {
                self.compile_expr(target)?;
                self.chunk.emit(Op::Dup, self.current_line);
                let cond_jump = match op {
                    AssignOp::AndAssign => Op::JumpIfFalse(0),
                    AssignOp::OrAssign => Op::JumpIfTrue(0),
                    AssignOp::NullishAssign => Op::JumpIfNotNullish(0),
                    _ => unreachable!(),
                };
                let jskip = self.chunk.code.len();
                self.chunk.emit(cond_jump, 0);
                // Short-circuit fired: drop old value, evaluate RHS, store, keep result.
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(value)?;
                self.chunk.emit(Op::Dup, self.current_line);
                self.compile_assign_target(target)?;
                self.chunk.patch_jump(jskip, self.chunk.code.len());
            }
        }
        Ok(())
    }

    /// Push the current value of a member expression onto the stack.
    fn compile_member_load(
        &mut self,
        object: &Expr,
        property: &Expr,
        computed: bool,
    ) -> error::Result<()> {
        self.compile_expr(object)?;
        if computed {
            self.compile_expr(property)?;
            self.chunk.emit(Op::GetElem, self.current_line);
        } else {
            let key = if let Expr::String(s) = property {
                s.to_string()
            } else {
                String::new()
            };
            let key_idx = self
                .chunk
                .add_constant(Value::String(Rc::from(key.as_str())));
            self.chunk.emit(Op::Const(key_idx), self.current_line);
            self.chunk.emit(Op::GetProp, self.current_line);
        }
        Ok(())
    }

    /// Push the object and key for a member store (without the value).
    fn compile_member_key(
        &mut self,
        object: &Expr,
        property: &Expr,
        computed: bool,
    ) -> error::Result<()> {
        self.compile_expr(object)?;
        if computed {
            self.compile_expr(property)?;
        } else {
            let key = if let Expr::String(s) = property {
                s.to_string()
            } else {
                String::new()
            };
            let key_idx = self
                .chunk
                .add_constant(Value::String(Rc::from(key.as_str())));
            self.chunk.emit(Op::Const(key_idx), self.current_line);
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
