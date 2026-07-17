//! Concrete specializations demanded by compiler-generated derived bodies.

use super::{
    concrete_sig_for_instance, mangle_mono_name, MonoFunction, MonoFunctionIdentity,
    MonoFunctionOrigin,
};
use ori_ir::{DerivedMethodShape, Name, StringInterner};
use ori_types::{
    AcceptedDerivedImpl, GenericArg, Idx, MethodProducer, MonoInstance, Pool, Tag, TypeFlags,
};

/// Failure while turning one generated method demand into a concrete body.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DerivedMonoMaterializationError {
    /// A generic accepted derive did not retain an applied owner template.
    #[error("generic accepted derive owner {owner:?} is not an applied type")]
    InvalidAcceptedOwner { owner: Idx },
    /// The demanded receiver does not belong to the supplied type pool.
    #[error("generated derived call carries invalid receiver {receiver:?}")]
    InvalidReceiver { receiver: Idx },
    /// A receiver still contains inference or generic state.
    #[error("generated derived call receiver {receiver:?} is not concrete")]
    NonConcreteReceiver { receiver: Idx },
    /// A resolved body lost its exact applied receiver and no carrier recovers it.
    #[error(
        "generated derived call for owner {owner:?} carries resolved receiver {receiver:?}, but no concrete applied receiver resolves to that body"
    )]
    MissingAppliedReceiver { owner: Name, receiver: Idx },
    /// More than one applied receiver could own the same resolved body.
    #[error(
        "generated derived call for owner {owner:?} carries ambiguous resolved receiver {receiver:?}: {candidates} concrete applied receivers resolve to that body"
    )]
    AmbiguousAppliedReceiver {
        owner: Name,
        receiver: Idx,
        candidates: usize,
    },
}

/// Materialize one generic accepted derive for an exact generated receiver.
///
/// `Ok(None)` means the accepted derive belongs to a different receiver head.
/// A resolved struct/enum demand is recovered through the unique concrete
/// `Applied(owner, args)` carrier already registered in the pool.
pub fn materialize_derived_mono_for_receiver(
    accepted: &AcceptedDerivedImpl,
    demanded_receiver: Idx,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<Option<MonoFunction>, DerivedMonoMaterializationError> {
    if !accepted.signature.is_generic() {
        return Ok(None);
    }
    if !pool.is_valid_idx(accepted.owner_type) || pool.tag(accepted.owner_type) != Tag::Applied {
        return Err(DerivedMonoMaterializationError::InvalidAcceptedOwner {
            owner: accepted.owner_type,
        });
    }
    if !pool.is_valid_idx(demanded_receiver) {
        return Err(DerivedMonoMaterializationError::InvalidReceiver {
            receiver: demanded_receiver,
        });
    }

    let Some(receiver) = recover_applied_receiver(accepted, demanded_receiver, pool)? else {
        return Ok(None);
    };
    if !is_concrete_receiver(receiver, accepted, pool) {
        return Err(DerivedMonoMaterializationError::NonConcreteReceiver { receiver });
    }

    let receiver_args = pool.applied_args(receiver);
    let impl_args: Vec<_> = receiver_args
        .iter()
        .copied()
        .map(GenericArg::Type)
        .collect();
    let mangled_name = mangle_mono_name(
        accepted.method_name,
        &[],
        &impl_args,
        &[],
        Some(receiver),
        interner,
        pool,
    );
    let shape = accepted.trait_kind.shape();
    let concrete_param_types = if shape.has_other() {
        vec![receiver]
    } else {
        Vec::new()
    };
    let concrete_return_type = concrete_return_type(shape, receiver);
    let mut body_type_map = vec![(accepted.owner_type, receiver)];
    for (&generic, &concrete) in pool
        .applied_args(accepted.owner_type)
        .iter()
        .zip(&receiver_args)
    {
        if generic != concrete {
            body_type_map.push((generic, concrete));
        }
    }
    body_type_map.sort_unstable_by_key(|(generic, _)| generic.raw());
    body_type_map.dedup_by_key(|(generic, _)| generic.raw());
    let instance = MonoInstance::new_method(
        accepted.method_name,
        MethodProducer::Derived(accepted.id),
        impl_args,
        Vec::new(),
        ori_types::ConcreteMethodMono {
            receiver_type: receiver,
            param_types: concrete_param_types,
            return_type: concrete_return_type,
            body_type_map,
        },
    );
    let sig = concrete_sig_for_instance(&instance, &accepted.signature, pool, mangled_name);

    Ok(Some(MonoFunction {
        mangled_name,
        origin: MonoFunctionOrigin::Derived(accepted.id),
        identity: MonoFunctionIdentity::generated(&instance),
        sig,
        body_type_map: instance.body_type_map.iter().copied().collect(),
        is_imported: false,
        receiver_type_name: Some(accepted.owner_name),
    }))
}

fn recover_applied_receiver(
    accepted: &AcceptedDerivedImpl,
    demanded: Idx,
    pool: &Pool,
) -> Result<Option<Idx>, DerivedMonoMaterializationError> {
    if pool.tag(demanded) == Tag::Applied {
        return Ok((pool.applied_name(demanded) == accepted.owner_name).then_some(demanded));
    }

    let resolved = pool.resolve_fully(demanded);
    let resolved_owner = match pool.tag(resolved) {
        Tag::Struct => Some(pool.struct_name(resolved)),
        Tag::Enum => Some(pool.enum_name(resolved)),
        _ => None,
    };
    if resolved_owner != Some(accepted.owner_name) {
        return Ok(None);
    }

    let candidates: Vec<_> = pool
        .iter_indices()
        .filter(|&candidate| {
            candidate != accepted.owner_type
                && pool.tag(candidate) == Tag::Applied
                && pool.applied_name(candidate) == accepted.owner_name
                && is_concrete_receiver(candidate, accepted, pool)
                && pool.structural_eq(candidate, demanded)
        })
        .collect();
    match candidates.as_slice() {
        [receiver] => Ok(Some(*receiver)),
        [] => Err(DerivedMonoMaterializationError::MissingAppliedReceiver {
            owner: accepted.owner_name,
            receiver: demanded,
        }),
        _ => Err(DerivedMonoMaterializationError::AmbiguousAppliedReceiver {
            owner: accepted.owner_name,
            receiver: demanded,
            candidates: candidates.len(),
        }),
    }
}

fn is_concrete_receiver(receiver: Idx, accepted: &AcceptedDerivedImpl, pool: &Pool) -> bool {
    pool.flags(receiver).is_recordable()
        && pool.applied_args(receiver).iter().all(|&argument| {
            !contains_declaration_binder(argument, &accepted.signature.type_params, pool)
        })
}

fn contains_declaration_binder(ty: Idx, binders: &[Name], pool: &Pool) -> bool {
    if !pool.is_valid_idx(ty) {
        return true;
    }
    if pool
        .flags(ty)
        .intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR)
    {
        return true;
    }
    match pool.tag(ty) {
        Tag::Named => binders.contains(&pool.named_name(ty)),
        Tag::Applied => pool
            .applied_args(ty)
            .iter()
            .any(|&argument| contains_declaration_binder(argument, binders, pool)),
        _ => false,
    }
}

const fn concrete_return_type(shape: DerivedMethodShape, receiver: Idx) -> Idx {
    match shape {
        DerivedMethodShape::BinaryPredicate => Idx::BOOL,
        DerivedMethodShape::UnaryIdentity | DerivedMethodShape::Nullary => receiver,
        DerivedMethodShape::UnaryToInt => Idx::INT,
        DerivedMethodShape::UnaryToStr => Idx::STR,
        DerivedMethodShape::BinaryToOrdering => Idx::ORDERING,
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::{DerivedImplId, DerivedTrait, Span, StringInterner};
    use ori_types::{AcceptedDerivedImpl, FunctionSig, Idx, Pool};

    use super::{materialize_derived_mono_for_receiver, DerivedMonoMaterializationError};

    fn accepted_hashable(
        interner: &StringInterner,
        pool: &mut Pool,
    ) -> (AcceptedDerivedImpl, ori_ir::Name) {
        let owner_name = interner.intern("Wrap");
        let binder_name = interner.intern("T");
        let binder = pool.named(binder_name);
        let owner = pool.applied(owner_name, &[binder]);
        let method = interner.intern("hash");
        let mut signature =
            FunctionSig::synthetic(method, vec![interner.intern("self")], vec![owner], Idx::INT);
        signature.type_params = vec![binder_name];
        signature.type_param_bounds = vec![vec![interner.intern("Hashable")]];
        signature.generic_param_mapping = vec![None];
        signature.populate_hashes(pool);
        (
            AcceptedDerivedImpl {
                id: DerivedImplId::new(3),
                owner_name,
                owner_type: owner,
                trait_type: pool.named(interner.intern("Hashable")),
                trait_kind: DerivedTrait::Hashable,
                method_name: method,
                signature,
                span: Span::DUMMY,
            },
            owner_name,
        )
    }

    #[test]
    fn resolved_body_recovers_exact_applied_receiver_and_impl_argument() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let (accepted, owner_name) = accepted_hashable(&interner, &mut pool);
        let applied = pool.applied(owner_name, &[Idx::INT]);
        let body = pool.struct_type(owner_name, &[(interner.intern("inner"), Idx::INT)]);
        pool.set_resolution(applied, body);

        let mono = materialize_derived_mono_for_receiver(&accepted, body, &interner, &pool)
            .unwrap_or_else(|error| panic!("unique applied receiver must recover: {error}"))
            .unwrap_or_else(|| panic!("receiver belongs to the accepted generic derive"));

        assert_eq!(mono.identity.receiver_type(), Some(applied));
        assert_eq!(mono.sig.param_types, vec![applied]);
        assert_eq!(mono.sig.return_type, Idx::INT);
        assert!(mono.identity.instance_ids().is_empty());
        assert_eq!(
            interner.lookup(mono.mangled_name),
            "hash$m$5_SWrap3_int$im$"
        );
    }

    #[test]
    fn resolved_body_with_two_applied_owners_fails_closed() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let (accepted, owner_name) = accepted_hashable(&interner, &mut pool);
        let int_owner = pool.applied(owner_name, &[Idx::INT]);
        let str_owner = pool.applied(owner_name, &[Idx::STR]);
        let body = pool.struct_type(owner_name, &[]);
        pool.set_resolution(int_owner, body);
        pool.set_resolution(str_owner, body);

        let Err(error) = materialize_derived_mono_for_receiver(&accepted, body, &interner, &pool)
        else {
            panic!("one resolved body must not guess between concrete receiver arguments")
        };

        assert!(matches!(
            error,
            DerivedMonoMaterializationError::AmbiguousAppliedReceiver { candidates: 2, .. }
        ));
    }
}
