use crate::ast::{ExportEntry, Program};
use crate::error::{self, Error};
use crate::value::{GcIdx, Value};
use crate::{Compiler, Parser, Vm};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleStatus {
    Linked,
    Evaluating,
    Evaluated,
    Errored,
}

#[derive(Clone)]
pub(crate) struct ModuleRecord {
    pub(crate) program: Program,
    pub(crate) env: GcIdx,
    pub(crate) dependencies: Vec<PathBuf>,
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

fn reject_cyclic_graph(graph: &HashMap<PathBuf, ModuleRecord>) -> error::Result<()> {
    fn visit(
        path: &Path,
        graph: &HashMap<PathBuf, ModuleRecord>,
        visiting: &mut HashSet<PathBuf>,
        visited: &mut HashSet<PathBuf>,
    ) -> error::Result<()> {
        if visited.contains(path) {
            return Ok(());
        }
        if !visiting.insert(path.to_path_buf()) {
            return Err(Error::syntax(format!(
                "Cyclic module graph at '{}' is not supported yet",
                path.display()
            )));
        }
        let record = graph
            .get(path)
            .ok_or_else(|| Error::syntax("Module graph is incomplete"))?;
        for dependency in &record.dependencies {
            visit(dependency, graph, visiting, visited)?;
        }
        visiting.remove(path);
        visited.insert(path.to_path_buf());
        Ok(())
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for path in graph.keys() {
        visit(path, graph, &mut visiting, &mut visited)?;
    }
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
        ModuleStatus::Linked => {}
    }
    graph.get_mut(path).expect("module exists").status = ModuleStatus::Evaluating;
    let dependencies = graph.get(path).expect("module exists").dependencies.clone();
    for dependency in dependencies {
        if let Err(err) = evaluate_module(vm, &dependency, graph) {
            let record = graph.get_mut(path).expect("module exists");
            record.status = ModuleStatus::Errored;
            record.error = Some(err.clone());
            return Err(err);
        }
    }

    let (program, env) = {
        let record = graph.get(path).expect("module exists");
        (record.program.clone(), record.env)
    };
    let mut compiler = Compiler::new();
    let (chunk, funcs) = compiler.compile_program(&program)?;
    let chunk = vm.append_compiled_functions(chunk, funcs);
    match vm.execute_chunk(chunk, env, Value::Undefined) {
        Ok(value) => {
            graph.get_mut(path).expect("module exists").status = ModuleStatus::Evaluated;
            Ok(value)
        }
        Err(err) => {
            let record = graph.get_mut(path).expect("module exists");
            record.status = ModuleStatus::Errored;
            record.error = Some(err.clone());
            Err(err)
        }
    }
}

impl Vm {
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
        reject_cyclic_graph(&graph)?;
        link_imports_with_vm(self, &graph)?;

        let pin_count = graph.len();
        for record in graph.values() {
            self.gc_pins.push(record.env.0);
        }
        let result = evaluate_module(self, &root, &mut graph);
        for (path, record) in &graph {
            self.module_records.insert(path.clone(), record.clone());
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
