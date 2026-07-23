use super::*;

#[test]
fn repr_c_stored_and_retrieved() {
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(100), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "#repr(\"c\") must be stored as ReprAttribute::C"
    );
}

#[test]
fn repr_packed_stored_and_retrieved() {
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(101), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Packed),);
}

#[test]
fn repr_transparent_stored_and_retrieved() {
    let mut pool = ori_types::Pool::new();
    let field = (ori_ir::Name::from_raw(200), Idx::INT);
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(102), &[field]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Transparent)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::Transparent),
    );
}

#[test]
fn repr_aligned_stored_and_retrieved() {
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(103), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::Aligned(8))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::Aligned(8)),);
}

#[test]
fn no_repr_returns_none() {
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(104), &[]);
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "no #repr → None from query"
    );
}

#[test]
fn repr_c_semantic_pin() {
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(105);
    let f1 = (ori_ir::Name::from_raw(201), Idx::INT);
    let f2 = (ori_ir::Name::from_raw(202), Idx::FLOAT);
    let struct_idx = pool.struct_type(name, &[f1, f2]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(plan.repr_attr(struct_idx), Some(&ReprAttribute::C));
    assert_eq!(plan.repr_attr(Idx::INT), None);
}

#[test]
fn repr_c_aligned_stored_and_retrieved() {
    let mut pool = ori_types::Pool::new();
    let struct_idx = pool.struct_type(ori_ir::Name::from_raw(106), &[]);
    let repr_attrs = [(struct_idx, ori_ir::ReprAttrKind::CAligned(16))];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);
    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::CAligned(16)),
        "#repr(\"c\") + #repr(\"aligned\", 16) must be stored as ReprAttribute::CAligned(16)"
    );
}

#[test]
fn repr_convert_c_aligned_roundtrip() {
    let kind = ori_ir::ReprAttrKind::CAligned(32);
    let attr = crate::pipeline::convert_repr_attr_kind(&kind);
    assert_eq!(attr, ReprAttribute::CAligned(32));
}

#[test]
fn repr_attr_stored_via_named_idx() {
    let mut pool = ori_types::Pool::new();

    let named_idx = pool.named(ori_ir::Name::from_raw(500));

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::C)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::C),
        "Named Idx must store and retrieve #repr attrs — this is the production path"
    );

    let other_named = pool.named(ori_ir::Name::from_raw(501));
    assert_eq!(
        plan.repr_attr(other_named),
        None,
        "repr_attr on unrelated Named Idx must return None"
    );
}

#[test]
fn repr_attr_named_vs_struct_idx_independent() {
    let mut pool = ori_types::Pool::new();
    let name = ori_ir::Name::from_raw(600);

    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[]);

    let repr_attrs = [(named_idx, ori_ir::ReprAttrKind::Packed)];
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &repr_attrs);

    assert_eq!(
        plan.repr_attr(named_idx),
        Some(&ReprAttribute::Packed),
        "Named Idx should have the attr"
    );
    assert_eq!(
        plan.repr_attr(struct_idx),
        None,
        "struct_type Idx should NOT have the attr — different pool entry"
    );
}
