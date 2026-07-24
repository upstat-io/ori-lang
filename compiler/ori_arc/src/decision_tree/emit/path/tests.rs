use ori_ir::Name;
use ori_types::{Idx, Pool};

use super::lookup_struct_field;

#[test]
fn struct_field_name_with_reordered_declaration_maps_to_declaration_slot() {
    let mut pool = Pool::new();
    let struct_name = Name::from_raw(20);
    let first_by_name = Name::from_raw(1);
    let second_by_name = Name::from_raw(2);
    let struct_ty = pool.struct_type(
        struct_name,
        &[(second_by_name, Idx::BOOL), (first_by_name, Idx::INT)],
    );

    assert_eq!(
        lookup_struct_field(&pool, struct_ty, first_by_name),
        Some((1, Idx::INT))
    );
}

#[test]
fn struct_field_name_missing_from_type_returns_none() {
    let mut pool = Pool::new();
    let struct_name = Name::from_raw(20);
    let field = Name::from_raw(1);
    let missing = Name::from_raw(2);
    let struct_ty = pool.struct_type(struct_name, &[(field, Idx::INT)]);

    assert_eq!(lookup_struct_field(&pool, struct_ty, missing), None);
}
