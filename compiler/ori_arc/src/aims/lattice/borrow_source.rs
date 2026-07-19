//! Borrow provenance (sparse side table, not in the finite lattice).

use crate::ir::ArcVarId;

/// Tracks where a borrowed value comes from.
///
/// Stored in a sparse side table keyed by [`ArcVarId`], NOT in the finite
/// lattice. Only relevant for variables with [`super::AccessClass::Borrowed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorrowSource {
    /// Known exact source variable, optionally with the projected field index.
    ///
    /// `field` is `Some(idx)` when the borrow comes from a `Project` instruction,
    /// identifying which field of the source struct/enum was extracted. Used by
    /// the disjoint-field COW optimization to prove that a COW
    /// mutation on a different field doesn't conflict with this borrow.
    Exact {
        source: ArcVarId,
        field: Option<u32>,
    },
    /// Multiple sources or unknown origin.
    Unknown,
}

impl BorrowSource {
    /// Create an `Exact` borrow source without field info.
    #[must_use]
    pub fn exact(source: ArcVarId) -> Self {
        Self::Exact {
            source,
            field: None,
        }
    }

    /// Create an `Exact` borrow source with field info from a `Project`.
    #[must_use]
    pub fn exact_field(source: ArcVarId, field: u32) -> Self {
        Self::Exact {
            source,
            field: Some(field),
        }
    }

    /// Join two borrow sources: same source+field → keep; different → `Unknown`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (
                Self::Exact {
                    source: a,
                    field: fa,
                },
                Self::Exact {
                    source: b,
                    field: fb,
                },
            ) if a == b && fa == fb => Self::Exact {
                source: a,
                field: fa,
            },
            _ => Self::Unknown,
        }
    }
}
