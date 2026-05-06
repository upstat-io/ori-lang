//! BUG-04-111 §05 Step 1 — Emission-disposition predicate SSOT.
//!
//! The `EmissionSite` enum classifies WHERE in a block a variable's RcDec
//! would be emitted. The companion `var_emits_dec_in_block` helper (added
//! incrementally in subsequent §05 steps) returns `Option<EmissionSite>`
//! for each `(ctx, var, is_unwind_block)` triple — the SSOT predicate that
//! all four PIN-4 gate sites consult.
//!
//! Variants carry `instr_idx` for body-position emission sites so
//! canonical-rep selection (§05 Step 3) picks the LATEST-emitting class
//! member by program order (gemini Round 2 F1).
//!
//! See `bug-tracker/plans/BUG-04-111/section-05-implementation.md` Step 1
//! for the gate table per variant + the bypass-safe-region carve-out.

/// Where in a block's emission a variable's `RcDec` would actually fire.
///
/// Pure classification — does NOT include sites mutated stateful by
/// `let_reps_dec_emitted` (the Phase A bypass-safe inline-emit path is
/// handled separately in §05 Step 1's Bypass-safe-region interaction
/// guard at lines 148-156 of section-05-implementation.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    dead_code,
    reason = "BUG-04-111 §05 Step 1 ground-laying — variants consumed by `var_emits_dec_in_block` + canonical_rep_for in §05 Step 1+3 (incremental landing)"
)]
pub(crate) enum EmissionSite {
    /// Source 1 in `dead_cleanup/mod.rs:80-241` — dead-at-entry block-prepended
    /// emission. Fires at block entry (EARLIEST emission position).
    PhaseAEntry,

    /// Source 2 in `dead_cleanup/mod.rs:265-426` — dead-block-param
    /// block-prepended emission. Fires at block entry.
    PhaseABlockParam,

    /// Source 3 — `realize/walk_dec.rs::emit_last_use_decs` body last-use
    /// position. `instr_idx` is the per-block instruction position.
    BodyWalkLastUse { instr_idx: usize },

    /// Source 6 (codex F2) — `walk_dec.rs::emit_defined_dead` at the
    /// defining instruction position.
    DefinedDead { instr_idx: usize },

    /// Source 5 — seeded into `walk_body_unified` via
    /// `child_effective_last_use`. `instr_idx` is the deferred-parent
    /// emission point (where the child's last use dies).
    DeferredParent { instr_idx: usize },

    /// Source 4a — `merge_edge_decs` for `pred_count > 1` paths where the
    /// variable is NOT defined in all predecessors. PIN-4 treats as
    /// out-of-intra-block-order: emission deferred to per-edge cleanup;
    /// classes whose ONLY coverage is `MergeEdgeRouted` DO NOT participate
    /// in PIN-4 suppression of other class members (per codex F1 — edge-
    /// cleanup gates run on their own; PIN-4 cannot predict whether the
    /// per-edge fire will actually occur without inspecting successor
    /// entry states).
    MergeEdgeRouted,
}
