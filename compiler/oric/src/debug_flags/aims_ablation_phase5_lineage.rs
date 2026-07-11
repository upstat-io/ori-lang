//! AIMS Phase-5 (burden-lowering) ablation toggles — lineage/borrow-view/closure family.
//!
//! Each `ORI_DISABLE_*` toggle reverts one named lineage-tracking or
//! borrowed-invoke/closure-borrow-view treatment in
//! `ori_arc::lower::burden_lower` for bisection; default (unset) leaves the
//! cured behavior active. See the crate-level `debug_flags` module doc for the
//! `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
    /// Disable the `ori_panic` message ownership-transfer contract seed
    /// (default: the panic machinery owns + releases its message; a
    /// still-live message dup-incs per RL-1).
    ///
    /// Consumed in `ori_arc::aims::builtins` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// panic-message leak / double-free to the transfer seed.
    /// Usage: `ORI_DISABLE_PANIC_MSG_TRANSFER=1 ori build file.ori`
    ORI_DISABLE_PANIC_MSG_TRANSFER

    /// Decline the Phase-5 borrowed-`Invoke` lineage treatment: the inline
    /// borrowed-`Invoke`-terminator dec suppression + the single placed
    /// death-point release for a FRESH collection-`Construct` buffer (or a
    /// may-unwind borrowed-receiver user-call heap result) borrowed into a
    /// may-unwind `Invoke` and reaching a DEAD merge block-param.
    ///
    /// Default (unset): the same-alloc closure is removed from
    /// `owned_vars_needing_rc` (suppressing the dup-alias incs + the inline
    /// terminator dec) and EXACTLY ONE whole-var release is placed at the
    /// lineage's dead-block-param death point; the dying unwind / unreachable
    /// edges are released by the Surface-1 Category-2 `deadAtSucc` conjunct.
    /// With the toggle set, the lineage reverts to the pre-fix base walk
    /// (inline dec before the terminator → use-after-free / double-free on the
    /// still-live receiver). The Cat-2 `deadAtSucc` conjunct itself is NOT
    /// gated (a pure narrowing of over-release, unconditional).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// borrowed-`Invoke` lineage leak / double-free to this treatment vs the
    /// rest of the Phase-5 walk. Spec: Annex E §AIMS RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1 ori build file.ori`
    ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE

    /// Restore the inline last-use dec for a SOLE-CARRIER borrowed-`Invoke`
    /// alias (`compute_sole_carrier_borrowed_invoke_aliases` returns empty):
    /// the lineage's single release lands BEFORE the borrowed terminator that
    /// reads it (use-after-free / double-free when the callee aliases the value
    /// into its result).
    ///
    /// Default (unset): a `Let { Var(src) }` dst that carries its lineage's
    /// only release while its sole use is a borrowed arg of a may-unwind
    /// `Invoke` terminator is removed from `owned_vars_needing_rc` (suppressing
    /// the early inline dec) and CLAIMED for the Category-2 `deadAtSucc`
    /// per-edge release, so each executing path releases exactly once AFTER the
    /// call completes.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// borrowed-call-arg early-release to the edge-claim vs the rest of the
    /// Phase-5 walk. Spec: Annex E §AIMS RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM=1 ori build file.ori`
    ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM

    /// Disable the as-compiled impl-method contract pre-pass + per-caller
    /// Phase-5 binding; impl-method call sites revert to the conservative
    /// no-contract treatment.
    ///
    /// Consumed in `oric::commands::codegen_pipeline`. Bisects an
    /// impl-method-caller RC change to contract visibility.
    /// Usage: `ORI_DISABLE_IMPL_METHOD_CONTRACTS=1 ori build file.ori`
    ORI_DISABLE_IMPL_METHOD_CONTRACTS

    /// Keep each elided fresh-site `BurdenInc` as a codegen-no-op marker
    /// instead of removing it during Phase-7 lowering.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified`. Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// behavior change to the marker removal vs the elision verdict.
    /// Usage: `ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL=1 ori build file.ori`
    ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL

    /// Keep `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` in
    /// burden spelling through Phase-7 instead of re-spelling them to the
    /// realized `RcDecPartial` / `RcDecField` / `RcDecVariant` instructions.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified`. Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// gated-verification change to the field-grain re-spelling.
    /// Usage: `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1 ori build file.ori`
    ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING

    /// Disable the Phase-5 RL-1 + RL-2 same-alloc lineage treatment for a
    /// fresh niche-family sum allocation whose payload is extracted live
    /// through a match.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// match-extract leak / double-free to this treatment vs the rest of
    /// the Phase-5 walk.
    /// Usage: `ORI_DISABLE_FRESH_SUM_LIVE_EXTRACT_RELEASE=1 ori build file.ori`
    ORI_DISABLE_FRESH_SUM_LIVE_EXTRACT_RELEASE

    /// Restore the legacy type-level-only Phase-5 burden admission (do not
    /// exclude provably-`Scalar`-repr vars from burden admission).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// gated-verification change to the repr-aware admission vs the rest of
    /// the Phase-5 walk.
    /// Usage: `ORI_DISABLE_SCALAR_REPR_BURDEN_SKIP=1 ori build file.ori`
    ORI_DISABLE_SCALAR_REPR_BURDEN_SKIP

    /// Omit the RL-31 param `noalias` attribute emission on function
    /// declarations.
    ///
    /// Consumed in `ori_llvm::codegen::function_compiler` (which cannot depend
    /// on `oric`; raw `var_os`). Defined here for documentation and
    /// `check-debug-flags.sh` consistency. Bisects a miscompile to the
    /// AIMS-exported `noalias` attribute vs the rest of codegen.
    /// Usage: `ORI_DISABLE_RL31_NOALIAS=1 ori build file.ori`
    ORI_DISABLE_RL31_NOALIAS

    /// Restore the decoupled DP-2/DP-3 split for `Let { Var }` alias dsts whose
    /// alias-chain root is a loop-carried block-param (back-edge-carrying) and
    /// whose every use is a `Project` read feeding only borrowed positions.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// loop-carried borrow-alias double-free to the pair coupling vs the rest of
    /// the Phase-6 elimination. Spec: Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING=1 ori build file.ori`
    ORI_DISABLE_LOOP_CARRIED_PAIR_COUPLING

    /// Restore per-field attribution WITHOUT the all-arms match-handoff
    /// extract-transfer verdict: a rebuild carrier's `BurdenDecPartial` keeps
    /// releasing a sum field whose payload was extracted through the match
    /// block-param handoff and re-wrapped into the rebuild construct.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects the
    /// sum-payload match-rebuild double-free to this verdict vs the rest of the
    /// Phase-5 walk. Spec: Annex E §AIMS RL-2.
    /// Usage: `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER=1 ori build file.ori`
    ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER

    /// Decline the Phase-5 RL-5 dead-at-entry release for a sibling-union-fired
    /// loop-carried rebuild lineage reaching a DEAD loop-exit block-param (the
    /// loop-carried struct unused after the loop: the union suppressed the
    /// in-loop releases, so the dead param is the lineage's sole terminal owner).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a loop-exit
    /// leak to this release vs the rest of the Phase-5 walk. Spec: Annex E §AIMS RL-5.
    /// Usage: `ORI_DISABLE_REBUILD_LINEAGE_DEAD_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_REBUILD_LINEAGE_DEAD_PARAM_RELEASE

    /// Decline the LOOP-EXIT death-point mode for a loop-invariant borrowed
    /// collection lineage: the in-loop borrowed-`Invoke` carrier's inline dec
    /// returns, releasing the loop-invariant buffer once PER ITERATION
    /// (use-after-free on iteration 2+).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// loop-invariant borrowed-collection use-after-free to the loop-exit
    /// release vs the rest of the death-point selection. Spec: Annex E §AIMS
    /// RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_LOOP_BORROWED_LINEAGE_EXIT_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LOOP_BORROWED_LINEAGE_EXIT_RELEASE

    /// Decline the Phase-5 RL-5 dead-at-entry treatment for a FRESH
    /// `PartialApply` closure threaded through a loop and dead at the post-loop
    /// block-param. Default (unset): the whole same-alloc closure lineage is
    /// removed from `owned_vars_needing_rc` and EXACTLY ONE whole-var
    /// `BurdenDec` is placed at the dead post-loop block-param entry; with the
    /// toggle set the base walk's emission returns (borrowed-arg HOF shape leaks
    /// the closure env; direct-call FM shape double-frees the loop-invariant
    /// closure, exit -134).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// loop-carried-closure leak / double-free to this treatment vs the rest of
    /// the Phase-5 walk. Spec: Annex E §AIMS RL-5 + RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_LOOP_CLOSURE_DEAD_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LOOP_CLOSURE_DEAD_PARAM_RELEASE

    /// Skip the Phase-5 RL-5 dead-at-entry treatment for a fresh `ori_list_take`
    /// (for-yield collect) collection threaded through a loop and dead at the
    /// post-loop block-param — the collection analog of the loop-closure scan
    /// (`compute_loop_carried_dead_collection_param_lineage`). Default (unset):
    /// the whole same-alloc lineage (the `ori_list_take` root + `Let`-Var aliases
    /// + COW-mutator-result edge + `Jump`-arg-threaded loop block-params) is
    /// removed from `owned_vars_needing_rc` and EXACTLY ONE `BurdenDec` is placed
    /// at the dead post-loop block-param entry; with the toggle set the dead
    /// loop-carried collect leaks its allocation (`let a = for .. yield ..; for ..
    /// yield ..` shape, exit 2).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Defined here for documentation
    /// and `check-debug-flags.sh` consistency. Bisects a loop-carried-dead-
    /// collection leak to this treatment vs the rest of the Phase-5 walk. Spec:
    /// Annex E §AIMS RL-5 + RL-2.
    /// Usage: `ORI_DISABLE_LOOP_CARRIED_DEAD_COLLECTION_PARAM_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LOOP_CARRIED_DEAD_COLLECTION_PARAM_RELEASE

    /// Skip the Phase-5 RL-2 treatment for a FRESH `PartialApply` closure
    /// borrowed into a lazy-iterator builtin (`@map` / `@filter`) whose result
    /// iterator retains the closure env as a borrowed raw pointer across the
    /// chain's terminal consumer. Default (unset): the whole same-alloc closure
    /// lineage is removed from `owned_vars_needing_rc` (suppressing the early
    /// borrowed-arg `BurdenDec` the base walk places at the `@map`/`@filter`
    /// call) and EXACTLY ONE whole-var `BurdenDec` is placed at the lazy-iterator
    /// chain's terminal consumer's (`@collect`/…) normal-successor entry. With the
    /// toggle set the base walk's early dec returns — the env (and its
    /// cascade-freed captured heap payload) is freed while the lazy iterator still
    /// holds the borrowed env pointer, a use-after-free when the closure runs at
    /// `@collect` time (exit -139). `@fold` is EAGER (closure runs synchronously)
    /// and is excluded.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a lazy-HOF
    /// closure-borrow use-after-free to this treatment vs the rest of the Phase-5
    /// walk. Spec: Annex E §AIMS RL-2 + TF-4 + TF-7.
    /// Usage: `ORI_DISABLE_LAZY_ITER_CLOSURE_BORROW_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LAZY_ITER_CLOSURE_BORROW_RELEASE

    /// Skip the Phase-5 RL-1 + RL-2 orphaned-inc elision for a fresh collection
    /// iter-consumed by an inline `for`-loop whose loop-carried thread is dead
    /// post-loop. Default (unset): a fresh Construct collection whose SOLE genuine
    /// consume is the for-loop's `@iter [own]` (Invoke @iter terminator / Apply
    /// @iter; `ori_iter_drop` frees it) AND whose Jump-arg thread across the loop
    /// back-edge terminates in a DEAD param (never read) is removed from
    /// `owned_vars_needing_rc` — eliding the orphaned FRESH-site keep-alive
    /// `BurdenInc` (the dead-thread is a `JumpArg` transfer into a dead param, NOT a
    /// genuine duplication, so the value is move-once into @iter). With the toggle
    /// set the orphan inc returns — the buffer rc never reaches 0 (the @iter
    /// consume suppresses the scope-exit dec), a +1 leak (exit 2). Follows
    /// Jump-args across ALL edges (forward + back) — the back-edge inclusion the
    /// foreclosed forward-only scans lacked; does NOT relax the @iter COW-taint.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a `for_yield`
    /// iter-source leak to this treatment vs the rest of the Phase-5 walk. Spec:
    /// Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_ITER_CONSUME_DEAD_THREAD_ORPHAN_INC=1 ori build file.ori`
    ORI_DISABLE_ITER_CONSUME_DEAD_THREAD_ORPHAN_INC

    /// Skip the Phase-5 RL-2 in-callee container-passthrough suppression for a
    /// fresh nested struct/tuple `Construct` chain whose deepest projection is the
    /// function's Return value, wrapping a transfers-through-return param. Default
    /// (unset): a param moved into a chain of fresh struct/tuple Constructs and
    /// projected back out (`Project (Construct args) field == args[field]`) as the
    /// Return value has the whole wrapper-chain lineage removed from
    /// `owned_vars_needing_rc` — eliding the surplus container drops that would
    /// free the transferred-out param (the wrapper owns nothing surviving). With
    /// the toggle set the container drops return — the outermost wrapper's
    /// transitive drop frees the returned nested field, a use-after-free
    /// (exit -139). Pairs with the caller-side construct-project round-trip
    /// `Direct` return-alias.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a nested-
    /// construct-return use-after-free to this treatment vs the rest of the
    /// Phase-5 walk. Spec: Annex E §AIMS RL-2 + TF-3 + TF-4.
    /// Usage: `ORI_DISABLE_NESTED_CONSTRUCT_RETURN_PASSTHROUGH=1 ori build file.ori`
    ORI_DISABLE_NESTED_CONSTRUCT_RETURN_PASSTHROUGH

    /// Decline the Phase-5 treatment for a FRESH closure result (`Unique` +
    /// `preserves_freshness`, non-forwarder) consumed ONLY at `ApplyIndirect`
    /// borrow-receiver sites across a short-circuit CFG. Default (unset): the
    /// whole same-alloc closure is removed from `owned_vars_needing_rc`
    /// (suppressing every dup/keep-alive inc + the spurious pre-use source dec)
    /// and EXACTLY ONE whole-var release is placed per terminal path (RL-2 dec
    /// after the execution-final borrow-read on each reading path; RL-4 edge dec
    /// on each short-circuit-bypass successor entry).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// short-circuit closure-borrow double-free to this treatment vs the per-var
    /// DP-2/DP-3 split. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_MULTI_EXIT_BORROW_VIEW_RELEASE=1 ori build file.ori`
    ORI_DISABLE_MULTI_EXIT_BORROW_VIEW_RELEASE

    /// Decline the Phase-5 accessor-retain retain-aliasing per-reference
    /// treatment for the branchy `m[k].unwrap().starts_with(..)` shape: a
    /// niche-family sum (`@__index` Option) read through N accessor-retain hops
    /// (`@unwrap`/`@get`), each self-incrementing the SAME allocation, across a
    /// short-circuit CFG where a reader-bypass arm reaches a terminal without the
    /// read. Default (unset): the whole same-alloc closure is removed from
    /// `owned_vars_needing_rc` and ONE release is placed per RETAINED REFERENCE
    /// per terminal path (each accessor-retain hop's self-inc'd reference
    /// released once at its own last use). A straight-line accessor-retain
    /// declines (no bypass; the base walk is already correct).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a branchy
    /// retain-aliasing double-free to this treatment vs the single-site Phase-5
    /// fresh-sum live-extract scan. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4 + TF-4.
    /// Usage: `ORI_DISABLE_RETAIN_ALIASING_RELEASE=1 ori build file.ori`
    ORI_DISABLE_RETAIN_ALIASING_RELEASE

    /// Decline the Phase-5 closure-extract borrow-view suppression for N
    /// `ApplyIndirect` results that are PROVEN same-allocation borrow-views of
    /// ONE captured field of a closure env (the resolved lambda's
    /// `return_alias = Project { field }` capture-param contract). The closure
    /// captures a sum payload (`Result<int, str>::Err(str)`) and the lambda
    /// returns it via a match-Switch-extract-to-block-param Return; each result
    /// is a TF-4 borrow-view of the captured str (canonical owner = the closure
    /// env). Default (unset): the whole result lineage is removed from
    /// `owned_vars_needing_rc` (killing the N surplus per-result decs); NO
    /// release is placed — the closure env's OWN scope-exit dec cascade-frees
    /// the captured payload exactly once (RL-2 release-exactly-once, the
    /// joint-release theorem with the env as canonical owner).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// closure-extract double-free to this treatment vs the rest of the Phase-5
    /// walk. Spec: Annex E §AIMS RL-2 + TF-4.
    /// Usage: `ORI_DISABLE_CLOSURE_EXTRACT_BORROW_VIEW_RELEASE=1 ori build file.ori`
    ORI_DISABLE_CLOSURE_EXTRACT_BORROW_VIEW_RELEASE

    /// Restore the surplus same-allocation dec on a use-once owned source whose
    /// sole `Let { Var }` alias is a same-allocation borrow-view (proven by the
    /// burden-path `genuine_same_alloc_reps` union-find) live downstream — the
    /// forwarder-result `%6 = %4` keystone.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects the
    /// forwarder-result double-free to the surplus-dec suppression arm vs the
    /// rest of the Phase-5 walk. Spec: Annex E §AIMS RL-2.
    /// Usage: `ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS

    /// Restore the surplus per-alias decs on a multi-borrow-view-alias owner —
    /// the N-borrow-view-alias generalization of the single-alias
    /// `ORI_DISABLE_BORROW_VIEW_DST_SURPLUS_DEC_SUPPRESS` keystone (a
    /// `Config { settings, name }` whose `.settings` and `.name` are projected
    /// from distinct whole-var aliases of the same aggregate).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. With the toggle
    /// set, the surplus per-alias decs return (over-release toward a double-free)
    /// — bisects the multi-borrow-view-alias surplus-dec suppression arm.
    /// Spec: Annex E §AIMS RL-2.
    /// Usage: `ORI_DISABLE_MULTI_BORROW_VIEW_ALIAS_SURPLUS=1 ori build file.ori`
    ORI_DISABLE_MULTI_BORROW_VIEW_ALIAS_SURPLUS

    /// Restore the surplus same-allocation dec on a use-once owned source whose
    /// SOLE owned RC field is projected-returned by a borrowed-receiver callee
    /// (`@unwrap(b) = b.value`, an `ApplyAliasSource::Project` apply-result alias)
    /// to a LIVE caller result — the joint borrow-projection keystone. Default
    /// (unset): the source's premature field-drop is the surplus; the live
    /// projected result carries the joint lineage's single release at its true
    /// last use (RL-2 release exactly once).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects the
    /// `edge_project_return_not_param` double-free to this arm vs the rest of the
    /// Phase-5 walk. Spec: Annex E §AIMS RL-2 + TF-4.
    /// Usage: `ORI_DISABLE_PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_PROJECT_RETURN_SURPLUS_OWNER_DEC_SUPPRESS

    /// Restore the syntactic last-use placement of an owned aggregate's whole-var
    /// `BurdenDec`, ignoring the downstream liveness of its `Project` borrow-views.
    /// Default (unset): an aggregate `%agg` whose `Project` borrow-view (or a
    /// `Let{Var}` alias of one) is read AFTER `%agg`'s own syntactic last use has
    /// its whole-var release PLACEMENT extended to the borrow-view's last use — the
    /// owner's drop (which cascade-frees the projected field) must not precede a
    /// borrow-read of that field (TF-14 source-liveness joins the view's liveness;
    /// RL-2 the owner drop is the field's single release). With the toggle set, the
    /// owner drop lands at the aggregate's syntactic last use (use-after-free on a
    /// borrow-view read past the owner's drop — `let xs = c.items; xs.fold(..)`).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects the
    /// borrow-view-read-past-owner-drop UAF to this placement extension vs the
    /// rest of the Phase-5 walk. Spec: Annex E §AIMS TF-14 + RL-2.
    /// Usage: `ORI_DISABLE_OWNER_DROP_BORROW_VIEW_LIVENESS=1 ori build file.ori`
    ORI_DISABLE_OWNER_DROP_BORROW_VIEW_LIVENESS

    /// Decline the SELF-ALLOCATING-BUILTIN `Invoke`-result root family of the
    /// Phase-5 borrowed-`Invoke` lineage treatment (the CARRIER-SUCC mode): a
    /// fresh builtin result (`@concat` template-chain link, heap `@to_str` /
    /// `@debug`, `@split` / `@keys`) consumed at a later borrowed-`Invoke` arg
    /// reverts to the base walk's split inc/dec pair (the template-literal
    /// str-concat chain use-after-free / per-template leak). The
    /// collection-`Construct` + contract-result families stay active.
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a template
    /// str-chain use-after-free / leak to the third root family vs the rest of
    /// the Phase-5 walk. Spec: Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_BUILTIN_INVOKE_RESULT_LINEAGE=1 ori build file.ori`
    ORI_DISABLE_BUILTIN_INVOKE_RESULT_LINEAGE

    /// Restore the base behavior of the self-recursive `Invoke` tail-call
    /// loop-lowering rewrite: move EVERY normal-continuation RC op into the call
    /// block, INCLUDING ops on the Invoke's now-eliminated result var. Default
    /// (unset): drop every RC op whose subject is the eliminated result var — the
    /// recursive result is never materialized after the loop-back rewrite, so a
    /// post-call dec on it is forbidden (a transferred tail-call result carries no
    /// post-call dec) and would dangle as a use-before-def in the rewritten loop
    /// (the `list_reverse_helper` `match`-arm tail-recursion shape: an
    /// `RcDec(result)` moved into the back-edge block references a var the loop
    /// form no longer defines).
    ///
    /// Consumed in `ori_arc::tail_call::rewrite` (Step 8 loop lowering). Bisects a
    /// TRMC tail-call use-before-def to this result-dec drop. Spec: Annex E §AIMS
    /// RL-34.
    /// Usage: `ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP=1 ori build file.ori`
    ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP

    /// Restore the surplus FRESH-site `BurdenInc` on a fresh local consumed
    /// EXACTLY ONCE as a `Binary(Add)` concat operand. Default (unset): such an
    /// operand is move-once-linear — the runtime concat helper
    /// (`ori_str_concat` / `ori_list_concat_cow`) BORROWS it and the caller's
    /// single dec frees it (RL-2 `ApplyToBorrowedParam`), so the keep-alive inc
    /// is surplus and is suppressed (else net +1 leak: alloc rc=1 + inc rc=2 -
    /// one dec rc=1 — the `matched_some + str(v)` match-arm-result literal leak).
    /// The single-use gate excludes the re-read-after-concat shape (`let s = a +
    /// b; a.starts_with(..)`) where the inc is LOAD-BEARING — it raises rc >= 2
    /// so the helper COPIES instead of mutating `a` in place. With the toggle
    /// set, the surplus inc returns and the terminal-concat operand leaks again.
    ///
    /// Consumed in `ori_arc::lower::burden_lower`
    /// (`compute_cow_terminal_concat_inc_dsts` -> `fresh_site_burden_inc_dst`
    /// suppression). Bisects a terminal-concat-operand leak to this suppression.
    /// Spec: Annex E §AIMS RL-1.
    /// Usage: `ORI_DISABLE_COW_TERMINAL_CONCAT_INC_ELISION=1 ori build file.ori`
    ORI_DISABLE_COW_TERMINAL_CONCAT_INC_ELISION

    /// Restore the spurious RL-1 source keep-alive `BurdenInc` on a fresh owned
    /// aggregate fully consumed by `(N-1)` dup-aliases + 1 move alias into ONE
    /// collection-literal `Construct` (`let held = [a, a]`, `a` dead-after).
    /// Default (unset): the source's fresh-site inc is suppressed (the dup-alias
    /// incs already fund the duplicate slots; the last-use alias MOVES the
    /// source's ref into the Nth slot).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`
    /// (`compute_collection_literal_dead_source_suppression` in
    /// `ownership_scans/collection_literal_dead_source.rs`, raw `var`). Defined
    /// here for documentation and `check-debug-flags.sh` consistency. With the
    /// toggle set, the duplicate-element aggregate's heap field leaks (rc never
    /// reaches 0). Bisects a duplicate-element collection-literal aggregate leak
    /// to this suppression vs the rest of the Phase-5 walk.
    /// Spec: Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_COLLECTION_LITERAL_DEAD_SOURCE_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_COLLECTION_LITERAL_DEAD_SOURCE_SUPPRESS
}
