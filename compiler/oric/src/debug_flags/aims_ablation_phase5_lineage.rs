//! Current compiled-counter adapter ablations at the AIMS carrier boundary.
//!
//! Each `ORI_DISABLE_*` toggle disables the named behavior at its documented
//! consumer for bisection. See the crate-level `debug_flags` module doc for the
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

    /// Omit the RL-31 param `noalias` attribute emission on function
    /// declarations.
    ///
    /// Consumed in `ori_llvm::codegen::function_compiler` (which cannot depend
    /// on `oric`; raw `var_os`). Defined here for documentation and
    /// `check-debug-flags.sh` consistency. Bisects a miscompile to LLVM's
    /// `noalias` projection of the backend-neutral AIMS fact vs the rest of
    /// codegen.
    /// Usage: `ORI_DISABLE_RL31_NOALIAS=1 ori build file.ori`
    ORI_DISABLE_RL31_NOALIAS

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
}
