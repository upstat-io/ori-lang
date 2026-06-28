//! Phase-5 burden-emission scan orchestration — the emission-assembly driver.
//!
//! `emit_burden_ops` runs the burden walk in two phases: the suppression-filter
//! prologue (`scan_helpers::compute_owned_rc_filter` — exclusion retains +
//! lineage scans producing the settled owned set + placed releases) and the
//! emission-assembly phase here (terminator / move-field / dup-alias /
//! transfer-suppression computation → `BurdenAnalysisCtx` → the per-block emit
//! walk). The per-scan `apply_*` wrappers and the `burden_emitted` population
//! live in `scan_helpers`. Spec: Annex E §AIMS RL-1..RL-5 + L-9.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::lower::burden_lookup::type_has_user_drop;
use crate::ownership::DerivedOwnership;

use super::cow_aliases::compute_cow_inc_and_mutators;
use super::ctx::BurdenLowerCtx;
use super::emit::{emit_burden_ops_for_blocks, BurdenAnalysisCtx};
use super::moved_fields::{
    compute_full_move_vars, compute_partial_move_vars, populate_moved_out_fields,
};
use super::ownership_scans::{
    compute_borrowed_store_dup_args, compute_borrowed_terminator_invoke_args,
    compute_branch_exclusive_edge_releases, compute_collection_literal_dead_source_suppression,
    compute_cow_terminal_concat_inc_dsts, compute_dead_forwarder_block_param_releases,
    compute_dead_owned_param_branch_releases, compute_fresh_call_result_borrowed_arg_inc_dsts,
    compute_genuine_dup_move_aliases, compute_live_out_owned,
    compute_loop_invariant_dead_local_releases, compute_multi_borrow_view_alias_surplus,
    compute_readonly_borrow_orphan_inc_suppression, compute_reassign_rebind_releases,
    compute_rebuild_lineage_dead_param_releases, compute_sharing_view_surplus_inc_dsts,
    compute_sum_payload_iter_consume_dup_inc_suppression,
    compute_transfer_through_return_param_vars, compute_transfer_through_return_results,
    compute_transfer_via_move_alias, compute_ttr_iter_consume_dup_aliases,
    compute_use_counts_and_dup_aliases, compute_yield_identity_push_dup_args, instr_transfer_vars,
    list_concat_consumed_operands,
};
use super::scan_helpers::{
    collect_invoke_ttr_edges, compute_owned_rc_filter, populate_burden_emitted, OwnedRcFilter,
};
use super::terminator::{compute_terminator_inc_per_block, compute_terminator_transfer_per_block};
use super::{
    extract_transfer, sibling_union, DEAD_FORWARDER_PARAM_RELEASE_DISABLED,
    DEAD_OWNED_PARAM_BRANCH_RELEASE_DISABLED, SUM_PAYLOAD_ITER_CONSUME_DUP_INC_DISABLED,
    TTR_ITER_CONSUME_DUP_INC_DISABLED,
};

/// Merge `source`'s per-key `ArcVarId` release lists into `target`, skipping a
/// var already present under its key. Shared by the branch-exclusive funded
/// release merge and the dead-edge birth release merge. Spec: Annex E §AIMS
/// RL-4 + RL-5.
fn merge_release_vars<K: Eq + std::hash::Hash>(
    target: &mut FxHashMap<K, Vec<ArcVarId>>,
    source: FxHashMap<K, Vec<ArcVarId>>,
) {
    for (key, vars) in source {
        let entry = target.entry(key).or_default();
        for var in vars {
            if !entry.contains(&var) {
                entry.push(var);
            }
        }
    }
}

/// Walk `func` and emit `BurdenInc` / `BurdenDec` ops per SSA variable from
/// `BurdenSpec` lookups, filtered to owned positions via `DerivedOwnership`.
///
/// Two phases: the suppression-filter prologue
/// ([`compute_owned_rc_filter`](super::scan_helpers::compute_owned_rc_filter))
/// followed by the emission-assembly phase below. Invoked from the AIMS pipeline
/// at Phase 5 (ARC lowering); see `pipeline/aims_pipeline/`.
#[expect(
    clippy::too_many_lines,
    reason = "single Phase-5 emission-assembly phase: the terminator / \
              move-field / dup-alias / transfer-suppression computation and the \
              per-block emit walk are one cohesive pass; splitting mid-sequence \
              fragments the load-bearing emission order"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Phase-5 emission inputs are the typed pre-pass side tables \
              (ownership, contracts, immortals, apply-result aliases) the walk \
              consumes per AIMS Invariant 5"
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
    // Apply-result allocation-identity map (`AimsStateMap::apply_result_aliases`,
    // converged at Step 4) — input to the §1.9 unified alias-table construction
    // (`compute_project_alias_table`) the sibling-union cross-block identity
    // consumes. AIMS Invariant 5 — the ONE table builder; no parallel tracker.
    // Tests pass `&FxHashMap::default()` (the structural Let / Jump-arg /
    // Project edges still build the table).
    apply_result_aliases: &FxHashMap<
        ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    >,
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
    // Phase 1 — suppression-filter prologue: populate `ctx`, compute the owned
    // set, apply every exclusion retain + lineage suppression, merge the placed
    // releases. SSOT for the load-bearing scan order. Spec: Annex E §AIMS
    // RL-1..RL-5 + L-9.
    let OwnedRcFilter {
        owned_vars_needing_rc,
        borrowed_aliases,
        forwarder_identity_transparent_aliases,
        mut forwarder_result_releases,
        construct_fed_dead_param,
        last_uses_at,
        claimed_no_sink_vars,
        final_read_release_aliases,
        owner_borrow_view_dec_suppress,
    } = compute_owned_rc_filter(
        &mut ctx,
        func,
        type_registry,
        immortals,
        contracts,
        interner,
    );

    // Phase 2 — emission assembly.
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

    // RL-1 duplication-alias classification for Let-Var aliases: a `Let {
    // Var(src) }` alias whose SOURCE stays live after the alias is a genuine
    // duplication — a new reference to `src`'s allocation. The burden path
    // emits the alias's own paired RC: a FRESH-site `BurdenInc dst` at the
    // alias site (emit_fresh_site_burden_inc) balanced by a `BurdenDec dst` at
    // the alias's true last-use (emit_last_use_decs /
    // emit_terminator_burden_decs). Net 0. A move-alias (source used only at
    // the alias) is NOT a dup_alias_dst — its ownership forwards through the
    // move chain (transfer_via_move_alias) and the source's own FRESH-site inc
    // covers the lineage. "Source stays live" = source appears in >= 2 used-var
    // positions (the alias use plus at least one more downstream). Computed
    // BEFORE the sibling union (which declines multi-hop chains whose
    // intermediate carries a kept dup inc) and BEFORE the inc-suppression scans
    // (so the genuine-duplication exemption can consult it). The zero-use vars
    // it inserts into `inc_suppressed_vars` are independent of the sibling
    // union's `full_move_vars` mutation; the full-move coupling is applied
    // AFTER the union via `extend` (set union — order-independent).
    let mut inc_suppressed_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    let (use_counts, dup_alias_dsts) = compute_use_counts_and_dup_aliases(
        func,
        &mut inc_suppressed_vars,
        &forwarder_identity_transparent_aliases,
    );

    // §1.9 unified alias table + per-predecessor-edge attribution — the
    // cross-block same-allocation identity the sibling union consumes (ONE
    // table; the table-resolved hop widening + back-edge decline live in
    // `sibling_union`). Spec: Annex E §AIMS.
    let alias_table = crate::aims::intraprocedural::project_aliases::compute_project_alias_table(
        func,
        apply_result_aliases,
    );
    let param_edge_args =
        crate::aims::intraprocedural::project_aliases::compute_param_edge_args(func);
    let sibling_union_outcome = sibling_union::apply_sibling_moved_field_union(
        func,
        type_registry,
        &ctx.moved_out_fields_union,
        &owned_vars_needing_rc,
        &alias_table,
        &param_edge_args,
        &dup_alias_dsts,
        &mut full_move_vars,
        &mut partial_move_vars,
    );

    // RL-2 match-handoff extract-transfer attribution: a rebuild carrier's
    // still-released SUM field counts MOVED when EVERY switch arm over the
    // field's tag either transfers the extracted payload into the rebuild
    // construct (unconditional arm path, per-edge identity) or is the
    // payload-less-variant arm — the `r = Pair { o: Some(extracted), b: r.b }`
    // shape where the partial dec would double-free the re-wrapped payload. A
    // conditional / partial flow DECLINES (the dropped-payload path keeps its
    // release). Runs AFTER the sibling union (consumes the widened skip sets)
    // and BEFORE the `inc_suppressed_vars = full_move_vars` coupling below.
    // Toggle `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER=1` restores the
    // un-widened attribution. Spec: Annex E §AIMS RL-2.
    extract_transfer::apply_match_handoff_extract_transfer(
        func,
        type_registry,
        &owned_vars_needing_rc,
        &alias_table,
        &param_edge_args,
        &mut full_move_vars,
        &mut partial_move_vars,
    );

    // RL-2 transfer-suppression symmetry: a fresh value whose paired BurdenDec
    // is transfer-suppressed at its LAST-USE must have its FRESH-site BurdenInc
    // suppressed too, else the inc is orphaned and the per-value burden ledger
    // nets +1 (VF-1 imbalance). Mirror the EXACT instruction-level
    // dec-suppression condition in emit_instr_burdens: dec
    // suppressed iff the var is transferred at its last-use instr OR its whole
    // owned-field set was moved (full_move_vars). Terminator-position transfers
    // are NOT included — their decs are emitted by emit_terminator_burden_decs
    // and balanced by emit_terminator_burden_incs, a separate inc/dec pair from
    // the FRESH-site inc. A value transferred at a NON-last use (aliased, still
    // live) keeps its Inc — its dec is emitted at the later non-transfer use.
    inc_suppressed_vars.extend(full_move_vars.iter().copied());
    // RL-1 collection-literal dead-source suppression: a fresh owned aggregate
    // fully consumed by (N-1) duplication aliases + 1 last-use move alias into a
    // collection-literal `Construct` (`[a, a]`) is transferred at its last use
    // through the move alias — its fresh-site keep-alive `BurdenInc` is the
    // spurious +1 (the dup-alias incs already fund the duplicate slots). Extends
    // the RL-2 transfer-suppression symmetry above to the one-hop-alias case.
    // Empty when `ORI_DISABLE_COLLECTION_LITERAL_DEAD_SOURCE_SUPPRESS=1`.
    // Spec: Annex E §AIMS RL-1 + RL-2.
    inc_suppressed_vars.extend(
        compute_collection_literal_dead_source_suppression(func, &owned_vars_needing_rc)
            .iter()
            .copied(),
    );
    // RL-1 keeper inc-suppression: the assigned keeper's whole `burden_dec` at
    // last use is the designated balancing release for its dup INTERMEDIATE's
    // kept inc; the keeper's own FRESH-site dup-alias inc is therefore
    // suppressed here (one allocation, one net release). Encoded at Phase 5
    // rather than relying on the Phase-6 DP-3 split (which the loop-carried
    // pair-atomicity guard bans for pure borrow-view aliases). Spec: Annex E
    // §AIMS RL-1 (`RL1_duplication_balanced`).
    inc_suppressed_vars.extend(sibling_union_outcome.keepers.iter().copied());

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

    // RL-1 genuine-duplication OWNED-CALL-ARG aliases: dup-alias dsts whose
    // source stays live PAST the alias site and whose move chain ends at an
    // owned call-arg consume (builtin COW receiver `push`/`insert`/`set`/
    // `remove`, or a user callee whose contract proves the param Owned). Each
    // such fork is a genuine duplication: the kept alias-site `BurdenInc`
    // funds the consumer's release (the COW helper's copy-source dec / the
    // callee's owned-param release), so it skips BOTH inc-suppression scans
    // below AND escapes the `emit_terminator_burden_decs` symmetric same-block
    // cancellation (the `inc_counts` tally gate in `emit_burden_ops_for_blocks`).
    // The FUNDED set already carries the Phase-5 emission gates (a dst outside
    // `dup_alias_dsts` never gets the alias-site inc; a `full_move_vars`
    // member's whole-var ops are owned by the field-projection machinery) —
    // ONE SSOT shared with the Phase-6 pair-atomic collector and the Phase-7
    // lineage-net machinery. Empty when `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1`
    // (the compute fn owns the toggle). Spec: Annex E §AIMS RL-1 + RL-2.
    let genuine_dup_call_arg_aliases =
        super::compute_funded_call_arg_dup_aliases(func, contracts, interner);

    // RL-2 final-read release carriers: suppress the dup-alias keep-alive inc
    // so the designated alias carries ONLY its last-use `BurdenDec` — the
    // multi-read call-result-aggregate element's single release. SSOT:
    // `compute_call_result_element_final_read_releases` (scan_helpers re-adds
    // the alias to `owned_vars_needing_rc`).
    inc_suppressed_vars.extend(final_read_release_aliases.iter().copied());

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

    // RL-1 move-once surplus dup-alias-inc suppression: a `Let { Var }` dup-alias
    // of a fresh niche-family sum-aggregate whose extracted payload is
    // iter-consumed adds NO owner (the by-value aggregate carries the buffer's
    // own allocation), so its duplication inc is move-once-elidable
    // (`RL1_duplication_balanced`, `incElidable = true`). The base walk's
    // `use_counts >= 2` proxy emits the surplus inc; on the live-extract path the
    // payload transfers out via `@iter [own]` (`ori_iter_drop` releases it,
    // `RL2_iter_consuming_no_caller_dec`), leaving the dup-alias inc unmatched →
    // +1 leak. Suppress it. Empty when
    // `ORI_DISABLE_SUM_PAYLOAD_ITER_CONSUME_DUP_INC=1`.
    let sum_payload_iter_consume_suppressed = if *SUM_PAYLOAD_ITER_CONSUME_DUP_INC_DISABLED {
        FxHashSet::default()
    } else {
        compute_sum_payload_iter_consume_dup_inc_suppression(
            func,
            contracts,
            &owned_vars_needing_rc,
            &dup_alias_dsts,
            type_registry,
            interner,
        )
    };
    inc_suppressed_vars.extend(sum_payload_iter_consume_suppressed.iter().copied());

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let Some(last_used) = last_uses_at.get(&(block_idx, instr_idx)) else {
                continue;
            };
            let tv = instr_transfer_vars(instr, func);
            for &var in last_used {
                if tv.contains(&var)
                    && !genuine_dup_move_aliases.contains(&var)
                    && !genuine_dup_call_arg_aliases.contains(&var)
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
    let mut transfer_via_move_alias = compute_transfer_via_move_alias(
        func,
        &terminator_transfer_per_block,
        &use_counts,
        ctx.last_use_points(),
        &owned_vars_needing_rc,
        &invoke_ttr_edges,
        &super::ownership_scans::SameAllocIdentity {
            genuine_same_alloc_reps: &alias_table.genuine_same_alloc_reps,
            apply_result_aliases,
            type_registry,
        },
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
            && !genuine_dup_call_arg_aliases.contains(&var)
            && !ttr_iter_consume_dup_aliases.contains(&var)
        {
            inc_suppressed_vars.insert(var);
        }
    }

    // TF-14 owner-drop borrow-view DEC-ONLY suppression: an owner aggregate whose
    // release was relocated to a borrowed-`Invoke` normal-successor `BlockEntry`
    // has its OWN in-block last-use `BurdenDec` suppressed — but its FRESH
    // (`Construct`) inc is KEPT (the relocated `BlockEntry` release is the single
    // release of that rc=1 allocation, RL-2 release-once). Merged AFTER the
    // inc-suppression loop so the owner's inc is never swept into
    // `inc_suppressed_vars`. Spec: Annex E §AIMS TF-14 + RL-2 + RL-4.
    transfer_via_move_alias.extend(owner_borrow_view_dec_suppress.iter().copied());

    // DP-3 read-only-borrow orphan-inc suppression: a fresh `Construct` whose
    // scope-exit dec is transfer-suppressed (via the owned-RC-dst move-edge seed,
    // when its terminal `Let { Var }` borrow-alias is itself owned-RC) AND whose
    // entire same-allocation lineage is consumed ONLY at read-only borrow
    // positions leaves its FRESH-site inc ORPHANED (the `use_counts <= 1`
    // inc-suppression above fires only for the pure-move source, not the
    // multi-borrow lineage). Per DP-3 (`is_rc_inc_elidable`: `Once ∧ Affine`
    // borrow → duplicate-inc elidable) the inc is unnecessary; suppress it to
    // restore RL-2 single-release balance. The read-only-borrow-ONLY gate
    // excludes self-rebuild / store-dup (a lineage member at an owned consume is
    // in `owned_consumed` → declines, kept inc load-bearing per
    // `RL1_duplication_balanced`). Spec: Annex E §AIMS DP-3 + RL-1 + RL-2.
    let mut owned_consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            owned_consumed.extend(instr_transfer_vars(instr, func).iter().copied());
        }
    }
    for set in &terminator_transfer_per_block {
        owned_consumed.extend(set.iter().copied());
    }
    let readonly_borrow_orphan_incs = compute_readonly_borrow_orphan_inc_suppression(
        func,
        &transfer_via_move_alias,
        &owned_vars_needing_rc,
        &owned_consumed,
        &dup_alias_dsts,
    );
    inc_suppressed_vars.extend(readonly_borrow_orphan_incs.iter().copied());

    // Multi-borrow-view-alias surplus suppression (RL-2 release-once + TF-4 +
    // DP-3): a fresh `Construct` owner consumed ONLY through >= 2 same-allocation
    // whole-var `Let { Var }` borrow-view aliases (each projected for a
    // borrow-read) keeps surplus per-alias whole-var decs + a spurious keep-alive
    // FRESH inc. Suppress each alias's surplus dec (`transfer_via_move_alias`)
    // and the owner's keep-alive inc (`inc_suppressed_vars`); the owner's
    // born-at-alloc reference is released exactly once by its surviving
    // edge-cleanup dec. The N-alias generalization of the single-alias
    // borrow-view-dst keystone. Full rationale + gates:
    // `compute_multi_borrow_view_alias_surplus`. Spec: Annex E §AIMS RL-2 + TF-4
    // + DP-3.
    if !super::multi_borrow_view_alias_surplus_disabled() {
        let (alias_dec_suppress, owner_inc_suppress) = compute_multi_borrow_view_alias_surplus(
            func,
            &owned_vars_needing_rc,
            &owned_consumed,
            &alias_table.genuine_same_alloc_reps,
        );
        transfer_via_move_alias.extend(alias_dec_suppress.iter().copied());
        inc_suppressed_vars.extend(owner_inc_suppress.iter().copied());
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

    // RL-2 mutable-`Ident` reassignment release, merged into the forwarder-result
    // placed-release surface (contains-gated). Leak signature + 5-condition gate:
    // `compute_reassign_rebind_releases` module doc. Spec: Annex E §AIMS RL-2.
    let reassign_rebind_releases = compute_reassign_rebind_releases(
        func,
        &owned_vars_needing_rc,
        &transfer_via_move_alias,
        &inc_suppressed_vars,
        &full_move_vars,
    );
    merge_release_vars(&mut forwarder_result_releases, reassign_rebind_releases);

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

    // RL-1 surplus-inc suppression for read-only-in-place-co-owner seamless-slice
    // RESULTS: the sharing-view runtime self-incs the shared buffer, so the result
    // is an owned co-reference whose `+1` is the runtime's own inc — AIMS emits
    // only the balancing dec, never a FRESH-site inc (else net +1 leak). Gated to
    // the surplus shape (receiver survives + read downstream; result lineage a
    // pure borrow-read co-owner not fed to another sharing-view). Empty under
    // `ORI_DISABLE_SHARING_VIEW_SURPLUS_INC_SUPPRESS=1`. SSOT:
    // `compute_sharing_view_surplus_inc_dsts`.
    let sharing_view_surplus_inc_dsts = compute_sharing_view_surplus_inc_dsts(func, interner);

    // RL-1 terminal-concat surplus incs: a FRESH local consumed EXACTLY ONCE as a
    // `Binary(Add)` concat operand is move-once-linear (the concat helper borrows
    // it, the caller's single dec frees it), so its keep-alive FRESH-site inc is
    // surplus (else net +1 leak). The single-use gate excludes the
    // re-read-after-concat shape (where the inc forces a COW copy). Empty under
    // `ORI_DISABLE_COW_TERMINAL_CONCAT_INC_ELISION=1` (the compute fn owns the
    // toggle). SSOT: `compute_cow_terminal_concat_inc_dsts`.
    let cow_terminal_concat_inc_dsts = compute_cow_terminal_concat_inc_dsts(func);

    // RL-1 / DP-3 fresh-call-result borrowed-arg surplus inc: a fresh
    // self-allocating Invoke result (`@to_str`) whose sole use is a borrowed
    // body-`Apply` arg (`print(msg: xs.len().to_str())`) is move-once-linear —
    // its keep-alive fresh-site inc is surplus (the paired single dec frees the
    // alloc; else net +1 leak). Disjoint from the terminator-Invoke-consume
    // lineage treatment. Empty under
    // `ORI_DISABLE_FRESH_CALL_RESULT_BORROWED_ARG_INC_ELISION=1` (the compute fn
    // owns the toggle). SSOT: `compute_fresh_call_result_borrowed_arg_inc_dsts`.
    let fresh_call_result_borrowed_arg_inc_dsts =
        compute_fresh_call_result_borrowed_arg_inc_dsts(func, interner);

    // RL-1 borrowed-store duplication incs: a BORROWED-param-rooted value
    // consumed at an aggregate-STORE position duplicates the caller's retained
    // reference — the store-site inc is load-bearing (the container's drop is
    // the matched release). Empty when `ORI_DISABLE_BORROWED_STORE_DUP_INC=1`
    // (the compute fn owns the toggle). SSOT: `compute_borrowed_store_dup_args`.
    let borrowed_store_dup_args = compute_borrowed_store_dup_args(func, type_registry);

    // RL-1 yield-identity push duplication incs: an iterator-element borrow-view
    // (`Project field:1` of `@__iter_next` over a BORROWED-param-rooted source)
    // consumed at the OWNED element-store position of `@ori_list_push`. The push
    // copies the element into the fresh result buffer (a real SECOND reference);
    // the borrowed source survives in the caller (RL-2), so the store duplicates
    // and owes one inc — matched by the result collection's `elem_dec_fn` drop.
    // The iterator-element-view exclusion (`collect_iter_element_defs`) dropped
    // both the spurious dec AND this load-bearing inc; this restores ONLY the inc
    // on the genuine duplication. Kept DISTINCT from `borrowed_store_dup_args`
    // (those fire at aggregate-store nodes; these fire at the `@ori_list_push`
    // call-arg site) so an element never double-incs. Empty when
    // `ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC=1`. SSOT:
    // `compute_yield_identity_push_dup_args`. Spec: Annex E §AIMS RL-1.
    let yield_identity_push_dup_args =
        compute_yield_identity_push_dup_args(func, type_registry, interner);

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
    // RL-5 rebuild-lineage dead-param release: a sibling-union-fired loop-carried
    // rebuild lineage whose loop-exit Jump edge feeds a DEAD successor block-param
    // (the loop-carried var unused after the loop). The union suppressed the
    // in-loop releases, making the dead param the lineage's sole terminal owner.
    // DISJOINT root kind from the forwarder-identity + sum-aggregate-Construct-fed
    // scans above (union-fired back-edge param roots only); the contains-gated
    // push below keeps the merge idempotent regardless. SSOT:
    // `compute_rebuild_lineage_dead_param_releases`.
    let rebuild_lineage_releases = compute_rebuild_lineage_dead_param_releases(
        func,
        &sibling_union_outcome.fired_roots,
        &alias_table.genuine_same_alloc_reps,
        &param_edge_args,
    );
    merge_release_vars(&mut dead_forwarder_param_releases, rebuild_lineage_releases);
    // RL-5 release for a purely-dead loop-invariant fresh-collection local: a
    // fresh `Construct List/Map/Set` threaded UNCHANGED through loop block-params
    // and NEVER read (`let root = [1]; for .. { xs = xs.push(..) }; xs[k]` —
    // `root` dead). The loop back-edge fractures the union-find lineage so the
    // construct-fed scan declines it; this self-contained scan (decoupled from
    // the keystone same-alloc union per the reverted broad-union dead-end) emits
    // ONE RL-5 dead-at-entry dec at the lineage's terminal dead block-param.
    // DISJOINT root kind from the scans above (purely-dead loop-invariant
    // Construct, threaded-only, never read); the contains-gated push keeps the
    // merge idempotent. Empty when
    // `ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE=1`. SSOT:
    // `compute_loop_invariant_dead_local_releases`. Spec: Annex E §AIMS RL-5.
    let loop_invariant_dead_releases =
        compute_loop_invariant_dead_local_releases(func, &owned_vars_needing_rc);
    merge_release_vars(
        &mut dead_forwarder_param_releases,
        loop_invariant_dead_releases,
    );

    // RL-4 branch-exclusive terminal-move edge release: a FRESH local
    // `Construct` lineage consumed at an owned position on a strict subset of
    // branch paths leaves the pre-branch funding inc unmatched on each
    // non-consuming sibling path (+1 per call). One ADDITIVE `BurdenDec(root)`
    // lands after the path's final lineage read on each admitted edge, merged
    // into the same placed-release surface as the forwarder-result releases
    // (contains-gated — never two releases of one root at one position).
    // Toggle `ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE=1` (the compute fn
    // owns it). SSOT: `compute_branch_exclusive_edge_releases`. Spec: Annex E
    // §AIMS RL-4 + RL-1 + RL-2.
    let branch_exclusive_releases = compute_branch_exclusive_edge_releases(
        func,
        &owned_vars_needing_rc,
        &inc_suppressed_vars,
        &full_move_vars,
        &genuine_dup_call_arg_aliases,
        contracts,
    );
    merge_release_vars(
        &mut forwarder_result_releases,
        branch_exclusive_releases.releases,
    );
    // RL-4 dead-on-edge birth release for fully-dead non-consuming edges: a
    // NO-USE admitted target has TWO outstanding references (birth + kept
    // funding inc) and no RL-2 last-use anchor, so it owes a SECOND dec —
    // emitted through the block-entry dead-param release surface (a distinct
    // surface from the funded-duplicate release above, so the pair lands as
    // two decs at the same entry). Spec: Annex E §AIMS RL-4 + RL-2.
    merge_release_vars(
        &mut dead_forwarder_param_releases,
        branch_exclusive_releases.dead_edge_birth_releases,
    );

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
        sharing_view_surplus_inc_dsts: &sharing_view_surplus_inc_dsts,
        cow_terminal_concat_inc_dsts: &cow_terminal_concat_inc_dsts,
        fresh_call_result_borrowed_arg_inc_dsts: &fresh_call_result_borrowed_arg_inc_dsts,
        borrowed_store_dup_args: &borrowed_store_dup_args,
        yield_identity_push_dup_args: &yield_identity_push_dup_args,
        call_arg_dup_aliases: &genuine_dup_call_arg_aliases,
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
    // RL-2 + RL-4 no-sink borrowed-`Invoke` claim: a carrier var whose inline
    // terminator dec was suppressed (no dead-param sink) carries NO in-body
    // burden ops, so `populate_burden_emitted` leaves its bit false and the
    // landed Category-2 `release_with_burden_edge` would drop the paired
    // `BurdenDec` (the `carries_burden` gate). Set the bit so Cat-2 emits the
    // per-edge `BurdenDec` (lowered to the real `RcDec` on each dying successor
    // edge) — the receiver is released exactly once per executing path. Spec:
    // Annex E §AIMS RL-2 + RL-4.
    for var in &claimed_no_sink_vars {
        super::mark_emitted(&mut func.burden_emitted, var.index());
    }
    emit_unused_scalar_user_drop_decs(func, type_registry, &owned_vars_needing_rc);
    ctx
}

/// RL-DROP completeness: a never-used scalar-repr local whose type carries a
/// user `@drop` gets NO last-use anchor (`detect_last_uses` records no point for
/// a zero-use var) and NO predicate-stack dead-value dec (the legacy path skips
/// `Scalar` repr), so its `@drop` would be lost. Emit EXACTLY ONE scope-exit
/// `BurdenDec` (lowered Phase-7 to `RcDec { UserDrop }`) at the end of the
/// defining block's body. Fires on BOTH paths: the predicate stack emits nothing
/// competing for a scalar, so there is no double-free. Spec: Annex E §AIMS
/// RL-DROP (`RLDROP_exactly_once_on_glue`) + RL-2 (unused-owned dec).
fn emit_unused_scalar_user_drop_decs(
    func: &mut ArcFunction,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) {
    let mut used: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            used.extend(instr.used_vars().iter().copied());
        }
        used.extend(block.terminator.used_vars().iter().copied());
    }
    let mut to_emit: Vec<(usize, ArcVarId)> = Vec::new();
    for &var in owned_vars_needing_rc {
        if used.contains(&var)
            || func
                .burden_emitted
                .get(var.index())
                .copied()
                .unwrap_or(false)
            || !super::is_provably_scalar_repr(func, var)
        {
            continue;
        }
        if !type_has_user_drop(func.var_type(var), type_registry) {
            continue;
        }
        // Why: `defines_var` covers every definition site — body instrs, block
        // params, AND Invoke/InvokeIndirect terminator results — so a never-used
        // scalar+`@drop` local defined by a may-unwind Invoke result is not missed.
        if let Some(block_idx) = func.blocks.iter().position(|b| b.defines_var(var)) {
            to_emit.push((block_idx, var));
        }
    }
    for (block_idx, var) in to_emit {
        func.blocks[block_idx]
            .body
            .push(ArcInstr::BurdenDec { var });
        super::mark_emitted(&mut func.burden_emitted, var.index());
    }
}
