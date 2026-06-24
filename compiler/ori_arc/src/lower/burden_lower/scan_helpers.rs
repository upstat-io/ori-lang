//! Phase-5 burden-emission scan helpers — the owned-RC suppression-filter phase
//! and the per-scan `apply_*` lineage wrappers.
//!
//! `compute_owned_rc_filter` runs the contiguous prologue of the burden walk:
//! the `owned_vars_needing_rc` exclusion sequence (scalar-repr / immortal /
//! borrowed-alias / borrowed-arg / borrowed-projection / scalar-literal /
//! iter-element / forwarder-identity retains) plus the lineage scans
//! (construct-fed dead-param, fresh-sum live-extract, borrowed-`Invoke`
//! collection, forwarder-result) and the placed-release merges, returning the
//! settled filter state the emission-assembly phase in `scan_orchestration`
//! consumes. Spec: Annex E §AIMS RL-1..RL-5 + L-9.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::cow_aliases::{compute_borrowed_alias_vars, compute_scalar_literal_vars};
use super::ctx::BurdenLowerCtx;
use super::ownership_scans::borrowed_invoke_lineage_release_disabled;
use super::ownership_scans::{
    collect_owned_burdens, compute_borrowed_arg_let_aliases,
    compute_borrowed_invoke_collection_lineage, compute_borrowed_projection_dsts,
    compute_catch_recover_release_lineage, compute_closure_extract_borrow_view_lineage,
    compute_construct_fed_dead_param_lineage, compute_forwarder_identity_transparent_aliases,
    compute_forwarder_result_under_release, compute_fresh_sum_live_extract_lineage,
    compute_iter_consume_dead_thread_orphan_inc, compute_lazy_iter_closure_borrow_lineage,
    compute_loop_closure_dead_param_lineage, compute_multi_exit_borrow_view_lineage,
    compute_nested_construct_return_passthrough, compute_owned_vars_needing_rc,
    compute_retain_aliasing_lineage, detect_last_uses, detect_transfer_points,
    extend_owner_last_use_for_borrow_views, group_last_uses_filtered, ConstructFedDeadParamLineage,
};
use super::{
    is_provably_scalar_repr, mark_emitted, ownership_scans, PlacedReleaseMap,
    CATCH_RECOVER_RELEASE_DISABLED, CLOSURE_EXTRACT_BORROW_VIEW_RELEASE_DISABLED,
    CONSTRUCT_FED_DEAD_PARAM_RELEASE_DISABLED, FORWARDER_IDENTITY_ALIAS_DEDUP_DISABLED,
    FORWARDER_RESULT_RELEASE_DISABLED, FRESH_SUM_LIVE_EXTRACT_RELEASE_DISABLED,
    ITER_CONSUME_DEAD_THREAD_ORPHAN_INC_DISABLED, LAZY_ITER_CLOSURE_BORROW_RELEASE_DISABLED,
    LOOP_CLOSURE_DEAD_PARAM_RELEASE_DISABLED, MULTI_EXIT_BORROW_VIEW_RELEASE_DISABLED,
    NESTED_CONSTRUCT_RETURN_PASSTHROUGH_DISABLED, RETAIN_ALIASING_RELEASE_DISABLED,
};

/// Settled output of the burden-walk suppression-filter phase consumed by the
/// emission-assembly phase (`scan_orchestration::emit_burden_ops`).
pub(super) struct OwnedRcFilter {
    /// Vars that survived every exclusion retain + lineage suppression — the
    /// set the emission walk emits paired burden ops for.
    pub(super) owned_vars_needing_rc: FxHashSet<ArcVarId>,
    /// Borrowed-derived locals (fixpoint over `Let { Var }` hops) — reused by
    /// the COW-inc set computation.
    pub(super) borrowed_aliases: FxHashSet<ArcVarId>,
    /// Forwarder-identity transparent aliases — reused by
    /// `compute_use_counts_and_dup_aliases`.
    pub(super) forwarder_identity_transparent_aliases: FxHashSet<ArcVarId>,
    /// Placed releases for the forwarder-result / fresh-sum / borrowed-`Invoke`
    /// lineages, merged into one `ForwarderReleasePos`-keyed surface.
    pub(super) forwarder_result_releases: PlacedReleaseMap,
    /// Construct-fed dead-param lineage — its `releases` merge into the
    /// dead-forwarder-param release map in the emission phase.
    pub(super) construct_fed_dead_param: ConstructFedDeadParamLineage,
    /// Per-`(block, instr)` last-use sites filtered to `owned_vars_needing_rc`.
    pub(super) last_uses_at: FxHashMap<(usize, usize), Vec<ArcVarId>>,
    /// No-sink borrowed-`Invoke` carrier vars CLAIMED for the landed Category-2
    /// `deadAtSucc` per-edge release. Their `func.burden_emitted` bit is set
    /// after `populate_burden_emitted` so Cat-2's `release_with_burden_edge`
    /// admits the paired `BurdenDec` even though the var carries NO in-body
    /// burden ops (the inline dec was suppressed). Spec: Annex E §AIMS RL-2 + RL-4.
    pub(super) claimed_no_sink_vars: FxHashSet<ArcVarId>,
    /// Execution-final read aliases designated as the single release carrier
    /// for a multi-read call-result-aggregate element lineage — their
    /// keep-alive inc is suppressed in `scan_orchestration` so the lone
    /// last-use `BurdenDec` survives. SSOT:
    /// `compute_call_result_element_final_read_releases`.
    pub(super) final_read_release_aliases: FxHashSet<ArcVarId>,
    /// Owner aggregates whose own in-block last-use `BurdenDec` is SUPPRESSED by
    /// the TF-14 owner-drop borrow-view liveness extension (the borrowed-`Invoke`
    /// case relocates the release to the normal-successor `BlockEntry`). Merged
    /// into `transfer_via_move_alias` (dec-only suppression) in
    /// `scan_orchestration`. SSOT: `extend_owner_last_use_for_borrow_views`.
    pub(super) owner_borrow_view_dec_suppress: FxHashSet<ArcVarId>,
}

/// Run the burden-walk suppression-filter prologue: populate `ctx`, compute the
/// initial owned set, apply every exclusion retain + lineage suppression, and
/// merge the placed releases. Returns the settled [`OwnedRcFilter`]. The caller
/// owns `ctx` (the function's return value); this borrows it `&mut`.
#[expect(
    clippy::too_many_lines,
    reason = "single contiguous suppression-filter prologue: the exclusion \
              retains and the lineage scans run in a load-bearing order \
              (construct-fed before fresh-sum before borrowed-Invoke), and \
              splitting mid-sequence fragments the gate-(b) claimed-member \
              threading"
)]
pub(super) fn compute_owned_rc_filter<'a>(
    ctx: &mut BurdenLowerCtx<'a>,
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
    immortals: &[bool],
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> OwnedRcFilter {
    collect_owned_burdens(ctx, func, type_registry);
    detect_transfer_points(ctx, func, type_registry);
    detect_last_uses(ctx, func);
    // TF-14 owner-drop placement extension: a SOLE-owned-RC-field inline aggregate
    // whose projected borrow-view is read at a borrowed-`Invoke` terminator arg has
    // its whole-var `[AggFields]` release relocated to the normal-successor
    // `BlockEntry` (merged into `forwarder_result_releases` below) + its own in-block
    // dec suppressed — the owner drop cascade-frees the projected field, so it must
    // land AFTER the borrow-read (else UAF). Gated on the SAME same-allocation
    // sole-owned-field discriminator as the surplus-dec arms (`arg_sole_owned_rc_field_is`),
    // NOT a structural Project-edge proxy. Spec: Annex E §AIMS TF-14 + RL-2 + RL-4.
    let owner_borrow_view_ext = extend_owner_last_use_for_borrow_views(&*ctx, func, type_registry);

    // `owned_vars_needing_rc` filters scalars whose `lookup_burden` returns
    // `Some(BurdenRef)` wrapping the empty builtin burden — required by AIMS
    // DP-1 (`is_rc_needed: Owned ∧ ¬Dead ∧ ¬is_scalar`) + VF-1 `RcOnScalar`.
    let mut owned_vars_needing_rc = compute_owned_vars_needing_rc(ctx);
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
    let mut borrowed_arg_aliases = compute_borrowed_arg_let_aliases(func);
    // RL-1 owned-call-arg duplication carve-out: an alias whose sole use is a
    // borrowed-ANNOTATED terminator-Invoke arg at a position the callee's
    // contract proves OWNED (the Phase-5 `arg_ownership` annotation is still
    // the pre-`realize_rc_reuse` borrowed default) is a genuine duplication,
    // NOT a borrow-view — it keeps its membership so the alias-site
    // `BurdenInc` is emitted (`compute_funded_call_arg_dup_aliases` SSOT;
    // the consumer's owned-param release is the matched release).
    let call_arg_dup_aliases =
        ownership_scans::compute_funded_call_arg_dup_aliases(func, contracts, interner);
    borrowed_arg_aliases.retain(|v| !call_arg_dup_aliases.contains(v));
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
    // RL-2 final-read release designation: a MULTI-read element of a fresh
    // caller-owned call-result aggregate (a returned tuple's `Project` view
    // with no burden carrier anywhere on its lineage) gets its execution-final
    // read alias re-included so that alias's lone last-use `BurdenDec` is the
    // element's single release (the alias's keep-alive inc is suppressed in
    // `scan_orchestration`). SSOT + decline fences:
    // `compute_call_result_element_final_read_releases`.
    let final_read_release_aliases =
        ownership_scans::compute_call_result_element_final_read_releases(
            func,
            contracts,
            &owned_vars_needing_rc,
        );
    owned_vars_needing_rc.extend(final_read_release_aliases.iter().copied());
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
        compute_construct_fed_dead_param_lineage(func, contracts, &owned_vars_needing_rc, interner)
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
    // scans (Invoke roots with `transfers_through_return` callees decline). SSOT
    // for the niche-family-sum match-extract RESULT lineage — runs BEFORE the
    // borrowed-`Invoke` scan so its claimed-member web is threaded forward and the
    // borrowed-`Invoke` RESULT-root family declines any overlapping closure (the
    // two would otherwise both place a death-point release on one allocation).
    // SSOT: `compute_fresh_sum_live_extract_lineage` (niche-family-sum + vetted
    // borrow-read-only closure + live-extract + execution-final-site gates bound
    // the over-fire surface). Cures the `match_arm_alias_option_str` family
    // both-paths-fail lineage; default-path-safe because the predicate stack
    // provably emits zero ops for this shape (predicate-only probe: bare alloc).
    let (fresh_sum_claimed, fresh_sum_releases) =
        apply_fresh_sum_live_extract(func, contracts, &mut owned_vars_needing_rc, type_registry);
    // RL-2 catch-recover release: a `catch(panic(...))` recovered `str` whose
    // `Result::Err(msg)` payload is match-extracted + borrow-read (`@chars` /
    // `@contains`) but never released on the NORMAL path — the walk's only dec
    // sits on the unwind / `Resume` edge. The recovered copy is a FRESH
    // `ori_catch_recover` builtin result with NO seeded contract (the deliberate
    // no-contract decision per `arc.md §Protocol Builtins`), so the fresh-sum
    // live-extract scan FORECLOSES it (`collect_fresh_sum_roots` requires a callee
    // contract; `ori_catch_recover` is a protocol builtin with no seeded contract
    // by deliberate design). This scan roots on the `ori_catch_recover` callee IDENTITY,
    // builds the borrow-only same-alloc closure (recover result + `Construct
    // Variant` Result wrap + niche-payload `Project` + Let-Var aliases +
    // Jump-threaded loop block-params), confirms every existing dec is unwind-only
    // (the MISSING-release shape), and places EXACTLY ONE whole-var `BurdenDec`
    // after the closure's execution-final GENUINE borrow-read on the normal path —
    // excluding the loop-carried keep-alive churn from the final-read computation.
    // Runs AFTER the fresh-sum scan (disjoint family — `ori_catch_recover`-rooted
    // closures vs contract-rooted niche-family sums); a root either scan claimed
    // auto-declines via gate (b)'s owned-set check. SSOT:
    // `compute_catch_recover_release_lineage`. Spec: Annex E §AIMS RL-2.
    let catch_recover_releases =
        apply_catch_recover_release(func, &mut owned_vars_needing_rc, interner);
    // RL-1 + RL-2 + RL-4 multi-exit borrow-view treatment: a FRESH closure
    // (`Apply`/`Invoke` result, `Unique ∧ preserves_freshness`, non-forwarder)
    // consumed ONLY at `ApplyIndirect` borrow-receiver sites across a
    // short-circuit CFG (`gt5(10) && !gt5(3)`) names ONE allocation across its
    // Let-Var alias closure. The per-var DP-2/DP-3 split frees the first var at
    // its last *var*-use (the `%dup = %root` alias-bind placed BEFORE the
    // execution-final `ApplyIndirect` read) — a use-after-free + double-free
    // (exit -134). The cure removes the whole closure from
    // `owned_vars_needing_rc` (every dup/keep-alive inc + the spurious pre-use
    // source dec) and places EXACTLY ONE whole-var release per terminal path
    // (RL-2 after the final borrow-read on each READING path; RL-4 edge dec at
    // each SHORT-CIRCUIT-BYPASS successor entry). Runs AFTER the fresh-sum scan
    // (disjoint family — closure FatValue root vs niche-family Aggregate sum).
    // SSOT: `compute_multi_exit_borrow_view_lineage`. Spec: Annex E §AIMS RL-1
    // + RL-2 + RL-4.
    let multi_exit_releases =
        apply_multi_exit_borrow_view(func, contracts, &mut owned_vars_needing_rc);
    // RL-1 + RL-2 + RL-4 retain-aliasing treatment: a FRESH niche-family sum
    // (`@__index` Option result) whose payload is read through an ACCESSOR-RETAIN
    // call (`@unwrap`/`@get`, a per-hop self-inc on the SAME allocation) across a
    // SHORT-CIRCUIT branchy CFG (`if v.is_some() && v.unwrap().starts_with(..)`).
    // The base walk emits the spurious dup-alias keep-alive incs + an INLINE
    // source dec BEFORE the accessor-retain call reads the payload → use-after-
    // free + double-free (exit -134: coll_map_index_int_str / set_str_union /
    // catch-cohort). The single-site fresh-sum live-extract scan is FORECLOSED
    // (ledger 212): the allocation is live on the BYPASS branch that reaches
    // Return WITHOUT passing the reader's site. The cure removes the whole same-
    // alloc closure (root + Let-Var aliases + niche-payload Projects + accessor-
    // retain results) from `owned_vars_needing_rc` and places a per-path release
    // for EACH retained reference (the niche-sum root for the `@__index` self-inc
    // + each accessor-retain result for its `@unwrap`/`@get` self-inc — ledger
    // 214/216): RL-2 after the final borrow-read on each READING path (a body
    // read, or a terminator-`Invoke` read released at its normal successor), RL-4
    // edge dec on each BYPASS edge — exactly-once-per-path-per-reference. Runs
    // AFTER the fresh-sum live-extract scan so a root that scan claimed (its
    // single-site shape) auto-declines (gate b owned-set check) — disjoint
    // families. SSOT: `compute_retain_aliasing_lineage`. Spec: Annex E §AIMS
    // RL-1 + RL-2 + RL-4 + TF-4.
    let retain_aliasing_releases = apply_retain_aliasing(
        func,
        contracts,
        &mut owned_vars_needing_rc,
        type_registry,
        interner,
    );
    // RL-2 + RL-4 closure-extract borrow-view treatment: N `ApplyIndirect`
    // results that are PROVEN same-allocation borrow-views of ONE captured field
    // of a closure env (the resolved lambda's `return_alias = Project { field }`
    // contract over its capture param — `let f = () -> { match captured {
    // Err(e) -> e, .. } }; let a = f(); let b = f()`) name ONE allocation. The
    // base walk decs EACH result borrow-view, double-freeing the captured payload
    // on call 2 (exit 134). The cure removes the whole result lineage (every
    // result + its Let-Var aliases) from `owned_vars_needing_rc` (suppressing the
    // N surplus per-result decs) and places EXACTLY ONE whole-var release per
    // terminal path (RL-2 after the execution-final borrow-read; RL-4 edge dec at
    // a dead normal-successor entry). Runs AFTER the multi-exit scan (disjoint
    // family — interprocedural closure-extract RESULT roots vs the intraprocedural
    // FRESH-closure RECEIVER roots). The PROVEN same-allocation identity is the
    // lambda contract's `Project` edge (NOT a use-count / type-membership proxy —
    // dead-end #150); the borrow-only vetting declines any owned-position escape
    // (the fresh-sum live-extract over-fire boundary — dead-ends #174/#175).
    // SSOT: `compute_closure_extract_borrow_view_lineage`. Spec: Annex E §AIMS
    // RL-2 + TF-4.
    apply_closure_extract_borrow_view(func, contracts, &mut owned_vars_needing_rc);
    // RL-5 dead-at-entry treatment for a FRESH `PartialApply` closure threaded
    // through a loop and dead at the post-loop block-param: `let f = (n) -> ..;
    // for i in xs do { .. f(i) .. }`. The fresh closure (TF-7) is loop-carried
    // via the loop's block-params, BORROW-used inside the body (an
    // `ApplyIndirect` receiver, or a borrowed `Apply`/`Invoke` call arg), and
    // DEAD at the post-loop merge param. `RL5_dead_at_entry_cleanup` owes ONE
    // immediate dec there; the base walk emits none (the borrowed-arg shape
    // leaks) or a spurious per-iteration borrow-receiver dec (the direct-call
    // shape double-frees the loop-invariant closure). Removes the whole same-
    // alloc lineage from `owned_vars_needing_rc` (suppressing the spurious ops)
    // and places ONE release at the dead post-loop param. Disjoint family
    // (loop-carried `PartialApply` FatValue roots vs call-result closures /
    // forwarder results / niche-family sums). SSOT:
    // `compute_loop_closure_dead_param_lineage`. Spec: Annex E §AIMS RL-5 +
    // RL-2 + RL-4.
    let loop_closure_releases = apply_loop_closure_dead_param(func, &mut owned_vars_needing_rc);
    // RL-2 lazy-iterator closure-borrow treatment: a FRESH `PartialApply` closure
    // borrowed into a lazy-iterator builtin (`@map`/`@filter`) whose result
    // iterator retains the closure env as a borrowed raw pointer (the runtime
    // adapter never incs/drops the env — dead-end #154) across the chain's
    // terminal consumer (`@collect`/…). The base walk places the closure's
    // `BurdenDec` at its last var-use (the `@map`/`@filter` borrowed arg), BEFORE
    // the lazy iterator runs the closure at `@collect` time — freeing the env (and
    // its cascade-freed captured heap payload) while the iterator still holds the
    // borrowed env pointer → use-after-free (exit −139). The cure removes the
    // whole same-alloc closure from `owned_vars_needing_rc` (suppressing the early
    // dec) and places EXACTLY ONE whole-var release at the iterator chain's
    // terminal consumer's normal-successor entry (`RL2_release_exactly_once`:
    // released once after the closure's final run). `@fold` is EAGER (closure runs
    // synchronously) and is excluded. Disjoint family (lazy-builtin-borrowed
    // FRESH-closure roots vs loop-carried closures / call-result closures /
    // forwarder results / niche-family sums). SSOT:
    // `compute_lazy_iter_closure_borrow_lineage`. Spec: Annex E §AIMS RL-2 + TF-4
    // + TF-7.
    let lazy_iter_closure_releases =
        apply_lazy_iter_closure_borrow(func, &mut owned_vars_needing_rc, interner);
    // RL-1 + RL-2 iter-consume dead-thread orphaned-inc elision: a fresh
    // collection (`let xs = [..]; let ys = for x in xs.iter() yield ...`) whose
    // SOLE genuine consume is the inline for-loop's `@iter [own]` (Invoke @iter
    // terminator / Apply @iter; ori_iter_drop frees it) AND whose Jump-arg thread
    // across the loop back-edge terminates in a DEAD param (never read post-loop).
    // The base walk's use-count counts the dead-thread as a 2nd live use ->
    // classifies xs duplicated -> emits an orphaned FRESH-site keep-alive
    // BurdenInc whose scope-exit dec is RL-2 transfer-suppressed by @iter -> +1
    // leak. The dead-thread is a JumpArg transfer (RL-4 exemption) into a dead
    // param, NOT a genuine duplication, so xs is move-once into @iter and the inc
    // is elidable (RL1_duplication_balanced move-once nets 0). Removes the whole
    // same-alloc dead-thread lineage from owned_vars_needing_rc. Follows Jump-args
    // across ALL edges (forward + back) via compute_param_edge_args — the
    // back-edge inclusion the foreclosed forward-only scans (#163/#164/#185)
    // lacked; does NOT relax the @iter COW-taint (#181's over-fire). SSOT:
    // `compute_iter_consume_dead_thread_orphan_inc`. Spec: Annex E §AIMS RL-1 +
    // RL-2.
    apply_iter_consume_dead_thread_orphan_inc(
        func,
        &mut owned_vars_needing_rc,
        contracts,
        interner,
    );
    // RL-2 in-callee container-passthrough suppression: a param moved into a
    // chain of fresh struct/tuple Constructs and projected back out
    // (`Project (Construct args) field == args[field]`) as the Return value has
    // the whole wrapper-chain lineage removed from owned_vars_needing_rc — the
    // wrapper owns nothing surviving (its sole owned content IS the transferred-
    // out param), so the base walk's container drop is a surplus release that
    // frees the returned nested field (in-callee UAF). Pairs with the caller-side
    // construct-project round-trip Direct return-alias. SSOT:
    // `compute_nested_construct_return_passthrough`
    // (`wrapper_passthrough_release_exactly_once` nets 0). Spec: Annex E §AIMS
    // RL-2 + TF-3 + TF-4.
    apply_nested_construct_return_passthrough(
        func,
        &mut owned_vars_needing_rc,
        contracts,
        type_registry,
    );
    // RL-2 + RL-4 borrowed-`Invoke`-collection lineage treatment: a FRESH
    // collection-`Construct` buffer (`%2 = Construct List(..)`) read through
    // Let-Var aliases + a length `Project` and BORROWED into a may-unwind
    // `Invoke @__index(%5 [borrow], ..)` terminator, whose lineage is
    // LIVE-ACROSS the catch (read again past it — `xs.len()` / `xs[0]`). The
    // construct-fed dead-param collection arm DECLINED it (call-site-count > 1),
    // so the base walk OVER-emits: a dup-alias `BurdenInc` + an inline
    // `BurdenDec %5` BEFORE the terminator that reads `%5` → use-after-free /
    // double-free on the still-live `%2`. The cure
    // removes the whole same-alloc closure from `owned_vars_needing_rc`
    // (suppressing the dup incs + the inline terminator dec) and places EXACTLY
    // ONE whole-var release at the lineage's execution-final borrow-read. The
    // dying unwind / unreachable edges are released by the Surface-1 Category-2
    // `deadAtSucc` conjunct (`edge_cleanup.rs`) — disjoint edges. Runs AFTER the
    // construct-fed retain so dead-param-claimed roots auto-decline (gate b) AND
    // AFTER the live-extract scan so its claimed-member web declines overlapping
    // RESULT-root closures (gate b'). SSOT:
    // `compute_borrowed_invoke_collection_lineage` (collection-Construct
    // root + vetted borrow-read-only closure + borrowed-Invoke-arg + execution
    // -final-site + pairwise-disjoint gates bound the over-fire surface). Spec:
    // Annex E §AIMS RL-2 + RL-4.
    let (borrowed_invoke_releases, mut claimed_no_sink_vars) =
        apply_borrowed_invoke_collection_lineage(
            func,
            &mut owned_vars_needing_rc,
            &construct_fed_dead_param.suppressed_lineage_vars,
            &fresh_sum_claimed,
            contracts,
            interner,
        );
    // RL-2 + RL-4 sole-carrier borrowed-`Invoke` aliases (settled-set scan —
    // runs AFTER every lineage suppression so "sole carrier" is evaluated
    // against the final per-value ledger): an alias whose SOLE use is a
    // borrowed may-unwind `Invoke` arg while its src is read nowhere else
    // carries the lineage's ONLY release — and the base walk would place it
    // INLINE before the borrowed terminator (early release: use-after-free /
    // double-free when the callee aliases the value into its result, the
    // `wrap_ok(m: m)` mint shape). Remove the alias from the owned set
    // (suppressing the early dec) and CLAIM it for the Category-2 `deadAtSucc`
    // per-edge release: the alias is dead at every successor of its carrier
    // `Invoke`, so each executing path releases exactly once AFTER the call
    // (the claim sets `burden_emitted` post-emission so Cat-2's
    // `release_with_burden_edge` admits the paired dec). SSOT + gates:
    // `compute_sole_carrier_borrowed_invoke_aliases`. Spec: Annex E §AIMS
    // RL-2 + RL-4.
    let sole_carrier_claims = ownership_scans::compute_sole_carrier_borrowed_invoke_aliases(
        func,
        &owned_vars_needing_rc,
        contracts,
        interner,
    );
    owned_vars_needing_rc.retain(|v| !sole_carrier_claims.contains(v));
    claimed_no_sink_vars.extend(sole_carrier_claims.iter().copied());
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
    // Merge the catch-recover normal-path releases into the same surface.
    // Disjoint family (`ori_catch_recover`-rooted message closures vs forwarder
    // results / contract-rooted niche-family sums / closures), so the merge
    // cannot double-release.
    for (site, vars) in catch_recover_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the multi-exit borrow-view per-path releases into the same surface.
    // Disjoint family (FatValue closure roots vs forwarder results / niche-family
    // sums), so the merge cannot double-release.
    for (site, vars) in multi_exit_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the retain-aliasing per-path-per-reference releases into the same
    // surface. Disjoint family (niche-family-sum + accessor-retain closure roots
    // vs FatValue multi-exit closures / forwarder results / loop-carried
    // closures), so the merge cannot double-release.
    for (site, vars) in retain_aliasing_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the loop-closure dead-param releases into the same surface. Disjoint
    // family (loop-carried PartialApply FatValue roots vs call-result closures /
    // forwarder results / niche-family sums), so the merge cannot double-release.
    for (site, vars) in loop_closure_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the lazy-iterator closure-borrow releases into the same surface.
    // Disjoint family (lazy-builtin-borrowed FRESH-closure roots vs loop-carried
    // closures / call-result closures / forwarder results / niche-family sums),
    // so the merge cannot double-release.
    for (site, vars) in lazy_iter_closure_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the borrowed-`Invoke`-collection lineage releases into the same
    // surface. Disjoint family (fresh collection-buffer roots vs forwarder
    // results / niche-family sums), so the merge cannot double-release.
    for (site, vars) in borrowed_invoke_releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    // Merge the TF-14 owner-drop borrow-view relocated releases (borrowed-`Invoke`
    // case → normal-successor `BlockEntry`) into the same surface. Disjoint family
    // (owner aggregates whose borrow-view survives a borrowed call vs forwarder
    // results / niche-family sums / closures), so the merge cannot double-release.
    for (site, vars) in owner_borrow_view_ext.releases {
        forwarder_result_releases
            .entry(site)
            .or_default()
            .extend(vars);
    }
    let last_uses_at = group_last_uses_filtered(ctx, &owned_vars_needing_rc);
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
    OwnedRcFilter {
        owned_vars_needing_rc,
        borrowed_aliases,
        forwarder_identity_transparent_aliases,
        forwarder_result_releases,
        construct_fed_dead_param,
        last_uses_at,
        claimed_no_sink_vars,
        final_read_release_aliases,
        owner_borrow_view_dec_suppress: owner_borrow_view_ext.suppress,
    }
}

/// Invoke / Apply transfer-through-return result→arg move-edges: when a callee
/// transfers an owned param THROUGH its return, the call result IS the
/// forwarded arg (a move across the call), so the move-alias chain must span
/// it. Consumed by `compute_transfer_via_move_alias`.
pub(super) fn collect_invoke_ttr_edges(
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

/// Toggle-gated application of the RL-2 + RL-4 borrowed-`Invoke`-collection
/// lineage treatment ([`compute_borrowed_invoke_collection_lineage`]): computes
/// the vetted same-alloc closures, removes them from `owned_vars_needing_rc`
/// (suppressing the dup-alias incs + the inline terminator dec), and returns a
/// pair: the placed dead-param releases for the `forwarder_result_releases`
/// merge, and the no-sink carrier vars claimed for the Category-2 per-edge
/// release. Empty when `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1`.
fn apply_borrowed_invoke_collection_lineage(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    claimed_by_construct_fed: &FxHashSet<ArcVarId>,
    claimed_by_live_extract: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> (PlacedReleaseMap, FxHashSet<ArcVarId>) {
    if borrowed_invoke_lineage_release_disabled() {
        return (FxHashMap::default(), FxHashSet::default());
    }
    let treatment = compute_borrowed_invoke_collection_lineage(
        func,
        owned_vars_needing_rc,
        claimed_by_construct_fed,
        claimed_by_live_extract,
        contracts,
        interner,
    );
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    (treatment.releases, treatment.claimed_no_sink_vars)
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
) -> (FxHashSet<ArcVarId>, PlacedReleaseMap) {
    if *FRESH_SUM_LIVE_EXTRACT_RELEASE_DISABLED {
        return (FxHashSet::default(), FxHashMap::default());
    }
    let treatment = compute_fresh_sum_live_extract_lineage(
        func,
        contracts,
        owned_vars_needing_rc,
        type_registry,
    );
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    (treatment.suppressed_lineage_vars, treatment.releases)
}

/// Toggle-gated application of the RL-2 catch-recover missing-release treatment
/// ([`compute_catch_recover_release_lineage`]): computes the vetted borrow-only
/// `ori_catch_recover`-rooted same-alloc closure, removes it from
/// `owned_vars_needing_rc` (suppressing the keep-alive incs + unwind-only decs),
/// and returns the single placed normal-path release for the
/// `forwarder_result_releases` merge. Empty when
/// `ORI_DISABLE_CATCH_RECOVER_RELEASE=1`.
fn apply_catch_recover_release(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    interner: &StringInterner,
) -> PlacedReleaseMap {
    if *CATCH_RECOVER_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_catch_recover_release_lineage(func, owned_vars_needing_rc, interner);
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Toggle-gated application of the RL-1 + RL-2 + RL-4 + TF-4 retain-aliasing
/// treatment ([`compute_retain_aliasing_lineage`]): computes the vetted
/// niche-family-sum + accessor-retain same-alloc closures, removes them from
/// `owned_vars_needing_rc` (suppressing the dup keep-alive incs + the inline
/// source dec), and returns the per-path-per-reference placed releases for the
/// `forwarder_result_releases` merge. Empty when
/// `ORI_DISABLE_RETAIN_ALIASING_RELEASE=1`.
fn apply_retain_aliasing(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> PlacedReleaseMap {
    if *RETAIN_ALIASING_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_retain_aliasing_lineage(
        func,
        contracts,
        owned_vars_needing_rc,
        type_registry,
        interner,
    );
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Toggle-gated application of the RL-1 + RL-2 + RL-4 multi-exit borrow-view
/// treatment ([`compute_multi_exit_borrow_view_lineage`]): computes the vetted
/// same-alloc closures, removes them from `owned_vars_needing_rc` (suppressing
/// every dup/keep-alive inc + the spurious pre-use source dec), and returns the
/// placed per-terminal-path releases for the `forwarder_result_releases` merge.
/// Empty when `ORI_DISABLE_MULTI_EXIT_BORROW_VIEW_RELEASE=1`.
fn apply_multi_exit_borrow_view(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
) -> PlacedReleaseMap {
    if *MULTI_EXIT_BORROW_VIEW_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_multi_exit_borrow_view_lineage(func, contracts, owned_vars_needing_rc);
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Toggle-gated application of the closure-extract borrow-view treatment
/// ([`compute_closure_extract_borrow_view_lineage`]): computes the vetted
/// same-alloc `ApplyIndirect`-result lineages and removes them from
/// `owned_vars_needing_rc` (suppressing the N surplus per-result decs). Places
/// NO release — the closure env's own scope-exit dec cascade-frees the captured
/// payload. No-op when `ORI_DISABLE_CLOSURE_EXTRACT_BORROW_VIEW_RELEASE=1`.
fn apply_closure_extract_borrow_view(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
) {
    if *CLOSURE_EXTRACT_BORROW_VIEW_RELEASE_DISABLED {
        return;
    }
    let treatment =
        compute_closure_extract_borrow_view_lineage(func, contracts, owned_vars_needing_rc);
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
}

/// Toggle-gated application of the RL-5 loop-closure dead-param treatment
/// ([`compute_loop_closure_dead_param_lineage`]): computes the vetted loop-
/// carried `PartialApply` closure lineages, removes them from
/// `owned_vars_needing_rc` (suppressing the spurious per-iteration ops + net-0
/// keep-alive pairs), and returns the single placed RL-5 dead-at-entry release
/// per lineage for the `forwarder_result_releases` merge. Empty when
/// `ORI_DISABLE_LOOP_CLOSURE_DEAD_PARAM_RELEASE=1`.
fn apply_loop_closure_dead_param(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
) -> PlacedReleaseMap {
    if *LOOP_CLOSURE_DEAD_PARAM_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_loop_closure_dead_param_lineage(func, owned_vars_needing_rc);
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Toggle-gated application of the RL-2 lazy-iterator closure-borrow treatment
/// ([`compute_lazy_iter_closure_borrow_lineage`]): computes the vetted fresh-
/// closure lineages borrowed into `@map`/`@filter`, removes them from
/// `owned_vars_needing_rc` (suppressing the early borrowed-arg dec the base walk
/// places at the lazy builtin), and returns the single placed RL-2 release at
/// the iterator chain's terminal consumer's normal-successor entry for the
/// `forwarder_result_releases` merge. Empty when
/// `ORI_DISABLE_LAZY_ITER_CLOSURE_BORROW_RELEASE=1`.
fn apply_lazy_iter_closure_borrow(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    interner: &StringInterner,
) -> PlacedReleaseMap {
    if *LAZY_ITER_CLOSURE_BORROW_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let treatment = compute_lazy_iter_closure_borrow_lineage(func, owned_vars_needing_rc, interner);
    owned_vars_needing_rc.retain(|v| !treatment.suppressed_lineage_vars.contains(v));
    treatment.releases
}

/// Toggle-gated application of the RL-1 + RL-2 iter-consume dead-thread
/// orphaned-inc elision ([`compute_iter_consume_dead_thread_orphan_inc`]):
/// computes the vetted fresh-collection dead-thread lineages (a fresh collection
/// iter-consumed by an inline for-loop whose loop-carried thread is dead
/// post-loop) and removes them from `owned_vars_needing_rc` — eliding the orphan
/// FRESH-site inc + the symmetric per-thread pairs. Places NO release (the
/// `@iter`'s `ori_iter_drop` is the single buffer release). No-op when
/// `ORI_DISABLE_ITER_CONSUME_DEAD_THREAD_ORPHAN_INC=1`.
fn apply_iter_consume_dead_thread_orphan_inc(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) {
    if *ITER_CONSUME_DEAD_THREAD_ORPHAN_INC_DISABLED {
        return;
    }
    let suppressed = compute_iter_consume_dead_thread_orphan_inc(
        func,
        contracts,
        owned_vars_needing_rc,
        interner,
    );
    owned_vars_needing_rc.retain(|v| !suppressed.contains(v));
}

/// Toggle-gated application of the RL-2 in-callee container-passthrough
/// suppression ([`compute_nested_construct_return_passthrough`]): computes the
/// vetted fresh nested struct/tuple Construct chain whose deepest projection is
/// the function's Return value wrapping a transfers-through-return param, and
/// removes the whole wrapper-chain lineage from `owned_vars_needing_rc` — eliding
/// the surplus container drops that would free the transferred-out param. No-op
/// when `ORI_DISABLE_NESTED_CONSTRUCT_RETURN_PASSTHROUGH=1`.
fn apply_nested_construct_return_passthrough(
    func: &ArcFunction,
    owned_vars_needing_rc: &mut FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &TypeRegistry,
) {
    if *NESTED_CONSTRUCT_RETURN_PASSTHROUGH_DISABLED {
        return;
    }
    let suppressed = compute_nested_construct_return_passthrough(
        func,
        contracts,
        owned_vars_needing_rc,
        type_registry,
    );
    owned_vars_needing_rc.retain(|v| !suppressed.contains(v));
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
pub(super) fn populate_burden_emitted(func: &mut ArcFunction) {
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
