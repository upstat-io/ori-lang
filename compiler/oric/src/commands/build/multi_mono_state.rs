//! Cross-module imported-generic monomorphization state.
//!
//! Builds the merged-pool re-interning state consumed by
//! `compile_to_llvm_with_imports` for cross-module mono dispatch: clones the
//! host pool, re-interns each imported module's canon arena + generic
//! signatures into it, and resolves the host's `MonoInstance` list into
//! `ImportedMonoFn` triples via the same SSOT builder the JIT test runner uses.

use std::path::Path;

use rustc_hash::FxHashMap;

/// State derived from cross-module imported-generic resolution.
///
/// Returned by `build_imported_mono_state` and consumed by
/// `compile_to_llvm_with_imports` for cross-module mono dispatch.
///
/// Carries merged-pool coordinates: every `Idx` in `imported_mono_fns.sig`
/// and `body_type_map`, plus every `TypeId` in entries of `re_interned_canons`,
/// is valid in `merged_pool`. The host module's own `canon_result` (built
/// against the original host pool) is ALSO valid in `merged_pool` because
/// the merged pool is cloned from the host pool — host `Idx` values are
/// preserved by clone.
pub(crate) struct ImportedMonoState {
    /// Merged pool — host pool with imported types re-interned alongside.
    pub(crate) merged_pool: ori_types::Pool,
    /// Per-imported-module re-interned canons, indexed by entry index in
    /// `resolved_imports.modules`. `imported_mono_fns[i].1` indexes this
    /// vector so `arc_lowering` can fetch the SOURCE canon for body
    /// specialization.
    pub(crate) re_interned_canons: Vec<ori_ir::canon::CanonResult>,
    /// `(MonoFunction, source_module_idx, source_body_name)` triples — one
    /// entry per unique imported mono instance reachable from the host's
    /// `MonoInstance` list. Empty when the host has no imported generic
    /// instantiations.
    pub(crate) imported_mono_fns: Vec<crate::commands::ImportedMonoFn>,
}

impl ImportedMonoState {
    /// Borrowed view threaded through the codegen pipeline as one parameter.
    /// `merged_pool` stays a separate borrow (`'ctx` lifetime).
    pub(crate) fn surfaces(&self) -> crate::commands::ImportedSurfaces<'_> {
        crate::commands::ImportedSurfaces {
            imported_mono_fns: &self.imported_mono_fns,
            re_interned_canons: &self.re_interned_canons,
        }
    }
}

/// Type-check + canonicalize every imported module returned by
/// `resolve_imports`, via the Salsa-cached query helpers.
///
/// Each returned vector is index-aligned with `resolved_imports.modules`;
/// modules whose type-check returns `None` (internal Pool cache miss) get
/// empty placeholders to preserve the alignment.
fn type_check_and_canonicalize_imports(
    db: &oric::CompilerDb,
    resolved_imports: &crate::imports::ResolvedImports,
) -> (
    Vec<ori_types::TypeCheckResult>,
    Vec<std::sync::Arc<ori_types::Pool>>,
    Vec<ori_ir::canon::SharedCanonResult>,
) {
    // Process the prelude as a trailing module slot so its `pub` generic free
    // functions (min/max/…) participate in imported-generic monomorphization
    // exactly like explicit imports. Its slot index is `modules.len()`.
    let all_import_modules: Vec<&crate::imports::ResolvedImportedModule> = resolved_imports
        .modules
        .iter()
        .chain(resolved_imports.prelude.as_ref())
        .collect();
    let count = all_import_modules.len();
    let mut imported_type_results: Vec<ori_types::TypeCheckResult> = Vec::with_capacity(count);
    let mut imported_pools: Vec<std::sync::Arc<ori_types::Pool>> = Vec::with_capacity(count);
    let mut imported_canon_results: Vec<ori_ir::canon::SharedCanonResult> =
        Vec::with_capacity(count);

    for &imp_module in &all_import_modules {
        let Some((imp_tc, imp_pool)) = oric::query::type_check_module(
            db,
            &imp_module.parse_output,
            &imp_module.module_path,
            imp_module.source_file,
        ) else {
            imported_type_results.push(ori_types::TypeCheckResult::ok(
                ori_types::TypedModule::default(),
            ));
            imported_pools.push(std::sync::Arc::new(ori_types::Pool::new()));
            imported_canon_results.push(ori_ir::canon::SharedCanonResult::new(
                ori_ir::canon::CanonResult::empty(),
            ));
            continue;
        };
        let imp_canon = oric::query::canonicalize_cached_by_path(
            db,
            &imp_module.module_path,
            &imp_module.parse_output,
            &imp_tc,
            &imp_pool,
        );
        imported_type_results.push(imp_tc);
        imported_pools.push(imp_pool);
        imported_canon_results.push(imp_canon);
    }

    (
        imported_type_results,
        imported_pools,
        imported_canon_results,
    )
}

/// Build the merged-pool re-interning state for cross-module imported
/// generic body specialization via body-import linkage.
///
/// Clones the host pool (host `Idx` values stay valid), re-interns each
/// imported module's canon arena + generic sigs into the merged pool, and
/// resolves the host's `MonoInstance` list into `ImportedMonoFn` triples.
/// Short-circuits to a host-pool clone + empty vectors when the host has no
/// imports or no imported generic refs — zero-cost for single-file builds.
pub(crate) fn build_imported_mono_state(
    db: &oric::CompilerDb,
    source_path: &Path,
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    host_pool: &std::sync::Arc<ori_types::Pool>,
) -> ImportedMonoState {
    use oric::Db;

    let interner = db.interner();

    // INVARIANT: host Idx values are preserved by clone; re-interning only appends.
    let mut merged_pool = (**host_pool).clone();

    // Why: resolve_imports covers EVERY `use` statement, including stdlib —
    // broader than the build dependency graph, which filters stdlib out.
    let resolved_imports = crate::imports::resolve_imports(db, parse_result, source_path);

    // The prelude is processed as a trailing module slot (in
    // `type_check_and_canonicalize_imports`), so a file with ONLY prelude
    // imports still has imported-generic surfaces to build.
    if resolved_imports.modules.is_empty() && resolved_imports.prelude.is_none() {
        return ImportedMonoState {
            merged_pool,
            re_interned_canons: Vec::new(),
            imported_mono_fns: Vec::new(),
        };
    }

    let (imported_type_results, imported_pools, imported_canon_results) =
        type_check_and_canonicalize_imports(db, &resolved_imports);

    // INVARIANT: the same per-module cache + var_remap is shared across canon-arena
    // and sig re-interning so scheme_var_ids + leaf Tag::Var ids stay coherent.
    // Sized by the module-array length (explicit imports + trailing prelude slot).
    let module_count = imported_canon_results.len();
    let mut per_module_caches: Vec<FxHashMap<ori_types::Idx, ori_types::Idx>> =
        vec![FxHashMap::default(); module_count];
    let mut per_module_var_remaps: Vec<FxHashMap<u32, u32>> =
        vec![FxHashMap::default(); module_count];

    let re_interned_canons = re_intern_imported_canons(
        &imported_canon_results,
        &imported_pools,
        &mut per_module_caches,
        &mut per_module_var_remaps,
        &mut merged_pool,
    );

    // Why: imported_generic_sigs is keyed by LOCAL name — MonoInstance.fn_name
    // is the call-site identifier (the local/aliased name from the use statement).
    let mut imported_generic_sigs: FxHashMap<
        ori_ir::Name,
        (ori_types::FunctionSig, usize, ori_ir::Name),
    > = FxHashMap::default();
    for func_ref in &resolved_imports.imported_functions {
        if func_ref.is_module_alias {
            continue;
        }
        let Some(sig) = imported_type_results[func_ref.module_index]
            .typed
            .functions
            .iter()
            .find(|s| s.name == func_ref.original_name)
        else {
            continue;
        };
        if !sig.is_generic() {
            continue;
        }
        let source_pool = &imported_pools[func_ref.module_index];
        let cache = &mut per_module_caches[func_ref.module_index];
        let var_remap = &mut per_module_var_remaps[func_ref.module_index];
        let re_interned = ori_types::re_intern_sig_with_var_remap(
            sig,
            source_pool,
            &mut merged_pool,
            cache,
            var_remap,
        );
        imported_generic_sigs.insert(
            func_ref.local_name,
            (re_interned, func_ref.module_index, func_ref.original_name),
        );
    }

    // Register the prelude's `pub` generic free functions (min/max/…) — they
    // are implicit (not ImportedFunctionRef entries) so the loop above never
    // sees them. The prelude slot is the LAST module index.
    if let Some(prelude_module) = resolved_imports.prelude.as_ref() {
        let prelude_idx = resolved_imports.modules.len();
        let source_pool = std::sync::Arc::clone(&imported_pools[prelude_idx]);
        let cache = &mut per_module_caches[prelude_idx];
        let var_remap = &mut per_module_var_remaps[prelude_idx];
        crate::commands::register_prelude_generic_sigs_for_test_runner(
            &mut imported_generic_sigs,
            &prelude_module.parse_output,
            &imported_type_results[prelude_idx].typed,
            &source_pool,
            prelude_idx,
            &mut merged_pool,
            cache,
            var_remap,
        );
    }

    // Why: delegates to the SSOT builder shared with the JIT test runner —
    // one monomorphization mechanism for JIT + AOT.
    let imported_mono_fns = crate::commands::build_imported_mono_functions_for_test_runner(
        type_result,
        &imported_generic_sigs,
        &per_module_caches,
        &mut merged_pool,
        interner,
    );

    ImportedMonoState {
        merged_pool,
        re_interned_canons,
        imported_mono_fns,
    }
}

/// Re-intern each imported module's canon arena into the merged pool.
///
/// Clones every shared `CanonResult` and remaps its arena types through
/// [`re_intern_type_with_var_remap`](ori_types::re_intern_type_with_var_remap),
/// using the per-module cache + var-remap so `scheme_var_ids` + leaf `Tag::Var`
/// ids stay coherent with the sig re-interning that shares the same maps.
fn re_intern_imported_canons(
    imported_canon_results: &[ori_ir::canon::SharedCanonResult],
    imported_pools: &[std::sync::Arc<ori_types::Pool>],
    per_module_caches: &mut [FxHashMap<ori_types::Idx, ori_types::Idx>],
    per_module_var_remaps: &mut [FxHashMap<u32, u32>],
    merged_pool: &mut ori_types::Pool,
) -> Vec<ori_ir::canon::CanonResult> {
    imported_canon_results
        .iter()
        .enumerate()
        .map(|(module_idx, shared_canon)| {
            let source_pool = &imported_pools[module_idx];
            let cache = &mut per_module_caches[module_idx];
            let var_remap = &mut per_module_var_remaps[module_idx];

            let mut remapped: ori_ir::canon::CanonResult = (**shared_canon).clone();
            remapped.arena.remap_types(|type_id| {
                let source_idx = ori_types::Idx::from_raw(type_id.raw());
                let target_idx = ori_types::re_intern_type_with_var_remap(
                    source_pool,
                    source_idx,
                    merged_pool,
                    cache,
                    var_remap,
                );
                ori_ir::TypeId::from_raw(target_idx.raw())
            });
            remapped
        })
        .collect()
}
