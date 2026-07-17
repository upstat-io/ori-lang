//! Exact nested-method producer selection and bound validation.

use super::{
    FxHashMap, FxHashSet, Idx, MethodProducer, Name, Pool, ProducerSelection,
    RegistryMethodIdentity, SelectionOutcome, StringInterner, Tag, TraitRegistry,
};

pub(super) fn select_producer(
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
    let trait_name = traits
        .get_trait_by_idx(trait_type)
        .map(|entry| entry.name)
        .or_else(|| match pool.tag(trait_type) {
            Tag::Named => Some(pool.named_name(trait_type)),
            _ => None,
        });
    let Some(trait_text) = trait_name.and_then(|name| interner.try_lookup(name)) else {
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

pub(super) fn selected_local_impl(
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
        let Some(producer) = traits.method_producer(impl_index, method) else {
            continue;
        };
        if &producer != expected {
            continue;
        }
        let impl_args = implementation
            .type_params
            .iter()
            .map(|name| subst.get(name).copied())
            .collect::<Option<Vec<_>>>();
        let Some(impl_args) = impl_args else {
            continue;
        };
        return Some(ProducerSelection {
            producer,
            impl_args,
            has_self: method.has_self,
        });
    }
    None
}
