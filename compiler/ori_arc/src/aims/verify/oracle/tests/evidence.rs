//! VF-3 theorem witnesses projected onto realized ARC IR.

use super::super::demand::derive_param_demand_dimensions;
use super::*;

fn param(access: AccessClass, consumption: Consumption) -> ParamContract {
    ParamContract {
        access,
        consumption,
        ..ParamContract::OPTIMISTIC
    }
}

fn return_block(id: u32, body: Vec<ArcInstr>) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Return { value: v(999) },
    }
}

fn borrowed_call(dst: u32, callee: ori_ir::Name, arg: crate::ir::ArcVarId) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: Idx::UNIT,
        func: callee,
        args: vec![arg],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

fn owned_call(dst: u32, callee: ori_ir::Name, arg: crate::ir::ArcVarId) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: Idx::UNIT,
        func: callee,
        args: vec![arg],
        arg_ownership: vec![ArgOwnership::Owned],
        mono_instance_id: None,
    }
}

#[test]
fn credit_before_transfer_is_borrowed_but_transfer_before_credit_is_owned() {
    let callee = ori_ir::Name::from_raw(100);
    let credit = ArcInstr::RcInc {
        var: v(0),
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: crate::ir::RcAtomicity::default_atomic(),
    };
    let funded = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![credit.clone(), owned_call(1, callee, v(0))],
    );
    let unfunded = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![owned_call(1, callee, v(0)), credit],
    );

    assert_eq!(
        derive_param_contracts(&funded)[0].access,
        AccessClass::Borrowed
    );
    assert_eq!(
        derive_param_contracts(&unfunded)[0].access,
        AccessClass::Owned
    );
}

#[test]
fn caller_funded_borrowed_cow_terminal_consume_is_borrowed() {
    let interner = StringInterner::new();
    let push = interner.intern("push");
    let func = func_with_body(
        vec![borrowed_param(0, Idx::UNIT)],
        vec![owned_call(1, push, v(0))],
    );

    let realized = derive_param_contracts_with_context(&func, &FxHashMap::default(), &interner);

    assert_eq!(
        realized[0].access,
        AccessClass::Borrowed,
        "the caller-supplied COW credit must fund the terminal owned transfer"
    );
}

#[test]
fn closure_capture_credit_funds_target_release() {
    let mut func = func_with_body(
        vec![borrowed_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );
    func.num_captures = 1;

    assert_eq!(
        derive_param_contracts(&func)[0].access,
        AccessClass::Borrowed,
        "the closure adapter supplies the captured value's target credit"
    );
}

#[test]
fn ordinary_borrowed_param_release_remains_unfunded() {
    let func = func_with_body(
        vec![borrowed_param(0, Idx::UNIT)],
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        }],
    );

    assert_eq!(
        derive_param_contracts(&func)[0].access,
        AccessClass::Owned,
        "a non-capture borrowed parameter has no implicit entry credit"
    );
}

#[test]
fn ordinary_borrowed_param_owned_transfer_remains_rejected() {
    let func = func_with_body(
        vec![borrowed_param(0, Idx::UNIT)],
        vec![owned_call(1, ori_ir::Name::from_raw(100), v(0))],
    );
    let inferred = make_contract(vec![param(AccessClass::Borrowed, Consumption::Linear)]);

    let mismatches = verify_isolated(&func, &inferred, 0);

    assert!(mismatches.iter().any(|mismatch| matches!(
        mismatch,
        CoherenceMismatch::ParamAccess {
            inferred: AccessClass::Borrowed,
            realized: AccessClass::Owned,
            ..
        }
    )));
}

#[test]
fn one_unfunded_branch_cannot_be_masked_by_a_funded_branch() {
    let cond = ArcInstr::Let {
        dst: v(10),
        ty: Idx::BOOL,
        value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
    };
    let credit = ArcInstr::RcInc {
        var: v(0),
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: crate::ir::RcAtomicity::default_atomic(),
    };
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![cond],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(
                1,
                vec![credit, owned_call(11, ori_ir::Name::from_raw(100), v(0))],
            ),
            return_block(2, vec![owned_call(12, ori_ir::Name::from_raw(101), v(0))]),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(derive_param_contracts(&func)[0].access, AccessClass::Owned);
}

#[test]
fn alternative_single_demands_remain_linear() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(10),
                    ty: Idx::BOOL,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(
                1,
                vec![borrowed_call(11, ori_ir::Name::from_raw(100), v(0))],
            ),
            return_block(
                2,
                vec![borrowed_call(12, ori_ir::Name::from_raw(101), v(0))],
            ),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Linear)
    );
}

#[test]
fn sequential_single_demands_are_unrestricted() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            borrowed_call(1, ori_ir::Name::from_raw(100), v(0)),
            borrowed_call(2, ori_ir::Name::from_raw(101), v(0)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Many, Consumption::Unrestricted)
    );
}

#[test]
fn dead_projection_alone_canonicalizes_to_dead() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Project {
            dst: v(10),
            ty: Idx::UNIT,
            value: v(0),
            field: 0,
        }],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Absent, Consumption::Dead),
        "the unconditional Affine transfer canonicalizes away only at final Absent observation"
    );
}

#[test]
fn dead_projection_fixed_affine_composes_with_same_span_direct_demand() {
    let span = ori_ir::Span::new(10, 20);
    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(0)),
        ],
    );
    func.spans[0] = vec![Some(span), Some(span)];

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Unrestricted),
        "source-span identity cannot erase TF-14's independent Affine contribution"
    );
}

#[test]
fn distinct_same_span_demands_remain_sequential() {
    let span = ori_ir::Span::new(10, 20);
    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            borrowed_call(10, ori_ir::Name::from_raw(100), v(0)),
            borrowed_call(11, ori_ir::Name::from_raw(101), v(0)),
        ],
    );
    func.spans[0] = vec![Some(span), Some(span)];

    assert_eq!(
        derive_param_contracts(&func)[0].consumption,
        Consumption::Unrestricted,
        "optional source spans do not define semantic demand identity"
    );
}

#[test]
fn dead_projection_fixed_affine_composes_without_source_span() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(0)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Unrestricted),
        "TF-14's fixed Affine contribution is structural and span-independent"
    );
}

#[test]
fn dead_projection_fixed_affine_is_source_order_independent() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            borrowed_call(11, ori_ir::Name::from_raw(100), v(0)),
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Unrestricted),
        "TF-14 must compose before observation regardless of reverse-walk encounter order"
    );
}

#[test]
fn oracle_rejects_linear_for_dead_projection_plus_live_source_demand() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(0)),
        ],
    );
    let inferred = make_contract(vec![param(AccessClass::Borrowed, Consumption::Linear)]);

    let mismatches = verify_isolated(&func, &inferred, 0);
    assert!(mismatches.iter().any(|mismatch| matches!(
        mismatch,
        CoherenceMismatch::ParamConsumption {
            inferred: Consumption::Linear,
            realized: Consumption::Unrestricted,
            ..
        }
    )));
}

#[test]
fn one_live_projection_is_affine() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(10)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Affine)
    );
}

#[test]
fn one_projection_with_many_downstream_uses_remains_affine() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(10)),
            borrowed_call(12, ori_ir::Name::from_raw(101), v(10)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Many, Consumption::Affine),
        "one Project carries Many cardinality without reconstructing Unrestricted consumption"
    );
}

#[test]
fn scalar_project_copy_out_caps_many_downstream_uses_at_once() {
    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::INT,
                value: v(0),
                field: 0,
            },
            borrowed_call(11, ori_ir::Name::from_raw(100), v(10)),
            borrowed_call(12, ori_ir::Name::from_raw(101), v(10)),
        ],
    );
    func.var_reprs = vec![crate::ir::ValueRepr::RcPointer; func.var_types.len()];
    func.var_reprs[v(10).index()] = crate::ir::ValueRepr::Scalar;

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Affine),
        "one scalar copy-out reads its managed source once regardless of later scalar reuse"
    );
}

#[test]
fn dead_scalar_project_uses_the_existing_dead_projection_shape() {
    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![ArcInstr::Project {
            dst: v(10),
            ty: Idx::INT,
            value: v(0),
            field: 0,
        }],
    );
    func.var_reprs = vec![crate::ir::ValueRepr::RcPointer; func.var_types.len()];
    func.var_reprs[v(10).index()] = crate::ir::ValueRepr::Scalar;

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Absent, Consumption::Dead)
    );
}

#[test]
fn scalar_project_liveness_maps_successor_param_to_jump_arg() {
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: v(10),
                    ty: Idx::INT,
                    value: v(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(10)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(11), Idx::INT)],
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(11) },
            },
        ],
        vec![Idx::UNIT; 12],
    );
    func.var_reprs = vec![crate::ir::ValueRepr::RcPointer; func.var_types.len()];
    func.var_reprs[v(10).index()] = crate::ir::ValueRepr::Scalar;
    func.var_reprs[v(11).index()] = crate::ir::ValueRepr::Scalar;

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Affine)
    );
}

#[test]
fn unused_scalar_successor_param_does_not_keep_jump_arg_live() {
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: v(10),
                    ty: Idx::INT,
                    value: v(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(10)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(11), Idx::INT)],
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::UNIT; 12],
    );
    func.var_reprs = vec![crate::ir::ValueRepr::RcPointer; func.var_types.len()];
    func.var_reprs[v(10).index()] = crate::ir::ValueRepr::Scalar;
    func.var_reprs[v(11).index()] = crate::ir::ValueRepr::Scalar;

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Absent, Consumption::Dead)
    );
}

#[test]
fn nested_scalar_project_liveness_crosses_jump_parameter() {
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: v(10),
                    ty: Idx::INT,
                    value: v(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(10)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(11), Idx::INT)],
                body: vec![ArcInstr::Project {
                    dst: v(12),
                    ty: Idx::INT,
                    value: v(11),
                    field: 0,
                }],
                terminator: ArcTerminator::Return { value: v(12) },
            },
        ],
        vec![Idx::UNIT; 13],
    );
    func.var_reprs = vec![crate::ir::ValueRepr::RcPointer; func.var_types.len()];
    for scalar in [v(10), v(11), v(12)] {
        func.var_reprs[scalar.index()] = crate::ir::ValueRepr::Scalar;
    }

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Affine)
    );
}

#[test]
fn two_live_projections_are_unrestricted() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Project {
                dst: v(10),
                ty: Idx::UNIT,
                value: v(0),
                field: 0,
            },
            ArcInstr::Project {
                dst: v(11),
                ty: Idx::UNIT,
                value: v(0),
                field: 1,
            },
            borrowed_call(12, ori_ir::Name::from_raw(100), v(10)),
            borrowed_call(13, ori_ir::Name::from_raw(101), v(11)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Many, Consumption::Unrestricted),
        "distinct Project sites contribute two sequential Affine endpoints"
    );
}

#[test]
fn projections_on_alternative_paths_remain_affine() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(10),
                    ty: Idx::BOOL,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(
                1,
                vec![
                    ArcInstr::Project {
                        dst: v(11),
                        ty: Idx::UNIT,
                        value: v(0),
                        field: 0,
                    },
                    borrowed_call(12, ori_ir::Name::from_raw(100), v(11)),
                ],
            ),
            return_block(
                2,
                vec![
                    ArcInstr::Project {
                        dst: v(13),
                        ty: Idx::UNIT,
                        value: v(0),
                        field: 1,
                    },
                    borrowed_call(14, ori_ir::Name::from_raw(101), v(13)),
                ],
            ),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Affine),
        "IC-3 joins mutually exclusive Project endpoints instead of adding them"
    );
}

#[test]
fn dead_projection_on_one_alternative_does_not_promote_a_linear_path() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(10),
                    ty: Idx::BOOL,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(
                1,
                vec![ArcInstr::Project {
                    dst: v(11),
                    ty: Idx::UNIT,
                    value: v(0),
                    field: 0,
                }],
            ),
            return_block(
                2,
                vec![borrowed_call(12, ori_ir::Name::from_raw(100), v(0))],
            ),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Once, Consumption::Linear),
        "each path canonicalizes before IC-3 joins mutually exclusive demand"
    );
}

#[test]
fn dead_projection_repeated_by_a_loop_remains_dead() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: v(10),
                        ty: Idx::BOOL,
                        value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                    },
                    ArcInstr::Project {
                        dst: v(11),
                        ty: Idx::UNIT,
                        value: v(0),
                        field: 0,
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(2, Vec::new()),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Absent, Consumption::Dead),
        "loop repetition cannot manufacture cardinality for a dead Project"
    );
}

#[test]
fn let_alias_preserves_many_affine_projection_demand() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(10),
                ty: Idx::UNIT,
                value: crate::ir::ArcValue::Var(v(0)),
            },
            ArcInstr::Project {
                dst: v(11),
                ty: Idx::UNIT,
                value: v(10),
                field: 0,
            },
            borrowed_call(12, ori_ir::Name::from_raw(100), v(11)),
            borrowed_call(13, ori_ir::Name::from_raw(101), v(11)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Many, Consumption::Affine),
        "Let must transfer both demand dimensions without replacing Affine from cardinality"
    );
}

#[test]
fn return_transfer_preserves_storage_alias_without_crossing_demand_definition() {
    let interner = StringInterner::new();
    let subject = interner.intern("subject");
    let transformer = interner.intern("transformer");
    let sharing_observer = interner.intern("sharing_observer");

    let mut transformer_param = param(AccessClass::Owned, Consumption::Linear);
    transformer_param.transfers_through_return = true;
    let mut sharing_param = param(AccessClass::Borrowed, Consumption::Linear);
    sharing_param.may_share = true;
    let contracts = FxHashMap::from_iter([
        (transformer, make_contract(vec![transformer_param])),
        (sharing_observer, make_contract(vec![sharing_param])),
    ]);

    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            owned_call(10, transformer, v(0)),
            borrowed_call(11, sharing_observer, v(10)),
        ],
    );
    func.name = subject;

    let realized = derive_param_contracts_with_context(&func, &contracts, &interner);
    assert_eq!(realized[0].consumption, Consumption::Linear);
    assert!(
        realized[0].may_share,
        "the independent storage relation still crosses return-transfer summaries"
    );
}

#[test]
fn loop_carried_produced_value_does_not_re_demand_input() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: crate::ir::ArcValue::Var(v(0)),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(2)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(10), Idx::UNIT)],
                body: vec![ArcInstr::Let {
                    dst: v(11),
                    ty: Idx::BOOL,
                    value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(11),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: v(12),
                        ty: Idx::UNIT,
                        value: crate::ir::ArcValue::Var(v(10)),
                    },
                    owned_call(13, ori_ir::Name::from_raw(100), v(12)),
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(13)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(10) },
            },
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_contracts(&func)[0].consumption,
        Consumption::Linear,
        "the input reaches either the first transform or the zero-iteration return"
    );
}

#[test]
fn free_parameter_demand_repeated_by_a_loop_is_unrestricted() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: v(10),
                        ty: Idx::BOOL,
                        value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                    },
                    borrowed_call(11, ori_ir::Name::from_raw(100), v(0)),
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(10),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            return_block(2, Vec::new()),
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_contracts(&func)[0].consumption,
        Consumption::Unrestricted
    );
}

#[test]
fn multi_parameter_select_aliases_preserve_both_roots() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(10),
                ty: Idx::BOOL,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
            },
            ArcInstr::Select {
                dst: v(11),
                ty: Idx::UNIT,
                cond: v(10),
                true_val: v(0),
                false_val: v(1),
            },
            ArcInstr::RcDec {
                var: v(11),
                strategy: RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
        ],
    );

    let realized = derive_param_contracts(&func);
    assert_eq!(realized[0].access, AccessClass::Owned);
    assert_eq!(realized[1].access, AccessClass::Owned);
}

#[test]
fn select_transfers_linear_demand_to_both_roots() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(10),
                ty: Idx::BOOL,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
            },
            ArcInstr::Select {
                dst: v(11),
                ty: Idx::UNIT,
                cond: v(10),
                true_val: v(0),
                false_val: v(1),
            },
            borrowed_call(12, ori_ir::Name::from_raw(100), v(11)),
        ],
    );

    let realized = derive_param_demand_dimensions(&func);
    assert_eq!(realized[0], (Cardinality::Once, Consumption::Linear));
    assert_eq!(realized[1], (Cardinality::Once, Consumption::Linear));
}

#[test]
fn select_with_the_same_root_adds_both_arms_sequentially() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(10),
                ty: Idx::BOOL,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
            },
            ArcInstr::Select {
                dst: v(11),
                ty: Idx::UNIT,
                cond: v(10),
                true_val: v(0),
                false_val: v(0),
            },
            borrowed_call(12, ori_ir::Name::from_raw(100), v(11)),
        ],
    );

    assert_eq!(
        derive_param_demand_dimensions(&func)[0],
        (Cardinality::Many, Consumption::Unrestricted),
        "the two Select operand slots are distinct sequential alias transfers"
    );
}

#[test]
fn select_preserves_many_affine_projection_demand_for_each_root() {
    let func = func_with_body(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        vec![
            ArcInstr::Let {
                dst: v(10),
                ty: Idx::BOOL,
                value: crate::ir::ArcValue::Literal(crate::ir::LitValue::Bool(true)),
            },
            ArcInstr::Select {
                dst: v(11),
                ty: Idx::UNIT,
                cond: v(10),
                true_val: v(0),
                false_val: v(1),
            },
            ArcInstr::Project {
                dst: v(12),
                ty: Idx::UNIT,
                value: v(11),
                field: 0,
            },
            borrowed_call(13, ori_ir::Name::from_raw(100), v(12)),
            borrowed_call(14, ori_ir::Name::from_raw(101), v(12)),
        ],
    );

    let realized = derive_param_demand_dimensions(&func);
    assert_eq!(realized[0], (Cardinality::Many, Consumption::Affine));
    assert_eq!(realized[1], (Cardinality::Many, Consumption::Affine));
}

#[test]
fn invoke_transfer_precedes_normal_and_unwind_successors() {
    let callee = ori_ir::Name::from_raw(100);
    let credit = || ArcInstr::RcInc {
        var: v(0),
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: crate::ir::RcAtomicity::default_atomic(),
    };
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Invoke {
                    dst: v(10),
                    ty: Idx::UNIT,
                    func: callee,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            return_block(1, vec![credit()]),
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![credit()],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::UNIT; 1000],
    );

    assert_eq!(
        derive_param_contracts(&func)[0].access,
        AccessClass::Owned,
        "successor credits cannot retroactively fund an Invoke argument"
    );
}

#[test]
fn inferred_iter_claim_and_self_summary_cannot_create_evidence() {
    let interner = StringInterner::new();
    let subject = interner.intern("subject");
    let mut func = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![borrowed_call(1, subject, v(0))],
    );
    func.name = subject;
    let mut inferred_param = param(AccessClass::Borrowed, Consumption::Linear);
    inferred_param.iter_consumes = true;
    let inferred = make_contract(vec![inferred_param]);
    let contracts = FxHashMap::from_iter([(subject, inferred)]);

    let mismatches = verify_coherence(&func, &contracts[&subject], &contracts, &interner, 0);
    assert!(mismatches.iter().any(|mismatch| matches!(
        mismatch,
        CoherenceMismatch::ParamIterTransfer {
            inferred: true,
            realized: false,
            ..
        }
    )));
}

#[test]
fn direct_and_transitive_iter_calls_create_independent_transfer_evidence() {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;

    let interner = StringInterner::new();
    let subject = interner.intern("subject");
    let iter = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_drop = interner.intern(ProtocolBuiltin::IterDrop.name());
    let mut direct = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![owned_call(1, iter, v(0)), owned_call(2, iter_drop, v(1))],
    );
    direct.name = subject;
    let no_contracts = FxHashMap::default();
    let direct_realized = derive_param_contracts_with_context(&direct, &no_contracts, &interner);
    assert_eq!(direct_realized[0].access, AccessClass::Borrowed);
    assert!(direct_realized[0].iter_transfers);

    let forwarding = interner.intern("forwarding");
    let callee = interner.intern("iterating_callee");
    let mut callee_param = param(AccessClass::Borrowed, Consumption::Linear);
    callee_param.iter_consumes = true;
    let contracts = FxHashMap::from_iter([(callee, make_contract(vec![callee_param]))]);
    let mut transitive = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![borrowed_call(1, callee, v(0))],
    );
    transitive.name = forwarding;
    let transitive_realized =
        derive_param_contracts_with_context(&transitive, &contracts, &interner);
    assert_eq!(transitive_realized[0].access, AccessClass::Borrowed);
    assert!(transitive_realized[0].iter_transfers);
}

#[test]
fn only_explicit_borrowed_ownership_propagates_callee_may_share() {
    let interner = StringInterner::new();
    let subject = interner.intern("subject");
    let callee = interner.intern("sharing_callee");
    let mut callee_param = param(AccessClass::Borrowed, Consumption::Linear);
    callee_param.may_share = true;
    let contracts = FxHashMap::from_iter([(callee, make_contract(vec![callee_param]))]);

    let mut borrowed = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![borrowed_call(1, callee, v(0))],
    );
    borrowed.name = subject;
    let borrowed_realized = derive_param_contracts_with_context(&borrowed, &contracts, &interner);
    assert_eq!(borrowed_realized[0].access, AccessClass::Borrowed);
    assert!(borrowed_realized[0].may_share);

    let mut inferred_param = param(AccessClass::Borrowed, Consumption::Linear);
    inferred_param.may_share = true;
    let inferred = make_contract(vec![inferred_param]);
    let mismatches = verify_coherence(&borrowed, &inferred, &contracts, &interner, 0);
    assert!(
        !mismatches
            .iter()
            .any(|m| matches!(m, CoherenceMismatch::ParamMayShare { .. })),
        "the published caller contract must match the hidden borrowed retain: {mismatches:?}"
    );

    let mut owned = func_with_body(
        vec![owned_param(0, Idx::UNIT)],
        vec![owned_call(1, callee, v(0))],
    );
    owned.name = subject;
    let owned_realized = derive_param_contracts_with_context(&owned, &contracts, &interner);
    assert!(!owned_realized[0].may_share);
}
