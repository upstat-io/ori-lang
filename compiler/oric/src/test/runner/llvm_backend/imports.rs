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

use modules::LoadedModules;

pub(super) struct PreparedImports {
    pub(super) resolved: Arc<crate::imports::ResolvedImports>,
    pub(super) type_results: Vec<TypeCheckResult>,
    pub(super) pool: Pool,
    pub(super) canons: Vec<ori_ir::canon::CanonResult>,
    pub(super) function_refs: Vec<FunctionRef>,
    pub(super) signatures: Vec<FunctionSig>,
    pub(super) renamed_functions: Vec<Option<crate::ir::Function>>,
    pub(super) mono_functions: Vec<ImportedMonoFn>,
}

pub(super) struct FunctionRef {
    func_index: usize,
    module_index: usize,
    local_name: crate::ir::Name,
    original_name: crate::ir::Name,
}

struct ImportSources<'a> {
    resolved: &'a crate::imports::ResolvedImports,
    type_results: &'a [TypeCheckResult],
    pools: &'a [Arc<Pool>],
}

struct ReinternState<'a> {
    pool: &'a mut Pool,
    caches: &'a mut [FxHashMap<Idx, Idx>],
    var_remaps: &'a mut [FxHashMap<u32, u32>],
}

struct FunctionInventory<'a> {
    signatures: &'a mut Vec<FunctionSig>,
    references: &'a mut Vec<FunctionRef>,
    declared: &'a mut FxHashSet<(PathBuf, usize, crate::ir::Name)>,
}

pub(super) fn prepare(
    db: &crate::db::CompilerDb,
    file_path: &Path,
    parse: &crate::parser::ParseOutput,
    typed: &TypeCheckResult,
    pool: &Pool,
    interner: &crate::ir::StringInterner,
) -> PreparedImports {
    let resolved = crate::imports::resolve_imports(db, parse, file_path);
    let LoadedModules {
        type_results,
        canons,
        pools,
    } = modules::load(db, &resolved);

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
        type_results: &type_results,
        pools: &pools,
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

    collect_direct_functions(parse, &sources, &mut state, &mut inventory);
    collect_module_alias_internals(&sources, &mut state, &mut inventory);

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
    let renamed_functions =
        rename_aliased_functions(&resolved, &function_refs, &mut re_interned_canons);

    PreparedImports {
        resolved,
        type_results,
        pool: merged_pool,
        canons: re_interned_canons,
        function_refs,
        signatures,
        renamed_functions,
        mono_functions,
    }
}

fn collect_direct_functions(
    parse: &crate::parser::ParseOutput,
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    inventory: &mut FunctionInventory<'_>,
) {
    for imported in &sources.resolved.imported_functions {
        if imported.is_module_alias
            || parse
                .module
                .functions
                .iter()
                .any(|function| function.name == imported.local_name)
        {
            continue;
        }
        let module = &sources.resolved.modules[imported.module_index];
        let Some((func_index, _)) = module
            .parse_output
            .module
            .functions
            .iter()
            .enumerate()
            .find(|(_, function)| function.name == imported.original_name)
        else {
            continue;
        };
        if !inventory
            .declared
            .insert((module.module_path.clone(), func_index, imported.local_name))
        {
            continue;
        }
        let Some(signature) = sources.type_results[imported.module_index]
            .typed
            .functions
            .iter()
            .find(|signature| signature.name == imported.original_name)
        else {
            continue;
        };
        if signature.is_generic() {
            continue;
        }

        let mut re_interned = ori_types::re_intern_sig_with_var_remap(
            signature,
            &sources.pools[imported.module_index],
            state.pool,
            &mut state.caches[imported.module_index],
            &mut state.var_remaps[imported.module_index],
        );
        re_interned.name = imported.local_name;
        inventory.signatures.push(re_interned);
        inventory.references.push(FunctionRef {
            func_index,
            module_index: imported.module_index,
            local_name: imported.local_name,
            original_name: imported.original_name,
        });
    }
}

/// Module-alias bodies call same-module functions by their original names, so
/// codegen needs declarations beyond the importer-facing `alias.function` set.
fn collect_module_alias_internals(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    inventory: &mut FunctionInventory<'_>,
) {
    for imported in &sources.resolved.imported_functions {
        if !imported.is_module_alias {
            continue;
        }
        let module_index = imported.module_index;
        let module = &sources.resolved.modules[module_index];
        let typed = &sources.type_results[module_index];
        for (func_index, function) in module.parse_output.module.functions.iter().enumerate() {
            if !inventory
                .declared
                .insert((module.module_path.clone(), func_index, function.name))
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
            if signature.is_generic() {
                continue;
            }
            let signature = ori_types::re_intern_sig_with_var_remap(
                signature,
                &sources.pools[module_index],
                state.pool,
                &mut state.caches[module_index],
                &mut state.var_remaps[module_index],
            );
            inventory.signatures.push(signature);
            inventory.references.push(FunctionRef {
                func_index,
                module_index,
                local_name: function.name,
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
    for imported in &sources.resolved.imported_functions {
        if imported.is_module_alias {
            continue;
        }
        let Some(signature) = sources.type_results[imported.module_index]
            .typed
            .functions
            .iter()
            .find(|signature| signature.name == imported.original_name)
        else {
            continue;
        };
        if !signature.is_generic() {
            continue;
        }
        let re_interned = ori_types::re_intern_sig_with_var_remap(
            signature,
            &sources.pools[imported.module_index],
            state.pool,
            &mut state.caches[imported.module_index],
            &mut state.var_remaps[imported.module_index],
        );
        signatures.insert(
            imported.local_name,
            (re_interned, imported.module_index, imported.original_name),
        );
    }
    signatures
}

fn register_prelude(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    signatures: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
) {
    let Some(prelude) = sources.resolved.prelude.as_ref() else {
        return;
    };
    let module_index = sources.resolved.modules.len();
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
}

fn collect_impl_templates(
    sources: &ImportSources<'_>,
    state: &mut ReinternState<'_>,
    interner: &crate::ir::StringInterner,
) -> Vec<crate::commands::ImportedImplTemplate> {
    let mut templates = Vec::new();
    for (module_index, module) in sources.resolved.modules.iter().enumerate() {
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
    resolved: &crate::imports::ResolvedImports,
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
            let mut function = resolved.modules[reference.module_index]
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
    resolved: &'a crate::imports::ResolvedImports,
    references: &[FunctionRef],
    renamed_functions: &'a [Option<crate::ir::Function>],
    signatures: &[FunctionSig],
    canons: &'a [ori_ir::canon::CanonResult],
) -> Vec<ImportedFunctionForCodegen<'a>> {
    references
        .iter()
        .enumerate()
        .map(|(signature_index, reference)| {
            let parse = &resolved.modules[reference.module_index].parse_output;
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
