//! Walking-skeleton pins for the RL-1 terminal-concat surplus-inc suppression:
//! admission (fresh heap operand consumed exactly once as a `Binary(Add)`
//! concat operand) and the over-fire boundary declines (multi-use, non-concat
//! sole use, scalar repr, terminator use).

use ori_ir::{BinaryOp, Name};
use ori_types::Idx;

use super::compute_cow_terminal_concat_inc_dsts;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue, PrimOp, ValueRepr,
};

fn vv(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn func(reprs: Vec<ValueRepr>, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..u32::try_from(reprs.len()).unwrap_or(u32::MAX))
            .map(|i| Idx::from_raw(i + 1))
            .collect(),
        var_reprs: reprs,
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn str_literal(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: vv(dst),
        ty: Idx::from_raw(90),
        value: ArcValue::Literal(LitValue::String(Name::from_raw(7))),
    }
}

fn concat(dst: u32, lhs: u32, rhs: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: vv(dst),
        ty: Idx::from_raw(91),
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![vv(lhs), vv(rhs)],
        },
    }
}

fn borrowed_read(dst: u32, arg: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: vv(dst),
        ty: Idx::INT,
        func: Name::from_raw(50),
        args: vec![vv(arg)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

#[test]
fn heap_str_literal_single_concat_use_admits() {
    // v0 = "..." (RcPointer); v2 = concat(v0, v1); v0 never used again.
    let f = func(
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
        ],
        vec![block(
            0,
            vec![str_literal(0), str_literal(1), concat(2, 0, 1)],
            ArcTerminator::Unreachable,
        )],
    );
    let dsts = compute_cow_terminal_concat_inc_dsts(&f);
    assert!(
        dsts.contains(&vv(0)),
        "single-concat-use heap literal admits"
    );
    assert!(dsts.contains(&vv(1)), "the rhs operand admits identically");
}

#[test]
fn fresh_collection_construct_single_concat_use_admits() {
    let f = func(
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
        ],
        vec![block(
            0,
            vec![
                ArcInstr::Construct {
                    dst: vv(0),
                    ty: Idx::from_raw(92),
                    ctor: CtorKind::ListLiteral,
                    args: vec![],
                },
                str_literal(1),
                concat(2, 0, 1),
            ],
            ArcTerminator::Unreachable,
        )],
    );
    assert!(compute_cow_terminal_concat_inc_dsts(&f).contains(&vv(0)));
}

#[test]
fn re_read_after_concat_declines() {
    // The keep-alive inc is LOAD-BEARING when the operand is read again:
    // multi-use excludes it.
    let f = func(
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
        ],
        vec![block(
            0,
            vec![
                str_literal(0),
                str_literal(1),
                concat(2, 0, 1),
                borrowed_read(3, 0),
            ],
            ArcTerminator::Unreachable,
        )],
    );
    assert!(!compute_cow_terminal_concat_inc_dsts(&f).contains(&vv(0)));
}

#[test]
fn non_concat_sole_use_declines() {
    let f = func(
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
        vec![block(
            0,
            vec![str_literal(0), borrowed_read(1, 0)],
            ArcTerminator::Unreachable,
        )],
    );
    assert!(compute_cow_terminal_concat_inc_dsts(&f).is_empty());
}

#[test]
fn scalar_repr_declines() {
    let f = func(
        vec![
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
        ],
        vec![block(
            0,
            vec![str_literal(0), str_literal(1), concat(2, 0, 1)],
            ArcTerminator::Unreachable,
        )],
    );
    assert!(!compute_cow_terminal_concat_inc_dsts(&f).contains(&vv(0)));
}

#[test]
fn terminator_use_declines() {
    // The sole use is a Jump arg (an ownership hand-off), never a concat operand.
    let f = func(
        vec![ValueRepr::RcPointer, ValueRepr::RcPointer],
        vec![
            block(
                0,
                vec![str_literal(0)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![vv(0)],
                },
            ),
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(vv(1), Idx::from_raw(90))],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
    );
    assert!(compute_cow_terminal_concat_inc_dsts(&f).is_empty());
}
