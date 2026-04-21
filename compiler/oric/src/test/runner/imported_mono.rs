//! Build imported monomorphization `MonoFunction` structs for the LLVM JIT
//! test backend.
//!
//! For each `MonoInstance` in the test file's type-check output that
//! references an imported generic, construct the concrete
//! `MonoFunction` — mangled name, concrete sig, fresh `body_type_map` —
//! keyed to merged-pool `Idx` values. The caller owns the merged pool; this
//! function mutates it via `build_mono_body_type_map` (which pre-interns
//! scheme-var `BoundVar` entries per §08.3b.1).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::TypeCheckResult;

/// Build imported monomorphization functions for the LLVM JIT backend.
///
/// Returns `Vec<(MonoFunction, module_index, source_body_name)>`:
///
/// - `MonoFunction.original_name` is the LOCAL/aliased name (for call-site
///   dispatch in the test's ARC IR).
/// - `source_body_name` is the function's name in the SOURCE module
///   (for `canon.root_for()` lookup in the imported canon).
pub(super) fn build_imported_mono_functions(
    type_result: &TypeCheckResult,
    imported_generic_sigs: &FxHashMap<ori_ir::Name, (ori_types::FunctionSig, usize, ori_ir::Name)>,
    per_module_caches: &[FxHashMap<ori_types::Idx, ori_types::Idx>],
    merged_pool: &mut ori_types::Pool,
    interner: &crate::ir::StringInterner,
) -> Vec<(ori_llvm::monomorphize::MonoFunction, usize, ori_ir::Name)> {
    let mut imported_mono_fns: Vec<(ori_llvm::monomorphize::MonoFunction, usize, ori_ir::Name)> =
        Vec::new();
    let mut seen_mono_names = FxHashSet::default();

    for instance in &type_result.typed.mono_instances {
        let Some((generic_sig, module_idx, source_original_name)) =
            imported_generic_sigs.get(&instance.fn_name)
        else {
            continue;
        };
        let mangled = ori_llvm::monomorphize::mangle_mono_name(
            instance.fn_name,
            &instance.generic_args,
            interner,
            merged_pool,
        );
        if !seen_mono_names.insert(mangled) {
            continue;
        }

        // Build concrete sig (same pattern as collect_mono_functions)
        let param_hashes: Vec<u64> = instance
            .concrete_param_types
            .iter()
            .map(|&idx| merged_pool.hash(idx))
            .collect();
        let return_hash = merged_pool.hash(instance.concrete_return_type);
        let concrete_sig = ori_types::FunctionSig {
            name: mangled,
            type_params: vec![],
            const_params: vec![],
            param_names: generic_sig.param_names.clone(),
            param_types: instance.concrete_param_types.clone(),
            return_type: instance.concrete_return_type,
            capabilities: generic_sig.capabilities.clone(),
            is_public: false,
            is_test: false,
            is_main: false,
            is_fbip: generic_sig.is_fbip,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            scheme_var_ids: vec![],
            required_params: generic_sig.required_params,
            param_defaults: generic_sig.param_defaults.clone(),
            param_hashes,
            return_hash,
        };

        // Build fresh body_type_map from re-interned types.
        // scheme_var_ids are u32 var_ids preserved by re-interning.
        // Iterate per_module_cache values only (scoped to imported types,
        // avoiding var_id collisions with test file types).
        let mut var_subst: FxHashMap<u32, ori_types::Idx> = FxHashMap::default();
        for (i, &var_id) in generic_sig.scheme_var_ids.iter().enumerate() {
            if let Some(ori_types::GenericArg::Type(concrete)) = instance.generic_args.get(i) {
                var_subst.insert(var_id, *concrete);
            }
        }
        // Ensure merged pool has var_states for imported var_ids.
        // Re-interned Vars carry source var_ids, but the merged pool's
        // var_states array was cloned from the test file's pool and may
        // not cover imported var_ids. substitute_in_pool follows links
        // via var_state(), which panics on out-of-bounds var_ids.
        let cache_values: Vec<ori_types::Idx> =
            per_module_caches[*module_idx].values().copied().collect();
        let max_imported_var_id = cache_values
            .iter()
            .filter(|&&idx| merged_pool.tag(idx) == ori_types::Tag::Var)
            .map(|&idx| merged_pool.data(idx))
            .max();
        if let Some(max_id) = max_imported_var_id {
            merged_pool.ensure_var_capacity(max_id + 1);
        }

        // Extend var_subst with union-find root var_ids so
        // build_mono_body_type_map can substitute raw Tag::Var leaves
        // whose var_id is the root rather than the declared scheme var.
        // Shared SSOT helper per impl-hygiene.md §Algorithmic DRY — also
        // invoked at the eager-path (infer::expr::calls::monomorphization)
        // and deferred-path (check::exports::resolve_deferred_mono_calls)
        // sites. Without this extension, imported generic JIT compilation
        // where the callee's scheme var is not the union-find representative
        // would silently miscompile (pre-§04.2) or fire the §04.2 PC-2 seam
        // assertion (post-§04.2) at codegen time.
        ori_types::extend_var_subst_with_roots(
            merged_pool,
            &generic_sig.scheme_var_ids,
            &mut var_subst,
        );

        // Build body_type_map via the canonical SSOT helper (typeck-
        // side), FxHashMap sink variant for LLVM-side MonoFunction
        // consumption. The helper handles the HAS_VAR|HAS_BOUND_VAR
        // mask + scheme-var BoundVar pre-intern per §08.3b.1.
        let mut body_type_map: FxHashMap<ori_types::Idx, ori_types::Idx> = FxHashMap::default();
        ori_types::build_mono_body_type_map(merged_pool, &var_subst, &mut body_type_map);

        imported_mono_fns.push((
            ori_llvm::monomorphize::MonoFunction {
                mangled_name: mangled,
                // Use LOCAL name for call-site dispatch (the test's ARC IR
                // calls `ae`, not `assert_eq`, when using aliased imports).
                original_name: instance.fn_name,
                sig: concrete_sig,
                body_type_map,
            },
            *module_idx,
            // Source body name for canon.root_for() lookup in imported canon
            *source_original_name,
        ));
    }

    imported_mono_fns
}
