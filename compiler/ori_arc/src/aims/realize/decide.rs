//! Unified decision functions for AIMS realization.
//!
//! `decide()` (Phase 1) makes RC and reuse decisions from one state query.
//! `decide_annotations()` (Phase 2) makes COW and drop hint decisions.
//!
//! These replace the scattered decision logic across `emit_rc/`, `emit_reuse/`,
//! `cow.rs`, and `drop_hints.rs`. In Stage 2, the unified `realize()` calls
//! these functions directly. In the current incremental migration (Stage 1→2),
//! the existing emission modules still make their own decisions, and these
//! decision types serve as the target API for progressive migration.

use crate::aims::lattice::Uniqueness;
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;

use rustc_hash::FxHashSet;

/// Phase 1 decisions: RC and reuse for a single instruction site.
///
/// Currently a target type — the existing `emit_rc_ops()` and `emit_reuse()`
/// still make their own decisions internally. As `realize_rc_reuse()` is
/// migrated to use `decide()` directly (Section 10.2), this struct becomes
/// the single decision point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionDecisions {
    /// RC decision for this instruction site.
    pub rc: RcDecision,
    /// Reuse decision for this instruction site.
    pub reuse: ReuseDecision,
}

/// RC operation to emit at an instruction site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RcDecision {
    /// No RC operation needed (scalar, dead, or absorbed by reuse).
    None,
    /// Emit `RcInc` (retain before non-last use).
    Inc,
    /// Emit `RcDec` (release at last use or dead variable).
    Dec,
    /// Skip RC — locality-driven (`FunctionLocal` + `Linear` → no RC needed).
    Skip,
}

/// Reuse decision for a dying value at an instruction site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReuseDecision {
    /// No reuse opportunity at this site.
    None,
    /// Static reuse: provably unique, no runtime check.
    StaticReuse,
    /// Dynamic reuse: emit `IsShared` + branch.
    DynamicReuse,
}

/// Phase 2 decisions: COW and drop hints for a post-merge instruction site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationDecisions {
    /// COW mode for this instruction site (None = not a COW site).
    pub cow: Option<CowMode>,
    /// Whether to emit a drop hint for unique collection fast-path.
    pub drop_hint: bool,
}

/// Context for Phase 2 annotation decisions at a single instruction site.
///
/// Provides the instruction-site facts needed by `decide_annotations()`.
pub struct AnnotationSiteContext<'a> {
    /// The variable being annotated.
    pub var: ArcVarId,
    /// Uniqueness from the state map (via `var_state_at_block_entry`).
    pub uniqueness: Uniqueness,
    /// Whether this variable has been `RcInc`'d (physical refcount may
    /// exceed logical uniqueness).
    pub rc_incremented: bool,
    /// Whether this variable is a function parameter.
    pub is_param: bool,
    /// Whether this variable was passed as a Borrowed argument to a function.
    pub is_borrowed_call_arg: bool,
    /// Set of all RC-incremented variables (for transitive alias checks).
    pub rc_incremented_set: &'a FxHashSet<ArcVarId>,
}

/// Make all Phase 2 annotation decisions for a single instruction site.
///
/// Combines COW and drop hint decisions from one `AnnotationSiteContext`.
/// This is the unified Phase 2 decision entry point — callers should prefer
/// this over calling `decide_cow()` and `decide_drop_hint()` separately.
///
/// `is_cow_site` indicates whether this instruction is a COW method call
/// (Apply/Invoke targeting a COW builtin). When false, `cow` is `None`.
/// `is_drop_site` indicates whether this instruction is an `RcDec`.
/// When false, `drop_hint` is `false`.
pub fn decide_annotations(
    ctx: &AnnotationSiteContext<'_>,
    is_cow_site: bool,
    is_drop_site: bool,
) -> AnnotationDecisions {
    AnnotationDecisions {
        cow: if is_cow_site {
            Some(decide_cow(ctx))
        } else {
            None
        },
        drop_hint: is_drop_site && decide_drop_hint(ctx),
    }
}

/// Make Phase 2 annotation decisions for a COW site.
///
/// Derives `CowMode` from uniqueness + instruction-site context. Matches
/// the logic in `cow.rs::uniqueness_to_cow_mode()`.
pub fn decide_cow(ctx: &AnnotationSiteContext<'_>) -> CowMode {
    match ctx.uniqueness {
        Uniqueness::Unique => {
            if ctx.rc_incremented {
                // Physical refcount may be >1 despite logical uniqueness.
                CowMode::Dynamic
            } else {
                CowMode::StaticUnique
            }
        }
        Uniqueness::MaybeShared => CowMode::Dynamic,
        Uniqueness::Shared => CowMode::StaticShared,
    }
}

/// Make Phase 2 annotation decisions for a drop hint site.
///
/// Returns `true` if the variable is eligible for unique-drop fast path.
/// Matches the logic in `drop_hints.rs::compute_aims_drop_hints()`.
pub fn decide_drop_hint(ctx: &AnnotationSiteContext<'_>) -> bool {
    // Unique variables with no RcInc and no borrowed-arg sharing
    // can use the unique-drop fast path.
    ctx.uniqueness == Uniqueness::Unique && !ctx.rc_incremented && !ctx.is_borrowed_call_arg
}
