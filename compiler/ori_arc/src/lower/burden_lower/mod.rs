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

mod chain_root;
mod cow_aliases;
mod ctx;
mod emit;
mod extract_transfer;
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
    collect_move_edges_and_store_consumes, compute_funded_call_arg_dup_aliases,
    compute_funded_store_dup_aliases, contract_consuming_arg_position,
    list_concat_consumed_operands, store_family_funding_disabled,
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

/// `ORI_DISABLE_MULTI_EXIT_BORROW_VIEW_RELEASE=1` skips the Phase-5 RL-1 / RL-2
/// / RL-4 multi-exit borrow-view treatment for a FRESH closure (`Apply`/`Invoke`
/// result, `Unique` + `preserves_freshness`, non-forwarder) consumed ONLY at
/// `ApplyIndirect` borrow-receiver sites across a short-circuit CFG
/// ([`compute_multi_exit_borrow_view_lineage`]). Default (unset): the whole
/// same-alloc closure (root + Let-Var aliases) is removed from
/// `owned_vars_needing_rc` (suppressing every dup/keep-alive inc + the spurious
/// pre-use source dec) and EXACTLY ONE whole-var release is placed per terminal
/// path — an RL-2 dec after the execution-final borrow-read on each READING
/// path, an RL-4 edge dec on each SHORT-CIRCUIT-BYPASS successor entry.
/// Bisection surface: isolates the short-circuit closure-borrow double-free to
/// this treatment vs the per-var DP-2/DP-3 split. Spec: Annex E §AIMS RL-1
/// (`RL1_duplication_balanced`) + RL-2 (`RL2_release_exactly_once`) + RL-4
/// (`RL4_edge_dec_decision`).
static MULTI_EXIT_BORROW_VIEW_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_MULTI_EXIT_BORROW_VIEW_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_RETAIN_ALIASING_RELEASE=1` skips the Phase-5 RL-1 + RL-2 + RL-4
/// retain-aliasing treatment: a FRESH niche-family sum (`@__index` Option result)
/// whose payload is read through an accessor-retain call (`@unwrap`/`@get`, a
/// per-hop self-inc on the SAME allocation) across a SHORT-CIRCUIT branchy CFG.
/// The whole same-alloc closure is removed from `owned_vars_needing_rc` and a
/// per-path release is placed for EACH retained reference (the niche-sum root +
/// each accessor-retain result). Bisection surface: isolates the branchy
/// retain-aliasing double-free to this treatment. Spec: Annex E §AIMS RL-1 + RL-2
/// + RL-4 + TF-4.
static RETAIN_ALIASING_RELEASE_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_RETAIN_ALIASING_RELEASE").as_deref() == Ok("1"));

/// `ORI_DISABLE_CLOSURE_EXTRACT_BORROW_VIEW_RELEASE=1` skips the Phase-5 RL-2 /
/// RL-4 closure-extract borrow-view treatment for `ApplyIndirect` results that
/// are PROVEN same-allocation borrow-views of ONE captured field of a closure env
/// (the resolved lambda's `return_alias = Project { field }` contract over its
/// capture param — `let f = () -> { match captured { Err(e) -> e, .. } }`
/// invoked N times) ([`compute_closure_extract_borrow_view_lineage`]). Default
/// (unset): the whole result lineage (every `ApplyIndirect` result + its Let-Var
/// aliases projecting the SAME captured field) is removed from
/// `owned_vars_needing_rc` (suppressing the N surplus per-result decs) and
/// EXACTLY ONE whole-var release is placed per terminal path (RL-2 after the
/// execution-final borrow-read; RL-4 edge dec on a dead normal-successor entry).
/// With the toggle set, the base walk's per-result dec returns (the captured
/// payload double-frees on call 2, exit 134). Spec: Annex E §AIMS RL-2
/// (`RL2_release_exactly_once`, the joint-release theorem) + RL-4
/// (`RL4_edge_dec_decision`) + TF-4 (`Project` borrow-view identity).
static CLOSURE_EXTRACT_BORROW_VIEW_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_CLOSURE_EXTRACT_BORROW_VIEW_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_LOOP_CLOSURE_DEAD_PARAM_RELEASE=1` skips the Phase-5 RL-5
/// dead-at-entry treatment for a FRESH `PartialApply` closure threaded through a
/// loop and dead at the post-loop block-param
/// ([`ownership_scans::compute_loop_closure_dead_param_lineage`]). Default
/// (unset): the whole same-alloc closure lineage (root + `Let`-Var aliases +
/// `Jump`-arg-threaded loop block-params) is removed from
/// `owned_vars_needing_rc` (suppressing the spurious per-iteration
/// borrow-receiver `BurdenDec` of the direct-`ApplyIndirect` shape + the net-0
/// keep-alive pairs) and EXACTLY ONE whole-var `BurdenDec` is placed at the dead
/// post-loop block-param entry. With the toggle set, the base walk's emission
/// returns — a missing release (the borrowed-arg HOF shape leaks the closure
/// env) or a per-iteration unmatched dec (the direct-call FM shape double-frees
/// the loop-invariant closure, exit −134). Bisection surface: isolates a
/// loop-carried-closure leak / double-free to this treatment vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-5 (`RL5_dead_at_entry_cleanup` +
/// `RL5_cleanup_balanced`) + RL-2 + RL-4.
static LOOP_CLOSURE_DEAD_PARAM_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_LOOP_CLOSURE_DEAD_PARAM_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_LAZY_ITER_CLOSURE_BORROW_RELEASE=1` skips the Phase-5 RL-2
/// treatment for a FRESH `PartialApply` closure borrowed into a lazy-iterator
/// builtin (`@map` / `@filter`) whose result iterator retains the closure env as
/// a borrowed raw pointer across the chain's terminal consumer
/// ([`ownership_scans::compute_lazy_iter_closure_borrow_lineage`]). Default
/// (unset): the whole same-alloc closure lineage (root + `Let`-Var aliases) is
/// removed from `owned_vars_needing_rc` (suppressing the early borrowed-arg
/// `BurdenDec` the base walk places at the `@map`/`@filter` call) and EXACTLY ONE
/// whole-var `BurdenDec` is placed at the lazy-iterator chain's terminal
/// consumer's (`@collect`/…) normal-successor entry. With the toggle set, the
/// base walk's early dec returns — the env (and its cascade-freed captured heap
/// payload) is freed while the lazy iterator still holds the borrowed env
/// pointer, a use-after-free when the closure runs at `@collect` time (exit −139).
/// `@fold` is EAGER (the closure runs synchronously inside `@fold`) so it survives
/// the identical early dec and is excluded. Bisection surface: isolates a lazy-HOF
/// closure-borrow UAF to this treatment vs the rest of the Phase-5 walk. Spec:
/// Annex E §AIMS RL-2 (`RL2_borrowed_param_emits_caller_dec` +
/// `RL2_release_exactly_once`) + TF-4 + TF-7.
static LAZY_ITER_CLOSURE_BORROW_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_LAZY_ITER_CLOSURE_BORROW_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_ITER_CONSUME_DEAD_THREAD_ORPHAN_INC=1` skips the Phase-5 RL-1 +
/// RL-2 orphaned-inc elision for a fresh collection iter-consumed by an inline
/// `for`-loop whose loop-carried thread is dead post-loop
/// ([`ownership_scans::compute_iter_consume_dead_thread_orphan_inc`]). Default
/// (unset): a fresh `Construct` collection whose SOLE genuine consume is the
/// for-loop's `@iter [own]` (`Invoke @iter` terminator / `Apply @iter`,
/// `ori_iter_drop` frees it) AND whose Jump-arg thread across the loop back-edge
/// terminates in a DEAD param (never read) is removed from
/// `owned_vars_needing_rc` — eliding the orphaned FRESH-site keep-alive
/// `BurdenInc` (the dead-thread is a `JumpArg` transfer into a dead param, NOT a
/// genuine duplication, so the value is move-once into `@iter`:
/// `RL1_duplication_balanced` move-once nets 0). With the toggle set, the orphan
/// inc returns — the buffer's rc never reaches 0 (the `@iter` consume suppresses
/// the scope-exit dec), a +1 leak (exit 2). Follows Jump-args across ALL edges
/// (forward + back) via `compute_param_edge_args` — the back-edge inclusion the
/// foreclosed forward-only scans (dead-ends #163/#164/#185) lacked; does NOT
/// relax the `@iter` COW-taint (#181's over-fire surface). Spec: Annex E §AIMS
/// RL-1 + RL-2.
static ITER_CONSUME_DEAD_THREAD_ORPHAN_INC_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_ITER_CONSUME_DEAD_THREAD_ORPHAN_INC").as_deref() == Ok("1")
});

/// `ORI_DISABLE_NESTED_CONSTRUCT_RETURN_PASSTHROUGH=1` skips the Phase-5 RL-2
/// in-callee container-passthrough suppression for a fresh nested struct/tuple
/// `Construct` chain whose deepest projection IS the function's Return value,
/// wrapping a transfers-through-return param
/// ([`ownership_scans::compute_nested_construct_return_passthrough`]). Default
/// (unset): a param moved into a chain of fresh struct/tuple `Construct`s and
/// projected back out (`Project (Construct args) field == args[field]`) as the
/// Return value has the whole wrapper-chain lineage removed from
/// `owned_vars_needing_rc` — eliding the surplus container drops that would free
/// the transferred-out param (the wrapper owns nothing surviving;
/// `wrapper_passthrough_release_exactly_once` nets 0). With the toggle set, the
/// container drops return — the outermost wrapper's transitive drop frees the
/// returned nested field, a use-after-free (exit -139). Pairs with the
/// caller-side construct-project round-trip `Direct` return-alias. Spec: Annex E
/// §AIMS RL-2 + TF-3 + TF-4.
static NESTED_CONSTRUCT_RETURN_PASSTHROUGH_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_NESTED_CONSTRUCT_RETURN_PASSTHROUGH").as_deref() == Ok("1")
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

/// `ORI_DISABLE_SUM_PAYLOAD_ITER_CONSUME_DUP_INC=1` restores the surplus
/// duplication `BurdenInc` on a `Let { Var }` dup-alias of a fresh niche-family
/// sum-aggregate whose extracted payload is iter-consumed
/// ([`ownership_scans::compute_sum_payload_iter_consume_dup_inc_suppression`]).
/// Default (unset): the buffer is born once (`Construct List` MOVED by-value
/// into `Construct Variant(Option.0)`), so the by-value aggregate's Let-Var
/// dup-alias adds NO owner and its inc is RL-1 move-once-elidable
/// (`AimsProof.Realization::RL1_duplication_balanced`, `incElidable = true`); the
/// suppression strips the surplus inc the `use_counts >= 2` proxy mis-emits. On
/// the live-extract path the payload (`Project alias.1`) transfers out via
/// `@iter [own]` whose `ori_iter_drop` releases it (RL-2
/// `ApplyToIterConsumingParam`, no caller dec); the kept construct-site inc is
/// released by the Some-arm scope-exit dec, the dead None / Unreachable arms
/// release birth + construct-site inc. With the toggle set the surplus dup-alias
/// inc returns (the `match m { Some(words) -> for w in words .. }` over
/// `Option<[str]>` leaks the buffer + its str elements, the
/// `str_list_iteration_in_match` shape). Bisection surface: isolates a
/// sum-payload-iter-consume leak to this suppression vs the rest of the Phase-5
/// walk. Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.
static SUM_PAYLOAD_ITER_CONSUME_DUP_INC_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_SUM_PAYLOAD_ITER_CONSUME_DUP_INC").as_deref() == Ok("1")
});

/// `ORI_DISABLE_CROSS_BLOCK_FINAL_USE_CANCEL=1` restores the single-block
/// `last_use_points == 1` over-approximation in the dup'd terminal-move gate
/// ([`ownership_scans::compute_transfer_via_move_alias`]). Default (unset): a
/// dup'd CROSS-BLOCK move source whose `Let { Var }` alias is its proven global
/// final use (successor-reachability final-use proof; loop back-edge re-use
/// declines; an alternative-arm sibling use — a non-terminal use whose block
/// does NOT dominate the terminal block — declines) joins the transfer
/// fixpoint, cancelling the pending last-use
/// release when the alias hands off at an owned position (Construct arg /
/// `[own]` call arg / Return) per `AimsProof.Realization::
/// RL2_transfer_kinds_no_dec`. With the toggle set, the post-Construct release
/// returns (double-free on the dup-inc'd pair-return lineage — the
/// `(base, fork)` COW shape). Bisection surface: isolates a pair-return
/// double-free / leak to the relaxed gate vs the rest of the Phase-5 walk.
/// Spec: Annex E §AIMS RL-2.
static CROSS_BLOCK_FINAL_USE_CANCEL_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_CROSS_BLOCK_FINAL_USE_CANCEL").as_deref() == Ok("1")
});

/// Read-only accessor for the `ORI_DISABLE_CROSS_BLOCK_FINAL_USE_CANCEL`
/// toggle — consumed by the move-alias transfer scan
/// (`ownership_scans::move_alias`).
pub(in crate::lower::burden_lower) fn cross_block_final_use_cancel_disabled() -> bool {
    *CROSS_BLOCK_FINAL_USE_CANCEL_DISABLED
}

/// `ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS=1` restores the kept
/// surplus same-allocation scope-exit dec on a use-once owned source whose sole
/// Let{Var} alias is a same-allocation borrow-view live downstream
/// ([`ownership_scans::compute_transfer_via_move_alias`] borrow-view-dst arm).
/// Default (unset): the source's dec is suppressed (the lineage's release is the
/// edge-cleanup dec keyed on the shared `genuine_same_alloc_reps` rep) per
/// RL-2 release-once — the forwarder-result `%6 = %4` keystone. With the toggle
/// set, the surplus dec returns (double-free on the forwarder-result lineage).
/// Spec: Annex E §AIMS RL-2.
static BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS").as_deref() == Ok("1")
});

/// Read-only accessor for the `ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS`
/// toggle — consumed by the move-alias transfer scan
/// (`ownership_scans::move_alias`).
pub(in crate::lower::burden_lower) fn borrow_view_dst_surplus_dec_suppress_disabled() -> bool {
    *BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS_DISABLED
}

/// `ORI_DISABLE_OWNER_DROP_BORROW_VIEW_LIVENESS=1` restores the syntactic
/// last-use placement of an owned aggregate's whole-var `BurdenDec`, ignoring
/// the downstream liveness of its `Project` borrow-views. Default (unset): an
/// aggregate `%agg` whose `Project` borrow-view (or a `Let{Var}` alias of one)
/// is read AFTER `%agg`'s own syntactic last use has its whole-var release
/// PLACEMENT extended to the borrow-view's last use — the owner's drop (which
/// cascade-frees the projected field) must not precede a borrow-read of that
/// field (TF-14 `TF14_propagation_spec`: a Project's source liveness joins the
/// view's liveness; RL-2 `RL2_release_exactly_once`: the owner drop is the
/// field's single release). With the toggle set, the owner drop lands at the
/// aggregate's syntactic last use (use-after-free on a borrow-view read past
/// the owner's drop — `let xs = c.items; xs.fold(..)`). Same-allocation-identity
/// gated on the structural `Project { value: agg }` borrow-view edge (NOT a
/// use-count / type-membership proxy). Spec: Annex E §AIMS TF-14 + RL-2.
static OWNER_DROP_BORROW_VIEW_LIVENESS_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_OWNER_DROP_BORROW_VIEW_LIVENESS").as_deref() == Ok("1")
});

/// Read-only accessor for the `ORI_DISABLE_OWNER_DROP_BORROW_VIEW_LIVENESS`
/// toggle — consumed by the last-use detection extension
/// (`ownership_scans::walk::extend_owner_last_use_for_borrow_views`).
pub(in crate::lower::burden_lower) fn owner_drop_borrow_view_liveness_disabled() -> bool {
    *OWNER_DROP_BORROW_VIEW_LIVENESS_DISABLED
}

/// `ORI_DISABLE_PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS=1` restores the surplus
/// same-allocation dec on a use-once owned source `%s` (`arg`) whose SOLE owned
/// RC field is projected-returned by a borrowed-receiver callee to an OWNED
/// caller result (`ApplyAliasSource::Project { arg, field }`). Default (unset):
/// `%s`'s own scope-exit dec is suppressed — the owned result carries the joint
/// lineage's single release at its true last use (RL-2 release-once), so `%s`'s
/// premature field-drop is the surplus. W/ the toggle set, the base walk's
/// double-release returns (the joint borrow-projection double-free, exit 134).
/// Bisection surface: isolates the `edge_project_return_not_param` double-free to
/// this arm vs the rest of the Phase-5 walk. Spec: Annex E §AIMS RL-2 + TF-4.
static PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS").as_deref() == Ok("1")
});

/// Read-only accessor for the
/// `ORI_DISABLE_PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS` toggle — consumed by
/// the move-alias transfer scan (`ownership_scans::move_alias`).
pub(in crate::lower::burden_lower) fn project_return_surplus_owner_dec_suppress_disabled() -> bool {
    *PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS_DISABLED
}

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

/// `ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING=1` restores the decoupled
/// DP-2/DP-3 split for `Let { Var }` alias dsts whose alias-chain root is a
/// loop-carried BLOCK-param (a block-param carrying a back-edge per the
/// per-predecessor-edge attribution). Default (unset): such alias pairs are
/// ATOMIC in the Phase-6 per-var elision — an alias of the loop's threaded
/// value is a same-allocation view, never a birth site, so splitting its
/// inc/dec pair (DP-3 elides the inc on the alias's own `Once` view, DP-2
/// keeps the dec) releases the loop lineage's still-live reference (net -1 —
/// the branch-exclusive read-arm double-free). Bisection surface: isolates a
/// loop-carried borrow-alias double-free to the pair coupling vs the rest of
/// the elimination. Spec: Annex E §AIMS RL-1 + RL-2.
static LOOP_CARRIED_PAIR_COUPLING_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING").as_deref() == Ok("1"));

/// Read-only accessor for the `ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING`
/// toggle — consumed by the Phase-6 per-var elision
/// (`aims::realize::burden_elim::collect_pair_atomic_alias_dsts`).
pub(crate) fn loop_carried_pair_coupling_disabled() -> bool {
    *LOOP_CARRIED_PAIR_COUPLING_DISABLED
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
