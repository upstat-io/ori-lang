//! Current compiled-counter adapter ablations at the AIMS carrier boundary.
//!
//! Each `ORI_DISABLE_*`/`ORI_FORCE_*` toggle changes the named behavior at its
//! documented consumer for bisection. See the crate-level `debug_flags` module
//! doc for the `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
    /// Disable the burden-op emission pass (Step 4b of the AIMS pipeline).
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Burden
    /// emission is the sole ownership-event input to the current compiled-counter
    /// adapter (no fallback adapter input exists), so setting this aborts
    /// compilation via the fail-loud migration gate for any function needing
    /// physical RC management — a negative-pin probe, never a build mode.
    /// Usage: `ORI_DISABLE_BURDEN_OPS=1 ori build file.ori`
    ORI_DISABLE_BURDEN_OPS

    /// Dump each function's ARC IR to stderr immediately after Step 4b
    /// `class_ledger_emission`, before mechanical Phase-7 lowering.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Surfaces
    /// the faithful Phase-5 `BurdenInc` / `BurdenDec*` emission for VF-1 residual
    /// localization (post-realize `ORI_DUMP_AFTER_ARC` cannot show pre-realize
    /// burden placement).
    /// Usage: `ORI_DUMP_AFTER_BURDEN=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN

    /// Dump each function's live post-Step-4b ARC IR through the retained
    /// compatibility flag.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. There is
    /// no separate elimination pass; this observes the same class-ledger plan as
    /// `ORI_DUMP_AFTER_BURDEN`.
    /// Usage: `ORI_DUMP_AFTER_BURDEN_ELIM=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN_ELIM

    /// Disable the predicate-stack `RcInc` / `RcDec` realization phases, leaving
    /// the burden path (elimination + mechanical `BurdenInc → RcInc` /
    /// `BurdenDec → RcDec` lowering) as the sole real-RC emitter in the current
    /// compiled-counter adapter.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` to drive the burden-path
    /// self-sufficiency probe. When set, the predicate-stack emission is suppressed
    /// and surviving whole-var burden ops lower to real RC; default (unset) leaves
    /// the default emission path byte-identical.
    /// Usage: `ORI_DISABLE_PREDICATE_STACK_RC=1 ori build file.ori`
    ORI_DISABLE_PREDICATE_STACK_RC

    /// Output path for the compiled-counter RC-survivor remark stream (JSONL).
    ///
    /// Consumed in `ori_arc::aims::realize::rc_remark` (which can't depend on
    /// `oric`). Defined here for documentation and `check-debug-flags.sh`
    /// consistency. CLI alternative: `--emit-rc-remarks <path>` (composes the
    /// burden-sole-path gating so the stream is a valid compiled-counter verdict
    /// surface). Valid only for that adapter on the burden-sole path
    /// (`ORI_DISABLE_PREDICATE_STACK_RC=1`); it is not an AIMS-wide verdict.
    /// Usage: `ORI_RC_REMARKS=out.jsonl ORI_DISABLE_PREDICATE_STACK_RC=1 ori build file.ori`
    ORI_RC_REMARKS

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

}
