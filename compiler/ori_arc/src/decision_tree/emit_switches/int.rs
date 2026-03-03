//! Integer/bool/char/list-length switch emitter.

use crate::ir::ArcVarId;

use super::super::emit::{emit_tree, EmitContext};
use super::super::{DecisionTree, TestValue};
use super::select::{emit_select_chain, is_select_eligible};

/// Emit a `Switch` terminator for integer/bool/list-length dispatch.
pub(in crate::decision_tree) fn emit_int_switch(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    // Try select chain for trivial matches (branchless).
    if is_select_eligible(lowerer, edges, default, ctx) {
        return emit_select_chain(lowerer, scrutinee, edges, default, ctx);
    }

    let mut case_blocks = Vec::with_capacity(edges.len());
    let mut edge_blocks = Vec::with_capacity(edges.len());

    for (tv, _) in edges {
        let block = lowerer.builder.new_block();
        let case_val = match tv {
            TestValue::Int(v) => (*v).cast_unsigned(),
            TestValue::Bool(v) => u64::from(*v),
            TestValue::Char(c) => u64::from(u32::from(*c)),
            TestValue::ListLen { len, .. } => u64::from(*len),
            _ => 0,
        };
        case_blocks.push((case_val, block));
        edge_blocks.push(block);
    }

    let default_block = lowerer.builder.new_block();
    lowerer
        .builder
        .terminate_switch(scrutinee, case_blocks, default_block);

    for (i, (_, subtree)) in edges.iter().enumerate() {
        lowerer.builder.position_at(edge_blocks[i]);
        emit_tree(lowerer, subtree, ctx);
    }

    lowerer.builder.position_at(default_block);
    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}
