//! Uniqueness lattice types for COW check elimination.
//!
//! Defines the three-point lattice (`Unique` / `MaybeShared` / `Shared`) and
//! the derived [`CowMode`] annotation consumed by LLVM codegen.

use std::fmt;

/// Abstract uniqueness state for a value.
///
/// Classifies whether a value is provably uniquely owned (RC == 1),
/// provably shared (RC > 1), or unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uniqueness {
    /// Provably uniquely owned. RC is guaranteed to be 1.
    /// The runtime COW check can be eliminated — emit only the fast path.
    Unique,

    /// May or may not be shared. The runtime check is needed.
    /// This is the conservative default for function parameters and
    /// values with unknown provenance.
    MaybeShared,

    /// Provably shared (RC > 1). The slow path is always taken.
    /// Rare in practice — mostly for values bound to multiple variables
    /// without intervening COW operations.
    Shared,
}

impl Uniqueness {
    /// Lattice join (least upper bound).
    ///
    /// Used at control flow merge points (if/else, match arms) to combine
    /// the uniqueness states from all incoming edges.
    ///
    /// ```text
    /// Unique ⊔ Unique = Unique
    /// Shared ⊔ Shared = Shared
    /// Unique ⊔ Shared = MaybeShared
    /// MaybeShared ⊔ _ = MaybeShared
    /// ```
    #[inline]
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unique, Self::Unique) => Self::Unique,
            (Self::Shared, Self::Shared) => Self::Shared,
            _ => Self::MaybeShared,
        }
    }

    /// Whether the runtime COW check can be eliminated.
    #[inline]
    pub fn is_unique(self) -> bool {
        self == Self::Unique
    }

    /// Whether the runtime COW check is definitely needed.
    #[inline]
    pub fn is_maybe_shared(self) -> bool {
        self == Self::MaybeShared
    }

    /// Whether the value is provably shared (slow path always).
    #[inline]
    pub fn is_shared(self) -> bool {
        self == Self::Shared
    }
}

impl fmt::Display for Uniqueness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unique => write!(f, "unique"),
            Self::MaybeShared => write!(f, "maybe_shared"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

/// Annotation on a COW operation indicating whether the runtime uniqueness
/// check can be eliminated.
///
/// Produced by static uniqueness analysis and consumed by LLVM codegen.
/// When a collection operation (push, insert, etc.) is annotated with
/// `StaticUnique`, the codegen emits only the fast (in-place) path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CowMode {
    /// Runtime check needed: `if ori_rc_is_unique(ptr) { fast } else { slow }`.
    /// Default when uniqueness cannot be proven statically.
    Dynamic,

    /// Statically proven unique — emit only the fast path.
    /// No `ori_rc_is_unique` call, no branch, no slow path code.
    StaticUnique,

    /// Statically proven shared — emit only the slow path.
    /// Rare; useful when a value is provably aliased (e.g., bound twice).
    StaticShared,
}

impl CowMode {
    /// Derive the `CowMode` from a `Uniqueness` state.
    #[inline]
    pub fn from_uniqueness(u: Uniqueness) -> Self {
        match u {
            Uniqueness::Unique => Self::StaticUnique,
            Uniqueness::MaybeShared => Self::Dynamic,
            Uniqueness::Shared => Self::StaticShared,
        }
    }
}

impl fmt::Display for CowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dynamic => write!(f, "dynamic"),
            Self::StaticUnique => write!(f, "static_unique"),
            Self::StaticShared => write!(f, "static_shared"),
        }
    }
}
