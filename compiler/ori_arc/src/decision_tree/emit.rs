//! Emit ARC IR basic blocks from a compiled [`DecisionTree`].
//!
//! `ori_canon` flattens `MatchPattern` into `FlatPattern` and compiles the
//! pattern matrix into a `DecisionTree`; this module walks that tree and
//! emits ARC IR blocks with `Switch`/`Branch` terminators.
//!
//! The emission is performed by [`emit_decision_tree`], called from
//! `lower_match` in `lower/control_flow.rs`.

use ori_ir::canon::LeafDiscardPaths;
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::ArcVarId;
use crate::lower::collections::{emit_list_element, emit_list_rest_slice};
use crate::lower::scope::ArcScope;

use super::{DecisionTree, PathInstruction, ScrutineePath};

/// Context for decision tree emission.
///
/// Holds references to the arms' body expressions and the merge block
/// where all arms converge. Also carries SSA merge info for mutable
/// variables that may be reassigned in arm bodies.
pub(crate) struct EmitContext {
    /// The root scrutinee variable.
    pub(crate) root_scrutinee: ArcVarId,
    /// Pool type of the root scrutinee (for type-aware path resolution).
    pub(crate) root_scrutinee_ty: Idx,
    /// The merge block all arms jump to after executing their body.
    pub(crate) merge_block: crate::ir::ArcBlockId,
    /// The body expression for each arm (indexed by `arm_index`).
    pub(crate) arm_bodies: Vec<ori_ir::canon::CanId>,
    /// Span of the match expression.
    pub(crate) span: Span,
    /// Scope snapshot from before the match. Each arm resets to this
    /// before lowering its body, ensuring arm-to-arm scope isolation.
    pub(crate) pre_scope: ArcScope,
    /// Mutable variable names to pass as merge block params (in order).
    /// These serve as SSA phi inputs at the match convergence point.
    pub(crate) mutable_var_names: Vec<Name>,
    /// Blank-pattern cleanup carriers in static Leaf/Guard success order.
    leaf_discard_paths: Vec<LeafDiscardPaths>,
    /// Next success node in the canonical preorder side table.
    next_success_index: usize,
    /// Variant context stack for type-aware path resolution.
    ///
    /// Each entry is `(enum_type, variant_index)` pushed by `emit_tag_switch`
    /// when entering a specific variant's case block. `resolve_path` uses this
    /// to look up the actual field type for `TagPayload` steps, which is
    /// critical for recursive enums where fields may be RC-boxed pointers.
    variant_stack: Vec<(Idx, u32)>,
}

pub(crate) struct EmitContextInit {
    pub(crate) root_scrutinee: ArcVarId,
    pub(crate) root_scrutinee_ty: Idx,
    pub(crate) merge_block: crate::ir::ArcBlockId,
    pub(crate) arm_bodies: Vec<ori_ir::canon::CanId>,
    pub(crate) span: Span,
    pub(crate) pre_scope: ArcScope,
    pub(crate) mutable_var_names: Vec<Name>,
    pub(crate) leaf_discard_paths: Vec<LeafDiscardPaths>,
}

impl EmitContext {
    /// Create a new emit context for decision tree emission.
    pub(crate) fn new(init: EmitContextInit) -> Self {
        Self {
            root_scrutinee: init.root_scrutinee,
            root_scrutinee_ty: init.root_scrutinee_ty,
            merge_block: init.merge_block,
            arm_bodies: init.arm_bodies,
            span: init.span,
            pre_scope: init.pre_scope,
            mutable_var_names: init.mutable_var_names,
            leaf_discard_paths: init.leaf_discard_paths,
            next_success_index: 0,
            variant_stack: Vec::new(),
        }
    }

    /// Push a variant onto the context stack (entering a variant's case block).
    pub(crate) fn push_variant(&mut self, enum_type: Idx, variant_index: u32) {
        self.variant_stack.push((enum_type, variant_index));
    }

    /// Pop the most recent variant from the context stack (leaving a variant's case block).
    pub(crate) fn pop_variant(&mut self) {
        self.variant_stack.pop();
    }

    /// Get the current variant stack (for path resolution).
    pub(crate) fn variant_stack(&self) -> &[(Idx, u32)] {
        &self.variant_stack
    }

    /// Whether any success node carries an explicit blank-pattern discard.
    pub(crate) fn has_discard_obligations(&self) -> bool {
        self.leaf_discard_paths
            .iter()
            .any(|paths| !paths.is_empty())
    }

    /// Advance to the cleanup carriers for the next static success node.
    fn take_next_discard_paths(&mut self) -> LeafDiscardPaths {
        let paths = self
            .leaf_discard_paths
            .get(self.next_success_index)
            .cloned()
            .unwrap_or_default();
        self.next_success_index += 1;
        paths
    }
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
        } => super::emit_switches::emit_switch(
            lowerer,
            path,
            *test_kind,
            edges,
            default.as_deref(),
            ctx,
        ),

        DecisionTree::Leaf {
            arm_index,
            bindings,
        } => {
            let discard_paths = ctx.take_next_discard_paths();
            emit_leaf(lowerer, *arm_index, bindings, &discard_paths, ctx);
        }

        DecisionTree::Guard {
            arm_index,
            bindings,
            guard,
            on_fail,
        } => {
            let discard_paths = ctx.take_next_discard_paths();
            emit_guard(
                lowerer,
                *arm_index,
                bindings,
                &discard_paths,
                *guard,
                on_fail,
                ctx,
            );
        }

        DecisionTree::Fail => {
            lowerer.builder.terminate_unreachable();
        }
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
    discard_paths: &[ScrutineePath],
    ctx: &mut EmitContext,
) {
    // Reset scope to pre-match snapshot for arm isolation.
    lowerer.scope = ctx.pre_scope.clone();

    // Bind pattern variables by resolving paths from root scrutinee.
    bind_pattern_variables(lowerer, bindings, ctx);
    emit_discard_paths(lowerer, discard_paths, ctx);

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
    discard_paths: &[ScrutineePath],
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
    emit_discard_paths(lowerer, discard_paths, ctx);
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
pub(super) fn resolve_path(
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
                current = emit_list_element(
                    lowerer.builder,
                    lowerer.interner,
                    current,
                    *idx,
                    elem_ty,
                    Some(span),
                );
                current_ty = elem_ty;
                continue;
            }
            PathInstruction::ListRest(start_idx) => {
                let list_ty = current_ty;
                current = emit_list_rest_slice(
                    lowerer.builder,
                    lowerer.interner,
                    current,
                    *start_idx,
                    list_ty,
                    Some(span),
                );
                current_ty = list_ty;
                continue;
            }
        };
        current = lowerer
            .builder
            .emit_project(output_ty, current, field, Some(span));
        current_ty = output_ty;
    }
    current
}

/// Look up the type of a field within a specific variant of an enum, Option, or Result.
fn lookup_variant_field_type(
    pool: &ori_types::Pool,
    enum_type: Idx,
    variant_index: u32,
    field_index: u32,
) -> Idx {
    let resolved = pool.resolve_fully(enum_type);
    match pool.tag(resolved) {
        Tag::Enum => {
            let variants = pool.enum_variants(resolved);
            if let Some((_, fields)) = variants.get(variant_index as usize) {
                if let Some(&field_ty) = fields.get(field_index as usize) {
                    return field_ty;
                }
            }
        }
        Tag::Option => {
            // Some (index 0) has one field (the inner type), None (index 1) has none.
            if variant_index == 0 && field_index == 0 {
                return pool.option_inner(resolved);
            }
        }
        Tag::Result => {
            // Ok (index 0) has one field (ok type), Err (index 1) has one field (err type).
            if field_index == 0 {
                return if variant_index == 0 {
                    pool.result_ok(resolved)
                } else {
                    pool.result_err(resolved)
                };
            }
        }
        _ => {}
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
            ctx.variant_stack(),
        );
        lowerer.scope.bind(*name, var);
    }
}

/// Materialize blank-pattern field paths as dead projections.
///
/// A dead `Project` is the backend-neutral carrier that lets AIMS attach the
/// discarded field's cleanup identity. LLVM consumes the resulting ARC IR; it
/// does not rediscover wildcard ownership from types or source patterns.
fn emit_discard_paths(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    paths: &[ScrutineePath],
    ctx: &EmitContext,
) {
    for path in paths {
        resolve_path(
            lowerer,
            ctx.root_scrutinee,
            ctx.root_scrutinee_ty,
            path,
            ctx.span,
            ctx.variant_stack(),
        );
    }
}
