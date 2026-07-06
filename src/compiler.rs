//! AST → bytecode compiler.

use crate::ast::*;
use crate::bytecode::{Chunk, Op};
use crate::error;
use crate::value::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[allow(dead_code)]
pub struct Compiler {
    chunk: Chunk,
    scopes: Vec<Scope>,
    /// Function table: compiled nested functions.
    funcs: Vec<Arc<crate::function::FunctionDef>>,
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
    pending_label: Option<Arc<str>>,
    /// Active finally guard ip stack (the `PushFinally` instruction's position).
    /// Each entry is the ip of the `PushFinally` op whose target has been (or
    /// will be) patched to the finally body's start ip. Used to detect whether
    /// a break/continue is inside an active try/finally so it can divert.
    finally_stack: Vec<Vec<usize>>,
    /// When inside a switch body, the env index of `#sw_val` so that
    /// expression statements store their value instead of popping it.
    switch_val_depth: Option<usize>,
    /// Stack of active switch contexts, each entry is `(sw_val_idx,
    /// saved_sw_val_idx, loop_stack_index)` so that a `continue` which exits
    /// one or more switches can copy the current switch completion value into
    /// the enclosing completion slot before jumping.
    switch_ctx_stack: Vec<(usize, Option<usize>, usize)>,
    /// Current source line being compiled; emitted onto each `Op` so runtime
    /// errors can report `(at line N)`. Updated as `compile_stmt` enters a stmt.
    current_line: usize,
}

#[allow(dead_code)]
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
    /// True when this compiler scope corresponds to a runtime environment
    /// frame that must be popped during non-local loop unwinding.
    has_runtime_env: bool,
    /// Whether strict-mode rules apply in this scope (inherited from the
    /// enclosing strict context or set by a `"use strict"` directive).
    is_strict: bool,
}

/// A step in the access path used while compiling destructuring patterns.
#[derive(Clone)]
#[allow(dead_code)]
enum PathStep {
    Index(usize),
    Prop(Arc<str>),
    RestFrom(usize),
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// A loop-stack frame: (continue target, pending break jumps,
/// pending continue jumps, optional label, is switch, scope depth at entry).
type LoopFrame = (usize, Vec<usize>, Vec<usize>, Option<Arc<str>>, bool, usize);
pub type GlobalDeclarationNames = (Vec<Arc<str>>, Vec<Arc<str>>, Vec<Arc<str>>);

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            chunk: Chunk::new(),
            scopes: vec![Scope {
                bindings: HashMap::new(),
                is_function: true,
                base: 0,
                is_with: false,
                has_runtime_env: true,
                is_strict: false,
            }],
            funcs: Vec::new(),
            names: Vec::new(),
            name_map: HashMap::new(),
            loop_stack: Vec::new(),
            pending_label: None,
            finally_stack: Vec::new(),
            switch_val_depth: None,
            switch_ctx_stack: Vec::new(),
            current_line: 0,
        }
    }

    fn intern(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.name_map.get(name) {
            return idx;
        }
        let idx = self.chunk.add_constant(Value::String(Arc::from(name)));
        self.name_map.insert(name.to_string(), idx);
        idx
    }

    /// Whether the current scope is strict (inherited from the enclosing
    /// strict context or set by a `"use strict"` directive).
    fn is_strict(&self) -> bool {
        self.scopes.last().map(|s| s.is_strict).unwrap_or(false)
    }

    /// Copy the current completion value of every switch that this
    /// `continue` exits into the switch's saved completion slot. This mirrors
    /// the spec's UpdateEmpty step so that abrupt completions from a switch
    /// carry the last non-empty value (e.g. `do { switch { case: { 6; continue; } } }`).
    fn propagate_switch_completion_on_continue(&mut self, target_loop_idx: usize) {
        for idx in (0..self.switch_ctx_stack.len()).rev() {
            let (sw_val_idx, saved_idx, loop_idx) = self.switch_ctx_stack[idx];
            if loop_idx <= target_loop_idx {
                break;
            }
            if let Some(saved) = saved_idx {
                self.chunk
                    .emit(Op::LoadEnvName(sw_val_idx), self.current_line);
                self.chunk.emit(Op::StoreEnvName(saved), self.current_line);
            }
        }
    }

    /// Reset the completion value slot to undefined when tracking is active.
    /// Called at the start of compound statements so that the statement's
    /// completion value starts fresh (per ES spec: each statement has its
    /// own completion, not inherited from the previous one).
    fn reset_completion(&mut self) {
        if let Some(sv) = self.switch_val_depth {
            self.chunk.emit(Op::Undefined, self.current_line);
            self.chunk.emit(Op::StoreEnvName(sv), self.current_line);
            self.chunk.emit(Op::Pop, self.current_line);
        }
    }

    pub fn compile_program(
        &mut self,
        program: &Program,
    ) -> error::Result<(Chunk, Vec<Arc<crate::function::FunctionDef>>)> {
        // The top-level scope inherits the program's strictness (from a leading
        // "use strict" directive prologue).
        if let Some(top) = self.scopes.last_mut() {
            top.is_strict = program.is_strict;
        }
        let _n = program.body.len();
        // Hoist function declarations: compile them first so they're available
        // before any statement in the body runs.
        let mut fn_decl_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for stmt in &program.body {
            if let StmtNode::FunctionDecl(f) = &stmt.node {
                if let Some(name) = &f.name {
                    fn_decl_names.insert(name.to_string());
                }
                self.compile_stmt(stmt)?;
                let _ = f;
            }
        }
        // Hoist `var` declarations as undefined at the top level.
        for stmt in &program.body {
            let mut var_names = Vec::new();
            if program.is_strict {
                collect_var_names_recursive_skip_functions(&stmt.node, &mut var_names);
            } else {
                collect_var_names_recursive(&stmt.node, &mut var_names);
            }
            for name in &var_names {
                // Skip names already hoisted by function declaration hoisting.
                if fn_decl_names.contains(&**name) {
                    continue;
                }
                self.declare(name, VarKind::Var)?;
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                // Use Undefined + DeclareVar (always creates a binding)
                // instead of StoreGlobal (which would throw in strict mode
                // when the binding doesn't exist yet).
                self.chunk.emit(Op::HoistVar(name_idx), self.current_line);
            }
        }
        // Hoist lexical (`let`/`const`) declarations into the TDZ at the top
        // level, so accessing them before the declaration throws ReferenceError.
        {
            let lex = Self::collect_lexical_names(&program.body);
            self.emit_lexical_hoist(&lex)?;
        }
        // Allocate a completion-value slot. Expression statements store their
        // value here; if/while/for bodies inherit the slot so that the last
        // expression in a taken branch becomes the script's completion value.
        let comp_idx = self.intern("#comp");
        self.chunk.emit(Op::Undefined, self.current_line);
        self.chunk.emit(Op::DeclareEnv(comp_idx), self.current_line);
        let saved_comp = self.switch_val_depth;
        self.switch_val_depth = Some(comp_idx);
        for stmt in &program.body {
            // Function declarations were hoisted above; skip them in the body pass.
            if matches!(&stmt.node, StmtNode::FunctionDecl(_)) {
                continue;
            }
            self.compile_stmt(stmt)?;
        }
        self.switch_val_depth = saved_comp;
        // Push the completion value onto the stack for Halt to return.
        self.chunk.emit(Op::LoadEnv(comp_idx), self.current_line);
        self.chunk.emit(Op::Halt, self.current_line);
        let mut chunk = std::mem::take(&mut self.chunk);
        chunk.is_strict = program.is_strict;
        let funcs = std::mem::take(&mut self.funcs);
        Ok((chunk, funcs))
    }

    fn push_scope_with_runtime(&mut self, is_function: bool, has_runtime_env: bool) {
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
            has_runtime_env,
            is_strict,
        });
    }

    fn push_scope(&mut self, is_function: bool) {
        self.push_scope_with_runtime(is_function, false);
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
            has_runtime_env: true,
            is_strict,
        });
    }

    /// Emit PopScope/PopWithEnv ops to unwind scopes opened since `loop_depth`,
    /// so `break`/`continue` don't leak `with` or block scopes past the loop.
    #[allow(dead_code)]
    fn emit_scope_unwind(&mut self, loop_depth: usize) {
        for i in (loop_depth..self.scopes.len()).rev() {
            if !self.scopes[i].has_runtime_env {
                continue;
            }
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
        self.loop_stack.push((
            continue_target,
            Vec::new(),
            Vec::new(),
            label,
            false,
            self.scopes.len(),
        ));
    }

    /// Like `begin_loop` but tags the loop with a label so `break label` /
    /// `continue label` can target it.
    #[allow(dead_code)]
    fn begin_labeled_loop(&mut self, continue_target: usize, label: Arc<str>) {
        self.loop_stack.push((
            continue_target,
            Vec::new(),
            Vec::new(),
            Some(label),
            false,
            self.scopes.len(),
        ));
    }

    /// Patch the current loop's continue target (used when the continue target is
    /// only known after the body, e.g. the update block of a C-style for).
    fn set_continue_target(&mut self, target: usize) {
        if let Some((cont, _, cont_jumps, _, _, _)) = self.loop_stack.last_mut() {
            *cont = target;
            // patch already-emitted continue jumps to the real target
            for j in cont_jumps.drain(..) {
                self.chunk.patch_jump(j, target);
            }
        }
    }

    /// Resolve the innermost active finally body start ip, if any.
    /// Returns None if no try/finally is active.
    fn finally_active(&self) -> bool {
        !self.finally_stack.is_empty()
    }

    #[allow(dead_code)]
    fn record_divert(&mut self, divert_ip: usize) {
        if let Some(frame) = self.finally_stack.last_mut() {
            frame.push(divert_ip);
        }
    }

    fn patch_diverts(&mut self, finally_start: usize) {
        if let Some(diverts) = self.finally_stack.last_mut() {
            for &ip in diverts.iter() {
                match &mut self.chunk.code[ip] {
                    Op::DivertBreak(t) => *t = finally_start,
                    Op::DivertContinue(t, _) => *t = finally_start,
                    _ => {}
                }
            }
            diverts.clear();
        }
    }

    /// End a loop: patch all pending `break` jumps to `end`.
    fn end_loop(&mut self, end: usize) {
        if let Some((cont, breaks, _, _, _, _)) = self.loop_stack.pop() {
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

    fn declare_var(&mut self, name: &str) -> error::Result<()> {
        let Some(scope_idx) = self.scopes.iter().rposition(|scope| scope.is_function) else {
            return self.declare(name, VarKind::Var);
        };
        let scope = &mut self.scopes[scope_idx];
        if let Some((_, existing_kind)) = scope.bindings.get(name) {
            if *existing_kind != VarKind::Var {
                return Err(error::Error::syntax(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
            return Ok(());
        }
        let slot = scope.base + scope.bindings.len();
        scope
            .bindings
            .insert(name.to_string(), (slot, VarKind::Var));
        Ok(())
    }

    /// Declare a function parameter. Parameters participate in the function's
    /// variable environment, so `var x` and function declarations named `x`
    /// may reuse/overwrite a parameter binding. Runtime still declares the
    /// binding as `BindingKind::Param` so default-parameter TDZ semantics stay
    /// intact.
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
                .insert(name.to_string(), (slot, VarKind::Var));
        }
        Ok(())
    }

    /// Collect all binding names introduced by a destructuring pattern.
    fn pattern_names(pattern: &Pattern, out: &mut Vec<Arc<str>>) {
        match pattern {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Hole => {}
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

    fn lexical_for_head_bindings(left: &Stmt) -> Option<(VarKind, Vec<Arc<str>>)> {
        match &left.node {
            StmtNode::VarDecl {
                kind: kind @ (VarKind::Let | VarKind::Const),
                decls,
            } => Some((*kind, decls.iter().map(|(name, _)| name.clone()).collect())),
            StmtNode::Destructure {
                kind: kind @ (VarKind::Let | VarKind::Const),
                pattern,
                ..
            } => {
                let mut names = Vec::new();
                Self::pattern_names(pattern, &mut names);
                Some((*kind, names))
            }
            _ => None,
        }
    }

    fn lexical_bindings_from_names(kind: VarKind, names: &[Arc<str>]) -> Vec<(Arc<str>, VarKind)> {
        names.iter().map(|name| (name.clone(), kind)).collect()
    }

    fn compile_for_var_existing_lexical(&mut self, left: &Stmt) -> error::Result<()> {
        match &left.node {
            StmtNode::VarDecl { kind, decls } => {
                if let Some((name, _)) = decls.first() {
                    let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                    match kind {
                        VarKind::Const => self
                            .chunk
                            .emit(Op::InitEnvConst(name_idx), self.current_line),
                        _ => self.chunk.emit(Op::InitEnv(name_idx), self.current_line),
                    }
                } else {
                    self.chunk.emit(Op::Pop, self.current_line);
                }
            }
            StmtNode::Destructure { kind, pattern, .. } => {
                let temp_idx = self.intern("#destr");
                self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                self.compile_pattern(pattern, temp_idx, &[], *kind)?;
            }
            _ => self.compile_for_var(left)?,
        }
        Ok(())
    }

    /// Collect lexical (`let`/`const`) names declared at the top level of a
    /// statement list. Does NOT descend into nested blocks/functions/loops:
    /// those introduce their own scopes and hoist their own lexicals.
    fn collect_lexical_names(body: &[Stmt]) -> Vec<(Arc<str>, VarKind)> {
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
                StmtNode::ExprStmt(Expr::Class(c)) if c.is_declaration => {
                    if let Some(name) = &c.name {
                        out.push((name.clone(), VarKind::Let));
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn collect_block_lexical_names(
        body: &[Stmt],
        include_function_declarations: bool,
    ) -> Vec<(Arc<str>, VarKind)> {
        let mut out = Self::collect_lexical_names(body);
        if include_function_declarations {
            for stmt in body {
                if let StmtNode::FunctionDecl(f) = &stmt.node {
                    if let Some(name) = &f.name {
                        out.push((name.clone(), VarKind::Let));
                    }
                }
            }
        }
        out
    }

    fn collect_switch_lexical_names(cases: &[SwitchCase]) -> Vec<(Arc<str>, VarKind)> {
        let mut out = Vec::new();
        for case in cases {
            for stmt in &case.body {
                match &stmt.node {
                    StmtNode::VarDecl { kind, decls } => {
                        if *kind != VarKind::Var {
                            for (name, _) in decls {
                                out.push((name.clone(), *kind));
                            }
                        }
                    }
                    StmtNode::Destructure { kind, pattern, .. } if *kind != VarKind::Var => {
                        let mut names = Vec::new();
                        Self::pattern_names(pattern, &mut names);
                        for name in names {
                            out.push((name, *kind));
                        }
                    }
                    StmtNode::FunctionDecl(f) => {
                        if let Some(name) = &f.name {
                            out.push((name.clone(), VarKind::Let));
                        }
                    }
                    StmtNode::ExprStmt(Expr::Class(c)) if c.is_declaration => {
                        if let Some(name) = &c.name {
                            out.push((name.clone(), VarKind::Let));
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }

    fn collect_switch_var_names(cases: &[SwitchCase]) -> Vec<Arc<str>> {
        let mut out = Vec::new();
        for case in cases {
            for stmt in &case.body {
                collect_var_names_recursive_skip_functions(&stmt.node, &mut out);
            }
        }
        out
    }

    /// Collect top-level `var` and function-declaration names from a statement
    /// list (for direct-eval leak into the caller's function scope).
    /// Recursively descends into nested blocks, loops, if/else, switch, and
    /// try/catch bodies so that `var` declarations inside these are hoisted
    /// to the function scope (per ES spec: `var` is function-scoped).
    pub fn collect_var_names(body: &[Stmt]) -> Vec<Arc<str>> {
        let mut out = Vec::new();
        for stmt in body {
            collect_var_names_recursive(&stmt.node, &mut out);
        }
        out
    }

    pub fn collect_global_declaration_names(
        body: &[Stmt],
        is_strict: bool,
    ) -> GlobalDeclarationNames {
        let lexical_names = Self::collect_lexical_names(body)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let mut function_names = Vec::new();
        for stmt in body {
            if let StmtNode::FunctionDecl(f) = &stmt.node {
                if let Some(name) = &f.name {
                    function_names.push(name.clone());
                }
            }
        }
        let mut var_names = Vec::new();
        for stmt in body {
            if is_strict {
                collect_var_names_recursive_skip_functions(&stmt.node, &mut var_names);
            } else {
                collect_var_names_recursive(&stmt.node, &mut var_names);
            }
        }
        for name in &function_names {
            if !var_names.iter().any(|existing| existing == name) {
                var_names.push(name.clone());
            }
        }
        (lexical_names, var_names, function_names)
    }

    /// Emit TDZ (uninitialized) declarations for lexical bindings at scope entry.
    /// Also registers them in the compiler's scope table so `resolve` works and
    /// later `declare` calls for the same name are no-ops (preventing slot reuse).
    fn emit_lexical_hoist(&mut self, names: &[(Arc<str>, VarKind)]) -> error::Result<()> {
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
            StmtNode::ExprStmt(Expr::Class(c)) if c.is_declaration => {
                self.compile_expr(&Expr::Class(c.clone()))?;
                self.chunk.emit(Op::Pop, self.current_line);
            }
            StmtNode::ExprStmt(e) => {
                self.compile_expr(e)?;
                if let Some(sv) = self.switch_val_depth {
                    // Inside a switch: store the expression value as the
                    // switch completion value instead of discarding it.
                    self.chunk.emit(Op::StoreEnvName(sv), self.current_line);
                    // StoreEnvName pushes Undefined as a side effect; pop it
                    // to keep the stack balanced (net effect: push expr, pop expr).
                    self.chunk.emit(Op::Pop, self.current_line);
                } else {
                    self.chunk.emit(Op::Pop, self.current_line);
                }
            }
            StmtNode::VarDecl { kind, decls } => {
                for (name, init) in decls {
                    if *kind == VarKind::Var {
                        // `var` is function-scoped: declare at the function-scope root
                        // (or global at top level), regardless of block nesting.
                        self.declare_var(name)?;
                        let Some(e) = init else {
                            continue;
                        };
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                        self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
                        self.compile_expr(e)?;
                        self.chunk.emit(Op::Swap, self.current_line);
                        self.chunk.emit(Op::PutValue, self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line);
                    } else {
                        if let Some(e) = init {
                            self.compile_expr(e)?;
                        } else {
                            self.chunk.emit(Op::Undefined, self.current_line);
                        }
                        // Lexical (let/const): already declared uninitialized at scope
                        // entry by `emit_lexical_hoist`. Initialize the binding with the
                        // value now (this lifts the TDZ).
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
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
                self.push_scope_with_runtime(false, true);
                self.chunk.emit(Op::PushScope, self.current_line);
                let strict_block = self.is_strict();
                if !strict_block {
                    // Sloppy block-level function declarations keep RuJa's
                    // existing Annex-B-style function-scope behavior.
                    for s in body {
                        if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                            self.compile_stmt(s)?;
                        }
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
                            self.declare_var(name)?;
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            if self.scopes.len() == 1 {
                                self.chunk.emit(Op::Const(name_idx), self.current_line);
                                self.chunk.emit(Op::StoreGlobal, self.current_line);
                            } else {
                                self.chunk.emit(Op::HoistVar(name_idx), self.current_line);
                            }
                        }
                    }
                }
                // Hoist lexical (`let`/`const`) declarations into the TDZ at block
                // entry, so accessing them before the declaration throws ReferenceError.
                {
                    let lex = Self::collect_block_lexical_names(body, strict_block);
                    self.emit_lexical_hoist(&lex)?;
                }
                if strict_block {
                    // In strict mode, block-level function declarations are
                    // lexical bindings scoped to the block, not Annex B vars.
                    for s in body {
                        if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                            self.compile_stmt(s)?;
                        }
                    }
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
                self.reset_completion();
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
                self.reset_completion();
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
                self.reset_completion();
                let loop_start = self.chunk.code.len();
                // continue target is the condition test, which is after the
                // body. Use usize::MAX as placeholder; set_continue_target
                // patches it once the condition IP is known.
                self.begin_loop(usize::MAX);
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
                let lexical_head = init.as_deref().and_then(Self::lexical_for_head_bindings);
                if let Some((kind, names)) = &lexical_head {
                    let bindings = Self::lexical_bindings_from_names(*kind, names);
                    self.chunk.emit(Op::PushScope, self.current_line);
                    self.push_scope_with_runtime(false, true);
                    self.emit_lexical_hoist(&bindings)?;
                } else {
                    self.push_scope(false);
                }
                if let Some(init_stmt) = init {
                    self.compile_stmt(init_stmt)?;
                }
                self.reset_completion();
                let per_iteration_let = lexical_head.is_some();
                let loop_names: Vec<Arc<str>> = lexical_head
                    .as_ref()
                    .map(|(_, names)| names.clone())
                    .unwrap_or_default();
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
                self.compile_stmt(body)?;
                let continue_target = self.chunk.code.len();
                if per_iteration_let {
                    self.chunk
                        .emit(Op::RecloneLetNames(loop_names_idx), self.current_line);
                }
                if let Some(u) = update {
                    self.compile_expr(u)?;
                    self.chunk.emit(Op::Pop, self.current_line);
                }
                // if there's no update, continue jumps to the condition (loop_start).
                self.set_continue_target(continue_target);
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                let normal_cleanup = self.chunk.code.len();
                if let Some(jf) = jump_false {
                    self.chunk.patch_jump(jf, normal_cleanup);
                }
                if lexical_head.is_some() {
                    if per_iteration_let {
                        self.chunk.emit(Op::RestoreParentEnv, self.current_line);
                    }
                    self.chunk.emit(Op::PopScope, self.current_line);
                }
                self.end_loop(normal_cleanup);
                self.pop_scope();
            }
            StmtNode::ForOf {
                left,
                right,
                body,
                is_await,
            } => {
                self.reset_completion();
                // for (let x of iterable): iterate values. `for await` uses the
                // async iterator protocol (Symbol.asyncIterator) and awaits each
                // next() result.
                self.push_scope(false);
                let lexical_head = Self::lexical_for_head_bindings(left);
                if let Some((kind, names)) = &lexical_head {
                    let bindings = Self::lexical_bindings_from_names(*kind, names);
                    self.chunk.emit(Op::PushScope, self.current_line);
                    self.push_scope_with_runtime(false, true);
                    self.emit_lexical_hoist(&bindings)?;
                    self.compile_expr(right)?;
                    self.chunk.emit(Op::PopScope, self.current_line);
                    self.pop_scope();
                } else {
                    self.compile_expr(right)?;
                }
                if *is_await {
                    self.chunk.emit(Op::GetAsyncIterator, self.current_line);
                } else {
                    // GetIterator pops the iterable, pushes an iterator object.
                    self.chunk.emit(Op::GetIterator, self.current_line);
                }
                let it_name_idx = self.intern("#iter");
                self.chunk
                    .emit(Op::DeclareEnv(it_name_idx), self.current_line);
                let done_name_idx = self.intern("#iterDone");
                self.chunk.emit(Op::False, self.current_line);
                self.chunk
                    .emit(Op::DeclareEnv(done_name_idx), self.current_line);
                let finally_guard_ip = self.chunk.code.len();
                self.chunk.emit(Op::PushFinally(0), self.current_line);
                self.finally_stack.push(Vec::new());
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
                if let Some((kind, names)) = &lexical_head {
                    let bindings = Self::lexical_bindings_from_names(*kind, names);
                    self.chunk.emit(Op::PushScope, self.current_line);
                    self.push_scope_with_runtime(false, true);
                    self.emit_lexical_hoist(&bindings)?;
                    self.compile_for_var_existing_lexical(left)?;
                    self.compile_stmt(body)?;
                    self.chunk.emit(Op::PopScope, self.current_line);
                    self.pop_scope();
                } else {
                    self.compile_for_var(left)?;
                    self.compile_stmt(body)?;
                }
                self.chunk.emit(Op::Pop, self.current_line); // discard body's expr result
                self.chunk.emit(Op::Jump(loop_start), self.current_line);
                let end = self.chunk.code.len();
                self.chunk.patch_jump(done_jump, end);
                self.end_loop(end);
                // When done, the stale value is still on the stack; drop it.
                self.chunk.emit(Op::Pop, self.current_line);
                let finally_start = self.chunk.code.len();
                if let Op::PushFinally(ref mut target) = self.chunk.code[finally_guard_ip] {
                    *target = finally_start;
                }
                self.patch_diverts(finally_start);
                self.chunk.emit(Op::PopFinally, self.current_line);
                self.finally_stack.pop();
                self.chunk.emit(
                    Op::IteratorCloseIfAbrupt {
                        iter: it_name_idx,
                        done: done_name_idx,
                        inner_continue: Some(loop_start),
                        ignore_close_errors: false,
                    },
                    self.current_line,
                );
                self.chunk.emit(Op::PopFinallyRethrow, self.current_line);
                self.pop_scope();
            }
            StmtNode::ForIn { left, right, body } => self.compile_for_in(left, right, body)?,
            StmtNode::With { object, body } => {
                if self.is_strict() {
                    return Err(error::Error::syntax(
                        "'with' statement is not allowed in strict mode".to_string(),
                    ));
                }
                self.reset_completion();
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
                self.reset_completion();
                let has_finally = finally_body.is_some();
                let has_catch = catch_body.is_some();
                // --- try body, guarded by the catch handler (and finally guard) ---
                let try_start = self.chunk.code.len();
                if has_finally {
                    // Push a finally guard whose target is patched to finally_start
                    // below; non-local transfers (return/break/continue/throw) inside
                    // try/catch divert to it with their completion recorded.
                    self.chunk.emit(Op::PushFinally(0), self.current_line);
                }
                if has_catch {
                    self.chunk.emit(Op::PushTry(0), self.current_line); // catch handler placeholder
                }
                let finally_guard_ip = if has_finally {
                    // finally guard is the first opcode (PushFinally) of the try.
                    try_start
                } else {
                    usize::MAX
                };
                let _ = &finally_guard_ip;
                let try_guard_ip = if has_catch {
                    if has_finally {
                        try_start + 1
                    } else {
                        try_start
                    }
                } else {
                    usize::MAX
                };
                if has_finally {
                    self.finally_stack.push(Vec::new());
                }
                self.compile_stmt(try_body)?;
                if has_catch {
                    self.chunk.emit(Op::PopTry, self.current_line);
                }
                // Normal try completion -> jump to finally (or end).
                let jump_past_catch = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                // --- catch handler ---
                let catch_start = self.chunk.code.len();
                if has_catch {
                    if let Op::PushTry(ref mut h) = self.chunk.code[try_guard_ip] {
                        *h = catch_start;
                    }
                    self.push_scope_with_runtime(false, true);
                    self.chunk.emit(Op::PushScope, self.current_line);
                    if let Some(param) = catch_param {
                        match param {
                            crate::ast::Pattern::Ident(name) => {
                                self.declare(name, VarKind::Let)?;
                                let name_idx = self.intern(name);
                                self.chunk.emit(Op::DeclareEnv(name_idx), self.current_line);
                            }
                            pat => {
                                // Destructuring catch parameter: the thrown
                                // value is on the stack. Store it in a temp
                                // then destructure.
                                let temp_name: Arc<str> = Arc::from("#catchval");
                                self.declare(&temp_name, VarKind::Let)?;
                                let name_idx = self.intern(&temp_name);
                                self.chunk.emit(Op::DeclareEnv(name_idx), self.current_line);
                                self.compile_pattern(pat, name_idx, &[], VarKind::Let)?;
                            }
                        }
                    }
                    self.compile_stmt(catch_body.as_ref().unwrap())?;
                    self.chunk.emit(Op::PopScope, self.current_line);
                    self.pop_scope();
                    // Normal catch completion -> jump to finally (or end).
                    let jump_past_catch2 = self.chunk.code.len();
                    self.chunk.emit(Op::Jump(0), self.current_line);
                    // --- finally entry ---
                    let finally_start = self.chunk.code.len();
                    if has_finally {
                        if let Op::PushFinally(ref mut t) = self.chunk.code[finally_guard_ip] {
                            *t = finally_start;
                        }
                        self.patch_diverts(finally_start);
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
                        self.patch_diverts(finally_start);
                    }
                    self.chunk.patch_jump(jump_past_catch, finally_start);
                }
                if let Some(fin) = finally_body {
                    // Drop the finally guard before running the finally body
                    // AND pop the finally_stack so that non-local transfers
                    // inside the finally (return/throw/break/continue) use
                    // direct jumps instead of DivertContinue/DivertBreak
                    // (which would loop back into this same finally).
                    self.chunk.emit(Op::PopFinally, self.current_line);
                    self.finally_stack.pop();
                    let saved_sv_depth = self.switch_val_depth;
                    let saved_finally_completion = saved_sv_depth.map(|comp_idx| {
                        let temp_name = format!("#finallycomp{}", self.chunk.code.len());
                        let temp_idx = self.intern(&temp_name);
                        self.chunk.emit(Op::LoadEnv(comp_idx), self.current_line);
                        self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                        self.chunk.emit(Op::Undefined, self.current_line);
                        self.chunk
                            .emit(Op::StoreEnvName(comp_idx), self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line);
                        (comp_idx, temp_idx)
                    });
                    self.compile_stmt(fin)?;
                    if let Some((comp_idx, temp_idx)) = saved_finally_completion {
                        self.chunk.emit(Op::LoadEnv(temp_idx), self.current_line);
                        self.chunk
                            .emit(Op::StoreEnvName(comp_idx), self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line);
                    }
                    // Re-raise the pending completion (return/break/continue/throw)
                    // that diverted here. A normal completion falls through.
                    self.chunk.emit(Op::PopFinallyRethrow, self.current_line);
                    self.switch_val_depth = saved_sv_depth;
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
                    chunk: Arc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                    has_parameter_expressions: Self::has_parameter_expressions(f),
                    length: Self::fn_length(f),
                    is_method: f.is_method,
                    has_name_binding: false,
                    is_derived: false,
                };
                self.funcs.push(Arc::new(fdef));
                self.chunk
                    .emit(Op::MakeClosure(func_idx), self.current_line);
                if let Some(name) = &f.name {
                    if let Some((_, kind)) = self.resolve(name) {
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                        if kind == VarKind::Var {
                            self.chunk
                                .emit(Op::StoreEnvName(name_idx), self.current_line);
                        } else {
                            self.chunk.emit(Op::InitLet(name_idx), self.current_line);
                        }
                    } else {
                        // store as global so recursive calls can find it
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                        self.chunk
                            .emit(Op::DeclareGlobalFunction(name_idx), self.current_line);
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
                        .rposition(|(_, _, _, lbl, _, _): &LoopFrame| {
                            lbl.as_ref().is_some_and(|x| x == l)
                        })
                } else {
                    if self.loop_stack.is_empty() {
                        None
                    } else {
                        Some(self.loop_stack.len() - 1)
                    }
                };
                if let Some(i) = target {
                    let scope_depth = self.loop_stack[i].5;
                    let active = self.finally_active();
                    self.emit_scope_unwind(scope_depth);
                    if active {
                        let divert_ip = self.chunk.code.len();
                        self.chunk.emit(Op::DivertBreak(0), self.current_line);
                        self.finally_stack.last_mut().unwrap().push(divert_ip);
                    }
                    let (_, breaks, _, _, _, _) = &mut self.loop_stack[i];
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
                        .rposition(|(_, _, _, lbl, _, _): &LoopFrame| {
                            lbl.as_ref().is_some_and(|x| x == l)
                        })
                } else {
                    if self.loop_stack.is_empty() {
                        None
                    } else {
                        // Find innermost non-switch loop (switch uses
                        // begin_loop for break support, but continue must
                        // target the enclosing real loop).
                        self.loop_stack
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, (_, _, _, _, is_switch, _))| !is_switch)
                            .map(|(i, _)| i)
                    }
                };
                if let Some(i) = target {
                    self.propagate_switch_completion_on_continue(i);
                    let scope_depth = self.loop_stack[i].5;
                    let active = self.finally_active();
                    let cont = self.loop_stack[i].0;
                    self.emit_scope_unwind(scope_depth);
                    if active {
                        if cont != usize::MAX {
                            let divert_ip = self.chunk.code.len();
                            self.chunk
                                .emit(Op::DivertContinue(0, cont), self.current_line);
                            self.finally_stack.last_mut().unwrap().push(divert_ip);
                        } else {
                            let divert_ip = self.chunk.code.len();
                            self.chunk.emit(Op::DivertBreak(0), self.current_line);
                            self.finally_stack.last_mut().unwrap().push(divert_ip);
                            let j = self.chunk.code.len();
                            self.chunk.emit(Op::Jump(0), self.current_line);
                            self.loop_stack[i].2.push(j);
                        }
                    } else if cont != usize::MAX {
                        self.chunk.emit(Op::Jump(cont), self.current_line);
                    } else {
                        // Target unknown yet (C-style for); record and patch later.
                        let j = self.chunk.code.len();
                        self.chunk.emit(Op::Jump(0), self.current_line);
                        self.loop_stack[i].2.push(j);
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
                    StmtNode::While { .. }
                        | StmtNode::DoWhile { .. }
                        | StmtNode::For { .. }
                        | StmtNode::ForIn { .. }
                        | StmtNode::ForOf { .. }
                ) {
                    // Hand the label to the inner loop's begin_loop by stashing
                    // it on a pending-label field that begin_loop consumes.
                    self.pending_label = Some(label.clone());
                    self.compile_stmt(body)?;
                } else {
                    // Non-loop labeled statement: push a frame that only honors
                    // `break label`. continue is invalid here; mark as MAX.
                    self.loop_stack.push((
                        usize::MAX,
                        Vec::new(),
                        Vec::new(),
                        Some(label.clone()),
                        false,
                        self.scopes.len(),
                    ));
                    let result = self.compile_stmt(body);
                    // break jumps patch to here (after the body).
                    if let Some((_, breaks, _, _, _, _)) = self.loop_stack.pop() {
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
                // Switch introduces a new lexical environment (like a block).
                self.push_scope_with_runtime(false, true);
                self.chunk.emit(Op::PushScope, self.current_line);
                // Hoist `var` declarations from all case bodies.
                {
                    let var_names = Self::collect_switch_var_names(cases);
                    for name in &var_names {
                        self.declare_var(name)?;
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                        self.chunk.emit(Op::HoistVar(name_idx), self.current_line);
                    }
                }
                // Hoist lexical declarations into TDZ at switch entry.
                {
                    let lex = Self::collect_switch_lexical_names(cases);
                    self.emit_lexical_hoist(&lex)?;
                }
                // Hoist function declarations from all case bodies after the
                // switch lexical names exist, so they stay scoped to the
                // CaseBlock instead of leaking into the outer variable env.
                for case in cases.iter() {
                    for s in &case.body {
                        if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                            self.compile_stmt(s)?;
                        }
                    }
                }
                let sw_idx = self.intern("#switch");
                self.chunk.emit(Op::DeclareEnv(sw_idx), self.current_line);
                // Track the switch completion value: the last non-empty
                // expression value seen before a break or end. Initialized
                // to undefined; each expression statement updates it.
                let sw_val_idx = self.intern("#sw_val");
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk
                    .emit(Op::DeclareEnv(sw_val_idx), self.current_line);
                // Switch uses a loop frame for break support, but marks
                // is_switch=true so continue targets the enclosing loop.
                let saved_sw_val = self.switch_val_depth;
                let switch_loop_idx = self.loop_stack.len();
                self.loop_stack.push((
                    usize::MAX,
                    Vec::new(),
                    Vec::new(),
                    None,
                    true,
                    self.scopes.len(),
                ));
                self.switch_ctx_stack
                    .push((sw_val_idx, saved_sw_val, switch_loop_idx));
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
                self.switch_val_depth = Some(sw_val_idx);
                for (i, case) in cases.iter().enumerate() {
                    body_starts[i] = Some(self.chunk.code.len());
                    for s in &case.body {
                        if matches!(&s.node, StmtNode::FunctionDecl(_)) {
                            continue;
                        }
                        self.compile_stmt(s)?;
                    }
                }
                self.switch_val_depth = saved_sw_val;
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
                // Push the tracked completion value onto the stack.
                self.chunk.emit(Op::LoadEnv(sw_val_idx), self.current_line);
                if let Some(cv) = saved_sw_val {
                    let comp_idx = cv;
                    self.chunk
                        .emit(Op::StoreEnvName(comp_idx), self.current_line);
                    self.chunk.emit(Op::Pop, self.current_line);
                }
                self.chunk.emit(Op::PopScope, self.current_line);
                self.pop_scope();
                self.end_loop(end);
                self.switch_ctx_stack.pop();
            }
            #[allow(unreachable_patterns)]
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
                    let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                    match kind {
                        VarKind::Const => {
                            self.chunk
                                .emit(Op::DeclareConst(name_idx), self.current_line);
                        }
                        _ => {
                            self.chunk.emit(Op::DeclareEnv(name_idx), self.current_line);
                        }
                    }
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
                // Non-declaration left side (e.g. `for (x.y of/in ...)`):
                // the iterator value is on the stack. For a simple identifier
                // we can use StoreEnvName/StoreGlobal directly (they pop the
                // value). For a member expression we need to evaluate obj+key
                // first, then the value is already on the stack in the right
                // position for SetProp ([obj, key, value]).
                if let StmtNode::ExprStmt(expr) = &left.node {
                    match expr {
                        Expr::Ident(name) => self.store_identifier_target_value(name),
                        Expr::Member {
                            object,
                            property,
                            computed,
                            ..
                        } => {
                            // Stack: [value]. We need [obj, key, value] for
                            // SetProp/SetElem. Evaluate obj, swap value below
                            // it, then evaluate key, swap again.
                            self.compile_expr(object)?;
                            // [value, obj] -> [obj, value]
                            self.chunk.emit(Op::Swap, self.current_line);
                            if *computed {
                                self.compile_expr(property)?;
                                // [obj, value, key] -> [obj, key, value]
                                self.chunk.emit(Op::Swap, self.current_line);
                                self.chunk.emit(Op::SetElem, self.current_line);
                            } else {
                                let key = if let Expr::String(s) = property.as_ref() {
                                    s.to_string()
                                } else {
                                    String::new()
                                };
                                let key_idx = self
                                    .chunk
                                    .add_constant(Value::String(Arc::from(key.as_str())));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                                // [obj, value, key] -> [obj, key, value]
                                self.chunk.emit(Op::Swap, self.current_line);
                                self.chunk.emit(Op::SetProp, self.current_line);
                            }
                            self.chunk.emit(Op::Pop, self.current_line);
                        }
                        Expr::Array(_) | Expr::Object(_) => {
                            let temp_idx = self.intern("#for-assign");
                            self.chunk.emit(Op::DeclareEnv(temp_idx), self.current_line);
                            self.compile_assign_value_to_target(expr, temp_idx, None)?;
                        }
                        _ => {
                            self.compile_assign_target(expr)?;
                        }
                    }
                } else {
                    self.compile_stmt(left)?;
                }
            }
        }
        Ok(())
    }

    /// Compile `for (left in right)`: iterate enumerable own+inherited string keys.
    fn compile_for_in(&mut self, left: &Stmt, right: &Expr, body: &Stmt) -> error::Result<()> {
        self.push_scope(false);
        self.reset_completion();
        let lexical_head = Self::lexical_for_head_bindings(left);
        if let Some((kind, names)) = &lexical_head {
            let bindings = Self::lexical_bindings_from_names(*kind, names);
            self.chunk.emit(Op::PushScope, self.current_line);
            self.push_scope_with_runtime(false, true);
            self.emit_lexical_hoist(&bindings)?;
            self.compile_expr(right)?;
            self.chunk.emit(Op::PopScope, self.current_line);
            self.pop_scope();
        } else {
            self.compile_expr(right)?;
        }
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
        if let Some((kind, names)) = &lexical_head {
            let bindings = Self::lexical_bindings_from_names(*kind, names);
            self.chunk.emit(Op::PushScope, self.current_line);
            self.push_scope_with_runtime(false, true);
            self.emit_lexical_hoist(&bindings)?;
            self.compile_for_var_existing_lexical(left)?;
            self.compile_stmt(body)?;
            self.chunk.emit(Op::PopScope, self.current_line);
            self.pop_scope();
        } else {
            self.compile_for_var(left)?;
            self.compile_stmt(body)?;
        }
        self.chunk.emit(Op::Pop, self.current_line);
        self.chunk.emit(Op::Jump(loop_start), self.current_line);
        let end = self.chunk.code.len();
        self.chunk.patch_jump(done_jump, end);
        self.end_loop(end);
        self.chunk.emit(Op::Pop, self.current_line);
        self.pop_scope();
        Ok(())
    }

    /// ES function `length`: number of parameters before the first default
    /// or the rest parameter.
    pub fn fn_length(f: &FunctionExpr) -> usize {
        if f.rest_param.is_some() {
            return f.params.len();
        }
        for (i, d) in f.param_defaults.iter().enumerate() {
            if d.is_some() {
                return i;
            }
        }
        f.params.len()
    }

    pub fn has_parameter_expressions(f: &FunctionExpr) -> bool {
        f.param_defaults.iter().any(Option::is_some)
            || f.rest_param.is_some()
            || !f.param_decls.is_empty()
            || Self::parameter_prelude_len(f) > 0
    }

    fn parameter_prelude_len(f: &FunctionExpr) -> usize {
        f.body
            .iter()
            .take_while(|stmt| {
                if stmt.line != 0 {
                    return false;
                }
                match &stmt.node {
                    StmtNode::Destructure {
                        kind: VarKind::Let,
                        init: Some(init),
                        ..
                    } => match init {
                        Expr::Ident(name) => {
                            f.params.iter().any(|param| param.as_ref() == name.as_ref())
                                || f.rest_param.as_deref() == Some(name.as_ref())
                        }
                        _ => false,
                    },
                    _ => false,
                }
            })
            .count()
    }

    pub fn compile_function(&mut self, f: &FunctionExpr) -> error::Result<(Chunk, Vec<usize>)> {
        let saved_chunk = std::mem::take(&mut self.chunk);
        let saved_names = std::mem::take(&mut self.name_map);
        let saved_switch_val = self.switch_val_depth;
        self.switch_val_depth = None;
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            is_function: true,
            base: 0,
            is_with: false,
            has_runtime_env: true,
            is_strict: f.is_strict,
        });

        // Declare each parameter in the compiler's function-scope binding
        // table and remember the raw argument slot for each formal. The VM
        // stores argument values into `locals[slot]` before the frame runs, so
        // defaults can read the raw argument via `LoadLocal` (bypassing the
        // environment TDZ) while the runtime binding stays in the TDZ until
        // `InitLet` -- this is what makes `function f(a = b, b = 2)` a
        // ReferenceError while `function f(a, b = a)` still works.
        let mut param_slots: Vec<usize> = Vec::with_capacity(f.params.len());
        for (i, param) in f.params.iter().enumerate() {
            self.declare_param(param, f.is_strict)?;
            param_slots.push(i);
        }
        // Declare the rest parameter in the compiler's scope table so that
        // references to it resolve to a local slot. The VM declares the
        // actual binding (with the array value) at call time.
        if let Some(rest) = &f.rest_param {
            self.declare_param(rest, f.is_strict)?;
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
                // Undefined path: the binding is still in the TDZ. Evaluate
                // the default and initialize the existing parameter binding.
                self.chunk.emit(Op::Pop, self.current_line);
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
        let parameter_prelude_len = Self::parameter_prelude_len(f);
        let (parameter_prelude, body_stmts) = f.body.split_at(parameter_prelude_len);
        for stmt in parameter_prelude {
            self.compile_stmt(stmt)?;
        }
        if Self::has_parameter_expressions(f) {
            let param_names: std::collections::HashSet<&str> =
                f.params.iter().map(|p| p.as_ref()).collect();
            let lex = Self::collect_lexical_names(body_stmts);
            for (name, _) in &lex {
                if param_names.contains(name.as_ref()) {
                    return Err(error::Error::syntax(format!(
                        "Identifier '{}' has already been declared",
                        name
                    )));
                }
            }
            self.chunk.emit(Op::PushFunctionScope, self.current_line);
        }
        // Hoist `var` declarations within the function body as undefined.
        for stmt in body_stmts {
            let mut var_names = Vec::new();
            if f.is_strict {
                collect_var_names_recursive_skip_functions(&stmt.node, &mut var_names);
            } else {
                collect_var_names_recursive(&stmt.node, &mut var_names);
            }
            for name in &var_names {
                // Skip names that will be hoisted by function declaration
                // hoisting below (declaring them as Var here would make
                // resolve() find them and use StoreLocal instead of DeclareVar,
                // causing a storage mismatch).
                let is_fn_decl = body_stmts.iter().any(|s| {
                    matches!(&s.node, StmtNode::FunctionDecl(fd) if fd.name.as_deref() == Some(&**name))
                });
                if is_fn_decl {
                    continue;
                }
                self.declare_var(name)?;
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::HoistVar(name_idx), self.current_line);
            }
        }
        // Hoist function declarations: compile them first so they're available
        // before any statement in the body runs (matches top-level behavior).
        for stmt in body_stmts {
            if let StmtNode::FunctionDecl(_) = &stmt.node {
                self.compile_stmt(stmt)?;
            }
        }
        // Hoist lexical (`let`/`const`) declarations into the TDZ at function
        // entry, so accessing them before the declaration throws ReferenceError.
        {
            let lex = Self::collect_lexical_names(body_stmts);
            self.emit_lexical_hoist(&lex)?;
        }
        let body_start_ip = self.chunk.code.len();
        for stmt in body_stmts {
            if matches!(&stmt.node, StmtNode::FunctionDecl(_)) {
                continue;
            }
            self.compile_stmt(stmt)?;
        }
        self.chunk.emit(Op::ReturnUndefined, self.current_line);
        self.pop_scope();
        let mut func_chunk = std::mem::take(&mut self.chunk);
        func_chunk.is_strict = f.is_strict;
        func_chunk.body_start_ip = body_start_ip;
        self.name_map = saved_names;
        self.chunk = saved_chunk;
        self.switch_val_depth = saved_switch_val;
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
                let mut bound_keys: Vec<Arc<str>> = Vec::new();
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
                            bound_keys.push(Arc::from(ks.as_str()));
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
            Pattern::Hole => {
                // An elision hole: the source value (already loaded by the
                // caller's IteratorNext) is consumed but not bound. Discard it.
                self.chunk.emit(Op::Pop, self.current_line);
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
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::GetIterator, self.current_line);
                let iter_idx = self.intern("#arr-assign-iter");
                self.chunk.emit(Op::DeclareEnv(iter_idx), self.current_line);
                for el in elems {
                    match el {
                        Expr::ArrayHole => {
                            self.chunk.emit(Op::LoadEnv(iter_idx), self.current_line);
                            self.chunk.emit(Op::IteratorNext, self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line);
                        }
                        Expr::Spread(inner) => {
                            self.chunk.emit(Op::LoadEnv(iter_idx), self.current_line);
                            self.chunk.emit(Op::IteratorCollectRest, self.current_line);
                            let rest_idx = self.intern("#arr-assign-rest");
                            self.chunk.emit(Op::DeclareEnv(rest_idx), self.current_line);
                            self.compile_assign_value_to_target(inner, rest_idx, None)?;
                        }
                        _ => {
                            let member_target = self
                                .compile_assign_member_target_temps(Self::assignment_target(el))?;
                            self.chunk.emit(Op::LoadEnv(iter_idx), self.current_line);
                            self.chunk.emit(Op::IteratorNext, self.current_line);
                            let done_idx = self.intern("#arr-assign-done");
                            self.chunk.emit(Op::DeclareEnv(done_idx), self.current_line);
                            let elem_idx = self.intern("#arr-assign-elem");
                            self.chunk.emit(Op::DeclareEnv(elem_idx), self.current_line);
                            self.compile_assign_value_guarded_by_iterator(
                                el,
                                elem_idx,
                                iter_idx,
                                done_idx,
                                member_target,
                            )?;
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
                            if let Some(member_target) =
                                self.compile_assign_member_target_temps(&p.value)?
                            {
                                self.load_path(temp_idx, &new_path);
                                self.store_current_value_to_member_target(member_target);
                                continue;
                            }
                        }
                        PropertyKey::Number(n) => {
                            let key = self
                                .chunk
                                .add_constant(Value::String(Arc::from(n.to_string().as_str())));
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
                            self.compile_expr(e)?;
                            self.chunk.emit(Op::ToPropertyKey, self.current_line);
                            let source_key = self.intern("#dkey");
                            self.chunk
                                .emit(Op::DeclareEnv(source_key), self.current_line);
                            if let Some(member_target) =
                                self.compile_assign_member_target_temps(&p.value)?
                            {
                                self.load_path(temp_idx, path);
                                self.chunk.emit(Op::LoadEnv(source_key), self.current_line);
                                self.chunk.emit(Op::GetElem, self.current_line);
                                self.store_current_value_to_member_target(member_target);
                                continue;
                            }
                            self.load_path(temp_idx, path);
                            self.chunk.emit(Op::LoadEnv(source_key), self.current_line);
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
                            self.store_identifier_target_value(name);
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
            Expr::Assign(AssignOp::Assign, left, default) => {
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
                self.compile_assign_pattern(left, t2, &[])?;
            }
            Expr::Ident(name) => {
                self.load_path(temp_idx, path);
                self.store_identifier_target_value(name);
                self.chunk.emit(Op::Pop, self.current_line);
            }
            Expr::Member { .. } => {
                if let Some(member_target) = self.compile_assign_member_target_temps(target)? {
                    self.load_path(temp_idx, path);
                    self.store_current_value_to_member_target(member_target);
                }
            }
            _ => {
                // Non-pattern element (e.g. a hole `[,`): just discard.
                self.load_path(temp_idx, path);
                self.chunk.emit(Op::Pop, self.current_line);
            }
        }
        Ok(())
    }

    fn assignment_target(expr: &Expr) -> &Expr {
        if let Expr::Assign(AssignOp::Assign, left, _) = expr {
            left
        } else {
            expr
        }
    }

    fn compile_assign_value_guarded_by_iterator(
        &mut self,
        target: &Expr,
        value_idx: usize,
        iter_idx: usize,
        done_idx: usize,
        member_target: Option<(usize, usize, bool)>,
    ) -> error::Result<()> {
        let finally_guard_ip = self.chunk.code.len();
        self.chunk.emit(Op::PushFinally(0), self.current_line);
        self.compile_assign_value_to_target(target, value_idx, member_target)?;
        self.chunk.emit(Op::PopFinally, self.current_line);
        let jump_after_finally = self.chunk.code.len();
        self.chunk.emit(Op::Jump(0), self.current_line);
        let finally_start = self.chunk.code.len();
        if let Op::PushFinally(ref mut target) = self.chunk.code[finally_guard_ip] {
            *target = finally_start;
        }
        self.chunk.emit(Op::PopFinally, self.current_line);
        self.chunk.emit(
            Op::IteratorCloseIfAbrupt {
                iter: iter_idx,
                done: done_idx,
                inner_continue: None,
                ignore_close_errors: true,
            },
            self.current_line,
        );
        self.chunk.emit(Op::PopFinallyRethrow, self.current_line);
        let after_finally = self.chunk.code.len();
        self.chunk.patch_jump(jump_after_finally, after_finally);
        Ok(())
    }

    fn compile_assign_value_to_target(
        &mut self,
        target: &Expr,
        value_idx: usize,
        member_target: Option<(usize, usize, bool)>,
    ) -> error::Result<()> {
        match target {
            Expr::Assign(AssignOp::Assign, left, default) => {
                self.load_path(value_idx, &[]);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::Undefined, self.current_line);
                self.chunk.emit(Op::StrictEq, self.current_line);
                let skip = self.chunk.code.len();
                self.chunk.emit(Op::JumpIfFalse(0), self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(default)?;
                let after = self.chunk.code.len();
                self.chunk.patch_jump(skip, after);
                let t2 = self.intern("#arr-assign-value");
                self.chunk.emit(Op::DeclareEnv(t2), self.current_line);
                self.compile_assign_value_to_target(left, t2, member_target)?;
            }
            Expr::Ident(name) => {
                self.load_path(value_idx, &[]);
                self.store_identifier_target_value(name);
                self.chunk.emit(Op::Pop, self.current_line);
            }
            Expr::Member { .. } => {
                let target = match member_target {
                    Some(target) => target,
                    None => self
                        .compile_assign_member_target_temps(target)?
                        .ok_or_else(|| error::Error::internal("expected member target"))?,
                };
                self.load_path(value_idx, &[]);
                self.store_current_value_to_member_target(target);
            }
            Expr::Array(_) | Expr::Object(_) => {
                self.compile_assign_pattern(target, value_idx, &[])?;
            }
            _ => {
                self.load_path(value_idx, &[]);
                self.chunk.emit(Op::Pop, self.current_line);
            }
        }
        Ok(())
    }

    fn compile_assign_member_target_temps(
        &mut self,
        target: &Expr,
    ) -> error::Result<Option<(usize, usize, bool)>> {
        let Expr::Member {
            object,
            property,
            computed,
            ..
        } = target
        else {
            return Ok(None);
        };
        self.compile_expr(object)?;
        let obj_idx = self.intern("#dtarget_obj");
        self.chunk.emit(Op::DeclareEnv(obj_idx), self.current_line);
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
                .add_constant(Value::String(Arc::from(key.as_str())));
            self.chunk.emit(Op::Const(key_idx), self.current_line);
        }
        let key_idx = self.intern("#dtarget_key");
        self.chunk.emit(Op::DeclareEnv(key_idx), self.current_line);
        Ok(Some((obj_idx, key_idx, *computed)))
    }

    fn store_current_value_to_member_target(&mut self, target: (usize, usize, bool)) {
        let (obj_idx, key_idx, computed) = target;
        self.chunk.emit(Op::LoadEnv(obj_idx), self.current_line);
        self.chunk.emit(Op::Swap, self.current_line);
        self.chunk.emit(Op::LoadEnv(key_idx), self.current_line);
        self.chunk.emit(Op::Swap, self.current_line);
        if computed {
            self.chunk.emit(Op::SetElem, self.current_line);
        } else {
            self.chunk.emit(Op::SetProp, self.current_line);
        }
        self.chunk.emit(Op::Pop, self.current_line);
    }

    fn compile_super_member_target_temps(
        &mut self,
        property: &Expr,
        computed: bool,
    ) -> error::Result<(usize, usize, usize)> {
        let this_name = self.intern("this");
        self.chunk.emit(Op::LoadEnv(this_name), self.current_line);
        let receiver_idx = self.intern("#super_target_receiver");
        self.chunk
            .emit(Op::DeclareEnv(receiver_idx), self.current_line);

        let super_name = self.intern("#super");
        self.chunk.emit(Op::LoadEnv(super_name), self.current_line);
        self.chunk.emit(Op::GetProto, self.current_line);
        let super_idx = self.intern("#super_target_base");
        self.chunk
            .emit(Op::DeclareEnv(super_idx), self.current_line);

        if computed {
            self.compile_expr(property)?;
            self.chunk.emit(Op::ToPropertyKey, self.current_line);
        } else {
            let key = if let Expr::String(s) = property {
                s.to_string()
            } else {
                String::new()
            };
            let key_idx = self
                .chunk
                .add_constant(Value::String(Arc::from(key.as_str())));
            self.chunk.emit(Op::Const(key_idx), self.current_line);
        }
        let key_idx = self.intern("#super_target_key");
        self.chunk.emit(Op::DeclareEnv(key_idx), self.current_line);

        Ok((receiver_idx, super_idx, key_idx))
    }

    fn load_super_member_target_temps(&mut self, target: (usize, usize, usize)) {
        let (receiver_idx, super_idx, key_idx) = target;
        self.chunk
            .emit(Op::LoadEnv(receiver_idx), self.current_line);
        self.chunk.emit(Op::LoadEnv(super_idx), self.current_line);
        self.chunk.emit(Op::LoadEnv(key_idx), self.current_line);
    }

    fn emit_make_closure_capturing_super_from_stack(&mut self, func_idx: usize) {
        self.chunk.emit(Op::PushScope, self.current_line);
        self.push_scope_with_runtime(false, true);
        let super_idx = self.intern("#super");
        self.chunk
            .emit(Op::DeclareEnv(super_idx), self.current_line);
        self.chunk
            .emit(Op::MakeClosure(func_idx), self.current_line);
        self.chunk.emit(Op::PopScope, self.current_line);
        self.pop_scope();
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
        let slice_key = self.chunk.add_constant(Value::String(Arc::from("slice")));
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
    #[allow(dead_code)]
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
        let slice_key = self.chunk.add_constant(Value::String(Arc::from("slice")));
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
            Expr::BigInt(n) => {
                let idx = self.chunk.add_constant(Value::BigInt(n.clone()));
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
                    // Template interpolation uses the *string* hint, not the
                    // number/default hint that binary `+` would use, so coerce
                    // via ToPrimitive(string) + ToString before concatenating.
                    self.chunk.emit(Op::ToString, self.current_line);
                    self.chunk.emit(Op::Add, self.current_line); // string + string concat
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
            Expr::ArrayHole => self.chunk.emit(Op::Undefined, self.current_line),
            Expr::This => {
                let name_idx = self.intern("this");
                self.chunk.emit(Op::LoadEnv(name_idx), self.current_line);
            }
            Expr::Super => {
                // `super` resolves to the parent prototype. In class methods,
                // it's bound as `#super` in the closure environment. In object
                // literal methods, fall back to `this.__proto__`.
                let name_idx = self.intern("#super");
                self.chunk.emit(Op::LoadEnv(name_idx), self.current_line);
            }
            Expr::Ident(name) => {
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk
                    .emit(Op::LoadEnvName(name_idx), self.current_line);
            }
            Expr::Update(op, prefix, target) => {
                // `x++`/`++x`/`x--`/`--x`: evaluate the reference once, read
                // its value, write the new value through the same reference,
                // and return either the old or new numeric value.
                let inc_op = || match op {
                    UpdateOp::Inc => Op::Inc,
                    UpdateOp::Dec => Op::Dec,
                };
                match target.as_ref() {
                    Expr::Member {
                        object,
                        property,
                        computed,
                        ..
                    } => {
                        if matches!(object.as_ref(), Expr::Super) {
                            let target =
                                self.compile_super_member_target_temps(property, *computed)?;
                            self.load_super_member_target_temps(target);
                            self.chunk.emit(Op::GetSuperProp, self.current_line);
                            self.chunk.emit(Op::ToNumeric, self.current_line);

                            let old_idx = self.intern("#super_update_old");
                            self.chunk.emit(Op::Dup, self.current_line);
                            self.chunk.emit(Op::DeclareEnv(old_idx), self.current_line);
                            self.chunk.emit(inc_op(), self.current_line);

                            let new_idx = self.intern("#super_update_new");
                            self.chunk.emit(Op::DeclareEnv(new_idx), self.current_line);
                            self.load_super_member_target_temps(target);
                            self.chunk.emit(Op::LoadEnv(new_idx), self.current_line);
                            self.chunk.emit(Op::SetSuperProp, self.current_line);
                            if !*prefix {
                                self.chunk.emit(Op::Pop, self.current_line);
                                self.chunk.emit(Op::LoadEnv(old_idx), self.current_line);
                            }
                            return Ok(());
                        }
                        self.compile_expr(object)?;
                        if *computed {
                            self.compile_expr(property)?;
                            self.chunk.emit(Op::CheckNullBase, self.current_line);
                            self.chunk.emit(Op::ToString, self.current_line);
                        } else {
                            self.chunk.emit(Op::CheckNullBase, self.current_line);
                            let key = if let Expr::String(s) = property.as_ref() {
                                s.to_string()
                            } else {
                                String::new()
                            };
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Arc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                        }
                        self.chunk.emit(Op::Dup2, self.current_line);
                        if *computed {
                            self.chunk.emit(Op::GetElem, self.current_line);
                        } else {
                            self.chunk.emit(Op::GetProp, self.current_line);
                        }
                        self.chunk.emit(Op::ToNumeric, self.current_line);
                        let tmp_idx = self.intern("#upd");
                        self.chunk.emit(Op::Dup, self.current_line);
                        self.chunk.emit(Op::DeclareEnv(tmp_idx), self.current_line);
                        self.chunk.emit(inc_op(), self.current_line);
                        if *computed {
                            self.chunk.emit(Op::SetElem, self.current_line);
                        } else {
                            self.chunk.emit(Op::SetProp, self.current_line);
                        }
                        if !*prefix {
                            self.chunk.emit(Op::Pop, self.current_line);
                            self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line);
                        }
                    }
                    _ => {
                        // Identifier target.
                        // Private field target: `obj.#f++` / `++obj.#f`.
                        if let Expr::PrivateGet { object, name } = target.as_ref() {
                            // Evaluate the private reference base once and
                            // keep it for SetPrivate after GetPrivate consumes
                            // its copy.
                            self.compile_expr(object)?; // [obj]
                            self.chunk.emit(Op::Dup, self.current_line); // [obj, obj]
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            self.chunk.emit(Op::GetPrivate(name_idx), self.current_line);
                            // [obj, oldVal]
                            self.chunk.emit(Op::ToNumeric, self.current_line);
                            // [obj, oldNum]
                            let tmp_idx = self.intern("#upd");
                            self.chunk.emit(Op::Dup, self.current_line);
                            self.chunk.emit(Op::DeclareEnv(tmp_idx), self.current_line);
                            // [obj, oldNum]
                            self.chunk.emit(inc_op(), self.current_line);
                            // [obj, newNum]
                            self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
                            // [newNum]
                            if *prefix {
                                // keep newNum
                            } else {
                                self.chunk.emit(Op::Pop, self.current_line);
                                self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line);
                                // [oldNum]
                            }
                            return Ok(());
                        }
                        if let Expr::Ident(name) = target.as_ref() {
                            let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                            self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
                            self.chunk.emit(Op::Dup, self.current_line);
                            self.chunk.emit(Op::GetValue, self.current_line);
                            self.chunk.emit(Op::ToNumeric, self.current_line);
                            let tmp_idx = self.intern("#upd");
                            self.chunk.emit(Op::Dup, self.current_line);
                            self.chunk.emit(Op::DeclareEnv(tmp_idx), self.current_line);
                            self.chunk.emit(inc_op(), self.current_line);
                            self.chunk.emit(Op::Swap, self.current_line);
                            self.chunk.emit(Op::PutValue, self.current_line);
                            if !*prefix {
                                self.chunk.emit(Op::Pop, self.current_line);
                                self.chunk.emit(Op::LoadEnv(tmp_idx), self.current_line);
                            }
                        } else {
                            self.compile_expr(target)?;
                            self.chunk.emit(Op::ToNumeric, self.current_line);
                            self.chunk.emit(Op::Dup, self.current_line);
                            self.chunk.emit(inc_op(), self.current_line);
                            self.compile_assign_target(target)?;
                            self.chunk.emit(Op::Pop, self.current_line);
                            if *prefix {
                                self.chunk.emit(Op::Dup, self.current_line);
                                self.chunk.emit(inc_op(), self.current_line);
                                self.chunk.emit(Op::Swap, self.current_line);
                                self.chunk.emit(Op::Pop, self.current_line);
                            }
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
                            Expr::Ident(name) => {
                                if self.is_strict() {
                                    return Err(error::Error::syntax(format!(
                                        "Cannot delete identifier {} in strict mode",
                                        name
                                    )));
                                }
                                let name_idx =
                                    self.chunk.add_constant(Value::String(Arc::from(&**name)));
                                self.chunk.emit(Op::DeleteVar(name_idx), self.current_line);
                            }
                            Expr::Member {
                                object,
                                property,
                                computed,
                                ..
                            } => {
                                if matches!(object.as_ref(), Expr::Super) {
                                    let this_idx = self.intern("this");
                                    self.chunk.emit(Op::LoadEnv(this_idx), self.current_line);
                                    self.chunk.emit(Op::Pop, self.current_line);
                                    if *computed {
                                        self.compile_expr(property)?;
                                        self.chunk.emit(Op::Pop, self.current_line);
                                    }
                                    let msg_idx = self.chunk.add_constant(Value::String(
                                        Arc::from("Cannot delete super property"),
                                    ));
                                    self.chunk
                                        .emit(Op::ThrowReference(msg_idx), self.current_line);
                                    return Ok(());
                                }
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
                                        .add_constant(Value::String(Arc::from(key.as_str())));
                                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                                    self.chunk.emit(Op::DeleteProp, self.current_line);
                                }
                            }
                            #[allow(unreachable_patterns)]
                            _ => {
                                // delete of a non-reference expression still
                                // evaluates its operand, then succeeds.
                                self.compile_expr(e)?;
                                self.chunk.emit(Op::Pop, self.current_line);
                                self.chunk.emit(Op::True, self.current_line);
                            }
                        }
                    }
                    #[allow(unreachable_patterns)]
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
                    // Getter/setter: {get x(){...}} / {set x(v){...}}
                    if matches!(
                        p.kind,
                        crate::ast::PropKind::Get | crate::ast::PropKind::Set
                    ) {
                        let kind = if matches!(p.kind, crate::ast::PropKind::Get) {
                            0u8
                        } else {
                            1u8
                        };
                        self.chunk.emit(Op::Dup, self.current_line); // [obj, obj]
                        match &p.key {
                            PropertyKey::Ident(s) | PropertyKey::String(s) => {
                                let key_idx = self.chunk.add_constant(Value::String(s.clone()));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                            }
                            PropertyKey::Number(n) => {
                                let key = crate::value::num_to_string(*n);
                                let key_idx = self
                                    .chunk
                                    .add_constant(Value::String(Arc::from(key.as_str())));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                            }
                            PropertyKey::Computed(e) => {
                                self.compile_expr(e)?;
                                self.chunk.emit(Op::ToPropertyKey, self.current_line);
                            }
                            PropertyKey::Spread(_) => unreachable!(),
                        }
                        self.compile_expr(&p.value)?; // [obj, obj, key, fn]
                        self.chunk.emit(Op::DefineAccessor(kind), self.current_line); // [obj, obj]
                        self.chunk.emit(Op::Pop, self.current_line); // [obj]
                        continue;
                    }
                    self.chunk.emit(Op::Dup, self.current_line);
                    match &p.key {
                        PropertyKey::Computed(e) => {
                            // Computed key: evaluate the expression and define an
                            // own data property. ToPropertyKey runs before the
                            // value expression and preserves Symbol keys.
                            self.compile_expr(e)?;
                            self.chunk.emit(Op::ToPropertyKey, self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::DefineDataProperty, self.current_line);
                        }
                        PropertyKey::Ident(s) => {
                            if !p.computed && !p.shorthand && s.as_ref() == "__proto__" {
                                self.compile_expr(&p.value)?;
                                self.chunk.emit(Op::SetProto, self.current_line);
                                continue;
                            }
                            let key_idx = self.chunk.add_constant(Value::String(s.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::DefineDataProperty, self.current_line);
                        }
                        PropertyKey::String(s) => {
                            if !p.computed && !p.shorthand && s.as_ref() == "__proto__" {
                                self.compile_expr(&p.value)?;
                                self.chunk.emit(Op::SetProto, self.current_line);
                                continue;
                            }
                            let key_idx = self.chunk.add_constant(Value::String(s.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::DefineDataProperty, self.current_line);
                        }
                        PropertyKey::Number(n) => {
                            let key = crate::value::num_to_string(*n);
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Arc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                            self.compile_expr(&p.value)?;
                            self.chunk.emit(Op::DefineDataProperty, self.current_line);
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
                        Expr::ArrayHole => {
                            self.chunk.emit(Op::ArrayHolePush, self.current_line);
                        }
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
                    // Push a placeholder for `this`. In derived constructors,
                    // `this` is in the TDZ until super() runs, so we cannot
                    // LoadEnv("this"). The CallSuperCtor opcode uses the
                    // frame's this_val directly.
                    self.chunk.emit(Op::Undefined, self.current_line); // [placeholder]
                    let superctor_idx = self.intern("#superctor");
                    self.chunk
                        .emit(Op::LoadEnv(superctor_idx), self.current_line); // [this, superCtor]
                                                                              // Check if all args are a single spread (super(...args))
                    let is_single_spread = args.len() == 1 && matches!(args[0], Expr::Spread(_));
                    if is_single_spread {
                        if let Expr::Spread(inner) = &args[0] {
                            self.compile_expr(inner)?;
                        }
                        self.chunk.emit(Op::CallSuperCtorSpread, self.current_line);
                        return Ok(());
                    }
                    let has_spread = args.iter().any(|a| matches!(a, Expr::Spread(_)));
                    if has_spread {
                        self.chunk.emit(Op::NewArray(0), self.current_line);
                        for a in args {
                            match a {
                                Expr::Spread(inner) => {
                                    self.compile_expr(inner)?;
                                    self.chunk.emit(Op::SpreadPush, self.current_line);
                                }
                                _ => {
                                    self.compile_expr(a)?;
                                    self.chunk.emit(Op::ArrayPush, self.current_line);
                                }
                            }
                        }
                        self.chunk.emit(Op::CallSuperCtorSpread, self.current_line);
                    } else {
                        for a in args {
                            self.compile_expr(a)?;
                        }
                        self.chunk
                            .emit(Op::CallSuperCtor(args.len()), self.current_line);
                    }
                    return Ok(());
                }
                match callee.as_ref() {
                    // `obj.#method(args)`: call a private method with this=obj.
                    Expr::PrivateGet { object, name } => {
                        self.compile_expr(object)?; // [obj]
                        for a in args {
                            if let Expr::Spread(_) = a {
                            } else {
                                self.compile_expr(a)?;
                            }
                        }
                        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                        self.chunk.emit(
                            Op::CallPrivateMethod(name_idx, args.len()),
                            self.current_line,
                        );
                        return Ok(());
                    }
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
                            self.chunk.emit(Op::GetProto, self.current_line);
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
                                    .add_constant(Value::String(Arc::from(key.as_str())));
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
                            // Computed key: compile the expression so its
                            // value is on the stack as the property key.
                            self.compile_expr(property)?;
                            String::new()
                        };
                        // For computed keys, the key expression is already on
                        // the stack; for non-computed, push the key constant.
                        if *computed {
                            // key is on the stack from compile_expr above
                        } else {
                            let key_idx = self
                                .chunk
                                .add_constant(Value::String(Arc::from(key.as_str())));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                        }
                        if *call_opt {
                            // `a?.b?.()`: keep the optional-call path, which
                            // short-circuits if the method value is nullish.
                            for a in args {
                                if let Expr::Spread(_) = a {
                                } else {
                                    self.compile_expr(a)?;
                                }
                            }
                            self.chunk
                                .emit(Op::CallMethodOpt(args.len()), self.current_line);
                        } else {
                            // Ordinary member calls resolve the property
                            // before evaluating arguments. Callability is
                            // still checked by CallThis after args run.
                            let has_spread = args.iter().any(|a| matches!(a, Expr::Spread(_)));
                            self.chunk.emit(Op::GetMethodForCall, self.current_line);
                            if has_spread {
                                self.chunk.emit(Op::NewArray(0), self.current_line);
                                for a in args {
                                    match a {
                                        Expr::Spread(inner) => {
                                            self.compile_expr(inner)?;
                                            self.chunk.emit(Op::SpreadPush, self.current_line);
                                        }
                                        _ => {
                                            self.compile_expr(a)?;
                                            self.chunk.emit(Op::ArrayPush, self.current_line);
                                        }
                                    }
                                }
                                self.chunk.emit(Op::CallThisSpread, self.current_line);
                            } else {
                                for a in args {
                                    self.compile_expr(a)?;
                                }
                                self.chunk.emit(Op::CallThis(args.len()), self.current_line);
                            }
                        }
                        if *m_opt {
                            let end = self.chunk.code.len();
                            self.chunk.patch_jump(jend, end);
                        }
                    }
                    _ => {
                        // If any argument is a spread, build an args array and use CallSpread.
                        let has_spread = args.iter().any(|a| matches!(a, Expr::Spread(_)));
                        let is_eval_call = !*call_opt
                            && matches!(callee.as_ref(), Expr::Ident(name) if &**name == "eval");
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
                            if is_eval_call {
                                self.chunk.emit(Op::CallEvalSpread, self.current_line);
                            } else {
                                self.chunk.emit(Op::CallSpread, self.current_line);
                                // pops argsArr then callee
                            }
                        } else {
                            for a in args {
                                if let Expr::Spread(_) = a {
                                } else {
                                    self.compile_expr(a)?;
                                }
                            }
                            if is_eval_call {
                                self.chunk.emit(Op::CallEval(args.len()), self.current_line);
                            } else {
                                self.chunk.emit(Op::Call(args.len()), self.current_line);
                            }
                        }
                        if *call_opt {
                            let end = self.chunk.code.len();
                            self.chunk.patch_jump(jend, end);
                        }
                    }
                }
            }
            Expr::NewTarget => {
                self.chunk.emit(Op::NewTarget, self.current_line);
            }
            Expr::New { callee, args } => {
                self.compile_expr(callee)?; // [ctor]
                let has_spread = args.iter().any(|a| matches!(a, Expr::Spread(_)));
                if has_spread {
                    self.chunk.emit(Op::NewArray(0), self.current_line); // [ctor, argsArr]
                    for a in args {
                        match a {
                            Expr::Spread(inner) => {
                                self.compile_expr(inner)?;
                                self.chunk.emit(Op::SpreadPush, self.current_line);
                            }
                            _ => {
                                self.compile_expr(a)?;
                                self.chunk.emit(Op::ArrayPush, self.current_line);
                            }
                        }
                    }
                    self.chunk.emit(Op::NewSpread, self.current_line);
                } else {
                    for a in args {
                        self.compile_expr(a)?;
                    }
                    self.chunk.emit(Op::New(args.len()), self.current_line);
                }
            }
            Expr::Member {
                object,
                property,
                computed,
                optional,
            } => {
                if matches!(object.as_ref(), Expr::Super) {
                    let this_idx = self.intern("this");
                    self.chunk.emit(Op::LoadEnv(this_idx), self.current_line);
                    let super_idx = self.intern("#super");
                    self.chunk.emit(Op::LoadEnv(super_idx), self.current_line);
                    self.chunk.emit(Op::GetProto, self.current_line);
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
                            .add_constant(Value::String(Arc::from(key.as_str())));
                        self.chunk.emit(Op::Const(key_idx), self.current_line);
                    }
                    self.chunk.emit(Op::GetSuperProp, self.current_line);
                    return Ok(());
                }
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
                        .add_constant(Value::String(Arc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line);
                }
                if *optional {
                    let end = self.chunk.code.len();
                    self.chunk.patch_jump(jend, end);
                }
            }
            Expr::TaggedTemplate {
                tag,
                quasis,
                raw,
                exprs,
            } => {
                // tag`q0${e0}q1` => tag(strings, e0, ...).
                // The VM builds a cached, frozen template object with a frozen
                // `raw` property per GetTemplateObject.
                let tag_is_member = if let Expr::Member {
                    object,
                    property,
                    computed,
                    optional: _,
                } = tag.as_ref()
                {
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
                            .add_constant(Value::String(Arc::from(key.as_str())));
                        self.chunk.emit(Op::Const(key_idx), self.current_line); // [obj, key]
                    }
                    self.chunk.emit(Op::GetMethodForCall, self.current_line); // [obj, tag]
                    true
                } else {
                    self.compile_expr(tag)?; // [tag]
                    false
                };
                let quasi_ids: Vec<usize> = quasis
                    .iter()
                    .map(|q| match q {
                        Some(s) => self.chunk.add_constant(Value::String(s.clone())),
                        None => self.chunk.add_constant(Value::Undefined),
                    })
                    .collect();
                let raw_ids: Vec<usize> = raw
                    .iter()
                    .map(|r| self.chunk.add_constant(Value::String(r.clone())))
                    .collect();
                self.chunk
                    .emit(Op::GetTemplateObject(quasi_ids, raw_ids), self.current_line);
                // Interpolated expressions as additional arguments.
                for e in exprs {
                    self.compile_expr(e)?;
                }
                if tag_is_member {
                    self.chunk
                        .emit(Op::CallThis(1 + exprs.len()), self.current_line);
                } else {
                    self.chunk
                        .emit(Op::Call(1 + exprs.len()), self.current_line);
                }
            }
            Expr::Regex(pattern, flags) => {
                // Compile to `new RegExp(pattern, flags)`.
                let name_idx = self.chunk.add_constant(Value::String(Arc::from("RegExp")));
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
                    chunk: Arc::new(func_chunk),
                    num_locals: f.params.len() + 16,
                    is_arrow: f.is_arrow,
                    is_async: f.is_async,
                    is_generator: f.is_generator,
                    has_parameter_expressions: Self::has_parameter_expressions(f),
                    length: Self::fn_length(f),
                    is_method: f.is_method,
                    has_name_binding: f.has_name_binding,
                    is_derived: false,
                };
                self.funcs.push(Arc::new(fdef));
                self.chunk
                    .emit(Op::MakeClosure(func_idx), self.current_line);
            }
            Expr::Class(cls) => {
                let explicit_class_name = cls.name.as_ref();
                let display_name = cls.name.as_ref().or(cls.inferred_name.as_ref()).cloned();
                self.chunk.emit(Op::PushScope, self.current_line);
                self.push_scope_with_runtime(false, true);
                if let Some(name) = explicit_class_name {
                    self.declare(name, VarKind::Const)?;
                    let name_idx = self.intern(name);
                    self.chunk
                        .emit(Op::DeclareConstUninit(name_idx), self.current_line);
                }
                // Build a constructor function from the class.
                // Methods become prototype properties (or static on the constructor).
                let has_ctor = cls.methods.iter().any(|m| m.is_constructor);
                // For derived classes without an explicit constructor, synthesize
                // one that forwards all arguments via super(...rest).
                let rest_name: Arc<str> = Arc::from("#rest");
                let _synthetic_params: Vec<Arc<str>> = if cls.superclass.is_some() && !has_ctor {
                    vec![rest_name.clone()]
                } else {
                    Vec::new()
                };
                let ctor_fn = FunctionExpr {
                    name: display_name.clone(),
                    params: cls
                        .methods
                        .iter()
                        .find(|m| m.is_constructor)
                        .map(|m| m.params.clone())
                        .or_else(|| {
                            if cls.superclass.is_some() {
                                // No positional params; only a rest param.
                                Some(vec![])
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
                        .and_then(|m| m.rest_param.clone())
                        .or_else(|| {
                            if cls.superclass.is_some() && !has_ctor {
                                Some(rest_name.clone())
                            } else {
                                None
                            }
                        }),
                    body: {
                        let body = cls
                            .methods
                            .iter()
                            .find(|m| m.is_constructor)
                            .map(|m| m.body.clone())
                            .or_else(|| {
                                if cls.superclass.is_some() {
                                    // super(...#rest)
                                    let args: Vec<Expr> = vec![Expr::Spread(Box::new(
                                        Expr::Ident(rest_name.clone()),
                                    ))];
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
                            .unwrap_or_default();
                        // Prepend private field initializers.
                        // Private methods are stored as private fields whose
                        // value is a function expression, so `this.#m()`
                        // resolves via PrivateGet like a field.
                        let pm_fields: Vec<crate::ast::PrivateFieldDecl> = cls
                            .methods
                            .iter()
                            .filter(|m| m.is_private && !m.is_static)
                            .filter(|m| matches!(m.kind, crate::ast::PropKind::Method))
                            .map(|m| crate::ast::PrivateFieldDecl {
                                name: m.name.clone(),
                                init: Some(Box::new(Expr::Function(FunctionExpr {
                                    name: Some(m.name.clone()),
                                    params: m.params.clone(),
                                    param_defaults: m.param_defaults.clone(),
                                    rest_param: m.rest_param.clone(),
                                    body: m.body.clone(),
                                    is_arrow: false,
                                    is_async: m.is_async,
                                    is_generator: m.is_generator,
                                    param_decls: Vec::new(),
                                    is_strict: true,
                                    is_method: false,
                                    has_name_binding: false,
                                }))),
                                is_static: false,
                                kind: crate::ast::PropKind::Method,
                            })
                            .collect();
                        let private_accessors: Vec<Stmt> = cls
                            .methods
                            .iter()
                            .filter(|m| m.is_private && !m.is_static)
                            .filter(|m| {
                                matches!(
                                    m.kind,
                                    crate::ast::PropKind::Get | crate::ast::PropKind::Set
                                )
                            })
                            .map(|m| {
                                let fn_expr = Expr::Function(FunctionExpr {
                                    name: Some(m.name.clone()),
                                    params: m.params.clone(),
                                    param_defaults: m.param_defaults.clone(),
                                    rest_param: m.rest_param.clone(),
                                    body: m.body.clone(),
                                    is_arrow: false,
                                    is_async: m.is_async,
                                    is_generator: false,
                                    param_decls: Vec::new(),
                                    is_strict: true,
                                    is_method: false,
                                    has_name_binding: false,
                                });
                                Stmt {
                                    line: 0,
                                    node: StmtNode::ExprStmt(Expr::PrivateDefineAccessor {
                                        object: Box::new(Expr::This),
                                        name: m.name.clone(),
                                        get: if matches!(m.kind, crate::ast::PropKind::Get) {
                                            Some(Box::new(fn_expr.clone()))
                                        } else {
                                            None
                                        },
                                        set: if matches!(m.kind, crate::ast::PropKind::Set) {
                                            Some(Box::new(fn_expr))
                                        } else {
                                            None
                                        },
                                    }),
                                }
                            })
                            .collect();
                        let pf_stmts: Vec<Stmt> = cls
                            .private_fields
                            .iter()
                            .filter(|pf| !pf.is_static)
                            .chain(pm_fields.iter())
                            .map(|pf| {
                                let init =
                                    pf.init.clone().unwrap_or_else(|| Box::new(Expr::Undefined));
                                Stmt {
                                    line: 0,
                                    node: StmtNode::ExprStmt(Expr::PrivateSet {
                                        object: Box::new(Expr::This),
                                        name: pf.name.clone(),
                                        value: init,
                                    }),
                                }
                            })
                            .collect();
                        let mut init_stmts = pf_stmts;
                        init_stmts.extend(private_accessors);
                        if cls.superclass.is_some() {
                            let mut combined = Vec::new();
                            let mut inserted = false;
                            for stmt in body {
                                let is_super_stmt = matches!(
                                    &stmt.node,
                                    StmtNode::ExprStmt(Expr::Call { callee, .. })
                                        if matches!(callee.as_ref(), Expr::Super)
                                );
                                combined.push(stmt);
                                if is_super_stmt && !inserted {
                                    combined.extend(init_stmts.clone());
                                    inserted = true;
                                }
                            }
                            if !inserted {
                                combined.extend(init_stmts);
                            }
                            combined
                        } else {
                            let mut combined = init_stmts;
                            combined.extend(body);
                            combined
                        }
                    },
                    is_arrow: false,
                    is_async: false,
                    is_generator: false,
                    param_decls: Vec::new(),
                    is_strict: true, // classes are always strict
                    is_method: true,
                    has_name_binding: false,
                };
                let (func_chunk, param_slots) = self.compile_function(&ctor_fn)?;
                let func_idx = self.funcs.len();
                let fdef = crate::function::FunctionDef {
                    name: display_name,
                    params: ctor_fn.params.clone(),
                    param_slots,
                    rest_param: ctor_fn.rest_param.clone(),
                    chunk: Arc::new(func_chunk),
                    num_locals: ctor_fn.params.len() + 16,
                    is_arrow: false,
                    is_async: false,
                    is_generator: false,
                    has_parameter_expressions: Self::has_parameter_expressions(&ctor_fn),
                    length: Self::fn_length(&ctor_fn),
                    is_method: false,
                    has_name_binding: false,
                    is_derived: cls.superclass.is_some(),
                };
                self.funcs.push(Arc::new(fdef));
                self.chunk.emit(Op::MakeClass(func_idx), self.current_line);
                // If there is a superclass, evaluate it and wire up the prototype chain.
                // The class evaluation scope is captured by the constructor, so
                // its `#super` binding is the instance HomeObject:
                // `childCtor.prototype`. Static elements shadow this with
                // `childCtor` when their closures are made. Super property
                // evaluation reads the HomeObject prototype dynamically.
                if let Some(super_expr) = &cls.superclass {
                    // stack: [ctor]
                    self.compile_expr(super_expr)?;
                    // Validate the superclass is a constructor with valid prototype.
                    self.chunk.emit(Op::ValidateExtends, self.current_line);
                    // stack: [ctor, parentCtor, parentProto]
                    // `ValidateExtends` performs the single spec [[Get]] of
                    // parentCtor.prototype, so superclass prototype getters
                    // are not invoked twice during class definition.
                    let super_proto_idx = self.intern("#super_proto");
                    self.chunk
                        .emit(Op::DeclareEnv(super_proto_idx), self.current_line);
                    // stack: [ctor, parentCtor]
                    // Keep the evaluated superclass only for class wiring
                    // below. Runtime super() must not close over it.
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, parentCtor, parentCtor]
                    let super_parent_ctor_idx = self.intern("#super_parent_ctor");
                    self.chunk
                        .emit(Op::DeclareEnv(super_parent_ctor_idx), self.current_line); // [ctor, parentCtor]

                    // Bind child ctor as `#superctor`. SuperCall reads this
                    // function's [[Prototype]] dynamically, so later
                    // Object.setPrototypeOf(C, ...) changes are visible.
                    self.chunk.emit(Op::Swap, self.current_line); // [parentCtor, ctor]
                    self.chunk.emit(Op::Dup, self.current_line); // [parentCtor, ctor, ctor]
                    let superctor_idx = self.intern("#superctor");
                    self.chunk
                        .emit(Op::DeclareEnv(superctor_idx), self.current_line); // [parentCtor, ctor]
                    self.chunk.emit(Op::Swap, self.current_line); // [ctor, parentCtor]
                    self.chunk.emit(Op::Pop, self.current_line); // [ctor]

                    // Set childCtor.prototype.__proto__ = parentProto (link prototype chain).
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    let cp_key = self
                        .chunk
                        .add_constant(Value::String(Arc::from("prototype")));
                    self.chunk.emit(Op::Const(cp_key), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line); // [ctor, childProto]
                    self.chunk
                        .emit(Op::LoadEnv(super_proto_idx), self.current_line); // [ctor, childProto, parentProto]
                    self.chunk.emit(Op::SetProto, self.current_line); // pop parentProto,childProto; set childProto.__proto__
                                                                      // stack: [ctor]
                                                                      // Also link the constructors: childCtor.__proto__ = parentCtor (static inheritance).
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    self.chunk
                        .emit(Op::LoadEnv(super_parent_ctor_idx), self.current_line); // [ctor, ctor, parentCtor]
                    self.chunk.emit(Op::SetProto, self.current_line); // set ctor.__proto__ = parentCtor
                                                                      // stack: [ctor]

                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    let cp_key = self
                        .chunk
                        .add_constant(Value::String(Arc::from("prototype")));
                    self.chunk.emit(Op::Const(cp_key), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line); // [ctor, childProto]
                    let super_name_idx = self.intern("#super");
                    self.chunk
                        .emit(Op::DeclareEnv(super_name_idx), self.current_line);
                    // stack: [ctor]
                } else {
                    // Base class constructors and instance methods use
                    // Class.prototype as their HomeObject.
                    let super_name_idx = self.intern("#super");
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    let cp_key = self
                        .chunk
                        .add_constant(Value::String(Arc::from("prototype")));
                    self.chunk.emit(Op::Const(cp_key), self.current_line);
                    self.chunk.emit(Op::GetProp, self.current_line); // [ctor, childProto]
                    self.chunk
                        .emit(Op::DeclareEnv(super_name_idx), self.current_line);
                }
                // assign each non-constructor method to prototype (or constructor if static)
                for method in &cls.methods {
                    if method.is_constructor {
                        continue;
                    }
                    // Private methods/accessors are installed into private
                    // slots, not as public properties.
                    if method.is_private {
                        continue;
                    }
                    let m_fn = FunctionExpr {
                        name: Some(method.name.clone()),
                        params: method.params.clone(),
                        param_defaults: method.param_defaults.clone(),
                        rest_param: method.rest_param.clone(),
                        body: method.body.clone(),
                        is_arrow: false,
                        is_async: method.is_async,
                        is_generator: method.is_generator,
                        param_decls: Vec::new(),
                        is_strict: true, // class methods are always strict
                        is_method: true,
                        has_name_binding: false,
                    };
                    let (m_chunk, m_slots) = self.compile_function(&m_fn)?;
                    let m_idx = self.funcs.len();
                    let mdef = crate::function::FunctionDef {
                        name: Some(method.name.clone()),
                        params: method.params.clone(),
                        param_slots: m_slots,
                        rest_param: method.rest_param.clone(),
                        chunk: Arc::new(m_chunk),
                        num_locals: method.params.len() + 16,
                        is_arrow: false,
                        is_async: method.is_async,
                        is_generator: method.is_generator,
                        has_parameter_expressions: Self::has_parameter_expressions(&m_fn),
                        length: Self::fn_length(&m_fn),
                        is_method: true,
                        has_name_binding: false,
                        is_derived: false,
                    };
                    self.funcs.push(Arc::new(mdef));
                    let is_accessor = matches!(
                        method.kind,
                        crate::ast::PropKind::Get | crate::ast::PropKind::Set
                    );
                    let akind = if matches!(method.kind, crate::ast::PropKind::Get) {
                        0u8
                    } else {
                        1u8
                    };
                    // Each branch leaves [ctor] on the stack.
                    if is_accessor {
                        if method.is_static {
                            // [ctor] -> define accessor on ctor
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor, ctor]
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor, ctor, ctor]
                            let static_super_idx = self.intern("#static_super");
                            self.chunk
                                .emit(Op::DeclareEnv(static_super_idx), self.current_line);
                            if let Some(ce) = &method.computed_name {
                                self.compile_expr(ce)?;
                                self.chunk.emit(Op::ToString, self.current_line);
                            } else {
                                let key_idx =
                                    self.chunk.add_constant(Value::String(method.name.clone()));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                            }
                            self.chunk
                                .emit(Op::LoadEnv(static_super_idx), self.current_line);
                            self.emit_make_closure_capturing_super_from_stack(m_idx);
                            self.chunk
                                .emit(Op::DefineClassAccessor(akind), self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line); // [ctor, ctor]
                            self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                        } else {
                            // [ctor] -> get proto, define accessor on proto
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                            let proto_key = self
                                .chunk
                                .add_constant(Value::String(Arc::from("prototype")));
                            self.chunk.emit(Op::Const(proto_key), self.current_line);
                            self.chunk.emit(Op::GetProp, self.current_line); // [ctor, proto]
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, proto, proto]
                            let instance_super_idx = self.intern("#instance_super");
                            self.chunk
                                .emit(Op::DeclareEnv(instance_super_idx), self.current_line);
                            self.chunk.emit(Op::Dup, self.current_line); // [ctor, proto, proto]
                            if let Some(ce) = &method.computed_name {
                                self.compile_expr(ce)?;
                                self.chunk.emit(Op::ToString, self.current_line);
                            } else {
                                let key_idx =
                                    self.chunk.add_constant(Value::String(method.name.clone()));
                                self.chunk.emit(Op::Const(key_idx), self.current_line);
                            }
                            self.chunk
                                .emit(Op::LoadEnv(instance_super_idx), self.current_line);
                            self.emit_make_closure_capturing_super_from_stack(m_idx);
                            self.chunk
                                .emit(Op::DefineClassAccessor(akind), self.current_line);
                            self.chunk.emit(Op::Pop, self.current_line); // [ctor, proto]
                            self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                        }
                    } else if method.is_static {
                        // Constructor.method = fn (non-enumerable)
                        self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                        self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor, ctor]
                        let static_super_idx = self.intern("#static_super");
                        self.chunk
                            .emit(Op::DeclareEnv(static_super_idx), self.current_line);
                        if let Some(ce) = &method.computed_name {
                            self.compile_expr(ce)?;
                            self.chunk.emit(Op::ToString, self.current_line);
                        } else {
                            let key_idx =
                                self.chunk.add_constant(Value::String(method.name.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                        }
                        self.chunk
                            .emit(Op::LoadEnv(static_super_idx), self.current_line);
                        self.emit_make_closure_capturing_super_from_stack(m_idx);
                        self.chunk.emit(Op::DefineMethod, self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                    } else {
                        // Constructor.prototype.method = fn (non-enumerable)
                        self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                        let proto_key = self
                            .chunk
                            .add_constant(Value::String(Arc::from("prototype")));
                        self.chunk.emit(Op::Const(proto_key), self.current_line);
                        self.chunk.emit(Op::GetProp, self.current_line); // [ctor, proto]
                        self.chunk.emit(Op::Dup, self.current_line); // [ctor, proto, proto]
                        let instance_super_idx = self.intern("#instance_super");
                        self.chunk
                            .emit(Op::DeclareEnv(instance_super_idx), self.current_line);
                        if let Some(ce) = &method.computed_name {
                            self.compile_expr(ce)?;
                            self.chunk.emit(Op::ToString, self.current_line);
                        } else {
                            let key_idx =
                                self.chunk.add_constant(Value::String(method.name.clone()));
                            self.chunk.emit(Op::Const(key_idx), self.current_line);
                        }
                        self.chunk
                            .emit(Op::LoadEnv(instance_super_idx), self.current_line);
                        self.emit_make_closure_capturing_super_from_stack(m_idx);
                        self.chunk.emit(Op::DefineMethod, self.current_line);
                        self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                    }
                }
                // Initialize the explicit class-name binding captured by the
                // constructor, methods, and static blocks (but keep ctor on stack).
                if let Some(name) = &cls.name {
                    let name_idx = self.intern(name);
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    self.chunk
                        .emit(Op::InitEnvConst(name_idx), self.current_line); // [ctor]
                }
                for pf in cls.private_fields.iter().filter(|pf| pf.is_static) {
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    let init = pf.init.clone().unwrap_or_else(|| Box::new(Expr::Undefined));
                    self.compile_expr(&init)?;
                    let name_idx = self.chunk.add_constant(Value::String(pf.name.clone()));
                    self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
                    self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                }
                for method in cls.methods.iter().filter(|m| m.is_private && m.is_static) {
                    let m_fn = Expr::Function(FunctionExpr {
                        name: Some(method.name.clone()),
                        params: method.params.clone(),
                        param_defaults: method.param_defaults.clone(),
                        rest_param: method.rest_param.clone(),
                        body: method.body.clone(),
                        is_arrow: false,
                        is_async: false,
                        is_generator: method.is_generator,
                        param_decls: Vec::new(),
                        is_strict: true,
                        is_method: false,
                        has_name_binding: false,
                    });
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    match method.kind {
                        crate::ast::PropKind::Get => {
                            self.compile_expr(&m_fn)?;
                            self.chunk.emit(Op::Undefined, self.current_line);
                            let name_idx =
                                self.chunk.add_constant(Value::String(method.name.clone()));
                            self.chunk
                                .emit(Op::DefinePrivateAccessor(name_idx), self.current_line);
                        }
                        crate::ast::PropKind::Set => {
                            self.chunk.emit(Op::Undefined, self.current_line);
                            self.compile_expr(&m_fn)?;
                            let name_idx =
                                self.chunk.add_constant(Value::String(method.name.clone()));
                            self.chunk
                                .emit(Op::DefinePrivateAccessor(name_idx), self.current_line);
                        }
                        _ => {
                            self.compile_expr(&m_fn)?;
                            let name_idx =
                                self.chunk.add_constant(Value::String(method.name.clone()));
                            self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
                        }
                    }
                    self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                }
                // Static initialization blocks: each runs with `this` = the
                // class (constructor), in source order. We bind `this` in a
                // temp env so the block body sees it, then compile inline.
                // Static initialization blocks: compile each as a separate
                // function and call it with this=ctor via CallThis.
                for block in &cls.static_blocks {
                    let sb_fn = FunctionExpr {
                        name: None,
                        params: Vec::new(),
                        param_defaults: Vec::new(),
                        rest_param: None,
                        body: block.clone(),
                        is_arrow: false,
                        is_async: false,
                        is_generator: false,
                        param_decls: Vec::new(),
                        is_strict: true,
                        is_method: false,
                        has_name_binding: false,
                    };
                    let (sb_chunk, sb_slots) = self.compile_function(&sb_fn)?;
                    let sb_idx = self.funcs.len();
                    let sbdef = crate::function::FunctionDef {
                        name: None,
                        params: Vec::new(),
                        param_slots: sb_slots,
                        rest_param: None,
                        chunk: Arc::new(sb_chunk),
                        num_locals: 16,
                        is_arrow: false,
                        is_async: false,
                        is_generator: false,
                        has_parameter_expressions: false,
                        length: 0,
                        is_method: false,
                        has_name_binding: false,
                        is_derived: false,
                    };
                    self.funcs.push(Arc::new(sbdef));
                    // stack: [ctor]. Dup ctor for `this`, then MakeClosure.
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                    self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor, ctor]
                    self.emit_make_closure_capturing_super_from_stack(sb_idx);
                    // CallThis expects [..., this, fn, args...]; here this=ctor
                    // (dup), fn on top.
                    self.chunk.emit(Op::CallThis(0), self.current_line); // [ctor, result]
                    self.chunk.emit(Op::Pop, self.current_line); // [ctor]
                }
                self.chunk.emit(Op::PopScope, self.current_line);
                self.pop_scope();
                if cls.is_declaration {
                    if let Some(name) = &cls.name {
                        let name_idx = self.intern(name);
                        self.chunk.emit(Op::Dup, self.current_line); // [ctor, ctor]
                        self.chunk.emit(Op::InitEnv(name_idx), self.current_line);
                        // [ctor]
                    }
                }
            }
            Expr::PrivateGet { object, name } => {
                self.compile_expr(object)?;
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::GetPrivate(name_idx), self.current_line);
            }
            Expr::PrivateSet {
                object,
                name,
                value,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(value)?;
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
            }
            Expr::PrivateDefineAccessor {
                object,
                name,
                get,
                set,
            } => {
                self.compile_expr(object)?;
                if let Some(get) = get {
                    self.compile_expr(get)?;
                } else {
                    self.chunk.emit(Op::Undefined, self.current_line);
                }
                if let Some(set) = set {
                    self.compile_expr(set)?;
                } else {
                    self.chunk.emit(Op::Undefined, self.current_line);
                }
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk
                    .emit(Op::DefinePrivateAccessor(name_idx), self.current_line);
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
            // Private field assignment: obj.#name = value
            Expr::PrivateGet { object, name } => {
                self.compile_expr(object)?;
                self.compile_expr(value)?;
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
            }
            // Destructuring assignment: `[a, b] = expr` / `{a, b} = expr`.
            Expr::Array(_) | Expr::Object(_) => {
                self.compile_expr(value)?;
                self.chunk.emit(Op::Dup, self.current_line);
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
                if matches!(object.as_ref(), Expr::Super) {
                    let this_idx = self.intern("this");
                    self.chunk.emit(Op::LoadEnv(this_idx), self.current_line);
                    let super_idx = self.intern("#super");
                    self.chunk.emit(Op::LoadEnv(super_idx), self.current_line);
                    self.chunk.emit(Op::GetProto, self.current_line);
                    if *computed {
                        self.compile_expr(property)?;
                    } else {
                        let key = if let Expr::String(s) = &**property {
                            s.to_string()
                        } else {
                            String::new()
                        };
                        let key_idx = self
                            .chunk
                            .add_constant(Value::String(Arc::from(key.as_str())));
                        self.chunk.emit(Op::Const(key_idx), self.current_line);
                    }
                    self.compile_expr(value)?;
                    self.chunk.emit(Op::SetSuperProp, self.current_line);
                    return Ok(());
                }
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
                        .add_constant(Value::String(Arc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                    self.compile_expr(value)?;
                    self.chunk.emit(Op::SetProp, self.current_line);
                }
            }
            Expr::Ident(name) => {
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
                self.compile_expr(value)?;
                self.chunk.emit(Op::Swap, self.current_line);
                self.chunk.emit(Op::PutValue, self.current_line);
            }
            _ => {
                self.compile_expr(value)?;
            }
        }
        Ok(())
    }

    fn compile_assign_target(&mut self, target: &Expr) -> error::Result<()> {
        match target {
            Expr::Ident(name) => self.store_identifier_target_value(name),
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
                        .add_constant(Value::String(Arc::from(key.as_str())));
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

    fn store_identifier_target_value(&mut self, name: &Arc<str>) {
        let name_idx = self.chunk.add_constant(Value::String(name.clone()));
        self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
        self.chunk.emit(Op::PutValue, self.current_line);
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
            Expr::PrivateGet { object, name } => {
                self.compile_expr(object)?;
                self.chunk.emit(Op::Dup, self.current_line);
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::GetPrivate(name_idx), self.current_line);
                self.compile_expr(value)?;
                self.chunk.emit(bin, 0);
                self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
            }
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                if matches!(object.as_ref(), Expr::Super) {
                    let target = self.compile_super_member_target_temps(property, *computed)?;
                    self.load_super_member_target_temps(target);
                    self.chunk.emit(Op::GetSuperProp, self.current_line);
                    self.compile_expr(value)?;
                    self.chunk.emit(bin, 0);

                    let result_idx = self.intern("#super_compound_result");
                    self.chunk
                        .emit(Op::DeclareEnv(result_idx), self.current_line);
                    self.load_super_member_target_temps(target);
                    self.chunk.emit(Op::LoadEnv(result_idx), self.current_line);
                    self.chunk.emit(Op::SetSuperProp, self.current_line);
                    return Ok(());
                }
                // Evaluate obj+key ONCE, then Dup2 so the same pair is
                // available for both the load and the store. This matches
                // the spec requirement that ToPropertyKey is called only
                // once for compound assignment.
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    // Per spec: ToObject(base) is called AFTER key evaluation
                    // but BEFORE ToPropertyKey. So: evaluate base, evaluate
                    // key, check base for null/undefined (ToObject), then
                    // ToPropertyKey (ToString).
                    self.chunk.emit(Op::CheckNullBase, self.current_line);
                    self.chunk.emit(Op::ToString, self.current_line);
                } else {
                    // Non-computed: check base for null/undefined after
                    // evaluating the key constant.
                    self.chunk.emit(Op::CheckNullBase, self.current_line);
                    let key = if let Expr::String(s) = property.as_ref() {
                        s.to_string()
                    } else {
                        String::new()
                    };
                    let key_idx = self
                        .chunk
                        .add_constant(Value::String(Arc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                }
                // stack: [obj, key]
                self.chunk.emit(Op::Dup2, self.current_line);
                // stack: [obj, key, obj, key]
                // Load current value.
                if *computed {
                    self.chunk.emit(Op::GetElem, self.current_line);
                } else {
                    self.chunk.emit(Op::GetProp, self.current_line);
                }
                // stack: [obj, key, currentValue]
                // Evaluate RHS and apply binary op.
                self.compile_expr(value)?;
                self.chunk.emit(bin, 0);
                // stack: [obj, key, result]
                // Store: SetProp/SetElem consumes [obj, key, value] and pushes value.
                if *computed {
                    self.chunk.emit(Op::SetElem, self.current_line);
                } else {
                    self.chunk.emit(Op::SetProp, self.current_line);
                }
            }
            Expr::Ident(name) => {
                // Spec-conforming compound assignment: evaluate the reference
                // ONCE, then GetValue, operate, and PutValue back into the
                // SAME reference (preserving the original binding even if it
                // was deleted between GetValue and PutValue, e.g. inside
                // `with` where a getter deletes the property).
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
                // stack: [ref]
                self.chunk.emit(Op::Dup, self.current_line);
                // stack: [ref, ref]
                self.chunk.emit(Op::GetValue, self.current_line);
                // stack: [ref, currentValue]
                self.compile_expr(value)?;
                // stack: [ref, currentValue, rhs]
                self.chunk.emit(bin, 0);
                // stack: [ref, result]
                self.chunk.emit(Op::Swap, self.current_line);
                // stack: [result, ref]
                self.chunk.emit(Op::PutValue, self.current_line);
                // stack: [result]
            }
            _ => {
                self.compile_expr(target)?;
                self.compile_expr(value)?;
                self.chunk.emit(bin, 0);
                self.chunk.emit(Op::Dup, self.current_line);
                self.compile_assign_target(target)?;
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
            Expr::PrivateGet { object, name } => {
                self.compile_expr(object)?;
                self.chunk.emit(Op::Dup, self.current_line);
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::GetPrivate(name_idx), self.current_line);
                self.chunk.emit(Op::Dup, self.current_line);
                let cond_jump = match op {
                    AssignOp::AndAssign => Op::JumpIfFalse(0),
                    AssignOp::OrAssign => Op::JumpIfTrue(0),
                    AssignOp::NullishAssign => Op::JumpIfNotNullish(0),
                    _ => unreachable!(),
                };
                let jskip = self.chunk.code.len();
                self.chunk.emit(cond_jump, 0);
                // Assignment path: drop old value, keep object, evaluate RHS,
                // and store through the same private reference base.
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(value)?;
                self.chunk.emit(Op::SetPrivate(name_idx), self.current_line);
                let jend = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                self.chunk.patch_jump(jskip, self.chunk.code.len());
                // Short-circuit path leaves [obj, currentValue]. Drop object
                // and keep currentValue as the expression result.
                self.chunk.emit(Op::Swap, self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.chunk.patch_jump(jend, self.chunk.code.len());
            }
            Expr::Member {
                object,
                property,
                computed,
                ..
            } => {
                // Evaluate obj+key once, Dup2 for reuse in the store.
                self.compile_expr(object)?;
                if *computed {
                    self.compile_expr(property)?;
                    // ToObject(base) happens after evaluating the property
                    // expression but before ToPropertyKey.
                    self.chunk.emit(Op::CheckNullBase, self.current_line);
                    // Convert the key to a property key string ONCE, so that
                    // ToPropertyKey (and thus toString) is called only once
                    // per spec. Both GetElem and SetElem use this string.
                    self.chunk.emit(Op::ToString, self.current_line);
                } else {
                    let key = if let Expr::String(s) = property.as_ref() {
                        s.to_string()
                    } else {
                        String::new()
                    };
                    let key_idx = self
                        .chunk
                        .add_constant(Value::String(Arc::from(key.as_str())));
                    self.chunk.emit(Op::Const(key_idx), self.current_line);
                }
                // stack: [obj, key]
                self.chunk.emit(Op::Dup2, self.current_line);
                // stack: [obj, key, obj, key]
                // Load current value.
                if *computed {
                    self.chunk.emit(Op::GetElem, self.current_line);
                } else {
                    self.chunk.emit(Op::GetProp, self.current_line);
                }
                // stack: [obj, key, currentValue]
                self.chunk.emit(Op::Dup, self.current_line);
                // stack: [obj, key, currentValue, currentValue]
                let (cond_jump, fires_when) = match op {
                    AssignOp::AndAssign => (Op::JumpIfFalse(0), "falsy"),
                    AssignOp::OrAssign => (Op::JumpIfTrue(0), "truthy"),
                    AssignOp::NullishAssign => (Op::JumpIfNotNullish(0), "not-nullish"),
                    _ => unreachable!(),
                };
                let _ = fires_when;
                let jskip = self.chunk.code.len();
                self.chunk.emit(cond_jump, 0);
                // Assignment path: drop the old value (keep obj+key),
                // evaluate the RHS, and store it.
                self.chunk.emit(Op::Pop, self.current_line);
                // stack: [obj, key]
                self.compile_expr(value)?;
                // stack: [obj, key, result]
                if *computed {
                    self.chunk.emit(Op::SetElem, self.current_line);
                } else {
                    self.chunk.emit(Op::SetProp, self.current_line);
                }
                let jend = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                self.chunk.patch_jump(jskip, self.chunk.code.len());
                // Short-circuit path leaves [obj, key, currentValue]. Drop the
                // saved target pair so the assignment expression yields the
                // existing value.
                self.chunk.emit(Op::Rot3, self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.chunk.emit(Op::Swap, self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.chunk.patch_jump(jend, self.chunk.code.len());
            }
            Expr::Ident(name) => {
                // Identifier logical assignment must preserve the same
                // Reference from GetValue through PutValue. Re-resolving the
                // identifier after the RHS is wrong when a with/global object
                // property is deleted by the RHS.
                let name_idx = self.chunk.add_constant(Value::String(name.clone()));
                self.chunk.emit(Op::LoadRef(name_idx), self.current_line);
                self.chunk.emit(Op::Dup, self.current_line);
                self.chunk.emit(Op::GetValue, self.current_line);
                self.chunk.emit(Op::Dup, self.current_line);
                let cond_jump = match op {
                    AssignOp::AndAssign => Op::JumpIfFalse(0),
                    AssignOp::OrAssign => Op::JumpIfTrue(0),
                    AssignOp::NullishAssign => Op::JumpIfNotNullish(0),
                    _ => unreachable!(),
                };
                let jskip = self.chunk.code.len();
                self.chunk.emit(cond_jump, 0);
                // Assignment path: drop old value, evaluate RHS, and put it
                // through the original Reference.
                self.chunk.emit(Op::Pop, self.current_line);
                self.compile_expr(value)?;
                self.chunk.emit(Op::Swap, self.current_line);
                self.chunk.emit(Op::PutValue, self.current_line);
                let jend = self.chunk.code.len();
                self.chunk.emit(Op::Jump(0), self.current_line);
                self.chunk.patch_jump(jskip, self.chunk.code.len());
                // Short-circuit path leaves [ref, currentValue]. Drop the ref
                // and keep currentValue as the expression result.
                self.chunk.emit(Op::Swap, self.current_line);
                self.chunk.emit(Op::Pop, self.current_line);
                self.chunk.patch_jump(jend, self.chunk.code.len());
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

/// Recursively collect `var` and function-declaration names from a
/// statement tree. `var` is function-scoped, so declarations inside
/// blocks, loops, if/else, switch, and try/catch must all be hoisted.
fn collect_var_names_recursive(node: &StmtNode, out: &mut Vec<Arc<str>>) {
    match node {
        StmtNode::VarDecl { kind, decls } if *kind == VarKind::Var => {
            for (name, _) in decls {
                out.push(name.clone());
                // Skip duplicate names to avoid double-hoisting.
            }
        }
        // Function declarations are also collected here so eval leak-back
        // knows about them. They're hoisted separately by a dedicated pass
        // that runs before var hoisting, and DeclareVar doesn't overwrite
        // existing bindings (declare_var checks already_exists).
        StmtNode::FunctionDecl(f) => {
            if let Some(name) = &f.name {
                out.push(name.clone());
            }
        }
        StmtNode::Block(body) => {
            for s in body {
                collect_var_names_recursive(&s.node, out);
            }
        }
        StmtNode::If { then, else_, .. } => {
            collect_var_names_recursive(&then.node, out);
            if let Some(e) = else_ {
                collect_var_names_recursive(&e.node, out);
            }
        }
        StmtNode::While { body, .. } => collect_var_names_recursive(&body.node, out),
        StmtNode::DoWhile { body, .. } => collect_var_names_recursive(&body.node, out),
        StmtNode::For { init, body, .. } => {
            if let Some(init) = init {
                collect_var_names_recursive(&init.node, out);
            }
            collect_var_names_recursive(&body.node, out);
        }
        StmtNode::ForIn { left, body, .. } | StmtNode::ForOf { left, body, .. } => {
            collect_var_names_recursive(&left.node, out);
            collect_var_names_recursive(&body.node, out);
        }
        StmtNode::With { body, .. } => collect_var_names_recursive(&body.node, out),
        StmtNode::Switch { cases, .. } => {
            for case in cases {
                for s in &case.body {
                    collect_var_names_recursive_skip_functions(&s.node, out);
                }
            }
        }
        StmtNode::TryCatch {
            try_body,
            catch_body,
            finally_body,
            ..
        } => {
            collect_var_names_recursive(&try_body.node, out);
            if let Some(cb) = catch_body {
                collect_var_names_recursive(&cb.node, out);
            }
            if let Some(fb) = finally_body {
                collect_var_names_recursive(&fb.node, out);
            }
        }
        StmtNode::Labeled(_, body) => collect_var_names_recursive(&body.node, out),
        _ => {}
    }
}

fn collect_var_names_recursive_skip_functions(node: &StmtNode, out: &mut Vec<Arc<str>>) {
    match node {
        StmtNode::VarDecl { kind, decls } if *kind == VarKind::Var => {
            for (name, _) in decls {
                out.push(name.clone());
            }
        }
        StmtNode::Destructure { kind, pattern, .. } if *kind == VarKind::Var => {
            Compiler::pattern_names(pattern, out);
        }
        StmtNode::Block(body) => {
            for s in body {
                collect_var_names_recursive_skip_functions(&s.node, out);
            }
        }
        StmtNode::If { then, else_, .. } => {
            collect_var_names_recursive_skip_functions(&then.node, out);
            if let Some(e) = else_ {
                collect_var_names_recursive_skip_functions(&e.node, out);
            }
        }
        StmtNode::While { body, .. } => collect_var_names_recursive_skip_functions(&body.node, out),
        StmtNode::DoWhile { body, .. } => {
            collect_var_names_recursive_skip_functions(&body.node, out);
        }
        StmtNode::For { init, body, .. } => {
            if let Some(init) = init {
                collect_var_names_recursive_skip_functions(&init.node, out);
            }
            collect_var_names_recursive_skip_functions(&body.node, out);
        }
        StmtNode::ForIn { left, body, .. } | StmtNode::ForOf { left, body, .. } => {
            collect_var_names_recursive_skip_functions(&left.node, out);
            collect_var_names_recursive_skip_functions(&body.node, out);
        }
        StmtNode::With { body, .. } => {
            collect_var_names_recursive_skip_functions(&body.node, out);
        }
        StmtNode::Switch { cases, .. } => {
            for case in cases {
                for s in &case.body {
                    collect_var_names_recursive_skip_functions(&s.node, out);
                }
            }
        }
        StmtNode::TryCatch {
            try_body,
            catch_body,
            finally_body,
            ..
        } => {
            collect_var_names_recursive_skip_functions(&try_body.node, out);
            if let Some(cb) = catch_body {
                collect_var_names_recursive_skip_functions(&cb.node, out);
            }
            if let Some(fb) = finally_body {
                collect_var_names_recursive_skip_functions(&fb.node, out);
            }
        }
        StmtNode::Labeled(_, body) => collect_var_names_recursive_skip_functions(&body.node, out),
        _ => {}
    }
}
