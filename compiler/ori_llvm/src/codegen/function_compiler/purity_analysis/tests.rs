//! Tests for purity analysis of ARC functions.

use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, RcStrategy,
};
use ori_arc::{LitValue, Ownership};
use ori_ir::Name;
use ori_types::Idx;

use super::has_only_pure_arc_instructions;

/// Helper to create a minimal `ArcFunction` with the given body instructions.
fn make_func(body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        name: Name::from_raw(1),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        ..Default::default()
    }
}

#[test]
fn let_only_is_pure() {
    let func = make_func(vec![ArcInstr::Let {
        dst: ArcVarId::new(1),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    }]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn construct_does_not_block_purity() {
    let func = make_func(vec![
        ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        },
    ]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn project_does_not_block_purity() {
    let func = make_func(vec![
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Project {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcVarId::new(1),
            field: 0,
        },
    ]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn apply_blocks_purity() {
    let func = make_func(vec![ArcInstr::Apply {
        dst: ArcVarId::new(1),
        func: Name::from_raw(2),
        ty: Idx::INT,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![],
    }]);
    assert!(!has_only_pure_arc_instructions(&func));
}

#[test]
fn rc_inc_blocks_purity() {
    let func = make_func(vec![ArcInstr::RcInc {
        var: ArcVarId::new(0),
        count: 1,
        strategy: RcStrategy::HeapPointer,
    }]);
    assert!(!has_only_pure_arc_instructions(&func));
}

#[test]
fn empty_body_is_pure() {
    let func = make_func(vec![]);
    assert!(has_only_pure_arc_instructions(&func));
}
