//! Cross-module import-info gathering for the multi-file build pipeline.
//!
//! Resolves each module's imports against already-compiled sibling modules:
//! function signatures (for correct calling conventions), exported type
//! metadata (for `ReprPlan` narrowing exemptions), and exported collection
//! surface hashes (transitive metadata propagation).

use std::path::{Path, PathBuf};

use super::multi::{CompiledModuleInfo, ExportedFunctionInfo};

/// Resolve each direct import of `source_path` to its already-compiled
/// `CompiledModuleInfo`, in import order.
///
/// Shared iteration skeleton for `build_import_infos`,
/// `collect_imported_type_metadata`, and `collect_imported_collection_surfaces`.
/// `None` info marks an import missing from `compiled_modules` (topological
/// ordering normally guarantees presence); consumers decide whether to warn.
fn imported_module_infos<'a>(
    source_path: &Path,
    graph: &'a ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &'a [CompiledModuleInfo],
) -> Vec<(&'a Path, Option<&'a CompiledModuleInfo>)> {
    let Some(imports) = graph.get_imports(source_path) else {
        return Vec::new();
    };

    let module_index: rustc_hash::FxHashMap<&Path, &CompiledModuleInfo> = compiled_modules
        .iter()
        .map(|m| (m.path.as_path(), m))
        .collect();

    imports
        .iter()
        .map(|import_path| {
            let info = module_index.get(import_path.as_path()).copied();
            (import_path.as_path(), info)
        })
        .collect()
}

type LocalFunctionNames = rustc_hash::FxHashMap<(PathBuf, String), Vec<String>>;

fn collect_local_function_names(
    resolved_imports: &crate::imports::ResolvedImports,
    interner: &ori_ir::StringInterner,
) -> LocalFunctionNames {
    let mut local_names = LocalFunctionNames::default();
    for func_ref in &resolved_imports.imported_functions {
        if func_ref.is_module_alias {
            continue;
        }
        let Some(module) = resolved_imports.modules.get(func_ref.module_index) else {
            continue;
        };
        let key_path = module.module_path.canonicalize().unwrap_or_else(|error| {
            eprintln!(
                "warning: cannot canonicalize import '{}' for local-name resolution: {error}",
                module.module_path.display()
            );
            module.module_path.clone()
        });
        let local = interner.lookup(func_ref.local_name).to_string();
        let entry = local_names
            .entry((
                key_path,
                interner.lookup(func_ref.original_name).to_string(),
            ))
            .or_default();
        if !entry.contains(&local) {
            entry.push(local);
        }
    }
    local_names
}

fn collect_resolved_module_indices(
    resolved_imports: &crate::imports::ResolvedImports,
) -> rustc_hash::FxHashMap<PathBuf, usize> {
    resolved_imports
        .modules
        .iter()
        .enumerate()
        .map(|(index, module)| {
            let path = module.module_path.canonicalize().unwrap_or_else(|error| {
                eprintln!(
                    "warning: cannot canonicalize resolved module '{}': {error}",
                    module.module_path.display()
                );
                module.module_path.clone()
            });
            (path, index)
        })
        .collect()
}

/// Build import information for a module based on its dependencies.
///
/// Uses actual type information from already-compiled modules rather than
/// defaulting to INT. This ensures correct calling conventions for cross-module calls.
pub(super) fn build_import_infos(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
    resolved_imports: &crate::imports::ResolvedImports,
    re_interned_function_sigs: &[rustc_hash::FxHashMap<ori_ir::Name, ori_types::FunctionSig>],
    interner: &ori_ir::StringInterner,
) -> Result<Vec<crate::commands::compile_common::ImportedFunctionInfo>, String> {
    // Call-site local/aliased names keyed by (imported module path, exported
    // fn name). Only functions the host names in a `use` get local keys;
    // module-alias imports expand to qualified `alias.fn` entries upstream.
    // ONE exported fn can carry SEVERAL local names (`use { f as g, f as h }`,
    // or a named import plus a module-alias qualified entry) - every alias
    // needs its own registration or its call sites miss callee resolution.
    let local_names = collect_local_function_names(resolved_imports, interner);

    let mut imported_functions = Vec::new();
    let module_indices = collect_resolved_module_indices(resolved_imports);

    for (import_path, info) in imported_module_infos(source_path, graph, compiled_modules) {
        let Some(module_info) = info else {
            // Module not yet compiled - shouldn't happen in topological order
            eprintln!(
                "warning: import '{}' not found in compiled modules",
                import_path.display()
            );
            continue;
        };

        let key_path = import_path.canonicalize().unwrap_or_else(|e| {
            eprintln!(
                "warning: cannot canonicalize compiled module '{}' for local-name resolution: {e}",
                import_path.display()
            );
            import_path.to_path_buf()
        });
        let module_index = module_indices.get(&key_path).copied().ok_or_else(|| {
            format!(
                "compiled import '{}' has no matching resolved-module identity",
                import_path.display()
            )
        })?;
        let module_sigs = re_interned_function_sigs.get(module_index).ok_or_else(|| {
            format!(
                "compiled import '{}' has no merged-pool signature table",
                import_path.display()
            )
        })?;
        imported_functions.reserve(module_info.public_functions.len());
        for ExportedFunctionInfo {
            mangled_name,
            source_name,
            metadata,
            ..
        } in &module_info.public_functions
        {
            let source_name_id = interner.intern(source_name);
            let signature = module_sigs.get(&source_name_id).ok_or_else(|| {
                format!(
                    "compiled import '{}' exports '{}' without a merged-pool signature",
                    import_path.display(),
                    source_name
                )
            })?;
            match local_names.get(&(key_path.clone(), source_name.clone())) {
                Some(locals) => {
                    // One entry PER local alias - each call-site name resolves
                    // to the same extern symbol.
                    for local in locals {
                        imported_functions.push(
                            crate::commands::compile_common::ImportedFunctionInfo {
                                mangled_name: mangled_name.clone(),
                                local_name: Some(local.clone()),
                                param_types: signature.param_types.clone(),
                                return_type: signature.return_type,
                                metadata: metadata.clone(),
                            },
                        );
                    }
                }
                None => {
                    imported_functions.push(
                        crate::commands::compile_common::ImportedFunctionInfo {
                            mangled_name: mangled_name.clone(),
                            local_name: None,
                            param_types: signature.param_types.clone(),
                            return_type: signature.return_type,
                            metadata: metadata.clone(),
                        },
                    );
                }
            }
        }
    }

    Ok(imported_functions)
}

/// Collect exported type metadata from all modules this module imports.
///
/// Mirrors `build_import_infos()` but collects `ExportedTypeMetadata` instead
/// of function signatures. Enables `ReprPlan` to exempt imported `pub` and
/// `#repr(...)` types from integer narrowing.
pub(super) fn collect_imported_type_metadata(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
) -> Vec<ori_types::ExportedTypeMetadata> {
    let mut metadata = Vec::new();
    for (_, info) in imported_module_infos(source_path, graph, compiled_modules) {
        if let Some(module_info) = info {
            metadata.extend(module_info.exported_type_metadata.iter().cloned());
        }
    }
    metadata
}

/// Collect imported collection surface hashes from dependency modules.
///
/// Parallel to `collect_imported_type_metadata()` but collects merkle hashes
/// of collection types (List, Set) in imported public function signatures.
/// Forwarded for downstream metadata only (A→B→C transitive propagation);
/// imported surfaces do not suppress narrowing.
pub(super) fn collect_imported_collection_surfaces(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
) -> Vec<u64> {
    let mut surfaces = Vec::new();
    for (_, info) in imported_module_infos(source_path, graph, compiled_modules) {
        if let Some(module_info) = info {
            surfaces.extend(module_info.exported_collection_surfaces.iter().copied());
        }
    }
    surfaces
}
