//! Concrete monomorphization inventory materialization.

use super::{
    build_finalized_body_type_map, selected_local_impl, substitute_in_pool, AcceptedDerivedImpl,
    ConcreteMethodMono, DerivedMethodShape, FxHashMap, GenericArg, Idx, ImplSig, ImportedImplSig,
    MethodProducer, MonoInstance, Name, Pool, Tag, TraitRegistry,
};

#[derive(Clone, Copy)]
pub(super) struct MethodMonoDemand<'a> {
    pub(super) producer: &'a MethodProducer,
    pub(super) traits: &'a TraitRegistry,
    pub(super) trait_type: Idx,
    pub(super) method_name: Name,
}

pub(super) fn push_derived_mono(
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

pub(super) fn push_impl_mono(
    signature: &ImplSig,
    receiver: Idx,
    demand: MethodMonoDemand<'_>,
    mono_instances: &mut Vec<MonoInstance>,
    pool: &mut Pool,
) {
    if !signature.sig.is_generic()
        || mono_instances.iter().any(|instance| {
            instance.method_producer.as_ref() == Some(demand.producer)
                && instance.receiver_type == Some(receiver)
        })
    {
        return;
    }

    let Some(selection) = selected_local_impl(
        receiver,
        demand.trait_type,
        demand.method_name,
        demand.producer,
        demand.traits,
        pool,
    ) else {
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
        demand.producer.clone(),
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

pub(super) fn push_imported_impl_mono(
    signature: &ImportedImplSig,
    receiver: Idx,
    demand: MethodMonoDemand<'_>,
    mono_instances: &mut Vec<MonoInstance>,
    pool: &mut Pool,
) {
    if !signature.sig.is_generic()
        || mono_instances.iter().any(|instance| {
            instance.method_producer.as_ref() == Some(demand.producer)
                && instance.receiver_type == Some(receiver)
        })
    {
        return;
    }
    let Some(selection) = selected_local_impl(
        receiver,
        demand.trait_type,
        demand.method_name,
        demand.producer,
        demand.traits,
        pool,
    ) else {
        return;
    };
    let Some(named_subst) = crate::infer::match_self_type(
        pool,
        signature.receiver,
        receiver,
        &signature.sig.type_params,
    ) else {
        return;
    };
    let mut concrete_params: Vec<_> = signature
        .sig
        .param_types
        .iter()
        .map(|&parameter| {
            crate::pool::substitute::substitute_named_in_pool(pool, parameter, &named_subst)
        })
        .collect();
    if signature.has_self && !concrete_params.is_empty() {
        concrete_params.remove(0);
    }
    let concrete_return = crate::pool::substitute::substitute_named_in_pool(
        pool,
        signature.sig.return_type,
        &named_subst,
    );
    if concrete_params
        .iter()
        .any(|&parameter| !pool.flags(parameter).is_recordable())
        || !pool.flags(concrete_return).is_recordable()
    {
        return;
    }

    let mut body_type_map = vec![(signature.receiver, receiver)];
    for (name, concrete) in named_subst {
        if let Some(generic) = pool
            .iter_indices()
            .find(|&idx| pool.tag(idx) == Tag::Named && pool.named_name(idx) == name)
        {
            body_type_map.push((generic, concrete));
        }
    }
    body_type_map.sort_unstable_by_key(|(generic, _)| generic.raw());
    body_type_map.dedup_by_key(|(generic, _)| generic.raw());
    mono_instances.push(MonoInstance::new_method(
        signature.name,
        demand.producer.clone(),
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
