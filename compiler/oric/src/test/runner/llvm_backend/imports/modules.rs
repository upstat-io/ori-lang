use std::path::{Path, PathBuf};
use std::sync::Arc;

use ori_ir::{ImportCycleGuard, Name, StringInterner};
use ori_types::{Idx, Pool, TypeCheckResult};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::imports::{normalize_path, ResolvedImportedModule, ResolvedImports};

pub(super) struct LoadedModules {
    pub(super) modules: Vec<ResolvedImportedModule>,
    pub(super) type_results: Vec<TypeCheckResult>,
    pub(super) canons: Vec<ori_ir::canon::SharedCanonResult>,
    pub(super) pools: Vec<Arc<Pool>>,
    pub(super) target_maps: Vec<FxHashMap<Name, Name>>,
    pub(super) root_targets: FxHashMap<Name, Name>,
    pub(super) explicit_len: usize,
    pub(super) prelude_index: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum LoadError {
    #[error(
        "circular import detected while closing LLVM JIT callables: {path}; remove one import edge so the module graph is acyclic (Spec: Clause 18.7)"
    )]
    CircularImport { path: String },
}

struct Loader<'a> {
    db: &'a crate::db::CompilerDb,
    interner: &'a StringInterner,
    guard: ImportCycleGuard,
    by_path: FxHashMap<PathBuf, usize>,
    modules: Vec<ResolvedImportedModule>,
    type_results: Vec<TypeCheckResult>,
    canons: Vec<ori_ir::canon::SharedCanonResult>,
    pools: Vec<Arc<Pool>>,
    target_maps: Vec<FxHashMap<Name, Name>>,
}

impl<'a> Loader<'a> {
    fn new(db: &'a crate::db::CompilerDb, interner: &'a StringInterner) -> Self {
        Self {
            db,
            interner,
            guard: ImportCycleGuard::new(),
            by_path: FxHashMap::default(),
            modules: Vec::new(),
            type_results: Vec::new(),
            canons: Vec::new(),
            pools: Vec::new(),
            target_maps: Vec::new(),
        }
    }

    fn visit(&mut self, module: ResolvedImportedModule) -> Result<usize, LoadError> {
        let path = normalize_path(&module.module_path);
        if let Some(&index) = self.by_path.get(&path) {
            return Ok(index);
        }
        self.guard
            .start_loading(path.clone())
            .map_err(|cycle| LoadError::CircularImport {
                path: cycle
                    .iter()
                    .map(|entry| entry.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            })?;
        // Keep the resolver-provided spelling for imported producer symbols:
        // `./segment` is semantically normalized for module identity, but it is
        // part of the versioned producer symbol emitted by type checking.
        let resolved =
            crate::imports::resolve_imports(self.db, &module.parse_output, &module.module_path);
        for imported in &resolved.modules {
            self.visit(imported.clone())?;
        }

        let (type_result, canon, pool) = load_analysis(self.db, &module);
        let own_names: FxHashSet<Name> = module
            .parse_output
            .module
            .functions
            .iter()
            .map(|function| function.name)
            .collect();
        let mut target_map: FxHashMap<Name, Name> = own_names
            .iter()
            .map(|&name| (name, qualified_function_name(self.interner, &path, name)))
            .collect();

        for imported in &resolved.imported_functions {
            if imported.is_module_alias || own_names.contains(&imported.local_name) {
                continue;
            }
            let imported_path =
                normalize_path(&resolved.modules[imported.module_index].module_path);
            let Some(&module_index) = self.by_path.get(&imported_path) else {
                continue;
            };
            let Some(&target) = self.target_maps[module_index].get(&imported.original_name) else {
                continue;
            };
            target_map.insert(imported.local_name, target);
        }

        let index = self.modules.len();
        self.modules.push(module);
        self.type_results.push(type_result);
        self.canons.push(canon);
        self.pools.push(pool);
        self.target_maps.push(target_map);
        self.by_path.insert(path.clone(), index);
        self.guard.finish_loading(&path);
        Ok(index)
    }

    fn append_prelude(&mut self, prelude: ResolvedImportedModule) -> usize {
        let (type_result, canon, pool) = load_analysis(self.db, &prelude);
        let index = self.modules.len();
        self.modules.push(prelude);
        self.type_results.push(type_result);
        self.canons.push(canon);
        self.pools.push(pool);
        self.target_maps.push(FxHashMap::default());
        index
    }
}

pub(super) fn load(
    db: &crate::db::CompilerDb,
    root_parse: &crate::parser::ParseOutput,
    resolved: &ResolvedImports,
    interner: &StringInterner,
) -> Result<LoadedModules, LoadError> {
    let mut loader = Loader::new(db, interner);
    for module in &resolved.modules {
        loader.visit(module.clone())?;
    }
    let explicit_len = loader.modules.len();
    let prelude_index = resolved
        .prelude
        .as_ref()
        .map(|prelude| loader.append_prelude(prelude.clone()));

    let root_names: FxHashSet<Name> = root_parse
        .module
        .functions
        .iter()
        .map(|function| function.name)
        .collect();
    let mut root_targets = FxHashMap::default();
    for imported in &resolved.imported_functions {
        if imported.is_module_alias || root_names.contains(&imported.local_name) {
            continue;
        }
        let imported_path = normalize_path(&resolved.modules[imported.module_index].module_path);
        let Some(&module_index) = loader.by_path.get(&imported_path) else {
            continue;
        };
        let Some(&target) = loader.target_maps[module_index].get(&imported.original_name) else {
            continue;
        };
        root_targets.insert(imported.local_name, target);
    }

    Ok(LoadedModules {
        modules: loader.modules,
        type_results: loader.type_results,
        canons: loader.canons,
        pools: loader.pools,
        target_maps: loader.target_maps,
        root_targets,
        explicit_len,
        prelude_index,
    })
}

fn load_analysis(
    db: &crate::db::CompilerDb,
    module: &ResolvedImportedModule,
) -> (TypeCheckResult, ori_ir::canon::SharedCanonResult, Arc<Pool>) {
    let Some((typed, pool)) = crate::query::type_check_module(
        db,
        &module.parse_output,
        &module.module_path,
        module.source_file,
    ) else {
        return (
            TypeCheckResult::ok(ori_types::TypedModule::default()),
            ori_ir::canon::SharedCanonResult::new(ori_ir::canon::CanonResult::empty()),
            Arc::new(Pool::new()),
        );
    };
    let canon = if module.source_file.is_some() {
        crate::query::canonicalize_cached_by_path(
            db,
            &module.module_path,
            &module.parse_output,
            &typed,
            &pool,
        )
    } else {
        crate::query::canonicalize_uncached_by_path(
            db,
            &module.module_path,
            &module.parse_output,
            &typed,
            &pool,
        )
    };
    (typed, canon, pool)
}

fn qualified_function_name(interner: &StringInterner, path: &Path, source: Name) -> Name {
    let path = path.to_string_lossy().replace('\\', "/");
    interner.intern(&format!(
        "$module${path}$function${}",
        interner.lookup(source)
    ))
}

pub(super) fn remap_canons(
    canons: &[ori_ir::canon::SharedCanonResult],
    pools: &[Arc<Pool>],
    merged_pool: &mut Pool,
    caches: &mut [FxHashMap<Idx, Idx>],
    var_remaps: &mut [FxHashMap<u32, u32>],
) -> Vec<ori_ir::canon::CanonResult> {
    canons
        .iter()
        .enumerate()
        .map(|(module_index, shared)| {
            let source_pool = &pools[module_index];
            let cache = &mut caches[module_index];
            let var_remap = &mut var_remaps[module_index];
            let mut remapped = (**shared).clone();
            remapped.arena.remap_types(|type_id| {
                let source = Idx::from_raw(type_id.raw());
                let target = ori_types::re_intern_type_with_var_remap(
                    source_pool,
                    source,
                    merged_pool,
                    cache,
                    var_remap,
                );
                ori_ir::TypeId::from_raw(target.raw())
            });
            remapped
        })
        .collect()
}

#[cfg(test)]
#[path = "modules/tests.rs"]
mod tests;
