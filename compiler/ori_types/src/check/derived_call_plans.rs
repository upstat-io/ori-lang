//! Type-checker-owned closure of calls emitted by accepted derived bodies.

mod monos;
mod planning;
mod selection;

use monos::{push_derived_mono, push_impl_mono, push_imported_impl_mono, MethodMonoDemand};
use planning::build_plan;
use selection::{select_producer, selected_local_impl};

use ori_ir::{DerivedImplId, DerivedMethodShape, DerivedTrait, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::pool::substitute::{build_finalized_body_type_map, substitute_in_pool};
use crate::{
    AcceptedDerivedImpl, ConcreteMethodMono, DerivedCallPlan, DerivedCallPosition,
    DerivedCallSelection, DerivedDirectCallSelection, FunctionSig, GenericArg, Idx, ImplSig,
    ImportedImplSig, MethodProducer, MonoInstance, Pool, RegistryMethodIdentity,
    RegistryPreludeIdentity, Tag, TraitRegistry, TypeCheckError,
};

#[derive(Clone)]
struct ProducerSelection {
    producer: MethodProducer,
    impl_args: Vec<Idx>,
    has_self: bool,
}

enum SelectionOutcome {
    Found(ProducerSelection),
    Missing,
    Ambiguous(usize),
}

struct PlanSeed {
    accepted: DerivedImplId,
    receiver: Idx,
}

fn push_source_plan_seed(
    seeded: &mut FxHashSet<(DerivedImplId, Vec<Idx>)>,
    pending: &mut Vec<PlanSeed>,
    accepted: DerivedImplId,
    receiver: Idx,
    canonical_substitutions: Vec<Idx>,
) {
    if seeded.insert((accepted, canonical_substitutions)) {
        pending.push(PlanSeed { accepted, receiver });
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DerivedCallClosureSources<'a> {
    pub(super) generic_type_params: &'a FxHashMap<Name, Vec<Name>>,
    pub(super) source_method_demands: &'a [(Idx, Name)],
    pub(super) traits: &'a TraitRegistry,
    pub(super) functions: &'a [FunctionSig],
    pub(super) impl_sigs: &'a [ImplSig],
    pub(super) imported_impl_sigs: &'a [ImportedImplSig],
    pub(super) accepted_derives: &'a [AcceptedDerivedImpl],
    pub(super) interner: &'a StringInterner,
}

struct CallPositions {
    methods: Vec<(DerivedCallPosition, Idx)>,
    direct: Vec<DerivedCallPosition>,
}

#[derive(Clone, Copy)]
struct PlanSelectionSources<'a> {
    traits: &'a TraitRegistry,
    interner: &'a StringInterner,
}

/// Freeze every reachable generated call and add its generic local producer
/// specialization to the existing mono inventory.
pub(super) fn close_derived_call_plans(
    pool: &mut Pool,
    sources: DerivedCallClosureSources<'_>,
    mono_instances: &mut Vec<MonoInstance>,
    errors: &mut Vec<TypeCheckError>,
) -> Vec<DerivedCallPlan> {
    let concrete_applied: Vec<_> = pool
        .iter_indices()
        .filter(|&idx| pool.tag(idx) == Tag::Applied)
        .collect();
    let mut in_progress = FxHashSet::default();
    for applied in concrete_applied {
        crate::pool::substitute::materialize_applied_body(
            pool,
            applied,
            sources.generic_type_params,
            &mut in_progress,
        );
    }
    let accepted_by_id: FxHashMap<_, _> = sources
        .accepted_derives
        .iter()
        .map(|accepted| (accepted.id, accepted))
        .collect();
    let impl_by_id: FxHashMap<_, _> = sources.impl_sigs.iter().map(|sig| (sig.id, sig)).collect();
    let mut pending = Vec::new();

    seed_bound_derived_monos(pool, &sources, mono_instances);
    seed_source_derived_plans(pool, &sources, &mut pending);

    for accepted in sources.accepted_derives {
        if !accepted.signature.is_generic() {
            pending.push(PlanSeed {
                accepted: accepted.id,
                receiver: accepted.owner_type,
            });
        }
    }
    for instance in mono_instances.iter() {
        let (Some(MethodProducer::Derived(derived)), Some(receiver)) =
            (&instance.method_producer, instance.receiver_type)
        else {
            continue;
        };
        pending.push(PlanSeed {
            accepted: *derived,
            receiver,
        });
    }

    close_plan_worklist(
        pool,
        &sources,
        &accepted_by_id,
        &impl_by_id,
        pending,
        mono_instances,
        errors,
    )
}

/// Seed concrete generic-derived receivers demanded by source method calls.
///
/// Constructor-led inference can resolve `Full(Full("x")).debug()` through a
/// rigid bound before the receiver's nested generic arguments become concrete.
/// That early selection has no executable producer yet. At module finalization,
/// the source receiver type is concrete and the accepted-derive inventory can
/// prove the exact producer. Seed only those source demands; the generated-body
/// worklist then closes inner derives such as `Holder<str>.debug()`.
fn seed_source_derived_plans(
    pool: &mut Pool,
    sources: &DerivedCallClosureSources<'_>,
    pending: &mut Vec<PlanSeed>,
) {
    let mut seeded: FxHashSet<(DerivedImplId, Vec<Idx>)> = FxHashSet::default();
    for &(demanded, method) in sources.source_method_demands {
        for accepted in sources
            .accepted_derives
            .iter()
            .filter(|accepted| accepted.signature.is_generic())
            .filter(|accepted| method == accepted.method_name)
        {
            seed_concrete_derived_plan(pool, sources, accepted, demanded, &mut seeded, pending);
        }

        let Some(trait_kind) = structural_builtin_trait_demand(demanded, method, sources, pool)
        else {
            continue;
        };
        let nested_receivers = structural_builtin_nested_receivers(demanded, trait_kind, pool);
        for nested_receiver in nested_receivers {
            for accepted in sources
                .accepted_derives
                .iter()
                .filter(|accepted| accepted.signature.is_generic())
                .filter(|accepted| accepted.trait_kind == trait_kind)
            {
                seed_concrete_derived_plan(
                    pool,
                    sources,
                    accepted,
                    nested_receiver,
                    &mut seeded,
                    pending,
                );
            }
        }
    }
}

fn seed_concrete_derived_plan(
    pool: &mut Pool,
    sources: &DerivedCallClosureSources<'_>,
    accepted: &AcceptedDerivedImpl,
    demanded: Idx,
    seeded: &mut FxHashSet<(DerivedImplId, Vec<Idx>)>,
    pending: &mut Vec<PlanSeed>,
) {
    let Some((receiver, substitutions)) = concrete_derived_receiver(accepted, demanded, pool)
    else {
        tracing::debug!(
            accepted = ?accepted.id,
            ?demanded,
            "derived seed miss: concrete receiver"
        );
        return;
    };
    tracing::debug!(
        accepted = ?accepted.id,
        ?demanded,
        ?receiver,
        ?substitutions,
        "derived seed concrete"
    );
    if substitutions.iter().any(|&substitution| {
        !is_concrete_derived_substitution(substitution, &accepted.signature.type_params, pool)
    }) {
        return;
    }
    let canonical_substitutions: Vec<_> = substitutions
        .iter()
        .map(|&substitution| pool.method_receiver_key(substitution))
        .collect();
    let mut in_progress = FxHashSet::default();
    crate::pool::substitute::materialize_applied_body(
        pool,
        receiver,
        sources.generic_type_params,
        &mut in_progress,
    );
    let selection = match select_producer(
        receiver,
        accepted.trait_type,
        accepted.method_name,
        sources.traits,
        sources.interner,
        pool,
    ) {
        SelectionOutcome::Found(selection) => selection,
        SelectionOutcome::Missing => {
            tracing::debug!(
                accepted = ?accepted.id,
                receiver = ?receiver,
                "derived seed miss: missing selection"
            );
            return;
        }
        SelectionOutcome::Ambiguous(count) => {
            tracing::debug!(
                count,
                accepted = ?accepted.id,
                receiver = ?receiver,
                "derived seed miss: ambiguous selection"
            );
            return;
        }
    };
    if selection.producer != MethodProducer::Derived(accepted.id) {
        tracing::debug!(
            producer = ?selection.producer,
            accepted = ?accepted.id,
            receiver = ?receiver,
            "derived seed miss: producer mismatch"
        );
        return;
    }
    tracing::debug!(
        accepted = ?accepted.id,
        receiver = ?receiver,
        "derived seed push"
    );
    push_source_plan_seed(
        seeded,
        pending,
        accepted.id,
        receiver,
        canonical_substitutions,
    );
}

/// Translate a source builtin-method call into the derived trait its
/// structural implementation invokes on nested values.
fn structural_builtin_trait_demand(
    receiver: Idx,
    method: Name,
    sources: &DerivedCallClosureSources<'_>,
    pool: &Pool,
) -> Option<DerivedTrait> {
    let receiver_tag = pool.builtin_method_type_tag(receiver)?;
    let method_name = sources.interner.try_lookup(method)?;
    let trait_name = ori_registry::find_method(receiver_tag, method_name)?.trait_name?;
    let trait_kind = DerivedTrait::from_name(trait_name)?;
    matches!(
        trait_kind,
        DerivedTrait::Eq
            | DerivedTrait::Hashable
            | DerivedTrait::Printable
            | DerivedTrait::Debug
            | DerivedTrait::Comparable
    )
    .then_some(trait_kind)
}

/// Return nominal receivers reached by the recursive builtin implementation.
/// Compound children stay in the worklist so nested containers close all the
/// way to the user-defined value whose derived body must be callable.
fn structural_builtin_nested_receivers(
    receiver: Idx,
    trait_kind: DerivedTrait,
    pool: &Pool,
) -> Vec<Idx> {
    let mut pending = vec![receiver];
    let mut seen = FxHashSet::default();
    let mut nested = Vec::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(pool.method_receiver_key(current)) {
            continue;
        }
        for child in structural_builtin_children(current, trait_kind, pool) {
            nested.push(child);
            if pool.builtin_method_type_tag(child).is_some() {
                pending.push(child);
            }
        }
    }
    nested
}

fn structural_builtin_children(receiver: Idx, trait_kind: DerivedTrait, pool: &Pool) -> Vec<Idx> {
    let receiver = pool.method_receiver_type(receiver);
    match pool.tag(receiver) {
        Tag::List => vec![pool.list_elem(receiver)],
        Tag::Option => vec![pool.option_inner(receiver)],
        Tag::Result => vec![pool.result_ok(receiver), pool.result_err(receiver)],
        Tag::Tuple => pool.tuple_elems(receiver),
        Tag::Map if trait_kind != DerivedTrait::Comparable => {
            vec![pool.map_key(receiver), pool.map_value(receiver)]
        }
        Tag::Set if trait_kind != DerivedTrait::Comparable => vec![pool.set_elem(receiver)],
        _ => Vec::new(),
    }
}

fn is_concrete_derived_substitution(ty: Idx, binders: &[Name], pool: &Pool) -> bool {
    let resolved = pool.resolve_fully(ty);
    pool.is_valid_idx(resolved)
        && pool.flags(resolved).is_recordable()
        && !crate::pool::substitute::has_unproven_named_leaf(pool, ty, binders)
}

/// Materialize accepted derives required by concrete generic-function bounds.
///
/// A method call on a rigid receiver (`T: Debug`, then `value.debug()`) cannot
/// select its concrete producer while the generic body is checked. The
/// concrete receiver first exists on the caller's [`MonoInstance`]. Close that
/// semantic demand before generated-body closure, so every executor sees
/// the same exact [`MethodProducer::Derived`] specialization in the typed mono
/// inventory. The generated-body worklist then closes calls made
/// by the newly reachable derived body.
fn seed_bound_derived_monos(
    pool: &mut Pool,
    sources: &DerivedCallClosureSources<'_>,
    mono_instances: &mut Vec<MonoInstance>,
) {
    let functions: FxHashMap<_, _> = sources
        .functions
        .iter()
        .map(|signature| (signature.name, signature))
        .collect();
    let callers: Vec<_> = mono_instances
        .iter()
        .filter(|instance| instance.receiver_type.is_none())
        .map(|instance| (instance.fn_name, instance.generic_args.clone()))
        .collect();

    for (caller, generic_args) in callers {
        let Some(signature) = functions.get(&caller).copied() else {
            continue;
        };
        for (position, argument) in generic_args.iter().enumerate() {
            let GenericArg::Type(demanded_receiver) = argument else {
                continue;
            };
            let Some(&parameter) = signature.type_params.get(position) else {
                continue;
            };
            let mut bounds = signature
                .type_param_bounds
                .get(position)
                .cloned()
                .unwrap_or_default();
            bounds.extend(
                signature
                    .where_clauses
                    .iter()
                    .filter(|constraint| {
                        constraint.param == parameter && constraint.projection.is_none()
                    })
                    .flat_map(|constraint| constraint.bounds.iter().copied()),
            );
            bounds.sort_unstable();
            bounds.dedup();

            for bound in bounds {
                let Some(required_trait) = sources.traits.get_trait_by_name(bound) else {
                    continue;
                };
                for accepted in sources
                    .accepted_derives
                    .iter()
                    .filter(|accepted| accepted.signature.is_generic())
                    .filter(|accepted| accepted.trait_type == required_trait.idx)
                {
                    let Some((receiver, binder_substitutions)) =
                        concrete_derived_receiver(accepted, *demanded_receiver, pool)
                    else {
                        continue;
                    };
                    let SelectionOutcome::Found(selection) = select_producer(
                        receiver,
                        required_trait.idx,
                        accepted.method_name,
                        sources.traits,
                        sources.interner,
                        pool,
                    ) else {
                        continue;
                    };
                    if selection.producer != MethodProducer::Derived(accepted.id) {
                        continue;
                    }
                    push_derived_mono(
                        accepted,
                        receiver,
                        &binder_substitutions,
                        mono_instances,
                        pool,
                    );
                }
            }
        }
    }
}

fn close_plan_worklist(
    pool: &mut Pool,
    sources: &DerivedCallClosureSources<'_>,
    accepted_by_id: &FxHashMap<DerivedImplId, &AcceptedDerivedImpl>,
    impl_by_id: &FxHashMap<crate::ImplMethodId, &ImplSig>,
    mut pending: Vec<PlanSeed>,
    mono_instances: &mut Vec<MonoInstance>,
    errors: &mut Vec<TypeCheckError>,
) -> Vec<DerivedCallPlan> {
    let mut seen = FxHashSet::default();
    let mut plans = Vec::new();
    while let Some(seed) = pending.pop() {
        let Some(accepted) = accepted_by_id.get(&seed.accepted).copied() else {
            errors.push(TypeCheckError::unsatisfied_bound(
                ori_ir::Span::DUMMY,
                "generated method demand references an unknown accepted derive",
            ));
            continue;
        };
        let Some((receiver, binder_substitutions)) =
            concrete_derived_receiver(accepted, seed.receiver, pool)
        else {
            errors.push(TypeCheckError::unknown_method(
                accepted.span,
                seed.receiver,
                accepted.method_name,
            ));
            continue;
        };
        if !seen.insert((accepted.id, binder_substitutions.clone())) {
            continue;
        }

        if accepted.signature.is_generic() {
            push_derived_mono(
                accepted,
                receiver,
                &binder_substitutions,
                mono_instances,
                pool,
            );
        }

        let Some(plan) = build_plan(
            accepted,
            receiver,
            binder_substitutions,
            sources.traits,
            sources.interner,
            pool,
            errors,
        ) else {
            continue;
        };

        for call in &plan.calls {
            match call.producer {
                MethodProducer::Derived(derived) => pending.push(PlanSeed {
                    accepted: derived,
                    receiver: call.receiver_type,
                }),
                MethodProducer::Impl(id) => {
                    if let Some(sig) = impl_by_id.get(&id).copied() {
                        push_impl_mono(
                            sig,
                            call.receiver_type,
                            MethodMonoDemand {
                                producer: &call.producer,
                                traits: sources.traits,
                                trait_type: accepted.trait_type,
                                method_name: call.method_name,
                            },
                            mono_instances,
                            pool,
                        );
                    }
                }
                MethodProducer::Imported { .. } => {
                    close_imported_call(call, accepted.span, sources, mono_instances, errors, pool);
                }
                MethodProducer::Registry(_) | MethodProducer::Prelude(_) => {}
            }
        }
        plans.push(plan);
    }

    plans.sort_unstable_by(|left, right| {
        left.derived.cmp(&right.derived).then_with(|| {
            left.binder_substitutions
                .iter()
                .map(|idx| idx.raw())
                .cmp(right.binder_substitutions.iter().map(|idx| idx.raw()))
        })
    });
    plans
}

fn close_imported_call(
    call: &DerivedCallSelection,
    span: ori_ir::Span,
    sources: &DerivedCallClosureSources<'_>,
    mono_instances: &mut Vec<MonoInstance>,
    errors: &mut Vec<TypeCheckError>,
    pool: &mut Pool,
) {
    let mut matches = sources
        .imported_impl_sigs
        .iter()
        .filter(|signature| signature.producer == call.producer);
    let Some(signature) = matches.next() else {
        errors.push(TypeCheckError::unsatisfied_bound(
            span,
            "generated method demand references a missing imported producer",
        ));
        return;
    };
    if matches.next().is_some() {
        errors.push(TypeCheckError::unsatisfied_bound(
            span,
            "generated method demand references duplicate imported producers",
        ));
        return;
    }
    push_imported_impl_mono(
        signature,
        call.receiver_type,
        MethodMonoDemand {
            producer: &call.producer,
            traits: sources.traits,
            trait_type: call.trait_type,
            method_name: call.method_name,
        },
        mono_instances,
        pool,
    );
}

fn concrete_derived_receiver(
    accepted: &AcceptedDerivedImpl,
    demanded: Idx,
    pool: &mut Pool,
) -> Option<(Idx, Vec<Idx>)> {
    if accepted.signature.type_params.is_empty() {
        return Some((accepted.owner_type, Vec::new()));
    }
    // Source expressions retain pre-link `Applied` carriers whose cached flags
    // still contain `HAS_VAR`. Re-intern the same nominal receiver with its
    // linked arguments before selecting a producer. The caller must preserve
    // this physical carrier; rebuilding it from canonical semantic args can
    // create a structurally-equal but ABI-distinct body.
    let demanded = substitute_in_pool(pool, demanded, &FxHashMap::default());
    let receiver = if pool.tag(demanded) == Tag::Applied
        && pool.applied_name(demanded) == accepted.owner_name
    {
        demanded
    } else {
        let resolved = pool.resolve_fully(demanded);
        let mut candidates = pool.iter_indices().filter(|&candidate| {
            pool.tag(candidate) == Tag::Applied
                && pool.applied_name(candidate) == accepted.owner_name
                && pool.flags(candidate).is_recordable()
                && pool.structural_eq(candidate, resolved)
        });
        let receiver = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        receiver
    };
    let subst = crate::infer::match_self_type(
        pool,
        accepted.owner_type,
        receiver,
        &accepted.signature.type_params,
    )?;
    let substitutions = accepted
        .signature
        .type_params
        .iter()
        .map(|name| subst.get(name).copied())
        .collect::<Option<Vec<_>>>()?;
    Some((receiver, substitutions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_plan_seed_keeps_first_physical_receiver_and_deduplicates_semantics() {
        let accepted = DerivedImplId::new(7);
        let first_physical = Idx::from_raw(101);
        let equivalent_physical = Idx::from_raw(102);
        let canonical_inner = Idx::from_raw(77);
        let mut seeded = FxHashSet::default();
        let mut pending = Vec::new();

        push_source_plan_seed(
            &mut seeded,
            &mut pending,
            accepted,
            first_physical,
            vec![canonical_inner],
        );
        push_source_plan_seed(
            &mut seeded,
            &mut pending,
            accepted,
            equivalent_physical,
            vec![canonical_inner],
        );

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].accepted, accepted);
        assert_eq!(pending[0].receiver, first_physical);
    }
}
