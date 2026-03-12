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
///
/// ## Algebraic structure (QTT correspondence)
///
/// `(Cardinality, seq_add, Absent)` forms a commutative monoid analogous to
/// QTT's 0-1-ω semiring (Atkey, LICS 2018):
/// - [`seq_add`](Self::seq_add) = QTT's `+` (resource accumulation along one execution path)
/// - [`alt_join`](Self::alt_join) = QTT's lub (branch join from mutually exclusive paths)
/// - `seq_add` distributes over `alt_join` (verified exhaustively in tests)
///
/// GHC demand analysis uses three composition operations: `lubCard`, `plusCard`,
/// `multCard`. AIMS needs only two: `alt_join` (= `lubCard`) and `seq_add`
/// (= `plusCard`). The third, `multCard` (demand scaling), models nested
/// evaluation contexts in lazy languages — a lambda called zero times zeros out
/// inner demands, called many times multiplies them. In Ori's strict evaluation,
/// every function body executes exactly once per call, so `multCard` is
/// unnecessary. `seq_add` subsumes the sequential composition role that GHC
/// splits between `plusCard` and `multCard`.
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
    /// Alternative control-flow join: lattice lub (idempotent).
    ///
    /// Used at control-flow merge points where only one path executes.
    /// `join(Once, Once) = Once` — a value used once in each branch of
    /// an `if` is still used once per execution.
    ///
    /// This is the lattice lub, NOT semiring addition — it is idempotent
    /// (`a.alt_join(a) = a`), unlike [`seq_add`](Self::seq_add) which is
    /// additive (`Once.seq_add(Once) = Many`).
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }

    /// Alias for [`join`](Self::join) — alternative control-flow join (lattice lub).
    #[must_use]
    pub fn alt_join(self, other: Self) -> Self {
        self.join(other)
    }

    /// Sequential composition along one execution path (QTT's `+`, GHC's `plusCard`).
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
///
/// ## Sequencing algebra
///
/// Both `seq_add` (sequential composition) and `alt_join` (branch join)
/// coincide with [`join`](Self::join) (= max) for `Locality`. This is
/// intentional, not accidental: locality tracks where a value *escapes to*,
/// which widens monotonically. A value that escapes to the heap in one
/// instruction stays heap-escaping regardless of subsequent instructions
/// (`seq_add` = max). A value that escapes in either branch of a conditional
/// escapes overall (`alt_join` = max). No separate methods are needed;
/// use `join` for both operations.
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
///
/// ## Sequencing algebra
///
/// Both `seq_add` (sequential composition) and `alt_join` (branch join)
/// coincide with [`join`](Self::join) (= componentwise OR) for `EffectClass`.
/// This is a design choice: effects are boolean (has/hasn't), not counted
/// (how many). Sequential effects accumulate via OR
/// (`NONE.seq_add(MayAlloc) = MayAlloc`); branch effects join via OR
/// (`MayAlloc.alt_join(MayThrow) = MayAlloc+MayThrow`). If effect *counts*
/// were tracked (e.g., number of allocations), `seq_add` would be addition
/// while `alt_join` would be max — but boolean flags make both operations
/// identical. No separate methods are needed; use `join` for both operations.
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
