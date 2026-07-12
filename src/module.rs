use crate::ast::{ExportEntry, Program};
use crate::bytecode::Chunk;
use crate::error::{self, Error};
use crate::value::{GcIdx, Value};
use crate::{Compiler, Parser, Vm};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleStatus {
    Linked,
    Instantiating,
    Instantiated,
    Evaluating,
    Evaluated,
    Errored,
}

#[derive(Clone)]
pub(crate) struct ModuleRecord {
    pub(crate) program: Program,
    pub(crate) env: GcIdx,
    pub(crate) dependencies: Vec<PathBuf>,
    chunk: Option<Arc<Chunk>>,
    scc_id: usize,
    status: ModuleStatus,
    pub(crate) error: Option<Arc<Error>>,
}

fn resolve_specifier(referrer: &Path, specifier: &str) -> error::Result<PathBuf> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return Err(Error::syntax(format!(
            "Unsupported module specifier '{}': only relative paths are supported",
            specifier
        )));
    }
    let parent = referrer.parent().unwrap_or_else(|| Path::new("."));
    parent.join(specifier).canonicalize().map_err(|err| {
        Error::syntax(format!(
            "Cannot resolve module '{}' from '{}': {}",
            specifier,
            referrer.display(),
            err
        ))
    })
}

fn load_graph(
    vm: &mut Vm,
    path: PathBuf,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<()> {
    if graph.contains_key(&path) {
        return Ok(());
    }
    if let Some(cached) = vm.module_records.get(&path).cloned() {
        let dependencies = cached.dependencies.clone();
        graph.insert(path, cached);
        for dependency in dependencies {
            load_graph(vm, dependency, graph)?;
        }
        return Ok(());
    }
    let source = std::fs::read_to_string(&path).map_err(|err| {
        Error::syntax(format!("Cannot read module '{}': {}", path.display(), err))
    })?;
    let program = Parser::parse_module(&source)?;
    let env = crate::environment::new_env(&vm.heap, Some(vm.global), true)?;
    crate::environment::declare(
        &vm.heap,
        env,
        "this",
        Value::Undefined,
        crate::value::BindingKind::Const,
    );
    graph.insert(
        path.clone(),
        ModuleRecord {
            program: program.clone(),
            env,
            dependencies: Vec::new(),
            chunk: None,
            scc_id: usize::MAX,
            status: ModuleStatus::Linked,
            error: None,
        },
    );

    let mut dependencies = Vec::new();
    for request in &program.module_requests {
        let dependency = resolve_specifier(&path, &request.specifier)?;
        if !dependencies.contains(&dependency) {
            dependencies.push(dependency.clone());
        }
        load_graph(vm, dependency, graph)?;
    }
    graph
        .get_mut(&path)
        .expect("module record inserted before dependencies")
        .dependencies = dependencies;
    Ok(())
}

fn resolve_export(
    graph: &HashMap<PathBuf, ModuleRecord>,
    module: &Path,
    export_name: &str,
    seen: &mut HashSet<(PathBuf, Arc<str>)>,
) -> error::Result<(GcIdx, Arc<str>)> {
    resolve_export_optional(graph, module, export_name, seen)?.ok_or_else(|| {
        Error::syntax(format!(
            "Module '{}' does not provide an export named '{}'",
            module.display(),
            export_name
        ))
    })
}

fn resolve_export_optional(
    graph: &HashMap<PathBuf, ModuleRecord>,
    module: &Path,
    export_name: &str,
    seen: &mut HashSet<(PathBuf, Arc<str>)>,
) -> error::Result<Option<(GcIdx, Arc<str>)>> {
    let key = (module.to_path_buf(), Arc::from(export_name));
    if !seen.insert(key) {
        return Ok(None);
    }
    let record = graph
        .get(module)
        .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
    for entry in &record.program.export_entries {
        match entry {
            ExportEntry::Local {
                local_name,
                export_name: candidate,
            } if candidate.as_ref() == export_name => {
                return Ok(Some((record.env, local_name.clone())));
            }
            ExportEntry::ReExport {
                module_request,
                import_name,
                export_name: candidate,
            } if candidate.as_ref() == export_name => {
                let dependency = resolve_specifier(module, &module_request.specifier)?;
                return resolve_export_optional(graph, &dependency, import_name, seen);
            }
            _ => {}
        }
    }
    if export_name == "default" {
        return Ok(None);
    }
    let mut star_resolution: Option<(GcIdx, Arc<str>)> = None;
    for entry in &record.program.export_entries {
        if let ExportEntry::Star { module_request } = entry {
            let dependency = resolve_specifier(module, &module_request.specifier)?;
            if let Some(candidate) = resolve_export_optional(graph, &dependency, export_name, seen)?
            {
                if let Some(existing) = &star_resolution {
                    if existing != &candidate {
                        return Err(Error::syntax(format!(
                            "Ambiguous star export '{}' in module '{}'",
                            export_name,
                            module.display()
                        )));
                    }
                } else {
                    star_resolution = Some(candidate);
                }
            }
        }
    }
    Ok(star_resolution)
}

fn link_imports_with_vm(vm: &Vm, graph: &HashMap<PathBuf, ModuleRecord>) -> error::Result<()> {
    for (path, record) in graph {
        for export in &record.program.export_entries {
            if let ExportEntry::ReExport { export_name, .. } = export {
                resolve_export(graph, path, export_name, &mut HashSet::new())?;
            }
        }
        for import in &record.program.import_entries {
            let dependency = resolve_specifier(path, &import.module_request.specifier)?;
            let (target_env, target_name) =
                resolve_export(graph, &dependency, &import.import_name, &mut HashSet::new())?;
            crate::environment::declare_import(
                &vm.heap,
                record.env,
                &import.local_name,
                target_env,
                target_name,
            );
        }
    }
    Ok(())
}

fn assign_scc_ids(graph: &mut HashMap<PathBuf, ModuleRecord>) {
    struct Tarjan {
        next_index: usize,
        next_component: usize,
        indices: HashMap<PathBuf, usize>,
        lowlinks: HashMap<PathBuf, usize>,
        stack: Vec<PathBuf>,
        on_stack: HashSet<PathBuf>,
        components: HashMap<PathBuf, usize>,
    }

    fn visit(path: PathBuf, graph: &HashMap<PathBuf, ModuleRecord>, state: &mut Tarjan) {
        let index = state.next_index;
        state.next_index += 1;
        state.indices.insert(path.clone(), index);
        state.lowlinks.insert(path.clone(), index);
        state.stack.push(path.clone());
        state.on_stack.insert(path.clone());

        let dependencies = graph
            .get(&path)
            .map(|record| record.dependencies.clone())
            .unwrap_or_default();
        for dependency in dependencies {
            if !state.indices.contains_key(&dependency) {
                visit(dependency.clone(), graph, state);
                let dependency_low = state.lowlinks[&dependency];
                let low = state.lowlinks[&path].min(dependency_low);
                state.lowlinks.insert(path.clone(), low);
            } else if state.on_stack.contains(&dependency) {
                let dependency_index = state.indices[&dependency];
                let low = state.lowlinks[&path].min(dependency_index);
                state.lowlinks.insert(path.clone(), low);
            }
        }

        if state.lowlinks[&path] == state.indices[&path] {
            loop {
                let member = state.stack.pop().expect("SCC stack cannot be empty");
                state.on_stack.remove(&member);
                state
                    .components
                    .insert(member.clone(), state.next_component);
                if member == path {
                    break;
                }
            }
            state.next_component += 1;
        }
    }

    let mut state = Tarjan {
        next_index: 0,
        next_component: 0,
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        stack: Vec::new(),
        on_stack: HashSet::new(),
        components: HashMap::new(),
    };
    let mut paths: Vec<PathBuf> = graph.keys().cloned().collect();
    paths.sort();
    for path in paths {
        if !state.indices.contains_key(&path) {
            visit(path, graph, &mut state);
        }
    }
    for (path, component) in state.components {
        if let Some(record) = graph.get_mut(&path) {
            record.scc_id = component;
        }
    }
}

fn mark_scc_error(graph: &mut HashMap<PathBuf, ModuleRecord>, path: &Path, error: Arc<Error>) {
    let component = graph
        .get(path)
        .map(|record| record.scc_id)
        .unwrap_or(usize::MAX);
    for record in graph.values_mut() {
        if record.scc_id == component {
            record.status = ModuleStatus::Errored;
            record.error = Some(error.clone());
        }
    }
}

fn instantiate_module(
    vm: &mut Vm,
    path: &Path,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<()> {
    let (status, cached_error, dependencies) = {
        let record = graph
            .get(path)
            .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
        (
            record.status,
            record.error.clone(),
            record.dependencies.clone(),
        )
    };
    match status {
        ModuleStatus::Instantiating
        | ModuleStatus::Instantiated
        | ModuleStatus::Evaluating
        | ModuleStatus::Evaluated => return Ok(()),
        ModuleStatus::Errored => {
            return Err(cached_error.unwrap_or_else(|| Error::syntax("Module linking failed")));
        }
        ModuleStatus::Linked => {}
    }
    graph.get_mut(path).expect("module exists").status = ModuleStatus::Instantiating;
    for dependency in dependencies {
        instantiate_module(vm, &dependency, graph)?;
    }

    let (program, env) = {
        let record = graph.get(path).expect("module exists");
        (record.program.clone(), record.env)
    };
    let mut compiler = Compiler::new();
    let (chunk, funcs) = compiler.compile_program(&program)?;
    let chunk = Arc::new(vm.append_compiled_functions(chunk, funcs));
    if let Err(error) = vm.instantiate_module_chunk(chunk.clone(), env) {
        let record = graph.get_mut(path).expect("module exists");
        record.status = ModuleStatus::Errored;
        record.error = Some(error.clone());
        return Err(error);
    }
    let record = graph.get_mut(path).expect("module exists");
    record.chunk = Some(chunk);
    record.status = ModuleStatus::Instantiated;
    Ok(())
}

fn evaluate_module(
    vm: &mut Vm,
    path: &Path,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<Value> {
    let (status, cached_error) = {
        let record = graph
            .get(path)
            .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
        (record.status, record.error.clone())
    };
    match status {
        ModuleStatus::Evaluated => return Ok(Value::Undefined),
        ModuleStatus::Evaluating => return Ok(Value::Undefined),
        ModuleStatus::Errored => {
            return Err(cached_error.unwrap_or_else(|| Error::syntax("Module evaluation failed")));
        }
        ModuleStatus::Linked | ModuleStatus::Instantiating => {
            return Err(Error::internal("Module evaluated before instantiation"));
        }
        ModuleStatus::Instantiated => {}
    }
    graph.get_mut(path).expect("module exists").status = ModuleStatus::Evaluating;
    let dependencies = graph.get(path).expect("module exists").dependencies.clone();
    for dependency in dependencies {
        if let Err(err) = evaluate_module(vm, &dependency, graph) {
            mark_scc_error(graph, path, err.clone());
            return Err(err);
        }
    }

    let (chunk, env) = {
        let record = graph.get(path).expect("module exists");
        (
            record
                .chunk
                .clone()
                .ok_or_else(|| Error::internal("Instantiated module has no bytecode"))?,
            record.env,
        )
    };
    match vm.evaluate_module_chunk(chunk, env) {
        Ok(value) => {
            graph.get_mut(path).expect("module exists").status = ModuleStatus::Evaluated;
            Ok(value)
        }
        Err(err) => {
            mark_scc_error(graph, path, err.clone());
            Err(err)
        }
    }
}

impl Vm {
    /// Parse, resolve, and instantiate a module graph without evaluating it.
    pub fn link_module_file(&mut self, path: impl AsRef<Path>) -> error::Result<()> {
        let root = path.as_ref().canonicalize().map_err(|err| {
            Error::syntax(format!(
                "Cannot resolve entry module '{}': {}",
                path.as_ref().display(),
                err
            ))
        })?;
        let mut graph = HashMap::new();
        load_graph(self, root.clone(), &mut graph)?;
        assign_scc_ids(&mut graph);
        link_imports_with_vm(self, &graph)?;
        let pin_count = graph.len();
        for record in graph.values() {
            self.gc_pins.push(record.env.0);
        }
        let result = instantiate_module(self, &root, &mut graph);
        if result.is_ok() {
            for (path, record) in graph {
                self.module_records.insert(path, record);
            }
        }
        self.gc_pins
            .truncate(self.gc_pins.len().saturating_sub(pin_count));
        result
    }

    /// Load, link, and evaluate an ECMAScript module graph rooted at `path`.
    pub fn run_module_file(&mut self, path: impl AsRef<Path>) -> error::Result<Value> {
        let root = path.as_ref().canonicalize().map_err(|err| {
            Error::syntax(format!(
                "Cannot resolve entry module '{}': {}",
                path.as_ref().display(),
                err
            ))
        })?;
        let mut graph = HashMap::new();
        load_graph(self, root.clone(), &mut graph)?;
        assign_scc_ids(&mut graph);
        link_imports_with_vm(self, &graph)?;

        let pin_count = graph.len();
        for record in graph.values() {
            self.gc_pins.push(record.env.0);
        }
        let (result, cache_graph) = match instantiate_module(self, &root, &mut graph) {
            Ok(()) => (evaluate_module(self, &root, &mut graph), true),
            Err(error) => (Err(error), false),
        };
        if cache_graph {
            for (path, record) in &graph {
                self.module_records.insert(path.clone(), record.clone());
            }
        }
        self.gc_pins
            .truncate(self.gc_pins.len().saturating_sub(pin_count));

        let result_roots: Vec<Value> = match &result {
            Ok(value) => vec![value.clone()],
            Err(err) => err.thrown_value.iter().cloned().collect(),
        };
        let pinned_result = self.pin_many(&result_roots);
        self.clear_kept_objects();
        let microtask_result = if !self.microtask_queue.is_empty() {
            self.run_microtasks()
        } else {
            Ok(())
        };
        self.unpin_many(pinned_result);
        microtask_result?;
        result
    }
}
