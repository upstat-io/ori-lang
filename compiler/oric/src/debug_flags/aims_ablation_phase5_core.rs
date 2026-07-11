//! AIMS Phase-5 (burden-lowering) ablation toggles — core release/dedup/funding family.
//!
//! Each `ORI_DISABLE_*`/`ORI_FORCE_*` toggle reverts one named optimization in
//! `ori_arc::lower::burden_lower` for bisection; default (unset) leaves the
//! cured behavior active. See the crate-level `debug_flags` module doc for the
//! `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
    /// Disable the post-realize redundant project-alias dec cleanup pass.
    ///
    /// Consumed in `ori_arc::aims::realize::cleanup_redundant`.
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Usage: `ORI_DISABLE_REDUNDANT_CLEANUP=1 ori build file.ori`
    ORI_DISABLE_REDUNDANT_CLEANUP

    /// Disable the burden-op emission pass (Step 4b of the AIMS pipeline).
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Burden
    /// emission is the sole RC-emission input (no fallback emitter exists), so
    /// setting this aborts compilation via the fail-loud migration gate for any
    /// function needing RC management — a negative-pin probe, never a build mode.
    /// Usage: `ORI_DISABLE_BURDEN_OPS=1 ori build file.ori`
    ORI_DISABLE_BURDEN_OPS

    /// Disable the Phase-5 RL-2 mutable-rebind release scan.
    ///
    /// Consumed in `ori_arc::lower::burden_lower::ownership_scans::reassign_release`.
    /// When set, the gated `BurdenDec(old_var)` at a self-referential rebind is
    /// not emitted (bisects a reassignment leak / double-free to this scan).
    /// Usage: `ORI_DISABLE_REASSIGN_REBIND_RELEASE=1 ori build file.ori`
    ORI_DISABLE_REASSIGN_REBIND_RELEASE

    /// Dump each function's ARC IR to stderr immediately after Step 4b
    /// `emit_burden_ops`, before any realization.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Surfaces
    /// the faithful Phase-5 `BurdenInc` / `BurdenDec*` emission for VF-1 residual
    /// localization (post-realize `ORI_DUMP_AFTER_ARC` cannot show pre-realize
    /// burden placement).
    /// Usage: `ORI_DUMP_AFTER_BURDEN=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN

    /// Dump each function's ARC IR to stderr immediately after the DP-2/DP-3
    /// burden-op elimination pass.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Surfaces
    /// the post-elimination `BurdenInc` / `BurdenDec*` set so the eliminator's
    /// effect can be bisected against the pre-elimination `ORI_DUMP_AFTER_BURDEN`
    /// snapshot.
    /// Usage: `ORI_DUMP_AFTER_BURDEN_ELIM=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN_ELIM

    /// Disable the predicate-stack `RcInc` / `RcDec` realization phases, leaving
    /// the burden path (elimination + mechanical `BurdenInc → RcInc` /
    /// `BurdenDec → RcDec` lowering) as the sole real-RC emitter.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` to drive the burden-path
    /// self-sufficiency probe. When set, the predicate-stack emission is suppressed
    /// and surviving whole-var burden ops lower to real RC; default (unset) leaves
    /// the default emission path byte-identical.
    /// Usage: `ORI_DISABLE_PREDICATE_STACK_RC=1 ori build file.ori`
    ORI_DISABLE_PREDICATE_STACK_RC

    /// Output path for the AIMS RC-survivor remark stream (JSONL).
    ///
    /// Consumed in `ori_arc::aims::realize::rc_remark` (which can't depend on
    /// `oric`). Defined here for documentation and `check-debug-flags.sh`
    /// consistency. CLI alternative: `--emit-rc-remarks <path>` (composes the
    /// burden-sole-path gating so the stream is a valid verdict surface).
    /// Valid verdict only on the burden-sole path (`ORI_DISABLE_PREDICATE_STACK_RC=1`).
    /// Usage: `ORI_RC_REMARKS=out.jsonl ORI_DISABLE_PREDICATE_STACK_RC=1 ori build file.ori`
    ORI_RC_REMARKS

    /// Disable the lineage-aware Phase-6 burden-op re-balance (group-by-rep
    /// net-targeted one-release elision) in `ori_arc::aims::realize::burden_elim`,
    /// falling back to the decoupled per-var elision.
    ///
    /// Diagnostic bisection only: toggles the lineage re-balance off so an
    /// alias-chain double-free can be attributed to the rep-grouping vs the
    /// per-var pass; default (unset) keeps the re-balance active.
    /// Usage: `ORI_DISABLE_LINEAGE_REBALANCE=1 ori build file.ori`
    ORI_DISABLE_LINEAGE_REBALANCE

    /// Decline the Phase-6 class-grain whole-pair elision (RL-22/23/25 with
    /// T3 sibling-liveness evidence) in
    /// `ori_arc::aims::realize::burden_elim::class_grain`, restoring the
    /// per-var-only disposition (every kept keep-alive pair survives).
    ///
    /// Diagnostic bisection only: attributes a class-grain-elision regression
    /// vs the per-var DP-2/DP-3 path; default (unset) keeps the pass active.
    /// Usage: `ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION=1 ori build file.ori`
    ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION

    /// Revert the Phase-6 lineage-rebalance single-release selection to
    /// terminal-net-only placement. Default: the kept whole-var release is
    /// rejected when a borrow-read of the rep is forward-reachable from its
    /// position (release-before-read is a UAF / double-free — RL-2 requires the
    /// single release after the lineage's LAST read; the `copy[0]` then `copy[1]`
    /// yield-identity shape frees the list buffer in an early block while a later
    /// `@__index(alias)` still reads it). With the toggle set, the selection
    /// accepts any per-path-net-zero dec regardless of a later read, restoring
    /// the early-placement double-free.
    ///
    /// Consumed in `ori_arc::aims::realize::burden_elim` (Phase 6). Bisects a
    /// lineage-rebalance read-after-release double-free to this filter.
    /// Usage: `ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ=1 ori build file.ori`
    ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ

    /// Disable the Phase-5 RL-5 dead-at-entry release for forwarder-identity
    /// allocations reaching a merge/return block's dead block-params.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a
    /// forwarder-dead-block-param leak / double-free to that emission.
    /// Usage: `ORI_DISABLE_DEAD_FORWARDER_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_DEAD_FORWARDER_PARAM_RELEASE

    /// Disable the Phase-5 RL-5 dead-at-entry release + spurious-op suppression
    /// for sum-aggregate-`Construct`-fed allocations reaching dead block-params.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a
    /// construct-fed-dead-param leak / double-free to that lineage cure.
    /// Usage: `ORI_DISABLE_CONSTRUCT_FED_DEAD_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_CONSTRUCT_FED_DEAD_PARAM_RELEASE

    /// Restrict the construct-fed-dead-param scan's gate (a) to the
    /// sum-aggregate-`Construct` root, declining the FRESH collection-buffer
    /// (`ListLiteral`/`MapLiteral`/`SetLiteral`) + `Let { Literal::String }`
    /// heap-buffer arm.
    ///
    /// Default (unset): a fresh list/map/set/str borrowed into a may-unwind
    /// user-call `Invoke` whose lineage dies at a merge/return DEAD block-param
    /// (the `catch(expr: callee(coll))` shape) gets its sole RL-5 dead-at-entry
    /// release — curing the borrowed-Invoke-arg buffer leak. The collection arm
    /// is gated by a structural call-site-count <= 1 over-fire boundary (a
    /// collection consumed at >1 call site is live-across, declined to avoid
    /// freeing it before a later borrowed use). Consumed in
    /// `ori_arc::lower::burden_lower`. Bisects a borrowed-collection-into-catch
    /// leak to that arm vs the rest of the Phase-5 walk.
    /// Usage: `ORI_DISABLE_FRESH_COLLECTION_DEAD_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_FRESH_COLLECTION_DEAD_PARAM_RELEASE

    /// Decline the ALT-CONSUMED mode of the construct-fed dead-param scan:
    /// a lineage with a NON-forwarder owned-transfer consumer reverts to the
    /// unconditional gate-(d) decline even when every consume is a FUNDED
    /// duplication site.
    ///
    /// Default (unset): a fresh sum-aggregate / collection `Construct` lineage
    /// whose every owned consume is funded (each consume carries its own kept
    /// inc matched by the consumer's release) gets ONE RL-5 dead-at-entry
    /// release at the dead merge block-param — the Jump-transferred birth
    /// reference's sole release (releases-only; the funded machinery is
    /// untouched). Consumed in `ori_arc::lower::burden_lower`. Bisects a
    /// dead-merge-param leak / double-free to the alt-consumed release vs the
    /// rest of the Phase-5 walk.
    /// Usage: `ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE

    /// Disable the `lower_match` merge-param divergence pruning: every
    /// in-scope mutable binding is threaded into the match merge block-params
    /// unconditionally (the pre-cure arrangement), manufacturing DEAD merge
    /// params for unchanged bindings.
    ///
    /// Default (unset): `lower_match` pre-traverses the arm bodies + decision
    /// -tree guards and threads ONLY bindings an `Assign` could rebind — the
    /// same divergence semantics `merge_mutable_vars` applies to `if/else`.
    /// Consumed in `ori_arc::lower::control_flow`. Bisects a dead-merge-param
    /// leak / wrong-post-merge-value to the pruning vs the RL-5 dead-param
    /// release machinery.
    /// Usage: `ORI_DISABLE_MATCH_PARAM_PRUNING=1 ori build file.ori`
    ORI_DISABLE_MATCH_PARAM_PRUNING

    /// Disable the Phase-5 RL-2 scope-exit release for a transfer-through-return
    /// forwarder result whose monomorphized result-type burden is empty.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a forwarder-result
    /// leak to that release vs the rest of the Phase-5 walk.
    /// Usage: `ORI_DISABLE_FORWARDER_RESULT_RELEASE=1 ori build file.ori`
    ORI_DISABLE_FORWARDER_RESULT_RELEASE

    /// Disable the Phase-5 RL-4 per-edge release for an Owned non-scalar
    /// function param that dies crossing a CFG edge without transfer.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a
    /// dead-owned-param-branch leak to that emission.
    /// Usage: `ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE=1 ori build file.ori`
    ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE

    /// Disable the Phase-5 RL-4 per-edge release for a FRESH local `Construct`
    /// lineage consumed at an owned position on a strict subset of branch
    /// paths (the branch-exclusive terminal-move shape — the non-consuming
    /// sibling path otherwise leaks the pre-branch funding inc).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a branch-exclusive
    /// terminal-move leak to that emission vs the rest of the Phase-5 walk.
    /// Usage: `ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE=1 ori build file.ori`
    ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE

    /// Restore the RL-1 duplication classification for `Let { Var(src) }`
    /// aliases of a forwarder-identity source (default: alias is transparent).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a forwarder-lineage
    /// double-free / leak to the alias de-classification.
    /// Usage: `ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP=1 ori build file.ori`
    ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP

    /// Restore the decoupled treatment of a genuine RL-1 duplication pair
    /// (default: the pair is atomic — Phase 5 keeps the load-bearing inc;
    /// Phase 6 elides an alias pair only whole).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5) and the Phase-6
    /// per-var elision. Bisects a duplication-lineage double-free / leak.
    /// Usage: `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1 ori build file.ori`
    ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING

    /// Restore the symmetric-cancellation treatment for a `Let { Var }`
    /// dup-alias consumed at an OWNED call-arg position while its source stays
    /// live (default: the alias keeps its RL-1 duplication inc — each
    /// owned-call-arg fork of a shared collection lineage is funded by its own
    /// surviving inc whose matched release is the consumer's; without it the
    /// lineage gets ONE net inc regardless of fork count, the COW uniqueness
    /// check sees rc 1 early and mutates an aliased buffer in place).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5 admission +
    /// cancellation escape), the Phase-6 pair-atomic elision, and the Phase-7
    /// lineage-net machinery. Bisects a multi-fork COW-lineage double-free /
    /// silent wrong value to the kept duplication inc.
    /// Usage: `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1 ori build file.ori`
    ORI_DISABLE_OWNED_CALL_ARG_DUP_INC

    /// Revert the RL-1 store-family duplication funding on FRESH-local
    /// lineages (default: the funded store-site dup incs of a fresh local
    /// used past an aggregate store are pair-atomic in the Phase-6 elision —
    /// each container holds a funded reference whose matched release is its
    /// drop — and the Phase-7 lineage net prices store / forward-Jump
    /// hand-offs so the surplus fresh-site keep-alive elides, designating the
    /// execution-final read alias as the lineage's single release carrier;
    /// without it the multi-store lineage under-incs and double-frees, or
    /// over-incs and leaks).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (the funded SSOT), the
    /// Phase-6 pair-atomic elision, and the Phase-7 lineage-net machinery.
    /// Bisects a fresh-local multi-store double-free / store-lineage leak to
    /// the funded store treatment.
    /// Usage: `ORI_DISABLE_STORE_FAMILY_FUNDING=1 ori build file.ori`
    ORI_DISABLE_STORE_FAMILY_FUNDING

    /// Skip the FINAL-READ release designation for multi-read elements of a
    /// caller-owned call-result aggregate (default: the lineage's
    /// execution-final read alias carries the element's single last-use
    /// release — a returned tuple's element read more than once otherwise
    /// keeps only net-0 keep-alive pairs and leaks one element per multi-read
    /// lineage).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// returned-tuple element leak / double-free to the designated release.
    /// Usage: `ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE=1 ori build file.ori`
    ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE

    /// Restore the RL-1 inc-suppression for a `Let { Var }` alias of an Owned
    /// transfer-through-return param that is iter-consumed (default: the
    /// iter-consumed alias keeps its duplication inc — the iterator frees the
    /// duplicate via `ori_iter_drop` while the param's original transfers out
    /// through the Return; without it the single allocation is freed once by the
    /// iterator AND returned = double-free).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// for-yield-then-return double-free to this inc.
    /// Usage: `ORI_DISABLE_TTR_ITER_CONSUME_DUP_INC=1 ori build file.ori`
    ORI_DISABLE_TTR_ITER_CONSUME_DUP_INC

    /// Restore the surplus duplication inc on a `Let { Var }` dup-alias of a
    /// fresh niche-family sum-aggregate whose extracted payload is iter-consumed
    /// (default: the by-value aggregate's Let-Var dup-alias adds no owner, so its
    /// inc is RL-1 move-once-elidable and suppressed; the payload transfers out
    /// via `@iter [own]` whose `ori_iter_drop` releases it, so the surplus inc
    /// would leak the buffer + its heap elements — the
    /// `str_list_iteration_in_match` shape over `Option<[str]>`).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// sum-payload-iter-consume leak to this suppression.
    /// Usage: `ORI_DISABLE_SUM_PAYLOAD_ITER_CONSUME_DUP_INC=1 ori build file.ori`
    ORI_DISABLE_SUM_PAYLOAD_ITER_CONSUME_DUP_INC

    /// Decline admitting a fresh-self-alloc user-callee `Apply`/`Invoke` result
    /// as an iter-consume dead-thread orphan-inc root (default: a `clone_list` /
    /// fresh-collection result whose `ReturnContract.returns_fresh_self_alloc`
    /// certifies rc=1 is admitted as a dead-thread root, so its keep-alive inc
    /// balances the downstream `@iter [own]` consume — the two-iter-consumed
    /// `str_list` `two_calls` shape, surface (a)). `=1` restores the pre-cure leak.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// fresh-Invoke-result iter-consume leak to this admission.
    /// Usage: `ORI_DISABLE_ITER_CONSUME_FRESH_INVOKE_RESULT_ROOT=1 ori build file.ori`
    ORI_DISABLE_ITER_CONSUME_FRESH_INVOKE_RESULT_ROOT

    /// Decline the loop-threaded fresh-lineage return certification (default:
    /// a return value threaded through loop block-params whose every feeder
    /// resolves — via `Let` aliases, receiver-rooted COW mutator results, and
    /// block-param feeders — to this function's own fresh collection allocation
    /// certifies `ReturnContract.returns_fresh_self_alloc`). `=1` restores the
    /// conservative block-param bail (the pre-cure cross-call keep-alive leak
    /// on an iter-consumed returned rebuild).
    ///
    /// Consumed in `ori_arc::aims::interprocedural::extract`. Bisects a
    /// returned-rebuild contract change to the lineage trace.
    /// Usage: `ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE=1 ori build file.ori`
    ORI_DISABLE_FRESH_LINEAGE_RETURN_TRACE

    /// Restore the header-less push-grown list buffer (default: a `push` result
    /// whose receiver lineage is RETURNED from the function gets `elem_dec_fn` +
    /// `elem_count` stored in its buffer's RC header, so the caller-side free
    /// releases the funded element refs — the balancing release of the
    /// element-escape keep-alive). `=1` skips the store; the returned buffer
    /// frees without element cleanup (the pre-cure element leak).
    ///
    /// Consumed in `ori_llvm::codegen::arc_emitter` (list push emission).
    /// Bisects a pushed-element leak / double-free to the header store.
    /// Usage: `ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE=1 ori build file.ori`
    ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE

    /// Trace one `ArcVarId`'s `owned_vars_needing_rc` membership across every
    /// exclusion site in the Phase-5 suppression-filter prologue (one trace
    /// event per site, target `ori_arc::aims::realize`). Value = the raw var
    /// index to trace; pairs with `ORI_LOG=ori_arc::aims::realize=trace`.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects which
    /// exclusion site removed a var from the owned set.
    /// Usage: `ORI_TRACE_BURDEN_VAR=2 ORI_LOG=ori_arc::aims::realize=trace ori build file.ori`
    ORI_TRACE_BURDEN_VAR

    /// Restore the pre-K3 conservative class-ledger `TrmcContext` decline
    /// (default: a TRMC `ContextHole`-shaped function is ELIGIBLE for
    /// class-ledger replacement — the fill-at-recursive-call's `Set`
    /// classifies as mutate(context) + consume(filled value), the proven
    /// hole-fill derivation whose release-after-fill is rejected). `=1`
    /// restores the blanket fallback for bisection.
    ///
    /// Consumed in `ori_arc::aims::class_ledger` (replacement gating).
    /// Bisects a TRMC-function class-ledger regression to the admission.
    /// Usage: `ORI_DISABLE_TRMC_CONTEXT_LEDGER=1 ori build file.ori`
    ORI_DISABLE_TRMC_CONTEXT_LEDGER

    /// Restore the pre-terminator placement of a borrowed-`Invoke`-arg
    /// terminal-read release on the burden-sole path (default: the release
    /// RELOCATES to the single-pred normal successor's entry — a dec emitted
    /// before the terminator frees the borrowed arg BEFORE the call reads it,
    /// a use-after-free the read floor rejects). `=1` restores the
    /// pre-relocation placement for bisection.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// borrowed-Invoke-arg UAF / release-placement change to the relocation.
    /// Usage: `ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION=1 ori build file.ori`
    ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION

    /// Restore the single-block over-approximation in the dup'd terminal-move
    /// gate (default: a dup'd cross-block move source whose `Let { Var }` alias
    /// is its proven global final use — successor-reachability proof, loop
    /// back-edge re-use declines — joins the transfer fixpoint, cancelling the
    /// pending last-use release on a proven owned-position hand-off; without
    /// the cancellation the post-Construct release double-frees the dup-inc'd
    /// pair-return lineage).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// pair-return double-free / leak to the relaxed gate.
    /// Usage: `ORI_DISABLE_CROSS_BLOCK_FINAL_USE_CANCEL=1 ori build file.ori`
    ORI_DISABLE_CROSS_BLOCK_FINAL_USE_CANCEL

    /// Suppress the Phase-5 RL-1 duplication inc for a borrowed-param-rooted
    /// value consumed at an aggregate-store position (default: the inc is
    /// emitted — the store duplicates the caller's retained reference; the
    /// container's drop is the matched release).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// borrowed-store lineage use-after-free / leak to this inc.
    /// Usage: `ORI_DISABLE_BORROWED_STORE_DUP_INC=1 ori build file.ori`
    ORI_DISABLE_BORROWED_STORE_DUP_INC

    /// Restore per-alias moved-field attribution for a loop-carried struct
    /// self-rebuild (default: the sibling-alias moved-field cross-check unifies
    /// the moved-out-field sets across the `Let { Var }` projection aliases of
    /// one alias-chain root, widening each alias's `BurdenDecPartial.skip_fields`
    /// so a field transferred by a SIBLING is not double-freed). With the toggle
    /// set, each alias keeps the partial-dec for its sibling's transferred field
    /// — the pre-fix double-free shape.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// loop-carried struct self-rebuild double-free to the sibling-union
    /// post-process vs the rest of the Phase-5 walk.
    /// Usage: `ORI_DISABLE_SIBLING_MOVED_FIELD_UNION=1 ori build file.ori`
    ORI_DISABLE_SIBLING_MOVED_FIELD_UNION

    /// Restore the decoupled DP-2/DP-3 split for alias dsts rooted at a local
    /// fresh `Construct` (default: such alias pairs are atomic in the Phase-6
    /// per-var elision — elided whole or kept whole, never split).
    ///
    /// Consumed in the Phase-6 per-var elision
    /// (`ori_arc::aims::realize::burden_elim`). Bisects a local-rooted
    /// alias-lineage double-free / leak to the pair coupling.
    /// Usage: `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1 ori build file.ori`
    ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING

    /// Restore the decoupled DP-2/DP-3 split for `Let { Var }` alias dsts
    /// rooted at an `Apply` / `Invoke` RESULT whose every use is a `Project`
    /// read (default: such Project-only view alias pairs are atomic in the
    /// Phase-6 per-var elision — elided whole or kept whole, never split).
    ///
    /// Consumed in the Phase-6 per-var elision
    /// (`ori_arc::aims::realize::burden_elim`). Bisects a call-result
    /// borrow-alias double-free to the pair coupling.
    /// Usage: `ORI_DISABLE_RESULT_ROOT_PROJECT_VIEW_PAIR_COUPLING=1 ori build file.ori`
    ORI_DISABLE_RESULT_ROOT_PROJECT_VIEW_PAIR_COUPLING

    /// Decline admitting a user-callee `Apply` / `Invoke` result certified
    /// fresh by contract (`ReturnContract.returns_fresh_self_alloc` at
    /// `Unique`) as a fresh-alloc root of the Phase-7 alloc-aware fresh-inc
    /// elision (default: such a result joins the M1 net so its surplus
    /// fresh-site keep-alive inc elides when every path nets +1).
    ///
    /// Consumed in the Phase-7 fresh-inc elision
    /// (`ori_arc::aims::realize::emit_unified`). Bisects a call-result
    /// fresh-inc leak to this admission.
    /// Usage: `ORI_DISABLE_CERTIFIED_FRESH_USER_RESULT_INC_ELISION=1 ori build file.ori`
    ORI_DISABLE_CERTIFIED_FRESH_USER_RESULT_INC_ELISION

    /// Restore the RL-1 inc on a fresh self-alloc `Invoke`-terminator result
    /// used exactly once at a borrowed (non-owned) arg position of a body
    /// `Apply` / `ApplyIndirect` (default: the FRESH + ONCE + AFFINE-borrowed
    /// shape is inc-elidable, so the duplication inc is suppressed; an
    /// owned-position transfer, `Return` escape, non-call read, or terminator
    /// consume keeps the inc).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// borrowed-arg fresh-result leak / use-after-free to this elision.
    /// Spec: Annex E §AIMS RL-1 + DP-3.
    /// Usage: `ORI_DISABLE_FRESH_CALL_RESULT_BORROWED_ARG_INC_ELISION=1 ori build file.ori`
    ORI_DISABLE_FRESH_CALL_RESULT_BORROWED_ARG_INC_ELISION

    /// Restore the RL-4 edge-deadness release for a fresh `Construct` owned
    /// non-scalar local that dies on a branch merge edge (default: the untaken
    /// parent aggregate of an `if c then p1.first else p2.first` shape is
    /// released on its own merge edge; reuses the single-pred + edge-deadness +
    /// Jump-transfer-exemption gates of the param scan).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects an
    /// untaken-merge-parent leak to this release.
    /// Spec: Annex E §AIMS RL-4 + RL-2.
    /// Usage: `ORI_DISABLE_FRESH_CONSTRUCT_DEAD_BRANCH_RELEASE=1 ori build file.ori`
    ORI_DISABLE_FRESH_CONSTRUCT_DEAD_BRANCH_RELEASE

    /// Decline admitting a direct `@ori_list_take` builtin result (the
    /// for-yield-result finalizer — a fresh rc=1 buffer moved out of the loop
    /// accumulator, lattice-identical to a `Construct`) as an iter-consume
    /// dead-thread orphan-inc root (default: admitted as a root so its keep-alive
    /// inc balances the downstream `@iter [own]` consume; `=1` restores the
    /// pre-cure leak on the loop-carried for-yield iter-consumed shape).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// loop-carried for-yield-result iter-consume leak to this admission.
    /// Spec: Annex E §AIMS RL-1.
    /// Usage: `ORI_DISABLE_ITER_CONSUME_FOR_YIELD_TAKE_ROOT=1 ori build file.ori`
    ORI_DISABLE_ITER_CONSUME_FOR_YIELD_TAKE_ROOT
}
