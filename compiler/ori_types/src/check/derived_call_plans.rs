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

#[derive(Clone, Copy)]
pub(super) struct DerivedCallClosureSources<'a> {
    pub(super) generic_type_params: &'a FxHashMap<Name, Vec<Name>>,
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

/// Materialize accepted derives required by concrete generic-function bounds.
///
/// A method call on a rigid receiver (`T: Debug`, then `value.debug()`) cannot
/// select its concrete producer while the generic body is checked. The
/// concrete receiver first exists on the caller's [`MonoInstance`]. Close that
/// semantic demand here, before generated-body closure, so every executor sees
/// the same exact [`MethodProducer::Derived`] specialization in the typed mono
/// inventory. The normal generated-body worklist below then closes calls made
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
    pool: &Pool,
) -> Option<(Idx, Vec<Idx>)> {
    if accepted.signature.type_params.is_empty() {
        return Some((accepted.owner_type, Vec::new()));
    }
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
