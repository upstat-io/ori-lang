//! DP-2/DP-3 burden-op elimination consumer.
//!
//! Walks every block backward; for each `BurdenInc` / `BurdenDec` /
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` site, queries
//! the AIMS lattice via DP-2 (`is_rc_dec_unnecessary` in
//! `aims/transfer/mod.rs`) or DP-3 (`is_rc_inc_elidable` in
//! `aims/transfer/mod.rs`); on `true`, removes the instruction.
//!
//! # Pipeline position
//!
//! Runs inside `emit_rc_unified` between Phase 2.1 (project-escape Incs) and
//! Phase 3 (coalesce). Burden ops are TF-N/A in both forward and backward
//! transfer (verified by the TF-N/A transfer rules in `aims/transfer/mod.rs`), so the
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
//! violating VF-1 per `aims/verify/burden_balance.rs`.
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
//! - `aims/transfer/mod.rs` (`is_rc_dec_unnecessary` / `is_rc_inc_elidable`) — predicate source.
//! - Koka Perceus paired dup/drop elimination (Reinking et al., PLDI 2021).

#[cfg(test)]
mod tests;

mod loop_carried;

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::lattice::dimensions::{AccessClass, Consumption, Locality};
use crate::aims::lattice::AimsState;
use crate::aims::transfer::is_rc_inc_elidable;
use crate::aims::verify::burden_balance::{var_def_kind, var_repr_kind};
use crate::aims::verify::burden_delta::{compute_burden_entry_nets, whole_var_dec_target};
use crate::graph::{compute_predecessors, DominatorTree};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::lower::burden_lower::{
    collect_move_edges_and_store_consumes, genuine_dup_pair_coupling_disabled,
    local_construct_pair_coupling_disabled,
};

use super::rc_remark::{emit_rc_survivor_remark, rc_remarks_enabled, RcSurvivorSite};

/// `ORI_DISABLE_LINEAGE_REBALANCE=1` skips the per-same-alloc-rep lineage
/// re-balance ([`mark_lineage_rebalance_removals`]), leaving the per-var
/// DP-2/DP-3 elision as the sole Phase-6 consumer. Bisection surface: isolates a
/// double-free / leak to the lineage re-balance vs the per-var path without
/// toggling the whole burden-vs-predicate-stack pipeline. Default (unset): the
/// re-balance runs on the burden-only path. Spec: Annex E §AIMS RL-2.
static LINEAGE_REBALANCE_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_LINEAGE_REBALANCE").as_deref() == Ok("1"));

/// `ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ=1` reverts the single-release
/// selection to terminal-net-only (the pre-cure behavior): a kept dec is
/// accepted whenever its retention drives the per-path terminal net to 0,
/// ignoring whether a borrow-read of the rep is forward-reachable from that dec.
/// Default (unset): a candidate dec whose position is followed by a borrow-read
/// of the rep on a forward path is REJECTED (release-before-read is a UAF /
/// double-free per RL-2 `RL2_release_exactly_once` — the single release must
/// sit after the lineage's last read). Spec: Annex E §AIMS RL-2.
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
    // than silently lowering to a spurious `RcInc`/`RcDec` downstream.
    // Canonical RC-Emission Path: Phase 5 emits, Phase 6
    // eliminates, Phase 7 mechanically lowers.
    #[cfg(debug_assertions)]
    let before = burden_op_census(func);

    eliminate_whole_function(func, state_map, same_alloc_reps, contracts, interner);

    #[cfg(debug_assertions)]
    debug_assert_burden_removal_only(&before, &burden_op_census(func));
}

/// Per-kind burden-op census across all blocks of a function.
///
/// Five counts, one per burden-op variant, in the order `[BurdenInc,
/// BurdenDec, BurdenDecPartial, BurdenDecField, BurdenDecVariant]`. Used by the
/// removal-only structural guard in [`eliminate_burden_ops`].
#[cfg(any(debug_assertions, test))]
fn burden_op_census(func: &ArcFunction) -> [usize; 5] {
    let mut census = [0usize; 5];
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { .. } => census[0] += 1,
                ArcInstr::BurdenDec { .. } => census[1] += 1,
                ArcInstr::BurdenDecPartial { .. } => census[2] += 1,
                ArcInstr::BurdenDecField { .. } => census[3] += 1,
                ArcInstr::BurdenDecVariant { .. } => census[4] += 1,
                _ => {}
            }
        }
    }
    census
}

/// Whether a Phase-6 burden census transition is removal-only.
///
/// `true` iff the post-pass census of EVERY burden-op kind is `≤` its
/// pre-pass census — i.e. Phase 6 removed or left-alone every kind and
/// constructed none. This is the SSOT predicate behind the removal-only
/// structural guard; both the debug-assert and the negative pin consult it.
#[cfg(any(debug_assertions, test))]
fn is_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) -> bool {
    (0..5).all(|kind| after[kind] <= before[kind])
}

/// Removal-only structural guard.
///
/// Phase 6 (the lattice optimizer) MUST only remove or annotate burden ops —
/// it MUST NOT construct new ones: `eliminate_burden_ops` consumes DP-2/DP-3
/// at burden-op sites and NEVER constructs burden ops. The post-pass census
/// of EVERY burden-op kind is
/// therefore `≤` its pre-pass census. A violation is a Phase-6 construction
/// regression: a `BurdenInc`/`BurdenDec*` was appended where only elimination
/// is permitted, which would mechanically lower to a spurious `RcInc`/`RcDec`
/// in Phase 7 and corrupt RC balance.
#[cfg(debug_assertions)]
#[track_caller]
fn debug_assert_burden_removal_only(before: &[usize; 5], after: &[usize; 5]) {
    const KIND_NAMES: [&str; 5] = [
        "BurdenInc",
        "BurdenDec",
        "BurdenDecPartial",
        "BurdenDecField",
        "BurdenDecVariant",
    ];
    debug_assert!(
        is_burden_removal_only(before, after),
        "AIMS Phase-6 invariant: eliminate_burden_ops constructed burden ops \
         where only elimination is permitted — Phase 6 MUST eliminate burden \
         ops, never construct them. \
         per-kind census {KIND_NAMES:?}: before = {before:?}, after = {after:?}",
    );
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
///    NOT included in the whole-var balance (per `aims/verify/burden_delta.rs`
///    it contributes to a separate field-grain accumulator); elision is
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
// ledger to +/-1 — the VF-1 imbalance per `aims/verify/burden_balance.rs`.
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
    let rebalanced_vars = mark_lineage_rebalance_removals(
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

/// Read-only structural scan: every `Let { Var }` alias dst whose alias-chain
/// ROOT is a function PARAM or a LOCAL fresh `Construct`.
///
/// An inc-carrying alias dst with such a root is an RL-1 duplication-alias
/// pair on the root's allocation: its `BurdenInc` IS the alias's own `+1` (an
/// alias definition is never a birth site — the var is a same-allocation view
/// of the root's reference), paired with either its own last-use `BurdenDec`
/// or a cross-var release (the consumer's, when the alias was
/// transfer-suppressed at Phase 5). The pair is ATOMIC per
/// `AimsProof.Realization::RL1_duplication_balanced` —
/// [`mark_whole_var_removals`] consults this set to ban the decoupled inc-only
/// split for these vars (splitting nets -1 on the still-live root lineage —
/// the `@stash_and_return` double-free for param roots; the local
/// terminal-move-store double-free for Construct roots).
///
/// Aliases rooted at an `Apply` / `Invoke` RESULT stay on the decoupled path:
/// their splits are load-bearing compensation for pre-existing
/// under-emissions, coupled only WITH the matching under-emission cure (each
/// its own cycle). Construct-rooted lineages WITHOUT a store consume likewise
/// stay decoupled — their splits compensate borrowed-call-arg lineage
/// arrangements the same way. The Construct-rooted admission is gated by
/// `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1` independently of the
/// param-rooted admission (master toggle:
/// `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1` gates the whole pair-atomic
/// guard in [`mark_whole_var_removals`]).
fn collect_pair_atomic_alias_dsts(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    // Pass 1: resolve every `Let { Var }` alias chain to its root (vars are
    // defined before use within the function walk).
    let mut root: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let r = root.get(src).copied().unwrap_or(*src);
                root.insert(*dst, r);
            }
        }
    }
    let root_of = |v: ArcVarId| root.get(&v).copied().unwrap_or(v);
    // Pass 2: admissible roots — every param; plus local `Construct` dsts
    // whose lineage TERMINALLY moves into an aggregate-STORE (the local
    // terminal-move-store family, `Holder { kept: w }` with no use of `w`
    // after the store): the store transfers the root's reference into the
    // container, so the pre-elim per-lineage ledger is exact and any split
    // nets -1. A lineage with a use AFTER the store (the local genuine-dup
    // shape — store + source read after) stays DECOUPLED: its FRESH-site inc
    // is balanced by a read-alias split, the compensation arrangement the
    // decoupled path provides until that over-emission gets its own cycle.
    // Store-consume membership shares the Phase-5 SSOT
    // (`collect_move_edges_and_store_consumes`).
    let mut root_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    if !local_construct_pair_coupling_disabled() {
        root_vars.extend(terminal_store_construct_roots(func, &root));
    }
    // Pass 3: collect alias dsts whose chain root is admissible.
    let mut pair_atomic: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if root_vars.contains(&root_of(*src)) || root_vars.contains(src) {
                    pair_atomic.insert(*dst);
                }
            }
        }
    }
    // Pass 4: loop-carried pure-borrow-view aliases (RL-1 pair atomicity) —
    // see `loop_carried::extend_with_loop_carried_borrow_view_aliases`.
    loop_carried::extend_with_loop_carried_borrow_view_aliases(func, &root, &mut pair_atomic);
    pair_atomic
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

/// A whole-var burden op site located across the function: `(block_idx,
/// instr_idx)` of the instruction. Same as [`OpSite`] but named for the
/// lineage-rebalance pass which keys ops by their lineage rep, not their var.
type LineageOp = (usize, usize);

/// Whole-var burden inc/dec op sites of ONE same-alloc lineage rep, plus the
/// rep's fresh-self-alloc block footprint + loop membership — the grouping the
/// lineage re-balance decides removals over.
#[derive(Default)]
struct RepOps {
    inc_sites: Vec<LineageOp>,
    dec_sites: Vec<LineageOp>,
    /// Distinct vars that carry a whole-var burden op (inc OR dec) on this
    /// lineage — the alias-chain signature is ≥2.
    op_vars: FxHashSet<ArcVarId>,
    all_vars: FxHashSet<ArcVarId>,
    /// Per-block fresh-self-alloc count (block index → number of lineage
    /// allocations in that block). The `+1`s of the post-re-balance net.
    alloc_counts: FxHashMap<usize, i64>,
    touches_loop: bool,
}

/// Group every whole-var burden op (inc / dec) + fresh self-allocation by its
/// `same_alloc_reps` rep into [`RepOps`]. The single grouping pass the lineage
/// re-balance decides over; `loop_blocks` taints a rep touching any loop block.
fn group_lineage_ops(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    list_take_name: ori_ir::Name,
    loop_blocks: &FxHashSet<usize>,
) -> FxHashMap<ArcVarId, RepOps> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashMap<ArcVarId, RepOps> = FxHashMap::default();
    for (b, block) in func.blocks.iter().enumerate() {
        let in_loop = loop_blocks.contains(&b);
        for (i, instr) in block.body.iter().enumerate() {
            if let Some(dst) =
                super::emit_unified::fresh_rc_alloc_dst(instr, func, interner, list_take_name)
            {
                let entry = reps.entry(rep_of(dst)).or_default();
                *entry.alloc_counts.entry(b).or_insert(0) += 1;
                entry.all_vars.insert(dst);
                entry.touches_loop |= in_loop;
            }
            match instr {
                ArcInstr::BurdenInc { var } => {
                    let entry = reps.entry(rep_of(*var)).or_default();
                    entry.inc_sites.push((b, i));
                    entry.op_vars.insert(*var);
                    entry.all_vars.insert(*var);
                    entry.touches_loop |= in_loop;
                }
                _ => {
                    if let Some(var) = whole_var_dec_target(instr) {
                        let entry = reps.entry(rep_of(var)).or_default();
                        entry.dec_sites.push((b, i));
                        entry.op_vars.insert(var);
                        entry.all_vars.insert(var);
                        entry.touches_loop |= in_loop;
                    }
                }
            }
        }
        // An `Invoke`-terminator self-allocating builtin result FIRST lives at the
        // `normal` successor block (where Phase-5 prepends its fresh-site
        // `BurdenInc`); attribute the lineage alloc to that successor so the
        // re-balance net counts the allocation on the path the result is defined.
        // Spec: Annex E §AIMS RL-2.
        if let Some((dst, normal)) =
            super::emit_unified::fresh_rc_alloc_dst_terminator(&block.terminator, func, interner)
        {
            let nb = normal.index();
            let entry = reps.entry(rep_of(dst)).or_default();
            *entry.alloc_counts.entry(nb).or_insert(0) += 1;
            entry.all_vars.insert(dst);
            entry.touches_loop |= loop_blocks.contains(&nb);
        }
    }
    reps
}

/// Per-same-alloc-rep lineage re-balance — the cross-var release-exactly-once
/// pass that the per-var DP-2/DP-3 elimination cannot express.
///
/// For an alias chain (`let b = a; a == b`), Phase 5 emits BALANCED per-alias
/// inc/dec pairs on each Let-Var / borrow-operand alias of ONE allocation; the
/// per-var pass elides the borrow aliases' incs (DP-3: `Once+Linear`) but keeps
/// every dec (DP-2 fails: `Once`, not `Absent/Dead`), so the lineage's kept
/// incs/decs net to a non-zero per-path balance — a double-free on the alias
/// branch + leak on the dead branch. The per-var pass cannot see the cross-var
/// balance because it never receives `same_alloc_reps`.
///
/// This pass groups burden ops by `same_alloc_reps` rep and, for each rep it
/// can re-balance by removal ALONE, marks: elide ALL incs (the allocation
/// supplies the lineage's +1 — every alias inc is spurious) + keep EXACTLY ONE
/// dec that releases the allocation on EVERY alloc-reachable terminal path
/// (RL-2 `RL2_release_exactly_once`: a value allocated at RC = 1 nets 0 by one
/// release), eliding every other dec. The kept dec is verification-selected:
/// only a dec whose retention drives the lineage's per-path terminal net to 0
/// (via `compute_burden_entry_nets`) is accepted.
///
/// SCOPE (conservative, non-loop only): a rep is re-balanced ONLY when ALL hold,
/// else the per-var pass owns its vars unchanged:
/// - the rep has ≥1 fresh self-alloc member (it is a real allocation lineage);
/// - NO member's burden ops sit in a loop block — `same_alloc_reps` EXCLUDES the
///   Jump-phi back-edge by design, so a loop-carried value's release attributes
///   to a different rep and the per-path net mis-computes (the known blind spot
///   per the M-series dead-ends); loop-carried lineages defer to a later pass;
/// - eliding all incs + keeping exactly one dec yields a per-path terminal net
///   of 0 on every alloc-reachable terminal (else removal alone cannot balance
///   it — the missing release must be edge-emitted, out of scope here).
///
/// Returns the set of vars whose removals this pass owns; `mark_whole_var_removals`
/// skips them. Spec: Annex E §AIMS RL-1 (alias inc spurious) + RL-2 (one release).
fn mark_lineage_rebalance_removals(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    remove: &mut [Vec<bool>],
) -> FxHashSet<ArcVarId> {
    let mut owned_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    // The lineage re-balance keeps exactly one dec as the RL-2 release. The
    // burden path is the sole real-RC emitter, so that kept dec is the correct
    // sole release. `ORI_DISABLE_LINEAGE_REBALANCE=1` is the bisection escape
    // hatch that defers every rep to the per-var pass.
    if *LINEAGE_REBALANCE_DISABLED {
        return owned_vars;
    }
    let list_take_name = super::emit_unified::for_yield_result_finalizer_name(interner);
    let preds = compute_predecessors(func);
    let dom = DominatorTree::build(func);
    let loop_blocks = compute_loop_blocks(func, &preds, &dom);

    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);

    // Forwarder-tainted reps partition into TWO classes via the callee
    // `ParamContract`/`ReturnContract` provenance:
    //
    // (1) TRANSFER-through-return forwarders (`ApplyAliasSource::Direct(arg)` whose
    //     callee param `transfers_through_return == true`, `@id<T>(x: T) -> T = x`):
    //     the callee OWN-CONSUMES the arg and transfers it back out (RL-34
    //     `transferOwnership`; RL-2 `Return` transfer terminal use), so the caller
    //     acquires the SAME allocation at the result `dst`. That acquisition IS the
    //     lineage `+1` (`transfer_forwarder_anchors` records the transfer block);
    //     the per-alias incs AND the Aggregate-result apply-result inc are spurious
    //     duplicates on top (RL-1: a single moved transfer, not a duplication). The
    //     rep IS re-balanceable: elide ALL incs + keep EXACTLY ONE POST-transfer
    //     release. The anchor at the transfer block makes the pre-transfer arg-side
    //     dec NON-forward-reachable, so `select_single_release_dec` never keeps it
    //     (keeping it frees the value before the `[own]` move — UAF).
    //
    // (2) Every OTHER apply-result alias — a `Project` borrow-view return (the
    //     callee returns `arg.field` it BORROWED), a `Conditional`/`Wrapped` shape,
    //     or a `Direct` whose contract does NOT prove `transfers_through_return` —
    //     stays EXCLUDED. Its result is not a uniform moved transfer of a single
    //     owned arg, so its result-inc is not a uniform spurious duplicate; eliding
    //     it under-counts the shared allocation → UAF. The per-var pass owns it.
    //
    // Spec: Annex E §AIMS RL-1 (genuine vs spurious inc) + RL-2 (release exactly
    // once) + RL-34 (tail-call ownership transfer).
    let transfer_forwarder_anchors = compute_transfer_forwarder_anchors(
        func,
        same_alloc_reps,
        state_map.apply_result_aliases(),
        contracts,
    );
    let forwarder_tainted_reps: FxHashSet<ArcVarId> = state_map
        .apply_result_aliases()
        .keys()
        .map(|&v| rep_of(v))
        .collect();

    let reps = group_lineage_ops(
        func,
        same_alloc_reps,
        interner,
        list_take_name,
        &loop_blocks,
    );

    // COW-mutated lineage reps DEFER to the per-var pass. The re-balance elides ALL
    // incs of a rep + keeps one release; for a lineage whose value flows into a
    // COW-mutation operand (`push`/`set`/`insert`/`remove`/`sort`/`reverse` at an
    // owned arg, collection `+`/concat, or a may-COW user-call arg) AND is re-read
    // after the consume, the fresh keep-alive inc is LOAD-BEARING (RL-1: raises the
    // runtime rc ≥ 2 so the `ori_rc_is_unique` COW protocol COPIES instead of
    // mutating in place). Eliding it lets the mutation hit a freed/aliased buffer →
    // double-free. The per-var pass (`classify_burden_ops` /
    // `mark_whole_var_removals`) is COW-aware (`inc_elidable_at_realization` /
    // `compute_elidable_fresh_self_alloc_incs`) and correctly KEEPS the inc. SSOT
    // detector reused — never duplicated. Spec: Annex E §AIMS RL-1.
    let cow_mutated_reps = super::emit_unified::compute_cow_mutated_lineage_reps(
        func,
        same_alloc_reps,
        interner,
        contracts,
    );

    for (rep, ops) in &reps {
        // A forwarder-tainted rep is un-excluded ONLY when it BOTH is a proven
        // owned-transfer-through-return forwarder AND has NO `fresh_rc_alloc_dst`
        // self-alloc — the AGGREGATE case where the transferred-in `Construct` is
        // not RcPtr/FatVal so Phase 5 KEEPS the spurious apply-result inc and the
        // lineage has no recognized `+1` (`box_list` / `option_list`). The
        // transfer anchor supplies that `+1` and the re-balance elides the spurious
        // inc + keeps one post-transfer release.
        //
        // An RcPtr/FatVal forwarder (`id<T>([int]) -> [int]`, or `borrow_if_use`
        // returning RcPtr lists) HAS a `fresh_rc_alloc_dst` self-alloc: Phase 5
        // already SUPPRESSED its apply-result inc (`compute_transfer_through_return
        // _results`, repr-gated to RcPtr/FatVal), so it was sound under the per-var
        // pass while EXCLUDED. Un-excluding it would let the self-alloc re-balance
        // fire on a multi-block forwarder lineage and mis-select the kept release
        // (a used-then-returned forwarder body corrupts the value) — keep it
        // excluded. Spec: Annex E §AIMS RL-1 + RL-2.
        let transfer_anchor_blocks = transfer_forwarder_anchors.get(rep);
        let is_aggregate_transfer_forwarder =
            transfer_anchor_blocks.is_some() && ops.alloc_counts.is_empty();
        let excluded_forwarder =
            forwarder_tainted_reps.contains(rep) && !is_aggregate_transfer_forwarder;
        // The lineage `+1` anchor: where the allocation is BORN owned in this rep.
        // A transfer forwarder THREADS an existing allocation through — it adds no
        // new `+1`. So the anchor is:
        //   - the `fresh_rc_alloc_dst` self-alloc block(s), when the rep has any
        //     (RcPtr/FatVal `Construct`/literal/collection-source — the canonical
        //     birth site). The forwarder transfer of such a value adds nothing.
        //   - ELSE, for an Aggregate transfer forwarder whose `Construct` is NOT
        //     `fresh_rc_alloc_dst`-recognized (repr-gated to RcPtr/FatVal), the
        //     transfer block(s) where the transferred-in value arrives owned at the
        //     result `dst` — the earliest point in the rep where the allocation is
        //     known to exist owned by the caller.
        // Using the self-alloc when present prevents a DOUBLE `+1` (born once +
        // transferred once) that would over-count an RcPtr forwarder's lineage.
        let mut alloc_delta: Vec<i64> = vec![0; func.blocks.len()];
        let alloc_blocks: Vec<usize> = if ops.alloc_counts.is_empty() {
            // Aggregate transfer forwarder: anchor `+1` at each transfer block
            // (one acquisition per block on every path through it).
            match transfer_anchor_blocks {
                Some(blocks) => {
                    for &b in blocks {
                        alloc_delta[b] = 1;
                    }
                    let mut v: Vec<usize> = blocks.clone();
                    v.sort_unstable();
                    v.dedup();
                    v
                }
                None => Vec::new(),
            }
        } else {
            for (&b, &c) in &ops.alloc_counts {
                alloc_delta[b] = c;
            }
            ops.alloc_counts.keys().copied().collect()
        };
        // Candidate gate: a loop-free lineage with a `+1` anchor whose burden ops
        // span ≥2 distinct SSA vars (the alias-chain / forwarder signature). A
        // single-var lineage is already handled by the per-var pass; a loop-carried
        // lineage's release attributes to a different rep (`same_alloc_reps`
        // excludes the back-edge) so its per-path net mis-computes; an excluded
        // forwarder's apply-result inc is not a uniform spurious duplicate; a
        // COW-mutated rep's fresh keep-alive inc is load-bearing (RL-1) and is
        // owned by the COW-aware per-var pass, not elidable here.
        if alloc_blocks.is_empty()
            || ops.touches_loop
            || ops.op_vars.len() < 2
            || excluded_forwarder
            || cow_mutated_reps.contains(rep)
        {
            continue;
        }
        // The SOLE discriminator: eliding ALL incs (alias-spurious by the rep's
        // same-allocation construction — every member shares the one rc, needs
        // no inc) + keeping EXACTLY ONE POST-anchor dec must drive the lineage's
        // per-path terminal net to 0 on every anchor-reachable terminal. Verified
        // per-path via `compute_burden_entry_nets` — never a flat op count (it
        // double-counts mutually-exclusive paths). A transferred lineage (alloc
        // returned / moved out → consumer decs, needs no local release) has NO
        // balancing single-dec and is rejected (the per-var pass keeps it). Spec:
        // Annex E §AIMS RL-1 (alias inc spurious) + RL-2 (release exactly once).
        let Some(kept_dec) = select_single_release_dec(
            func,
            &preds,
            &alloc_blocks,
            &alloc_delta,
            &ops.dec_sites,
            *rep,
            &rep_of,
        ) else {
            continue;
        };
        // Commit: elide all incs + every dec except the kept release.
        for &(b, i) in &ops.inc_sites {
            remove[b][i] = true;
        }
        for &(b, i) in &ops.dec_sites {
            if (b, i) != kept_dec {
                remove[b][i] = true;
            }
        }
        owned_vars.extend(ops.all_vars.iter().copied());

        if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = interner.lookup(func.name),
                rep = rep.raw(),
                incs_elided = ops.inc_sites.len(),
                decs = ops.dec_sites.len(),
                kept_dec_block = kept_dec.0,
                "lineage re-balance: alias-chain release-exactly-once"
            );
        }
    }
    owned_vars
}

/// Per-rep transfer-through-return forwarder anchors: the rep of a generics
/// forwarder (`@id<T>(x: T) -> T = x`) → the block indices where the forwarder
/// RESULT is defined (the `Apply`/`Invoke` `dst`).
///
/// A forwarder unions its consumed arg and its result into ONE
/// `same_alloc_reps` rep via `ApplyAliasSource::Direct` (the callee returns the
/// param unchanged). The callee OWN-CONSUMES the arg and transfers it back out
/// (RL-34 `transferOwnership`: callee Owned param → no post-call dec; RL-2
/// `Return` is a transfer terminal use), so the caller acquires the SAME
/// allocation at the result `dst`. That acquisition IS the lineage's `+1` — the
/// transferred-in value arrives owned at `dst`, not at a fresh `Construct` (an
/// Aggregate forwarder result whose `Construct` `fresh_rc_alloc_dst` does not
/// count). The result-INC the Phase-5 walk emits for an Aggregate forwarder
/// result is a spurious duplicate ON TOP of that `+1` (RL-1: the value is moved
/// once through the forwarder, not duplicated), elidable by the re-balance.
///
/// Discriminator (the precise transfer-vs-alias-spurious distinction): the entry
/// is admitted ONLY for an `ApplyAliasSource::Direct(arg)` whose callee's
/// `ParamContract` for `arg`'s position has `transfers_through_return == true`
/// (the proven Return-flow transfer fact, `IcReturnContract` provenance). A
/// `Project` borrow-view return (the callee returns `arg.field` it borrowed) or
/// a `Conditional`/`Wrapped` shape is NOT admitted — its result is not a moved
/// transfer of a single owned arg, so its result-inc is not a uniform spurious
/// duplicate. Those reps stay excluded (the per-var pass owns them).
///
/// Spec: Annex E §AIMS RL-1 (genuine vs spurious inc) + RL-2 (release exactly
/// once) + RL-34 (tail-call ownership transfer).
fn compute_transfer_forwarder_anchors(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<ArcVarId, Vec<usize>> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    // arg-position → OWNED-transfer-through-return lookup for a callee's contract.
    //
    // BOTH conditions are required (the over-fire boundary):
    //   - `access == Owned`: the callee OWN-CONSUMES the param (RL-34
    //     `rl34_action .Owned = transferOwnership` — ownership moves through; no
    //     post-call dec). A `Borrowed` param is `decBeforeCall` (RL-34): the callee
    //     borrows a VIEW and returns it; the caller STILL OWNS the original, so the
    //     apply-result is a borrow alias (NOT a moved transfer-dup). A borrowed
    //     forwarder (`@borrow_if_use(xs: [int]) -> [int] = xs`, `xs` borrow-read
    //     then returned) sets `transfers_through_return` (the param flows to Return)
    //     yet keeps `access: Borrowed` — its rep must stay EXCLUDED; the per-var
    //     pass owns it. Without the `Owned` gate the re-balance over-fires on the
    //     borrowed-forwarder lineage.
    //   - `transfers_through_return`: the param flows to a `Return { value }`
    //     terminator (the proven `facts.return_flow` fact), so the caller acquires
    //     the same allocation at the result `dst`.
    let arg_transfers_through_return = |callee: &Name, arg: ArcVarId, args: &[ArcVarId]| -> bool {
        let Some(contract) = contracts.get(callee) else {
            return false;
        };
        args.iter()
            .zip(contract.params.iter())
            .any(|(&a, p)| a == arg && p.transfers_through_return && p.access == AccessClass::Owned)
    };
    // The anchor block is where the forwarder RESULT `dst` is first LIVE OWNED:
    //   - `Apply` instruction: `dst` is bound in the SAME block (anchor = that
    //     block).
    //   - `Invoke` TERMINATOR: `dst` is bound on the NORMAL-successor edge — it is
    //     NOT live in the Invoke's own block (the unwind edge does not bind it).
    //     Anchor = the normal successor. Anchoring at the Invoke's own block would
    //     make the pre-call arg-side dec forward-reachable (the Invoke block
    //     dominates everything), so the `select_single_release_dec` filter could
    //     keep that pre-transfer dec (UAF — it frees before the `[own]` move).
    let mut anchors: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if let Some(ApplyAliasSource::Direct(arg)) = apply_result_aliases.get(dst) {
                    if arg_transfers_through_return(callee, *arg, args) {
                        // Apply result is live in its own block.
                        anchors
                            .entry(rep_of(*dst))
                            .or_default()
                            .push(block.id.index());
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            normal,
            ..
        } = &block.terminator
        {
            if let Some(ApplyAliasSource::Direct(arg)) = apply_result_aliases.get(dst) {
                if arg_transfers_through_return(callee, *arg, args) {
                    // Invoke result is bound on the normal-successor edge.
                    anchors
                        .entry(rep_of(*dst))
                        .or_default()
                        .push(normal.index());
                }
            }
        }
    }
    anchors
}

/// Select the ONE dec op site to keep as the lineage's RL-2 release: the dec
/// whose retention (with all incs + all OTHER decs elided) drives the lineage's
/// per-path terminal net to 0 on EVERY alloc-reachable terminal block, with no
/// merge disagreement. Returns `None` when no single dec satisfies this (the
/// lineage transfers out / needs a multi-branch release that removal alone
/// cannot supply — both defer to the per-var pass).
///
/// The kept release MUST be in a block forward-reachable from an alloc/transfer
/// anchor block. For a pure-alias-chain lineage every dec is post-alloc, so this
/// is a no-op. For a forwarder lineage anchored at the TRANSFER block (where the
/// caller acquires the transferred-in value at the result `dst`), it excludes
/// the pre-transfer arg-side decs: a dec there releases the value BEFORE it is
/// moved into the forwarder `[own]` arg, freeing a reference about to be
/// transferred — a use-after-free even when the per-path net verifies 0. Those
/// decs are elided with the rest; only a post-transfer dec is the RL-2 release.
///
/// The delta passed to the per-path net dataflow models the post-re-balance
/// lineage exactly: ALL incs elided (the only `+1`s are the per-block allocation
/// counts in `alloc_delta`) + the single kept dec's `−1`.
///
/// Candidates are ordered alloc-block-first: a dec in the allocation's own block
/// fires on every path through it (every path from the allocation passes through
/// it), so it is the most likely single dominating release.
fn select_single_release_dec(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    alloc_blocks: &[usize],
    alloc_delta: &[i64],
    dec_sites: &[LineageOp],
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> Option<LineageOp> {
    let alloc_reachable = forward_reachable(func, alloc_blocks);
    // The kept release must be forward-reachable from an anchor block — a
    // pre-anchor dec (the pre-transfer arg-side dec of a forwarder) frees the
    // value before it is moved into the forwarder `[own]` arg (UAF). Filter
    // those out before selection; they are elided with the other decs.
    let mut candidates: Vec<LineageOp> = dec_sites
        .iter()
        .copied()
        .filter(|&(b, _)| alloc_reachable.contains(&b))
        .collect();
    // RL-2 release-exactly-once: the kept release MUST sit after the lineage's
    // LAST read. A dec followed by a borrow-read of the rep on a forward path
    // frees the allocation while a later block still reads it — a use-after-free
    // / double-free (the `copy[0]` … `copy[1]` yield-identity shape: an early
    // alias's dec in bb3 frees the list buffer the bb7 `@__index(alias)` still
    // reads). The terminal-net check below only validates per-path BALANCE, not
    // read-ordering; this filter removes the unsafe candidates before selection.
    // Disabled by `ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ=1`.
    if !*SINGLE_RELEASE_AFTER_LAST_READ_DISABLED {
        candidates.retain(|&(b, i)| !dec_precedes_rep_read(func, rep, rep_of, b, i));
    }
    // Ordering: alloc-block decs first, then the rest (stable within each group).
    candidates.sort_by_key(|&(b, _)| usize::from(!alloc_blocks.contains(&b)));

    for &keep in &candidates {
        let mut delta = alloc_delta.to_vec();
        delta[keep.0] -= 1;
        let nets = compute_burden_entry_nets(func, preds, &delta);
        if !nets.disagree_blocks.is_empty() {
            continue;
        }
        let mut all_zero = true;
        for (b, block) in func.blocks.iter().enumerate() {
            let Some(eb) = nets.entry_net[b] else {
                continue;
            };
            if !alloc_reachable.contains(&b) {
                continue;
            }
            if !matches!(
                block.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
            ) {
                continue;
            }
            if eb + delta[b] != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            return Some(keep);
        }
    }
    None
}

/// True iff a borrow-read of `rep` is forward-reachable from the dec at
/// `(dec_block, dec_idx)` — i.e. keeping the release there would free the
/// allocation before a later read (UAF). Scans: (1) the same block AFTER
/// `dec_idx` (body instrs + terminator), and (2) every block forward-reachable
/// across the dec block's successor edges (full body + terminator). Reuses the
/// borrow-read-of-rep SSOT (`instr_borrow_reads_rep` / `terminator_borrow_reads_rep`)
/// so the read predicate never drifts. Spec: Annex E §AIMS RL-2.
fn dec_precedes_rep_read(
    func: &ArcFunction,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    dec_block: usize,
    dec_idx: usize,
) -> bool {
    use super::emit_unified::{instr_borrow_reads_rep, terminator_borrow_reads_rep};
    let dec_blk = &func.blocks[dec_block];
    // (1) Same block, strictly after the dec position.
    for instr in dec_blk.body.iter().skip(dec_idx + 1) {
        if instr_borrow_reads_rep(instr, rep_of, rep) {
            return true;
        }
    }
    if terminator_borrow_reads_rep(&dec_blk.terminator, rep_of, rep) {
        return true;
    }
    // (2) Strictly-forward blocks (successors of the dec block, transitively;
    // the dec block itself is excluded — its post-dec slice is covered above).
    let succ_starts: Vec<usize> = crate::graph::successor_block_ids(&dec_blk.terminator)
        .iter()
        .map(|s| s.index())
        .collect();
    let reachable = forward_reachable(func, &succ_starts);
    for b in reachable {
        if b == dec_block {
            continue;
        }
        let block = &func.blocks[b];
        for instr in &block.body {
            if instr_borrow_reads_rep(instr, rep_of, rep) {
                return true;
            }
        }
        if terminator_borrow_reads_rep(&block.terminator, rep_of, rep) {
            return true;
        }
    }
    false
}

/// Local `Construct` roots whose alias lineage TERMINALLY moves into an
/// aggregate-STORE: the lineage has at least one store-consumed member
/// (`Construct` / `Reuse` / `CollectionReuse` arg, `Set.value` — per the
/// Phase-5 SSOT `collect_move_edges_and_store_consumes`) and NO lineage-member
/// use strictly after any store site ("after" = later in the store's block,
/// OR in a block forward-reachable from the store block's successors — the
/// loop back-edge re-reach counts, mirroring the Phase-5 reachability
/// discriminator). A lineage used past the store is the local genuine-dup
/// shape and stays decoupled (its FRESH-site inc is compensated by a
/// read-alias split until that over-emission's own cycle).
fn terminal_store_construct_roots(
    func: &ArcFunction,
    root: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let root_of = |v: ArcVarId| root.get(&v).copied().unwrap_or(v);
    let mut construct_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, .. } = instr {
                construct_dsts.insert(*dst);
            }
        }
    }
    if construct_dsts.is_empty() {
        return FxHashSet::default();
    }
    let (_, store_consumed) = collect_move_edges_and_store_consumes(func);
    // Per-root lineage use sites + store-consume sites. Terminator uses at
    // `usize::MAX` so any body index in the block precedes them.
    let mut lineage_uses: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    let mut lineage_store_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            for &v in &instr.used_vars() {
                let r = root_of(v);
                if construct_dsts.contains(&r) {
                    lineage_uses
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
            let store_args: &[ArcVarId] = match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => args,
                ArcInstr::Set { value, .. } => std::slice::from_ref(value),
                _ => &[],
            };
            for &v in store_args {
                if !store_consumed.contains(&v) {
                    continue;
                }
                let r = root_of(v);
                if construct_dsts.contains(&r) {
                    lineage_store_sites
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
        }
        for v in block.terminator.used_vars() {
            let r = root_of(v);
            if construct_dsts.contains(&r) {
                lineage_uses
                    .entry(r)
                    .or_default()
                    .push((block_idx, usize::MAX));
            }
        }
    }
    // Forward-reachable-from-successors per store block, memoized.
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    'roots: for (r, stores) in &lineage_store_sites {
        let Some(uses) = lineage_uses.get(r) else {
            continue;
        };
        for &(sb, si) in stores {
            let reachable = reachable_cache.entry(sb).or_insert_with(|| {
                let succs: Vec<usize> = func
                    .blocks
                    .get(sb)
                    .map(|b| {
                        crate::graph::successor_block_ids(&b.terminator)
                            .into_iter()
                            .map(crate::ir::ArcBlockId::index)
                            .collect()
                    })
                    .unwrap_or_default();
                forward_reachable(func, &succs)
            });
            let used_after = uses
                .iter()
                .any(|&(ub, ui)| (ub == sb && ui > si) || reachable.contains(&ub));
            if used_after {
                continue 'roots;
            }
        }
        out.insert(*r);
    }
    out
}

/// Forward-reachable block set from `starts` (inclusive), via CFG successors.
fn forward_reachable(func: &ArcFunction, starts: &[usize]) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = starts.to_vec();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    visited
}

/// Blocks that lie inside a natural loop: a block is in a loop iff it is the
/// target of a back-edge `b → h` (an edge whose head `h` dominates its tail `b`)
/// OR it can reach the back-edge tail while dominated by the head. Computed as
/// the union of every back-edge's natural-loop body.
fn compute_loop_blocks(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    dom: &DominatorTree,
) -> FxHashSet<usize> {
    let mut loop_blocks: FxHashSet<usize> = FxHashSet::default();
    let n = func.blocks.len();
    for (b, block) in func.blocks.iter().enumerate() {
        let tail = ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
        for h in crate::graph::successor_block_ids(&block.terminator) {
            // Back-edge: successor `h` dominates the current block `b`.
            if dom.dominates(h, tail) {
                // Natural-loop body of back-edge `b → h`: `h` plus every block
                // that reaches `b` without passing through `h` (the standard
                // backward-reachability from the tail, bounded by the header).
                loop_blocks.insert(h.index());
                loop_blocks.insert(b);
                let mut stack = vec![b];
                while let Some(x) = stack.pop() {
                    if x == h.index() {
                        continue;
                    }
                    for &p in &preds[x] {
                        if p < n && loop_blocks.insert(p) {
                            stack.push(p);
                        }
                    }
                }
            }
        }
    }
    loop_blocks
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

/// Compact each block: retain only non-removed instructions.
fn compact_removed(func: &mut ArcFunction, remove: &[Vec<bool>]) {
    for (block_idx, block) in func.blocks.iter_mut().enumerate() {
        if !remove[block_idx].iter().any(|r| *r) {
            continue;
        }
        let mut idx = 0usize;
        block.body.retain(|_| {
            let keep = !remove[block_idx][idx];
            idx += 1;
            keep
        });
    }
}
