//! Tests for ARC IR verification.

use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue, RcStrategy,
    ValueRepr,
};
use crate::test_helpers::{b, borrowed_param, make_func, owned_param, v};
use crate::verify::{check_function, VerifyError};

fn simple_return_block(id: u32, ret_var: ArcVarId) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: ret_var },
    }
}

// Variable scope checks

#[test]
fn valid_straight_line() {
    // v0 = param, v1 = let unit, return v1
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Unit),
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::NONE; 2],
    );
    let errors = check_function(&func);
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}

#[test]
fn use_before_def_detected() {
    // Return v(5) which is never defined.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![simple_return_block(0, v(5))],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        VerifyError::UseBeforeDef { var, block, .. }
        if *var == v(5) && *block == b(0)
    ));
}

#[test]
fn block_param_counts_as_def() {
    // Block 1 has param v(2), uses it in return.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(2), Idx::NONE)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::NONE; 3],
    );
    let errors = check_function(&func);
    assert!(
        errors.is_empty(),
        "block param should be treated as def: {errors:?}"
    );
}

#[test]
fn invoke_dst_counts_as_def() {
    // Invoke defines v(1), used in the normal successor.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::NONE,
                    func: ori_ir::Name::from_raw(1),
                    args: vec![v(0)],
                    arg_ownership: vec![],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::NONE; 2],
    );
    let errors = check_function(&func);
    assert!(
        errors.is_empty(),
        "invoke dst should be treated as def: {errors:?}"
    );
}

// Block connectivity checks

#[test]
fn valid_block_refs() {
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(0),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            simple_return_block(1, v(0)),
            simple_return_block(2, v(0)),
        ],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert!(errors.is_empty(), "valid branch targets: {errors:?}");
}

#[test]
fn dangling_block_ref_detected() {
    // Jump to block 99 which doesn't exist.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(99),
                args: vec![],
            },
        }],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        VerifyError::DanglingBlockRef { from_block, target }
        if *from_block == b(0) && *target == ArcBlockId::new(99)
    ));
}

#[test]
fn switch_dangling_default() {
    // Switch with valid case targets but dangling default.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Switch {
                    scrutinee: v(0),
                    cases: vec![(0, b(1))],
                    default: ArcBlockId::new(99),
                },
            },
            simple_return_block(1, v(0)),
        ],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert!(errors.iter().any(|e| matches!(
        e,
        VerifyError::DanglingBlockRef { target, .. }
        if *target == ArcBlockId::new(99)
    )));
}

// RC on scalar checks

#[test]
fn rc_on_scalar_detected() {
    let mut func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    // Mark v(0) as Scalar.
    func.var_reprs = vec![ValueRepr::Scalar];

    let errors = check_function(&func);
    assert_eq!(
        errors.len(),
        2,
        "both RcInc and RcDec on scalar: {errors:?}"
    );
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::RcOnScalar { is_inc: true, .. })));
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::RcOnScalar { is_inc: false, .. })));
}

#[test]
fn rc_on_non_scalar_ok() {
    let mut func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    func.var_reprs = vec![ValueRepr::RcPointer];

    let errors = check_function(&func);
    assert!(
        errors.is_empty(),
        "RcPointer should not trigger: {errors:?}"
    );
}

#[test]
fn skips_rc_check_when_var_reprs_empty() {
    // Before compute_var_reprs is called, var_reprs is empty.
    // The check should be a no-op.
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcInc {
                var: v(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    // var_reprs is empty (default from make_func).
    assert!(func.var_reprs.is_empty());
    let errors = check_function(&func);
    // Should not report RC-on-scalar since var_reprs hasn't been computed.
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, VerifyError::RcOnScalar { .. })),
        "should skip RC check when var_reprs empty: {errors:?}"
    );
}

// Dec on borrowed param checks

#[test]
fn dec_on_borrowed_detected() {
    let func = make_func(
        vec![borrowed_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert!(errors.iter().any(|e| matches!(
        e,
        VerifyError::DecOnBorrowed { var, .. }
        if *var == v(0)
    )));
}

#[test]
fn inc_on_borrowed_ok() {
    // RcInc on borrowed is fine (temporary ownership for passing to owned param).
    let func = make_func(
        vec![borrowed_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcInc {
                var: v(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, VerifyError::DecOnBorrowed { .. })),
        "RcInc on borrowed should not trigger dec-on-borrowed: {errors:?}"
    );
}

#[test]
fn dec_on_owned_param_ok() {
    let func = make_func(
        vec![owned_param(0, Idx::NONE)],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: v(0),
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, VerifyError::DecOnBorrowed { .. })),
        "RcDec on owned param should be fine: {errors:?}"
    );
}

// Combined checks

#[test]
fn empty_function_ok() {
    let func = make_func(
        vec![],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::NONE; 1],
    );
    let errors = check_function(&func);
    // v(0) is not defined — should report use-before-def.
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UseBeforeDef { .. })));
}

#[test]
fn multiple_errors_collected() {
    // Use undefined var AND jump to non-existent block.
    let func = make_func(
        vec![],
        Idx::NONE,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(42),
                args: vec![v(99)],
            },
        }],
        vec![],
    );
    let errors = check_function(&func);
    assert!(
        errors.len() >= 2,
        "should collect multiple errors: {errors:?}"
    );
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UseBeforeDef { .. })));
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::DanglingBlockRef { .. })));
}
