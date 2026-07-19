//! Pre-traversal reassignment scan for `lower_match` merge-param pruning.
//!
//! `lower_match` allocates its merge block parameters before arm lowering (the decision-tree
//! `EmitContext` needs the merge signature before any arm lowers), so the
//! post-hoc `merge_mutable_vars` divergence filter `lower_if` uses cannot
//! apply. This scan pre-traverses the arm bodies plus every decision-tree
//! guard expression and collects the mutable names a `CanExpr::Assign`
//! could rebind. A binding outside the set provably cannot diverge in any
//! arm (`scope.lookup(name) != Some(pre_var)` requires an `Assign`), so its
//! merge param is pruned — the same divergence semantics as
//! `merge_mutable_vars`, applied before param creation.
//!
//! Conservative by construction: a shadowed reassignment, an assignment
//! inside a nested lambda, or a `Field`/`Index` assignment target all KEEP
//! the param (no pruning) — over-collection reproduces the unpruned
//! behavior; only a provably-absent assignment prunes.

use std::sync::LazyLock;

use ori_ir::canon::{CanArena, CanExpr, CanId, CanonResult, DecisionTree};
use ori_ir::Name;
use rustc_hash::FxHashSet;

/// `ORI_DISABLE_MATCH_PARAM_PRUNING=1` disables merge-param pruning:
/// `lower_match` threads every in-scope mutable binding into the merge block
/// parameters. The toggle isolates merge pruning from RL-5 dead-parameter
/// release behavior (Spec: Annex E §AIMS RL-4 + RL-5).
// Env: ORI_DISABLE_MATCH_PARAM_PRUNING - disables match merge-param pruning, debug-only.
static MATCH_PARAM_PRUNING_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    report_match_param_pruning_toggle(
        std::env::var("ORI_DISABLE_MATCH_PARAM_PRUNING").as_deref() == Ok("1"),
    )
});

fn report_match_param_pruning_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_MATCH_PARAM_PRUNING",
            effect = "thread every in-scope mutable binding through match merges",
            "ablation toggle fired"
        );
    }
    disabled
}

/// Whether the `lower_match` merge-param pruning is disabled
/// (`ORI_DISABLE_MATCH_PARAM_PRUNING=1`).
pub(super) fn match_param_pruning_disabled() -> bool {
    *MATCH_PARAM_PRUNING_DISABLED
}

/// Collect every name a `CanExpr::Assign` under the match's arm bodies (or
/// decision-tree guard expressions, including nested matches' guards) could
/// rebind. Mutable bindings OUTSIDE the returned set cannot diverge in any
/// arm and are pruned from the merge block-params.
pub(super) fn collect_reassigned_mutable_names(
    arena: &CanArena,
    canon: &CanonResult,
    arm_ids: &[CanId],
    tree: &DecisionTree,
) -> FxHashSet<Name> {
    let mut reassigned = FxHashSet::default();
    let mut stack: Vec<CanId> = arm_ids.to_vec();
    push_tree_guards(tree, &mut stack);
    let mut visited: FxHashSet<CanId> = FxHashSet::default();

    while let Some(id) = stack.pop() {
        if !id.is_valid() || !visited.insert(id) {
            continue;
        }
        visit_expr(arena, canon, id, &mut stack, &mut reassigned);
    }
    reassigned
}

fn visit_expr(
    arena: &CanArena,
    canon: &CanonResult,
    id: CanId,
    stack: &mut Vec<CanId>,
    reassigned: &mut FxHashSet<Name>,
) {
    let kind = *arena.kind(id);
    match kind {
        CanExpr::Int(_)
        | CanExpr::Float(_)
        | CanExpr::Bool(_)
        | CanExpr::Str(_)
        | CanExpr::Char(_)
        | CanExpr::Duration { .. }
        | CanExpr::Size { .. }
        | CanExpr::Unit
        | CanExpr::Constant(_)
        | CanExpr::Ident(_)
        | CanExpr::Const(_)
        | CanExpr::SelfRef
        | CanExpr::FunctionRef(_)
        | CanExpr::TypeRef(_)
        | CanExpr::HashLength
        | CanExpr::None
        | CanExpr::Error => {}
        CanExpr::Assign { target, value } => {
            visit_assignment(arena, target, value, stack, reassigned);
        }
        CanExpr::Binary { .. }
        | CanExpr::Unary { .. }
        | CanExpr::Cast { .. }
        | CanExpr::FormatWith { .. }
        | CanExpr::Call { .. }
        | CanExpr::MethodCall { .. }
        | CanExpr::Field { .. }
        | CanExpr::Index { .. } => push_simple_children(arena, kind, stack),
        CanExpr::If { .. }
        | CanExpr::Match { .. }
        | CanExpr::For { .. }
        | CanExpr::Loop { .. }
        | CanExpr::Break { .. }
        | CanExpr::Continue { .. }
        | CanExpr::Block { .. }
        | CanExpr::Let { .. }
        | CanExpr::Lambda { .. }
        | CanExpr::WithCapability { .. } => push_control_children(arena, canon, kind, stack),
        CanExpr::List(_)
        | CanExpr::Tuple(_)
        | CanExpr::Map(_)
        | CanExpr::Struct { .. }
        | CanExpr::Range { .. }
        | CanExpr::Ok(_)
        | CanExpr::Err(_)
        | CanExpr::Some(_)
        | CanExpr::Try(_)
        | CanExpr::Await(_)
        | CanExpr::Unsafe(_)
        | CanExpr::FunctionExp { .. } => push_container_children(arena, kind, stack),
    }
}

fn visit_assignment(
    arena: &CanArena,
    target: CanId,
    value: CanId,
    stack: &mut Vec<CanId>,
    reassigned: &mut FxHashSet<Name>,
) {
    if let Some(name) = assign_root_name(arena, target) {
        reassigned.insert(name);
    }
    stack.extend([target, value]);
}

fn push_simple_children(arena: &CanArena, kind: CanExpr, stack: &mut Vec<CanId>) {
    match kind {
        CanExpr::Binary { left, right, .. } => stack.extend([left, right]),
        CanExpr::Unary { operand, .. } => stack.push(operand),
        CanExpr::Cast { expr, .. } | CanExpr::FormatWith { expr, .. } => stack.push(expr),
        CanExpr::Call { func, args } => {
            stack.push(func);
            stack.extend(arena.get_expr_list(args).iter().copied());
        }
        CanExpr::MethodCall { receiver, args, .. } => {
            stack.push(receiver);
            stack.extend(arena.get_expr_list(args).iter().copied());
        }
        CanExpr::Field { receiver, .. } => stack.push(receiver),
        CanExpr::Index {
            receiver, index, ..
        } => stack.extend([receiver, index]),
        _ => unreachable!("push_simple_children called with non-simple expression"),
    }
}

fn push_control_children(
    arena: &CanArena,
    canon: &CanonResult,
    kind: CanExpr,
    stack: &mut Vec<CanId>,
) {
    match kind {
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => stack.extend([cond, then_branch, else_branch]),
        CanExpr::Match {
            scrutinee,
            decision_tree,
            arms,
        } => {
            stack.push(scrutinee);
            stack.extend(arena.get_expr_list(arms).iter().copied());
            push_tree_guards(canon.decision_trees.get(decision_tree), stack);
        }
        CanExpr::For {
            iter, guard, body, ..
        } => stack.extend([iter, guard, body]),
        CanExpr::Loop { body, .. } => stack.push(body),
        CanExpr::Break { value, .. } | CanExpr::Continue { value, .. } => stack.push(value),
        CanExpr::Block { stmts, result } => {
            stack.extend(arena.get_expr_list(stmts).iter().copied());
            stack.push(result);
        }
        CanExpr::Let { init, .. } => stack.push(init),
        CanExpr::Lambda { params, body } => {
            stack.extend(arena.get_params(params).iter().map(|param| param.default));
            stack.push(body);
        }
        CanExpr::WithCapability { provider, body, .. } => stack.extend([provider, body]),
        _ => unreachable!("push_control_children called with non-control expression"),
    }
}

fn push_container_children(arena: &CanArena, kind: CanExpr, stack: &mut Vec<CanId>) {
    match kind {
        CanExpr::List(range) | CanExpr::Tuple(range) => {
            stack.extend(arena.get_expr_list(range).iter().copied());
        }
        CanExpr::Map(entries) => {
            for entry in arena.get_map_entries(entries) {
                stack.extend([entry.key, entry.value]);
            }
        }
        CanExpr::Struct { fields, .. } => {
            stack.extend(arena.get_fields(fields).iter().map(|field| field.value));
        }
        CanExpr::Range {
            start, end, step, ..
        } => stack.extend([start, end, step]),
        CanExpr::Ok(inner)
        | CanExpr::Err(inner)
        | CanExpr::Some(inner)
        | CanExpr::Try(inner)
        | CanExpr::Await(inner)
        | CanExpr::Unsafe(inner) => stack.push(inner),
        CanExpr::FunctionExp { props, .. } => {
            stack.extend(arena.get_named_exprs(props).iter().map(|prop| prop.value));
        }
        _ => unreachable!("push_container_children called with non-container expression"),
    }
}

/// Root identifier of an assignment target chain: `x` for `x = v`,
/// `x.f = v`, and `x[i] = v`. `None` for non-identifier roots (the caller
/// keeps the conservative no-prune behavior via the target/value traversal).
fn assign_root_name(arena: &CanArena, mut target: CanId) -> Option<Name> {
    loop {
        if !target.is_valid() {
            return Option::None;
        }
        match *arena.kind(target) {
            CanExpr::Ident(name) => return Some(name),
            CanExpr::Field { receiver, .. } | CanExpr::Index { receiver, .. } => {
                target = receiver;
            }
            _ => return Option::None,
        }
    }
}

/// Push every guard expression reachable in `tree` (guards live in the
/// decision tree, not the arm-body list).
fn push_tree_guards(tree: &DecisionTree, stack: &mut Vec<CanId>) {
    let mut nodes: Vec<&DecisionTree> = vec![tree];
    while let Some(node) = nodes.pop() {
        match node {
            DecisionTree::Switch { edges, default, .. } => {
                for (_, sub) in edges {
                    nodes.push(sub);
                }
                if let Some(sub) = default {
                    nodes.push(sub);
                }
            }
            DecisionTree::Guard { guard, on_fail, .. } => {
                stack.push(*guard);
                nodes.push(on_fail);
            }
            DecisionTree::Leaf { .. } | DecisionTree::Fail => {}
        }
    }
}

#[cfg(test)]
mod toggle_tests {
    crate::test_helpers::ablation_env_event_test!(
        match_param_pruning_toggle_reports_effect,
        "ORI_DISABLE_MATCH_PARAM_PRUNING",
        "thread every in-scope mutable binding through match merges",
        super::match_param_pruning_disabled,
    );
}
