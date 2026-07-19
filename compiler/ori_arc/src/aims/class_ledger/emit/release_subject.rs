//! Dominance-safe release subject selection.

use rustc_hash::FxHashMap as DefMap;

use crate::aims::intraprocedural::ledger_events::ClassOrigin;
use crate::graph::DominatorTree;
use crate::ir::{ArcFunction, ArcVarId};
use crate::Ownership;

use super::super::events::ClassEvents;
use super::super::placement::{collect_def_points, def_reaches_slot, DefPoint};
use super::{DeclineReason, PlanSlot, PlannedOp};

/// Dominance context for release-var selection.
pub(super) struct ReleaseCtx<'a> {
    pub(super) func: &'a ArcFunction,
    pub(super) dom: &'a DominatorTree,
    pub(super) defs: DefMap<ArcVarId, DefPoint>,
}

impl<'a> ReleaseCtx<'a> {
    pub(super) fn new(func: &'a ArcFunction, dom: &'a DominatorTree) -> Self {
        Self {
            func,
            dom,
            defs: collect_def_points(func),
        }
    }
}

/// The member variable a release names: the last resolved event var in the
/// releasing block whose definition reaches the slot, else ANY class member
/// var that reaches it — event vars first, then planned-op vars (a seeded
/// extraction inc's subject is a class member naming the same allocation
/// whose def often dominates edges a branch-local read alias cannot reach) —
/// else `UnresolvedOpVar` (fail-closed).
pub(super) fn release_var_for_slot(
    ctx: &ReleaseCtx<'_>,
    events: &ClassEvents,
    ops: &[PlannedOp],
    block: usize,
    slot: PlanSlot,
) -> Result<ArcVarId, DeclineReason> {
    #[derive(Clone, Copy)]
    enum BorrowedCreditPolicy {
        Exclude,
        IncludeOwnedCredit,
    }

    // A caller-retained borrowed param is never a release subject. A borrowed
    // ABI param whose class is Foreign is different: its contract supplied a
    // distinct whole-value credit that the callee owns. That param may be the
    // only class member dominating an early unwind edge; plan against it and
    // let application materialize the verifier-safe entry alias.
    let borrowed_credit_owned = events.origin == Some(ClassOrigin::Foreign);
    let is_borrowed_param = |var: ArcVarId| {
        ctx.func
            .params
            .iter()
            .any(|param| param.var == var && param.ownership == Ownership::Borrowed)
    };
    let resolve = |policy: BorrowedCreditPolicy| {
        let eligible = |var: ArcVarId| {
            ctx.defs
                .get(&var)
                .is_some_and(|&def| def_reaches_slot(ctx.func, ctx.dom, def, slot))
                && (!is_borrowed_param(var)
                    || (matches!(policy, BorrowedCreditPolicy::IncludeOwnedCredit)
                        && borrowed_credit_owned))
        };
        events.per_block[block]
            .iter()
            .rev()
            .filter_map(|event| event.var)
            .find(|&var| eligible(var))
            .or_else(|| {
                events
                    .per_block
                    .iter()
                    .flatten()
                    .filter_map(|event| event.var)
                    .find(|&var| eligible(var))
            })
            .or_else(|| ops.iter().map(|op| op.var).find(|&var| eligible(var)))
    };
    // Prefer an existing verifier-safe alias. Fall back to the credited ABI
    // parameter only when no such member dominates the release slot.
    let resolved = resolve(BorrowedCreditPolicy::Exclude)
        .or_else(|| resolve(BorrowedCreditPolicy::IncludeOwnedCredit));
    if resolved.is_none() {
        class_ledger_trace!(
            block,
            slot = ?slot,
            event_vars = ?events
                .per_block
                .iter()
                .flatten()
                .filter_map(|event| event.var)
                .collect::<Vec<_>>(),
            planned_vars = ?ops.iter().map(|op| op.var).collect::<Vec<_>>(),
            "release placement declined: no class member reaches the release slot"
        );
    }
    resolved.ok_or(DeclineReason::UnresolvedOpVar)
}
