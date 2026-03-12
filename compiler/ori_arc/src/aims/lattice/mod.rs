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

// Exposed for dead-code lint satisfaction — the glob re-export below
// is the intended public API surface.
pub mod dimensions;
#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests use unwrap for clearer failure messages"
)]
mod tests;

pub use dimensions::*;

use crate::ir::ArcVarId;
use crate::ArcClass;

// SizeClass (for cross-type reuse in Stage 2+)

/// Allocation size class for reuse compatibility.
///
/// Two allocations are reuse-compatible when they have the same `SizeClass`.
/// In Stage 1 (same-type matching), size compatibility is implied by type
/// equality. In Stage 2+, `SizeClass` enables cross-type reuse when
/// allocations have the same rounded size.
///
/// Derived from Pool type size queries, rounded to allocation granularity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SizeClass(u32);

impl SizeClass {
    /// Unknown size — used when Pool-based size queries are unavailable
    /// or when size classification is deferred (Stage 1).
    pub const UNKNOWN: Self = Self(0);

    /// Create a size class from a byte count.
    #[must_use]
    pub fn from_bytes(bytes: u32) -> Self {
        Self(bytes)
    }

    /// The byte count for this size class.
    #[must_use]
    pub fn bytes(self) -> u32 {
        self.0
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
    /// Known exact source variable, optionally with the projected field index.
    ///
    /// `field` is `Some(idx)` when the borrow comes from a `Project` instruction,
    /// identifying which field of the source struct/enum was extracted. Used by
    /// the disjoint-field COW optimization (Section 07.3.2) to prove that a COW
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

    /// Get the source variable, if this is an `Exact` borrow.
    #[must_use]
    pub fn source_var(&self) -> Option<ArcVarId> {
        match self {
            Self::Exact { source, .. } => Some(*source),
            Self::Unknown => None,
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
