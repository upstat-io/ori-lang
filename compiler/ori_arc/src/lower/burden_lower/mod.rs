//! Phase 5 trivial burden emission walker.
//!
//! Reads each owned non-scalar SSA value's `BurdenSpec` and emits `BurdenInc`
//! at every transfer point + `BurdenDec` at every last-use along every
//! reachable CFG path. Pure per-instruction emission driven by SSA def-use;
//! no global flow analysis, no fixpoint, no lattice consultation.
//!
//! Cluster layout: the analysis-assembly driver (`emit_burden_ops`),
//! `BurdenLowerCtx`, and the collect / detect / filter helpers live here.
//! `terminator` owns terminator-position transfer + inc precompute;
//! `moved_fields` owns the moved-out-fields forward dataflow + full/partial-move
//! partition; `emit` owns the per-instruction + per-terminator emission.

mod cow_aliases;
mod ctx;
mod emit;
mod moved_fields;
mod ownership_scans;
mod scan_helpers;
mod scan_orchestration;
mod sibling_union;
mod terminator;

// The analysis-assembly driver (`emit_burden_ops`) + the per-scan `apply_*`
// wrapper family live in `scan_orchestration`; re-export so consumers continue
// to resolve `burden_lower::emit_burden_ops`.
pub(crate) use scan_orchestration::emit_burden_ops;

pub(crate) use ctx::BurdenLowerCtx;

pub(crate) use cow_aliases::{compute_borrowed_alias_vars, compute_cow_inc_borrowed_aliases};
pub(crate) use ownership_scans::{
    collect_move_edges_and_store_consumes, list_concat_consumed_operands,
};
// Re-exported into `burden_lower` scope so sibling submodules (`emit`,
// `moved_fields`) resolve them via `super::`.
use ownership_scans::{instr_owned_position_transfer_vars, instr_transfer_vars};
// Test-only re-export so the `tests` child resolves it via `super::`.
#[cfg(test)]
use ownership_scans::compute_borrowed_arg_let_aliases;

use std::sync::LazyLock;

use crate::ir::{ArcFunction, ArcVarId};
use rustc_hash::FxHashMap;

/// `(block_idx, pos) -> [release var]` placed-release map produced by the
/// dead-param / fresh-sum / borrowed-`Invoke` lineage scans and merged into the
/// `forwarder_result_releases` emission surface.
type PlacedReleaseMap = FxHashMap<(usize, ownership_scans::ForwarderReleasePos), Vec<ArcVarId>>;

/// `ORI_DISABLE_DEAD_FORWARDER_PARAM_RELEASE=1` skips the Phase-5 RL-5
/// dead-at-entry release for forwarder-identity allocations reaching a merge/return
/// block's dead block-params ([`compute_dead_forwarder_block_param_releases`]).
/// Bisection surface: isolates a leak / double-free to the dead-forwarder-param
/// emission vs the rest of the Phase-5 walk without toggling the whole burden
/// pipeline. Default (unset): the release is emitted. Spec: Annex E §AIMS RL-5.
static DEAD_FORWARDER_PARAM_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_DEAD_FORWARDER_PARAM_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_CONSTRUCT_FED_DEAD_PARAM_RELEASE=1` skips the Phase-5 RL-5
/// dead-at-entry release + spurious-op suppression for a SUM-AGGREGATE-`Construct`-fed
/// allocation reaching a merge/return block's dead block-params via Jump args
/// ([`compute_construct_fed_dead_param_lineage`]). Bisection surface: isolates a leak /
/// double-free to the construct-fed dead-param lineage cure vs the rest of the Phase-5
/// walk. Default (unset): the lineage is suppressed + the single release emitted. Cures
/// the `for x in Some(str) yield { ... }` over-emission (2 spurious keep-alive incs + 1
/// misplaced release → +1 leak). Spec: Annex E §AIMS RL-5 + RL-4 + RL-2.
static CONSTRUCT_FED_DEAD_PARAM_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_CONSTRUCT_FED_DEAD_PARAM_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_FRESH_SUM_LIVE_EXTRACT_RELEASE=1` skips the Phase-5 RL-1 + RL-2
/// same-alloc lineage treatment for a FRESH niche-family sum allocation whose
/// payload is EXTRACTED LIVE through a match (`match result { Some(s) -> s, .. }`)
/// ([`compute_fresh_sum_live_extract_lineage`]). Default (unset): the whole
/// same-alloc closure (the sum root + Let-Var aliases + niche-payload Project
/// views + extract block-params) is removed from `owned_vars_needing_rc`
/// (suppressing the spurious FRESH-result + dup-alias keep-alive incs AND the
/// misplaced arm / last-use releases) and EXACTLY ONE whole-var release is
/// emitted after the closure's final borrow-read. Bisection surface: isolates a
/// match-extract leak / double-free to this treatment vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-1 + RL-2.
static FRESH_SUM_LIVE_EXTRACT_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_FRESH_SUM_LIVE_EXTRACT_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_FORWARDER_RESULT_RELEASE=1` skips the Phase-5 RL-2 scope-exit
/// release for a transfer-through-return forwarder RESULT whose monomorphized
/// result-type burden is empty (`burden_carries_rc == false`), so the result
/// lineage gets neither a FRESH inc nor any scope-exit dec, leaks its
/// transferred-in allocation when consumed only by a borrow-projection then dead
/// ([`compute_forwarder_result_under_release`]). Bisection surface: isolates a leak
/// to the forwarder-result release vs the rest of the Phase-5 walk. Default
/// (unset): the single whole-var release is emitted at the lineage's last use.
/// Spec: Annex E §AIMS RL-2 (`RL2_borrowed_param_emits_caller_dec` +
/// `RL2_release_exactly_once`) + RL-34 (forwarder identity).
static FORWARDER_RESULT_RELEASE_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_FORWARDER_RESULT_RELEASE").as_deref() == Ok("1"));

/// `ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE=1` skips the Phase-5 RL-4 per-edge
/// release for an Owned non-scalar FUNCTION-param that dies crossing a CFG edge without
/// transfer ([`compute_dead_owned_param_branch_releases`]). Shape: a multi-param callee
/// returning ONE param path-sensitively (`triple<T>(c, x, y, z)`) — the non-returned
/// params are dead on the returning branch and the Phase-5 walk emits no release, leaking
/// them. Bisection surface: isolates a leak to the dead-owned-param-branch emission vs the
/// rest of the Phase-5 walk. Default (unset): the single per-edge release is emitted at the
/// dead branch's entry. Spec: Annex E §AIMS RL-4 (`RL4_edge_dec_decision`) + RL-2.
static DEAD_OWNED_PARAM_BRANCH_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP=1` restores the RL-1 duplication
/// classification for `Let { Var(src) }` aliases of a forwarder-identity source —
/// an Owned param of THIS function whose own `MemoryContract` proves
/// `transfers_through_return` with a read-only-or-move-out lineage
/// ([`compute_forwarder_identity_transparent_aliases`]). Default (unset): such an
/// alias is transparent (NO paired alias inc/dec; the moved-through allocation's
/// release accounting stays with the lineage per RL-34), curing the
/// multi-use-then-return forwarder over-release (the per-var DP-2/DP-3 pass splits
/// the spurious pair: DP-3 elides the inc, DP-2 keeps the dec → double-free).
/// Bisection surface: isolates a forwarder-lineage double-free / leak to the
/// alias de-classification vs the rest of the Phase-5 walk.
/// Spec: Annex E §AIMS RL-1 + RL-34 + RL-2.
static FORWARDER_IDENTITY_ALIAS_DEDUP_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP").as_deref() == Ok("1")
});

/// `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1` restores the decoupled treatment of a
/// genuine RL-1 duplication pair: Phase 5 re-suppresses the alias-site `BurdenInc`
/// of a [`compute_genuine_dup_move_aliases`] member (the move-chain inc-suppression
/// symmetry fires as if the alias were a pure move), and Phase 6's per-var
/// DP-2/DP-3 elision reverts to splitting an alias pair (inc elided on `Once ∧
/// (Linear ∨ Affine)`, dec kept). Default (unset): the duplication pair is ATOMIC —
/// Phase 5 keeps the load-bearing inc for an alias consumed at an owned position
/// while its source stays live (the store creates a real second reference,
/// `AimsProof.Realization::RL1_duplication_balanced`), and Phase 6 elides an
/// alias pair only WHOLE (both inc and dec) — curing the
/// genuine-duplication-from-ttr-param double-free (`@stash_and_return`: alias
/// stored into a struct while the source is read after and returned). Bisection
/// surface: isolates a duplication-lineage double-free / leak to the pair
/// coupling vs the rest of the Phase-5 walk + Phase-6 elimination.
/// Spec: Annex E §AIMS RL-1 + RL-2.
static GENUINE_DUP_PAIR_COUPLING_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING").as_deref() == Ok("1"));

/// `ORI_DISABLE_TTR_ITER_CONSUME_DUP_INC=1` restores the RL-1 inc-suppression for
/// a `Let { Var }` alias of an Owned `transfers_through_return` param that is
/// iter-consumed (`for x in xs` -> `@iter [own]` -> `ori_iter_drop`) — the
/// move-chain symmetry suppresses the alias inc as if the iter-consume were a
/// pure move. Default (unset): the iter-consumed alias of a ttr param KEEPS its
/// duplication `BurdenInc` ([`compute_ttr_iter_consume_dup_aliases`]) — the
/// iterator frees the duplicate via `ori_iter_drop` (RL-2
/// `ApplyToIterConsumingParam` transfer) while the param's original reference
/// transfers out through the `Return`. Without the kept inc the single
/// allocation is freed once by the iterator AND transferred out via Return =
/// double-free (the `borrow_*_for_yield_then_return` family). Bisection surface:
/// isolates a for-yield-then-return double-free to this inc vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-1 + RL-2 + RL-34.
static TTR_ITER_CONSUME_DUP_INC_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_TTR_ITER_CONSUME_DUP_INC").as_deref() == Ok("1"));

/// Read-only accessor for the `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING` toggle —
/// consumed by Phase 5 here and by the Phase-6 per-var elision
/// (`aims::realize::burden_elim`) so both halves of the pair-coupling cure flip
/// on ONE switch.
pub(crate) fn genuine_dup_pair_coupling_disabled() -> bool {
    *GENUINE_DUP_PAIR_COUPLING_DISABLED
}

/// `ORI_DISABLE_BORROWED_STORE_DUP_INC=1` suppresses the Phase-5 RL-1
/// duplication `BurdenInc` for a BORROWED-param-rooted value consumed at an
/// aggregate-STORE position (`Construct` / `Reuse` / `CollectionReuse` arg,
/// `Set.value`). Default (unset): the inc is emitted — the caller retains its
/// reference for the whole call (RL-2 borrowed-param caller-dec), so the store
/// creates a real SECOND reference whose matched release is the container's
/// drop (`AimsProof.Realization::RL1_duplication_balanced`); without the inc
/// the container drop releases a reference no inc supplied (use-after-free on
/// the caller's still-live value). Bisection surface: isolates a
/// borrowed-store lineage use-after-free / leak to this inc vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-1 + RL-2.
static BORROWED_STORE_DUP_INC_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BORROWED_STORE_DUP_INC").as_deref() == Ok("1"));

/// Read-only accessor for the `ORI_DISABLE_BORROWED_STORE_DUP_INC` toggle —
/// consumed by the Phase-5 borrowed-store duplication scan
/// (`ownership_scans::compute_borrowed_store_dup_args`).
pub(in crate::lower::burden_lower) fn borrowed_store_dup_inc_disabled() -> bool {
    *BORROWED_STORE_DUP_INC_DISABLED
}

/// `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1` restores the decoupled
/// DP-2/DP-3 split for `Let { Var }` alias dsts whose alias-chain root is a
/// LOCAL fresh `Construct`. Default (unset): such alias pairs are ATOMIC in
/// the Phase-6 per-var elision (`aims::realize::burden_elim`) — elided WHOLE
/// or kept WHOLE, never split — because an alias definition carries no birth
/// `+1`: its `BurdenInc` IS the duplication's `+1` on the root's allocation,
/// so eliding the inc while keeping the dec releases the root's still-live
/// reference (net -1 over-release double-free; the local terminal-move-store
/// shape). Apply / Invoke RESULT-rooted aliases stay on the decoupled split
/// (load-bearing compensation for under-emissions, each cured in its own
/// cycle). Bisection surface: isolates a local-rooted alias-lineage
/// double-free / leak to the pair coupling vs the rest of the elimination.
/// Spec: Annex E §AIMS RL-1 + RL-2.
static LOCAL_CONSTRUCT_PAIR_COUPLING_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING").as_deref() == Ok("1")
});

/// Read-only accessor for the `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING`
/// toggle — consumed by the Phase-6 per-var elision
/// (`aims::realize::burden_elim::collect_pair_atomic_alias_dsts`).
pub(crate) fn local_construct_pair_coupling_disabled() -> bool {
    *LOCAL_CONSTRUCT_PAIR_COUPLING_DISABLED
}

use super::burden::{Burden, BurdenRef};

/// True iff `burden` carries any RC-tracked dimension. Used by the filter at
/// `emit_burden_ops` to exclude scalars whose `lookup_burden` returns the empty
/// builtin burden. Defends VF-1 `RcOnScalar` invariant.
///
/// TYPE-level only: a burden with variant entries (e.g. an all-scalar-payload
/// sum type) passes this filter even when the concrete var's monomorphized
/// repr is `Scalar` (niche-packed). Per-var admission pairs this with
/// [`is_provably_scalar_repr`].
pub(super) fn burden_carries_rc(burden: &BurdenRef<'_>) -> bool {
    burden.self_heap_alloc()
        || burden.element_burden().is_some()
        || burden.variant_burdens().next().is_some()
        || burden.owned_fields().next().is_some()
}

/// `ORI_DISABLE_SCALAR_REPR_BURDEN_SKIP=1` restores the legacy TYPE-level-only
/// burden admission: vars whose monomorphized repr is `Scalar` receive
/// whole-var burden ops again (which can never lower and survive as VF-1
/// ledger residue). Bisection surface: isolates a gated-verification change to
/// the repr-aware admission vs the rest of the Phase-5 walk. Default (unset):
/// provably-Scalar-repr vars are excluded from burden admission.
/// Spec: Annex E §AIMS L-9.
static SCALAR_REPR_BURDEN_SKIP_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_SCALAR_REPR_BURDEN_SKIP").as_deref() == Ok("1"));

/// True iff `var`'s monomorphized machine repr is PROVABLY `Scalar` per the
/// pipeline Step-3 `compute_var_reprs` classification (`func.var_reprs`) — the
/// SAME repr source the Phase-7 burden lowering consults.
///
/// A Scalar-repr value (a primitive, or a niche-packed all-scalar-payload sum
/// instantiation) carries NO RC header: the scalar sentinel is excluded from
/// the state map entirely and whole-var burden ops on it can never lower to
/// RC — they would survive as VF-1 ledger residue. Admission skips ONLY the
/// provable case: `None` (`var_reprs` unpopulated) and every non-`Scalar` repr
/// KEEP the admission (over-admitting a scalar is ledger residue;
/// under-admitting a heap value is a missing release — a leak).
/// Spec: Annex E §AIMS L-9 (scalar sentinel exclusion) + TF-1/TF-2a.
pub(super) fn is_provably_scalar_repr(func: &ArcFunction, var: ArcVarId) -> bool {
    if *SCALAR_REPR_BURDEN_SKIP_DISABLED {
        return false;
    }
    matches!(func.var_repr(var), Some(crate::ir::ValueRepr::Scalar))
}

/// Set `emitted[idx] = true` when `idx` is in bounds; out-of-bounds is a no-op.
fn mark_emitted(emitted: &mut [bool], idx: usize) {
    if let Some(slot) = emitted.get_mut(idx) {
        *slot = true;
    }
}

#[cfg(test)]
mod tests;
