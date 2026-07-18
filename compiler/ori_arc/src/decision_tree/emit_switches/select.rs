//! Branchless lowering for small matches with trivial arms.
//!
//! The default arm seeds an accumulator, and each explicit case conditionally
//! selects its value into that accumulator. One final jump supplies the result
//! to the merge block, avoiding per-arm blocks and a switch terminator.

use ori_ir::canon::CanExpr;
use ori_ir::Span;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::super::emit::EmitContext;
use super::super::{DecisionTree, TestValue};

/// Maximum edges for select chain eligibility.
///
/// Beyond this threshold a switch/jump table is likely more efficient
/// than a linear chain of `select` instructions.
const MAX_SELECT_EDGES: usize = 8;

/// Check whether a `CanExpr` is simple enough to lower without creating
/// new basic blocks — i.e. literals and variable references.
///
/// An `Ident` qualifies ONLY when its type is SCALAR: a `Select` between two
/// owned RC-bearing values makes the non-selected operand's ownership
/// runtime-conditional with no CFG edge to release it on (the RL-4 edge
/// machinery needs branch form), so RC-bearing arms take the standard
/// branch + merge lowering instead.
fn is_simple_body(lowerer: &crate::lower::ArcLowerer<'_>, body: ori_ir::canon::CanId) -> bool {
    match lowerer.arena.kind(body) {
        CanExpr::Int(_)
        | CanExpr::Bool(_)
        | CanExpr::Float(_)
        | CanExpr::Char(_)
        | CanExpr::Unit => true,
        CanExpr::Str(_) | CanExpr::Ident(_) => {
            use crate::classify::ArcClassification;
            let ty = lowerer.expr_type(body);
            crate::classify::ArcClassifier::new(lowerer.pool).is_scalar(ty)
        }
        _ => false,
    }
}

/// Check whether a switch is eligible for select chain optimization.
///
/// A switch qualifies when:
/// - All edges lead to `Leaf` nodes with no pattern variable bindings
/// - Each leaf's arm body is a simple expression (literal or variable)
/// - No mutable variables need SSA merge at the convergence point
/// - No leaf carries blank-pattern cleanup projections
/// - The number of edges is within [`MAX_SELECT_EDGES`]
pub(super) fn is_select_eligible(
    lowerer: &crate::lower::ArcLowerer<'_>,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &EmitContext,
) -> bool {
    if !ctx.mutable_var_names.is_empty() || ctx.has_discard_obligations() {
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
pub(super) fn emit_select_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    // A wildcard supplies the fallback while all edges are compared; an
    // exhaustive match uses its implied final edge as the fallback.
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
