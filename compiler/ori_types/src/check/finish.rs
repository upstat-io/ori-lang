//! Type-check result finalization and monomorphization dispatch normalization.

use ori_ir::{ExprArena, Name, PatternKey, PatternResolution, SparseSideTable};

use super::finish_mono::dedup_and_remap_mono_instances;
use super::{derived_call_plans, exports, ModuleChecker};
use crate::{FunctionSig, Idx, Pool, TypeCheckResult, TypedModule};

impl ModuleChecker<'_> {
    /// Finalize checking and produce the result.
    ///
    /// Consumes the checker and returns the typed module with any errors.
    #[must_use]
    pub fn finish(self) -> TypeCheckResult {
        self.finish_with_pool().0
    }

    /// Collect metadata for `ori_canon`'s non-primitive format desugaring.
    ///
    /// Only explicit `Formattable` implementations are registered. Resolving
    /// their self types matches canon's normalized `FormatWith` receivers.
    fn collect_formattable_metadata(
        &mut self,
    ) -> (Vec<Idx>, Option<crate::output::FormatSpecTypes>) {
        // Why: Detaching self types keeps the immutable traits borrow disjoint
        // from mutable pool resolution.
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

        // Why: Re-interning preserves the identities used by canonical field synthesis.
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
    /// Provides the pool for type resolution after checking is complete.
    #[must_use]
    pub fn finish_with_pool(mut self) -> (TypeCheckResult, Pool) {
        let (formattable_impl_types, format_spec_types) = self.collect_formattable_metadata();
        let mut pool = self.pool;
        let deferred_mono_calls = self.deferred_mono_calls;

        // Why: Stable name order preserves Salsa equality across map iteration.
        let mut functions: Vec<FunctionSig> = self.signatures.into_values().collect();
        functions.sort_by_key(|f| f.name);

        // Why: Collection instances have no nominal `TypeEntry` for their AIMS burdens.
        let collection_burdens = self.types.drain_collection_burdens();

        let generic_type_params = collect_generic_type_params(self.types.iter());

        let types = self.types.into_entries();

        let pattern_resolutions = normalize_pattern_resolutions(self.pattern_resolutions);
        let assign_desugar_map = self.assign_desugars;
        let module_alias_call_map = self.module_alias_calls;
        let iter_route_map = self.iter_route_desugars;
        let capability_call_map = self.capability_calls;
        let (method_producers, index_dispatch_map) =
            normalize_index_dispatch(self.index_dispatch_selections);

        let mut mono_instances = self.mono_instances;
        resolve_deferred_mono_calls(
            &mut pool,
            &mut mono_instances,
            &mut self.mono_dispatch_pre_dedup,
            &deferred_mono_calls,
        );

        let accepted_derives = sort_and_validate_accepted_derives(self.accepted_derives);
        let source_method_demands = collect_source_method_demands(self.arena, &self.expr_types);
        let derived_call_plans = derived_call_plans::close_derived_call_plans(
            &mut pool,
            derived_call_plans::DerivedCallClosureSources {
                generic_type_params: &generic_type_params,
                source_method_demands: &source_method_demands,
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

        // Why: Full pool interning exposes method-body composite substitutions.
        exports::refresh_method_mono_body_type_maps(
            &mut pool,
            &mut mono_instances,
            &generic_type_params,
        );

        let (mono_instances, mono_dispatch_map) =
            dedup_and_remap_mono_instances(mono_instances, self.mono_dispatch_pre_dedup);

        let (type_descriptors, exported_type_metadata, exported_collection_surfaces) =
            generate_export_metadata(
                &pool,
                &functions,
                &types,
                &self.imported_type_metadata,
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
            mono_dispatch_map: SparseSideTable::from_unsorted(mono_dispatch_map),
            method_producers,
            index_dispatch_map: SparseSideTable::from_unsorted(index_dispatch_map),
            capability_call_map: SparseSideTable::from_unsorted(capability_call_map),
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

fn normalize_pattern_resolutions(
    mut resolutions: Vec<(PatternKey, PatternResolution)>,
) -> Vec<(PatternKey, PatternResolution)> {
    resolutions.sort_by_key(|(key, _)| *key);
    resolutions.dedup_by_key(|(key, _)| *key);
    resolutions
}

fn collect_generic_type_params<'a>(
    entries: impl Iterator<Item = &'a crate::registry::TypeEntry>,
) -> rustc_hash::FxHashMap<Name, Vec<Name>> {
    entries
        .filter(|entry| !entry.type_params.is_empty())
        .map(|entry| (entry.name, entry.type_params.clone()))
        .collect()
}

fn resolve_deferred_mono_calls(
    pool: &mut Pool,
    mono_instances: &mut Vec<crate::MonoInstance>,
    dispatch: &mut Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
    deferred_calls: &[crate::DeferredMonoCall],
) {
    if deferred_calls.is_empty() {
        return;
    }
    exports::resolve_deferred_mono_calls(pool, mono_instances, dispatch, deferred_calls);
}

/// Assign deterministic dense handles to the exact producers selected by
/// user-defined index expressions.
///
/// Source `ExprId` order is stable for a parsed module, so first occurrence in
/// that order gives deterministic producer IDs without requiring an artificial
/// ordering over imported symbols and registry projections.
fn normalize_index_dispatch(
    mut selections: Vec<(ori_ir::ExprId, crate::IndexDispatchSelection)>,
) -> (
    Vec<crate::MethodProducer>,
    Vec<(ori_ir::ExprId, ori_ir::canon::IndexDispatch)>,
) {
    selections.sort_by_key(|(expr, _)| *expr);

    let mut refined = Vec::with_capacity(selections.len());
    for (expr, selection) in selections {
        if let Some(selection) = refine_last_index_dispatch(&mut refined, expr, selection) {
            refined.push((expr, selection));
        }
    }

    let mut producers = Vec::new();
    let mut ids = rustc_hash::FxHashMap::default();
    let mut dispatch = Vec::with_capacity(refined.len());
    for (expr, selection) in refined {
        let route = match selection {
            crate::IndexDispatchSelection::Builtin => ori_ir::canon::IndexDispatch::Builtin,
            crate::IndexDispatchSelection::Deferred => ori_ir::canon::IndexDispatch::Deferred,
            crate::IndexDispatchSelection::Error => ori_ir::canon::IndexDispatch::Error,
            crate::IndexDispatchSelection::Selected(producer) => {
                let id = intern_index_method_producer(producer, &mut producers, &mut ids);
                ori_ir::canon::IndexDispatch::Selected(id)
            }
        };
        dispatch.push((expr, route));
    }
    (producers, dispatch)
}

fn refine_last_index_dispatch(
    refined: &mut [(ori_ir::ExprId, crate::IndexDispatchSelection)],
    expr: ori_ir::ExprId,
    selection: crate::IndexDispatchSelection,
) -> Option<crate::IndexDispatchSelection> {
    let Some((last_expr, existing)) = refined.last_mut() else {
        return Some(selection);
    };
    if *last_expr != expr {
        return Some(selection);
    }
    refine_index_dispatch(existing, selection);
    None
}

fn refine_index_dispatch(
    existing: &mut crate::IndexDispatchSelection,
    selection: crate::IndexDispatchSelection,
) {
    if selection == crate::IndexDispatchSelection::Error {
        return;
    }
    assert!(
        *existing == crate::IndexDispatchSelection::Error
            || *existing == crate::IndexDispatchSelection::Deferred
            || *existing == selection,
        "one index expression cannot select two semantic dispatch routes"
    );
    *existing = selection;
}

fn intern_index_method_producer(
    producer: crate::MethodProducer,
    producers: &mut Vec<crate::MethodProducer>,
    ids: &mut rustc_hash::FxHashMap<crate::MethodProducer, crate::MethodProducerId>,
) -> crate::MethodProducerId {
    if let Some(&id) = ids.get(&producer) {
        return id;
    }
    let Ok(raw) = u32::try_from(producers.len()) else {
        unreachable!("method-producer table exceeds MethodProducerId capacity");
    };
    let id = crate::MethodProducerId::new(raw);
    producers.push(producer.clone());
    ids.insert(producer, id);
    id
}

/// Sort accepted derived-impl identities for deterministic output and assert
/// the dedup invariant (`AcceptedDerivedImpl.id` unique) that
/// `derived_call_plans::close_derived_call_plans` relies on.
fn sort_and_validate_accepted_derives(
    mut accepted_derives: Vec<crate::AcceptedDerivedImpl>,
) -> Vec<crate::AcceptedDerivedImpl> {
    accepted_derives.sort_unstable_by_key(|accepted| accepted.id);
    assert!(
        accepted_derives
            .windows(2)
            .all(|pair| pair[0].id != pair[1].id),
        "accepted derived implementation identities must be unique"
    );
    accepted_derives
}

/// Collect `(receiver_type, method_name)` demand pairs for every method-call
/// expression in the arena, keyed to the receiver's already-inferred type.
/// Feeds `derived_call_plans::close_derived_call_plans`'s demand-driven
/// derived-method-call closure.
fn collect_source_method_demands(arena: &ExprArena, expr_types: &[Idx]) -> Vec<(Idx, Name)> {
    (0..arena.expr_count())
        .filter_map(|raw| {
            let Ok(raw) = u32::try_from(raw) else {
                unreachable!("expression arena exceeded the ExprId domain");
            };
            let id = ori_ir::ExprId::new(raw);
            let (receiver, method) = match arena.expr_kind(id) {
                ori_ir::ExprKind::MethodCall {
                    receiver, method, ..
                }
                | ori_ir::ExprKind::MethodCallNamed {
                    receiver, method, ..
                } => (*receiver, *method),
                _ => return None,
            };
            expr_types
                .get(receiver.index())
                .copied()
                .map(|receiver| (receiver, method))
        })
        .collect()
}

/// Builds type descriptors, repr metadata, and collection ABI hashes.
///
/// The result preserves transitive type reconstruction, repr constraints, and
/// public collection ABI identities.
fn generate_export_metadata(
    pool: &Pool,
    functions: &[FunctionSig],
    types: &[crate::registry::TypeEntry],
    imported_type_metadata: &[crate::output::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
) -> (
    Vec<(u64, crate::TypeDescriptor)>,
    Vec<crate::output::ExportedTypeMetadata>,
    Vec<u64>,
) {
    let type_descriptors = exports::generate_export_descriptors(pool, functions);
    let exported_type_metadata =
        exports::generate_exported_type_metadata(types, imported_type_metadata);
    let exported_collection_surfaces = exports::generate_exported_collection_surfaces(
        pool,
        functions,
        imported_collection_surfaces,
    );
    (
        type_descriptors,
        exported_type_metadata,
        exported_collection_surfaces,
    )
}

#[cfg(test)]
mod tests;
