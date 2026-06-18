//! Centralized debug flags for the Ori compiler.
//!
//! All compiler debugging environment variables are defined here as the single
//! source of truth. Flags are checked at runtime via env vars in ALL builds
//! (debug and release). The overhead is a single `std::env::var()` call per
//! flag — negligible for a CLI compiler.
//!
//! # Usage
//!
//! ```bash
//! ORI_DUMP_AFTER_ARC=1 ori build program.ori
//! ORI_DEBUG_LLVM=1 ori check program.ori
//! ```
//!
//! # Pattern
//!
//! Historical influence: the `debug_flags` crate SHAPE from Roc:
//! - `dbg_set!` — returns `true` if the flag is set
//! - `dbg_do!` — executes an expression if the flag is set
//! - `flags!` — defines flag constants with doc comments
//!
//! Note: `ori_llvm` cannot depend on `oric` (the dep direction is reversed),
//! so flags consumed inside `ori_llvm` (e.g., evaluator JIT path) use raw
//! `std::env::var` checks. The `oric` call sites use `dbg_do!`/`dbg_set!`
//! macros for consistent flag checking.

/// Check if a debug flag is set. Works in both debug and release builds.
///
/// The flag is considered "set" if the env var exists and is not `"0"`.
///
/// # Examples
///
/// ```ignore
/// use crate::debug_flags;
///
/// if dbg_set!(debug_flags::ORI_DEBUG_LLVM) {
///     eprintln!("LLVM IR dump enabled");
/// }
/// ```
#[macro_export]
macro_rules! dbg_set {
    ($flag:expr) => {{
        let flag = std::env::var($flag);
        flag.is_ok() && flag.as_deref() != Ok("0")
    }};
}

/// Execute an expression only if a debug flag is set.
///
/// Works in both debug and release builds.
///
/// # Examples
///
/// ```ignore
/// use crate::debug_flags;
///
/// dbg_do!(debug_flags::ORI_DEBUG_LLVM, {
///     eprintln!("=== LLVM IR ===");
///     eprintln!("{}", module.print_to_string());
/// });
/// ```
#[macro_export]
macro_rules! dbg_do {
    ($flag:expr, $expr:expr) => {
        if $crate::dbg_set!($flag) {
            $expr
        }
    };
}

/// Define debug flag constants with doc comments.
///
/// Generates `pub const FLAG: &str = "FLAG"` for each flag, preserving
/// the doc comments for IDE support and `check-debug-flags.sh` parsing.
macro_rules! flags {
    ($($(#[doc = $doc:expr])+ $flag:ident)*) => {$(
        $(#[doc = $doc])+
        pub const $flag: &str = stringify!($flag);
    )*};
}

flags! {
    // === Phase Dumps ===

    /// Dump the parsed AST to stderr after parsing.
    ///
    /// Shows the raw AST structure before type checking.
    /// Usage: `ORI_DUMP_AFTER_PARSE=1 ori check file.ori`
    ORI_DUMP_AFTER_PARSE

    /// Dump the typed IR to stderr after type checking.
    ///
    /// Shows type annotations on every node and resolved method dispatch.
    /// Usage: `ORI_DUMP_AFTER_TYPECK=1 ori check file.ori`
    ORI_DUMP_AFTER_TYPECK

    /// Dump the ARC IR to stderr after ARC lowering.
    ///
    /// Shows RC strategy decisions, drop placement, and COW operations.
    /// Usage: `ORI_DUMP_AFTER_ARC=1 ori build file.ori`
    ORI_DUMP_AFTER_ARC

    /// Dump annotated LLVM IR to stderr after LLVM codegen.
    ///
    /// Enhanced version of `ORI_DEBUG_LLVM` with Ori-aware annotations.
    /// Usage: `ORI_DUMP_AFTER_LLVM=1 ori build file.ori`
    ORI_DUMP_AFTER_LLVM

    /// Emit `GraphViz` DOT output of ARC IR control-flow graphs to stderr.
    ///
    /// Each function becomes a digraph with basic blocks as table nodes and
    /// RC operations color-highlighted. Pipe to file and render with `dot`.
    /// Usage: `ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot`
    ORI_EMIT_ARC_DOT

    // === Repr-Opt Configuration ===
    // Note: Consumed directly in `ori_repr` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Disable all representation optimizations (integer narrowing, enum packing).
    ///
    /// CLI alternative: `--no-repr-opt`
    /// Usage: `ORI_NO_REPR_OPT=1 ori build file.ori`
    ORI_NO_REPR_OPT

    /// Disable the post-realize redundant project-alias dec cleanup pass.
    ///
    /// Consumed in `ori_arc::aims::realize::cleanup_redundant`.
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Usage: `ORI_DISABLE_REDUNDANT_CLEANUP=1 ori build file.ori`
    ORI_DISABLE_REDUNDANT_CLEANUP

    /// Disable the burden-op emission pass (Step 4b of the AIMS pipeline).
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline` for the
    /// empty-harness predicate-parity check. When set, `emit_burden_ops` is skipped
    /// entirely and the predicate-stack realization path runs as the baseline.
    /// Usage: `ORI_DISABLE_BURDEN_OPS=1 ori build file.ori`
    ORI_DISABLE_BURDEN_OPS

    /// Disable the DP-2/DP-3 burden-op elimination pass.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` to conditionally
    /// bypass burden-op elimination for diagnostic bisection.
    /// Usage: `ORI_DISABLE_BURDEN_ELIM=1 ori build file.ori`
    ORI_DISABLE_BURDEN_ELIM

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

    /// Disable the lineage-aware Phase-6 burden-op re-balance (group-by-rep
    /// net-targeted one-release elision) in `ori_arc::aims::realize::burden_elim`,
    /// falling back to the decoupled per-var elision.
    ///
    /// Diagnostic bisection only: toggles the lineage re-balance off so an
    /// alias-chain double-free can be attributed to the rep-grouping vs the
    /// per-var pass; default (unset) keeps the re-balance active.
    /// Usage: `ORI_DISABLE_LINEAGE_REBALANCE=1 ori build file.ori`
    ORI_DISABLE_LINEAGE_REBALANCE

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

    /// Decline the Phase-6.99 dead-at-bypass-entry fallback in
    /// `compute_take_project_source_plan`: an iterator-bearing take-project
    /// source enum dead-at-entry on the BYPASS edge of an outer runtime gate
    /// reverts to the pre-cure under-release (the bypass path's `+1` iterator
    /// leak).
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified`. Bisects an
    /// outer-gated take-project bypass-path leak to this fallback vs the rest
    /// of the take-project source-dec pass. Default (unset) emits the
    /// dominance-safe dead-at-bypass-entry release.
    /// Usage: `ORI_DISABLE_TAKE_PROJECT_BYPASS_ENTRY_RELEASE=1 ori build file.ori`
    ORI_DISABLE_TAKE_PROJECT_BYPASS_ENTRY_RELEASE

    /// Revert the Phase-6.68 nested-aggregate-into-in-scope-consumed-collection
    /// keep-alive admission (`aggregate_pushed_into_in_scope_consumed_collection`
    /// returns false): the `for p in parts yield Some(p); for opt in opts do ..`
    /// shape reverts to the under-funded double-free (the stored slice view's
    /// backing released by BOTH the source iter-drop and the in-scope
    /// `elem_dec_fn`).
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified`. Bisects the
    /// nested-aggregate arm vs the rest of Phase 6.68. Default (unset) keeps the
    /// RL-1 keep-alive.
    /// Usage: `ORI_DISABLE_NESTED_AGG_INSCOPE_KEEPALIVE=1 ori build file.ori`
    ORI_DISABLE_NESTED_AGG_INSCOPE_KEEPALIVE

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

    /// Disable the `ori_panic` message ownership-transfer contract seed
    /// (default: the panic machinery owns + releases its message; a
    /// still-live message dup-incs per RL-1).
    ///
    /// Consumed in `ori_arc::aims::builtins` (raw `var`). Defined here for
    /// documentation and `check-debug-flags.sh` consistency. Bisects a
    /// panic-message leak / double-free to the transfer seed.
    /// Usage: `ORI_DISABLE_PANIC_MSG_TRANSFER=1 ori build file.ori`
    ORI_DISABLE_PANIC_MSG_TRANSFER

    /// Bypass the Phase-6.98 RL-4 release on dying Invoke UNWIND edges for
    /// vars whose predecessor carries a self-canceling whole-var burden pair
    /// (default: the caught-panic path's dying borrowed arg is released at
    /// the unwind-successor front).
    ///
    /// Consumed in `ori_arc::aims::emit_rc::edge_cleanup` (raw `var`).
    /// Defined here for documentation and `check-debug-flags.sh`
    /// consistency. Bisects a caught-panic-path leak to this release.
    /// Usage: `ORI_DISABLE_INVOKE_UNWIND_PAIR_RELEASE=1 ori build file.ori`
    ORI_DISABLE_INVOKE_UNWIND_PAIR_RELEASE

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

    /// Bypass the Phase-6.99 transfer-anchor credit-net repair on the
    /// result-side lineage of a `transfers_through_return ∧ Owned ∧ Direct`
    /// forwarder call with a caller-fresh arg-side lineage (default: the
    /// net-verified repair removes the single spurious fresh-site keep-alive
    /// inc OR places one release after the execution-final value-read so
    /// every Return path nets 0).
    ///
    /// Consumed in `ori_arc::aims::realize::transfer_anchor_net` (raw
    /// `var`). Defined here for documentation and `check-debug-flags.sh`
    /// consistency. Bisects a forwarder-result leak / double-free to this
    /// repair vs the rest of the burden-strip pipeline.
    /// Usage: `ORI_DISABLE_TRANSFER_ANCHOR_CREDIT_NET=1 ori build file.ori`
    ORI_DISABLE_TRANSFER_ANCHOR_CREDIT_NET

    /// Force every Phase-6.99 same-allocation view into the Opaque
    /// (balanced-pair-or-decline) class, disabling the unified member+view
    /// ledger admission of niche-family (`Option` / `Result`) single-payload
    /// borrow-views (default: a proven same-alloc view's whole-var RC ops
    /// join the per-rep credit net at `±1`, so a lone niche-payload release
    /// is modeled instead of declining the lineage).
    ///
    /// Consumed in `ori_arc::aims::realize::transfer_anchor_net::views`
    /// (raw `var`). Defined here for documentation and
    /// `check-debug-flags.sh` consistency. Bisects a forwarder-result
    /// Option-family verdict change to the view-ledger admission vs the
    /// member-only net.
    /// Usage: `ORI_DISABLE_VIEW_LEDGER_ADMISSION=1 ori build file.ori`
    ORI_DISABLE_VIEW_LEDGER_ADMISSION

    /// Decline the Phase-6.99 WRAPPED transfer-anchor class (the
    /// `Ok(m)`-style wrap-forwarder credit: a callee whose contract proves
    /// `return_payload_contains_param` on EVERY return path with a
    /// same-allocation wrapper result), reverting wrap-forwarder call
    /// sites to the unadmitted treatment (default: the returned wrapper
    /// carries one credit on the payload's allocation; the wrapper, its
    /// payload args, and live extractions form one coupled lineage
    /// eligible for the combined remove-inc + place-release repair).
    ///
    /// Consumed in `ori_arc::aims::realize::transfer_anchor_net::anchors`
    /// (raw `var`). Defined here for documentation and
    /// `check-debug-flags.sh` consistency. Bisects a wrap-forwarder-result
    /// leak / repair change to the wrapped-credit admission vs the Direct
    /// anchor machinery.
    /// Usage: `ORI_DISABLE_WRAPPED_CREDIT_ANCHOR=1 ori build file.ori`
    ORI_DISABLE_WRAPPED_CREDIT_ANCHOR

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

    /// Bypass the Phase-6.68c returned-collection surplus-inc strip (the
    /// spurious cross-call `BurdenInc` on a callee-returned scalar-list
    /// acquired at N>=2 call sites and iter-consumed live).
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var_os`).
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Bisects a returned-scalar-list leak to this pass vs the base walk.
    /// Usage: `ORI_DISABLE_RETURNED_COLLECTION_SURPLUS_INC_STRIP=1 ori build file.ori`
    ORI_DISABLE_RETURNED_COLLECTION_SURPLUS_INC_STRIP

    /// Bypass the Phase-6.66c borrowed-`Invoke` iter-consume source
    /// suppression (the spurious caller FRESH `BurdenInc` + scope-exit
    /// `BurdenDec` on an owned fresh collection iter-consumed via a
    /// borrowed-`Invoke` arg).
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var_os`).
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Bisects a returned-collection-source leak to this pass vs the base walk.
    /// Usage: `ORI_DISABLE_BORROWED_INVOKE_ITER_CONSUME_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_BORROWED_INVOKE_ITER_CONSUME_SUPPRESS

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

    /// Decline the Phase-6.66e loop-invariant iter-consumed SURVIVOR surplus
    /// suppression. A loop-INVARIANT collection (`Construct` outside the loop)
    /// iter-consumed via the inline for-loop `@iter [own]` and READ AFTER the loop
    /// via a borrow (`words.len()`) is the survivor shape; the base walk over-emits
    /// across the loop-carried lineage (`same_alloc_reps` drops the Jump-phi
    /// back-edge) — a surplus fresh-site `BurdenInc` + a surplus pre-read
    /// `BurdenDec` beyond the keep-alive inc + the one post-read survivor release →
    /// net -1, double-free (exit -134). Default (unset): rewrite the survivor rep's
    /// burden ops to the proven oracle ledger (keep ONE keep-alive inc the
    /// `@iter`/`ori_iter_drop` pair balances + the LAST dec, the post-read survivor
    /// release; strip the surplus). The post-loop COLLECTION borrow-read is the
    /// discriminator vs `str_split`/`set_to_list`/`derive_clone` (no COLLECTION
    /// survivor read), so their `@iter` COW protection is untouched.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var`). Defined here
    /// for documentation and `check-debug-flags.sh` consistency. Bisects the
    /// loop-invariant iter-survivor double-free to this pass vs the rest of the
    /// burden path. Spec: Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_LOOP_INVARIANT_ITER_SURVIVOR_SURPLUS=1 ori build file.ori`
    ORI_DISABLE_LOOP_INVARIANT_ITER_SURVIVOR_SURPLUS

    /// Decline the Phase-6.66f sharing-view slice + iter-consume surplus-inc
    /// suppression. A FRESH owned collection (`let words = [..]`) BORROWED into a
    /// seamless-slice producer (`words.take(2)` / `.slice(..)` / `.substring(..)` /
    /// `.drop(..)`) AND iter-consumed via the inline for-loop `@iter [own]` is the
    /// slice+iter-interaction shape. The sharing-view producer rc-INCs the shared
    /// backing buffer (funding the surviving slice's ref, released by the slice's
    /// own scope-exit dec), and the `@iter [own]` -> `ori_iter_drop` is the source
    /// allocation's single transfer release — so the source's correct burden ledger
    /// is ZERO incs. The base walk over-emits keep-alive `BurdenInc`s on the source
    /// lineage (treating the live-across iter-consume of the slice's source as a
    /// duplication) beyond the producer's own inc, so the rc-1 buffer never reaches
    /// 0 (the buffer + its owned element strings leak via the never-run
    /// `elem_dec_fn`). Default (unset): strip every normal-path source `BurdenInc`/
    /// `BurdenDec` on the iter-consumed sharing-view source lineage (unwind-path ops
    /// are panic cleanup, left intact). The surviving-slice borrow-read after the
    /// producer is the discriminator vs the dead-after-slice case the base walk
    /// already balances.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var`). Defined here
    /// for documentation and `check-debug-flags.sh` consistency. Bisects the
    /// slice+iter-consume leak to this pass vs the rest of the burden path.
    /// Spec: Annex E §AIMS RL-1 + RL-2.
    /// Usage: `ORI_DISABLE_SHARING_VIEW_ITER_CONSUME_SURPLUS=1 ori build file.ori`
    ORI_DISABLE_SHARING_VIEW_ITER_CONSUME_SURPLUS

    /// Keep the comparison-operand same-root guard purely structural. Default: a
    /// `==`/`!=` whose two operands share one `same_alloc` rep BECAUSE one operand is
    /// a `transfers_through_return ∧ Direct` forwarder RESULT (`result == a` where
    /// `result` aliases the arg's allocation) is EXEMPTED from the same-root guard —
    /// the two operands are genuinely distinct co-references (the `b = a` duplication
    /// funds the transfer, rc 1 -> 2), so the M3/M4 comparison-operand strip fires
    /// (stripping the spurious operand keep-alive `BurdenInc`, leaving its dec as one
    /// of the two genuine releases). With the toggle set, the guard stays structural
    /// and the forwarder-transfer comparison reverts to the orphaned-dec double-free
    /// (Phase-6 elim strips the spurious inc, the paired dec survives unmatched).
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (Phase 6.97). Bisects a
    /// forwarder-transfer comparison double-free to this exemption.
    /// Usage: `ORI_DISABLE_COMPARISON_FORWARDER_SAME_ROOT_EXEMPT=1 ori build file.ori`
    ORI_DISABLE_COMPARISON_FORWARDER_SAME_ROOT_EXEMPT

    /// Decline the Phase-6.95b for_yield-result premature-release relocation. An
    /// eligible non-transferred-out `ori_list_take` result list (`let copied = for
    /// w in words yield w`) read via sibling `Let`-Var aliases across two blocks
    /// (`copied[0]` then `copied[1]`) has its single normal-path release placed by
    /// the base walk at the EARLY sibling's SSA last-use — the per-SSA-var `live_out`
    /// suppressor misses that a later-block sibling alias keeps the SAME allocation
    /// live, so the list is freed before the later block re-reads it (-134 UAF).
    /// Default (unset): relocate the single premature `BurdenDec` to AFTER the
    /// lineage's execution-final read (one release, moved later; `RL2_release_exactly_once`
    /// preserved). Unwind-path (`Resume`) releases untouched.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var`). Defined here
    /// for documentation and `check-debug-flags.sh` consistency. Bisects the
    /// for_yield-result premature-free to this pass vs the rest of the burden path.
    /// Spec: Annex E §AIMS RL-2 + RL-4.
    /// Usage: `ORI_DISABLE_FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION=1 ori build file.ori`
    ORI_DISABLE_FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION

    /// Bypass the iter-consume + transfer-through-return source-dec
    /// suppression: an owned param both iter-consumed via an `@iter [own]`
    /// call AND transferred through the function's own `Return` keeps its
    /// premature normal-path source `BurdenDec` (freeing the param before the
    /// Return). With the suppression ON, the source dec is dropped so the
    /// kept-from-arrival reference survives as the live Return value.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` (raw `var_os`).
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Bisects an iter-then-return UAF to this pass vs the base walk.
    /// Usage: `ORI_DISABLE_ITER_CONSUME_RETURN_SOURCE_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_ITER_CONSUME_RETURN_SOURCE_SUPPRESS

    // === Alive2 IR Capture ===

    /// Dump raw LLVM IR to a `.preopt.ll` file after verification, before optimization.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Distinct from `ORI_DUMP_AFTER_LLVM` which dumps annotated IR to stderr
    /// for human debugging (and before verification).
    /// Usage: `ORI_DUMP_PREOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_PREOPT_LLVM

    /// Dump raw LLVM IR to a `.postopt.ll` file after optimization, before emission.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Usage: `ORI_DUMP_POSTOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_POSTOPT_LLVM

    /// Enable Alive2 IR capture: dumps both pre-opt and post-opt IR.
    ///
    /// Convenience flag that enables both `ORI_DUMP_PREOPT_LLVM` and
    /// `ORI_DUMP_POSTOPT_LLVM` and places output in `build/alive2-results/`.
    /// Usage: `ORI_ALIVE2_CAPTURE=1 ori build file.ori`
    ORI_ALIVE2_CAPTURE

    // === Verification ===

    /// Enable ARC IR verification after the AIMS pipeline.
    ///
    /// Adds extra correctness checks (RC balance, drop placement).
    /// Usage: `ORI_VERIFY_ARC=1 ori build file.ori`
    ORI_VERIFY_ARC

    /// Enable LLVM IR verification after every optimization pass.
    ///
    /// Catches which optimization pass breaks IR well-formedness.
    /// Significant performance impact (~30-60% slower LLVM tests).
    /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
    ORI_VERIFY_EACH

    /// Run in-pipeline RC audit on emitted LLVM IR.
    ///
    /// Detects leaks, double-frees, COW sequencing bugs, and ABI violations.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ori build file.ori`
    ORI_AUDIT_CODEGEN

    /// Enable strict (pessimistic) mode for the codegen audit.
    ///
    /// Treats COW functions as always-freeing (potential double-free becomes
    /// definite error). Also tracks function pointer parameters as RC-managed.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ori build file.ori`
    ORI_AUDIT_STRICT

    /// Filter codegen audit to a single function by name.
    ///
    /// Only analyzes the function whose LLVM name contains the given string.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_FUNCTION=main ori build file.ori`
    ORI_AUDIT_FUNCTION

    /// Run LLVM lint pass (`function(lint)`) to detect likely-undefined behavior.
    ///
    /// Detects division by potential zero, suspicious alignment, unreachable
    /// patterns, and UB in instruction operands. Auto-enabled by `ORI_AUDIT_CODEGEN=1`.
    /// Usage: `ORI_LLVM_LINT=1 ori build file.ori`
    ORI_LLVM_LINT

    // === Test Harness Flags ===
    // Note: Consumed directly in `ori_test_harness` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Enable bless mode for the shared test harness.
    ///
    /// When set to `"1"`, `compare_or_bless()` writes actual output as the
    /// new expected baseline instead of comparing. Only `"1"` is accepted —
    /// `"0"`, `"false"`, `"true"` are all treated as disabled.
    /// Usage: `ORI_BLESS=1 cargo test -p ori_arc -- aims_snapshot`
    ORI_BLESS

    /// Per-file wall-clock budget (in seconds) for `ori test --backend=llvm`
    /// worker subprocesses.
    ///
    /// A worker still alive at the budget is killed by the runner's watchdog
    /// and its in-flight test counted FAILED (timeout). Default: 120.
    /// Usage: `ORI_TEST_WORKER_TIMEOUT_SECS=30 ori test --backend=llvm tests/`
    ORI_TEST_WORKER_TIMEOUT_SECS

    /// Path of the on-disk cache file for `ori test --incremental`.
    ///
    /// When set, the parent test runner loads the per-function body-hash
    /// snapshots from this file at startup and saves them after each run, so
    /// unchanged-test skipping works across CLI invocations (without it, the
    /// cache is in-memory only and lives for the runner's lifetime). The
    /// parent owns the file exclusively; LLVM worker subprocesses never read
    /// or write it. A missing or unreadable file is an empty cache.
    /// Usage: `ORI_TEST_INCREMENTAL_CACHE=.ori-test-cache ori test --incremental tests/`
    ORI_TEST_INCREMENTAL_CACHE

    /// Per-spawn worker-protocol nonce for `ori test --backend=llvm`
    /// subprocess isolation. Internal — set by the parent runner, not users.
    ///
    /// The parent generates a fresh unguessable token for each worker spawn;
    /// the worker stamps every protocol line with it and refuses `--__worker`
    /// mode without it. Protocol-shaped stdout lines whose token is absent or
    /// mismatched pass through as plain output, so test programs cannot forge
    /// protocol records (test `print()` shares the worker's stdout). The
    /// worker scrubs the variable from its own environment before any test
    /// code runs, so JIT'd code (and anything it spawns) never sees it.
    ORI_TEST_PROTOCOL_TOKEN

    // === Existing Flags (migrated) ===

    /// Print LLVM IR to stderr before JIT compilation.
    ///
    /// Legacy flag — `ORI_DUMP_AFTER_LLVM` is the preferred replacement.
    /// Usage: `ORI_DEBUG_LLVM=1 ori check file.ori`
    ORI_DEBUG_LLVM

    // === Sanitizer Flags ===

    /// Enable sanitizer instrumentation on generated AOT binaries.
    ///
    /// Value: comma-separated sanitizer names (`address`, `undefined`).
    /// Example: `ORI_SANITIZE=address,undefined ori build file.ori`
    ///
    /// Requires Clang on PATH (used as compilation driver for sanitizer passes).
    /// For full coverage, also recompiles `ori_rt` with sanitizer flags (nightly Rust).
    /// Significant performance impact (2-10x slower). Not for main test suite.
    ORI_SANITIZE

    /// Restore the missing RL-1 store-dup inc on a yield-identity
    /// `@ori_list_push` element (default: the inc is emitted — the push of a
    /// borrowed-source iterator-element view into a fresh result list
    /// duplicates the caller-retained source reference; the result
    /// collection's drop is the matched release).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// yield-identity borrowed-source double-free / leak to this inc.
    /// Usage: `ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC=1 ori build file.ori`
    ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC

    /// Restore the missing RL-5 release for a purely-dead loop-invariant
    /// fresh-collection local (default: one dead-at-entry dec is emitted at the
    /// lineage's terminal dead block-param — a `Construct List/Map/Set` threaded
    /// unchanged through loop block-params and never read owes exactly one
    /// release). With the toggle set, the buffer leaks (the pre-cure shape).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// dead loop-invariant local leak to this release.
    /// Usage: `ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LOOP_INVARIANT_DEAD_LOCAL_RELEASE

    /// Restore the FRESH-site `BurdenInc` on a read-only-in-place-co-owner
    /// seamless-slice RESULT (`slice` / `substring` / `take` / `drop`). Default:
    /// the inc is suppressed — the sharing-view runtime self-incs the shared
    /// buffer (`ori_rc_inc`, rc 1 -> 2), so the `MaybeShared` result is an owned
    /// co-reference whose `+1` is the runtime's own inc; AIMS emits ONLY the
    /// balancing last-use dec. A second FRESH-site inc double-counts under
    /// sole-emitter Phase-7 lowering (net +1 leak; the shared buffer never reaches
    /// rc 0). Gated to the surplus shape only (the receiver survives + is read
    /// downstream, the result lineage is a pure borrow-read co-owner not fed to
    /// another sharing-view) — the load-bearing-inc shapes (materialized / moved /
    /// chained / receiver-dies-at-slice) keep the inc. With the toggle set, the
    /// read-only-co-owner slice result reverts to the +1 leak (the pre-cure shape).
    ///
    /// Consumed in `ori_arc::lower::burden_lower` (Phase 5). Bisects a
    /// read-only-co-owner slice leak to this suppression.
    /// Usage: `ORI_DISABLE_SHARING_VIEW_SURPLUS_INC_SUPPRESS=1 ori build file.ori`
    ORI_DISABLE_SHARING_VIEW_SURPLUS_INC_SUPPRESS

    // === Runtime Trace Flags ===
    // Note: These are checked directly in `ori_rt` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Enable RC operation tracing in the runtime.
    ///
    /// Modes: `1` (summary on exit), `verbose` (per-operation log), `quiet` (stats only).
    /// Usage: `ORI_TRACE_RC=1 ori run file.ori`
    ORI_TRACE_RC

    /// Enable runtime debug assertions (bounds checks, underflow detection).
    ///
    /// Usage: `ORI_RT_DEBUG=1 ori run file.ori`
    ORI_RT_DEBUG

    /// Enable leak detection (report live RC objects on exit).
    ///
    /// Usage: `ORI_CHECK_LEAKS=1 ori run file.ori`
    ORI_CHECK_LEAKS
}

// Compile-time sync check: verify that audit env var names in `oric::debug_flags`
// match the canonical constants in `ori_llvm::verify`. If either side renames a
// flag, this assertion fails at compile time.
#[cfg(feature = "llvm")]
const _: () = {
    assert!(
        const_str_eq(ORI_AUDIT_CODEGEN, ori_llvm::verify::ENV_AUDIT_CODEGEN),
        "ORI_AUDIT_CODEGEN constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_AUDIT_STRICT, ori_llvm::verify::ENV_AUDIT_STRICT),
        "ORI_AUDIT_STRICT constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_AUDIT_FUNCTION, ori_llvm::verify::ENV_AUDIT_FUNCTION),
        "ORI_AUDIT_FUNCTION constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_NO_REPR_OPT, ori_repr::NarrowingPolicy::ENV_NO_REPR_OPT),
        "ORI_NO_REPR_OPT constant drifted between oric and ori_repr"
    );
};

/// Const-compatible string equality (stable Rust lacks `const PartialEq` for `&str`).
#[cfg(feature = "llvm")]
const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
