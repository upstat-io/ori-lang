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
        "hash$m$9_AWrap_int3_int$im$"
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
