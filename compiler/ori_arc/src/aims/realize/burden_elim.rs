//! DP-2/DP-3 burden-op elimination consumer.
//!
//! Walks every block backward; for each `BurdenInc` / `BurdenDec` /
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` site, queries
//! the AIMS lattice via DP-2 (`is_rc_dec_unnecessary`) or DP-3
//! (`is_rc_inc_elidable`); on `true`, removes the instruction.
//!
//! # Pipeline position
//!
//! Runs inside `emit_rc_unified` between Phase 2.1 (project-escape Incs) and
//! Phase 3 (coalesce). Burden ops are TF-N/A in both forward and backward
//! transfer (verified by the TF-N/A transfer rules), so the
//! per-block state queried via `var_state_at_block_exit(block, var)` is the
//! state at every burden-op site within that block: burden ops carry no
//! transfer effect, so `block_exit_state[var]` = state at every burden-op
//! position for `var` within the block (subject to non-burden instruction
//! effects within the block widening it — but only widening is conservative
//! for elimination, never unsound).
//!
//! # Soundness
//!
//! Per the DP-2 / DP-3 predicate truth tables:
//! - DP-2 `is_rc_dec_unnecessary(s)` ⟺ `s.cardinality = Absent ∨
//!   s.consumption = Dead` — both imply no future use, so a dec is
//!   redundant.
//! - DP-3 `is_rc_inc_elidable(s)` ⟺ `s.cardinality = Once ∧
//!   (s.consumption = Linear ∨ Affine)` — single non-duplicating consumer
//!   (moved or borrow-read-then-released), no dup inc needed.
//!
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` follow the
//! `BurdenDec` elimination rule on the WHOLE-VAR state. `BurdenDecField`
//! targets a sub-field of `base`, so DP-2 is queried against `base`'s
//! state — if the WHOLE value is dead/absent, every field-positional dec
//! against it is redundant too.
//!
//! # Paired elimination — VF-1 balance preservation
//!
//! `var_state_at_block_exit` returns `AimsState::BOTTOM` (Borrowed, Dead,
//! Absent, ...) for variables not present in the block's exit map — most
//! notably for variables defined and consumed in a terminal (Return /
//! Resume / Unreachable) block whose exit map is empty because there is
//! no successor demand to capture. Per DP-2 truth table, BOTTOM satisfies
//! `is_rc_dec_unnecessary` (Absent ∨ Dead), but BOTTOM fails DP-3
//! (`Once ∧ (Linear ∨ Affine)`; BOTTOM is Dead/Absent, not Once).
//! A naïve per-op pass would elide the `BurdenDec`
//! but retain the `BurdenInc`, producing `Σ Inc - Σ Dec = +1` and
//! violating the VF-1 intraprocedural balance invariant.
//!
//! The fix is paired elimination: group ops by target var per block,
//! check DP-2 / DP-3 against the var's exit state once, and elide ALL
//! Inc + Dec ops for that var ONLY when BOTH predicates fire — otherwise
//! retain every op for that var so the intraprocedural net stays zero.
//! When DP-2 elides a `BurdenDec`, the corresponding `BurdenInc` (if
//! elidable per DP-3) is also elided in the same pass; if DP-3 does not
//! fire on the matching Inc, the dec is retained to preserve VF-1 balance.
//!
//! # References
//!
//! - DP-2 + DP-3 — predicate truth tables.
//! - `is_rc_dec_unnecessary` (DP-2) / `is_rc_inc_elidable` (DP-3) — the AIMS
//!   transfer-function predicates.
//! - Koka Perceus paired dup/drop elimination (Reinking et al., PLDI 2021).

#[cfg(test)]
mod tests;

mod census;
mod compact;
mod lineage_rebalance;
mod loop_carried;
mod pair_atomic;

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::lattice::dimensions::{Consumption, Locality};
use crate::aims::lattice::AimsState;
use crate::aims::transfer::is_rc_inc_elidable;
use crate::aims::verify::burden_balance::{var_def_kind, var_repr_kind};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};
use crate::lower::burden_lower::genuine_dup_pair_coupling_disabled;

use super::rc_remark::{emit_rc_survivor_remark, rc_remarks_enabled, RcSurvivorSite};

#[cfg(test)]
use census::is_burden_removal_only;
use census::{assert_burden_removal_only, burden_op_census};
use compact::{compact_removed, force_overeliminate_releases};
use pair_atomic::collect_pair_atomic_alias_dsts;

/// `ORI_DISABLE_LINEAGE_REBALANCE=1` skips the per-same-alloc-rep lineage
/// re-balance ([`mark_lineage_rebalance_removals`]), leaving the per-var
/// DP-2/DP-3 elision as the sole Phase-6 consumer. Bisection surface: isolates a
/// double-free / leak to the lineage re-balance vs the per-var path without
/// toggling the whole burden-vs-predicate-stack pipeline. Default (unset): the
/// re-balance runs on the burden-only path. Spec: Annex E §AIMS RL-2.
// Env: ORI_DISABLE_LINEAGE_REBALANCE — reverts the Phase-6 lineage re-balance to per-var DP-2/DP-3 elision for bisection, debug-only
static LINEAGE_REBALANCE_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_LINEAGE_REBALANCE").as_deref() == Ok("1"));

/// `ORI_FORCE_OVERELIMINATE=1` forces the deliberately-over-eliminating shape:
/// every whole-var + field-grain `BurdenDec*` release the DP-2 guard normally
/// preserves is dropped, so a value that survives its container's drop (an
/// inner field destructured out of a `Result` while the container's own
/// release still runs) loses its RL-2 scope-exit release and leaks.
/// Default (unset): the guard stays; the pass is byte-identical.
/// Negative-pin harness only — never a production path.
// Env: ORI_FORCE_OVERELIMINATE — forces burden-elim over-elimination for negative-pin testing, debug-only
static FORCE_OVERELIMINATE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_FORCE_OVERELIMINATE").as_deref() == Ok("1"));

/// `ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ=1` reverts the single-release
/// selection to terminal-net-only (the pre-cure behavior): a kept dec is
/// accepted whenever its retention drives the per-path terminal net to 0,
/// ignoring whether a borrow-read of the rep is forward-reachable from that dec.
/// Default (unset): a candidate dec whose position is followed by a borrow-read
/// of the rep on a forward path is REJECTED (release-before-read is a UAF /
/// double-free per RL-2 `RL2_release_exactly_once` — the single release must
/// sit after the lineage's last read). Spec: Annex E §AIMS RL-2.
// Env: ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ — reverts single-release selection to terminal-net-only for bisection, debug-only
static SINGLE_RELEASE_AFTER_LAST_READ_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ").as_deref() == Ok("1")
});

/// Realization-level escape-safety gate over DP-3's `is_rc_inc_elidable` verdict.
///
/// An `Affine` (borrow) inc is a NET-0 retain on a value the borrow SHARES;
/// eliding it is sound ONLY when the value is genuinely `BlockLocal` (used +
/// released within one block — the spurious-inc case `is_rc_inc_elidable` was
/// extended to cure). A `FunctionLocal`/escaping `Affine` value (returned up a
/// TRMC chain, threaded, or aliased to a still-live owner) NEEDS its retain;
/// eliding it under-counts the shared allocation → UAF/double-free. `Linear`
/// (move) incs are unconstrained — a move transfers ownership, no shared-
/// allocation hazard. The proven DP-3 calculus (`is_rc_inc_elidable`) stays
/// intact; this is the realization consumer's guard (Spec: Annex E §AIMS
/// Locality dimension).
#[inline]
fn inc_elidable_at_realization(state: &AimsState) -> bool {
    is_rc_inc_elidable(state)
        && (state.consumption == Consumption::Linear || state.locality == Locality::BlockLocal)
}

/// Eliminate burden ops whose DP-2/DP-3 predicates fire.
///
/// Walks every block backward (matches the `coalesce_block_rc` shape in
/// `aims/emit_rc/coalesce`); for each burden-op instruction,
/// queries the appropriate predicate against `var_state_at_block_exit`
/// for the op's target var; removes the instruction when the predicate
/// returns `true`.
///
/// Operates in place on `func.blocks[*].body`. Non-burden instructions
/// are preserved verbatim. Run BEFORE `coalesce_block_rc` (Phase 3) so
/// coalesce operates on the post-elimination IR with redundant ops
/// already removed.
pub(crate) fn eliminate_burden_ops(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) {
    // INVARIANT: Phase 6 is an OPTIMIZER over the burden-emitted
    // baseline: it ELIMINATES burden ops, NEVER CONSTRUCTS them. Capture the
    // per-kind burden-op census before the pass; assert the post-pass census is
    // ≤ the pre-pass census for EVERY kind. Any future regression that appends a
    // `BurdenInc` / `BurdenDec*` inside Phase 6 trips this guard loudly rather
    // than silently lowering to a spurious `RcInc`/`RcDec` downstream and
    // corrupting RC balance in a shipped binary. Checked in every build
    // (never `debug_assert!`-gated): this is the sole structural proof that
    // Phase 6 stayed removal-only on the canonical RC-emission path — Phase 5
    // emits, Phase 6 eliminates, Phase 7 mechanically lowers.
    let before = burden_op_census(func);

    // The birth-site partition side table is installed on the converged
    // state map before Phase 6 whenever burden ops exist (the Step-4a-to-4b
    // population contract); zero release cost.
    debug_assert!(
        before.iter().sum::<usize>() == 0 || state_map.birth_site_partition().is_some(),
        "eliminate_burden_ops entered with burden ops but no birth-site partition side table"
    );

    eliminate_whole_function(func, state_map, same_alloc_reps, contracts, interner);

    assert_burden_removal_only(&before, &burden_op_census(func));
}

/// Eliminate redundant burden ops within one block's body.
///
/// Two-pass paired elimination per VF-1 intraprocedural
/// balance:
/// 1. Scan once; group all whole-var burden ops by target var; record per-op
///    DP-2 / DP-3 verdicts against the var's exit-state lookup.
/// 2. For each var, elide its Inc + Dec ops ONLY when DP-3 fires on EVERY
///    Inc AND DP-2 fires on EVERY whole-var Dec — otherwise retain every op
///    for that var so `Σ Inc - Σ Dec` stays at its pre-elimination value.
/// 3. `BurdenDecField` queries DP-2 against `base`'s whole-var state but is
///    NOT included in the whole-var balance (it contributes to a separate
///    field-grain burden-delta accumulator); elision is
///    per-op, independent of the whole-var pairing.
///
/// `OpSite` is a `(block_idx, instr_idx)` pair locating one whole-var burden
/// op in the function.
type OpSite = (usize, usize);

// Per-var WHOLE-FUNCTION balance state. `inc_sites` / `dec_sites` locate every
// whole-var Inc / Dec op for the var across ALL blocks; `all_inc_elidable`
// accumulates AND over per-op DP-3 verdicts so a single non-elidable inc
// anywhere in the function pins ALL the var's whole-var ops to RETAIN.
// Whole-function scope is load-bearing: a `BurdenInc` and its matching
// `BurdenDec` for a value live across a block boundary land in different blocks
// (FRESH alloc in one block, last-use release in another). A per-block pass
// could elide one side and retain the other, netting the per-value burden
// ledger to +/-1 — the VF-1 burden-balance imbalance.
// Pairing across the whole function keeps the inc/dec pair atomic.
#[derive(Default)]
struct WholeVarBalance {
    inc_sites: Vec<OpSite>,
    dec_sites: Vec<OpSite>,
    all_inc_elidable: bool,
}

impl WholeVarBalance {
    fn seed() -> Self {
        Self {
            all_inc_elidable: true,
            ..Self::default()
        }
    }
}

/// Whole-function DP-2/DP-3 burden-op elimination. Accumulates per-var Inc/Dec
/// balances across EVERY block, then eliminates a var's whole-var ops only when
/// DP-3 fires on every Inc AND DP-2 fires on every Dec function-wide (else
/// retains all of the var's ops). Field-grain `BurdenDecField` is settled
/// per-op (it does not participate in whole-var pairing).
fn eliminate_whole_function(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) {
    // Per-block removal bitsets, indexed [block_idx][instr_idx].
    let mut remove: Vec<Vec<bool>> = func
        .blocks
        .iter()
        .map(|b| vec![false; b.body.len()])
        .collect();

    // Lineage-aware re-balance (per `same_alloc_reps` rep) marks removals for the
    // reps it covers; per-var DP-2/DP-3 elision handles every other var. A rep is
    // covered ONLY when its per-path alloc-aware net is computable + balanceable
    // by removal alone + the lineage is loop-free — the per-var path is otherwise
    // correct and unchanged (Spec: Annex E §AIMS RL-2 release-exactly-once).
    let rebalanced_vars = lineage_rebalance::mark_lineage_rebalance_removals(
        func,
        state_map,
        same_alloc_reps,
        contracts,
        interner,
        &mut remove,
    );

    let balances = classify_burden_ops(func, state_map);
    let mut alias_dsts = collect_pair_atomic_alias_dsts(func);
    // RL-1 owned-call-arg duplication aliases are pair-atomic regardless of
    // root kind: their kept alias-site inc is inc-ONLY (the dec was
    // transfer-suppressed at Phase 5 — the consumer's release is the matched
    // release), so a DP-3 split would strip the fork's funded reference and
    // re-introduce the under-inc double-free. The FUNDED set excludes raw
    // members whose alias-site inc Phase 5 never kept (no inc → nothing to
    // guard; treating them as funded drifts accounting from emission). SSOT:
    // `compute_funded_call_arg_dup_aliases`. Spec: Annex E §AIMS RL-1.
    alias_dsts.extend(
        crate::lower::burden_lower::compute_funded_call_arg_dup_aliases(func, contracts, interner),
    );
    // RL-1 store-family duplication aliases (the fresh-local used-past-store
    // family): same inc-ONLY pair-atomicity as the owned-call-arg family —
    // the kept store-site dup inc's matched release is the CONTAINER's drop
    // (`RL1_duplication_balanced`), so a DP-3 split would strip the funded
    // reference and re-introduce the multi-store under-inc double-free. The
    // FUNDED set excludes raw members whose alias-site inc Phase 5 never kept.
    // Per-alias dsts, NEVER the root. SSOT: `compute_funded_store_dup_aliases`.
    // Spec: Annex E §AIMS RL-1.
    alias_dsts
        .extend(crate::lower::burden_lower::compute_funded_store_dup_aliases(func, contracts));
    mark_whole_var_removals(&balances, &rebalanced_vars, &alias_dsts, &mut remove);
    if *FORCE_OVERELIMINATE {
        force_overeliminate_releases(func, &mut remove);
    }
    // Observability-only (Spec: Annex E §AIMS): emit one `missed` remark per VAR
    // whose BurdenInc SURVIVED the final disposition (kept in `remove`). Per-VAR,
    // post-`mark_whole_var_removals` — the final keep/elide verdict after the
    // lineage-rebalance + pair-atomic + DP-2/DP-3 gates. A per-op verdict
    // mislabels kept incs (VF-1 balance). Reads state only; the census guard is
    // preserved (no instruction added/moved/altered).
    emit_survivor_remarks(func, state_map, &remove, interner);
    compact_removed(func, &remove);
}

/// Emit one observability-only `missed` remark per VAR whose `BurdenInc`
/// survives Phase-6 elimination (its op is kept — `remove` is false). Per-VAR,
/// not per-op: a var's whole-var incs are kept/elided atomically, so one remark
/// per surviving var reflects the final disposition. No-op (zero walk) when
/// `ORI_RC_REMARKS` is unset. Reads `AimsState` + the reused `burden_balance`
/// taxonomy helpers; emits no instruction.
fn emit_survivor_remarks(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    remove: &[Vec<bool>],
    interner: &ori_ir::StringInterner,
) {
    if !rc_remarks_enabled() {
        return;
    }
    let function_name = interner.lookup(func.name);
    let mut emitted: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::BurdenInc { var } = instr else {
                continue;
            };
            if remove[block_idx][instr_idx] || !emitted.insert(*var) {
                continue;
            }
            let state = state_map.var_state_at_definition(block_id, *var);
            // Raw byte-offset span of the surviving op site (None for synthetic
            // ops, or when spans are absent on a cache-hit IR). oric resolves it
            // to file:line via LineOffsetTable (carry here, resolve oric-side).
            let span = func
                .spans
                .get(block_idx)
                .and_then(|b| b.get(instr_idx))
                .copied()
                .flatten()
                .map(|s| (s.start, s.end));
            emit_rc_survivor_remark(&RcSurvivorSite {
                var: *var,
                consumption: state.consumption,
                locality: state.locality,
                cardinality: state.cardinality,
                def_kind: var_def_kind(func, *var),
                var_repr: var_repr_kind(func, *var),
                instr,
                span,
                function_name,
            });
        }
    }
}

/// Pass 1: classify every whole-var burden op into per-var balance buckets
/// across ALL blocks. Field-grain Decs (`BurdenDecField`) are always KEPT (the
/// burden path is the sole RC emitter; dec-side elision would leak).
///
/// Returns the per-var `WholeVarBalance` map. DP-3 (`is_rc_inc_elidable`) and
/// DP-2 (`is_rc_dec_unnecessary`) are queried against the CONVERGED
/// backward-demand at each op's target var's DEFINITION (TF-11 `seqAdd`
/// accumulation), NOT block-exit: block-exit returns BOTTOM (Dead/Absent) for a
/// fresh value defined+consumed within one block, so DP-3 could never fire and
/// the spurious fresh-site inc survived. The def-state carries `Once` for a
/// single-use value (DP-3 fires → inc elided) and `Many` for a multi-use one
/// (DP-3 does not fire → inc kept), per the proven `Cardinality.seqAdd` + DP-3
/// truth table.
fn classify_burden_ops(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> FxHashMap<ArcVarId, WholeVarBalance> {
    let mut balances: FxHashMap<ArcVarId, WholeVarBalance> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::BurdenInc { var } => {
                    let state = state_map.var_state_at_definition(block_id, *var);
                    let elidable = inc_elidable_at_realization(&state);
                    tracing::trace!(
                        target: "ori_arc::aims::realize",
                        ?var,
                        cardinality = ?state.cardinality,
                        consumption = ?state.consumption,
                        locality = ?state.locality,
                        uniqueness = ?state.uniqueness,
                        elidable,
                        "burden-inc elision decision (converged-at-def)"
                    );
                    let entry = balances.entry(*var).or_insert_with(WholeVarBalance::seed);
                    entry.inc_sites.push((block_idx, instr_idx));
                    entry.all_inc_elidable &= elidable;
                }
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    // The burden path is the sole RC emitter, so a whole-var dec
                    // is the necessary RL-2 scope-exit release and is always
                    // KEPT — the decoupled elision below elides only the
                    // spurious inc, never the release.
                    let entry = balances.entry(*var).or_insert_with(WholeVarBalance::seed);
                    entry.dec_sites.push((block_idx, instr_idx));
                }
                // `BurdenDecField` is field-grain dec-elision (dec-side, DP-2):
                // eliding it would drop a genuine release on the sole-emitter
                // path and leak, so it is always KEPT (no-op here, like every
                // non-classified instruction).
                _ => {}
            }
        }
    }
    balances
}

/// Pass 2: per-var elimination across the whole function, DECOUPLED for
/// allocation-backed vars, PAIR-ATOMIC for `Let { Var }` alias dsts.
///
/// When DP-3 fires on every Inc (`all_inc_elidable`, i.e. the converged
/// def-state cardinality is `Once` for a moved-once value), elide all of the
/// var's whole-var Incs — a moved-once value needs no inc; the alloc /
/// callee-return supplies the +1 and the surviving terminal Dec brings it to
/// 0 (RC-balanced per the proven `RL1_duplication_balanced`). The matching
/// Decs are ALSO elided ONLY when DP-2 additionally fires (`Absent`/`Dead`,
/// the genuinely-dead-value case where no release is needed). When DP-3 fires
/// but DP-2 does not (the single-use borrow-operand case), the inc is elided
/// and the RL-2 scope-exit Dec is KEPT — this cures the spurious-inc leak
/// without dropping the necessary release. A multi-use var has
/// `all_inc_elidable = false` (def-state `Many`), so its inc is retained.
///
/// The decoupled inc-only split is sound ONLY for a var whose definition
/// carries its own `+1` (fresh `Construct` / callee-return acquisition): the
/// kept dec releases the birth reference. A PARAM- or local-Construct-rooted
/// `Let { Var }` ALIAS dst (`alias_dsts` per
/// [`collect_pair_atomic_alias_dsts`]) has no birth `+1` — its `BurdenInc` IS
/// the duplication's `+1` (RL-1 dup-alias pair on the root's allocation).
/// Splitting that pair (inc elided per the alias's own `Once` state, dec
/// kept) makes the kept dec release the SOURCE's still-live reference — net
/// -1, the over-release double-free (`@stash_and_return` for param roots; the
/// local terminal-move-store shape for Construct roots). The duplication
/// decision belongs to the duplicated VALUE (the alias source, `Many`), not
/// the alias's per-var `Once` view, so for these alias dsts the pair is
/// ATOMIC per `RL1_duplication_balanced`:
/// - inc+dec pair: elide WHOLE (DP-3 on every inc AND DP-2 on every dec, with
///   the co-emitter gate) or keep WHOLE — never split;
/// - inc-only (the dec was transfer-suppressed at Phase 5; the move-out
///   consumer owns the release): NEVER elide — the inc backs the consumer's
///   cross-var release.
///
/// Toggle-gated: `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1` restores the
/// decoupled split for every pair-atomic alias dst;
/// `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1` narrows the set back to
/// param-rooted dsts only. Spec: Annex E §AIMS RL-1 + RL-2.
fn mark_whole_var_removals(
    balances: &FxHashMap<ArcVarId, WholeVarBalance>,
    rebalanced_vars: &FxHashSet<ArcVarId>,
    alias_dsts: &FxHashSet<ArcVarId>,
    remove: &mut [Vec<bool>],
) {
    let pair_coupling_active = !genuine_dup_pair_coupling_disabled();
    for (var, balance) in balances {
        // Vars whose same-alloc lineage was re-balanced as a unit own their
        // removals — skip the per-var DP-2/DP-3 pass for them (it would
        // re-strip incs whose paired decs the lineage pass kept as the one
        // RL-2 release → re-introduce the cross-var imbalance).
        if rebalanced_vars.contains(var) {
            continue;
        }
        if balance.all_inc_elidable {
            // Whole-var decs are never elided: the burden path is the sole RC
            // emitter, so each dec is the necessary RL-2 scope-exit release.
            // Only the spurious inc is elided.
            if pair_coupling_active && alias_dsts.contains(var) && !balance.inc_sites.is_empty() {
                // RL-1 dup-alias pair atomicity: an alias inc whose dec is never
                // elidable is either the backing of a cross-var release
                // (inc-only) or one half of a balanced pair — splitting nets
                // -1. Keep every op for this var.
                continue;
            }
            for &(b, i) in &balance.inc_sites {
                remove[b][i] = true;
            }
        }
    }
}
