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
static MATCH_PARAM_PRUNING_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_MATCH_PARAM_PRUNING").as_deref() == Ok("1"));

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

            CanExpr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            CanExpr::Unary { operand, .. } => stack.push(operand),
            CanExpr::Cast { expr, .. } => stack.push(expr),
            CanExpr::Call { func, args } => {
                stack.push(func);
                stack.extend(arena.get_expr_list(args).iter().copied());
            }
            CanExpr::MethodCall { receiver, args, .. } => {
                stack.push(receiver);
                stack.extend(arena.get_expr_list(args).iter().copied());
            }
            CanExpr::Field { receiver, .. } => stack.push(receiver),
            CanExpr::Index { receiver, index } => {
                stack.push(receiver);
                stack.push(index);
            }
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                stack.push(cond);
                stack.push(then_branch);
                stack.push(else_branch);
            }
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => {
                stack.push(scrutinee);
                stack.extend(arena.get_expr_list(arms).iter().copied());
                // Nested matches keep their guard expressions in the tree,
                // not in the arm-body list — walk those too.
                push_tree_guards(canon.decision_trees.get(decision_tree), &mut stack);
            }
            CanExpr::For {
                iter, guard, body, ..
            } => {
                stack.push(iter);
                stack.push(guard);
                stack.push(body);
            }
            CanExpr::Loop { body, .. } => stack.push(body),
            CanExpr::Break { value, .. } | CanExpr::Continue { value, .. } => stack.push(value),
            CanExpr::Block { stmts, result } => {
                stack.extend(arena.get_expr_list(stmts).iter().copied());
                stack.push(result);
            }
            CanExpr::Let { init, .. } => stack.push(init),
            CanExpr::Lambda { params, body } => {
                for param in arena.get_params(params) {
                    stack.push(param.default);
                }
                stack.push(body);
            }
            CanExpr::List(range) | CanExpr::Tuple(range) => {
                stack.extend(arena.get_expr_list(range).iter().copied());
            }
            CanExpr::Map(entries) => {
                for entry in arena.get_map_entries(entries) {
                    stack.push(entry.key);
                    stack.push(entry.value);
                }
            }
            CanExpr::Struct { fields, .. } => {
                for field in arena.get_fields(fields) {
                    stack.push(field.value);
                }
            }
            CanExpr::Range {
                start, end, step, ..
            } => {
                stack.push(start);
                stack.push(end);
                stack.push(step);
            }
            CanExpr::Ok(inner)
            | CanExpr::Err(inner)
            | CanExpr::Some(inner)
            | CanExpr::Try(inner)
            | CanExpr::Await(inner)
            | CanExpr::Unsafe(inner) => stack.push(inner),
            CanExpr::WithCapability { provider, body, .. } => {
                stack.push(provider);
                stack.push(body);
            }
            CanExpr::FunctionExp { props, .. } => {
                for prop in arena.get_named_exprs(props) {
                    stack.push(prop.value);
                }
            }
            CanExpr::FormatWith { expr, .. } => stack.push(expr),
        }
    }
    reassigned
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
