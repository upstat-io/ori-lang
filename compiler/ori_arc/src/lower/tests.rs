use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcTerminator, ArcValue, LitValue};

use super::*;

#[test]
fn builder_creates_entry_block() {
    let builder = ArcIrBuilder::new();
    assert_eq!(builder.current_block(), ArcBlockId::new(0));
    assert!(!builder.is_terminated());
}

#[test]
fn builder_allocates_fresh_vars() {
    let mut builder = ArcIrBuilder::new();
    let v0 = builder.fresh_var(Idx::INT);
    let v1 = builder.fresh_var(Idx::BOOL);
    assert_eq!(v0, ArcVarId::new(0));
    assert_eq!(v1, ArcVarId::new(1));
    assert_eq!(builder.var_types[v0.index()], Idx::INT);
    assert_eq!(builder.var_types[v1.index()], Idx::BOOL);
}

#[test]
fn builder_new_block_and_position() {
    let mut builder = ArcIrBuilder::new();
    let bb1 = builder.new_block();
    assert_eq!(bb1, ArcBlockId::new(1));
    builder.position_at(bb1);
    assert_eq!(builder.current_block(), bb1);
}

#[test]
fn builder_emit_let_and_return() {
    let mut builder = ArcIrBuilder::new();
    let v = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(42)), None);
    builder.terminate_return(v);
    assert!(builder.is_terminated());

    let func = builder.finish(Name::from_raw(1), vec![], Idx::INT, ArcBlockId::new(0));
    assert_eq!(func.blocks.len(), 1);
    assert_eq!(func.blocks[0].body.len(), 1);
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Return { .. }
    ));
}

#[test]
fn builder_block_params() {
    let mut builder = ArcIrBuilder::new();
    let bb1 = builder.new_block();
    let param_var = builder.add_block_param(bb1, Idx::INT);
    assert_eq!(param_var.raw(), 0);

    let func = builder.finish(Name::from_raw(1), vec![], Idx::UNIT, ArcBlockId::new(0));
    assert_eq!(func.blocks[1].params.len(), 1);
    assert_eq!(func.blocks[1].params[0].1, Idx::INT);
}

#[test]
fn builder_branch_terminator() {
    let mut builder = ArcIrBuilder::new();
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let cond = builder.emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(true)), None);
    builder.terminate_branch(cond, then_bb, else_bb);

    assert!(builder.is_terminated());
}

#[test]
fn builder_jump_terminator() {
    let mut builder = ArcIrBuilder::new();
    let target = builder.new_block();
    builder.terminate_jump(target, vec![]);
    assert!(builder.is_terminated());
}

#[test]
fn builder_switch_terminator() {
    let mut builder = ArcIrBuilder::new();
    let bb1 = builder.new_block();
    let bb2 = builder.new_block();
    let default = builder.new_block();
    let scrut = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
    builder.terminate_switch(scrut, vec![(0, bb1), (1, bb2)], default);
    assert!(builder.is_terminated());
}

#[test]
fn builder_emit_apply() {
    let mut builder = ArcIrBuilder::new();
    let arg = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    let _result = builder.emit_apply(Idx::INT, Name::from_raw(10), vec![arg], None);
    assert_eq!(builder.blocks[0].body.len(), 2);
}

#[test]
fn builder_emit_construct() {
    let mut builder = ArcIrBuilder::new();
    let a = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    let b = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(2)), None);
    let _tup = builder.emit_construct(Idx::UNIT, CtorKind::Tuple, vec![a, b], None);
    assert_eq!(builder.blocks[0].body.len(), 3);
}

#[test]
fn builder_emit_project() {
    let mut builder = ArcIrBuilder::new();
    let tup = builder.emit_let(Idx::UNIT, ArcValue::Literal(LitValue::Unit), None);
    let _field = builder.emit_project(Idx::INT, tup, 0, None);
    assert_eq!(builder.blocks[0].body.len(), 2);
}

#[test]
fn builder_finish_adds_unreachable_to_unterminated() {
    let mut builder = ArcIrBuilder::new();
    builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    // Don't terminate — finish should add Unreachable.
    let func = builder.finish(Name::from_raw(1), vec![], Idx::INT, ArcBlockId::new(0));
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Unreachable
    ));
}

#[test]
fn builder_multi_block_function() {
    let mut builder = ArcIrBuilder::new();

    // Entry block: branch to then/else.
    let then_bb = builder.new_block();
    let else_bb = builder.new_block();
    let merge_bb = builder.new_block();

    let cond = builder.emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(true)), None);
    builder.terminate_branch(cond, then_bb, else_bb);

    // Then block.
    builder.position_at(then_bb);
    let v1 = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    builder.terminate_jump(merge_bb, vec![v1]);

    // Else block.
    builder.position_at(else_bb);
    let v2 = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(2)), None);
    builder.terminate_jump(merge_bb, vec![v2]);

    // Merge block.
    builder.position_at(merge_bb);
    let result = builder.add_block_param(merge_bb, Idx::INT);
    builder.terminate_return(result);

    let func = builder.finish(Name::from_raw(1), vec![], Idx::INT, ArcBlockId::new(0));
    assert_eq!(func.blocks.len(), 4);
    assert_eq!(func.blocks[3].params.len(), 1); // merge block has 1 param
}

#[test]
fn builder_spans_tracked_per_instruction() {
    let mut builder = ArcIrBuilder::new();
    let span1 = Some(Span::new(0, 5));
    let span2 = Some(Span::new(10, 15));
    let v = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), span1);
    builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(2)), span2);
    builder.terminate_return(v);

    let func = builder.finish(Name::from_raw(1), vec![], Idx::INT, ArcBlockId::new(0));
    assert_eq!(func.spans[0].len(), 2);
    assert_eq!(func.spans[0][0], span1);
    assert_eq!(func.spans[0][1], span2);
}
