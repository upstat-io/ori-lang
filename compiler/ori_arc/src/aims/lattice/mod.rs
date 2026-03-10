//! Unified ownership lattice for ARC analysis.
//!
//! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
//! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
//! more dimensions simultaneously.
//!
//! **Core**: [`AccessClass`], [`Consumption`], [`Cardinality`], [`Uniqueness`]
//! **Auxiliary** (conservative in v1): [`Locality`], [`ShapeClass`], [`EffectClass`]
//!
//! Lattice properties: idempotent, commutative, associative join; monotonic
//! transfer; finite height 15. See tests for exhaustive verification.
//!
//! References: Perceus (PLDI 2021), GHC demand analysis (POPL 2014),
//! Lean 4 borrow inference (IFL 2019), Linearity ≠ Uniqueness (ESOP 2022),
//! `OxCaml` (ICFP 2024).

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests use unwrap for clearer failure messages"
)]
mod tests;

use crate::ir::ArcVarId;
use crate::ArcClass;

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

// AimsState — the product lattice

/// Unified ownership state for a variable at a program point.
///
/// Product of seven dimensions. Join is componentwise.
/// Core: access, consumption, cardinality, uniqueness.
/// Auxiliary (conservative in v1): locality, shape, effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AimsState {
    /// Aliasing: owned allocation vs borrowed view.
    pub access: AccessClass,
    /// Substructural: how the value is consumed.
    pub consumption: Consumption,
    /// Forward usage count.
    pub cardinality: Cardinality,
    /// Runtime reference count knowledge.
    pub uniqueness: Uniqueness,
    /// Escape analysis (auxiliary, conservative in v1).
    pub locality: Locality,
    /// Structural shape (auxiliary, conservative in v1).
    pub shape: ShapeClass,
    /// Memory effects (auxiliary, conservative in v1).
    pub effect: EffectClass,
}

impl AimsState {
    /// Most conservative state: analysis starts here for unknown variables.
    pub const TOP: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        uniqueness: Uniqueness::Shared,
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::ALL,
    };

    /// Most optimistic state: componentwise bottom.
    ///
    /// Note: `(*, Dead, Absent, *)` is redundant by design — componentwise
    /// bottom. `ShapeClass::NonReusable` is bottom because `ShapeClass` is a
    /// flat lattice where the value is set by the defining instruction's
    /// transfer function.
    pub const BOTTOM: Self = Self {
        access: AccessClass::Borrowed,
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        uniqueness: Uniqueness::Unique,
        locality: Locality::BlockLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    /// Scalar variable — excluded from analysis entirely.
    ///
    /// This is NOT a lattice element. It is a sentinel that short-circuits
    /// the analysis for non-RC types. Scalar variables never need RC
    /// operations, COW checks, or reuse.
    ///
    /// Uses an infeasible combination (`Unrestricted` + `Absent`) as a
    /// sentinel. This pair cannot survive [`canonicalize`](Self::canonicalize)
    /// (which forces `Absent` → `Dead`), so it uniquely identifies `SCALAR`
    /// among all post-canonicalization states.
    pub const SCALAR: Self = Self {
        access: AccessClass::Borrowed,
        consumption: Consumption::Unrestricted, // sentinel: infeasible with Absent
        cardinality: Cardinality::Absent,       // sentinel: infeasible with Unrestricted
        uniqueness: Uniqueness::Unique,
        locality: Locality::BlockLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    /// Convenience base for freshly constructed values.
    ///
    /// Transfer functions override `shape` based on the constructor kind.
    /// `FRESH` uses `NonReusable` as a default starting point.
    pub const FRESH: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    /// Returns `true` if this is the `SCALAR` sentinel (not a lattice element).
    ///
    /// Detects the infeasible `Unrestricted` + `Absent` combination that
    /// uniquely identifies `SCALAR`. This pair cannot survive
    /// [`canonicalize`](Self::canonicalize), so it never appears in valid
    /// lattice states.
    #[must_use]
    pub fn is_scalar(&self) -> bool {
        self.consumption == Consumption::Unrestricted && self.cardinality == Cardinality::Absent
    }

    /// Componentwise join, followed by canonicalization.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        let mut result = Self {
            access: self.access.join(other.access),
            consumption: self.consumption.join(other.consumption),
            cardinality: self.cardinality.join(other.cardinality),
            uniqueness: self.uniqueness.join(other.uniqueness),
            locality: self.locality.join(other.locality),
            shape: self.shape.join(other.shape),
            effect: self.effect.join(other.effect),
        };
        result.canonicalize();
        result
    }

    /// Enforce feasibility invariants.
    ///
    /// Called after every join and transfer function. Must be a pure
    /// function on `AimsState` — no control-flow position or instruction
    /// context.
    ///
    /// # Invariants enforced
    ///
    /// - `Dead` ↔ `Absent`: dead means zero future uses, and vice versa
    /// - `Linear` + `Absent` is infeasible → collapse to `Dead`
    /// - `Shared` + reusable shape → collapse shape to `NonReusable`
    pub fn canonicalize(&mut self) {
        // Dead ↔ Absent bidirectional sync
        if self.consumption == Consumption::Dead {
            self.cardinality = Cardinality::Absent;
        }
        if self.cardinality == Cardinality::Absent {
            self.consumption = Consumption::Dead;
        }

        // Linear + Absent is infeasible (linear requires at least one use).
        // Already handled by the two rules above: Absent forces Dead,
        // so Linear + Absent → Dead + Absent. Guard explicitly for safety.
        if self.consumption == Consumption::Linear && self.cardinality == Cardinality::Absent {
            self.consumption = Consumption::Dead;
        }

        // Shared values cannot be reused via constructor reset
        if self.uniqueness == Uniqueness::Shared
            && matches!(self.shape, ShapeClass::ReusableCtor(_))
        {
            self.shape = ShapeClass::NonReusable;
        }
    }

    /// Whether this variable needs RC operations.
    ///
    /// False for: dead variables, scalar variables, and borrowed views.
    /// Only owned, live variables carry RC obligations.
    #[must_use]
    pub fn is_rc_needed(&self) -> bool {
        self.access == AccessClass::Owned
            && self.consumption != Consumption::Dead
            && !self.is_scalar()
    }

    /// Whether this variable needs a COW uniqueness check at mutation points.
    ///
    /// Only `MaybeShared` values need runtime checks — `Unique` values
    /// take the fast path statically, `Shared` values take the slow path.
    #[must_use]
    pub fn needs_cow_check(&self) -> bool {
        self.uniqueness == Uniqueness::MaybeShared
    }

    /// Whether this variable is a reuse candidate.
    ///
    /// Requires: owned, reusable shape, and not definitely shared.
    /// - `Unique` → static reuse (direct `Reset`)
    /// - `MaybeShared` → dynamic reuse (`IsShared` check + conditional)
    /// - `Shared` → never a reuse candidate
    #[must_use]
    pub fn is_reuse_candidate(&self) -> bool {
        self.access == AccessClass::Owned
            && self.uniqueness != Uniqueness::Shared
            && !matches!(self.shape, ShapeClass::NonReusable)
    }

    /// Whether this variable is local (does not escape its defining function).
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(
            self.locality,
            Locality::BlockLocal | Locality::FunctionLocal
        )
    }

    /// Map from [`ArcClass`] to an initial `AimsState`.
    ///
    /// - `Scalar` → [`SCALAR`](Self::SCALAR) (excluded from analysis)
    /// - `DefiniteRef` → [`TOP`](Self::TOP) (conservative starting point)
    /// - `PossibleRef` → [`TOP`](Self::TOP) (conservative; post-mono
    ///   `PossibleRef` is a compiler bug but analysis must not crash)
    #[must_use]
    pub fn from_arc_class(arc_class: ArcClass) -> Self {
        match arc_class {
            ArcClass::Scalar => Self::SCALAR,
            ArcClass::DefiniteRef | ArcClass::PossibleRef => Self::TOP,
        }
    }

    /// Maximum chain height of the product lattice.
    ///
    /// The sum of per-dimension chain heights:
    /// - `AccessClass`: 1 (`Borrowed` → `Owned`)
    /// - `Consumption`: 3 (`Dead` → `Linear` → `Affine` → `Unrestricted`)
    /// - `Cardinality`: 2 (`Absent` → `Once` → `Many`)
    /// - `Uniqueness`: 2 (`Unique` → `MaybeShared` → `Shared`)
    /// - `Locality`: 3 (`BlockLocal` → `FunctionLocal` → `HeapEscaping` → `Unknown`)
    /// - `ShapeClass`: 1 (flat lattice — any value → `NonReusable`)
    /// - `EffectClass`: 3 (three independent booleans)
    ///
    /// Total: 15. Fixed-point iteration converges in at most
    /// `CHAIN_HEIGHT × num_variables × num_blocks` steps.
    pub const CHAIN_HEIGHT: usize = 15;

    /// Compute the non-convergence iteration limit for a function.
    ///
    /// If the worklist exceeds this many iterations, the lattice
    /// properties guarantee a bug exists in the transfer functions.
    /// The analysis should widen all non-converged variables to `TOP`
    /// and emit a `tracing::warn!`.
    #[must_use]
    pub fn iteration_limit(num_variables: usize, num_blocks: usize) -> usize {
        Self::CHAIN_HEIGHT
            .saturating_mul(num_variables)
            .saturating_mul(num_blocks)
    }
}

// Borrow provenance (sparse side table, not in the finite lattice)

/// Tracks where a borrowed value comes from.
///
/// Stored in a sparse side table keyed by [`ArcVarId`], NOT in the finite
/// lattice. Only relevant for variables with [`AccessClass::Borrowed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorrowSource {
    /// Known exact source variable.
    Exact(ArcVarId),
    /// Multiple sources or unknown origin.
    Unknown,
}

impl BorrowSource {
    /// Join two borrow sources: same → keep; different → `Unknown`.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) if a == b => Self::Exact(a),
            _ => Self::Unknown,
        }
    }
}
