use super::*;

#[test]
fn imported_pub_type_seeded_via_metadata() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 900);
    let field_x = Name::new(0, 901);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: None,
        is_public: true,
    }];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    assert!(
        plan.is_public_type(struct_idx),
        "imported pub type must be seeded as public via metadata"
    );
}

#[test]
fn imported_repr_c_type_seeded_via_metadata() {
    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 910);
    let field_x = Name::new(0, 911);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: false,
    }];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

    assert_eq!(
        plan.repr_attr(struct_idx),
        Some(&ReprAttribute::C),
        "imported #repr(\"c\") type must have repr attr seeded via metadata"
    );
}

#[test]
fn imported_pub_type_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 920);
    let field_x = Name::new(0, 921);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: None,
        is_public: true,
    }];

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

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
                "semantic pin: imported pub struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn imported_repr_c_type_not_narrowed_semantic_pin() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 930);
    let field_x = Name::new(0, 931);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);
    let struct_hash = pool.hash(struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: struct_hash,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: false,
    }];

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &imported_meta,
        &[],
        &[],
        false,
    );

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
                "semantic pin: imported #repr(\"c\") struct must NOT be narrowed"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn no_imported_metadata_allows_narrowing() {
    use crate::narrowing::int::narrow_struct_fields;

    let mut pool = ori_types::Pool::new();
    let type_name = Name::new(0, 940);
    let field_x = Name::new(0, 941);

    let struct_idx = pool.struct_type(type_name, &[(field_x, Idx::INT)]);

    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[],
        &[],
        false,
    );

    plan.join_field_range(struct_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(struct_idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(
                s.fields[0].repr,
                MachineRepr::Int {
                    width: IntWidth::I8,
                    signed: true,
                },
                "Negative test: unprotected struct must be narrowed to i8"
            );
        }
        other => panic!("expected Struct repr, got {other:?}"),
    }
}

#[test]
fn direct_test_body_constructs_join_field_ranges_before_narrowing() {
    use ori_arc::ir::{
        ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue, ValueRepr,
    };
    use ori_arc::{ArcBlockId, ArcVarId};

    fn test_body(name: Name, struct_name: Name, struct_idx: Idx, x: i64, y: i64) -> ArcFunction {
        let x_var = ArcVarId::new(0);
        let y_var = ArcVarId::new(1);
        let struct_var = ArcVarId::new(2);
        ArcFunction {
            name,
            return_type: struct_idx,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: x_var,
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(x)),
                    },
                    ArcInstr::Let {
                        dst: y_var,
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(y)),
                    },
                    ArcInstr::Construct {
                        dst: struct_var,
                        ty: struct_idx,
                        ctor: CtorKind::Struct(struct_name),
                        args: vec![x_var, y_var],
                    },
                ],
                terminator: ArcTerminator::Return { value: struct_var },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT, Idx::INT, struct_idx],
            var_reprs: vec![ValueRepr::Scalar, ValueRepr::Scalar, ValueRepr::Scalar],
            spans: vec![vec![None; 3]],
            ..ArcFunction::default()
        }
    }

    let mut pool = Pool::new();
    let struct_name = Name::new(0, 960);
    let field_x = Name::new(0, 961);
    let field_y = Name::new(0, 962);
    let struct_idx = pool.struct_type(struct_name, &[(field_x, Idx::INT), (field_y, Idx::INT)]);
    let bodies = [
        test_body(Name::new(0, 963), struct_name, struct_idx, 1, 2),
        test_body(Name::new(0, 964), struct_name, struct_idx, 100, 20),
        test_body(Name::new(0, 965), struct_name, struct_idx, 999, 200),
    ];

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &bodies,
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[],
        &[],
        false,
    );

    let Some(MachineRepr::Struct(repr)) = plan.get_repr(struct_idx) else {
        panic!("direct test-body struct must have a representation");
    };
    for field_name in [field_x, field_y] {
        let field = repr
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .unwrap_or_else(|| panic!("direct test-body struct field must be retained"));
        assert_eq!(
            field.repr,
            MachineRepr::Int {
                width: IntWidth::I16,
                signed: true,
            },
            "all direct test-body construction sites must contribute before narrowing"
        );
    }
}

#[test]
fn imported_metadata_hash_not_in_pool_ignored() {
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();

    let type_name = interner.intern("Local");
    let field_r = interner.intern("r");
    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_r, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let imported_meta = vec![ExportedTypeMetadata {
        merkle_hash: 0xDEAD_BEEF_CAFE_1234,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: true,
    }];

    let plan_with_bogus_meta = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &imported_meta,
        &[],
        &[],
        false,
    );
    let plan_without_meta = crate::compute_repr_plan_with_interner(
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

    assert_eq!(
        plan_with_bogus_meta.get_repr(struct_idx),
        plan_without_meta.get_repr(struct_idx),
        "Imported metadata with a hash absent from the local pool must not change layout"
    );
}
