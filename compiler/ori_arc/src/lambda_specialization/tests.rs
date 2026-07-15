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

    let result = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &mut pool, &interner);

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

    let result = specialize_polymorphic_lambdas(&mut parent, &mut lambdas, &mut pool, &interner);

    assert!(result.is_err());
    assert_eq!(lambdas.len(), 1);
    assert!(matches!(
        parent.blocks[0].body[0],
        ArcInstr::PartialApply { func, .. } if func == lambda_name
    ));
}
