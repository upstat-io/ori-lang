//! AIMS late-pipeline ablation toggles (Phase 5/6, RL-1/RL-5/sharing-view/negative-pin family).
//!
//! Each `ORI_DISABLE_*` toggle reverts one named release/suppression treatment
//! in `ori_arc::lower::burden_lower` or `ori_arc::aims::realize::burden_elim`;
//! `ORI_FORCE_OVERELIMINATE` is a negative-pin harness toggle, never a
//! production path. See the crate-level `debug_flags` module doc for the
//! `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
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

    /// Disable the Phase-5 catch-recover release for the recovered message
    /// buffer (the recovered message buffer leaks on the normal path).
    ///
    /// Consumed in `ori_arc::lower::burden_lower`. Bisects a catch-recover
    /// message leak to this treatment vs the rest of the Phase-5 walk. Spec:
    /// Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
    /// Usage: `ORI_DISABLE_CATCH_RECOVER_RELEASE=1 ori build file.ori`
    ORI_DISABLE_CATCH_RECOVER_RELEASE

    /// Decline the RL-5 release for the BORROW-ONLY-READ loop-invariant
    /// fresh-collection local (the borrow-read sibling of the purely-dead
    /// family).
    ///
    /// Default (unset): the release is emitted. Consumed in
    /// `ori_arc::lower::burden_lower`. Spec: Annex E §AIMS RL-5.
    /// Usage: `ORI_DISABLE_LOOP_INVARIANT_BORROW_ONLY_RELEASE=1 ori build file.ori`
    ORI_DISABLE_LOOP_INVARIANT_BORROW_ONLY_RELEASE

    /// Force the deliberately-over-eliminating Phase-6 burden-elim shape:
    /// every whole-var + field-grain `BurdenDec*` release the DP-2 guard
    /// normally preserves is dropped, so a value that survives its
    /// container's drop loses its RL-2 scope-exit release and leaks.
    ///
    /// Default (unset): the guard stays; the pass is byte-identical.
    /// Negative-pin harness only — never a production path. Consumed in
    /// `ori_arc::aims::realize::burden_elim`.
    /// Usage: `ORI_FORCE_OVERELIMINATE=1 ori build file.ori`
    ORI_FORCE_OVERELIMINATE

}
