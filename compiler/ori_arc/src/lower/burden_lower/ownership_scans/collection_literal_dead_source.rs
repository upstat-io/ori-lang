//! RL-1 collection-literal dead-source keep-alive suppression.
//!
//! A fresh owned aggregate consumed at >= 2 collection-literal `Construct`
//! (`ListLiteral` / `MapLiteral` / `SetLiteral`) arg positions via `Let { Var }`
//! aliases, dead after the literal, owes `(N-1)` duplication incs + 1 transfer
//! (the last-use alias MOVES the source's ref into the Nth slot;
//! `RL1_duplication_balanced`). The base walk emits a spurious fresh-site
//! keep-alive `BurdenInc` on the SOURCE on top of the `(N-1)` dup-alias incs
//! and never releases the source's own ref -> +1 leak. This scan extends the
//! RL-2 transfer-suppression symmetry (`scan_orchestration` `inc_suppressed_vars
//! .extend(full_move_vars)`) to the one-hop-alias-into-collection-literal case:
//! the source is transferred at its last use through a `Let { Var }` alias
//! consumed at the collection-literal `Construct`, so its fresh-site inc is
//! suppressed. The `(N-1)` dup-alias incs are KEPT (each funds its slot).
//! Spec: Annex E §AIMS RL-1 + RL-2 (`RL1_duplication_balanced`).

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, CtorKind};

fn is_collection_literal_ctor(ctor: &CtorKind) -> bool {
    matches!(
        ctor,
        CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
    )
}

/// `ORI_DISABLE_COLLECTION_LITERAL_DEAD_SOURCE_SUPPRESS=1` restores the spurious
/// source keep-alive inc (the pre-cure +1 leak) for bisection.
fn disabled() -> bool {
    std::env::var_os("ORI_DISABLE_COLLECTION_LITERAL_DEAD_SOURCE_SUPPRESS").is_some()
}

/// Compute the set of fresh owned aggregate SOURCES whose fresh-site keep-alive
/// `BurdenInc` must be suppressed because the source is fully consumed by
/// collection-literal `Construct` arg aliases (every use is a `Let { Var }`
/// alias whose sole consume is an owned arg of ONE collection-literal
/// `Construct`, with >= 2 such aliases). A source with ANY other use (a borrow
/// `Project`, a non-collection consume, a use past the literal) is LIVE-after
/// and DECLINES — its keep-alive inc is load-bearing (the over-suppression
/// clamp). The returned set is merged into `inc_suppressed_vars`.
pub(in crate::lower::burden_lower) fn compute_collection_literal_dead_source_suppression(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    if disabled() {
        return FxHashSet::default();
    }

    // alias_dst -> (source var, owning collection-literal Construct dst) for every
    // `Let { Var(src) }` alias whose SOLE consume is an owned arg of a
    // collection-literal `Construct`. An alias consumed anywhere else (or unused)
    // is absent here, which forces its source to DECLINE (a non-collection use).
    let mut alias_to_source: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                alias_to_source.insert(*dst, *src);
            }
        }
    }

    // For each collection-literal Construct, record which of its args are aliases
    // (mapped above) and group by source.
    // source -> (set of distinct alias args feeding ONE collection-literal Construct,
    //            the Construct dst).
    let mut source_literal_aliases: FxHashMap<ArcVarId, (FxHashSet<ArcVarId>, Option<ArcVarId>)> =
        FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Construct {
                dst: construct_dst,
                ctor,
                args,
                ..
            } = instr
            else {
                continue;
            };
            if !is_collection_literal_ctor(ctor) {
                continue;
            }
            for arg in args {
                let Some(&src) = alias_to_source.get(arg) else {
                    continue;
                };
                let entry = source_literal_aliases
                    .entry(src)
                    .or_insert_with(|| (FxHashSet::default(), None));
                // Require ALL of this source's literal-consumed aliases land in the
                // SAME Construct. A second distinct Construct dst poisons the entry.
                match entry.1 {
                    None => entry.1 = Some(*construct_dst),
                    Some(prev) if prev == *construct_dst => {}
                    Some(_) => {
                        // Aliases of one source split across two collection literals
                        // — out of scope (each literal would need its own transfer).
                        // Poison: clearing forces a use-count mismatch below.
                        entry.0.clear();
                        entry.1 = None;
                        continue;
                    }
                }
                entry.0.insert(*arg);
            }
        }
    }

    // Count EVERY use of each source var (operand of any instr / terminator). A
    // source is fully consumed iff its total use count equals the number of its
    // collection-literal aliases (so it has NO use outside those aliases — the
    // live-after clamp breaks here).
    let mut total_uses: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                *total_uses.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *total_uses.entry(v).or_default() += 1;
        }
    }

    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for (src, (aliases, construct)) in &source_literal_aliases {
        // Gate 1: the source is an owned RC-bearing aggregate the walk would
        // otherwise keep-alive.
        if !owned_vars_needing_rc.contains(src) {
            continue;
        }
        // Gate 2: >= 2 distinct aliases feeding ONE collection literal
        // ((N-1) >= 1 duplications + 1 move).
        if construct.is_none() || aliases.len() < 2 {
            continue;
        }
        // Gate 3: the source is a FRESH aggregate producer (Construct / Reuse /
        // CollectionReuse) — not a param / call result / projection (those have
        // their own transfer machinery).
        if !source_is_fresh_aggregate(func, *src) {
            continue;
        }
        // Gate 4: every use of the source is one of its collection-literal
        // aliases — no borrow Project, no later read, no other consume. A
        // live-after source (the over-suppression clamp) has extra uses and
        // DECLINES here.
        let uses = total_uses.get(src).copied().unwrap_or(0);
        if uses as usize != aliases.len() {
            continue;
        }
        out.insert(*src);
    }
    out
}

/// True iff `var`'s definer is a fresh heap-aggregate producer — `Construct`
/// (TF-3), `Reuse` (TF-9), or `CollectionReuse` (TF-9a), each of which AIMS
/// classifies as a fresh allocation owned by the caller. Param / call-result /
/// projection sources are excluded — their ref does not originate at a
/// fresh-site inc this scan owns. Mirrors the sibling `dup_inc` fresh-producer
/// classification.
fn source_is_fresh_aggregate(func: &ArcFunction, var: ArcVarId) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let dst = match instr {
                ArcInstr::Construct { dst, .. }
                | ArcInstr::Reuse { dst, .. }
                | ArcInstr::CollectionReuse { dst, .. } => *dst,
                _ => continue,
            };
            if dst == var {
                return true;
            }
        }
    }
    false
}
