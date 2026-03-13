//! Unified decision functions for AIMS realization.
//!
//! `decide()` (Phase 1) makes RC and reuse decisions from one state query.
//! `decide_annotations()` (Phase 2) makes COW and drop hint decisions.
//!
//! These replace the scattered decision logic across `emit_rc/`, `emit_reuse/`,
//! `cow.rs`, and `drop_hints.rs`. The unified `realize()` calls these
//! functions directly, replacing the 4+ separate traversals with two phases.

use crate::aims::lattice::{AccessClass, Cardinality, Consumption, ShapeClass, Uniqueness};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;

use rustc_hash::FxHashSet;

/// Phase 1 decisions: RC and reuse for a single instruction site.
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
    /// Dec needed but deferred — parent aggregate has borrowed children with
    /// later uses. The caller places the Dec when all children are dead.
    Defer,
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

// Phase 1 context types

/// Pre-computed context for a per-variable RC/reuse decision.
///
/// The forward walk computes these facts for each (var, instruction) pair
/// and calls [`decide()`] to get the unified decision. This replaces the
/// scattered decision logic in `emit_pre_instr_incs()`,
/// `emit_post_instr_decs()`, and `collect_death_events()`.
pub struct DecisionContext {
    /// What kind of decision site this is.
    pub site: DecisionSite,
    /// Whether the variable is RC-managed (owned, non-scalar, non-excluded).
    pub is_rc_managed: bool,
}

/// Classification of the instruction site for RC decisions.
///
/// The forward walk determines this by inspecting use counts, liveness,
/// and instruction semantics before calling [`decide()`].
pub enum DecisionSite {
    /// A use of the variable before the instruction executes.
    ///
    /// `has_future_use` is true if: remaining block uses > 0, or terminator
    /// use pending from this point, or variable is live at block exit.
    Use {
        /// Whether the variable has remaining uses or exit liveness.
        has_future_use: bool,
        /// How this variable is used at the instruction site.
        semantics: UseSemantics,
    },
    /// The variable is defined by this instruction but never used in the
    /// block and dead at block exit. Immediate `RcDec` after definition.
    DefinedDead,
    /// The variable's last use is at this instruction and it's dead at
    /// block exit. This is the potential Dec + reuse site.
    LastUse {
        /// Instruction is a consuming `PrimOp` (list/string concat) that
        /// handles operand RC internally. No separate `RcDec` needed.
        is_consuming_primop: bool,
        /// Instruction transfers ownership (`Let{Var}`, `Construct`, `PartialApply`).
        /// Source variable's RC is subsumed by destination.
        is_ownership_transfer: bool,
        /// Variable is at an owned-transfer call position (Apply/Invoke).
        /// Callee takes ownership — no caller-side `RcDec`.
        is_owned_call_position: bool,
        /// `Project` source with non-scalar result (transfer semantics).
        /// The projected value takes ownership — suppress source `RcDec`.
        is_project_transfer: bool,
        /// Parent aggregate with borrowed children that have later uses.
        /// Dec must be deferred until all children are dead.
        has_deferred_children: bool,
        /// Reuse candidacy context (shape, uniqueness, cardinality).
        reuse: ReuseContext,
    },
}

/// RC identity classification for a variable use.
///
/// The legacy backward walk (`rc_insert`) implicitly encoded RC identity via
/// liveness propagation through `Let { Var }` alias definitions and the
/// `is_borrowing_instr` check for `Project` instructions. The unified forward
/// walk makes this classification explicit.
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — `proj i x` borrows `x`;
/// if the result is an object, Inc it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UseSemantics {
    /// Normal use — emit `RcInc` if `has_future_use`.
    Normal,
    /// `Let { dst, Var(src) }` alias use. The alias root variable already
    /// received an `RcInc` covering this alias's entire lifetime.
    /// No independent `RcInc` needed.
    AliasOf,
    /// `Project` source where the result is scalar (borrowing semantics).
    /// No pre-use `RcInc`. Source may still get a last-use `RcDec`.
    BorrowingProject,
    /// `Project` source where the result is non-scalar (transfer semantics).
    /// No pre-use `RcInc`. Source last-use `RcDec` is suppressed — the
    /// projected value takes ownership of the RC reference.
    TransferProject,
}

/// State-derived context for reuse candidacy at a death point.
///
/// These fields come from the [`AimsStateMap`] — shape from the per-variable
/// shape map, uniqueness and cardinality from the block exit state.
pub struct ReuseContext {
    /// From per-variable shape map (`state_map.var_shape(var)`).
    pub shape: ShapeClass,
    /// From block exit state's uniqueness dimension.
    pub uniqueness: Uniqueness,
    /// From block exit state's cardinality dimension.
    pub cardinality: Cardinality,
}

// Phase 1 decision functions

/// Make unified Phase 1 RC/reuse decisions for a single (var, instruction) site.
///
/// This is the Phase 1 counterpart of [`decide_annotations()`] (Phase 2).
/// The forward walk pre-computes a [`DecisionContext`] for each relevant
/// (var, instruction) pair and calls this function, replacing the scattered
/// decision logic in `emit_pre_instr_incs()`, `emit_post_instr_decs()`,
/// and `collect_death_events()`.
///
/// # RC decisions
///
/// - [`RcDecision::Inc`] — retain before a non-last use (future use exists)
/// - [`RcDecision::Dec`] — release at last use or defined-but-dead
/// - [`RcDecision::Defer`] — Dec needed but parent has live borrowed children
/// - [`RcDecision::None`] — no RC operation (not managed, consumed, transferred)
/// - [`RcDecision::Skip`] — locality-driven (reserved for future use)
///
/// # Reuse decisions
///
/// Reuse candidacy is checked at [`RcDecision::Dec`] sites only. A dying
/// variable with reusable shape and unique (or provably-unique) ownership
/// is a reuse candidate.
pub fn decide(ctx: &DecisionContext) -> InstructionDecisions {
    if !ctx.is_rc_managed {
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    }

    match &ctx.site {
        DecisionSite::Use {
            has_future_use,
            semantics,
        } => InstructionDecisions {
            rc: match semantics {
                // Normal use: Inc if future use exists.
                UseSemantics::Normal => {
                    if *has_future_use {
                        RcDecision::Inc
                    } else {
                        RcDecision::None
                    }
                }
                // Alias/Project sources: RC handled by root or projected value.
                UseSemantics::AliasOf
                | UseSemantics::BorrowingProject
                | UseSemantics::TransferProject => RcDecision::None,
            },
            reuse: ReuseDecision::None,
        },

        DecisionSite::DefinedDead => InstructionDecisions {
            rc: RcDecision::Dec,
            reuse: ReuseDecision::None,
        },

        site @ DecisionSite::LastUse { .. } => decide_last_use(site),
    }
}

/// Last-use decision: Dec with suppression checks and reuse candidacy.
fn decide_last_use(site: &DecisionSite) -> InstructionDecisions {
    let DecisionSite::LastUse {
        is_consuming_primop,
        is_ownership_transfer,
        is_owned_call_position,
        is_project_transfer,
        has_deferred_children,
        reuse,
    } = site
    else {
        // Unreachable: caller only passes LastUse variant.
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    };

    // Consuming PrimOps (list/string concat) handle operand RC internally.
    // Emitting a separate RcDec here would double-free.
    if *is_consuming_primop {
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    }

    // Ownership transfers (Let{Var}, Construct, PartialApply) move the value
    // to the destination. The destination's RcDec handles cleanup.
    if *is_ownership_transfer {
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    }

    // Callee takes ownership at this call position — no caller-side Dec.
    if *is_owned_call_position {
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    }

    // Project source with non-scalar result (transfer semantics).
    // The projected value takes ownership of the RC reference from the
    // parent aggregate. Emitting a Dec for the source would double-free.
    //
    // Ref: Lean 4 `proj i x` — when the result is an object, ownership
    // transfers from x to the projection. No separate Dec for x.
    if *is_project_transfer {
        return InstructionDecisions {
            rc: RcDecision::None,
            reuse: ReuseDecision::None,
        };
    }

    // Parent aggregate with live borrowed children — defer Dec until
    // all children are dead (parent must outlive its children).
    if *has_deferred_children {
        return InstructionDecisions {
            rc: RcDecision::Defer,
            reuse: ReuseDecision::None,
        };
    }

    // Regular last use: Dec with reuse candidacy.
    InstructionDecisions {
        rc: RcDecision::Dec,
        reuse: decide_reuse(reuse),
    }
}

/// Determine reuse candidacy at a death point.
///
/// Checks the cross-dimensional proof from Section 09.2 Shape Activation:
/// - `Unique` → `StaticReuse` (provably sole reference, no `IsShared` check)
/// - `MaybeShared + Once + ReusableCtor` → `StaticReuse` (cross-dimensional)
/// - `MaybeShared` (other) → `DynamicReuse` (needs `IsShared` check)
/// - `Shared` or `NonReusable` shape → `None`
fn decide_reuse(ctx: &ReuseContext) -> ReuseDecision {
    // Non-reusable shapes (collections, closures, etc.) — no reuse.
    if matches!(ctx.shape, ShapeClass::NonReusable) {
        return ReuseDecision::None;
    }

    match ctx.uniqueness {
        // Provably unique — static reuse without runtime check.
        Uniqueness::Unique => ReuseDecision::StaticReuse,

        Uniqueness::MaybeShared => {
            // Cross-dimensional proof: Once (single use) + ReusableCtor
            // (fresh construction, refcount=1) → the value is provably unique
            // at its death point, even though the uniqueness dimension
            // conservatively says MaybeShared.
            if ctx.cardinality == Cardinality::Once
                && matches!(ctx.shape, ShapeClass::ReusableCtor(_))
            {
                ReuseDecision::StaticReuse
            } else {
                ReuseDecision::DynamicReuse
            }
        }

        // Known shared — not a reuse candidate.
        Uniqueness::Shared => ReuseDecision::None,
    }
}

// Phase 2 types

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
/// Provides the instruction-site facts needed by [`decide_annotations()`].
/// All fields are pre-computed by the caller (`realize_annotations` walk)
/// from the state map and IR — [`decide_annotations()`] is a pure function.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool represents an independent binary property of the instruction site"
)]
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
    /// Whether this variable is excluded from analysis (scalar, immortal).
    pub is_excluded: bool,
    /// Access class from state map (for COW-aware borrowing: Owned + Linear + Once).
    pub access: AccessClass,
    /// Consumption from state map (for COW-aware borrowing).
    pub consumption: Consumption,
    /// Cardinality from state map (for cross-dimensional COW proofs).
    pub cardinality: Cardinality,
    /// Shape class from state map (for cross-dimensional COW proofs).
    pub shape: ShapeClass,
    /// Whether this variable's borrow is disjoint from all sibling borrows
    /// (uniqueness-preserving borrows, Section 07.3.2).
    pub is_borrow_disjoint: bool,
    /// Whether this variable's type is a collection (List/Map/Set) —
    /// required for drop hint eligibility.
    pub is_collection: bool,
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
/// Derives [`CowMode`] from uniqueness + instruction-site context.
/// Implements the full logic from `cow.rs::uniqueness_to_cow_mode()`:
///
/// 1. Excluded variables → `Dynamic` (safe fallback)
/// 2. RC-incremented → `Dynamic` (physical refcount > logical uniqueness)
/// 3. COW-aware borrowing (Section 07.3.1): param + `Owned` + `Linear` + `Once`
///    → `StaticUnique` (cross-dimensional proof of unique ownership)
/// 4. `Unique` → `StaticUnique`
/// 5. `MaybeShared` + `CollectionBuffer` + `Once` → `StaticUnique` (cross-dim)
/// 6. `MaybeShared` + `ReusableCtor` + `Once` → `StaticUnique` (cross-dim)
/// 7. `MaybeShared` + disjoint borrow → `StaticUnique` (Section 07.3.2)
/// 8. `MaybeShared` → `Dynamic`
/// 9. `Shared` → `StaticShared`
pub fn decide_cow(ctx: &AnnotationSiteContext<'_>) -> CowMode {
    // Excluded (scalar, immortal) → safe fallback.
    if ctx.is_excluded {
        return CowMode::Dynamic;
    }

    // Post-RC-emission guard: physical refcount may exceed logical uniqueness.
    if ctx.rc_incremented {
        return CowMode::Dynamic;
    }

    // COW-aware borrowing (Section 07.3.1): parameter with combined state
    // (Owned, Linear, Once) is provably uniquely owned at this point.
    if ctx.is_param && is_cow_aware_unique(ctx) {
        return CowMode::StaticUnique;
    }

    match ctx.uniqueness {
        Uniqueness::Unique => CowMode::StaticUnique,

        Uniqueness::MaybeShared => {
            // Cross-dimensional: CollectionBuffer + Once → fresh collection
            // literal (refcount=1) used once (no duplication). Non-param only
            // (params use COW-aware borrowing above).
            if !ctx.is_param
                && ctx.cardinality == Cardinality::Once
                && ctx.shape == ShapeClass::CollectionBuffer
            {
                return CowMode::StaticUnique;
            }

            // Cross-dimensional: ReusableCtor + Once → same proof as reuse.
            if !ctx.is_param
                && ctx.cardinality == Cardinality::Once
                && matches!(ctx.shape, ShapeClass::ReusableCtor(_))
            {
                return CowMode::StaticUnique;
            }

            // Uniqueness-preserving borrows (Section 07.3.2): receiver's
            // borrow is disjoint from all sibling borrows of the same source.
            if ctx.is_borrow_disjoint {
                return CowMode::StaticUnique;
            }

            CowMode::Dynamic
        }

        Uniqueness::Shared => CowMode::StaticShared,
    }
}

/// Check if a variable's combined state qualifies for COW-aware borrowing.
///
/// The combined state `(Owned, Linear, Once)` proves unique ownership from
/// cross-dimensional reasoning (Section 07.3.1).
fn is_cow_aware_unique(ctx: &AnnotationSiteContext<'_>) -> bool {
    ctx.access == AccessClass::Owned
        && ctx.consumption == Consumption::Linear
        && ctx.cardinality == Cardinality::Once
}

/// Make Phase 2 annotation decisions for a drop hint site.
///
/// Returns `true` if the variable is eligible for unique-drop fast path.
/// Matches the logic in `drop_hints.rs::compute_aims_drop_hints()`:
///
/// 1. Excluded variables (scalar, immortal) → not eligible
/// 2. Non-collection types → not eligible (drop hints are for buffer cleanup)
/// 3. RC-incremented → not eligible (physical refcount may exceed 1)
/// 4. Borrowed call arg → not eligible (callee may have internally inc'd)
/// 5. Unique → eligible for unique-drop fast path
pub fn decide_drop_hint(ctx: &AnnotationSiteContext<'_>) -> bool {
    // Excluded (scalar, immortal) — no drop hint needed.
    if ctx.is_excluded {
        return false;
    }

    // Only collections (List, Map, Set) have buffer-based drops.
    if !ctx.is_collection {
        return false;
    }

    // Unique variables with no RcInc and no borrowed-arg sharing
    // can use the unique-drop fast path.
    ctx.uniqueness == Uniqueness::Unique && !ctx.rc_incremented && !ctx.is_borrowed_call_arg
}
