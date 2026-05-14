//! `lookup_burden` dispatch tests — builtin path (`&'static`), user path
//! (`TypeRegistry`-borrowed), miss paths (`None`), and `Burden` trait
//! parity across the two partition variants.

use ori_ir::{Name, Span};
use ori_registry::burden::table::{TYPE_ID_BOOL, TYPE_ID_INT, TYPE_ID_STR};
use ori_types::burden::{UserBurdenSpec, UserOwnedField};
use ori_types::{FieldDef, Idx, TypeRegistry, Visibility};

use super::lookup_burden;
use crate::lower::burden::{Burden, BurdenRef, TypeRef};

fn test_name(s: &str) -> Name {
    Name::from_raw(
        s.as_bytes()
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b))),
    )
}

fn lookup_required<'a>(ty: TypeRef, registry: &'a TypeRegistry, label: &'a str) -> BurdenRef<'a> {
    match lookup_burden(ty, registry) {
        Some(burden) => burden,
        None => panic!("expected burden lookup hit for {label}"),
    }
}

fn registered_struct_with_burden(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
    burden: Option<UserBurdenSpec>,
) {
    let fields = vec![FieldDef {
        name: test_name("payload"),
        ty: Idx::INT,
        span: Span::DUMMY,
        visibility: Visibility::Public,
    }];
    registry.register_struct(
        test_name(name),
        idx,
        vec![],
        fields,
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        burden,
    );
}

#[test]
fn lookup_builtin_int_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let burden = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    match burden {
        BurdenRef::Builtin(_) => {}
        BurdenRef::User(_) => panic!("Builtin lookup must return Builtin variant"),
    }
    // Scalar primitive — empty owned/borrowed/variant lists.
    assert!(
        burden.owned_fields().next().is_none(),
        "int has no owned fields",
    );
    assert!(burden.user_drop().is_none(), "int has no user drop",);
}

#[test]
fn lookup_builtin_str_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let burden = lookup_required(TypeRef::Builtin(TYPE_ID_STR), &registry, "str");
    match burden {
        BurdenRef::Builtin(_) => {}
        BurdenRef::User(_) => panic!("str must dispatch through builtin path"),
    }
}

#[test]
fn lookup_builtin_bool_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let result = lookup_burden(TypeRef::Builtin(TYPE_ID_BOOL), &registry);
    assert!(matches!(result, Some(BurdenRef::Builtin(_))));
}

#[test]
fn lookup_user_returns_registry_borrowed_burden_ref() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(1024);
    let user_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        ..UserBurdenSpec::default()
    };
    registered_struct_with_burden(&mut registry, "Holder", user_idx, Some(user_spec));

    let burden = lookup_required(TypeRef::User(user_idx), &registry, "Holder user_idx");
    match burden {
        BurdenRef::User(spec) => {
            assert!(spec.self_heap_alloc);
            assert_eq!(spec.owned_fields.len(), 1);
            assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
        }
        BurdenRef::Builtin(_) => panic!("User lookup must return User variant"),
    }
}

#[test]
fn lookup_user_unregistered_returns_none() {
    let registry = TypeRegistry::new();
    let unregistered = Idx::from_raw(9999);
    let result = lookup_burden(TypeRef::User(unregistered), &registry);
    assert!(
        result.is_none(),
        "unregistered user idx must lookup as None"
    );
}

#[test]
fn lookup_user_registered_without_burden_returns_none() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(2048);
    registered_struct_with_burden(&mut registry, "NoBurden", user_idx, None);

    let result = lookup_burden(TypeRef::User(user_idx), &registry);
    assert!(
        result.is_none(),
        "registered type with burden=None must lookup as None",
    );
}

#[test]
fn dispatch_parity_owned_fields_across_partition() {
    // Build a user spec structurally equivalent to a builtin's empty shape:
    // both yield an empty owned-fields iterator. Burden trait method dispatch
    // must produce equivalent observations regardless of partition side.
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(4096);
    let user_spec = UserBurdenSpec::default(); // empty across all dims
    registered_struct_with_burden(&mut registry, "EmptyUser", user_idx, Some(user_spec));

    let builtin = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    let user = lookup_required(TypeRef::User(user_idx), &registry, "EmptyUser user_idx");

    let builtin_owned: Vec<_> = builtin.owned_fields().collect();
    let user_owned: Vec<_> = user.owned_fields().collect();
    assert_eq!(
        builtin_owned.len(),
        user_owned.len(),
        "parity: both partitions report zero owned fields for empty shape",
    );
    assert_eq!(
        builtin.self_heap_alloc(),
        user.self_heap_alloc(),
        "parity: empty shapes agree on self_heap_alloc",
    );
    assert_eq!(
        builtin.user_drop(),
        user.user_drop(),
        "parity: empty shapes agree on user_drop",
    );
}

#[test]
fn dispatch_parity_borrowed_fields_zero_arity() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(8192);
    let user_spec = UserBurdenSpec::default();
    registered_struct_with_burden(&mut registry, "EmptyU2", user_idx, Some(user_spec));

    let builtin = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    let user = lookup_required(TypeRef::User(user_idx), &registry, "EmptyU2 user_idx");

    assert!(
        builtin.borrowed_fields().next().is_none(),
        "builtin int has zero borrowed fields",
    );
    assert!(
        user.borrowed_fields().next().is_none(),
        "user empty spec has zero borrowed fields",
    );
}

#[test]
fn lookup_user_idx_partition_does_not_query_builtin_table() {
    // A user Idx whose raw value happens to match a builtin TypeId's raw value
    // MUST NOT route through BurdenRegistry::lookup_builtin. The partition is
    // the TypeRef variant, not the raw value.
    let mut registry = TypeRegistry::new();
    // Idx::from_raw(0) is Idx::INT — same raw value as TYPE_ID_INT's discriminant.
    // The TypeRef::User variant ensures we hit TypeRegistry, not BURDEN_TABLE.
    let collision_idx = Idx::INT;
    registered_struct_with_burden(&mut registry, "CollisionRaw", collision_idx, None);

    let result = lookup_burden(TypeRef::User(collision_idx), &registry);
    assert!(
        result.is_none(),
        "User-tagged collision_idx with burden=None routes through TypeRegistry, not BURDEN_TABLE",
    );

    // Same raw value, Builtin-tagged: routes through BURDEN_TABLE, finds int.
    let builtin_result = lookup_burden(TypeRef::Builtin(TYPE_ID_INT), &registry);
    assert!(matches!(builtin_result, Some(BurdenRef::Builtin(_))));
}
