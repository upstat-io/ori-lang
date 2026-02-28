//! Switch emitters for decision tree pattern matching.
//!
//! Handles tag switch (enum dispatch), int/bool/char switch, string/float
//! equality chains, range check chains, and the select chain optimization
//! for branchless trivial matches.

use ori_ir::canon::CanExpr;
use ori_ir::Span;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::emit::{emit_tree, EmitContext};
use super::{DecisionTree, ScrutineePath, TestKind, TestValue};

/// Dispatch to the appropriate switch emitter based on the test kind.
pub(super) fn emit_switch(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    path: &ScrutineePath,
    test_kind: TestKind,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    let scrutinee = super::emit::resolve_path(
        lowerer,
        ctx.root_scrutinee,
        ctx.root_scrutinee_ty,
        path,
        ctx.span,
        &ctx.variant_stack,
    );

    match test_kind {
        TestKind::EnumTag => emit_tag_switch(lowerer, scrutinee, edges, default, ctx),
        TestKind::IntEq | TestKind::BoolEq | TestKind::CharEq | TestKind::ListLen => {
            emit_int_switch(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::StrEq | TestKind::FloatEq => {
            emit_str_chain(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::IntRange => emit_range_chain(lowerer, scrutinee, edges, default, ctx),
    }
}

/// Emit a `Switch` terminator for enum tag dispatch.
///
/// Extracts the tag field (field 0) and switches on it.
fn emit_tag_switch(
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

/// Emit a `Switch` terminator for integer/bool/list-length dispatch.
fn emit_int_switch(
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

/// Emit an if-else chain for string/float equality dispatch.
///
/// LLVM doesn't support `switch` on strings/floats, so we emit sequential
/// `Branch` terminators comparing the scrutinee to each value.
fn emit_str_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    for (tv, subtree) in edges {
        // Emit the comparison.
        let expected = emit_test_value_literal(lowerer, tv, ctx.span);
        let cmp = lowerer.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![scrutinee, expected],
            },
            Some(ctx.span),
        );

        let match_block = lowerer.builder.new_block();
        let next_block = lowerer.builder.new_block();
        lowerer
            .builder
            .terminate_branch(cmp, match_block, next_block);

        // Match block: emit the subtree.
        lowerer.builder.position_at(match_block);
        emit_tree(lowerer, subtree, ctx);

        // Continue at next_block for the next comparison.
        lowerer.builder.position_at(next_block);
    }

    // After all comparisons: emit default or unreachable.
    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}

/// Emit range check chains (lo <= value && value <= hi).
fn emit_range_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    for (tv, subtree) in edges {
        if let TestValue::IntRange { lo, hi, inclusive } = tv {
            // lo <= scrutinee
            let lo_val =
                lowerer
                    .builder
                    .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*lo)), None);
            let lo_ok = lowerer.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::GtEq),
                    args: vec![scrutinee, lo_val],
                },
                Some(ctx.span),
            );

            // scrutinee <= hi (inclusive) or scrutinee < hi (exclusive)
            let hi_val =
                lowerer
                    .builder
                    .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*hi)), None);
            let hi_op = if *inclusive {
                PrimOp::Binary(ori_ir::BinaryOp::LtEq)
            } else {
                PrimOp::Binary(ori_ir::BinaryOp::Lt)
            };
            let hi_ok = lowerer.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: hi_op,
                    args: vec![scrutinee, hi_val],
                },
                Some(ctx.span),
            );

            // lo_ok && hi_ok
            let in_range = lowerer.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::And),
                    args: vec![lo_ok, hi_ok],
                },
                Some(ctx.span),
            );

            let match_block = lowerer.builder.new_block();
            let next_block = lowerer.builder.new_block();
            lowerer
                .builder
                .terminate_branch(in_range, match_block, next_block);

            lowerer.builder.position_at(match_block);
            emit_tree(lowerer, subtree, ctx);

            lowerer.builder.position_at(next_block);
        }
    }

    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}

// Select chain optimization

/// Maximum edges for select chain eligibility.
///
/// Beyond this threshold a switch/jump table is likely more efficient
/// than a linear chain of `select` instructions.
const MAX_SELECT_EDGES: usize = 8;

/// Check whether a `CanExpr` is simple enough to lower without creating
/// new basic blocks — i.e. literals and variable references.
fn is_simple_body(lowerer: &crate::lower::ArcLowerer<'_>, body: ori_ir::canon::CanId) -> bool {
    matches!(
        lowerer.arena.kind(body),
        CanExpr::Int(_)
            | CanExpr::Bool(_)
            | CanExpr::Float(_)
            | CanExpr::Char(_)
            | CanExpr::Str(_)
            | CanExpr::Unit
            | CanExpr::Ident(_)
    )
}

/// Check whether a switch is eligible for select chain optimization.
///
/// A switch qualifies when:
/// - All edges lead to `Leaf` nodes with no pattern variable bindings
/// - Each leaf's arm body is a simple expression (literal or variable)
/// - No mutable variables need SSA merge at the convergence point
/// - The number of edges is within [`MAX_SELECT_EDGES`]
fn is_select_eligible(
    lowerer: &crate::lower::ArcLowerer<'_>,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &EmitContext,
) -> bool {
    if !ctx.mutable_var_names.is_empty() {
        return false;
    }
    if edges.is_empty() || edges.len() > MAX_SELECT_EDGES {
        return false;
    }
    for (tv, tree) in edges {
        // Only Int/Bool/Char/Tag can be compared with a simple icmp eq.
        // ListLen requires length extraction; Str/Float use runtime calls;
        // IntRange needs range checks — none are suitable for select chains.
        if !matches!(
            tv,
            TestValue::Int(_) | TestValue::Bool(_) | TestValue::Char(_) | TestValue::Tag { .. }
        ) {
            return false;
        }
        match tree {
            DecisionTree::Leaf {
                arm_index,
                bindings,
            } => {
                if !bindings.is_empty() || !is_simple_body(lowerer, ctx.arm_bodies[*arm_index]) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    match default {
        Some(DecisionTree::Leaf {
            arm_index,
            bindings,
        }) => bindings.is_empty() && is_simple_body(lowerer, ctx.arm_bodies[*arm_index]),
        Some(DecisionTree::Fail) | None => true,
        _ => false,
    }
}

/// Emit a select chain for a switch node, producing branchless code.
///
/// Instead of creating N basic blocks + a switch terminator + phi merge,
/// emits a chain of `icmp eq` + `Select` instructions in a single block.
/// This eliminates branch misprediction for small, trivial matches like:
///
/// ```text
/// match x { 0 => 10, 1 => 20, _ => 30 }
/// ```
///
/// Becomes (in a single block, no branches):
/// ```text
/// acc   = 30                      // default body
/// cmp0  = eq x, 0
/// acc   = select cmp0, 10, acc    // if x==0 then 10 else 30
/// cmp1  = eq x, 1
/// acc   = select cmp1, 20, acc    // if x==1 then 20 else previous acc
/// jump merge_block(acc)
/// ```
fn emit_select_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    // Determine the fallback value and which edges need explicit comparison.
    //
    // If there's a default arm (wildcard), use its body as fallback and compare
    // all edges. If the match is exhaustive (no default), use the last edge's
    // body as fallback and skip its comparison — it's implied by elimination.
    let (mut acc, compare_edges) = if let Some(DecisionTree::Leaf { arm_index, .. }) = default {
        let val = lowerer.lower_expr(ctx.arm_bodies[*arm_index]);
        (val, edges)
    } else {
        let Some((last, rest)) = edges.split_last() else {
            unreachable!("is_select_eligible guarantees non-empty edges");
        };
        let arm_index = match &last.1 {
            DecisionTree::Leaf { arm_index, .. } => *arm_index,
            _ => unreachable!("is_select_eligible guarantees leaf"),
        };
        let val = lowerer.lower_expr(ctx.arm_bodies[arm_index]);
        (val, rest)
    };

    let result_ty = lowerer.builder.var_type(acc);

    // Build the select chain: for each edge, emit comparison + select.
    for (tv, subtree) in compare_edges {
        let arm_index = match subtree {
            DecisionTree::Leaf { arm_index, .. } => *arm_index,
            _ => unreachable!("is_select_eligible guarantees leaf"),
        };
        let body_val = lowerer.lower_expr(ctx.arm_bodies[arm_index]);
        let cmp = emit_eq_test(lowerer, scrutinee, tv, ctx.span);
        acc = lowerer
            .builder
            .emit_select(result_ty, cmp, body_val, acc, Some(ctx.span));
    }

    // Jump to merge block with the result (no mutable vars — checked by eligibility).
    lowerer.builder.terminate_jump(ctx.merge_block, vec![acc]);
}

/// Emit an equality test between a scrutinee variable and a test value.
///
/// Returns a bool variable that is `true` when `scrutinee == tv`.
fn emit_eq_test(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    tv: &TestValue,
    span: Span,
) -> ArcVarId {
    let expected = match tv {
        TestValue::Int(v) => {
            lowerer
                .builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*v)), Some(span))
        }
        TestValue::Bool(v) => {
            lowerer
                .builder
                .emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(*v)), Some(span))
        }
        TestValue::Char(c) => {
            lowerer
                .builder
                .emit_let(Idx::CHAR, ArcValue::Literal(LitValue::Char(*c)), Some(span))
        }
        TestValue::Tag { variant_index, .. } => lowerer.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(i64::from(*variant_index))),
            Some(span),
        ),
        _ => unreachable!("select chain only for int/bool/char/tag tests"),
    };

    lowerer.builder.emit_let(
        Idx::BOOL,
        ArcValue::PrimOp {
            op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
            args: vec![scrutinee, expected],
        },
        Some(span),
    )
}

/// Emit a literal value corresponding to a `TestValue`.
fn emit_test_value_literal(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    tv: &TestValue,
    span: Span,
) -> ArcVarId {
    match tv {
        TestValue::Str(name) => lowerer.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(*name)),
            Some(span),
        ),
        TestValue::Float(bits) => lowerer.builder.emit_let(
            Idx::FLOAT,
            ArcValue::Literal(LitValue::Int((*bits).cast_signed())),
            Some(span),
        ),
        TestValue::Char(c) => lowerer.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(i64::from(u32::from(*c)))),
            Some(span),
        ),
        // Int, Bool, Tag, IntRange, ListLen use emit_int_switch/emit_tag_switch, not this chain.
        _ => {
            tracing::warn!(
                ?tv,
                "unexpected TestValue in emit_test_value_literal; emitting 0"
            );
            lowerer
                .builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), Some(span))
        }
    }
}
