//! Enum tag switch emitter.

use ori_types::Idx;

use crate::ir::ArcVarId;

use super::super::emit::{emit_tree, EmitContext};
use super::super::{DecisionTree, TestValue};
use super::select::{emit_select_chain, is_select_eligible};

/// Emit a `Switch` terminator for enum tag dispatch.
///
/// Extracts the tag field (field 0) and switches on it.
pub(in crate::decision_tree) fn emit_tag_switch(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    // Extract the tag from the scrutinee (field 0 for enums).
    let tag = lowerer
        .builder
        .emit_project(Idx::INT, scrutinee, 0, Some(ctx.span));

    // Try select chain for trivial matches (branchless).
    if is_select_eligible(lowerer, edges, default, ctx) {
        return emit_select_chain(lowerer, tag, edges, default, ctx);
    }

    // Create blocks for each edge.
    let mut case_blocks = Vec::with_capacity(edges.len());
    let mut edge_blocks = Vec::with_capacity(edges.len());
    for (tv, _) in edges {
        let block = lowerer.builder.new_block();
        let variant_index = match tv {
            TestValue::Tag { variant_index, .. } => u64::from(*variant_index),
            _ => 0,
        };
        case_blocks.push((variant_index, block));
        edge_blocks.push(block);
    }

    // Default block.
    let default_block = lowerer.builder.new_block();

    // Emit the Switch terminator.
    lowerer
        .builder
        .terminate_switch(tag, case_blocks, default_block);

    // Get the scrutinee's enum type for variant context tracking.
    let scrut_ty = lowerer.builder.var_type(scrutinee);

    // Emit each edge's subtree with variant context pushed.
    for (i, (tv, subtree)) in edges.iter().enumerate() {
        lowerer.builder.position_at(edge_blocks[i]);
        let variant_index = match tv {
            TestValue::Tag { variant_index, .. } => *variant_index,
            _ => 0,
        };
        ctx.variant_stack.push((scrut_ty, variant_index));
        emit_tree(lowerer, subtree, ctx);
        ctx.variant_stack.pop();
    }

    // Emit the default block.
    lowerer.builder.position_at(default_block);
    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}
