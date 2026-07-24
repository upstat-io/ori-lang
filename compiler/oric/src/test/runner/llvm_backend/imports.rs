//! Import preparation for the LLVM JIT test backend.
//!
//! Imported modules own independent type pools. This module type-checks those
//! modules, re-interns every codegen-facing type into the test file's pool,
//! and assembles concrete and monomorphized imported functions.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ori_llvm::evaluator::ImportedFunctionForCodegen;
use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::imported_mono::{
    build_imported_mono_functions, register_prelude_generic_sigs, ImportedMonoFn,
};

mod modules;

pub(super) use modules::LoadError;
use modules::LoadedModules;

pub(super) struct PreparedImports {
    pub(super) modules: Vec<crate::imports::ResolvedImportedModule>,
    pub(super) type_results: Vec<TypeCheckResult>,
    pub(super) pool: Pool,
    pub(super) canons: Vec<ori_ir::canon::CanonResult>,
    pub(super) function_refs: Vec<FunctionRef>,
    pub(super) signatures: Vec<FunctionSig>,
    pub(super) renamed_functions: Vec<Option<crate::ir::Function>>,
    pub(super) mono_functions: Vec<ImportedMonoFn>,
    pub(super) generic_templates: Vec<crate::realization::ImportedGenericTemplate>,
    pub(super) target_maps: Vec<FxHashMap<crate::ir::Name, crate::ir::Name>>,
    pub(super) root_targets: FxHashMap<crate::ir::Name, crate::ir::Name>,
}

pub(super) struct FunctionRef {
    pub(super) func_index: usize,
    pub(super) module_index: usize,
    pub(super) local_name: crate::ir::Name,
    pub(super) original_name: crate::ir::Name,
}

struct ImportSources<'a> {
    resolved: &'a crate::imports::ResolvedImports,
    modules: &'a [crate::imports::ResolvedImportedModule],
    type_results: &'a [TypeCheckResult],
    pools: &'a [Arc<Pool>],
    target_maps: &'a [FxHashMap<crate::ir::Name, crate::ir::Name>],
    explicit_len: usize,
    prelude_index: Option<usize>,
}

struct ReinternState<'a> {
    pool: &'a mut Pool,
    caches: &'a mut [FxHashMap<Idx, Idx>],
    var_remaps: &'a mut [FxHashMap<u32, u32>],
}

struct FunctionInventory<'a> {
    signatures: &'a mut Vec<FunctionSig>,
    references: &'a mut Vec<FunctionRef>,
    declared: &'a mut FxHashSet<(PathBuf, crate::ir::Name)>,
}

pub(super) fn prepare(
    db: &crate::db::CompilerDb,
    file_path: &Path,
    parse: &crate::parser::ParseOutput,
    typed: &TypeCheckResult,
    pool: &Pool,
    interner: &crate::ir::StringInterner,
) -> Result<PreparedImports, LoadError> {
    let resolved = crate::imports::resolve_imports(db, parse, file_path);
    let LoadedModules {
        modules,
        type_results,
        canons,
        pools,
        target_maps,
        root_targets,
        explicit_len,
        prelude_index,
    } = modules::load(db, parse, &resolved, interner)?;

    let mut merged_pool = pool.clone();
    let mut caches = vec![FxHashMap::default(); pools.len()];
    let mut var_remaps = vec![FxHashMap::default(); pools.len()];
    let mut re_interned_canons = modules::remap_canons(
        &canons,
        &pools,
        &mut merged_pool,
        &mut caches,
        &mut var_remaps,
    );

    let sources = ImportSources {
        resolved: &resolved,
        modules: &modules,
        type_results: &type_results,
        pools: &pools,
        target_maps: &target_maps,
        explicit_len,
        prelude_index,
    };
    let mut state = ReinternState {
        pool: &mut merged_pool,
        caches: &mut caches,
        var_remaps: &mut var_remaps,
    };
    let mut function_refs = Vec::new();
    let mut signatures = Vec::new();
    let mut declared = FxHashSet::default();
    let mut inventory = FunctionInventory {
        signatures: &mut signatures,
        references: &mut function_refs,
        declared: &mut declared,
    };

    collect_module_functions(&sources, &mut state, &mut inventory);

    let mut generic_signatures = collect_generic_signatures(&sources, &mut state);
    register_prelude(&sources, &mut state, &mut generic_signatures);
    let impl_templates = collect_impl_templates(&sources, &mut state, interner);
    let mono_functions = build_imported_mono_functions(
        typed,
        &generic_signatures,
        &impl_templates,
        state.caches,
        state.pool,
        interner,
    );
    let mut generic_templates: Vec<_> = generic_signatures
        .iter()
        .map(|(&local_name, (signature, module_index, source_name))| {
            crate::realization::ImportedGenericTemplate {
                local_name,
                signature: signature.clone(),
                module_index: *module_index,
                source_name: *source_name,
                generic_type_params: crate::realization::generic_type_param_map(
                    &type_results[*module_index].typed.types,
                ),
            }
        })
        .collect();
    generic_templates.sort_by_key(|template| template.local_name.raw());
    let renamed_functions =
        rename_aliased_functions(&modules, &function_refs, &mut re_interned_canons);

    Ok(PreparedImports {
        modules,
        type_results,
        pool: merged_pool,
        canons: re_interned_canons,
        function_refs,
        signatures,
        renamed_functions,
        mono_functions,
        generic_templates,
        target_maps,
        root_targets,
    })
}

fn collect_module_functions(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    inventory: &mut FunctionInventory<'_>,
) {
    for (module_index, module) in sources.modules[..sources.explicit_len].iter().enumerate() {
        let typed = &sources.type_results[module_index];
        for (func_index, function) in module.parse_output.module.functions.iter().enumerate() {
            if !inventory
                .declared
                .insert((module.module_path.clone(), function.name))
            {
                continue;
            }
            let Some(signature) = typed
                .typed
                .functions
                .iter()
                .find(|signature| signature.name == function.name)
            else {
                continue;
            };
            if signature.requires_specialization() {
                continue;
            }
            let mut signature = ori_types::re_intern_sig_with_var_remap(
                signature,
                &sources.pools[module_index],
                state.pool,
                &mut state.caches[module_index],
                &mut state.var_remaps[module_index],
            );
            let Some(&target) = sources.target_maps[module_index].get(&function.name) else {
                continue;
            };
            signature.name = target;
            inventory.signatures.push(signature);
            inventory.references.push(FunctionRef {
                func_index,
                module_index,
                local_name: target,
                original_name: function.name,
            });
        }
    }
}

fn collect_generic_signatures(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
) -> FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> {
    let mut signatures = FxHashMap::default();
    for (module_index, module) in sources.modules[..sources.explicit_len].iter().enumerate() {
        for function in &module.parse_output.module.functions {
            let Some(signature) = sources.type_results[module_index]
                .typed
                .functions
                .iter()
                .find(|signature| signature.name == function.name)
            else {
                continue;
            };
            if !signature.requires_specialization() {
                continue;
            }
            let mut re_interned = ori_types::re_intern_sig_with_var_remap(
                signature,
                &sources.pools[module_index],
                state.pool,
                &mut state.caches[module_index],
                &mut state.var_remaps[module_index],
            );
            let Some(&target) = sources.target_maps[module_index].get(&function.name) else {
                continue;
            };
            re_interned.name = target;
            signatures.insert(target, (re_interned, module_index, function.name));
        }
    }

    // Root mono instances retain the importer-facing spelling until ARC target
    // closure rewrites their call sites. Keep that spelling as a lookup alias
    // so existing direct imported generic demands still materialize eagerly.
    for imported in &sources.resolved.imported_functions {
        if imported.is_module_alias {
            continue;
        }
        let Some(module_index) = flattened_module_index(sources, imported.module_index) else {
            continue;
        };
        let Some(&target) = sources.target_maps[module_index].get(&imported.original_name) else {
            continue;
        };
        let Some((signature, _, source_name)) = signatures.get(&target).cloned() else {
            continue;
        };
        signatures
            .entry(imported.local_name)
            .or_insert((signature, module_index, source_name));
    }
    signatures
}

fn flattened_module_index(
    sources: &ImportSources<'_>,
    direct_module_index: usize,
) -> Option<usize> {
    let path = crate::imports::normalize_path(
        &sources
            .resolved
            .modules
            .get(direct_module_index)?
            .module_path,
    );
    sources.modules[..sources.explicit_len]
        .iter()
        .position(|module| crate::imports::normalize_path(&module.module_path) == path)
}

fn register_prelude(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    signatures: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
) {
    let (Some(prelude), Some(module_index)) =
        (sources.resolved.prelude.as_ref(), sources.prelude_index)
    else {
        return;
    };
    let source_pool = Arc::clone(&sources.pools[module_index]);
    register_prelude_generic_sigs(
        signatures,
        crate::commands::ImportedPreludeSource {
            parse: &prelude.parse_output,
            typed: &sources.type_results[module_index].typed,
            source_pool: &source_pool,
            module_index,
        },
        crate::commands::PoolReinternState {
            merged_pool: state.pool,
            cache: &mut state.caches[module_index],
            var_remap: &mut state.var_remaps[module_index],
        },
    );
    for function in &prelude.parse_output.module.functions {
        let Some(&target) = sources.target_maps[module_index].get(&function.name) else {
            continue;
        };
        let Some((mut signature, source_index, source_name)) =
            signatures.get(&function.name).cloned()
        else {
            continue;
        };
        signature.name = target;
        signatures
            .entry(target)
            .or_insert((signature, source_index, source_name));
    }
}

fn collect_impl_templates(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    interner: &crate::ir::StringInterner,
) -> Vec<crate::commands::ImportedImplTemplate> {
    let mut templates = Vec::new();
    for (module_index, module) in sources.modules[..sources.explicit_len].iter().enumerate() {
        let source_pool = Arc::clone(&sources.pools[module_index]);
        templates.extend(crate::commands::collect_imported_impl_templates(
            crate::commands::ImportedImplTemplateSource {
                parse: &module.parse_output,
                typed: &sources.type_results[module_index].typed,
                source_pool: &source_pool,
                module_index,
                module_identity: &module.module_path.to_string_lossy(),
            },
            state.pool,
            &mut state.caches[module_index],
            &mut state.var_remaps[module_index],
            interner,
        ));
    }
    templates
}

fn rename_aliased_functions(
    modules: &[crate::imports::ResolvedImportedModule],
    references: &[FunctionRef],
    canons: &mut [ori_ir::canon::CanonResult],
) -> Vec<Option<crate::ir::Function>> {
    references
        .iter()
        .map(|reference| {
            if reference.local_name == reference.original_name {
                return None;
            }
            let canon = &mut canons[reference.module_index];
            if canon.root_for(reference.local_name).is_none() {
                if let Some(mut root) = canon
                    .roots
                    .iter()
                    .find(|root| root.name == reference.original_name)
                    .cloned()
                {
                    root.name = reference.local_name;
                    canon.roots.push(root);
                }
            }
            let mut function = modules[reference.module_index]
                .parse_output
                .module
                .functions[reference.func_index]
                .clone();
            function.name = reference.local_name;
            Some(function)
        })
        .collect()
}

pub(super) fn for_codegen<'a>(
    modules: &'a [crate::imports::ResolvedImportedModule],
    references: &[FunctionRef],
    renamed_functions: &'a [Option<crate::ir::Function>],
    signatures: &[FunctionSig],
    canons: &'a [ori_ir::canon::CanonResult],
) -> Vec<ImportedFunctionForCodegen<'a>> {
    references
        .iter()
        .enumerate()
        .map(|(signature_index, reference)| {
            let parse = &modules[reference.module_index].parse_output;
            let function = renamed_functions[signature_index]
                .as_ref()
                .unwrap_or(&parse.module.functions[reference.func_index]);
            ImportedFunctionForCodegen {
                function,
                sig: signatures[signature_index].clone(),
                canon: &canons[reference.module_index],
            }
        })
        .collect()
}
