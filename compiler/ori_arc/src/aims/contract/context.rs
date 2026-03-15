//! Constructor-context types for TRMC (Tail Recursion Modulo Context).
//!
//! [`ContextBehavior`] describes whether a function preserves or consumes
//! constructor contexts. [`ContextRegion`] describes a region of code where
//! a self-recursive function builds a constructor in tail context.
//!
//! Both types are produced during contract extraction (Section 13.1) and
//! consumed by intraprocedural analysis and the TRMC normalize pass.

/// Constructor-context behavior for TRMC (Stage 3a).
///
/// Describes whether a function preserves/consumes constructor contexts.
/// Computed from `ContextRegion` metadata during contract extraction
/// (Section 13.1). Functions without TRMC candidates get `default()`.
///
/// When the TRMC rewrite fires (`was_transformed`), the function is
/// converted to a loop with context block params. The context behavior
/// then describes the original function's structure (pre-rewrite).
///
/// **Soundness gate (Section 09.2):** In-place TRMC requires the context
/// variable to be unique (Lemma 2 — unique linear chain). When
/// `may_resume_nonlinearly == true`, effect handlers could capture the
/// context non-linearly, breaking uniqueness. The per-variable
/// `Uniqueness::Unique` gate in `detect_trmc_candidates()` is the enforced
/// soundness condition; the effect gate is a pending design decision
/// (blocked on effect-handler semantics — see Section 13.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "4 independent boolean flags from literature (Leijen & Lorenzen JFP 2025); \
              bitfields not warranted for a type used once per function contract"
)]
pub struct ContextBehavior {
    /// Does this function preserve a constructor context passed to it?
    /// True when the function's return value includes the context variable
    /// (not consumed or dropped).
    pub preserves_context: bool,
    /// Does this function consume the context hole?
    /// True when the function fills the hole field of the context variable.
    pub consumes_hole: bool,
    /// Does in-place TRMC require the context variable to be unique?
    /// Always `true` for the modulo-cons instantiation. `false` only if a
    /// CPS fallback is used (not implemented in v1).
    pub requires_unique_context: bool,
    /// Can effect handlers in scope resume more than once?
    /// Derived from `EffectSummary.may_share`. When `true`, the context
    /// variable may be captured non-linearly, breaking the unique linear
    /// chain invariant (Leijen & Lorenzen, JFP 2025, Lemma 2).
    pub may_resume_nonlinearly: bool,
}

impl Default for ContextBehavior {
    /// Conservative default: no context preservation or hole consumption,
    /// requires uniqueness, no non-linear resumption.
    fn default() -> Self {
        Self {
            preserves_context: false,
            consumes_hole: false,
            requires_unique_context: true, // conservative: require until proven unnecessary
            may_resume_nonlinearly: false,
        }
    }
}

impl ContextBehavior {
    /// Componentwise join (conservative in both directions):
    /// - `preserves_context`, `consumes_hole`: AND (must hold on ALL paths)
    /// - `requires_unique_context`, `may_resume_nonlinearly`: OR (ANY path triggers)
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            preserves_context: self.preserves_context && other.preserves_context,
            consumes_hole: self.consumes_hole && other.consumes_hole,
            requires_unique_context: self.requires_unique_context || other.requires_unique_context,
            may_resume_nonlinearly: self.may_resume_nonlinearly || other.may_resume_nonlinearly,
        }
    }
}

/// TRMC constructor-context region metadata (Stage 3).
///
/// Describes a region of code where a self-recursive function builds
/// a constructor in tail context — a `Construct` instruction where one
/// field argument is produced by a recursive call. The region spans from
/// the constructor allocation (context open) to the recursive call that
/// fills the hole (context close).
///
/// Produced by `aims::normalize::detect_context_regions()`, consumed by
/// `intraprocedural::analyze_function()` to record `ContextOpen`/`ContextClose`
/// events in the sparse event table.
///
/// # Soundness (Lemma 2, Leijen & Lorenzen JFP 2025)
///
/// In-place TRMC requires the context variable to be `Unique` (refcount == 1)
/// at every point between context creation and application. The intraprocedural
/// analysis verifies this via the converged `Uniqueness` dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRegion {
    /// Block containing the `Construct` instruction (context open).
    pub open_block: crate::ir::ArcBlockId,
    /// Instruction index of the `Construct` within `open_block`.
    pub open_instr: usize,
    /// Variable defined by the `Construct` (the context root).
    pub context_var: crate::ir::ArcVarId,
    /// Which field of the constructor holds the recursive call result (the "hole").
    pub hole_field: u32,
    /// Block containing the recursive call that produces the hole value.
    pub close_block: crate::ir::ArcBlockId,
    /// Instruction index of the `Apply`/`Invoke` that fills the hole.
    pub close_instr: usize,
    /// Variable defined by the recursive call (the value that fills the hole).
    pub hole_var: crate::ir::ArcVarId,
}
