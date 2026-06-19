//! Phase 2 decision functions for AIMS realization.
//!
//! `decide_annotations` makes COW and drop hint decisions from one
//! `AnnotationSiteContext`.

use crate::aims::lattice::{AccessClass, Cardinality, Consumption, ShapeClass, Uniqueness};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;

use rustc_hash::FxHashSet;

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
/// Provides the instruction-site facts needed by [`decide_annotations`].
/// All fields are pre-computed by the caller (`realize_annotations` walk)
/// from the state map and IR — [`decide_annotations`] is a pure function.
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
    /// Whether this variable is a function parameter with `Ownership::Borrowed`.
    /// Borrowed parameters must never use unique-drop because the caller
    /// retains a reference — the buffer is never uniquely owned by the callee.
    pub is_param_borrowed: bool,
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
    /// (uniqueness-preserving borrows).
    pub is_borrow_disjoint: bool,
    /// Whether any borrow from this variable (as an aggregate source) exists
    /// anywhere in the function. Used by DP-5/DP-9: Unique aggregates with
    /// active borrows must use `StaticShared`, not `StaticUnique`. Function-wide
    /// check — conservative (may block `StaticUnique` when borrows are dead at
    /// the COW site, but never permits unsafe in-place mutation).
    pub has_active_borrows: bool,
    /// Whether this variable's type is a collection (List/Map/Set) —
    /// required for drop hint eligibility.
    pub is_collection: bool,
}

/// Make all Phase 2 annotation decisions for a single instruction site.
///
/// Combines COW and drop hint decisions from one `AnnotationSiteContext`.
/// This is the unified Phase 2 decision entry point — callers should prefer
/// this over calling `decide_cow` and `decide_drop_hint` separately.
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
/// Derives [`CowMode`] from uniqueness + instruction-site context. Uniqueness
/// is the SOLE PAST-guarantee source for `StaticUnique` promotion per
/// §DP-9. The removed DP-10 pattern (deriving past uniqueness
/// from `Owned + Linear + Once` or `Once + CollectionBuffer`/`ReusableCtor`)
/// was unsound — backward-analysis facts (consumption, cardinality) are
/// FUTURE guarantees and cannot prove PAST uniqueness (RC == 1 right now).
/// Re-enabling the spec-approved `MaybeShared + IC-3 ParamContract.uniqueness
/// = Unique → StaticUnique` path is tracked as BUG-04-079, blocked on
/// BUG-04-069.
///
/// 1. Excluded variables → `Dynamic` (safe fallback)
/// 2. RC-incremented → `Dynamic` (physical refcount > logical uniqueness)
/// 3. `Unique` → `StaticUnique`
/// 4. `MaybeShared` + disjoint borrow → `StaticUnique` (spec §DP-5/§RL-10 —
///    source uniqueness at the receiver's block is enforced by
///    `is_borrow_disjoint_from_siblings`)
/// 5. `MaybeShared` → `Dynamic` (runtime `IsShared` check)
/// 6. `Shared` → `StaticShared`
pub fn decide_cow(ctx: &AnnotationSiteContext<'_>) -> CowMode {
    // Excluded (scalar, immortal) → safe fallback.
    if ctx.is_excluded {
        return CowMode::Dynamic;
    }

    // Post-RC-emission guard: physical refcount may exceed logical uniqueness.
    if ctx.rc_incremented {
        return CowMode::Dynamic;
    }

    match ctx.uniqueness {
        Uniqueness::Unique => {
            // Spec DP-5/DP-9: Unique AND NOT is_owned_and_unique+no_borrows → StaticShared.
            // IsShared on a Unique value always returns false, so a runtime
            // Dynamic check cannot distinguish "unique but borrowed" from
            // "unique and safe to mutate." Must copy unconditionally.
            if ctx.has_active_borrows {
                CowMode::StaticShared
            } else {
                CowMode::StaticUnique
            }
        }

        Uniqueness::MaybeShared => {
            // Uniqueness-preserving local mutation (spec §DP-5/§RL-10):
            // receiver's borrow is disjoint from all sibling borrows of
            // the SAME source, AND the source itself is `Uniqueness::Unique`
            // at the receiver's block (enforced by `is_borrow_disjoint_from_siblings`).
            if ctx.is_borrow_disjoint {
                return CowMode::StaticUnique;
            }

            CowMode::Dynamic
        }

        Uniqueness::Shared => CowMode::StaticShared,
    }
}

/// Make Phase 2 annotation decisions for a drop hint site.
///
/// Returns `true` if the variable is eligible for unique-drop fast path.
/// Matches the logic in `drop_hints.rs::compute_aims_drop_hints`:
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

    // Borrowed parameters must never use unique-drop: the caller retains
    // a reference, so the buffer is never uniquely owned by the callee.
    if ctx.is_param_borrowed {
        return false;
    }

    // Unique variables with no RcInc and no borrowed-arg sharing
    // can use the unique-drop fast path.
    ctx.uniqueness == Uniqueness::Unique && !ctx.rc_incremented && !ctx.is_borrowed_call_arg
}
