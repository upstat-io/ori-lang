use super::*;

#[test]
fn repr_attr_propagates_through_applied_to_concrete_struct() {
    // Why: Distinct field types prevent Pool deduplication from collapsing the two structs.
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 900);
    let field_a = Name::new(0, 901);
    let field_b = Name::new(0, 902);

    let named_idx = pool.named(type_name);
    let base_struct_idx =
        pool.struct_type(type_name, &[(field_a, Idx::FLOAT), (field_b, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    let applied_idx = pool.applied(type_name, &[Idx::INT, Idx::STR]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::STR)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(
        base_struct_idx, mono_struct_idx,
        "test setup: base and mono structs must be distinct pool entries"
    );

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(plan.repr_attr(named_idx), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(base_struct_idx), Some(&ReprAttribute::C));

    assert_eq!(
        plan.repr_attr(mono_struct_idx),
        Some(&ReprAttribute::C),
        "monomorphized concrete struct must inherit #repr(\"c\") from Named parent"
    );
}

#[test]
fn pub_type_propagates_through_applied_to_concrete_struct() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 910);
    let field_x = Name::new(0, 911);

    let named_idx = pool.named(type_name);
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    assert!(plan.is_public_type(named_idx));
    assert!(plan.is_public_type(base_struct_idx));

    assert!(
        plan.is_public_type(mono_struct_idx),
        "monomorphized concrete struct must inherit pub from Named parent"
    );
}

#[test]
fn repr_c_applied_concrete_struct_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 920);
    let field_x = Name::new(0, 921);

    let named_idx = pool.named(type_name);
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let mut plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    plan.join_field_range(mono_struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(mono_struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: #repr(\"c\") monomorphized struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for mono struct idx, got {other:?}"),
    }
}

#[test]
fn pub_applied_concrete_struct_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 930);
    let field_x = Name::new(0, 931);

    let named_idx = pool.named(type_name);
    let base_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct_idx);

    let applied_idx = pool.applied(type_name, &[Idx::INT]);
    let mono_struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(applied_idx, mono_struct_idx);

    assert_ne!(base_struct_idx, mono_struct_idx);

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    plan.join_field_range(mono_struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(mono_struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: pub monomorphized struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for mono struct idx, got {other:?}"),
    }
}

#[test]
fn applied_without_resolution_no_propagation() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 940);
    let field_x = Name::new(0, 941);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let _applied_idx = pool.applied(type_name, &[Idx::STR]);
    let unrelated_struct = pool.struct_type(type_name, &[(field_x, Idx::STR)]);

    assert_ne!(
        struct_idx, unrelated_struct,
        "test setup: must be distinct indices"
    );

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(plan.repr_attr(named_idx), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::C));

    assert_eq!(
        plan.repr_attr(unrelated_struct),
        None,
        "Struct without resolution chain must NOT inherit attr"
    );
}

#[test]
fn multiple_applied_instantiations_all_protected() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 950);
    let field_a = Name::new(0, 951);
    let field_b = Name::new(0, 952);

    let named_idx = pool.named(type_name);
    let base_struct = pool.struct_type(type_name, &[(field_a, Idx::FLOAT), (field_b, Idx::FLOAT)]);
    pool.set_resolution(named_idx, base_struct);

    let applied_1 = pool.applied(type_name, &[Idx::INT, Idx::INT]);
    let mono_1 = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::INT)]);
    pool.set_resolution(applied_1, mono_1);

    let applied_2 = pool.applied(type_name, &[Idx::INT, Idx::STR]);
    let mono_2 = pool.struct_type(type_name, &[(field_a, Idx::INT), (field_b, Idx::STR)]);
    pool.set_resolution(applied_2, mono_2);

    assert_ne!(base_struct, mono_1);
    assert_ne!(base_struct, mono_2);
    assert_ne!(mono_1, mono_2);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &repr_attrs,
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    assert_eq!(plan.repr_attr(mono_1), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(mono_2), Some(&ReprAttribute::C));
    assert!(plan.is_public_type(mono_1));
    assert!(plan.is_public_type(mono_2));
}
