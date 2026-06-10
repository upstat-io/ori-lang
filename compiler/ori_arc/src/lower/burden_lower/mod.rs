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
mod sibling_union;

pub(crate) use ownership_scans::collect_move_edges_and_store_consumes;
mod terminator;

pub(crate) use ctx::BurdenLowerCtx;

pub(crate) use cow_aliases::{compute_borrowed_alias_vars, compute_cow_inc_borrowed_aliases};
use cow_aliases::{compute_cow_inc_and_mutators, compute_scalar_literal_vars};
pub(crate) use ownership_scans::list_concat_consumed_operands;
use ownership_scans::{
    collect_owned_burdens, compute_borrowed_arg_let_aliases, compute_borrowed_projection_dsts,
    compute_borrowed_store_dup_args, compute_borrowed_terminator_invoke_args,
    compute_construct_fed_dead_param_lineage, compute_dead_forwarder_block_param_releases,
    compute_dead_owned_param_branch_releases, compute_forwarder_identity_transparent_aliases,
    compute_forwarder_result_under_release, compute_fresh_sum_live_extract_lineage,
    compute_genuine_dup_move_aliases, compute_live_out_owned, compute_owned_vars_needing_rc,
    compute_transfer_through_return_param_vars, compute_transfer_through_return_results,
    compute_transfer_via_move_alias, compute_ttr_iter_consume_dup_aliases,
    compute_use_counts_and_dup_aliases, detect_last_uses, detect_transfer_points,
    group_last_uses_filtered,
};
use ownership_scans::{instr_owned_position_transfer_vars, instr_transfer_vars};

use std::sync::LazyLock;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::ownership::DerivedOwnership;
use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

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

use emit::{emit_burden_ops_for_blocks, BurdenAnalysisCtx};
use moved_fields::{compute_full_move_vars, compute_partial_move_vars, populate_moved_out_fields};
use terminator::{compute_terminator_inc_per_block, compute_terminator_transfer_per_block};

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

/// Walk `func` and emit `BurdenInc` / `BurdenDec` ops per SSA variable from
/// `BurdenSpec` lookups, filtered to owned positions via `DerivedOwnership`.
///
/// Invoked from the AIMS pipeline at Phase 5 (ARC lowering); see
/// `pipeline/aims_pipeline/`.
#[expect(
    clippy::too_many_lines,
    reason = "single Phase-5 burden-emission orchestration: the owned-burden \
              collection, transfer-point detection, last-use detection, the \
              suppression-set computations (borrowed-alias / move-alias / \
              COW-inc / terminator-borrowed-arg), and the per-block emit walk \
              are one cohesive pass; splitting mid-sequence fragments the \
              load-bearing emission order"
)]
pub(crate) fn emit_burden_ops<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    // Block-param ownership lookup for Jump-to-Owned-param transfer detection.
    // DerivedOwnership side-table threaded as typed pre-pass input — slice
    // indexed by ArcVarId::raw() matches infer_derived_ownership() return shape.
    // Empty &[] semantically safe — out-of-bounds defaults to Owned. AIMS
    // Invariant 5 (unified model) preserved — DerivedOwnership is existing
    // analysis output, not a parallel ownership tracker.
    derived_ownership: &[DerivedOwnership],
    // Per-function MemoryContracts from interprocedural analysis. Consumed by
    // FRESH-site BurdenInc emission for Apply/Invoke whose callee
    // `ReturnContract.uniqueness ∈ {Unique, MaybeShared}` (i.e., return value
    // is a FRESH allocation owned by caller). AIMS Invariant 5 — read
    // unchanged, no parallel emission.
    // Immortal var bitvector (`detect_immortals`): empty-string literals carry
    // no RC, so they receive NO burden ops (the predicate-stack emits none) —
    // else the FRESH-site inc orphans (VF-1 net=+1). Tests pass `&[]`.
    immortals: &[bool],
    contracts: &FxHashMap<Name, MemoryContract>,
    // When true (`ORI_DISABLE_PREDICATE_STACK_RC=1`), the predicate-stack edge
    // cleanup is OFF, so the burden walk is the sole RC emitter. The
    // borrowed-Invoke-arg scope-exit `BurdenDec` (normally deferred to the
    // predicate stack's `release_with_burden_edge`) MUST be emitted by the
    // burden walk instead — the completeness pass per `emit.rs`
    // `emit_terminator_burden_decs`. On the default path (false) the dec stays
    // deferred so the two paths do not double-count.
    predicate_stack_rc_disabled: bool,
    // String interner — threaded so the iterator-element exclusion can resolve
    // the `__iter_next` protocol-builtin name via `collect_iter_element_defs`
    // (`Spec: Annex E §AIMS Protocol Builtins`). The SSOT element-classification
    // set is the predicate stack's `collect_iter_element_defs`; the burden walk
    // consumes it (AIMS Invariant 5 — no parallel iterator-element tracker).
    interner: &ori_ir::StringInterner,
) -> BurdenLowerCtx<'a> {
    let mut ctx = BurdenLowerCtx::new(func);
    collect_owned_burdens(&mut ctx, func, type_registry);
    detect_transfer_points(&mut ctx, func, type_registry);
    detect_last_uses(&mut ctx, func);

    // `owned_vars_needing_rc` filters scalars whose `lookup_burden` returns
    // `Some(BurdenRef)` wrapping the empty builtin burden — required by AIMS
    // DP-1 (`is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar`) + VF-1 `RcOnScalar`.
    let mut owned_vars_needing_rc = compute_owned_vars_needing_rc(&ctx);
    {
        let collected: Vec<(u32, String, bool)> = ctx
            .collected
            .iter()
            .map(|(v, b)| {
                let ty = func
                    .var_types
                    .get(v.index())
                    .map(|i| format!("{i:?}"))
                    .unwrap_or_default();
                (
                    u32::try_from(v.index()).unwrap_or(u32::MAX),
                    ty,
                    b.is_some(),
                )
            })
            .collect();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?collected,
            "burden ctx.collected (var, has_burden, carries_rc)"
        );
        let mut initial: Vec<u32> = owned_vars_needing_rc
            .iter()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        initial.sort_unstable();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?initial,
            "burden owned-vars INITIAL (collected, pre-retain)"
        );
    }
    // L-9 repr-aware admission gate: a var whose MONOMORPHIZED repr is provably
    // `Scalar` (a niche-packed all-scalar-payload sum instantiation — the
    // `burden_carries_rc` TYPE-level filter admits its variant entries while
    // the concrete value carries no RC header) gets NO whole-var burden ops:
    // Phase-7 lowering cannot rewrite them (`RcStrategy::from_repr` rejects
    // `Scalar`) and every admitted op survives as VF-1 ledger residue
    // (net=-1 per exit path). Consult the SAME `var_reprs` classification the
    // lowering consults; skip ONLY the provable case ([`is_provably_scalar_repr`]).
    // Spec: Annex E §AIMS L-9 + RE-2.
    owned_vars_needing_rc.retain(|v| !is_provably_scalar_repr(func, *v));
    // Exclude immortals (empty-string literals) — no RC, so no burden ops at all.
    owned_vars_needing_rc.retain(|v| !immortals.get(v.index()).copied().unwrap_or(false));
    // Exclude borrowed-derived locals: a `Let { Var(src) }` alias of a borrowed
    // value is itself a borrow (TF-2 propagates the source's Access; a borrowed
    // source yields a borrowed alias). Borrowed values carry NO RC obligation
    // (RL-2: "Borrowed variables do NOT receive decs"), so an alias of a
    // borrowed param MUST get no burden ops — `collect_owned_burdens` filters
    // borrowed PARAMS but a local alias of one is not a param and slips through,
    // producing an orphan last-use BurdenDec (VF-1 net=-1). Propagate the
    // borrow forward through every Let-Var hop to a fixpoint and exclude the set.
    let borrowed_aliases = compute_borrowed_alias_vars(func);
    owned_vars_needing_rc.retain(|v| !borrowed_aliases.contains(v));
    // A `Let { Var(src) }` alias whose sole use is a BORROWED terminator-Invoke
    // arg is a borrow-view of an owned source: per RL-1 the dup-inc is
    // Owned-param-only, so `f(x, x)` over Borrowed params creates no reference at
    // either arg — the owned source carries the inc+release, the alias gets
    // neither (else the source's FRESH inc orphans, VF-1 net=+1 leak).
    let borrowed_arg_aliases = compute_borrowed_arg_let_aliases(func);
    owned_vars_needing_rc.retain(|v| !borrowed_arg_aliases.contains(v));
    // RL-2 / TF-4: a `Project` dst used only at borrow positions is a borrow-view
    // of the parent aggregate's field — the parent owns + drops the field via
    // whole-var drop-glue, so the borrowed projection gets NO dec. Pairs with the
    // generic-user-struct burden composition (`compose_for_idx`) that makes the
    // owning aggregate carry RC: without the parent drop, excluding the projection
    // would leak; without this exclusion, both would dec and double-free.
    let borrowed_projection_dsts = compute_borrowed_projection_dsts(func);
    owned_vars_needing_rc.retain(|v| !borrowed_projection_dsts.contains(v));
    // Exclude scalar-`Literal`-defined vars: a var whose definition is a
    // `Let { value: Literal(lit) }` with `lit != String` is a scalar sentinel
    // (`Int`/`Float`/`Bool`/`Char`/`Duration`/`Size`/`Unit`/`Null`) carrying NO
    // RC burden regardless of its declared `var_types[v]` (`Spec: Annex E §AIMS
    // L-9` scalar exclusion; TF-1 `Let { Literal } -> SCALAR`). `collect_owned_burdens`
    // keys membership on the declared TYPE, so a var typed as a heap aggregate but
    // defined `Literal(Int(0))` (the `__iter_next` element-type-marker scratch slot)
    // is over-collected and receives an unbalanced `BurdenDec` the inc side never
    // emitted (`fresh_site_burden_inc_dst` emits an inc ONLY for `Literal::String`).
    // The exclusion restores INC/DEC symmetry on the DEFINITION grain.
    let scalar_literal_vars = compute_scalar_literal_vars(func);
    owned_vars_needing_rc.retain(|v| !scalar_literal_vars.contains(v));
    // Exclude iterator-element borrow-views: a `Project { field: 1 }` of an
    // `Apply @__iter_next` result (and its Let/Project/block-param closure) is
    // a BORROWED view into the collection buffer (`Spec: Annex E §AIMS Protocol
    // Builtins`: `IterNext` yields a borrowed element-type marker; the element
    // itself is owned by the collection, freed by `elem_dec_fn` when the
    // collection / iterator handle drops via `ori_iter_drop` / `CollectSet`).
    // The burden walk classifies such a `[str]`/`str` projection as owned (its
    // declared `var_types` carries RC burden) and would emit a last-use
    // `BurdenDec` — a double-free under the standalone ledger (the element view
    // owns no allocation). `compute_borrowed_alias_vars` only source-gates on
    // borrowed PARAMS, and the `__iter_next` result is a scalar handle (not a
    // borrowed param), so the projection slips through. Consume the predicate
    // stack's `collect_iter_element_defs` SSOT (AIMS Invariant 5 — no parallel
    // iterator-element tracker) to exclude the element-view lineage.
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    owned_vars_needing_rc.retain(|v| !iter_element_defs.contains(v));
    // RL-1 + RL-34 forwarder-identity alias transparency: a `Let { Var(src) }`
    // alias of an Owned param that transfers through the return (per THIS
    // function's own contract) with a read-only-or-move-out lineage is a
    // same-allocation view of the moved-through value — NOT a duplication. It
    // carries NO burden ops: no dup-alias classification (skip in
    // `compute_use_counts_and_dup_aliases`) and no membership here (no
    // last-use dec, no terminator inc/dec pair) — the lineage's single
    // transferred-in reference is released by the CALLER of the bound result.
    // Keeping the classification emits a paired alias inc/dec the per-var
    // DP-2/DP-3 pass splits (inc elided, dec kept) → over-release double-free
    // on the multi-use-then-return forwarder. SSOT + the all-or-nothing
    // conservative vetting: `compute_forwarder_identity_transparent_aliases`.
    let forwarder_identity_transparent_aliases = if *FORWARDER_IDENTITY_ALIAS_DEDUP_DISABLED {
        FxHashSet::default()
    } else {
        compute_forwarder_identity_transparent_aliases(func, contracts)
    };
    owned_vars_needing_rc.retain(|v| !forwarder_identity_transparent_aliases.contains(v));
    // RL-5 + RL-2: a SUM-AGGREGATE-`Construct`-fed allocation threaded (via Let-Var
    // aliases) to a merge/return block's DEAD block-param OVER-emits — a FRESH-site
    // `BurdenInc` on the Construct + a dup-alias `BurdenInc` on the Let-Var alias
    // (`use_counts >= 2` cardinality proxy mis-classes the same-alloc alias as a
    // duplication) + a misplaced alias release, netting +1 (leak). RL-2 requires ONE
    // allocation released EXACTLY once with ZERO keep-alive incs. The cure removes the
    // whole lineage class from `owned_vars_needing_rc` (Part B: suppresses both incs +
    // the misplaced dec) and emits the sole RL-5 release at the dead param
    // (`dead_forwarder_param_releases` merge below). SSOT:
    // `compute_construct_fed_dead_param_lineage` (sum-aggregate-Construct + heap-element
    // + dead-merge-param + no-alt-release gates bound the over-fire surface). Cures the
    // `for x in Some(str) yield { ... }` both-paths-fail lineage; default-path-safe
    // because the predicate stack provably emits no normal-path Option release here.
    let construct_fed_dead_param = if *CONSTRUCT_FED_DEAD_PARAM_RELEASE_DISABLED {
        ownership_scans::ConstructFedDeadParamLineage {
            releases: FxHashMap::default(),
            suppressed_lineage_vars: FxHashSet::default(),
        }
    } else {
        compute_construct_fed_dead_param_lineage(func, contracts, &owned_vars_needing_rc)
    };
    owned_vars_needing_rc.retain(|v| !construct_fed_dead_param.suppressed_lineage_vars.contains(v));
    // RL-1 + RL-2 fresh-sum live-extract treatment: a FRESH niche-family sum
    // (sum-aggregate Construct, or an Apply/Invoke result the callee hands the
    // caller as an owned reference) read through Let-Var aliases + a niche-payload
    // Project view and EXTRACTED LIVE to a merge block-param names ONE allocation
    // across the whole closure — per RL-1 no duplication exists, yet the
    // `use_counts >= 2` proxy + the FRESH-result inc leave 2 spurious keep-alive
    // incs against 2 misplaced releases (the arm dec before the live reads + the
    // extract's last-use dec before its final transitive read), netting +1 (the
    // caller-owned reference is never released). The cure removes the closure from
    // `owned_vars_needing_rc` (both incs + both misplaced decs) and emits EXACTLY
    // ONE whole-var release after the closure's final borrow-read (no UAF — the
    // read completes first). Runs AFTER the construct-fed retain so roots claimed
    // by the dead-param family auto-decline (gate b); disjoint from the forwarder
    // scans (Invoke roots with `transfers_through_return` callees decline). SSOT:
    // `compute_fresh_sum_live_extract_lineage` (niche-family-sum + vetted
    // borrow-read-only closure + live-extract + execution-final-site gates bound
    // the over-fire surface). Cures the `match_arm_alias_option_str` family
    // both-paths-fail lineage; default-path-safe because the predicate stack
    // provably emits zero ops for this shape (predicate-only probe: bare alloc).
    let fresh_sum_releases =
        apply_fresh_sum_live_extract(func, contracts, &mut owned_vars_needing_rc, type_registry);
    // RL-2 forwarder-result release: a transfer-through-return forwarder RESULT whose
    // monomorphized result-type burden is EMPTY (`burden_carries_rc == false`) is never
    // collected into `owned_vars_needing_rc`, so its lineage gets neither a FRESH inc nor
    // a scope-exit dec — leaking its transferred-in allocation when consumed only by a
    // borrow-projection then dead (`generic_forwarder_{set_int,result_list_str}`). RL-34
    // makes the caller own the returned allocation; `RL2_borrowed_param_emits_caller_dec`
    // mandates the release. Computed AFTER `owned_vars_needing_rc` is final so gate (b)
    // ("no existing release on the lineage") sees the settled set — this distinguishes the
    // genuinely-unreleased forwarder result (carries_rc=false) from the `inherent` over-emit
    // (carries_rc=true, in owned_vars, already over-decs). SSOT:
    // `compute_forwarder_result_under_release` (forwarder-identity + no-existing-release +
    // no-alt-consumer + used gates bound the over-fire / double-free surface).
    let mut forwarder_result_releases = if *FORWARDER_RESULT_RELEASE_DISABLED {
        FxHashMap::default()
    } else {
        compute_forwarder_result_under_release(func, contracts, &owned_vars_needing_rc)
    };
    // Merge the fresh-sum live-extract releases into the same placed-release
    // emission surface (identical `ForwarderReleasePos` placement contract).
    // The two families target disjoint lineages (forwarder-identity results vs
    // non-forwarder fresh-sum closures), so the merge cannot double-release.
    for (site, vars) in fresh_sum_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    let last_uses_at = group_last_uses_filtered(&ctx, &owned_vars_needing_rc);
    {
        let mut owned: Vec<u32> = owned_vars_needing_rc
            .iter()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        owned.sort_unstable();
        let dec_sites: Vec<u32> = last_uses_at
            .values()
            .flatten()
            .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
            .collect();
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            ?owned,
            ?dec_sites,
            "burden owned-vars-needing-rc + last-use dec sites"
        );
    }
    let terminator_transfer_per_block =
        compute_terminator_transfer_per_block(func, derived_ownership);
    let terminator_inc_per_block =
        compute_terminator_inc_per_block(func, &owned_vars_needing_rc, derived_ownership);

    // Populate `moved_out_fields` per the Non-Drop partial-move two-stage rule.
    // Pass 1 collects `(project_dst → (src, field))`; Pass 2 walks instructions
    // + terminators and sets the bit when a transferred var matches a
    // project_dst. Project alone leaves the bit unset (TF-4 Borrowed);
    // `Set.value` carve-out applies via `instr_transfer_vars` (TF-15).
    populate_moved_out_fields(
        &mut ctx,
        func,
        &terminator_transfer_per_block,
        type_registry,
    );

    // Derive the full-move var set: vars whose `moved_out_fields[var]` covers
    // every top-level field index of their `Burden::owned_fields()`. BurdenDec
    // emission is suppressed for these per AIMS RL-2 (full-move == complete
    // ownership transfer at field-projection grain → BurdenDec correctly
    // suppressed). Partial-move (some-but-not-all fields covered) still emits a
    // CONSERVATIVE FULL BurdenDec (over-emit, refined by the partial-drop IR
    // variant).
    let full_move_vars = compute_full_move_vars(
        func,
        &ctx.moved_out_fields_union,
        type_registry,
        &owned_vars_needing_rc,
    );

    // Derive the partial-move var map: vars with non-empty
    // `moved_out_fields[var]` that are NOT in `full_move_vars`. Each entry's
    // `skip_fields: Vec<u32>` lists top-level field indices to skip during
    // drop-glue iteration at codegen. `BurdenDecPartial` emission gates on this
    // map per AIMS RL-2 partial-transfer semantics (the non-moved fields still
    // need their drop; skip_fields names the transferred subset). AIMS
    // Invariant 5 case (b) — extends ArcInstr enum on the SAME var dimension;
    // no parallel emission, no shadow tracker.
    let mut partial_move_vars = compute_partial_move_vars(
        &ctx.moved_out_fields_union,
        &full_move_vars,
        &owned_vars_needing_rc,
    );

    // RL-2 sibling-alias moved-field cross-check: the loop-carried struct
    // self-rebuild (`r = T { a: r.a, b: r.b }`) lowers each plain self-projection
    // through a DISTINCT `Let { Var }` alias of the loop block-param; the
    // moved-out-field scan attributes each moved field ONLY to the alias that
    // lowered that projection, so each sibling's `BurdenDecPartial skip=[k]`
    // releases the field its SIBLING transferred into the new struct — a
    // double-free of a buffer carried to the next iteration. The post-process
    // unifies the sibling moved-field sets, widening each alias's skip with the
    // sibling-covered fields and absorbing fully-covered aliases into
    // `full_move_vars` (so their FRESH-site dup-alias inc is suppressed
    // coherently via the `inc_suppressed_vars = full_move_vars` coupling below).
    // The alias-chain ROOT is never a suppression target. Toggle
    // `ORI_DISABLE_SIBLING_MOVED_FIELD_UNION=1` restores per-alias attribution.
    // Spec: Annex E §AIMS RL-2.
    let mut full_move_vars = full_move_vars;
    sibling_union::apply_sibling_moved_field_union(
        func,
        type_registry,
        &ctx.moved_out_fields_union,
        &owned_vars_needing_rc,
        &mut full_move_vars,
        &mut partial_move_vars,
    );

    // RL-2 transfer-suppression symmetry: a fresh value whose paired BurdenDec
    // is transfer-suppressed at its LAST-USE must have its FRESH-site BurdenInc
    // suppressed too, else the inc is orphaned and the per-value burden ledger
    // nets +1 (VF-1 imbalance). Mirror the EXACT instruction-level
    // dec-suppression condition in emit_instr_burdens (line ~1221): dec
    // suppressed iff the var is transferred at its last-use instr OR its whole
    // owned-field set was moved (full_move_vars). Terminator-position transfers
    // are NOT included — their decs are emitted by emit_terminator_burden_decs
    // and balanced by emit_terminator_burden_incs, a separate inc/dec pair from
    // the FRESH-site inc. A value transferred at a NON-last use (aliased, still
    // live) keeps its Inc — its dec is emitted at the later non-transfer use.
    let mut inc_suppressed_vars: FxHashSet<ArcVarId> = full_move_vars.clone();

    // RL-1 duplication-alias emission for Let-Var aliases: a `Let { Var(src) }`
    // alias whose SOURCE stays live after the alias is a genuine duplication —
    // a new reference to `src`'s allocation. The burden path emits the alias's
    // own paired RC: a FRESH-site `BurdenInc dst` at the alias site
    // (emit_fresh_site_burden_inc) balanced by a `BurdenDec dst` at the alias's
    // true last-use (emit_last_use_decs / emit_terminator_burden_decs). Net 0.
    // A move-alias (source used only at the alias) is NOT a dup_alias_dst — its
    // ownership forwards through the move chain (transfer_via_move_alias) and the
    // source's own FRESH-site inc covers the lineage. "Source stays live" =
    // source appears in >= 2 used-var positions (the alias use plus at least one
    // more downstream). Computed BEFORE the inc-suppression scans so the
    // genuine-duplication exemption below can consult it.
    let (use_counts, dup_alias_dsts) = compute_use_counts_and_dup_aliases(
        func,
        &mut inc_suppressed_vars,
        &forwarder_identity_transparent_aliases,
    );

    // RL-1 genuine-duplication store-out aliases: dup-alias dsts whose source
    // stays live PAST the alias site and whose move chain ends at an
    // aggregate-store consume. Their alias-site `BurdenInc` is the lineage's
    // second reference — the one the container drop releases — so BOTH
    // inc-suppression scans below skip them (suppressing nets -1 →
    // double-free; the dec side stays transfer-suppressed). Empty when
    // `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1` (the compute fn owns the
    // toggle). Spec: Annex E §AIMS RL-1 + RL-2.
    let genuine_dup_move_aliases =
        compute_genuine_dup_move_aliases(func, &dup_alias_dsts, &full_move_vars);

    // RL-1 + RL-2 iter-consume duplication: an alias of an Owned ttr param that
    // is iter-consumed (`for x in xs` -> `@iter [own]` -> `ori_iter_drop` frees
    // the buffer) while the param ALSO transfers out via the Return is a genuine
    // duplication — its inc is the duplicate the iterator frees, leaving the
    // param's original for the Return. Like `genuine_dup_move_aliases` it skips
    // the inc-suppression (its dec stays transfer-suppressed via the `@iter
    // [own]` owned position). The store-only `genuine_dup_move_aliases` scan
    // EXCLUDES call-arg consumers, so the iter-consume case needs its own
    // structurally-discriminated scan. Empty when
    // `ORI_DISABLE_TTR_ITER_CONSUME_DUP_INC=1`.
    let ttr_iter_consume_dup_aliases = if *TTR_ITER_CONSUME_DUP_INC_DISABLED {
        FxHashSet::default()
    } else {
        compute_ttr_iter_consume_dup_aliases(func, contracts, interner)
    };

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let Some(last_used) = last_uses_at.get(&(block_idx, instr_idx)) else {
                continue;
            };
            let tv = instr_transfer_vars(instr, func);
            for &var in last_used {
                if tv.contains(&var)
                    && !genuine_dup_move_aliases.contains(&var)
                    && !ttr_iter_consume_dup_aliases.contains(&var)
                {
                    inc_suppressed_vars.insert(var);
                }
            }
        }
    }

    // RL-2 transitive-transfer suppression through Let-Var MOVE-alias chains.
    // A value whose ownership transfers out (Return value, owned call arg,
    // owned Jump arg) discharges its release at the transfer point — RL-2's
    // ownership-transferring-use exception. When a value reaches that transfer
    // point THROUGH a `Let { Var }` move-alias chain (`%d = %s` with `%s` used
    // exactly once — a move, not a duplication), the MOVE SOURCE also transfers
    // out: its only use forwards ownership to `%d`, which forwards to the
    // transfer point. Emitting a last-use BurdenDec on the move source then
    // releases a reference already handed off downstream — a VF-1 net=-1 orphan
    // dec (`id<T>(x) = x` over an owned param is the minimal witness:
    // `%1 = %0; Return %1` would dec `%0` though its ownership returns to the
    // caller). Backward-propagate transfer-suppression through every move-alias
    // hop to a fixpoint; the source set suppresses the source's last-use dec.
    // Invoke / Apply transfer-through-return result→arg move-edges: when a callee
    // transfers an owned param THROUGH its return, the call result IS the
    // forwarded arg (a move across the call), so the move-chain must span it.
    let invoke_ttr_edges = collect_invoke_ttr_edges(func, contracts);
    let transfer_via_move_alias = compute_transfer_via_move_alias(
        func,
        &terminator_transfer_per_block,
        &use_counts,
        ctx.last_use_points(),
        &owned_vars_needing_rc,
        &invoke_ttr_edges,
    );

    // Symmetry: a FRESH move source whose last-use dec is suppressed (it
    // transfers out via the alias chain) must also have its FRESH-site inc
    // suppressed, else the inc orphans (VF-1 net=+1). A param move source has no
    // FRESH inc, so the union is a no-op for it; only FRESH sources gain the
    // inc-suppression. Keeps the per-value ledger at net 0 for both shapes.
    // Both halves are suppressed on BOTH the default and probe paths: the value
    // genuinely moves through the alias chain to a real transfer point where its
    // single release is discharged (suppressing here prevents a double dec on the
    // shared allocation under the probe — the move-alias dec stays suppressed in
    // emit_last_use_decs on both paths).
    for &var in &transfer_via_move_alias {
        // Pure move source (used <= once): the value flows straight through to
        // the consumer, so BOTH halves of its FRESH pair are suppressed. A DUP'd
        // move source (used >= 2) forwards only its ORIGINAL allocation reference
        // at its terminal move; its FRESH inc supplies the DUPLICATE references
        // the non-terminal uses consume and MUST be kept — only the terminal
        // last-use dec is suppressed (in `transfer_via_move_alias`). Suppressing
        // the inc here would under-count and collapse a COW receiver's RC below
        // the live alias count. Per AIMS RL-1/RL-2.
        //
        // A use-once GENUINE-duplication alias (`genuine_dup_move_aliases`: a
        // dup-alias dst whose SOURCE stays live past the alias) is the one-hop-
        // down twin of the dup'd source: its move-out consumes the DUPLICATE
        // reference its alias-site inc supplies, while the source's original
        // reference stays live behind it. Its inc MUST be kept (suppressing
        // nets -1: the consumer releases a reference no inc supplied —
        // `RL1_duplication_balanced`); only its dec is transfer-suppressed. The
        // ttr-iter-consume dup alias is the same shape with an `@iter [own]`
        // freeing consume instead of a store — its inc is equally load-bearing.
        if use_counts.get(&var).copied().unwrap_or(0) <= 1
            && !genuine_dup_move_aliases.contains(&var)
            && !ttr_iter_consume_dup_aliases.contains(&var)
        {
            inc_suppressed_vars.insert(var);
        }
    }

    // Symmetry for borrowed terminator-Invoke args: `invoke_terminator_borrowed_args`
    // (emit.rs) suppresses the terminator-last-use `BurdenDec` for a BORROWED
    // `Invoke`/`InvokeIndirect` arg — the value survives the borrowed call and
    // the predicate-stack edge cleanup discharges its release. A FRESH value
    // created in the terminator block solely to pass borrowed (e.g.
    // `%0 = "hello"; Invoke @f(%0 [borrow])` where the callee stores `%0` into
    // its result) would otherwise keep its FRESH-site `BurdenInc` with the dec
    // suppressed → net=+1 on the path where the value lives on into the result.
    // Suppress the FRESH inc symmetrically so the var carries NO burden ops
    // (`burden_emitted` stays false → edge cleanup emits no paired `BurdenDec`)
    // and is fully predicate-stack-managed (`Spec: Annex E §AIMS RL-2`). A
    // genuine borrow (param / alias, no FRESH inc) gains nothing — no-op.
    //
    // Probe gate: under `predicate_stack_rc_disabled` the terminator-last-use
    // BurdenDec is un-suppressed (emit_burden_ops_for_blocks passes an empty
    // borrowed-arg set), so the FRESH inc must survive symmetrically — the
    // burden path carries the full paired inc+dec for the fresh-owned arg the
    // callee stores. Keep the inc suppressed only on the default path.
    let borrowed_terminator_args = compute_borrowed_terminator_invoke_args(func);
    if !predicate_stack_rc_disabled {
        for &var in &borrowed_terminator_args {
            inc_suppressed_vars.insert(var);
        }
    }

    // RL-4 live-out suppression set (`Spec: Annex E §AIMS RL-4`): a per-block
    // last-use `BurdenDec` is a genuine release only when the var is dead at
    // block exit; live-out vars are released on dying CFG edges. See
    // `compute_live_out_owned`.
    let live_out_per_block = compute_live_out_owned(func, &owned_vars_needing_rc);

    // Step-1 COW-inc set + step-2 COW-mutator-release-gate names. Probe-only —
    // empty on the default path (the predicate stack emits the equivalent RcInc,
    // so default AOT codegen is byte-identical). Detail: `compute_cow_inc_and_mutators`.
    let (cow_inc_borrowed_aliases, cow_mutator_names) = compute_cow_inc_and_mutators(
        func,
        &borrowed_aliases,
        interner,
        predicate_stack_rc_disabled,
    );

    // Function-wide list-concat consume set (precomputed: the `&mut` emit walk
    // cannot re-borrow `func` for `var_repr`). SSOT: `list_concat_consumed_operands`.
    let mut list_concat_transfer_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            list_concat_transfer_vars.extend(list_concat_consumed_operands(instr, func));
        }
    }

    // RL-1 result-inc elision for transfer-through-return forwarders: a callee
    // returning its owned param unchanged hands the SAME allocation back, so the
    // result is not a fresh value and its result-inc is elidable. SSOT:
    // `compute_transfer_through_return_results`.
    let transfer_through_return_results = compute_transfer_through_return_results(func, contracts);

    // RL-2 callee transfer-source-dec strip: a param of THIS function that flows
    // to a `Return` terminator (per its own `MemoryContract.transfers_through_return`)
    // transfers ownership back to the caller — its scope-exit `BurdenDec` is
    // suppressed (the caller decs the bound result). The interprocedural contract
    // is the SSOT for the proven Return-flow fact; the structural move-alias scan
    // conservatively keeps the dec when the param is multi-block-used, so consult
    // the contract directly for the param case. SSOT:
    // `compute_transfer_through_return_param_vars`.
    let transfer_through_return_param_vars =
        compute_transfer_through_return_param_vars(func, contracts);

    // RL-1 inc-elision callee identity: `__index` codegen self-increments its
    // extracted non-scalar result, so the burden path elides the result-inc
    // (AIMS emits only the balancing dec). Interned once; idempotent.
    let index_builtin_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());

    // RL-1 borrowed-store duplication incs: a BORROWED-param-rooted value
    // consumed at an aggregate-STORE position duplicates the caller's retained
    // reference — the store-site inc is load-bearing (the container's drop is
    // the matched release). Empty when `ORI_DISABLE_BORROWED_STORE_DUP_INC=1`
    // (the compute fn owns the toggle). SSOT: `compute_borrowed_store_dup_args`.
    let borrowed_store_dup_args = compute_borrowed_store_dup_args(func, type_registry);

    // RL-5 dead-at-entry cleanup for forwarder-identity allocations reaching a
    // merge/return block's dead block-params: the Jump-arg → Owned-param handoff
    // (RL-4 exemption) suppresses the source's last-use dec expecting RL-5 to release
    // the dead successor param; the Phase-5 walk otherwise emits no RL-5 dec, leaking
    // the forwarded allocation. SSOT: `compute_dead_forwarder_block_param_releases`
    // (one dec per distinct source allocation; forwarder-identity + edge-release gates
    // bound the over-fire surface).
    let mut dead_forwarder_param_releases = if *DEAD_FORWARDER_PARAM_RELEASE_DISABLED {
        FxHashMap::default()
    } else {
        compute_dead_forwarder_block_param_releases(func, contracts)
    };
    // Merge the construct-fed dead-param releases (Part A) into the same per-block
    // release map. Both are RL-5 dead-at-entry releases emitted identically by
    // `emit_burden_ops_for_blocks`; they target disjoint reps (forwarder-identity vs
    // sum-aggregate-Construct), so the merge cannot double-release one allocation.
    for (block_idx, params) in construct_fed_dead_param.releases {
        dead_forwarder_param_releases
            .entry(block_idx)
            .or_default()
            .extend(params);
    }
    // RL-4 dead-owned-param-branch release: an Owned non-scalar FUNCTION-param returned on
    // ONE branch is dead on the others (`triple<T>(c, x, y, z)`); its only last-use is a
    // transfer-return on a different branch (RL-2 transfer-suppressed there), so the
    // Phase-5 walk emits no release and it leaks on the dead branches. The per-edge dec
    // (block-entry placement, single-predecessor-gated) releases it once on each dead
    // branch. DISJOINT from the forwarder-identity + construct-fed dead-BLOCK-param
    // releases above (those target Jump-arg-reached block params; this targets function
    // params dead crossing a branch edge), so the merge cannot double-release. SSOT:
    // `compute_dead_owned_param_branch_releases` (Owned-function-param + edge-deadness +
    // not-transferred + single-pred gates bound the over-fire / double-free surface).
    let dead_owned_param_branch_releases = if *DEAD_OWNED_PARAM_BRANCH_RELEASE_DISABLED {
        FxHashMap::default()
    } else {
        compute_dead_owned_param_branch_releases(func, &owned_vars_needing_rc)
    };
    for (block_idx, params) in dead_owned_param_branch_releases {
        dead_forwarder_param_releases
            .entry(block_idx)
            .or_default()
            .extend(params);
    }

    let analysis = BurdenAnalysisCtx {
        owned_vars_needing_rc: &owned_vars_needing_rc,
        last_uses_at: &last_uses_at,
        full_move_vars: &full_move_vars,
        partial_move_vars: &partial_move_vars,
        inc_suppressed_vars: &inc_suppressed_vars,
        dup_alias_dsts: &dup_alias_dsts,
        transfer_via_move_alias: &transfer_via_move_alias,
        live_out_per_block: &live_out_per_block,
        contracts,
        predicate_stack_rc_disabled,
        list_concat_transfer_vars: &list_concat_transfer_vars,
        cow_inc_borrowed_aliases: &cow_inc_borrowed_aliases,
        cow_mutator_names: &cow_mutator_names,
        transfer_through_return_results: &transfer_through_return_results,
        transfer_through_return_param_vars: &transfer_through_return_param_vars,
        index_builtin_name,
        borrowed_store_dup_args: &borrowed_store_dup_args,
    };
    emit_burden_ops_for_blocks(
        func,
        &analysis,
        &terminator_transfer_per_block,
        &terminator_inc_per_block,
        &dead_forwarder_param_releases,
        &forwarder_result_releases,
    );
    populate_burden_emitted(func);
    ctx
}

/// Invoke / Apply transfer-through-return result→arg move-edges: when a callee
/// transfers an owned param THROUGH its return, the call result IS the
/// forwarded arg (a move across the call), so the move-alias chain must span
/// it. Consumed by `compute_transfer_via_move_alias`.
fn collect_invoke_ttr_edges(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<(ArcVarId, ArcVarId)> {
    let mut edges = Vec::new();
    let mut collect = |dst: ArcVarId, callee: &Name, args: &[ArcVarId]| {
        if let Some(contract) = contracts.get(callee) {
            for (i, param) in contract.params.iter().enumerate() {
                if param.transfers_through_return {
                    if let Some(&arg) = args.get(i) {
                        edges.push((dst, arg));
                    }
                }
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                collect(*dst, callee, args);
            }
        }
        if let crate::ir::ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            collect(*dst, callee, args);
        }
    }
    edges
}

/// Toggle-gated application of the RL-1 + RL-2 fresh-sum live-extract
/// treatment ([`compute_fresh_sum_live_extract_lineage`]): computes the
/// vetted same-alloc closures, removes them from `owned_vars_needing_rc`
/// (suppressing the spurious keep-alive incs + misplaced releases), and
/// returns the placed single releases for the `forwarder_result_releases`
/// merge. Empty when `ORI_DISABLE_FRESH_SUM_LIVE_EXTRACT_RELEASE=1`.
fn apply_fresh_sum_live_extract(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> FxHashMap<(usize, ownership_scans::ForwarderReleasePos), Vec<ArcVarId>> {
    if *FRESH_SUM_LIVE_EXTRACT_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_fresh_sum_live_extract_lineage(
        func,
        contracts,
        owned_vars_needing_rc,
        type_registry,
    );
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Populate `func.burden_emitted` from the just-emitted burden ops. Walks
/// every block's body once after `emit_burden_ops_for_blocks` completes and
/// sets `burden_emitted[var.index()] = true` for every var targeted by
/// `BurdenInc` / `BurdenDec` / `BurdenDecPartial` / `BurdenDecField` /
/// `BurdenDecVariant`. One linear pass per function, no per-var hash-map churn.
///
/// Coexistence-handshake input consumed downstream by the AIMS
/// post-convergence `class_covered` computation, which gates predicate-stack
/// realization deferral.
fn populate_burden_emitted(func: &mut ArcFunction) {
    if func.burden_emitted.len() != func.var_types.len() {
        func.burden_emitted = vec![false; func.var_types.len()];
    }
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var }
                | ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    mark_emitted(&mut func.burden_emitted, var.index());
                }
                ArcInstr::BurdenDecField { base, .. } => {
                    mark_emitted(&mut func.burden_emitted, base.index());
                }
                _ => {}
            }
        }
    }
}

/// Set `emitted[idx] = true` when `idx` is in bounds; out-of-bounds is a no-op.
fn mark_emitted(emitted: &mut [bool], idx: usize) {
    if let Some(slot) = emitted.get_mut(idx) {
        *slot = true;
    }
}

#[cfg(test)]
mod tests;
