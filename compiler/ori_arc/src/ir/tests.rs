use std::mem;

use ori_ir::{BinaryOp, Name, UnaryOp};
use ori_types::Idx;

use crate::Ownership;

use super::*;

// ID newtypes

#[test]
fn arc_var_id_basics() {
    let v = ArcVarId::new(42);
    assert_eq!(v.raw(), 42);
    assert_eq!(v.index(), 42);
}

#[test]
fn arc_block_id_basics() {
    let b = ArcBlockId::new(7);
    assert_eq!(b.raw(), 7);
    assert_eq!(b.index(), 7);
}

#[test]
fn arc_var_id_equality() {
    assert_eq!(ArcVarId::new(0), ArcVarId::new(0));
    assert_ne!(ArcVarId::new(0), ArcVarId::new(1));
}

#[test]
fn arc_block_id_equality() {
    assert_eq!(ArcBlockId::new(0), ArcBlockId::new(0));
    assert_ne!(ArcBlockId::new(0), ArcBlockId::new(1));
}

#[test]
fn arc_var_id_ordering() {
    assert!(ArcVarId::new(0) < ArcVarId::new(1));
    assert!(ArcVarId::new(5) > ArcVarId::new(3));
}

#[test]
fn id_sizes() {
    assert_eq!(mem::size_of::<ArcVarId>(), 4);
    assert_eq!(mem::size_of::<ArcBlockId>(), 4);
}

// LitValue

#[test]
fn lit_value_int() {
    let v = LitValue::Int(42);
    assert_eq!(v, LitValue::Int(42));
    assert_ne!(v, LitValue::Int(43));
}

#[test]
fn lit_value_bool() {
    assert_ne!(LitValue::Bool(true), LitValue::Bool(false));
}

#[test]
fn lit_value_unit() {
    assert_eq!(LitValue::Unit, LitValue::Unit);
}

#[test]
fn lit_value_string() {
    let s = LitValue::String(Name::from_raw(100));
    assert_eq!(s, LitValue::String(Name::from_raw(100)));
}

#[test]
fn lit_value_duration() {
    let d = LitValue::Duration {
        value: 500,
        unit: ori_ir::DurationUnit::Milliseconds,
    };
    assert_eq!(
        d,
        LitValue::Duration {
            value: 500,
            unit: ori_ir::DurationUnit::Milliseconds,
        }
    );
}

#[test]
fn lit_value_size() {
    let s = LitValue::Size {
        value: 1024,
        unit: ori_ir::SizeUnit::Kilobytes,
    };
    assert_eq!(
        s,
        LitValue::Size {
            value: 1024,
            unit: ori_ir::SizeUnit::Kilobytes,
        }
    );
}

// PrimOp

#[test]
fn prim_op_binary() {
    let op = PrimOp::Binary(BinaryOp::Add);
    assert_eq!(op, PrimOp::Binary(BinaryOp::Add));
    assert_ne!(op, PrimOp::Binary(BinaryOp::Sub));
}

#[test]
fn prim_op_unary() {
    let op = PrimOp::Unary(UnaryOp::Neg);
    assert_eq!(op, PrimOp::Unary(UnaryOp::Neg));
    assert_ne!(op, PrimOp::Unary(UnaryOp::Not));
}

#[test]
fn prim_op_binary_vs_unary() {
    assert_ne!(PrimOp::Binary(BinaryOp::Add), PrimOp::Unary(UnaryOp::Neg),);
}

// ArcValue

#[test]
fn arc_value_var() {
    let v = ArcValue::Var(ArcVarId::new(0));
    assert_eq!(v, ArcValue::Var(ArcVarId::new(0)));
}

#[test]
fn arc_value_literal() {
    let v = ArcValue::Literal(LitValue::Int(99));
    assert_eq!(v, ArcValue::Literal(LitValue::Int(99)));
}

#[test]
fn arc_value_prim_op() {
    let v = ArcValue::PrimOp {
        op: PrimOp::Binary(BinaryOp::Add),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert!(matches!(v, ArcValue::PrimOp { .. }));
}

// CtorKind

#[test]
fn ctor_kind_struct() {
    let c = CtorKind::Struct(Name::from_raw(1));
    assert_eq!(c, CtorKind::Struct(Name::from_raw(1)));
}

#[test]
fn ctor_kind_enum_variant() {
    let c = CtorKind::EnumVariant {
        enum_name: Name::from_raw(2),
        variant: 0,
    };
    assert!(matches!(c, CtorKind::EnumVariant { variant: 0, .. }));
}

#[test]
fn ctor_kind_collection_literals() {
    // All three collection literal kinds are distinct.
    assert_ne!(CtorKind::ListLiteral, CtorKind::MapLiteral);
    assert_ne!(CtorKind::MapLiteral, CtorKind::SetLiteral);
    assert_ne!(CtorKind::ListLiteral, CtorKind::SetLiteral);
}

// ArcParam

#[test]
fn arc_param_borrowed() {
    let p = ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership: Ownership::Borrowed,
    };
    assert_eq!(p.ownership, Ownership::Borrowed);
}

#[test]
fn arc_param_owned() {
    let p = ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership: Ownership::Owned,
    };
    assert_eq!(p.ownership, Ownership::Owned);
}

// ArcInstr

#[test]
fn instr_let() {
    let instr = ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    };
    assert!(matches!(instr, ArcInstr::Let { .. }));
}

#[test]
fn instr_apply() {
    let instr = ArcInstr::Apply {
        dst: ArcVarId::new(1),
        ty: Idx::INT,
        func: Name::from_raw(10),
        args: vec![ArcVarId::new(0)],
    };
    assert!(matches!(instr, ArcInstr::Apply { .. }));
}

#[test]
fn instr_construct() {
    let instr = ArcInstr::Construct {
        dst: ArcVarId::new(2),
        ty: Idx::UNIT,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    if let ArcInstr::Construct { ctor, args, .. } = &instr {
        assert_eq!(*ctor, CtorKind::Tuple);
        assert_eq!(args.len(), 2);
    } else {
        panic!("expected Construct");
    }
}

#[test]
fn instr_project() {
    let instr = ArcInstr::Project {
        dst: ArcVarId::new(3),
        ty: Idx::INT,
        value: ArcVarId::new(2),
        field: 0,
    };
    if let ArcInstr::Project { field, .. } = &instr {
        assert_eq!(*field, 0);
    } else {
        panic!("expected Project");
    }
}

#[test]
fn instr_rc_ops() {
    let inc = ArcInstr::RcInc {
        var: ArcVarId::new(0),
        count: 2,
    };
    let dec = ArcInstr::RcDec {
        var: ArcVarId::new(0),
    };
    assert!(matches!(inc, ArcInstr::RcInc { count: 2, .. }));
    assert!(matches!(dec, ArcInstr::RcDec { .. }));
}

#[test]
fn instr_apply_indirect() {
    let instr = ArcInstr::ApplyIndirect {
        dst: ArcVarId::new(5),
        ty: Idx::INT,
        closure: ArcVarId::new(4),
        args: vec![ArcVarId::new(0)],
    };
    if let ArcInstr::ApplyIndirect { closure, .. } = &instr {
        assert_eq!(*closure, ArcVarId::new(4));
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn instr_partial_apply() {
    let instr = ArcInstr::PartialApply {
        dst: ArcVarId::new(6),
        ty: Idx::UNIT,
        func: Name::from_raw(20),
        args: vec![ArcVarId::new(0)],
    };
    assert!(matches!(instr, ArcInstr::PartialApply { .. }));
}

// ArcTerminator

#[test]
fn terminator_return() {
    let t = ArcTerminator::Return {
        value: ArcVarId::new(0),
    };
    assert!(matches!(t, ArcTerminator::Return { .. }));
}

#[test]
fn terminator_jump() {
    let t = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![ArcVarId::new(0)],
    };
    if let ArcTerminator::Jump { target, args } = &t {
        assert_eq!(*target, ArcBlockId::new(1));
        assert_eq!(args.len(), 1);
    } else {
        panic!("expected Jump");
    }
}

#[test]
fn terminator_branch() {
    let t = ArcTerminator::Branch {
        cond: ArcVarId::new(0),
        then_block: ArcBlockId::new(1),
        else_block: ArcBlockId::new(2),
    };
    if let ArcTerminator::Branch {
        then_block,
        else_block,
        ..
    } = &t
    {
        assert_ne!(then_block, else_block);
    } else {
        panic!("expected Branch");
    }
}

#[test]
fn terminator_switch() {
    let t = ArcTerminator::Switch {
        scrutinee: ArcVarId::new(0),
        cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
        default: ArcBlockId::new(3),
    };
    if let ArcTerminator::Switch { cases, default, .. } = &t {
        assert_eq!(cases.len(), 2);
        assert_eq!(*default, ArcBlockId::new(3));
    } else {
        panic!("expected Switch");
    }
}

#[test]
fn terminator_unreachable() {
    let t = ArcTerminator::Unreachable;
    assert!(matches!(t, ArcTerminator::Unreachable));
}

// ArcBlock

#[test]
fn arc_block_construction() {
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(2)),
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    };
    assert_eq!(block.id, ArcBlockId::new(0));
    assert_eq!(block.body.len(), 2);
    assert!(block.params.is_empty());
}

#[test]
fn arc_block_with_params() {
    let block = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(ArcVarId::new(10), Idx::INT), (ArcVarId::new(11), Idx::STR)],
        body: vec![],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(10),
        },
    };
    assert_eq!(block.params.len(), 2);
    assert_eq!(block.params[0].0, ArcVarId::new(10));
    assert_eq!(block.params[1].1, Idx::STR);
}

// ArcFunction

#[test]
fn arc_function_var_type_single() {
    let func = ArcFunction {
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
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        spans: vec![vec![]],
    };
    assert_eq!(func.var_type(ArcVarId::new(0)), Idx::INT);
}

#[test]
fn arc_function_var_type_multiple() {
    let func = ArcFunction {
        name: Name::from_raw(2),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Borrowed,
            },
        ],
        return_type: Idx::BOOL,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(2),
                ty: Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::STR, Idx::BOOL],
        spans: vec![vec![None]],
    };
    assert_eq!(func.var_type(ArcVarId::new(0)), Idx::INT);
    assert_eq!(func.var_type(ArcVarId::new(1)), Idx::STR);
    assert_eq!(func.var_type(ArcVarId::new(2)), Idx::BOOL);
}

// ArcInstr::defined_var

#[test]
fn defined_var_let() {
    let instr = ArcInstr::Let {
        dst: ArcVarId::new(5),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(1)),
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(5)));
}

#[test]
fn defined_var_apply() {
    let instr = ArcInstr::Apply {
        dst: ArcVarId::new(3),
        ty: Idx::STR,
        func: Name::from_raw(10),
        args: vec![ArcVarId::new(0)],
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(3)));
}

#[test]
fn defined_var_apply_indirect() {
    let instr = ArcInstr::ApplyIndirect {
        dst: ArcVarId::new(7),
        ty: Idx::INT,
        closure: ArcVarId::new(1),
        args: vec![ArcVarId::new(2)],
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(7)));
}

#[test]
fn defined_var_project() {
    let instr = ArcInstr::Project {
        dst: ArcVarId::new(4),
        ty: Idx::INT,
        value: ArcVarId::new(0),
        field: 0,
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(4)));
}

#[test]
fn defined_var_construct() {
    let instr = ArcInstr::Construct {
        dst: ArcVarId::new(2),
        ty: Idx::UNIT,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(0)],
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(2)));
}

#[test]
fn defined_var_is_shared() {
    let instr = ArcInstr::IsShared {
        dst: ArcVarId::new(9),
        var: ArcVarId::new(1),
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(9)));
}

#[test]
fn defined_var_reset() {
    let instr = ArcInstr::Reset {
        var: ArcVarId::new(0),
        token: ArcVarId::new(10),
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(10)));
}

#[test]
fn defined_var_reuse() {
    let instr = ArcInstr::Reuse {
        token: ArcVarId::new(10),
        dst: ArcVarId::new(11),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(0)],
    };
    assert_eq!(instr.defined_var(), Some(ArcVarId::new(11)));
}

#[test]
fn defined_var_rc_inc_is_none() {
    let instr = ArcInstr::RcInc {
        var: ArcVarId::new(0),
        count: 1,
    };
    assert_eq!(instr.defined_var(), None);
}

#[test]
fn defined_var_rc_dec_is_none() {
    let instr = ArcInstr::RcDec {
        var: ArcVarId::new(0),
    };
    assert_eq!(instr.defined_var(), None);
}

#[test]
fn defined_var_set_is_none() {
    let instr = ArcInstr::Set {
        base: ArcVarId::new(0),
        field: 0,
        value: ArcVarId::new(1),
    };
    assert_eq!(instr.defined_var(), None);
}

#[test]
fn defined_var_set_tag_is_none() {
    let instr = ArcInstr::SetTag {
        base: ArcVarId::new(0),
        tag: 0,
    };
    assert_eq!(instr.defined_var(), None);
}

// ArcInstr::used_vars

#[test]
fn used_vars_let_var() {
    let instr = ArcInstr::Let {
        dst: ArcVarId::new(1),
        ty: Idx::INT,
        value: ArcValue::Var(ArcVarId::new(0)),
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(0)]);
}

#[test]
fn used_vars_let_literal() {
    let instr = ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    };
    assert!(instr.used_vars().is_empty(), "expected empty used_vars");
}

#[test]
fn used_vars_let_primop() {
    let instr = ArcInstr::Let {
        dst: ArcVarId::new(2),
        ty: Idx::INT,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        },
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn used_vars_apply() {
    let instr = ArcInstr::Apply {
        dst: ArcVarId::new(3),
        ty: Idx::INT,
        func: Name::from_raw(10),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn used_vars_apply_indirect() {
    let instr = ArcInstr::ApplyIndirect {
        dst: ArcVarId::new(5),
        ty: Idx::INT,
        closure: ArcVarId::new(3),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(3), ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn used_vars_construct() {
    let instr = ArcInstr::Construct {
        dst: ArcVarId::new(4),
        ty: Idx::UNIT,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)]
    );
}

#[test]
fn used_vars_project() {
    let instr = ArcInstr::Project {
        dst: ArcVarId::new(2),
        ty: Idx::INT,
        value: ArcVarId::new(0),
        field: 1,
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(0)]);
}

#[test]
fn used_vars_rc_inc() {
    let instr = ArcInstr::RcInc {
        var: ArcVarId::new(3),
        count: 2,
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(3)]);
}

#[test]
fn used_vars_rc_dec() {
    let instr = ArcInstr::RcDec {
        var: ArcVarId::new(7),
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(7)]);
}

#[test]
fn used_vars_set() {
    let instr = ArcInstr::Set {
        base: ArcVarId::new(0),
        field: 1,
        value: ArcVarId::new(2),
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(2)]
    );
}

#[test]
fn used_vars_set_tag() {
    let instr = ArcInstr::SetTag {
        base: ArcVarId::new(5),
        tag: 3,
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(5)]);
}

#[test]
fn used_vars_reset() {
    let instr = ArcInstr::Reset {
        var: ArcVarId::new(0),
        token: ArcVarId::new(10),
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(0)]);
}

#[test]
fn used_vars_reuse() {
    let instr = ArcInstr::Reuse {
        token: ArcVarId::new(10),
        dst: ArcVarId::new(11),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(10), ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn used_vars_is_shared() {
    let instr = ArcInstr::IsShared {
        dst: ArcVarId::new(9),
        var: ArcVarId::new(1),
    };
    assert_eq!(instr.used_vars().as_slice(), [ArcVarId::new(1)]);
}

#[test]
fn used_vars_partial_apply() {
    let instr = ArcInstr::PartialApply {
        dst: ArcVarId::new(6),
        ty: Idx::UNIT,
        func: Name::from_raw(20),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert_eq!(
        instr.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1)]
    );
}

// ArcTerminator::used_vars

#[test]
fn terminator_used_vars_return() {
    let t = ArcTerminator::Return {
        value: ArcVarId::new(5),
    };
    assert_eq!(t.used_vars().as_slice(), [ArcVarId::new(5)]);
}

#[test]
fn terminator_used_vars_jump() {
    let t = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
    };
    assert_eq!(
        t.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn terminator_used_vars_branch() {
    let t = ArcTerminator::Branch {
        cond: ArcVarId::new(3),
        then_block: ArcBlockId::new(1),
        else_block: ArcBlockId::new(2),
    };
    assert_eq!(t.used_vars().as_slice(), [ArcVarId::new(3)]);
}

#[test]
fn terminator_used_vars_switch() {
    let t = ArcTerminator::Switch {
        scrutinee: ArcVarId::new(7),
        cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
        default: ArcBlockId::new(3),
    };
    assert_eq!(t.used_vars().as_slice(), [ArcVarId::new(7)]);
}

#[test]
fn terminator_used_vars_invoke() {
    let t = ArcTerminator::Invoke {
        dst: ArcVarId::new(10),
        ty: Idx::INT,
        func: Name::from_raw(1),
        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        normal: ArcBlockId::new(1),
        unwind: ArcBlockId::new(2),
    };
    assert_eq!(
        t.used_vars().as_slice(),
        [ArcVarId::new(0), ArcVarId::new(1)]
    );
}

#[test]
fn terminator_used_vars_resume() {
    assert!(
        ArcTerminator::Resume.used_vars().is_empty(),
        "Resume should have no used vars"
    );
}

#[test]
fn terminator_used_vars_unreachable() {
    assert!(
        ArcTerminator::Unreachable.used_vars().is_empty(),
        "Unreachable should have no used vars"
    );
}

// ArcFunction helpers

#[test]
fn fresh_var_sequential_ids() {
    let mut func = ArcFunction {
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
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        spans: vec![vec![]],
    };

    let v1 = func.fresh_var(Idx::STR);
    assert_eq!(v1, ArcVarId::new(1));
    assert_eq!(func.var_type(v1), Idx::STR);

    let v2 = func.fresh_var(Idx::BOOL);
    assert_eq!(v2, ArcVarId::new(2));
    assert_eq!(func.var_type(v2), Idx::BOOL);
    assert_eq!(func.var_types.len(), 3);
}

// Serde roundtrip tests (cache feature)

#[cfg(feature = "cache")]
#[test]
fn test_arc_ir_roundtrip() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::INT],
        spans: vec![vec![Some(ori_ir::Span::new(10, 20))]],
    };

    let bytes = bincode::serialize(&func).unwrap_or_else(|e| panic!("serialize failed: {e}"));
    let deserialized: ArcFunction =
        bincode::deserialize(&bytes).unwrap_or_else(|e| panic!("deserialize failed: {e}"));

    // Core data should match exactly
    assert_eq!(deserialized.name, func.name);
    assert_eq!(deserialized.params, func.params);
    assert_eq!(deserialized.return_type, func.return_type);
    assert_eq!(deserialized.blocks, func.blocks);
    assert_eq!(deserialized.entry, func.entry);
    assert_eq!(deserialized.var_types, func.var_types);

    // Spans are skipped during serialization — deserialized gets Default (empty vec)
    assert!(
        deserialized.spans.is_empty(),
        "spans should be empty after deserialization (skipped by serde)"
    );
}

#[cfg(feature = "cache")]
#[test]
fn test_arc_ir_all_instr_variants() {
    // Every ArcInstr variant must serialize/deserialize cleanly
    let instrs = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        },
        ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::FLOAT,
            value: ArcValue::Var(ArcVarId::new(0)),
        },
        ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Add),
                args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            },
        },
        ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: Idx::STR,
            func: Name::from_raw(10),
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(4),
            ty: Idx::INT,
            closure: ArcVarId::new(3),
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::PartialApply {
            dst: ArcVarId::new(5),
            ty: Idx::UNIT,
            func: Name::from_raw(20),
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Project {
            dst: ArcVarId::new(6),
            ty: Idx::INT,
            value: ArcVarId::new(3),
            field: 2,
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(7),
            ty: Idx::UNIT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        },
        ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 3,
        },
        ArcInstr::RcDec {
            var: ArcVarId::new(0),
        },
        ArcInstr::IsShared {
            dst: ArcVarId::new(8),
            var: ArcVarId::new(0),
        },
        ArcInstr::Set {
            base: ArcVarId::new(0),
            field: 1,
            value: ArcVarId::new(1),
        },
        ArcInstr::SetTag {
            base: ArcVarId::new(0),
            tag: 42,
        },
        ArcInstr::Reset {
            var: ArcVarId::new(0),
            token: ArcVarId::new(9),
        },
        ArcInstr::Reuse {
            token: ArcVarId::new(9),
            dst: ArcVarId::new(10),
            ty: Idx::STR,
            ctor: CtorKind::Struct(Name::from_raw(5)),
            args: vec![ArcVarId::new(0)],
        },
    ];

    for (i, instr) in instrs.iter().enumerate() {
        let bytes =
            bincode::serialize(instr).unwrap_or_else(|e| panic!("serialize instr {i} failed: {e}"));
        let roundtripped: ArcInstr = bincode::deserialize(&bytes)
            .unwrap_or_else(|e| panic!("deserialize instr {i} failed: {e}"));
        assert_eq!(
            &roundtripped, instr,
            "roundtrip failed for instr variant {i}"
        );
    }

    // Also test all terminator variants
    let terminators = vec![
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
        ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![ArcVarId::new(0)],
        },
        ArcTerminator::Branch {
            cond: ArcVarId::new(0),
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
        ArcTerminator::Switch {
            scrutinee: ArcVarId::new(0),
            cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
            default: ArcBlockId::new(3),
        },
        ArcTerminator::Invoke {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: Name::from_raw(10),
            args: vec![ArcVarId::new(0)],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
        ArcTerminator::Resume,
        ArcTerminator::Unreachable,
    ];

    for (i, term) in terminators.iter().enumerate() {
        let bytes = bincode::serialize(term)
            .unwrap_or_else(|e| panic!("serialize terminator {i} failed: {e}"));
        let roundtripped: ArcTerminator = bincode::deserialize(&bytes)
            .unwrap_or_else(|e| panic!("deserialize terminator {i} failed: {e}"));
        assert_eq!(
            &roundtripped, term,
            "roundtrip failed for terminator variant {i}"
        );
    }
}

// ArcFunction block management

#[test]
fn next_block_id_and_push() {
    let mut func = ArcFunction {
        name: Name::from_raw(1),
        params: vec![],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![],
        spans: vec![vec![]],
    };

    assert_eq!(func.next_block_id(), ArcBlockId::new(1));

    func.push_block(ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        }],
        terminator: ArcTerminator::Unreachable,
    });

    assert_eq!(func.blocks.len(), 2);
    assert_eq!(func.spans.len(), 2);
    assert_eq!(func.spans[1].len(), 1); // one instr → one span slot
    assert_eq!(func.next_block_id(), ArcBlockId::new(2));
}
