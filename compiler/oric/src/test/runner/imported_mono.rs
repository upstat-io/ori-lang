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
    per_module_caches: &[FxHashMap<Idx, Idx>],
    merged_pool: &mut Pool,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedMonoFn> {
    crate::commands::build_imported_mono_functions_for_test_runner(
        type_result,
        imported_generic_sigs,
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
#[expect(
    clippy::too_many_arguments,
    reason = "forwards the production sig-registration surface verbatim"
)]
pub(super) fn register_prelude_generic_sigs(
    imported_generic_sigs: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    prelude_parse: &crate::parser::ParseOutput,
    prelude_typed: &ori_types::TypedModule,
    prelude_source_pool: &Pool,
    prelude_module_index: usize,
    merged_pool: &mut Pool,
    per_module_cache: &mut FxHashMap<Idx, Idx>,
    per_module_var_remap: &mut FxHashMap<u32, u32>,
) {
    crate::commands::register_prelude_generic_sigs_for_test_runner(
        imported_generic_sigs,
        prelude_parse,
        prelude_typed,
        prelude_source_pool,
        prelude_module_index,
        merged_pool,
        per_module_cache,
        per_module_var_remap,
    );
}
