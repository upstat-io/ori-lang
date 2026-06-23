//! Build imported monomorphization `MonoFunction` structs for the AOT path.
//!
//! For each `MonoInstance` in the host module's type-check output that
//! references an imported generic, construct the concrete `MonoFunction` —
//! mangled name, concrete sig, fresh `body_type_map` — keyed to merged-pool
//! `Idx` values. The caller owns the merged pool; this module mutates it
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
            instance.receiver_type,
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
                // Cross-module imported methods are out of scope here; imported
                // monos today are top-level free functions (free-function body
                // namespace).
                receiver_type_name: None,
            },
            *module_idx,
            // Source body name for canon.root_for() lookup in imported canon.
            *source_original_name,
        ));
    }

    imported_mono_fns
}

/// Register every `pub` generic free function of the prelude module into
/// `imported_generic_sigs`, keyed by its source name.
///
/// Prelude generic free functions (`min`, `max`, …) are implicitly available
/// in every module but are NOT explicit `ImportedFunctionRef` entries, so the
/// explicit-import registration loop never sees them. Without this, their
/// recorded `MonoInstance`s find no generic sig in `build_imported_mono_functions`
/// and the call site emits `unresolved function ... missing mono instance`.
///
/// `prelude_module_index` is the prelude's slot in the per-module re-interning
/// arrays (`per_module_caches[prelude_module_index]` / `*_var_remaps`). The
/// caller owns the merged pool; the sig is re-interned into it via the shared
/// per-module var-remap so scheme-var ids stay coherent with the prelude's
/// re-interned canon.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "prelude sig registration threads the same re-interning state the explicit-import loop uses"
)]
pub(crate) fn register_prelude_generic_sigs(
    imported_generic_sigs: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    prelude_parse: &crate::parser::ParseOutput,
    prelude_typed: &ori_types::TypedModule,
    prelude_source_pool: &Pool,
    prelude_module_index: usize,
    merged_pool: &mut Pool,
    per_module_cache: &mut FxHashMap<Idx, Idx>,
    per_module_var_remap: &mut FxHashMap<u32, u32>,
) {
    for func in &prelude_parse.module.functions {
        let Some(sig) = prelude_typed.functions.iter().find(|s| s.name == func.name) else {
            continue;
        };
        if !sig.is_generic() {
            continue;
        }
        // First registration wins — an explicit import shadowing a prelude name
        // (e.g. `use mymod { min }`) already registered the explicit sig.
        if imported_generic_sigs.contains_key(&func.name) {
            continue;
        }
        let re_interned = ori_types::re_intern_sig_with_var_remap(
            sig,
            prelude_source_pool,
            merged_pool,
            per_module_cache,
            per_module_var_remap,
        );
        imported_generic_sigs.insert(func.name, (re_interned, prelude_module_index, func.name));
    }
}

/// Build the `body_type_map` (`generic_idx` → `concrete_idx`) for one mono
/// instance on the merged pool.
///
/// Seeds `var_subst` from `scheme_var_ids` × `generic_args`, extends it with
/// union-find root var ids via the shared SSOT helper, and delegates to
/// `build_mono_body_type_map` (which handles the `HAS_VAR|HAS_BOUND_VAR`
/// mask + scheme-var `BoundVar` pre-intern contract). Var capacity is
/// ensured up front because `substitute_in_pool` panics on out-of-bounds ids.
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
