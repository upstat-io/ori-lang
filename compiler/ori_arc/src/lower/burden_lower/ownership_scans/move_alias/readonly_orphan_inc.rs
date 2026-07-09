//! DP-3 + RL-1 + RL-2 read-only-borrow orphan-inc suppression:
//! [`compute_readonly_borrow_orphan_inc_suppression`].

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

/// DP-3 + RL-1 + RL-2 read-only-borrow orphan-inc suppression. A fresh
/// `Construct` dst whose FRESH-site `BurdenInc` is ORPHANED — its scope-exit dec
/// is transfer-suppressed (`transferred`, via the owned-RC-dst move-edge seed)
/// AND its every direct use is a `Let { Var }` DUP-alias (`dup_alias_dsts`: the
/// source stays live, `use_count >= 2`, so each alias carries its OWN paired
/// FRESH inc + last-use dec and self-balances) — has NO paired dec for its root
/// inc and needs none. Per DP-3 (`is_rc_inc_elidable`: `Once ∧ Affine` borrow →
/// the duplicate-inc is elidable, the upstream alias entries DECOUPLED;
/// `AimsProof.Decision::DP3_is_rc_inc_elidable_table` admits the `Affine` row)
/// the root construct's FRESH inc is the orphan (VF-1 net=+1 leak). Suppressing
/// it restores RL-2 single-release balance
/// (`AimsProof.Realization::RL2_dec_at_last_use`).
///
/// TWO same-allocation-identity discriminators bound the family (NOT a use-count
/// proxy — each is a structural property of the construct's lineage):
///
/// - **every-use-is-dup-alias**: EXCLUDES the use-once MOVE case
///   (`let h = Holder { kept: s }`: `%30 -> %38` where `%38` carries the
///   move-dec that PAIRS with `%30`'s FRESH inc — not an orphan). A use-once
///   move dst is not in `dup_alias_dsts`, so the gate declines and the balanced
///   FRESH inc + move-dec pair is preserved.
/// - **Project-aware read-only-borrow-only closure**: EXCLUDES the
///   struct-self-rebuild field-move-out (`r = Pair { a: r.a, b: r.b }`: the heap
///   field `Project r.a` is owned-consumed at the rebuild `Construct`, so the
///   `Project` dst is in `owned_consumed` and the gate declines — the kept inc
///   stays load-bearing per `RL1_duplication_balanced`). The closure follows
///   BOTH `Let { Var }` alias edges AND `Project` field-extraction edges, so a
///   heap field moved out of the lineage at an owned position is caught.
///
/// Spec: Annex E §AIMS DP-3 + RL-1 + RL-2.
pub(in crate::lower::burden_lower) fn compute_readonly_borrow_orphan_inc_suppression(
    func: &ArcFunction,
    transferred: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    owned_consumed: &FxHashSet<ArcVarId>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    // Forward same-allocation flow edges: a `Let { Var }` alias OR a `Project`
    // field-extraction (the projected value names the same heap object's
    // sub-allocation). src -> [dst, ...].
    let mut flow_dsts: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    // Per-var direct uses, to enforce every-use-is-dup-alias.
    let mut direct_uses: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    flow_dsts.entry(*src).or_default().push(*dst);
                    direct_uses.entry(*src).or_default().push(*dst);
                }
                ArcInstr::Project { dst, value, .. } => {
                    flow_dsts.entry(*value).or_default().push(*dst);
                }
                _ => {}
            }
        }
    }
    let mut result: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Construct { dst, args, .. } = instr else {
                continue;
            };
            // Act only on a transfer-suppressed (orphaned) owned-RC construct.
            if !owned_vars_needing_rc.contains(dst) || !transferred.contains(dst) {
                continue;
            }
            // LEAF-allocation gate: the construct holds NO owned-RC argument, so
            // it is a single leaf heap object with no heap sub-allocations to
            // recursively drop (a scalar-element collection buffer like `[int]`).
            // EXCLUDES recursive-drop aggregates — a struct/tuple holding a heap
            // field, OR a heap-element collection (`[str]` / `[[int]]`) — whose
            // FRESH inc is BALANCED by its own drop-dec (the drop recursively
            // frees the children), so suppressing it over-releases the children
            // (the `narrowed_list_derived_eq` Container / `Holder { kept: s }`
            // shapes). Same-allocation-identity based (leaf vs recursive-owner),
            // NOT a type proxy.
            if args.iter().any(|a| owned_vars_needing_rc.contains(a)) {
                continue;
            }
            // every-use-is-dup-alias: EVERY direct use of the root is a
            // `Let { Var }` dup-alias (`dup_alias_dsts`). A use-once move dst (not
            // a dup) carries an UNPAIRED move-dec that pairs with the root inc —
            // not an orphan; decline.
            let root_uses = direct_uses.get(dst);
            let every_use_dup_alias = root_uses.is_some_and(|uses| {
                !uses.is_empty() && uses.iter().all(|u| dup_alias_dsts.contains(u))
            });
            if !every_use_dup_alias {
                continue;
            }
            // Project-aware read-only-borrow-only closure: NO same-allocation
            // member (alias OR projected field) is owned-consumed.
            let mut stack = vec![*dst];
            let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
            seen.insert(*dst);
            let mut borrow_only = true;
            while let Some(v) = stack.pop() {
                if owned_consumed.contains(&v) {
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
            if borrow_only {
                tracing::trace!(
                    target: "ori_arc::lower::burden_lower",
                    root = dst.index(),
                    "readonly-borrow orphan-inc suppression: fresh Construct \
                     fresh-site BurdenInc suppressed (DP-3 borrow lineage)"
                );
                result.insert(*dst);
            }
        }
    }
    result
}
