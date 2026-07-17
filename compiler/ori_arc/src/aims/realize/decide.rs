//! COW and drop-hint decisions for post-merge AIMS sites.

use crate::aims::lattice::Uniqueness;
use crate::uniqueness::CowMode;

/// Borrow facts consumed by the COW decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CowBorrowFacts {
    /// Whether every sibling borrow is disjoint from the mutation target.
    pub(super) is_disjoint_from_siblings: bool,
    /// Whether the receiver has any aggregate borrow in the function.
    pub(super) has_active_borrows: bool,
}

/// Converged facts consumed by a COW site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CowSiteContext {
    /// Uniqueness at the block entry, narrowed by any applicable contract.
    pub(super) uniqueness: Uniqueness,
    /// Whether realization added an owner credit before this site.
    pub(super) rc_incremented: bool,
    /// Whether the receiver is outside ownership analysis.
    pub(super) is_excluded: bool,
    /// Borrow facts required by DP-5 and DP-9.
    pub(super) borrows: CowBorrowFacts,
}

/// Borrowed-owner facts consumed by the drop-hint decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DropBorrowFacts {
    /// Whether the value is a borrowed function parameter.
    pub(super) is_param_borrowed: bool,
    /// Whether the value was passed as a borrowed call argument.
    pub(super) is_borrowed_call_arg: bool,
}

/// Converged facts consumed by an `RcDec` drop-hint site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DropSiteContext {
    /// Uniqueness at the block entry, narrowed by any applicable contract.
    pub(super) uniqueness: Uniqueness,
    /// Whether realization added an owner credit before this site.
    pub(super) rc_incremented: bool,
    /// Whether the value is outside ownership analysis.
    pub(super) is_excluded: bool,
    /// Whether the value has a collection representation.
    pub(super) is_collection: bool,
    /// Borrowed-owner facts that preclude the unique-drop fast path.
    pub(super) borrows: DropBorrowFacts,
}

/// Facts for one post-merge annotation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnnotationSite {
    /// A collection operation that requires a COW strategy.
    Cow(CowSiteContext),
    /// An `RcDec` that may take the unique-collection drop fast path.
    Drop(DropSiteContext),
}

/// Decision for one post-merge annotation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnnotationDecision {
    /// The COW mode for a collection mutation.
    Cow(CowMode),
    /// Whether an `RcDec` may use the unique-collection drop fast path.
    DropHint(bool),
}

/// Computes the only decision valid for the supplied annotation site.
pub(super) fn decide_annotation(site: AnnotationSite) -> AnnotationDecision {
    match site {
        AnnotationSite::Cow(ctx) => AnnotationDecision::Cow(decide_cow(ctx)),
        AnnotationSite::Drop(ctx) => AnnotationDecision::DropHint(decide_drop_hint(ctx)),
    }
}

/// Chooses the COW strategy allowed by current uniqueness and alias facts.
///
/// Outstanding owner credits force a dynamic probe. A unique value is mutable
/// in place only without active borrows; shared values always copy. A disjoint
/// borrow may preserve unique mutation for a `MaybeShared` receiver.
pub(super) fn decide_cow(ctx: CowSiteContext) -> CowMode {
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
            if ctx.borrows.has_active_borrows {
                CowMode::StaticShared
            } else {
                CowMode::StaticUnique
            }
        }

        Uniqueness::MaybeShared => {
            // INVARIANT: Disjoint sibling borrows preserve source uniqueness here.
            if ctx.borrows.is_disjoint_from_siblings {
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
pub(super) fn decide_drop_hint(ctx: DropSiteContext) -> bool {
    if ctx.is_excluded {
        return false;
    }

    if !ctx.is_collection {
        return false;
    }

    if ctx.borrows.is_param_borrowed {
        return false;
    }

    ctx.uniqueness == Uniqueness::Unique && !ctx.rc_incremented && !ctx.borrows.is_borrowed_call_arg
}
