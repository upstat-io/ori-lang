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

use ori_ir::canon::CanId;
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::scope::ArcScope;

use super::{DecisionTree, PathInstruction, ScrutineePath, TestKind, TestValue};

/// Context for decision tree emission.
///
/// Holds references to the arms' body expressions and the merge block
/// where all arms converge. Also carries SSA merge info for mutable
/// variables that may be reassigned in arm bodies.
pub(crate) struct EmitContext {
    /// The root scrutinee variable.
    pub root_scrutinee: ArcVarId,
    /// Pool type of the root scrutinee (for type-aware path resolution).
    pub root_scrutinee_ty: Idx,
    /// The merge block all arms jump to after executing their body.
    pub merge_block: ArcBlockId,
    /// The body expression for each arm (indexed by `arm_index`).
    pub arm_bodies: Vec<CanId>,
    /// Span of the match expression.
    pub span: Span,
    /// Scope snapshot from before the match. Each arm resets to this
    /// before lowering its body, ensuring arm-to-arm scope isolation.
    pub pre_scope: ArcScope,
    /// Mutable variable names to pass as merge block params (in order).
    /// These serve as SSA phi inputs at the match convergence point.
    pub mutable_var_names: Vec<Name>,
    /// Variant context stack for type-aware path resolution.
    ///
    /// Each entry is `(enum_type, variant_index)` pushed by `emit_tag_switch`
    /// when entering a specific variant's case block. `resolve_path` uses this
    /// to look up the actual field type for `TagPayload` steps, which is
    /// critical for recursive enums where fields may be RC-boxed pointers.
    pub variant_stack: Vec<(Idx, u32)>,
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

// Switch emission

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

// Leaf emission

/// Emit a leaf node: bind pattern variables and execute the arm body.
///
/// Resets the scope to the pre-match snapshot before lowering the body,
/// ensuring arm-to-arm isolation. Passes mutable variable values as
/// jump args for SSA merge at the convergence block.
fn emit_leaf(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    arm_index: usize,
    bindings: &[(Name, ScrutineePath)],
    ctx: &mut EmitContext,
) {
    // Reset scope to pre-match snapshot for arm isolation.
    lowerer.scope = ctx.pre_scope.clone();

    // Bind pattern variables by resolving paths from root scrutinee.
    bind_pattern_variables(lowerer, bindings, ctx);

    // Lower the arm body and jump to merge block.
    let body_expr = ctx.arm_bodies[arm_index];
    let body_val = lowerer.lower_expr(body_expr);
    if !lowerer.builder.is_terminated() {
        let mut jump_args = vec![body_val];
        // Append mutable variable values for SSA merge.
        for name in &ctx.mutable_var_names {
            let var = lowerer.scope.lookup(*name).unwrap_or(body_val);
            jump_args.push(var);
        }
        lowerer.builder.terminate_jump(ctx.merge_block, jump_args);
    }
}

// Guard emission

/// Emit a guard node: bind variables, test guard, branch.
///
/// Resets the scope to the pre-match snapshot before lowering, ensuring
/// arm isolation. Passes mutable variable values in the merge jump.
fn emit_guard(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    arm_index: usize,
    bindings: &[(Name, ScrutineePath)],
    guard: ori_ir::canon::CanId,
    on_fail: &DecisionTree,
    ctx: &mut EmitContext,
) {
    // Reset scope to pre-match snapshot for arm isolation.
    lowerer.scope = ctx.pre_scope.clone();

    // Bind pattern variables.
    bind_pattern_variables(lowerer, bindings, ctx);

    // Evaluate the guard expression.
    let guard_result = lowerer.lower_expr(guard);

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
        let mut jump_args = vec![body_val];
        for name in &ctx.mutable_var_names {
            let var = lowerer.scope.lookup(*name).unwrap_or(body_val);
            jump_args.push(var);
        }
        lowerer.builder.terminate_jump(ctx.merge_block, jump_args);
    }

    // Guard failed: continue matching.
    lowerer.builder.position_at(fail_block);
    emit_tree(lowerer, on_fail, ctx);
}

// Path resolution

/// Resolve a scrutinee path to an `ArcVarId` by emitting `Project` instructions.
///
/// Starting from `root`, follows each `PathInstruction` step, projecting
/// fields at each level to reach the target sub-value.
///
/// Uses `root_ty` and `variant_stack` to compute the actual type at each
/// step. This is critical for recursive enums: a `TagPayload` step into a
/// recursive field must emit the enum type (not `UNIT`/`i64`), so the LLVM
/// emitter can generate the correct pointer-deref sequence for RC-boxed fields.
fn resolve_path(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    root: ArcVarId,
    root_ty: Idx,
    path: &[PathInstruction],
    span: Span,
    variant_stack: &[(Idx, u32)],
) -> ArcVarId {
    let pool = lowerer.pool;
    let mut current = root;
    let mut current_ty = root_ty;
    let mut tag_step_idx = 0;

    for step in path {
        let (field, output_ty) = match step {
            // For enum variants, payload fields start at index 1 (index 0 is the tag).
            // Look up the actual field type from the variant context stack.
            PathInstruction::TagPayload(f) => {
                let field_ty = if tag_step_idx < variant_stack.len() {
                    let (enum_ty, variant_idx) = variant_stack[tag_step_idx];
                    lookup_variant_field_type(pool, enum_ty, variant_idx, *f)
                } else {
                    tracing::warn!(
                        tag_step = tag_step_idx,
                        stack_len = variant_stack.len(),
                        "TagPayload step has no variant context; falling back to UNIT"
                    );
                    Idx::UNIT
                };
                tag_step_idx += 1;
                (f + 1, field_ty)
            }
            PathInstruction::TupleIndex(idx) => {
                let resolved = pool.resolve_fully(current_ty);
                let elem_ty = if pool.tag(resolved) == Tag::Tuple {
                    let count = pool.tuple_elem_count(resolved);
                    if (*idx as usize) < count {
                        pool.tuple_elem(resolved, *idx as usize)
                    } else {
                        Idx::UNIT
                    }
                } else {
                    Idx::UNIT
                };
                (*idx, elem_ty)
            }
            PathInstruction::StructField(idx) => {
                let resolved = pool.resolve_fully(current_ty);
                let field_ty = if pool.tag(resolved) == Tag::Struct {
                    let count = pool.struct_field_count(resolved);
                    if (*idx as usize) < count {
                        let (_, fty) = pool.struct_field(resolved, *idx as usize);
                        fty
                    } else {
                        Idx::UNIT
                    }
                } else {
                    Idx::UNIT
                };
                (*idx, field_ty)
            }
            PathInstruction::ListElement(idx) => {
                let resolved = pool.resolve_fully(current_ty);
                let elem_ty = if pool.tag(resolved) == Tag::List {
                    pool.list_elem(resolved)
                } else {
                    Idx::UNIT
                };
                (*idx, elem_ty)
            }
            // List rest (slicing) not yet supported in LLVM backend.
            // Emit element 0 as a placeholder — will be replaced when
            // list pattern codegen is fully implemented.
            PathInstruction::ListRest(_) => (0, Idx::UNIT),
        };
        current = lowerer
            .builder
            .emit_project(output_ty, current, field, Some(span));
        current_ty = output_ty;
    }
    current
}

/// Look up the type of a field within a specific enum variant.
fn lookup_variant_field_type(
    pool: &ori_types::Pool,
    enum_type: Idx,
    variant_index: u32,
    field_index: u32,
) -> Idx {
    let resolved = pool.resolve_fully(enum_type);
    if pool.tag(resolved) != Tag::Enum {
        return Idx::UNIT;
    }
    let variants = pool.enum_variants(resolved);
    if let Some((_, fields)) = variants.get(variant_index as usize) {
        if let Some(&field_ty) = fields.get(field_index as usize) {
            return field_ty;
        }
    }
    Idx::UNIT
}

// Binding

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
            ctx.root_scrutinee_ty,
            path,
            ctx.span,
            &ctx.variant_stack,
        );
        lowerer.scope.bind(*name, var);
    }
}

// Literal emission

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
