//! Type-checker-owned closure of calls emitted by accepted derived bodies.

use ori_ir::{DerivedImplId, DerivedMethodShape, DerivedTrait, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::pool::substitute::{build_finalized_body_type_map, substitute_in_pool};
use crate::{
    AcceptedDerivedImpl, ConcreteMethodMono, DerivedCallPlan, DerivedCallPosition,
    DerivedCallSelection, DerivedDirectCallSelection, GenericArg, Idx, ImplSig, MethodProducer,
    MonoInstance, Pool, RegistryMethodIdentity, RegistryPreludeIdentity, Tag, TraitRegistry,
    TypeCheckError,
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
    pub(super) impl_sigs: &'a [ImplSig],
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
                            &call.producer,
                            sources.traits,
                            accepted.trait_type,
                            call.method_name,
                            mono_instances,
                            pool,
                        );
                    }
                }
                MethodProducer::Registry(_)
                | MethodProducer::Prelude(_)
                | MethodProducer::Imported { .. } => {}
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

fn build_plan(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    binder_substitutions: Vec<Idx>,
    traits: &TraitRegistry,
    interner: &StringInterner,
    pool: &mut Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<DerivedCallPlan> {
    let positions = collect_call_positions(accepted, receiver, pool, errors)?;
    build_plan_from_positions(
        accepted,
        receiver,
        binder_substitutions,
        positions,
        PlanSelectionSources { traits, interner },
        pool,
        errors,
    )
}

fn collect_call_positions(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    pool: &Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<CallPositions> {
    let mut method_positions = Vec::new();
    let mut direct_positions = Vec::new();
    let resolved = pool.resolve_fully(receiver);
    tracing::debug!(
        target: "ori_types::derived_call_plans",
        derived = ?accepted.id,
        trait_kind = ?accepted.trait_kind,
        receiver = ?receiver,
        receiver_tag = ?pool.tag(receiver),
        resolved = ?resolved,
        resolved_tag = ?pool.tag(resolved),
        "building frozen derived-call plan",
    );

    if pool.is_newtype_ctor(accepted.owner_name) {
        if derived_trait_emits_method_call(accepted.trait_kind, resolved, pool) {
            method_positions.push((DerivedCallPosition::Newtype, resolved));
        }
    } else {
        match pool.tag(resolved) {
            Tag::Struct => {
                for (field, (_, field_type)) in pool.struct_fields(resolved).into_iter().enumerate()
                {
                    let position = DerivedCallPosition::Field(index_u32(field)?);
                    if derived_trait_emits_method_call(accepted.trait_kind, field_type, pool) {
                        method_positions.push((position, field_type));
                    }
                    if accepted.trait_kind == DerivedTrait::Hashable
                        && pool.tag(pool.resolve_fully(field_type)) != Tag::Unit
                    {
                        direct_positions.push(DerivedCallPosition::FieldCombine(index_u32(field)?));
                    }
                }
            }
            Tag::Enum => {
                if accepted.trait_kind == DerivedTrait::Comparable {
                    method_positions.push((DerivedCallPosition::Discriminant, Idx::INT));
                }
                if accepted.trait_kind == DerivedTrait::Hashable {
                    direct_positions.push(DerivedCallPosition::DiscriminantCombine);
                }
                for (variant, (_, fields)) in pool.enum_variants(resolved).into_iter().enumerate() {
                    for (field, field_type) in fields.into_iter().enumerate() {
                        let position = DerivedCallPosition::VariantField {
                            variant: index_u32(variant)?,
                            field: index_u32(field)?,
                        };
                        if derived_trait_emits_method_call(accepted.trait_kind, field_type, pool) {
                            method_positions.push((position, field_type));
                        }
                        if accepted.trait_kind == DerivedTrait::Hashable
                            && pool.tag(pool.resolve_fully(field_type)) != Tag::Unit
                        {
                            direct_positions.push(DerivedCallPosition::VariantFieldCombine {
                                variant: index_u32(variant)?,
                                field: index_u32(field)?,
                            });
                        }
                    }
                }
            }
            _ if accepted.trait_kind == DerivedTrait::Clone => {}
            _ => {
                errors.push(TypeCheckError::unknown_method(
                    accepted.span,
                    receiver,
                    accepted.method_name,
                ));
                return None;
            }
        }
    }
    Some(CallPositions {
        methods: method_positions,
        direct: direct_positions,
    })
}

fn build_plan_from_positions(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    binder_substitutions: Vec<Idx>,
    positions: CallPositions,
    sources: PlanSelectionSources<'_>,
    pool: &mut Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<DerivedCallPlan> {
    let mut calls = Vec::with_capacity(positions.methods.len());
    for (position, nested_receiver) in positions.methods {
        tracing::debug!(
            target: "ori_types::derived_call_plans",
            derived = ?accepted.id,
            ?position,
            nested_receiver = ?nested_receiver,
            nested_tag = ?pool.tag(nested_receiver),
            nested_resolved = ?pool.resolve_fully(nested_receiver),
            "selecting nested generated-call producer",
        );
        match select_producer(
            nested_receiver,
            accepted.trait_type,
            accepted.method_name,
            sources.traits,
            sources.interner,
            pool,
        ) {
            SelectionOutcome::Found(selection) => calls.push(DerivedCallSelection {
                position,
                receiver_type: nested_receiver,
                trait_type: accepted.trait_type,
                method_name: accepted.method_name,
                has_self: selection.has_self,
                producer: selection.producer,
            }),
            SelectionOutcome::Missing => errors.push(TypeCheckError::unknown_method(
                accepted.span,
                nested_receiver,
                accepted.method_name,
            )),
            SelectionOutcome::Ambiguous(count) => {
                let trait_name = sources
                    .traits
                    .get_trait_by_idx(accepted.trait_type)
                    .map_or(accepted.method_name, |entry| entry.name);
                errors.push(TypeCheckError::ambiguous_method(
                    accepted.span,
                    accepted.method_name,
                    vec![trait_name; count],
                ));
            }
        }
    }
    if calls.len() != method_call_count(accepted.trait_kind, receiver, accepted.owner_name, pool)? {
        return None;
    }

    let mut direct_calls = Vec::with_capacity(positions.direct.len());
    if !positions.direct.is_empty() {
        let function_name = sources.interner.intern("hash_combine");
        let Some(identity) = ori_registry::find_prelude_function_id("hash_combine") else {
            errors.push(TypeCheckError::unknown_ident(
                accepted.span,
                function_name,
                Vec::new(),
            ));
            return None;
        };
        let producer = MethodProducer::Prelude(RegistryPreludeIdentity::from_registered(identity));
        direct_calls.extend(positions.direct.into_iter().map(|position| {
            DerivedDirectCallSelection {
                position,
                function_name,
                producer: producer.clone(),
            }
        }));
    }

    Some(DerivedCallPlan {
        derived: accepted.id,
        binder_substitutions,
        calls,
        direct_calls,
    })
}

fn index_u32(index: usize) -> Option<u32> {
    u32::try_from(index).ok()
}

fn derived_trait_emits_method_call(trait_kind: DerivedTrait, receiver: Idx, pool: &Pool) -> bool {
    let resolved = pool.resolve_fully(receiver);
    match trait_kind {
        DerivedTrait::Clone => false,
        DerivedTrait::Eq => pool.builtin_type_tag(resolved).is_none(),
        DerivedTrait::Default => !matches!(
            pool.tag(resolved),
            Tag::Int
                | Tag::Byte
                | Tag::Float
                | Tag::Bool
                | Tag::Str
                | Tag::Char
                | Tag::Unit
                | Tag::Duration
                | Tag::Size
        ),
        DerivedTrait::Hashable | DerivedTrait::Comparable => pool.tag(resolved) != Tag::Unit,
        DerivedTrait::Printable | DerivedTrait::Debug => true,
    }
}

fn method_call_count(
    trait_kind: DerivedTrait,
    receiver: Idx,
    owner_name: Name,
    pool: &Pool,
) -> Option<usize> {
    if pool.is_newtype_ctor(owner_name) {
        return Some(usize::from(derived_trait_emits_method_call(
            trait_kind,
            pool.resolve_fully(receiver),
            pool,
        )));
    }
    let resolved = pool.resolve_fully(receiver);
    let mut count =
        usize::from(trait_kind == DerivedTrait::Comparable && pool.tag(resolved) == Tag::Enum);
    match pool.tag(resolved) {
        Tag::Struct => {
            count += pool
                .struct_fields(resolved)
                .into_iter()
                .filter(|(_, ty)| derived_trait_emits_method_call(trait_kind, *ty, pool))
                .count();
        }
        Tag::Enum => {
            count += pool
                .enum_variants(resolved)
                .into_iter()
                .flat_map(|(_, fields)| fields)
                .filter(|ty| derived_trait_emits_method_call(trait_kind, *ty, pool))
                .count();
        }
        _ if trait_kind == DerivedTrait::Clone => {}
        _ => return None,
    }
    Some(count)
}

fn select_producer(
    receiver: Idx,
    trait_type: Idx,
    method_name: Name,
    traits: &TraitRegistry,
    interner: &StringInterner,
    pool: &mut Pool,
) -> SelectionOutcome {
    let mut active = FxHashSet::default();
    select_producer_inner(
        receiver,
        trait_type,
        method_name,
        traits,
        interner,
        pool,
        &mut active,
    )
}

fn select_producer_inner(
    receiver: Idx,
    trait_type: Idx,
    method_name: Name,
    traits: &TraitRegistry,
    interner: &StringInterner,
    pool: &mut Pool,
    active: &mut FxHashSet<(Idx, Idx)>,
) -> SelectionOutcome {
    let Some(trait_entry) = traits.get_trait_by_idx(trait_type) else {
        return SelectionOutcome::Missing;
    };
    let Some(trait_text) = interner.try_lookup(trait_entry.name) else {
        return SelectionOutcome::Missing;
    };
    let Some(method_text) = interner.try_lookup(method_name) else {
        return SelectionOutcome::Missing;
    };
    let resolved_receiver = pool.resolve_fully(receiver);
    if let Some(receiver_tag) = pool.builtin_type_tag(resolved_receiver) {
        if let Some(method) = ori_registry::find_method(receiver_tag, method_text) {
            if method.trait_name == Some(trait_text) {
                if let Some(identity) = ori_registry::find_method_id(receiver_tag, method_text) {
                    return SelectionOutcome::Found(ProducerSelection {
                        producer: MethodProducer::Registry(
                            RegistryMethodIdentity::from_registered(identity),
                        ),
                        impl_args: Vec::new(),
                        has_self: method.kind == ori_registry::MethodKind::Instance,
                    });
                }
            }
        }
    }

    if !active.insert((receiver, trait_type)) {
        return SelectionOutcome::Missing;
    }
    let mut candidates = Vec::new();
    for (impl_index, implementation) in traits.impls_iter() {
        if implementation.trait_idx != Some(trait_type) {
            continue;
        }
        let Some(method) = implementation.methods.get(&method_name) else {
            continue;
        };
        let subst = if implementation.self_type == receiver {
            FxHashMap::default()
        } else {
            let Some(subst) = crate::infer::match_self_type(
                pool,
                implementation.self_type,
                receiver,
                &implementation.type_params,
            ) else {
                continue;
            };
            subst
        };
        if !impl_bounds_hold(implementation, &subst, traits, interner, pool, active) {
            continue;
        }
        let Some(producer) = traits.method_producer(impl_index, method) else {
            continue;
        };
        let Some(impl_args) = implementation
            .type_params
            .iter()
            .map(|name| subst.get(name).copied())
            .collect::<Option<Vec<_>>>()
            .or_else(|| implementation.type_params.is_empty().then(Vec::new))
        else {
            continue;
        };
        candidates.push((
            implementation.specificity,
            ProducerSelection {
                producer,
                impl_args,
                has_self: method.has_self,
            },
        ));
    }
    active.remove(&(receiver, trait_type));

    let Some(max_specificity) = candidates.iter().map(|(specificity, _)| *specificity).max() else {
        return SelectionOutcome::Missing;
    };
    let mut best = candidates
        .into_iter()
        .filter(|(specificity, _)| *specificity == max_specificity)
        .map(|(_, selection)| selection);
    let Some(selection) = best.next() else {
        return SelectionOutcome::Missing;
    };
    let remaining = best.count();
    if remaining == 0 {
        SelectionOutcome::Found(selection)
    } else {
        SelectionOutcome::Ambiguous(remaining + 1)
    }
}

fn impl_bounds_hold(
    implementation: &crate::ImplEntry,
    subst: &FxHashMap<Name, Idx>,
    traits: &TraitRegistry,
    interner: &StringInterner,
    pool: &mut Pool,
    active: &mut FxHashSet<(Idx, Idx)>,
) -> bool {
    for (parameter, bounds) in implementation
        .type_params
        .iter()
        .zip(&implementation.type_param_bounds)
    {
        let Some(receiver) = subst.get(parameter).copied() else {
            return false;
        };
        for bound in bounds {
            let Some(bound_trait) = traits.get_trait_by_name(*bound) else {
                return false;
            };
            let Some(trait_name) = interner.try_lookup(bound_trait.name) else {
                return false;
            };
            if crate::infer::type_satisfies_named_trait(receiver, trait_name, pool) {
                continue;
            }
            let Some(method) = bound_trait.methods.keys().min().copied() else {
                continue;
            };
            if !matches!(
                select_producer_inner(
                    receiver,
                    bound_trait.idx,
                    method,
                    traits,
                    interner,
                    pool,
                    active,
                ),
                SelectionOutcome::Found(_)
            ) {
                return false;
            }
        }
    }

    for constraint in &implementation.where_clause {
        let constrained =
            crate::pool::substitute::substitute_named_in_pool(pool, constraint.ty, subst);
        for &bound in &constraint.bounds {
            let Some(bound_trait) = traits.get_trait_by_idx(bound) else {
                return false;
            };
            let Some(trait_name) = interner.try_lookup(bound_trait.name) else {
                return false;
            };
            if crate::infer::type_satisfies_named_trait(constrained, trait_name, pool) {
                continue;
            }
            let Some(method) = bound_trait.methods.keys().min().copied() else {
                continue;
            };
            if !matches!(
                select_producer_inner(constrained, bound, method, traits, interner, pool, active,),
                SelectionOutcome::Found(_)
            ) {
                return false;
            }
        }
    }
    true
}

fn push_derived_mono(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    binder_substitutions: &[Idx],
    mono_instances: &mut Vec<MonoInstance>,
    pool: &Pool,
) {
    let producer = MethodProducer::Derived(accepted.id);
    if mono_instances.iter().any(|instance| {
        instance.method_producer.as_ref() == Some(&producer)
            && instance.receiver_type == Some(receiver)
    }) {
        return;
    }

    let shape = accepted.trait_kind.shape();
    let concrete_param_types = if shape.has_other() {
        vec![receiver]
    } else {
        Vec::new()
    };
    let concrete_return_type = concrete_derived_return(shape, receiver);
    let mut body_type_map = vec![(accepted.owner_type, receiver)];
    for (&name, &concrete) in accepted
        .signature
        .type_params
        .iter()
        .zip(binder_substitutions)
    {
        let named = pool
            .iter_indices()
            .find(|&idx| pool.tag(idx) == Tag::Named && pool.named_name(idx) == name);
        if let Some(named) = named {
            body_type_map.push((named, concrete));
        }
    }
    body_type_map.sort_unstable_by_key(|(generic, _)| generic.raw());
    body_type_map.dedup_by_key(|(generic, _)| generic.raw());
    mono_instances.push(MonoInstance::new_method(
        accepted.method_name,
        producer,
        binder_substitutions
            .iter()
            .copied()
            .map(GenericArg::Type)
            .collect(),
        Vec::new(),
        ConcreteMethodMono {
            receiver_type: receiver,
            param_types: concrete_param_types,
            return_type: concrete_return_type,
            body_type_map,
        },
    ));
}

fn concrete_derived_return(shape: DerivedMethodShape, receiver: Idx) -> Idx {
    match shape {
        DerivedMethodShape::BinaryPredicate => Idx::BOOL,
        DerivedMethodShape::UnaryIdentity | DerivedMethodShape::Nullary => receiver,
        DerivedMethodShape::UnaryToInt => Idx::INT,
        DerivedMethodShape::UnaryToStr => Idx::STR,
        DerivedMethodShape::BinaryToOrdering => Idx::ORDERING,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "impl mono materialization carries exact selected trait and existing inventories"
)]
fn push_impl_mono(
    signature: &ImplSig,
    receiver: Idx,
    producer: &MethodProducer,
    traits: &TraitRegistry,
    trait_type: Idx,
    method_name: Name,
    mono_instances: &mut Vec<MonoInstance>,
    pool: &mut Pool,
) {
    if !signature.sig.is_generic()
        || mono_instances.iter().any(|instance| {
            instance.method_producer.as_ref() == Some(producer)
                && instance.receiver_type == Some(receiver)
        })
    {
        return;
    }

    let Some(selection) =
        selected_local_impl(receiver, trait_type, method_name, producer, traits, pool)
    else {
        return;
    };
    let Some(var_subst) = match_rigid_receiver(pool, signature.receiver, receiver) else {
        return;
    };
    let mut concrete_params: Vec<Idx> = signature
        .sig
        .param_types
        .iter()
        .map(|&parameter| substitute_in_pool(pool, parameter, &var_subst))
        .collect();
    if selection.has_self && !concrete_params.is_empty() {
        concrete_params.remove(0);
    }
    let concrete_return = substitute_in_pool(pool, signature.sig.return_type, &var_subst);
    if concrete_params
        .iter()
        .any(|&parameter| !pool.flags(parameter).is_recordable())
        || !pool.flags(concrete_return).is_recordable()
    {
        return;
    }
    let body_type_map = build_finalized_body_type_map(pool, &var_subst, &[]);
    mono_instances.push(MonoInstance::new_method(
        signature.name,
        producer.clone(),
        selection
            .impl_args
            .into_iter()
            .map(GenericArg::Type)
            .collect(),
        Vec::new(),
        ConcreteMethodMono {
            receiver_type: receiver,
            param_types: concrete_params,
            return_type: concrete_return,
            body_type_map,
        },
    ));
}

fn selected_local_impl(
    receiver: Idx,
    trait_type: Idx,
    method_name: Name,
    expected: &MethodProducer,
    traits: &TraitRegistry,
    pool: &Pool,
) -> Option<ProducerSelection> {
    for (impl_index, implementation) in traits.impls_iter() {
        if implementation.trait_idx != Some(trait_type) {
            continue;
        }
        let Some(method) = implementation.methods.get(&method_name) else {
            continue;
        };
        let subst = if implementation.self_type == receiver {
            FxHashMap::default()
        } else {
            crate::infer::match_self_type(
                pool,
                implementation.self_type,
                receiver,
                &implementation.type_params,
            )?
        };
        let producer = traits.method_producer(impl_index, method)?;
        if &producer != expected {
            continue;
        }
        let impl_args = implementation
            .type_params
            .iter()
            .map(|name| subst.get(name).copied())
            .collect::<Option<Vec<_>>>()?;
        return Some(ProducerSelection {
            producer,
            impl_args,
            has_self: method.has_self,
        });
    }
    None
}

fn match_rigid_receiver(pool: &Pool, pattern: Idx, target: Idx) -> Option<FxHashMap<u32, Idx>> {
    let mut subst = FxHashMap::default();
    if match_rigid_receiver_inner(pool, pattern, target, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

fn match_rigid_receiver_inner(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    subst: &mut FxHashMap<u32, Idx>,
) -> bool {
    if pattern == target {
        return true;
    }
    match (pool.tag(pattern), pool.tag(target)) {
        (Tag::RigidVar, _) => {
            let variable = pool.data(pattern);
            if let Some(existing) = subst.get(&variable) {
                *existing == target
            } else {
                subst.insert(variable, target);
                true
            }
        }
        (Tag::Applied, Tag::Applied) => {
            pool.applied_name(pattern) == pool.applied_name(target)
                && pool.applied_args(pattern).len() == pool.applied_args(target).len()
                && pool
                    .applied_args(pattern)
                    .iter()
                    .zip(pool.applied_args(target))
                    .all(|(&left, right)| match_rigid_receiver_inner(pool, left, right, subst))
        }
        _ => false,
    }
}
