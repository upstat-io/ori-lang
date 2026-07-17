//! Function-signature alignment and imported metadata aggregation.

use ori_types::{FunctionSig, TypeCheckResult};

use crate::ir::Name;
use crate::parser::ParseOutput;

/// Build function signatures aligned with `module.functions` source order.
///
/// `typed.functions` is sorted by name (for Salsa determinism), while
/// `module.functions` is in source order. `FunctionCompiler::declare_all`
/// zips them, so they must be aligned.
#[cfg_attr(
    not(feature = "llvm"),
    expect(
        dead_code,
        reason = "consumed by #[cfg(feature = \"llvm\")] paths in compile_common and test runner"
    )
)]
pub(crate) fn build_function_sigs(
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
) -> Vec<FunctionSig> {
    let sig_map: rustc_hash::FxHashMap<Name, &FunctionSig> = type_result
        .typed
        .functions
        .iter()
        .map(|ft| (ft.name, ft))
        .collect();

    parse_result
        .module
        .functions
        .iter()
        .map(|func| {
            sig_map
                .get(&func.name)
                .copied()
                .cloned()
                .unwrap_or_else(|| dummy_sig(func.name))
        })
        .collect()
}

/// Fallback signature for functions missing from the type check result.
///
/// Should never be reached after successful type checking — only exists to
/// prevent panics if the signature map is incomplete.
#[cold]
fn dummy_sig(name: Name) -> FunctionSig {
    use ori_types::Idx;

    debug_assert!(false, "function {name:?} has no type-checked signature");
    tracing::warn!(
        name = ?name,
        "function missing from type check result — using dummy signature"
    );
    FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 0,
        param_defaults: vec![],
        param_hashes: vec![],
        return_hash: 0,
        return_projection: None,
    }
}

/// Collect exported type metadata from prelude and imported module results.
///
/// Gathers `ExportedTypeMetadata` from the prelude (if present) and all
/// imported module `TypeCheckResult` objects for transitive forwarding.
pub(crate) fn collect_metadata_from_results(
    prelude: Option<&ori_types::TypeCheckResult>,
    module_results: &[Option<ori_types::TypeCheckResult>],
) -> Vec<ori_types::ExportedTypeMetadata> {
    let mut metadata = Vec::new();
    if let Some(tcr) = prelude {
        metadata.extend(tcr.typed.exported_type_metadata.iter().cloned());
    }
    for tcr in module_results.iter().flatten() {
        metadata.extend(tcr.typed.exported_type_metadata.iter().cloned());
    }
    metadata
}

/// Collect exported collection surface hashes from prelude and imported module results.
///
/// Gathers merkle hashes of collection types (List, Set) from the prelude (if
/// present) and all imported module `TypeCheckResult` objects for transitive
/// forwarding. The collected hashes are passed to `ModuleChecker::set_imported_collection_surfaces()`
/// which feeds them into `generate_exported_collection_surfaces()` for A→B→C propagation.
pub(crate) fn collect_surfaces_from_results(
    prelude: Option<&ori_types::TypeCheckResult>,
    module_results: &[Option<ori_types::TypeCheckResult>],
) -> Vec<u64> {
    let mut surfaces = Vec::new();
    if let Some(tcr) = prelude {
        surfaces.extend(tcr.typed.exported_collection_surfaces.iter().copied());
    }
    for tcr in module_results.iter().flatten() {
        surfaces.extend(tcr.typed.exported_collection_surfaces.iter().copied());
    }
    surfaces
}
