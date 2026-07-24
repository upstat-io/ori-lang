//! Emit ARC IR basic blocks from a compiled [`DecisionTree`].
//!
//! `ori_canon` flattens `MatchPattern` into `FlatPattern` and compiles the
//! pattern matrix into a `DecisionTree`; this module walks that tree and
//! emits ARC IR blocks with `Switch`/`Branch` terminators.
//!
//! The emission is performed by [`emit_tree`], called from match lowering.

mod path;
#[cfg(test)]
mod tests;

use ori_ir::canon::LeafDiscardPaths;
use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::ArcVarId;
use crate::lower::scope::ArcScope;

use super::{DecisionTree, ScrutineePath};

pub(super) use path::resolve_path;

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

// Why: Scope bindings stay opaque; traversal coordinates make diagnostics stable.
impl std::fmt::Debug for EmitContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmitContext")
            .field("root_scrutinee", &self.root_scrutinee)
            .field("root_scrutinee_ty", &self.root_scrutinee_ty)
            .field("merge_block", &self.merge_block)
            .field("arm_body_count", &self.arm_bodies.len())
            .field("next_success_index", &self.next_success_index)
            .field("variant_depth", &self.variant_stack.len())
            .finish_non_exhaustive()
    }
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

// Why: Initialization diagnostics expose coordinates without formatting bindings.
impl std::fmt::Debug for EmitContextInit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmitContextInit")
            .field("root_scrutinee", &self.root_scrutinee)
            .field("root_scrutinee_ty", &self.root_scrutinee_ty)
            .field("merge_block", &self.merge_block)
            .field("arm_body_count", &self.arm_bodies.len())
            .field("discard_path_count", &self.leaf_discard_paths.len())
            .finish_non_exhaustive()
    }
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
            .unwrap_or_else(|| {
                panic!(
                    "decision-tree success node {} has no matching cleanup carrier",
                    self.next_success_index
                )
            });
        self.next_success_index += 1;
        paths
    }
}

/// Emit a canonical decision tree as ARC blocks and terminators.
///
/// Switches resolve scrutinee paths, leaves bind variables before lowering the
/// arm, guards branch to the body or failure subtree, and impossible failures
/// terminate as unreachable. String tests use branch chains because ARC has no
/// string-valued switch terminator.
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
