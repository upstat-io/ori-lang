//! Thin wrapper around the production `build_imported_mono_functions` for the
//! LLVM JIT test backend.
//!
//! Forwards to the shared body-imported AOT path at
//! `crate::commands::codegen_pipeline::imported_mono`, so JIT and AOT use a
//! single monomorphization mechanism rather than parallel implementations.

use rustc_hash::FxHashMap;

use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult};

/// Re-export the production carrier type so existing test-runner callers
/// can keep referring to `super::imported_mono::ImportedMonoFn`.
pub(super) use crate::commands::ImportedMonoFn;

/// Build imported monomorphization functions for the LLVM JIT backend.
///
/// Forwards to the production `build_imported_mono_functions` in
/// `crate::commands::codegen_pipeline::imported_mono` via the
/// `crate::commands::build_imported_mono_functions_for_test_runner` re-export
/// (bridges the private `mod codegen_pipeline;` boundary).
pub(super) fn build_imported_mono_functions(
    type_result: &TypeCheckResult,
    imported_generic_sigs: &FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    imported_impl_templates: &[crate::commands::ImportedImplTemplate],
    per_module_caches: &[FxHashMap<Idx, Idx>],
    merged_pool: &mut Pool,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedMonoFn> {
    crate::commands::build_imported_mono_functions_for_test_runner(
        type_result,
        imported_generic_sigs,
        imported_impl_templates,
        per_module_caches,
        merged_pool,
        interner,
    )
}

/// Register the prelude's `pub` generic free functions into
/// `imported_generic_sigs` for the LLVM JIT backend.
///
/// Forwards to the production `register_prelude_generic_sigs` via the
/// `crate::commands` re-export.
pub(super) fn register_prelude_generic_sigs(
    imported_generic_sigs: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    source: crate::commands::ImportedPreludeSource<'_>,
    state: crate::commands::PoolReinternState<'_>,
) {
    crate::commands::register_prelude_generic_sigs_for_test_runner(
        imported_generic_sigs,
        source,
        state,
    );
}
