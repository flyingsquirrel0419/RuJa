use crate::ast::{ExportEntry, Program};
use crate::bytecode::Chunk;
use crate::error::{self, Error};
use crate::value::{GcIdx, HeapObj, PromiseStatus, Value};
use crate::{Compiler, Parser, Vm};
use indexmap::IndexMap;
use parking_lot::Mutex;
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

pub(crate) enum DynamicImportResult {
    Ready(Value),
    Pending {
        target: PathBuf,
        evaluation_promise: GcIdx,
    },
}

#[derive(Clone)]
pub(crate) struct ModuleRecord {
    pub(crate) program: Program,
    pub(crate) env: GcIdx,
    pub(crate) dependencies: Vec<PathBuf>,
    chunk: Option<Arc<Chunk>>,
    scc_id: usize,
    runtime: Arc<Mutex<ModuleRuntime>>,
}

struct ModuleRuntime {
    evaluation_promise: Option<GcIdx>,
    completion_value: Option<Value>,
    namespace: Option<GcIdx>,
    import_meta: Option<GcIdx>,
    namespace_initializing: bool,
    status: ModuleStatus,
    error: Option<Arc<Error>>,
}

impl ModuleRecord {
    fn status(&self) -> ModuleStatus {
        self.runtime.lock().status
    }

    pub(crate) fn evaluation_promise(&self) -> Option<GcIdx> {
        self.runtime.lock().evaluation_promise
    }

    pub(crate) fn error(&self) -> Option<Arc<Error>> {
        self.runtime.lock().error.clone()
    }

    pub(crate) fn completion_value(&self) -> Option<Value> {
        self.runtime.lock().completion_value.clone()
    }

    pub(crate) fn import_meta(&self) -> Option<GcIdx> {
        self.runtime.lock().import_meta
    }
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
    load_graph_from_source(vm, path, &source, graph)
}

fn load_graph_from_source(
    vm: &mut Vm,
    path: PathBuf,
    source: &str,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<()> {
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
            runtime: Arc::new(Mutex::new(ModuleRuntime {
                evaluation_promise: None,
                completion_value: None,
                namespace: None,
                import_meta: None,
                namespace_initializing: false,
                status: ModuleStatus::Linked,
                error: None,
            })),
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

fn data_module_cache_key(target: &Path, import_type: &str) -> PathBuf {
    let mut key = target.as_os_str().to_os_string();
    key.push(format!("\0ruja-data-module:{import_type}"));
    PathBuf::from(key)
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
            ExportEntry::NamespaceReExport {
                module_request,
                export_name: candidate,
            } if candidate.as_ref() == export_name => {
                let dependency = resolve_specifier(module, &module_request.specifier)?;
                let target = graph
                    .get(&dependency)
                    .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
                return Ok(Some((target.env, Arc::from("*namespace*"))));
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

fn exported_names(
    graph: &HashMap<PathBuf, ModuleRecord>,
    module: &Path,
    seen: &mut HashSet<PathBuf>,
) -> error::Result<Vec<Arc<str>>> {
    if !seen.insert(module.to_path_buf()) {
        return Ok(Vec::new());
    }
    let record = graph
        .get(module)
        .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
    let mut names = Vec::new();
    for entry in &record.program.export_entries {
        match entry {
            ExportEntry::Local { export_name, .. }
            | ExportEntry::ReExport { export_name, .. }
            | ExportEntry::NamespaceReExport { export_name, .. } => {
                names.push(export_name.clone());
            }
            ExportEntry::Star { module_request } => {
                let dependency = resolve_specifier(module, &module_request.specifier)?;
                for name in exported_names(graph, &dependency, seen)? {
                    if name.as_ref() != "default" {
                        names.push(name);
                    }
                }
            }
        }
    }
    names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    names.dedup();
    Ok(names)
}

fn get_module_namespace(
    vm: &mut Vm,
    path: &Path,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<GcIdx> {
    if let Some(record) = graph.get(path) {
        if let Some(namespace) = record.runtime.lock().namespace {
            return Ok(namespace);
        }
    }
    let mut tag = crate::value::PropertyDescriptor::data(Value::String(Arc::from("Module")));
    tag.writable = false;
    tag.enumerable = false;
    tag.configurable = false;
    let mut props = IndexMap::new();
    props.insert(
        crate::value::PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
        tag,
    );
    let namespace = GcIdx(vm.heap.allocate(crate::value::HeapObj::ModuleNamespace(
        crate::value::ModuleNamespaceData {
            exports: Mutex::new(IndexMap::new()),
            props: Mutex::new(props),
            proto: Mutex::new(None),
        },
    ))?);
    {
        let record = graph
            .get_mut(path)
            .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
        let mut runtime = record.runtime.lock();
        runtime.namespace = Some(namespace);
        runtime.namespace_initializing = true;
        drop(runtime);
        crate::environment::declare(
            &vm.heap,
            record.env,
            "*namespace*",
            Value::Object(namespace),
            crate::value::BindingKind::Const,
        );
    }

    let namespace_exports: Vec<(Arc<str>, PathBuf)> = graph
        .get(path)
        .expect("module exists")
        .program
        .export_entries
        .iter()
        .filter_map(|entry| {
            if let ExportEntry::NamespaceReExport {
                module_request,
                export_name,
            } = entry
            {
                Some((
                    export_name.clone(),
                    resolve_specifier(path, &module_request.specifier),
                ))
            } else {
                None
            }
        })
        .map(|(name, dependency)| dependency.map(|dependency| (name, dependency)))
        .collect::<error::Result<_>>()?;
    for (_, dependency) in namespace_exports {
        get_module_namespace(vm, &dependency, graph)?;
    }

    let names = exported_names(graph, path, &mut HashSet::new())?;
    let mut resolved = IndexMap::new();
    for name in names {
        if let Ok(Some(binding)) = resolve_export_optional(graph, path, &name, &mut HashSet::new())
        {
            resolved.insert(name, binding);
        }
    }
    vm.heap.with_obj(namespace.0, |object| {
        if let crate::value::HeapObj::ModuleNamespace(data) = object {
            *data.exports.lock() = resolved;
        }
    });
    graph
        .get(path)
        .expect("module exists")
        .runtime
        .lock()
        .namespace_initializing = false;
    Ok(namespace)
}

fn link_imports_with_vm(
    vm: &mut Vm,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<()> {
    for (path, record) in graph.iter() {
        for export in &record.program.export_entries {
            if let ExportEntry::ReExport { export_name, .. } = export {
                resolve_export(graph, path, export_name, &mut HashSet::new())?;
            }
        }
    }
    let namespace_reexporters: Vec<PathBuf> = graph
        .iter()
        .filter(|(_, record)| {
            record
                .program
                .export_entries
                .iter()
                .any(|entry| matches!(entry, ExportEntry::NamespaceReExport { .. }))
        })
        .map(|(path, _)| path.clone())
        .collect();
    for path in namespace_reexporters {
        get_module_namespace(vm, &path, graph)?;
    }
    let imports: Vec<(PathBuf, GcIdx, crate::ast::ImportEntry)> = graph
        .iter()
        .flat_map(|(path, record)| {
            record
                .program
                .import_entries
                .iter()
                .cloned()
                .map(|import| (path.clone(), record.env, import))
                .collect::<Vec<_>>()
        })
        .collect();
    for (path, env, import) in imports {
        let dependency = resolve_specifier(&path, &import.module_request.specifier)?;
        if import.import_name.as_ref() == "*" {
            let namespace = get_module_namespace(vm, &dependency, graph)?;
            crate::environment::declare(
                &vm.heap,
                env,
                &import.local_name,
                Value::Object(namespace),
                crate::value::BindingKind::Const,
            );
            continue;
        }
        let (target_env, target_name) =
            resolve_export(graph, &dependency, &import.import_name, &mut HashSet::new())?;
        crate::environment::declare_import(
            &vm.heap,
            env,
            &import.local_name,
            target_env,
            target_name,
        );
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

fn mark_dependent_errors(
    graph: &mut HashMap<PathBuf, ModuleRecord>,
    path: &Path,
    error: Arc<Error>,
) {
    let mut failed_components = HashSet::from([graph
        .get(path)
        .map(|record| record.scc_id)
        .unwrap_or(usize::MAX)]);
    loop {
        let mut changed = false;
        for record in graph.values() {
            if failed_components.contains(&record.scc_id) {
                continue;
            }
            if record.dependencies.iter().any(|dependency| {
                graph
                    .get(dependency)
                    .is_some_and(|dependency| failed_components.contains(&dependency.scc_id))
            }) {
                failed_components.insert(record.scc_id);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for record in graph.values() {
        if failed_components.contains(&record.scc_id) {
            let mut runtime = record.runtime.lock();
            runtime.status = ModuleStatus::Errored;
            runtime.error = Some(error.clone());
            runtime.evaluation_promise = None;
            runtime.completion_value = None;
        }
    }
}

fn clear_settled_module_runtime(graph: &HashMap<PathBuf, ModuleRecord>) {
    for record in graph.values() {
        let mut runtime = record.runtime.lock();
        if matches!(
            runtime.status,
            ModuleStatus::Evaluated | ModuleStatus::Errored
        ) {
            runtime.evaluation_promise = None;
            runtime.completion_value = None;
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
        (record.status(), record.error(), record.dependencies.clone())
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
    graph
        .get(path)
        .expect("module exists")
        .runtime
        .lock()
        .status = ModuleStatus::Instantiating;
    for dependency in dependencies {
        instantiate_module(vm, &dependency, graph)?;
    }

    let (program, env) = {
        let record = graph.get(path).expect("module exists");
        (record.program.clone(), record.env)
    };
    let mut compiler = Compiler::new();
    let (chunk, funcs) = compiler.compile_program(&program)?;
    let chunk = Arc::new(vm.append_compiled_functions_with_source(
        chunk,
        funcs,
        Arc::new(path.to_path_buf()),
    ));
    if let Err(error) = vm.instantiate_module_chunk(chunk.clone(), env) {
        let record = graph.get(path).expect("module exists");
        let mut runtime = record.runtime.lock();
        runtime.status = ModuleStatus::Errored;
        runtime.error = Some(error.clone());
        return Err(error);
    }
    let record = graph.get_mut(path).expect("module exists");
    record.chunk = Some(chunk);
    record.runtime.lock().status = ModuleStatus::Instantiated;
    Ok(())
}

fn module_evaluation_order(
    path: &Path,
    graph: &HashMap<PathBuf, ModuleRecord>,
    visiting: &mut HashSet<PathBuf>,
    visited: &mut HashSet<PathBuf>,
    order: &mut Vec<PathBuf>,
) -> error::Result<()> {
    if visited.contains(path) || !visiting.insert(path.to_path_buf()) {
        return Ok(());
    }
    let dependencies = graph
        .get(path)
        .ok_or_else(|| Error::syntax("Module graph is incomplete"))?
        .dependencies
        .clone();
    for dependency in dependencies {
        module_evaluation_order(&dependency, graph, visiting, visited, order)?;
    }
    visiting.remove(path);
    visited.insert(path.to_path_buf());
    order.push(path.to_path_buf());
    Ok(())
}

fn promise_state(vm: &Vm, promise: GcIdx) -> (PromiseStatus, Value) {
    vm.heap.with_obj(promise.0, |object| {
        if let HeapObj::Promise(data) = object {
            (*data.state.lock(), data.result.lock().clone())
        } else {
            (PromiseStatus::Rejected, Value::Undefined)
        }
    })
}

fn evaluate_module(
    vm: &mut Vm,
    path: &Path,
    graph: &mut HashMap<PathBuf, ModuleRecord>,
) -> error::Result<Value> {
    if let Some(record) = graph.get(path) {
        match record.status() {
            ModuleStatus::Evaluated => return Ok(Value::Undefined),
            ModuleStatus::Errored => {
                let error = record
                    .error()
                    .unwrap_or_else(|| Error::syntax("Module evaluation failed"));
                clear_settled_module_runtime(graph);
                return Err(error);
            }
            ModuleStatus::Linked | ModuleStatus::Instantiating => {
                let error = Error::internal("Module evaluated before instantiation");
                clear_settled_module_runtime(graph);
                return Err(error);
            }
            ModuleStatus::Instantiated | ModuleStatus::Evaluating => {}
        }
    }

    let mut order = Vec::new();
    module_evaluation_order(
        path,
        graph,
        &mut HashSet::new(),
        &mut HashSet::new(),
        &mut order,
    )?;
    let mut running: HashMap<PathBuf, GcIdx> = HashMap::new();
    let mut running_pin_count = 0;
    let mut completion_values: HashMap<PathBuf, Value> = HashMap::new();
    let order_positions: HashMap<PathBuf, usize> = order
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, path)| (path, index))
        .collect();
    for module in &order {
        let record = graph.get(module).expect("module exists");
        if record.status() == ModuleStatus::Evaluating {
            let promise = record
                .evaluation_promise()
                .ok_or_else(|| Error::internal("Evaluating module has no evaluation Promise"))?;
            running_pin_count += vm.pin(&Value::Object(promise));
            running.insert(module.clone(), promise);
        }
    }

    loop {
        let mut progressed = false;

        let running_paths: Vec<PathBuf> = running.keys().cloned().collect();
        for module in running_paths {
            let promise = running[&module];
            let (state, result) = promise_state(vm, promise);
            match state {
                PromiseStatus::Pending => {}
                PromiseStatus::Fulfilled => {
                    running.remove(&module);
                    let completion = graph
                        .get(&module)
                        .expect("module exists")
                        .runtime
                        .lock()
                        .completion_value
                        .clone()
                        .unwrap_or(result);
                    completion_values.insert(module.clone(), completion);
                    graph
                        .get(&module)
                        .expect("module exists")
                        .runtime
                        .lock()
                        .status = ModuleStatus::Evaluated;
                    progressed = true;
                }
                PromiseStatus::Rejected => {
                    let error = Error::thrown(result, &vm.heap);
                    mark_dependent_errors(graph, &module, error.clone());
                    vm.unpin_many(running_pin_count);
                    clear_settled_module_runtime(graph);
                    return Err(error);
                }
            }
        }

        if graph
            .get(path)
            .is_some_and(|record| record.status() == ModuleStatus::Evaluated)
        {
            vm.unpin_many(running_pin_count);
            let completion = completion_values.remove(path).unwrap_or(Value::Undefined);
            clear_settled_module_runtime(graph);
            return Ok(completion);
        }

        for module in &order {
            let (status, scc_id, dependencies) = {
                let record = graph.get(module).expect("module exists");
                (record.status(), record.scc_id, record.dependencies.clone())
            };
            if status != ModuleStatus::Instantiated {
                continue;
            }
            let mut ready = true;
            let module_position = order_positions[module];
            for earlier in &order[..module_position] {
                let earlier_record = graph.get(earlier).expect("module exists");
                if earlier_record.scc_id == scc_id
                    && earlier_record.status() != ModuleStatus::Evaluated
                {
                    ready = false;
                    break;
                }
            }
            if !ready {
                continue;
            }
            for dependency in dependencies {
                let dependency_record = graph.get(&dependency).expect("dependency exists");
                if dependency_record.status() == ModuleStatus::Errored {
                    let error = dependency_record
                        .error()
                        .unwrap_or_else(|| Error::syntax("Module dependency evaluation failed"));
                    mark_dependent_errors(graph, &dependency, error.clone());
                    vm.unpin_many(running_pin_count);
                    clear_settled_module_runtime(graph);
                    return Err(error);
                }
                if dependency_record.scc_id != scc_id {
                    let dependency_scc = dependency_record.scc_id;
                    if graph.values().any(|record| {
                        record.scc_id == dependency_scc
                            && record.status() != ModuleStatus::Evaluated
                    }) {
                        ready = false;
                        break;
                    }
                }
            }
            if !ready {
                continue;
            }
            let (chunk, env) = {
                let record = graph.get(module).expect("module exists");
                (
                    record
                        .chunk
                        .clone()
                        .ok_or_else(|| Error::internal("Instantiated module has no bytecode"))?,
                    record.env,
                )
            };
            graph
                .get(module)
                .expect("module exists")
                .runtime
                .lock()
                .status = ModuleStatus::Evaluating;
            match vm.evaluate_module_chunk_async(chunk, env) {
                Ok((promise, completion)) => {
                    let pin = vm.pin(&Value::Object(promise));
                    running_pin_count += pin;
                    {
                        let mut runtime = graph.get(module).expect("module exists").runtime.lock();
                        runtime.evaluation_promise = Some(promise);
                        runtime.completion_value = completion;
                    }
                    let (state, result) = promise_state(vm, promise);
                    match state {
                        PromiseStatus::Pending => {
                            running.insert(module.clone(), promise);
                            progressed = true;
                        }
                        PromiseStatus::Fulfilled => {
                            let completion = graph
                                .get(module)
                                .expect("module exists")
                                .runtime
                                .lock()
                                .completion_value
                                .clone()
                                .unwrap_or(result);
                            completion_values.insert(module.clone(), completion);
                            graph
                                .get(module)
                                .expect("module exists")
                                .runtime
                                .lock()
                                .status = ModuleStatus::Evaluated;
                            progressed = true;
                        }
                        PromiseStatus::Rejected => {
                            let error = Error::thrown(result, &vm.heap);
                            mark_dependent_errors(graph, module, error.clone());
                            vm.unpin_many(running_pin_count);
                            clear_settled_module_runtime(graph);
                            return Err(error);
                        }
                    }
                }
                Err(error) => {
                    mark_dependent_errors(graph, module, error.clone());
                    vm.unpin_many(running_pin_count);
                    clear_settled_module_runtime(graph);
                    return Err(error);
                }
            }
        }

        if graph
            .get(path)
            .is_some_and(|record| record.status() == ModuleStatus::Evaluated)
        {
            continue;
        }

        match vm.tick() {
            Ok(true) => progressed = true,
            Ok(false) => {}
            Err(error) => {
                vm.unpin_many(running_pin_count);
                clear_settled_module_runtime(graph);
                return Err(error);
            }
        }
        if !progressed {
            vm.unpin_many(running_pin_count);
            let error =
                Error::internal("Async module evaluation is pending without a runnable job");
            clear_settled_module_runtime(graph);
            return Err(error);
        }
    }
}

impl Vm {
    pub(crate) fn allocate_import_meta(&mut self) -> error::Result<GcIdx> {
        Ok(GcIdx(self.heap.allocate(HeapObj::Object(
            crate::value::ObjectData {
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(None),
                extensible: std::sync::atomic::AtomicBool::new(true),
                class_name: None,
                private_fields: Mutex::new(HashMap::new()),
                primitive: Mutex::new(None),
            },
        ))?))
    }

    pub(crate) fn import_meta_object(&mut self, path: &Path) -> error::Result<Value> {
        let record = self
            .module_records
            .get(path)
            .cloned()
            .ok_or_else(|| Error::syntax("import.meta module record is unavailable"))?;
        if let Some(meta) = record.import_meta() {
            return Ok(Value::Object(meta));
        }
        let meta = self.allocate_import_meta()?;
        record.runtime.lock().import_meta = Some(meta);
        Ok(Value::Object(meta))
    }

    pub(crate) fn set_module_completion(&mut self, path: &Path, value: Value) {
        if let Some(record) = self.module_records.get(path) {
            record.runtime.lock().completion_value = Some(value);
        }
    }

    pub(crate) fn finish_dynamic_import(&mut self, target: &Path) -> error::Result<Value> {
        let mut graph = HashMap::new();
        load_graph(self, target.to_path_buf(), &mut graph)?;
        let namespace = get_module_namespace(self, target, &mut graph)?;
        for (path, record) in graph {
            self.module_records.insert(path, record);
        }
        Ok(Value::Object(namespace))
    }

    pub(crate) fn dynamic_import_module(
        &mut self,
        referrer: &Path,
        specifier: &str,
        import_type: Option<&str>,
    ) -> error::Result<DynamicImportResult> {
        let target = resolve_specifier(referrer, specifier)?;
        if matches!(import_type, Some("json" | "text")) {
            let import_type = import_type.expect("matched supported data module type");
            let virtual_target = data_module_cache_key(&target, import_type);
            if !self.module_records.contains_key(&virtual_target) {
                let source = std::fs::read_to_string(&target).map_err(|error| {
                    Error::syntax(format!(
                        "Cannot read {} module '{}': {}",
                        import_type,
                        target.display(),
                        error
                    ))
                })?;
                let value = if import_type == "json" {
                    crate::builtins::json::parse_json_text(self, &source)?
                } else {
                    Value::String(Arc::from(source))
                };
                let value_pin = self.pin(&value);
                let mut graph = HashMap::new();
                let loaded = load_graph_from_source(
                    self,
                    virtual_target.clone(),
                    "export let __ruja_data_module_default; export { __ruja_data_module_default as default };",
                    &mut graph,
                )
                .and_then(|()| self.run_loaded_module_graph(&virtual_target, graph, false));
                self.unpin(value_pin);
                loaded?;
                let record = self.module_records.get(&virtual_target).ok_or_else(|| {
                    Error::syntax("Data module evaluation did not produce a record".to_string())
                })?;
                if !crate::environment::set(
                    &self.heap,
                    record.env,
                    "__ruja_data_module_default",
                    value,
                ) {
                    return Err(Error::syntax(
                        "Data module default binding is unavailable".to_string(),
                    ));
                }
            }
            return self
                .finish_dynamic_import(&virtual_target)
                .map(DynamicImportResult::Ready);
        }
        if let Some(record) = self.module_records.get(&target) {
            if record.status() == ModuleStatus::Evaluating {
                let evaluation_promise = record.evaluation_promise().ok_or_else(|| {
                    Error::internal("Evaluating module has no evaluation Promise")
                })?;
                return Ok(DynamicImportResult::Pending {
                    target,
                    evaluation_promise,
                });
            }
        }
        self.run_module_file_inner(&target, false)?;
        self.finish_dynamic_import(&target)
            .map(DynamicImportResult::Ready)
    }

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
        link_imports_with_vm(self, &mut graph)?;
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
        self.run_module_file_inner(path.as_ref(), true)
    }

    fn run_module_file_inner(
        &mut self,
        path: &Path,
        drain_microtasks: bool,
    ) -> error::Result<Value> {
        let root = path.canonicalize().map_err(|err| {
            Error::syntax(format!(
                "Cannot resolve entry module '{}': {}",
                path.display(),
                err
            ))
        })?;
        let mut graph = HashMap::new();
        load_graph(self, root.clone(), &mut graph)?;
        self.run_loaded_module_graph(&root, graph, drain_microtasks)
    }

    fn run_loaded_module_graph(
        &mut self,
        root: &Path,
        mut graph: HashMap<PathBuf, ModuleRecord>,
        drain_microtasks: bool,
    ) -> error::Result<Value> {
        assign_scc_ids(&mut graph);
        link_imports_with_vm(self, &mut graph)?;

        let pin_count = graph.len();
        for record in graph.values() {
            self.gc_pins.push(record.env.0);
        }
        let (result, cache_graph) = match instantiate_module(self, root, &mut graph) {
            Ok(()) => {
                // Publish the canonical runtime before evaluation so nested
                // dynamic imports share status, Promise, and namespace state.
                for (path, record) in &graph {
                    self.module_records.insert(path.clone(), record.clone());
                }
                (evaluate_module(self, root, &mut graph), true)
            }
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
        let microtask_result = if drain_microtasks && !self.microtask_queue.is_empty() {
            self.run_microtasks()
        } else {
            Ok(())
        };
        self.unpin_many(pinned_result);
        microtask_result?;
        result
    }
}
