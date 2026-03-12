//! Dimension enums for the AIMS ownership lattice.
//!
//! Each dimension is a small finite lattice with componentwise join.
//! The product of all seven dimensions forms [`super::AimsState`].

// Access dimension (aliasing)

/// Whether a value is an owned allocation or a borrowed view.
///
/// RC emission depends on access: only `Owned` values carry RC obligations.
/// Join: `Owned` if either side is `Owned`. Chain height: 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccessClass {
    /// Temporary view of another value. No RC operations.
    Borrowed,
    /// The value owns its allocation. RC operations may be needed.
    Owned,
}

impl AccessClass {
    /// Componentwise join: `Owned` absorbs `Borrowed`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

// Consumption dimension (substructural)

/// Substructural consumption mode. `Borrowed` is NOT here — see [`AccessClass`].
///
/// Ordered: `Dead < Linear < Affine < Unrestricted`. Chain height: 3.
/// Based on Chirimar et al.: `rc_inc` = contraction, `rc_dec` = weakening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Consumption {
    /// Not live at this point. No RC operations needed.
    Dead,
    /// Consumed exactly once (moved). No RC inc/dec needed.
    Linear,
    /// May be dropped without use (e.g., in an else branch).
    /// RC dec may be needed, but no RC inc.
    Affine,
    /// May be freely copied and dropped. Full RC required.
    Unrestricted,
}

impl Consumption {
    /// Componentwise join: max of the two.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

// Cardinality dimension (forward usage count)

/// Forward usage count. Inspired by GHC demand analysis (POPL 2014).
///
/// Ordered: `Absent < Once < Many`. Chain height: 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cardinality {
    /// Never used after this point.
    Absent,
    /// Used exactly once.
    Once,
    /// Used multiple times (or in a loop).
    Many,
}

impl Cardinality {
    /// Alternative control-flow join: `max` of the two.
    ///
    /// Used at control-flow merge points where only one path executes.
    /// `join(Once, Once) = Once` — a value used once in each branch of
    /// an `if` is still used once per execution.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }

    /// Alias for `join` — alternative control-flow join.
    #[must_use]
    pub fn alt_join(self, other: Self) -> Self {
        self.join(other)
    }

    /// Sequential composition along one execution path.
    ///
    /// `Absent + x = x`, `Once + Once = Many`, `Many + _ = Many`.
    ///
    /// Used when a value is demanded by multiple instructions along the
    /// same execution path (not at merge points).
    #[must_use]
    pub fn seq_add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Absent, x) | (x, Self::Absent) => x,
            (Self::Once, Self::Once) | (Self::Many, _) | (_, Self::Many) => Self::Many,
        }
    }
}

// Uniqueness dimension

/// Runtime reference count knowledge.
///
/// Ordered: `Unique < MaybeShared < Shared`. Chain height: 2.
/// Uniqueness is a PAST guarantee ("not duplicated"), distinct from linearity
/// which is FUTURE ("consumed once") — Marshall et al., ESOP 2022.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Uniqueness {
    /// Provably RC == 1. COW fast path, reset/reuse candidate.
    Unique,
    /// Unknown RC. Runtime check needed for COW.
    MaybeShared,
    /// Provably RC > 1. COW always takes slow path.
    Shared,
}

impl Uniqueness {
    /// Componentwise join: max (most conservative).
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

// Locality dimension (auxiliary)

/// Escape analysis. `OxCaml` locality mode (ICFP 2024). Conservative in v1.
///
/// Ordered: `BlockLocal` < `FunctionLocal` < `HeapEscaping` < `Unknown`.
/// Chain height: 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Locality {
    /// Does not escape its defining basic block.
    BlockLocal,
    /// Does not escape its defining function.
    FunctionLocal,
    /// May escape to the heap.
    HeapEscaping,
    /// Unknown — conservative default.
    Unknown,
}

impl Locality {
    /// Componentwise join: max (most conservative).
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

// ShapeClass dimension (auxiliary)

/// Constructor kind for reuse size matching.
///
/// Only `Struct` and `EnumVariant` are reuse-eligible. Other `ir::CtorKind`
/// variants (`Tuple`, `ListLiteral`, `MapLiteral`, `SetLiteral`, `Closure`)
/// map to either `CollectionBuffer` or `NonReusable` in [`ShapeClass`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReuseCtorKind {
    Struct,
    EnumVariant,
}

/// Structural shape classification for reuse compatibility.
///
/// Forms a **flat lattice** with `NonReusable` as top: any two distinct
/// non-`NonReusable` values join to `NonReusable`.
///
/// Chain height: 1 (any value reaches `NonReusable` in at most one step).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeClass {
    /// Not a candidate for allocation reuse. Top element.
    NonReusable,
    /// A constructor allocation that may be reusable.
    ReusableCtor(ReuseCtorKind),
    /// A collection buffer (list, map, set).
    CollectionBuffer,
    /// A constructor-context hole (Stage 3 TRMC).
    ContextHole,
}

impl ShapeClass {
    /// Flat lattice join: equal values stay; unequal → `NonReusable`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::NonReusable
        }
    }
}

// EffectClass dimension (auxiliary)

/// Memory effect classification for FIP certification.
///
/// Independent boolean flags — NOT a total order. Join is componentwise OR.
///
/// Chain height: 3 (three independent booleans, each flips once).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectClass {
    /// May allocate heap memory (blocks FIP certification).
    pub may_alloc: bool,
    /// May share references (refcount > 1).
    pub may_share: bool,
    /// May throw exceptions/panics.
    pub may_throw: bool,
}

impl EffectClass {
    /// Bottom: no effects.
    pub const NONE: Self = Self {
        may_alloc: false,
        may_share: false,
        may_throw: false,
    };

    /// Top: all effects possible.
    pub const ALL: Self = Self {
        may_alloc: true,
        may_share: true,
        may_throw: true,
    };

    /// Componentwise OR (each flag independently conservative).
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self {
            may_alloc: self.may_alloc || other.may_alloc,
            may_share: self.may_share || other.may_share,
            may_throw: self.may_throw || other.may_throw,
        }
    }
}
