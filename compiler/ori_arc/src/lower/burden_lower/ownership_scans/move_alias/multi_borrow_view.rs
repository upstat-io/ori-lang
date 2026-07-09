//! N-alias generalization of [`super::surplus_dec`]'s single-alias
//! borrow-view-dst keystone: [`compute_multi_borrow_view_alias_surplus`].

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::surplus_dec::alias_use_is_borrow_view_project_only;

/// Multi-borrow-view-alias surplus suppression (RL-2 release-once + TF-4 + DP-3).
/// The N-alias generalization of the single-alias
/// [`super::surplus_dec`] keystone: a fresh `Construct`
/// owner `%s` consumed ONLY through >= 2 same-allocation whole-var `Let { Var }`
/// aliases (`%6 = %s`, `%11 = %s`), each of which is a borrow-view (not owned-RC,
/// same `genuine_same_alloc_reps` rep) projected for a borrow-read
/// (`Project %6.0`, `Project %11.1`). The base walk emits a surplus whole-var
/// `BurdenDec` at EACH alias's last use AND keeps `%s`'s own scope-exit /
/// edge-cleanup dec — N+1 releases of ONE allocation (cleanup-on: the
/// redundant-cleanup pass over-strips toward a leak; cleanup-off: the surplus
/// decs over-release → double-free). It ALSO keeps `%s`'s spurious keep-alive
/// FRESH inc (the `use_counts >= 2` proxy mis-classes the borrow-view aliases as
/// duplication).
///
/// The proven correction (`RL2_release_exactly_once`): one allocation, exactly
/// one release. `%s`'s born-at-alloc reference (+1) is released by `%s`'s
/// surviving scope-exit / edge-cleanup dec (exactly one per CFG path); each
/// alias's whole-var dec is the SURPLUS same-allocation release — suppress it.
/// The keep-alive FRESH inc is spurious (the lineage is borrow-read-only, no
/// duplication, DP-3 `is_rc_inc_elidable`) — suppress it. Returns
/// `(alias_dec_suppress, owner_inc_suppress)`: the alias dsts whose surplus decs
/// are suppressed (marked `transfer_via_move_alias`) and the owners whose
/// keep-alive inc is suppressed (marked `inc_suppressed_vars`).
///
/// SAME-ALLOCATION-IDENTITY discriminators bound the family (NOT a use-count /
/// type-membership proxy — each is a structural property of the lineage):
///
/// - **fresh `Construct` owner in `owned_vars_needing_rc`, non-param**: the
///   allocation born here carries the single release.
/// - **EVERY direct use is a same-allocation whole-var borrow-view alias**: each
///   direct use is a `Let { Var(%s) }` whose dst is NOT owned-RC (a borrow-view)
///   and shares `%s`'s `genuine_same_alloc_reps` rep. A use-once MOVE alias (one
///   alias, owned dst) is the single-alias keystone's territory; an owned-consume
///   (Construct arg / `[own]` call arg / `Set` / Return) declines (the consume IS
///   the transfer — `%s`'s ownership leaves, not a borrow).
/// - **>= 2 such aliases**: the single-alias case is the existing keystone arm;
///   this arm owns the multi-alias generalization the keystone's
///   `use_counts(%s) == 1` gate declines.
/// - **whole same-allocation lineage is borrow-read-only**: no member (alias OR
///   projected field) is owned-consumed (`owned_consumed`). A field moved out at
///   an owned position keeps the owner's drop load-bearing — declined.
pub(in crate::lower::burden_lower) fn compute_multi_borrow_view_alias_surplus(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    owned_consumed: &FxHashSet<ArcVarId>,
    genuine_same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> (FxHashSet<ArcVarId>, FxHashSet<ArcVarId>) {
    let mut alias_dec_suppress: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut owner_inc_suppress: FxHashSet<ArcVarId> = FxHashSet::default();
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    // Forward whole-var alias edges + projected-field edges (the same-allocation
    // flow), and per-owner direct-use lists.
    let mut whole_var_aliases: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut flow_dsts: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut direct_uses: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    whole_var_aliases.entry(*src).or_default().push(*dst);
                    flow_dsts.entry(*src).or_default().push(*dst);
                    direct_uses.entry(*src).or_default().push(*dst);
                }
                ArcInstr::Project { dst, value, .. } => {
                    flow_dsts.entry(*value).or_default().push(*dst);
                    direct_uses.entry(*value).or_default().push(*dst);
                }
                _ => {}
            }
        }
    }

    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Construct { dst: owner, .. } = instr else {
                continue;
            };
            let owner = *owner;
            // Owner is a fresh owned-RC non-param allocation.
            if !owned_vars_needing_rc.contains(&owner) || param_vars.contains(&owner) {
                continue;
            }
            let Some(uses) = direct_uses.get(&owner) else {
                continue;
            };
            // EVERY direct use is a same-allocation whole-var `Let { Var }` alias
            // whose own sole downstream use is a `Project` borrow-read, and there
            // are >= 2 of them (the single-alias case is the keystone). Each alias
            // is the same allocation as the owner (`genuine_same_alloc_reps` rep,
            // unwrap-or-self: the owner is its own rep, never a key). Whether the
            // alias is tracked owned-RC or not, its whole-var dec at the borrow
            // point is the surplus same-allocation release.
            let owner_rep = genuine_same_alloc_reps
                .get(&owner)
                .copied()
                .unwrap_or(owner);
            let mut alias_count = 0usize;
            let mut all_borrow_view_aliases = true;
            for &u in uses {
                let is_whole_var_alias = whole_var_aliases
                    .get(&owner)
                    .is_some_and(|a| a.contains(&u));
                let same_alloc = genuine_same_alloc_reps.get(&u).copied().unwrap_or(u) == owner_rep;
                // The alias's own sole use is a `Project` borrow-read (it never
                // consumes the aggregate at an owned position itself).
                let alias_borrow_projects_only =
                    alias_use_is_borrow_view_project_only(func, u, owned_vars_needing_rc);
                if is_whole_var_alias && same_alloc && alias_borrow_projects_only {
                    alias_count += 1;
                } else {
                    all_borrow_view_aliases = false;
                    break;
                }
            }
            if !all_borrow_view_aliases || alias_count < 2 {
                continue;
            }
            // The whole same-allocation lineage is borrow-read-only: no member
            // (alias OR projected field) is owned-consumed. A field moved out at
            // an owned position keeps the owner's drop load-bearing — decline.
            let mut stack = vec![owner];
            let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
            seen.insert(owner);
            let mut borrow_only = true;
            while let Some(v) = stack.pop() {
                // The owner itself is the released root, never an "owned consume"
                // for this gate; only its descendants count.
                if v != owner && owned_consumed.contains(&v) {
                    borrow_only = false;
                    break;
                }
                if let Some(children) = flow_dsts.get(&v) {
                    for &child in children {
                        if seen.insert(child) {
                            stack.push(child);
                        }
                    }
                }
            }
            if !borrow_only {
                continue;
            }
            tracing::trace!(
                target: "ori_arc::lower::burden_lower",
                owner = owner.index(),
                alias_count,
                "multi-borrow-view-alias surplus: owner keep-alive inc + N alias \
                 decs suppressed (RL-2 release-once over N borrow-view aliases)"
            );
            owner_inc_suppress.insert(owner);
            for &u in uses {
                alias_dec_suppress.insert(u);
            }
        }
    }
    (alias_dec_suppress, owner_inc_suppress)
}
