//! Emit ARC IR basic blocks from a compiled [`DecisionTree`].
//!
//! This is the final step in pattern match compilation:
//! 1. `flatten.rs` converts `MatchPattern` → `FlatPattern`
//! 2. `compile.rs` compiles the pattern matrix into a `DecisionTree`
//! 3. **This module** walks the tree and emits ARC IR blocks with
//!    `Switch`/`Branch` terminators
//!
//! The emission is performed by [`emit_decision_tree`], called from
//! `lower_match` in `lower/control_flow.rs`.

use ori_ir::{ExprId, Name, Span};
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp};

use super::{DecisionTree, PathInstruction, ScrutineePath, TestKind, TestValue};

/// Context for decision tree emission.
///
/// Holds references to the arms' body expressions and the merge block
/// where all arms converge. Shared body blocks for or-patterns (same
/// `arm_index`) are tracked in `arm_body_blocks`.
pub(crate) struct EmitContext {
    /// The root scrutinee variable.
    pub root_scrutinee: ArcVarId,
    /// Type of the root scrutinee (for projections).
    pub scrutinee_ty: Idx,
    /// The merge block all arms jump to after executing their body.
    pub merge_block: ArcBlockId,
    /// The body expression for each arm (indexed by `arm_index`).
    pub arm_bodies: Vec<ExprId>,
    /// Span of the match expression.
    pub span: Span,
}

/// Emit a decision tree as ARC IR basic blocks.
///
/// This is a method on `ArcLowerer` because it needs access to the builder,
/// scope, arena, and expression lowering. It recursively walks the tree,
/// creating blocks and terminators.
///
/// # How it works
///
/// - **`Switch`**: Resolves the scrutinee path, then for `EnumTag`/`IntEq`/`BoolEq`
///   emits an `ArcTerminator::Switch`. For `StrEq` (no LLVM switch support for
///   strings), emits an if-else chain of `Branch` terminators.
///
/// - **`Leaf`**: Binds pattern variables by resolving paths from the root scrutinee,
///   then lowers the arm body and jumps to the merge block.
///
/// - **`Guard`**: Binds variables, evaluates the guard expression, then branches
///   to the body block (if guard passes) or the `on_fail` subtree (if it fails).
///
/// - **`Fail`**: Emits `unreachable` (exhaustiveness guarantees this is dead code).
pub(crate) fn emit_tree(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    tree: &DecisionTree,
    ctx: &mut EmitContext,
) {
    match tree {
        DecisionTree::Switch {
            path,
            test_kind,
            edges,
            default,
        } => emit_switch(lowerer, path, *test_kind, edges, default.as_deref(), ctx),

        DecisionTree::Leaf {
            arm_index,
            bindings,
        } => emit_leaf(lowerer, *arm_index, bindings, ctx),

        DecisionTree::Guard {
            arm_index,
            bindings,
            guard,
            on_fail,
        } => emit_guard(lowerer, *arm_index, bindings, *guard, on_fail, ctx),

        DecisionTree::Fail => {
            lowerer.builder.terminate_unreachable();
        }
    }
}

// ── Switch Emission ─────────────────────────────────────────────────

fn emit_switch(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    path: &ScrutineePath,
    test_kind: TestKind,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    let scrutinee = resolve_path(
        lowerer,
        ctx.root_scrutinee,
        ctx.scrutinee_ty,
        path,
        ctx.span,
    );

    match test_kind {
        TestKind::EnumTag => emit_tag_switch(lowerer, scrutinee, edges, default, ctx),
        TestKind::IntEq | TestKind::BoolEq | TestKind::ListLen => {
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

    // Emit each edge's subtree.
    for (i, (_, subtree)) in edges.iter().enumerate() {
        lowerer.builder.position_at(edge_blocks[i]);
        emit_tree(lowerer, subtree, ctx);
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
    let mut case_blocks = Vec::with_capacity(edges.len());
    let mut edge_blocks = Vec::with_capacity(edges.len());

    for (tv, _) in edges {
        let block = lowerer.builder.new_block();
        let case_val = match tv {
            TestValue::Int(v) => (*v).cast_unsigned(),
            TestValue::Bool(v) => u64::from(*v),
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
        if let TestValue::IntRange { lo, hi } = tv {
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

            // scrutinee <= hi
            let hi_val =
                lowerer
                    .builder
                    .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*hi)), None);
            let hi_ok = lowerer.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::LtEq),
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

// ── Leaf Emission ───────────────────────────────────────────────────

/// Emit a leaf node: bind pattern variables and execute the arm body.
fn emit_leaf(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    arm_index: usize,
    bindings: &[(Name, ScrutineePath)],
    ctx: &mut EmitContext,
) {
    // Bind pattern variables by resolving paths from root scrutinee.
    bind_pattern_variables(lowerer, bindings, ctx);

    // Lower the arm body and jump to merge block.
    let body_expr = ctx.arm_bodies[arm_index];
    let body_val = lowerer.lower_expr(body_expr);
    if !lowerer.builder.is_terminated() {
        lowerer
            .builder
            .terminate_jump(ctx.merge_block, vec![body_val]);
    }
}

// ── Guard Emission ──────────────────────────────────────────────────

/// Emit a guard node: bind variables, test guard, branch.
fn emit_guard(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    arm_index: usize,
    bindings: &[(Name, ScrutineePath)],
    guard: ori_ir::canon::CanId,
    on_fail: &DecisionTree,
    ctx: &mut EmitContext,
) {
    // Bind pattern variables.
    bind_pattern_variables(lowerer, bindings, ctx);

    // Evaluate the guard expression.
    // Bridge: ARC backend hasn't migrated to CanonResult yet, so convert
    // CanId back to ExprId for the current lowering path.
    let guard_result = lowerer.lower_expr(guard.to_expr_id());

    let body_block = lowerer.builder.new_block();
    let fail_block = lowerer.builder.new_block();
    lowerer
        .builder
        .terminate_branch(guard_result, body_block, fail_block);

    // Guard passed: execute arm body, jump to merge.
    lowerer.builder.position_at(body_block);
    let body_expr = ctx.arm_bodies[arm_index];
    let body_val = lowerer.lower_expr(body_expr);
    if !lowerer.builder.is_terminated() {
        lowerer
            .builder
            .terminate_jump(ctx.merge_block, vec![body_val]);
    }

    // Guard failed: continue matching.
    lowerer.builder.position_at(fail_block);
    emit_tree(lowerer, on_fail, ctx);
}

// ── Path Resolution ─────────────────────────────────────────────────

/// Resolve a scrutinee path to an `ArcVarId` by emitting `Project` instructions.
///
/// Starting from `root`, follows each `PathInstruction` step, projecting
/// fields at each level to reach the target sub-value.
// Field indices never exceed u32.
#[allow(clippy::cast_possible_truncation)]
fn resolve_path(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    root: ArcVarId,
    _root_ty: Idx,
    path: &[PathInstruction],
    span: Span,
) -> ArcVarId {
    let mut current = root;
    for step in path {
        let field = match step {
            // For enum variants, payload fields start at index 1 (index 0 is the tag).
            PathInstruction::TagPayload(f) => f + 1,
            PathInstruction::TupleIndex(idx)
            | PathInstruction::StructField(idx)
            | PathInstruction::ListElement(idx) => *idx,
        };
        // Use UNIT as the type — the type system has already validated the access.
        // The actual type will be resolved during subsequent ARC analysis passes.
        current = lowerer
            .builder
            .emit_project(Idx::UNIT, current, field, Some(span));
    }
    current
}

// ── Binding ─────────────────────────────────────────────────────────

/// Bind pattern variables by resolving their paths from the root scrutinee.
fn bind_pattern_variables(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    bindings: &[(Name, ScrutineePath)],
    ctx: &EmitContext,
) {
    for (name, path) in bindings {
        let var = resolve_path(
            lowerer,
            ctx.root_scrutinee,
            ctx.scrutinee_ty,
            path,
            ctx.span,
        );
        lowerer.scope.bind(*name, var);
    }
}

// ── Literal Emission ────────────────────────────────────────────────

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
        // Int, Bool, Tag, IntRange, ListLen shouldn't reach here (handled by Switch).
        _ => lowerer
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), Some(span)),
    }
}
