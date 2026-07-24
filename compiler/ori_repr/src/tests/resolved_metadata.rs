use super::*;

#[test]
fn repr_attr_propagates_to_resolved_struct_idx() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 700);
    let field_x = Name::new(0, 701);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named idx must retain its #repr(\"c\") attr"
    );

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "resolved struct idx must inherit #repr(\"c\") from named idx \
         via resolution chain — codegen uses the resolved idx"
    );
}

#[test]
fn repr_packed_propagates_to_resolved_struct_idx() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 710);
    let field_x = Name::new(0, 711);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Packed),
        "resolved struct idx must inherit #repr(\"packed\")"
    );
}

#[test]
fn repr_c_aligned_propagates_to_resolved_struct_idx() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 720);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::CAligned(16))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::CAligned(16)),
        "resolved struct idx must inherit #repr(\"c\", aligned 16)"
    );
}

#[test]
fn repr_transparent_propagates_to_resolved_struct_idx() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 730);
    let field_x = Name::new(0, 731);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Transparent)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Transparent),
        "resolved struct idx must inherit #repr(\"transparent\")"
    );
}

#[test]
fn pub_type_propagates_to_resolved_struct_idx() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 740);
    let field_x = Name::new(0, 741);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

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

    assert!(
        plan.is_public_type(named_idx),
        "Named idx must remain public"
    );
    assert!(
        plan.is_public_type(struct_idx),
        "resolved struct idx must inherit pub status from named idx"
    );
}

#[test]
fn repr_attr_no_resolution_no_propagation() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 750);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[]);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named idx should have the attr"
    );
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "Without resolution chain, struct_type idx must NOT inherit the attr"
    );
}

#[test]
fn repr_c_resolved_idx_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 760);
    let field_x = Name::new(0, 761);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let mut plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    plan.join_field_range(named_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: #repr(\"c\") resolved struct idx must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for resolved idx, got {other:?}"),
    }
}

#[test]
fn pub_resolved_idx_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 770);
    let field_x = Name::new(0, 771);

    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

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

    plan.join_field_range(named_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                },
                "semantic pin: pub resolved struct idx must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr for resolved idx, got {other:?}"),
    }
}
