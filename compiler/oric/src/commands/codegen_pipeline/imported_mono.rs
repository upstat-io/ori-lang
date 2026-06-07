//! Build imported monomorphization `MonoFunction` structs for the AOT path.
//!
//! Promoted from the JIT test-runner equivalent at
//! `oric/src/test/runner/imported_mono.rs`. The production AOT pipeline
//! (`compile_to_llvm_with_imports` → `run_codegen_pipeline` →
//! `run_borrow_inference`) and the JIT test runner share the same carrier
//! shape via the body-imported AOT dispatch path.
//!
//! For each `MonoInstance` in the host module's type-check output that
//! references an imported generic, construct the concrete `MonoFunction` —
//! mangled name, concrete sig, fresh `body_type_map` — keyed to merged-pool
//! `Idx` values. The caller owns the merged pool; this function mutates it
//! via `build_mono_body_type_map` (which pre-interns scheme-var `BoundVar`
//! entries per the type pool's re-intern contract).

#[cfg(feature = "llvm")]
use rustc_hash::FxHashMap;

#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, GenericArg, Idx, MonoInstance, Pool, TypeCheckResult};

/// Tuple returned for each resolved imported monomorphization.
///
/// - `MonoFunction.original_name` is the LOCAL/aliased name (for call-site
///   dispatch in the host ARC IR).
/// - `module_index` identifies which imported module owns the source body.
/// - `source_body_name` is the function's name in the SOURCE module
///   (for `canon.root_for()` lookup in the imported canon).
#[cfg(feature = "llvm")]
pub(crate) type ImportedMonoFn = (ori_llvm::monomorphize::MonoFunction, usize, ori_ir::Name);

/// Borrowed view over the imported-generic codegen surfaces produced by the
/// host's merged-pool re-interning (`ImportedMonoState`).
///
/// Threads as ONE parameter through `compile_to_llvm_with_imported_monos` /
/// `compile_to_llvm_with_imports` → `run_codegen_pipeline` →
/// `run_borrow_inference`. The merged pool stays a separate parameter — its
/// `'ctx` lifetime ties to the LLVM context.
#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
pub(crate) struct ImportedSurfaces<'a> {
    /// `(MonoFunction, source_module_idx, source_body_name)` triples — one
    /// entry per unique imported mono instance.
    pub(crate) imported_mono_fns: &'a [ImportedMonoFn],
    /// Per-imported-module canons re-interned into merged-pool coordinates;
    /// indexed by `imported_mono_fns[i].1`.
    pub(crate) re_interned_canons: &'a [ori_ir::canon::CanonResult],
}

/// Build imported monomorphization functions for the AOT path.
///
/// Returns one entry per unique imported mono instance. Dedups by mangled
/// name; skips instances whose `fn_name` is not in `imported_generic_sigs`.
#[cfg(feature = "llvm")]
pub(crate) fn build_imported_mono_functions(
    type_result: &TypeCheckResult,
    imported_generic_sigs: &FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    // Why: caches retained for signature stability; var-id watermark sourced from `Pool::next_var_id`.
    _per_module_caches: &[FxHashMap<Idx, Idx>],
    merged_pool: &mut Pool,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedMonoFn> {
    let mut imported_mono_fns: Vec<ImportedMonoFn> = Vec::new();
    let mut name_to_index: FxHashMap<ori_ir::Name, usize> = FxHashMap::default();

    #[expect(
        clippy::cast_possible_truncation,
        reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
    )]
    for (idx, instance) in type_result.typed.mono_instances.iter().enumerate() {
        let instance_id = ori_ir::canon::MonoInstanceId(idx as u32);
        let Some((generic_sig, module_idx, source_original_name)) =
            imported_generic_sigs.get(&instance.fn_name)
        else {
            continue;
        };

        let mangled = ori_llvm::monomorphize::mangle_mono_name(
            instance.fn_name,
            &instance.generic_args,
            &instance.impl_args,
            &instance.method_args,
            interner,
            merged_pool,
        );
        if let Some(&existing) = name_to_index.get(&mangled) {
            imported_mono_fns[existing].0.instance_ids.push(instance_id);
            continue;
        }

        let concrete_sig = ori_llvm::monomorphize::concrete_sig_for_instance(
            instance,
            generic_sig,
            merged_pool,
            mangled,
        );
        let body_type_map = build_body_type_map(merged_pool, instance, generic_sig);

        name_to_index.insert(mangled, imported_mono_fns.len());
        imported_mono_fns.push((
            ori_llvm::monomorphize::MonoFunction {
                mangled_name: mangled,
                // Use LOCAL name for call-site dispatch (the host ARC IR
                // calls e.g. `ae`, not `assert_eq`, when using aliased imports).
                original_name: instance.fn_name,
                sig: concrete_sig,
                body_type_map,
                instance_ids: vec![instance_id],
                is_imported: true,
            },
            *module_idx,
            // Source body name for canon.root_for() lookup in imported canon.
            *source_original_name,
        ));
    }

    imported_mono_fns
}

/// Build the `body_type_map` (`generic_idx` → `concrete_idx`) for one mono
/// instance on the merged pool.
///
/// Steps:
/// 1. Seed `var_subst` from `scheme_var_ids` × `generic_args`.
/// 2. Ensure the merged pool has `VarState` capacity for every `var_id`
///    we may follow during substitution — `substitute_in_pool` panics on
///    out-of-bounds `var_id`s. The merged pool's own `next_var_id()` is
///    the authoritative upper bound; `ensure_var_capacity` is a no-op
///    when the pool is already sized.
/// 3. Extend `var_subst` with union-find root `var_id`s via the shared
///    SSOT helper. Mirrors the eager-path site in
///    `ori_types::check::exports::resolve_deferred_mono_calls` and the
///    deferred-path site in
///    `ori_types::infer::expr::calls::monomorphization::maybe_record_mono_instance`.
/// 4. Delegate to `build_mono_body_type_map` — `FxHashMap` sink variant
///    for LLVM-side `MonoFunction` consumption. The helper handles the
///    `HAS_VAR|HAS_BOUND_VAR` mask + scheme-var `BoundVar` pre-intern
///    contract.
#[cfg(feature = "llvm")]
fn build_body_type_map(
    merged_pool: &mut Pool,
    instance: &MonoInstance,
    generic_sig: &FunctionSig,
) -> FxHashMap<Idx, Idx> {
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    for (i, &var_id) in generic_sig.scheme_var_ids.iter().enumerate() {
        if let Some(GenericArg::Type(concrete)) = instance.generic_args.get(i) {
            var_subst.insert(var_id, *concrete);
        }
    }

    // Why: `Pool::next_var_id` is the SSOT for the var_id watermark; re-deriving
    // via cache scan would scatter the lookup. `ensure_var_capacity` is idempotent
    // and guards against future re-intern paths that omit capacity extension.
    let watermark = merged_pool.next_var_id();
    merged_pool.ensure_var_capacity(watermark);

    ori_types::extend_var_subst_with_roots(
        merged_pool,
        &generic_sig.scheme_var_ids,
        &mut var_subst,
    );

    let mut body_type_map: FxHashMap<Idx, Idx> = FxHashMap::default();
    ori_types::build_mono_body_type_map(merged_pool, &var_subst, &mut body_type_map);
    body_type_map
}

#[cfg(test)]
#[cfg(feature = "llvm")]
mod tests;
