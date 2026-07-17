//! COW and drop-hint decisions for post-merge AIMS sites.

use crate::aims::lattice::{AccessClass, Cardinality, Consumption, ShapeClass, Uniqueness};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;

use rustc_hash::FxHashSet;

/// COW and drop decisions for one post-merge instruction site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationDecisions {
    /// COW mode for this instruction site (None = not a COW site).
    pub cow: Option<CowMode>,
    /// Whether to emit a drop hint for unique collection fast-path.
    pub drop_hint: bool,
}

/// Complete converged facts for one annotation site.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool represents an independent binary property of the instruction site"
)]
pub struct AnnotationSiteContext<'a> {
    /// The variable being annotated.
    pub var: ArcVarId,
    /// Uniqueness from the state map (via `var_state_at_block_entry`).
    pub uniqueness: Uniqueness,
    /// Whether realization added an owner credit through the transitional
    /// `RcInc` carrier, invalidating a pre-event single-owner fact.
    pub rc_incremented: bool,
    /// Whether this variable is a function parameter.
    pub is_param: bool,
    /// Whether this variable is a function parameter with `Ownership::Borrowed`.
    /// Borrowed parameters cannot use single-owner cleanup because the caller
    /// retains the governing owner credit.
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

/// Computes COW and drop decisions from one site context.
///
/// `cow` is set only for `is_cow_site`; `drop_hint` is set only for
/// `is_drop_site`.
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

/// Chooses the COW strategy allowed by current uniqueness and alias facts.
///
/// Outstanding owner credits force a dynamic probe. A unique value is mutable
/// in place only without active borrows; shared values always copy. A disjoint
/// borrow may preserve unique mutation for a `MaybeShared` receiver.
pub fn decide_cow(ctx: &AnnotationSiteContext<'_>) -> CowMode {
    if ctx.is_excluded {
        return CowMode::Dynamic;
    }

    // Why: An added owner credit invalidates a pre-emission uniqueness proof.
    if ctx.rc_incremented {
        return CowMode::Dynamic;
    }

    match ctx.uniqueness {
        Uniqueness::Unique => {
            // INVARIANT: Active aggregate borrows forbid in-place mutation.
            if ctx.has_active_borrows {
                CowMode::StaticShared
            } else {
                CowMode::StaticUnique
            }
        }

        Uniqueness::MaybeShared => {
            // INVARIANT: Disjoint sibling borrows preserve source uniqueness here.
            if ctx.is_borrow_disjoint {
                return CowMode::StaticUnique;
            }

            CowMode::Dynamic
        }

        Uniqueness::Shared => CowMode::StaticShared,
    }
}

/// Grants the unique-drop fast path only to analyzed collection owners.
///
/// Excluded values, borrowed parameters, outstanding owner credits, and
/// borrowed-call aliases are ineligible.
pub fn decide_drop_hint(ctx: &AnnotationSiteContext<'_>) -> bool {
    if ctx.is_excluded {
        return false;
    }

    if !ctx.is_collection {
        return false;
    }

    if ctx.is_param_borrowed {
        return false;
    }

    ctx.uniqueness == Uniqueness::Unique && !ctx.rc_incremented && !ctx.is_borrowed_call_arg
}
