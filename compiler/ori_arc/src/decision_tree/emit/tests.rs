use super::*;
use crate::decision_tree::PathInstruction;

fn context_with_discard_paths(leaf_discard_paths: Vec<LeafDiscardPaths>) -> EmitContext {
    EmitContext::new(EmitContextInit {
        root_scrutinee: ArcVarId::new(0),
        root_scrutinee_ty: Idx::UNIT,
        merge_block: crate::ir::ArcBlockId::new(0),
        arm_bodies: Vec::new(),
        span: Span::new(0, 0),
        pre_scope: ArcScope::new(),
        mutable_var_names: Vec::new(),
        leaf_discard_paths,
    })
}

#[test]
fn discard_carrier_table_success_order_is_preserved() {
    let first = vec![vec![PathInstruction::TupleIndex(0)]];
    let second = vec![vec![PathInstruction::TupleIndex(1)]];
    let mut ctx = context_with_discard_paths(vec![first.clone(), second.clone()]);

    assert_eq!(ctx.take_next_discard_paths(), first);
    assert_eq!(ctx.take_next_discard_paths(), second);
}

#[test]
#[should_panic(expected = "has no matching cleanup carrier")]
fn truncated_discard_carrier_table_is_not_silently_ignored() {
    let mut ctx = context_with_discard_paths(vec![Vec::new()]);

    let _ = ctx.take_next_discard_paths();
    let _ = ctx.take_next_discard_paths();
}
