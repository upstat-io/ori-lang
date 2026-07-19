use ori_ir::StringInterner;
use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcBlock, ArcInstr, ArcParam, ArcTerminator, ArcValue};
use crate::test_helpers::{b, make_func_named, v};
use crate::Ownership;

use super::specialize_polymorphic_lambdas;
use super::type_predicates::has_concrete_params;
use super::type_resolve::is_concrete_function;

#[test]
fn nested_bound_vars_are_not_concrete_function_components() {
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 7);
    let list_bound = pool.list(bound);
    let return_polymorphic = pool.function0(list_bound);
    let param_polymorphic = pool.function1(list_bound, Idx::INT);

    assert!(!is_concrete_function(&pool, return_polymorphic));
    assert!(!has_concrete_params(&pool, param_polymorphic));
}

#[test]
fn single_instantiation_preserves_nominal_callable_component_identity() {
    let mut pool = Pool::new();
    let interner = StringInterner::new();
    let parent_name = interner.intern("parent");
    let lambda_name = interner.intern("parent.__lambda0");
    let bundle_name = interner.intern("Bundle");
    let items_name = interner.intern("items");
    let first_bound = pool.intern(Tag::BoundVar, 21);
    let second_bound = pool.intern(Tag::BoundVar, 22);
    let schema_function = pool.function(&[first_bound, second_bound], first_bound);
    let nominal_bundle = pool.named(bundle_name);
    let bundle_body = pool.struct_type(bundle_name, &[(items_name, Idx::STR)]);
    pool.set_resolution(nominal_bundle, bundle_body);
    let list_int = pool.list(Idx::INT);
    let concrete_function = pool.function(&[nominal_bundle, list_int], nominal_bundle);

    let mut parent = make_func_named(
        parent_name,
        Vec::new(),
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: v(0),
                    ty: schema_function,
                    func: lambda_name,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: concrete_function,
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![schema_function, concrete_function, Idx::UNIT],
    );
    let lambda = make_func_named(
        lambda_name,
        vec![
            ArcParam {
                var: v(0),
                ty: first_bound,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: v(1),
                ty: second_bound,
                ownership: Ownership::Owned,
            },
        ],
        first_bound,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![first_bound, second_bound],
    );
    let mut lambdas = vec![lambda];

    let result = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &pool, &interner);

    assert!(
        result.is_ok(),
        "unexpected specialization error: {result:?}"
    );
    assert_ne!(nominal_bundle, bundle_body);
    assert_eq!(lambdas[0].params[0].ty, nominal_bundle);
    assert_eq!(lambdas[0].return_type, nominal_bundle);
    assert!(matches!(
        parent.blocks[0].body[0],
        ArcInstr::PartialApply { ty, .. } if ty == concrete_function
    ));
}

#[test]
fn unused_non_capturing_polymorphic_template_is_eliminated_exactly() {
    let mut pool = Pool::new();
    let interner = StringInterner::new();
    let parent_name = interner.intern("parent");
    let lambda_name = interner.intern("parent.__lambda0");
    let bound = pool.intern(Tag::BoundVar, 11);
    let function_type = pool.function1(bound, bound);

    let mut parent = make_func_named(
        parent_name,
        Vec::new(),
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: v(0),
                    ty: function_type,
                    func: lambda_name,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::LitValue::Int(7)),
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![function_type, Idx::INT],
    );
    let lambda = make_func_named(
        lambda_name,
        vec![ArcParam {
            var: v(0),
            ty: bound,
            ownership: Ownership::Owned,
        }],
        bound,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: bound,
                value: ArcValue::Var(v(0)),
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![bound, bound],
    );
    let mut lambdas = vec![lambda];

    let result = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &pool, &interner);

    assert!(result.is_ok());
    assert!(lambdas.is_empty());
    assert_eq!(parent.var_type(v(0)), Idx::NEVER);
    assert!(parent.blocks.iter().all(|block| block.body.iter().all(
        |instruction| !matches!(instruction, ArcInstr::PartialApply { func, .. } if *func == lambda_name)
    )));
}

#[test]
fn unused_capturing_polymorphic_template_is_not_eliminated() {
    let mut pool = Pool::new();
    let interner = StringInterner::new();
    let parent_name = interner.intern("parent");
    let lambda_name = interner.intern("parent.__lambda0");
    let bound = pool.intern(Tag::BoundVar, 13);
    let function_type = pool.function1(bound, bound);

    let mut parent = make_func_named(
        parent_name,
        vec![ArcParam {
            var: v(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: v(1),
                    ty: function_type,
                    func: lambda_name,
                    args: vec![v(0)],
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::LitValue::Int(7)),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, function_type, Idx::INT],
    );
    let mut lambda = make_func_named(
        lambda_name,
        vec![
            ArcParam {
                var: v(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: v(1),
                ty: bound,
                ownership: Ownership::Owned,
            },
        ],
        bound,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: v(2),
                ty: bound,
                value: ArcValue::Var(v(1)),
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, bound, bound],
    );
    lambda.num_captures = 1;
    let mut lambdas = vec![lambda];

    let result = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &pool, &interner);

    assert!(result.is_err());
    assert_eq!(lambdas.len(), 1);
    assert!(matches!(
        parent.blocks[0].body[0],
        ArcInstr::PartialApply { func, .. } if func == lambda_name
    ));
}

#[test]
fn specialization_rejects_compound_identity_missing_from_type_phase() {
    let mut pool = Pool::new();
    let interner = StringInterner::new();
    let parent_name = interner.intern("parent");
    let lambda_name = interner.intern("parent.__lambda0");
    let bound = pool.intern(Tag::BoundVar, 29);
    let list_bound = pool.list(bound);
    let schema_function = pool.function1(bound, bound);
    let concrete_function = pool.function1(Idx::INT, Idx::INT);

    let mut parent = make_func_named(
        parent_name,
        Vec::new(),
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: v(0),
                    ty: schema_function,
                    func: lambda_name,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: concrete_function,
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![schema_function, concrete_function, Idx::UNIT],
    );
    let lambda = make_func_named(
        lambda_name,
        vec![ArcParam {
            var: v(0),
            ty: bound,
            ownership: Ownership::Owned,
        }],
        bound,
        vec![ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![bound, list_bound],
    );
    let mut lambdas = vec![lambda];
    let pool_len = pool.len();

    let Err(error) = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &pool, &interner)
    else {
        panic!("a compound identity absent from the type phase must fail closed");
    };

    let Some(missing) = error
        .missing_materializations()
        .iter()
        .find(|missing| missing.source() == list_bound)
        .copied()
    else {
        panic!("the missing compound identity must retain its ARC provenance");
    };
    assert_eq!(missing.function(), lambda_name);
    assert_eq!(missing.var_id(), v(1));
    assert_eq!(pool.len(), pool_len);
}
