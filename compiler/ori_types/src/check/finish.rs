//! Type-check result finalization and monomorphization dispatch normalization.

use ori_ir::{Name, SparseSideTable};

use super::{derived_call_plans, exports, ModuleChecker};
use crate::{FunctionSig, Idx, Pool, TypeCheckResult, TypedModule};

/// Identity tuple for `MonoInstance` deduplication at `finish_with_pool()`.
///
/// Encodes the full distinguishing identity per the `MonoInstance`
/// invariant: `(fn_name, generic_args, impl_args, method_args,
/// concrete_param_types, receiver_type, method_producer)`. Two instances are
/// duplicates iff every field of the tuple matches.
type MonoIdentityKey = (
    Name,
    Vec<crate::GenericArg>,
    Vec<crate::GenericArg>,
    Vec<crate::GenericArg>,
    Vec<Idx>,
    Option<Idx>,
    Option<crate::MethodProducer>,
);

impl ModuleChecker<'_> {
    /// Finalize checking and produce the result.
    ///
    /// Consumes the checker and returns the typed module with any errors.
    pub fn finish(self) -> TypeCheckResult {
        self.finish_with_pool().0
    }

    /// Capture the explicit-`Formattable` impl set + builtin `FormatSpec` type
    /// idxs for `ori_canon`'s non-primitive `{expr:spec}` desugar. The blanket
    /// `impl<T: Printable> T: Formattable` is not a registered impl, so the impl
    /// set is exactly the user-written `Formattable` impls. Self types are
    /// resolved through the pool's chains so the canon-side query (which
    /// normalizes the `FormatWith` receiver type the same way) matches regardless
    /// of Named-vs-resolved divergence.
    fn collect_formattable_metadata(
        &mut self,
    ) -> (Vec<Idx>, Option<crate::output::FormatSpecTypes>) {
        // Collect self types under an immutable `traits` borrow, then resolve
        // them under a mutable `pool` borrow — the two stay disjoint.
        let formattable_name = self.interner.intern("Formattable");
        let raw_self_types: Vec<Idx> = self
            .traits
            .get_trait_by_name(formattable_name)
            .map(|t| t.idx)
            .map(|formattable_idx| {
                self.traits
                    .impls_of_trait(formattable_idx)
                    .map(|e| e.self_type)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut formattable_impl_types: Vec<Idx> = raw_self_types
            .into_iter()
            .map(|st| self.pool.resolve_fully(st))
            .collect();
        formattable_impl_types.sort_unstable_by_key(|i| i.raw());
        formattable_impl_types.dedup_by_key(|i| i.raw());

        // Builtin `FormatSpec` struct idx + its `Option<_>` field idxs
        // (idempotent interning returns the entries `register_format_spec_type`
        // already created) so ori_canon can type the synthesized FormatSpec
        // struct + field-value nodes.
        let spec_name = self.interner.intern("FormatSpec");
        let alignment_name = self.interner.intern("Alignment");
        let sign_name = self.interner.intern("Sign");
        let format_type_name = self.interner.intern("FormatType");
        let spec = self.pool.named(spec_name);
        let alignment_idx = self.pool.named(alignment_name);
        let sign_idx = self.pool.named(sign_name);
        let ft_idx = self.pool.named(format_type_name);
        let format_spec_types = Some(crate::output::FormatSpecTypes {
            spec,
            opt_char: self.pool.option(Idx::CHAR),
            opt_alignment: self.pool.option(alignment_idx),
            opt_sign: self.pool.option(sign_idx),
            opt_int: self.pool.option(Idx::INT),
            opt_format_type: self.pool.option(ft_idx),
            alignment: alignment_idx,
            sign: sign_idx,
            format_type: ft_idx,
        });

        (formattable_impl_types, format_spec_types)
    }

    /// Consume the checker and return the pool along with the result.
    ///
    /// Use this when you need access to the pool for type resolution after
    /// checking is complete.
    pub fn finish_with_pool(mut self) -> (TypeCheckResult, Pool) {
        let (formattable_impl_types, format_spec_types) = self.collect_formattable_metadata();
        let mut pool = self.pool;
        let deferred_mono_calls = self.deferred_mono_calls;

        // Sort functions by name for deterministic output regardless of
        // FxHashMap iteration order. Required for Salsa's Eq comparison.
        let mut functions: Vec<FunctionSig> = self.signatures.into_values().collect();
        functions.sort_by_key(|f| f.name);

        // Drain the monomorphized-collection burden side-table before
        // `into_entries` consumes the registry — these instances carry no
        // nominal `TypeEntry`, so `types` (below) excludes them by
        // construction. Exporting them lets the ARC pipeline reconstruct the
        // side-table for Phase 5 burden emission (Spec: Annex E §AIMS).
        let collection_burdens = self.types.drain_collection_burdens();

        // Generic-composite type-param map (`Name → [param names]`) for refreshing method
        // mono `body_type_map`s below — built BEFORE `into_entries` consumes the
        // registry. SSOT mirror of `monomorphization::collect_generic_type_params`.
        let generic_type_params: rustc_hash::FxHashMap<Name, Vec<Name>> = self
            .types
            .iter()
            .filter(|entry| !entry.type_params.is_empty())
            .map(|entry| (entry.name, entry.type_params.clone()))
            .collect();

        // Extract type definitions (already sorted by name via BTreeMap).
        let types = self.types.into_entries();

        // Dedup pattern resolutions (sorted by key first so duplicate keys are
        // adjacent); the final `SparseSideTable::from_unsorted` re-sorts for the
        // O(log n) binary-search shape. `assign_desugar_map` is sorted by the
        // table; the `AssignTarget` desugar plans carry unique `ExprId` keys.
        let mut pattern_resolutions = self.pattern_resolutions;
        pattern_resolutions.sort_by_key(|(k, _)| *k);
        pattern_resolutions.dedup_by_key(|(k, _)| *k);
        let assign_desugar_map = self.assign_desugars;
        let module_alias_call_map = self.module_alias_calls;
        let iter_route_map = self.iter_route_desugars;

        // Resolve transitive mono calls (generic calling generic) before dedup.
        // The deferred resolver publishes dispatch entries into
        // `mono_dispatch_pre_dedup` for each successfully-resolved deferred
        // call, using the `DeferredMonoCall.call_expr_id` recorded at inference
        // time. Pre-dedup ids are remapped through the same dedup pipeline as
        // eager-path entries below.
        let mut mono_instances = self.mono_instances;
        if !deferred_mono_calls.is_empty() {
            exports::resolve_deferred_mono_calls(
                &mut pool,
                &mut mono_instances,
                &mut self.mono_dispatch_pre_dedup,
                &deferred_mono_calls,
            );
        }

        let mut accepted_derives = self.accepted_derives;
        accepted_derives.sort_unstable_by_key(|accepted| accepted.id);
        debug_assert!(
            accepted_derives
                .windows(2)
                .all(|pair| pair[0].id != pair[1].id),
            "accepted derived implementation identities must be unique"
        );
        let derived_call_plans = derived_call_plans::close_derived_call_plans(
            &mut pool,
            derived_call_plans::DerivedCallClosureSources {
                generic_type_params: &generic_type_params,
                traits: &self.traits,
                functions: &functions,
                impl_sigs: &self.impl_sigs,
                imported_impl_sigs: &self.imported_impl_sigs,
                accepted_derives: &accepted_derives,
                interner: self.interner,
            },
            &mut mono_instances,
            &mut self.errors,
        );

        // Complete each method instance's `body_type_map` against the now-fully-
        // interned pool. The eager method-mono path builds the map at the call
        // site (Pass 3), before a generic-impl method body interns its own
        // composite ctor types (Pass 4) — e.g. a `Pair<B, A>` constructor inside
        // `swap`. This pass captures those post-recording body composites so they
        // reach codegen substituted to concrete instead of carrying impl
        // `RigidVar`s. Runs before dedup so refreshed maps participate in the
        // identity tuple unchanged (`body_type_map` is not a dedup key).
        exports::refresh_method_mono_body_type_maps(
            &mut pool,
            &mut mono_instances,
            &generic_type_params,
        );

        // Dedup mono instances by the full identity tuple, then sort by
        // `fn_name` and remap the dispatch entries through the composed
        // `pre-dedup → dedup → sorted` permutation. A free function sidesteps
        // the partial-move of `self.pool` earlier in this method.
        let (mono_instances, mono_dispatch_map) =
            dedup_and_remap_mono_instances(mono_instances, self.mono_dispatch_pre_dedup);

        // Generate portable type descriptors for all public function signatures.
        // These enable cross-module type reconstruction without AST access.
        let type_descriptors = exports::generate_export_descriptors(&pool, &functions);

        // Generate exported type metadata for cross-module repr plan construction.
        // Merges local types (repr/public) with forwarded imported metadata so that
        // transitive chains (A→B→C) propagate correctly.
        let exported_type_metadata =
            exports::generate_exported_type_metadata(&types, &self.imported_type_metadata);

        // Generate collection surface hashes for cross-module ABI protection.
        // Walks public function signatures to find List/Set types, merges with
        // imported surfaces for transitive forwarding.
        let exported_collection_surfaces = exports::generate_exported_collection_surfaces(
            &pool,
            &functions,
            &self.imported_collection_surfaces,
        );

        let typed = TypedModule {
            expr_types: self.expr_types,
            functions,
            types,
            errors: self.errors,
            warnings: self.warnings,
            pattern_resolutions: SparseSideTable::from_unsorted(pattern_resolutions),
            impl_sigs: self.impl_sigs,
            imported_impl_sigs: self.imported_impl_sigs,
            accepted_derives,
            derived_call_plans,
            trait_impl_fn_names: self.trait_impl_fn_names,
            mono_instances,
            // Populated from `mono_dispatch_pre_dedup` after remapping
            // pre-dedup `MonoInstanceId`s through dedup + sort, then sorted
            // by `ExprId` for binary-search lookup. The deferred-resolution
            // path `exports::resolve_deferred_mono_calls` publishes pre-dedup
            // entries via `DeferredMonoCall.call_expr_id`, so transitive
            // (generic-calls-generic) instantiations land in this map
            // alongside eager-path instantiations. Both flow through the
            // same dedup-remap pipeline.
            mono_dispatch_map: SparseSideTable::from_unsorted(mono_dispatch_map),
            type_descriptors,
            exported_type_metadata,
            exported_collection_surfaces,
            collection_burdens,
            formattable_impl_types,
            format_spec_types,
            assign_desugar_map: SparseSideTable::from_unsorted(assign_desugar_map),
            module_alias_call_map: SparseSideTable::from_unsorted(module_alias_call_map),
            iter_route_map: SparseSideTable::from_unsorted(iter_route_map),
        };

        (TypeCheckResult::from_typed(typed), pool)
    }
}

/// Dedup `mono_instances` by the full identity tuple, sort the survivors by
/// `fn_name`, and remap `mono_dispatch_pre_dedup` entries through the composed
/// `pre-dedup → dedup → sorted` permutation.
///
/// Returns the deduped+sorted instances and the remapped dispatch map sorted by
/// `ExprId` for binary-search lookup.
fn dedup_and_remap_mono_instances(
    mut mono_instances: Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) -> (
    Vec<crate::MonoInstance>,
    Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) {
    // Dedup mono instances by the full identity tuple — `fn_name` alone
    // is insufficient once method instances flow through this list:
    //
    // - Method-level args collision: `Foo<int>::bar<U>` instantiated via
    //   typed-binding inference at `U = str` vs `U = int` share `fn_name`
    //   AND empty `generic_args`; only `method_args` differ.
    // - Receiver-type collision: `Box<int>::map<U>` and `Option<int>::map<U>`
    //   both have `fn_name = "map"` and identical `impl_args = [int]`;
    //   only `receiver_type` (or `concrete_param_types[0]`) discriminates.
    // - Trait-method-from-different-impls: two impls of the same trait
    //   method on different self types — distinguished by `receiver_type`.
    //
    // Identity tuple: (fn_name, generic_args, impl_args, method_args,
    // concrete_param_types, receiver_type, method_producer) — see
    // `MonoIdentityKey` alias.
    //
    // Dedup tracks `old_idx → new_idx` so the pre-dedup
    // `mono_dispatch_map` entries (which carry pre-dedup `MonoInstanceId`s)
    // can be remapped to point at the same canonical instance after
    // non-adjacent duplicates collapse. FxHashMap stays deterministic
    // (FxHasher has no per-process random seed), satisfying Salsa SL-1
    // (query purity).
    let pre_dedup_len = mono_instances.len();
    let mut seen: rustc_hash::FxHashMap<MonoIdentityKey, u32> = rustc_hash::FxHashMap::default();
    let mut deduped: Vec<crate::MonoInstance> = Vec::with_capacity(pre_dedup_len);
    // `old_to_dedup[old_position]` = position in `deduped` after retain.
    let mut old_to_dedup: Vec<u32> = Vec::with_capacity(pre_dedup_len);
    for inst in mono_instances.drain(..) {
        let key: MonoIdentityKey = (
            inst.fn_name,
            inst.generic_args.clone(),
            inst.impl_args.clone(),
            inst.method_args.clone(),
            inst.concrete_param_types.clone(),
            inst.receiver_type,
            inst.method_producer.clone(),
        );
        if let Some(&existing) = seen.get(&key) {
            old_to_dedup.push(existing);
        } else {
            // Saturating `Vec::len() → u32` matches `pool/substitute/mod.rs`
            // — strict workspace clippy denies bare `as` truncation casts and
            // `expect`/`unwrap`. 4-billion-instance overflow is structurally
            // unreachable for any single module.
            let new_idx = u32::try_from(deduped.len()).unwrap_or(u32::MAX);
            seen.insert(key, new_idx);
            deduped.push(inst);
            old_to_dedup.push(new_idx);
        }
    }

    // Sort by fn_name for deterministic output ordering, tracking the
    // permutation so dispatch entries can be re-anchored. Pairing
    // each instance with its pre-sort index via `enumerate` and then
    // sorting the pair vector avoids the placeholder-`Option` dance
    // that an in-place permutation would require.
    let n_dedup = deduped.len();
    // Saturating `usize → u32` casts match `pool/substitute/mod.rs`'s
    // `unwrap_or(u32::MAX)` pattern (strict workspace clippy denies
    // `cast_possible_truncation`). Per the dedup-loop comment above,
    // `deduped.len()` is structurally bounded well below `u32::MAX`.
    let mut indexed: Vec<(u32, crate::MonoInstance)> = deduped
        .into_iter()
        .enumerate()
        .map(|(i, inst)| (u32::try_from(i).unwrap_or(u32::MAX), inst))
        .collect();
    indexed.sort_by_key(|(_, inst)| inst.fn_name);
    let mut dedup_to_sorted: Vec<u32> = vec![0; n_dedup];
    for (sorted_pos, (dedup_pos, _)) in indexed.iter().enumerate() {
        dedup_to_sorted[*dedup_pos as usize] = u32::try_from(sorted_pos).unwrap_or(u32::MAX);
    }
    let mono_instances: Vec<crate::MonoInstance> =
        indexed.into_iter().map(|(_, inst)| inst).collect();

    // Apply the composed `pre-dedup → dedup → sorted` remap to the dispatch
    // entries. The caller wraps the result in `SparseSideTable::from_unsorted`,
    // which sorts by `ExprId` for the O(log n) binary-search shape.
    let mono_dispatch_map: Vec<(ori_ir::ExprId, crate::MonoInstanceId)> = mono_dispatch_pre_dedup
        .into_iter()
        .map(|(eid, id)| {
            let dedup_idx = old_to_dedup[id.index()];
            let final_idx = dedup_to_sorted[dedup_idx as usize];
            (eid, crate::MonoInstanceId::new(final_idx))
        })
        .collect();

    (mono_instances, mono_dispatch_map)
}
