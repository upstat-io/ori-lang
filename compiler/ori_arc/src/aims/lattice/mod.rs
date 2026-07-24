//! Unified ownership lattice for ARC analysis.
//!
//! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
//! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
//! more dimensions simultaneously.
//!
//! **Core**: [`AccessClass`], [`Consumption`], [`Cardinality`], [`Uniqueness`]
//! **Active auxiliary**: [`Locality`] (precise),
//!   [`EffectClass`] (precise — Effect Activation),
//!   [`ShapeClass`] (precise — Shape Activation: per-variable
//!   shape map, cross-dimensional reuse/COW, TRMC `ContextHole` detection)
//!
//! Lattice properties: commutative, associative, idempotent join; monotonic
//! transfer; finite height 15. See `prop_tests.rs` for property-based
//! verification across all 7 dimensions.
//!
//! References: Perceus (PLDI 2021), GHC demand analysis (POPL 2014),
//! Lean 4 borrow inference (IFL 2019), Linearity ≠ Uniqueness (ESOP 2022),
//! `OxCaml` (ICFP 2024).

mod borrow_source;
pub mod dimensions;
pub mod prelude;
#[cfg(test)]
mod prop_tests;
#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests use unwrap for clearer failure messages"
)]
mod tests;

pub use prelude::*;

#[cfg(test)]
use crate::ir::ArcVarId;
use crate::ArcClass;

// Canonicalize feedback (Convergence Feedback)

/// Feedback from multi-round canonicalize.
///
/// Reports how many rounds of [`AimsState::canonicalize_single_pass`]
/// actually changed the state. With current rules, at most
/// one round makes changes. If `rounds > 1`, a cross-dimension chain fired
/// (one rule's output enabled another rule to fire in a subsequent pass).
/// The multi-round loop is bounded at 3 rounds — sufficient for any chain
/// of length ≤3 in the product lattice.
///
/// - `rounds == 0`: state was already canonical (no rules fired)
/// - `rounds == 1`: some rules fired, fixed point reached in one pass
/// - `rounds > 1`: cross-dimension chain detected
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanonicalizeFeedback {
    /// Number of rounds that changed the state (0 = already canonical).
    pub rounds: u8,
    /// Cross-dimension rule firings (Rules 4-8, total across all rounds).
    ///
    /// Each firing represents a state change that required reasoning across
    /// 2+ lattice dimensions. Used by `SynergyMetrics`.
    pub cross_dim_fires: u16,
}

impl CanonicalizeFeedback {
    /// Whether any cross-dimension chain fired (more than one changing round).
    #[must_use]
    pub fn cross_dimension_fired(self) -> bool {
        self.rounds > 1
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
    /// Aliasing: owned logical identity vs borrowed view.
    pub access: AccessClass,
    /// Substructural: how the value is consumed.
    pub consumption: Consumption,
    /// Forward usage count.
    pub cardinality: Cardinality,
    /// Logical owner multiplicity, independent of physical counting strategy.
    pub uniqueness: Uniqueness,
    /// Escape analysis (auxiliary, conservative in v1).
    pub locality: Locality,
    /// Structural shape (auxiliary, conservative in v1).
    pub shape: ShapeClass,
    /// Memory effects (active — Effect Activation).
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
    /// bottom. `ShapeClass::NonReusable` is top (the absorbing element of the
    /// flat `ShapeClass` join: any two distinct shapes join to `NonReusable`).
    /// It appears in both `TOP` and `BOTTOM` because the flat lattice has no
    /// separate ⊥ element.
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
    /// the analysis for types without managed ownership. Scalar variables
    /// require no ownership events, sharing observations, or reuse facts.
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
    ///
    /// Locality starts at `BlockLocal`: a fresh allocation hasn't escaped
    /// its defining block. Cross-block flow widens to `FunctionLocal`;
    /// return or heap storage widens to `HeapEscaping`. This is the
    /// precise locality computation.
    pub const FRESH: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::BlockLocal,
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

    /// Enforce feasibility invariants with multi-round convergence.
    ///
    /// Called after every join and transfer function. Runs
    /// [`canonicalize_single_pass`](Self::canonicalize_single_pass) in a
    /// bounded loop (up to 3 rounds) until no further changes occur. This
    /// catches chain reasoning where one rule's output enables another rule
    /// (e.g., locality→uniqueness→shape). With current rules,
    /// one pass always suffices; the multi-round loop is defensive
    /// infrastructure for future cross-dimension rules.
    ///
    /// See [`canonicalize_single_pass`](Self::canonicalize_single_pass) for
    /// the invariants enforced.
    pub fn canonicalize(&mut self) {
        let feedback = self.canonicalize_with_feedback();
        if feedback.cross_dimension_fired() {
            tracing::warn!(
                rounds = feedback.rounds,
                state = ?self,
                "canonicalize required multiple rounds — cross-dimension chain detected"
            );
        }
    }

    /// Like [`canonicalize`](Self::canonicalize) but returns feedback about
    /// the convergence process.
    #[must_use]
    pub(crate) fn canonicalize_with_feedback(&mut self) -> CanonicalizeFeedback {
        const MAX_ROUNDS: u8 = 3;
        let mut rounds: u8 = 0;
        let mut total_cross_fires: u16 = 0;

        loop {
            let before = *self;
            let fires = self.canonicalize_single_pass();
            total_cross_fires = total_cross_fires.saturating_add(fires);

            if *self == before {
                break; // Fixed point — no changes in this pass.
            }

            rounds += 1;
            if rounds >= MAX_ROUNDS {
                break; // Bound reached — conservatively correct.
            }
        }

        CanonicalizeFeedback {
            rounds,
            cross_dim_fires: total_cross_fires,
        }
    }

    /// Single pass of feasibility invariant enforcement.
    ///
    /// Must be a pure function on `AimsState` — no control-flow position
    /// or instruction context.
    ///
    /// # Invariants enforced
    ///
    /// 1. `Dead` ↔ `Absent`: dead means zero future uses, and vice versa
    /// 2. `Linear` + `Absent` is infeasible → collapse to `Dead`
    /// 3. `Shared` + reusable shape → collapse shape to `NonReusable`
    /// 4. `MaybeShared` is never auto-promoted to `Unique` (auto-promotion
    ///    would break join associativity)
    /// 5. `Unique` + `Dead` → preserve `ReusableCtor` shape (implicit — no
    ///    rule collapses shape here; documented for clarity)
    /// 6. `HeapEscaping` or higher locality → uniqueness ceiling `MaybeShared`
    /// 7. `Shared` + `CollectionBuffer` → force Dynamic COW
    ///    (enforced at query sites via `needs_cow_check()`)
    /// 8. `Borrowed` → locality ceiling `FunctionLocal`
    ///
    /// # Ordering and termination
    ///
    /// All active rules are monotone within the product lattice (they only
    /// move dimensions toward top / more conservative, or enforce same-level
    /// consistency). Rule 8 forces locality down (away from `HeapEscaping`),
    /// preventing Rule 6 from firing on Borrowed states. Chain height is
    /// bounded by the product of dimension heights. With current rules, one
    /// pass suffices (no rule creates a precondition for another rule to
    /// re-fire). The multi-round loop in [`canonicalize`](Self::canonicalize)
    /// is defensive infrastructure for future cross-dimension rules.
    /// Returns the number of cross-dimension rule firings (Rules 4-8).
    fn canonicalize_single_pass(&mut self) -> u16 {
        let mut cross_fires: u16 = 0;

        // Rule 1: Dead ↔ Absent bidirectional sync
        if self.consumption == Consumption::Dead {
            self.cardinality = Cardinality::Absent;
        }
        if self.cardinality == Cardinality::Absent {
            self.consumption = Consumption::Dead;
        }

        // Rule 2: Linear + Absent is infeasible (linear requires at least one use).
        // Already handled by the two rules above: Absent forces Dead,
        // so Linear + Absent → Dead + Absent. Guard explicitly for safety.
        if self.consumption == Consumption::Linear && self.cardinality == Cardinality::Absent {
            self.consumption = Consumption::Dead;
        }

        // CN-3 forbids resetting every reusable shape when multiple owners may
        // observe the allocation; comparing with `NonReusable` covers future
        // reusable variants as well.
        if self.uniqueness == Uniqueness::Shared && self.shape != ShapeClass::NonReusable {
            self.shape = ShapeClass::NonReusable;
        }

        // Rule 8 makes borrowed values function-local before later rules inspect
        // locality; a temporary view cannot escape its defining function.
        if self.access == AccessClass::Borrowed && self.locality > Locality::FunctionLocal {
            self.locality = Locality::FunctionLocal;
            cross_fires += 1; // Rule 8: Access → Locality (2 dimensions)
        }

        // Rule 6 weakens heap-reachable or unknown owned values from Unique to
        // MaybeShared. Rule 8 has already removed borrowed values from this
        // wide-locality state.
        if self.locality >= Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique {
            self.uniqueness = Uniqueness::MaybeShared;
            cross_fires += 1; // Rule 6: Locality → Uniqueness (2 dimensions)
        }

        // Rule 4 never reconstructs uniqueness: transfer establishes it and
        // joins may only preserve or lose it. Promotion during canonicalization
        // would be anti-monotone and break join associativity.

        // Rule 5 implicitly preserves reusable shape for unique dead values;
        // only Rule 3 collapses shape, and only for shared values.

        cross_fires
    }

    /// Whether this variable requires logical ownership events.
    ///
    /// False for: dead variables, scalar variables, and borrowed views.
    /// Only owned, live variables carry owner-credit/release obligations.
    #[must_use]
    pub fn needs_ownership_events(&self) -> bool {
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

    /// Whether this variable is eligible to elide a balanced ownership-event
    /// pair.
    ///
    /// A function-local value that is owned and consumed linearly has one
    /// logical ownership credit and a function-bounded lifetime. Its entry
    /// credit and last-use discharge form one balanced pair, so both logical
    /// events can be eliminated without selecting any counter mechanism.
    ///
    /// Requires precise locality. `Unknown` locality never
    /// qualifies.
    #[must_use]
    pub fn is_event_pair_elision_eligible(&self) -> bool {
        // DP-7: Uniqueness = Unique is load-bearing — a Shared value's
        // +1 inc from the caller is never balanced without it → leak.
        self.is_local()
            && self.access == AccessClass::Owned
            && self.consumption == Consumption::Linear
            && self.uniqueness == Uniqueness::Unique
            && !self.is_scalar()
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
    pub(crate) fn iteration_limit(num_variables: usize, num_blocks: usize) -> usize {
        Self::CHAIN_HEIGHT
            .saturating_mul(num_variables)
            .saturating_mul(num_blocks)
    }
}
