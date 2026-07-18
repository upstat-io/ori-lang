//! Pre-traversal reassignment scan for `lower_match` merge-param pruning.
//!
//! `lower_match` allocates its merge block-params UPFRONT (the decision-tree
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

/// `ORI_DISABLE_MATCH_PARAM_PRUNING=1` reverts the merge-param pruning:
/// `lower_match` threads EVERY in-scope mutable binding into the merge
/// block-params (the pre-cure arrangement). Bisection surface: isolates a
/// dead-merge-param leak / wrong-post-merge-value to the pruning vs the
/// RL-5 dead-param release machinery. Spec: Annex E §AIMS RL-4 + RL-5.
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
        match *arena.kind(id) {
            // Leaf nodes — no child expressions.
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
                if let Some(name) = assign_root_name(arena, target) {
                    reassigned.insert(name);
                }
                stack.push(target);
                stack.push(value);
            }

            expr @ (CanExpr::Binary { .. }
            | CanExpr::Unary { .. }
            | CanExpr::Cast { .. }
            | CanExpr::FormatWith { .. }
            | CanExpr::Call { .. }
            | CanExpr::MethodCall { .. }
            | CanExpr::Field { .. }
            | CanExpr::Index { .. }) => push_operator_children(expr, arena, &mut stack),
            expr @ (CanExpr::If { .. }
            | CanExpr::Match { .. }
            | CanExpr::For { .. }
            | CanExpr::Loop { .. }
            | CanExpr::Break { .. }
            | CanExpr::Continue { .. }
            | CanExpr::Block { .. }
            | CanExpr::Let { .. }
            | CanExpr::Lambda { .. }) => push_control_flow_children(expr, arena, canon, &mut stack),
            expr @ (CanExpr::List(_)
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
            | CanExpr::WithCapability { .. }
            | CanExpr::FunctionExp { .. }) => push_aggregate_children(expr, arena, &mut stack),
        }
    }
    reassigned
}

fn push_operator_children(expr: CanExpr, arena: &CanArena, stack: &mut Vec<CanId>) {
    match expr {
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
        CanExpr::Index { receiver, index } => stack.extend([receiver, index]),
        _ => unreachable!("operator child dispatch received a non-operator expression"),
    }
}

fn push_control_flow_children(
    expr: CanExpr,
    arena: &CanArena,
    canon: &CanonResult,
    stack: &mut Vec<CanId>,
) {
    match expr {
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
        _ => unreachable!("control-flow child dispatch received a non-control expression"),
    }
}

fn push_aggregate_children(expr: CanExpr, arena: &CanArena, stack: &mut Vec<CanId>) {
    match expr {
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
        CanExpr::WithCapability { provider, body, .. } => stack.extend([provider, body]),
        CanExpr::FunctionExp { props, .. } => {
            stack.extend(arena.get_named_exprs(props).iter().map(|prop| prop.value));
        }
        _ => unreachable!("aggregate child dispatch received a non-aggregate expression"),
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
    #[test]
    fn match_param_pruning_toggle_reports_effect() {
        crate::test_helpers::assert_ablation_env_event(
            concat!(
                module_path!(),
                "::match_param_pruning_toggle_reports_effect"
            ),
            "ORI_DISABLE_MATCH_PARAM_PRUNING",
            "thread every in-scope mutable binding through match merges",
            super::match_param_pruning_disabled,
        );
    }
}
