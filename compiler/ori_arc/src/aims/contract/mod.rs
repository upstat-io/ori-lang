//! Memory contracts for interprocedural analysis.
//!
//! [`MemoryContract`] describes a function's memory behavior: how it uses
//! parameters, what it returns, and what effects it has. Produced by
//! interprocedural analysis (Section 03), consumed by intraprocedural
//! analysis (Section 02) at call sites and by emission (Section 04).
//!
//! # Stage 1
//!
//! In Stage 1:
//! - `EffectSummary` fields use conservative defaults
//! - `FipContract` is always `Never` (FIP inference disabled)
//! - `ContextBehavior` is always default (TRMC disabled)
//! - Core fields (`access`, `consumption`, `cardinality`, `uniqueness`)
//!   are refined by interprocedural analysis

#[cfg(test)]
mod tests;

use super::lattice::{AccessClass, Cardinality, Consumption, Locality, ShapeClass, Uniqueness};
use crate::ir::ArcParam;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::uniqueness::UniquenessSummary;

use ori_ir::Name;
use ori_types::Idx;

/// Memory contract for a function.
///
/// Describes parameter ownership, return value uniqueness, effect summary,
/// constructor-context behavior, and FIP certification. Produced by
/// interprocedural analysis (Section 03), consumed by intraprocedural
/// analysis (Section 02) at call sites and by emission (Section 04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryContract {
    /// Per-parameter contracts, in parameter order.
    pub params: Vec<ParamContract>,
    /// Return value information.
    pub return_info: ReturnContract,
    /// Effect summary: what memory effects may the function produce?
    pub effects: EffectSummary,
    /// Constructor-context behavior (Stage 3 TRMC).
    pub context_behavior: ContextBehavior,
    /// FIP certification status.
    pub fip: FipContract,
    /// Whether the function meets FBIP criteria (functional-but-in-place).
    ///
    /// Inferred metadata: `!effects.may_allocate` (no fresh allocations).
    /// This does NOT replace `#fbip` as the user-facing enforcement annotation,
    /// and does NOT change `is_auto_fbip()` behavior. It makes FBIP status
    /// visible to interprocedural analysis without running the post-pipeline check.
    /// (Section 09.2 Effect Activation.)
    pub is_fbip: bool,
}

impl MemoryContract {
    /// Most-optimistic contract for fixed-point initialization.
    ///
    /// All params start as borrowed (most optimistic for RC — fewer ops needed).
    /// The fixed-point promotes parameters toward `Owned` as call sites demand it.
    ///
    /// `fip_initial` controls FIP behavior:
    /// - Stage 1: pass `FipContract::Never` (FIP inference disabled)
    /// - Stage 2: pass `FipContract::Certified` (most optimistic, refined downward)
    pub fn all_borrowed(num_params: usize, fip_initial: FipContract) -> Self {
        Self {
            params: vec![ParamContract::OPTIMISTIC; num_params],
            return_info: ReturnContract::OPTIMISTIC,
            effects: EffectSummary::OPTIMISTIC,
            context_behavior: ContextBehavior::default(),
            fip: fip_initial,
            is_fbip: true, // optimistic: refined downward during fixpoint
        }
    }

    /// Conservative contract: all params owned/unrestricted, return `MaybeShared`.
    ///
    /// Used as fallback when no interprocedural information is available
    /// (FFI functions, external calls, unknown callees).
    pub fn conservative(num_params: usize) -> Self {
        Self {
            params: vec![ParamContract::CONSERVATIVE; num_params],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::CONSERVATIVE,
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        }
    }

    /// Componentwise join for convergence detection.
    ///
    /// Produces the least upper bound (most conservative) of two contracts.
    /// Used in SCC fixed-point iteration: if `join(old, new) == old`, the
    /// contract has converged.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        debug_assert_eq!(
            self.params.len(),
            other.params.len(),
            "cannot join contracts with different parameter counts"
        );
        Self {
            params: self
                .params
                .iter()
                .zip(other.params.iter())
                .map(|(a, b)| a.join(b))
                .collect(),
            return_info: self.return_info.join(&other.return_info),
            effects: self.effects.join(&other.effects),
            context_behavior: self.context_behavior.join(&other.context_behavior),
            fip: self.fip.join(&other.fip),
            // is_fbip: AND (conservative direction — if either side allocates,
            // the joined contract is not FBIP).
            is_fbip: self.is_fbip && other.is_fbip,
        }
    }

    /// Convert to [`AnnotatedSig`] for compatibility during migration.
    ///
    /// Requires the function's parameter definitions (for names and types)
    /// and return type, since [`MemoryContract`] doesn't carry these.
    ///
    /// Mapping:
    /// - `ParamContract.consumption == Dead` → `Ownership::Borrowed` (dead params need no RC)
    /// - `ParamContract.access == Borrowed` → `Ownership::Borrowed`
    /// - `ParamContract.access == Owned` → `Ownership::Owned`
    pub fn to_annotated_sig(&self, func_params: &[ArcParam], return_type: Idx) -> AnnotatedSig {
        debug_assert_eq!(
            self.params.len(),
            func_params.len(),
            "MemoryContract params must match function params"
        );
        let params = self
            .params
            .iter()
            .zip(func_params.iter())
            .map(|(contract, arc_param)| {
                let ownership = if contract.consumption == Consumption::Dead {
                    // Dead params need no RC operations — treat as borrowed.
                    Ownership::Borrowed
                } else {
                    match contract.access {
                        AccessClass::Borrowed => Ownership::Borrowed,
                        AccessClass::Owned => Ownership::Owned,
                    }
                };
                AnnotatedParam {
                    name: Name::from_raw(arc_param.var.raw()),
                    ty: arc_param.ty,
                    ownership,
                }
            })
            .collect();
        AnnotatedSig {
            params,
            return_type,
        }
    }

    /// Convert to [`UniquenessSummary`] for compatibility during migration.
    ///
    /// Per-parameter uniqueness is always `MaybeShared` (the current system
    /// doesn't track per-param uniqueness from callers).
    pub fn to_uniqueness_summary(&self) -> UniquenessSummary {
        let legacy_uniqueness = aims_to_legacy_uniqueness(self.return_info.uniqueness);
        UniquenessSummary {
            params: self
                .params
                .iter()
                .map(|_| crate::uniqueness::Uniqueness::MaybeShared)
                .collect(),
            return_val: legacy_uniqueness,
            preserves_freshness: self.return_info.preserves_freshness,
        }
    }
}

/// Per-parameter memory contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamContract {
    /// Whether the parameter is owned or borrowed.
    pub access: AccessClass,
    /// How the parameter is consumed.
    pub consumption: Consumption,
    /// How many times the parameter is used.
    pub cardinality: Cardinality,
    /// May this parameter's value escape the callee (stored, returned, shared)?
    pub may_escape: bool,
    /// May this parameter's value be shared (refcount > 1) by the callee?
    pub may_share: bool,
    /// Locality lower bound: the callee guarantees this parameter stays at
    /// least this local (v1: always `Unknown`).
    pub locality_bound: Locality,
}

impl ParamContract {
    /// Conservative: owned, unrestricted, many uses, may escape/share, unknown locality.
    pub const CONSERVATIVE: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        may_escape: true,
        may_share: true,
        locality_bound: Locality::Unknown,
    };

    /// Most-optimistic: borrowed, dead, absent, no escape/share, block-local.
    ///
    /// Used as starting point for fixed-point iteration. All dimensions
    /// are at their bottom (most optimistic) values.
    pub const OPTIMISTIC: Self = Self {
        access: AccessClass::Borrowed,
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        may_escape: false,
        may_share: false,
        locality_bound: Locality::BlockLocal,
    };

    /// Componentwise join toward conservative.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            access: self.access.join(other.access),
            consumption: self.consumption.join(other.consumption),
            cardinality: self.cardinality.join(other.cardinality),
            may_escape: self.may_escape || other.may_escape,
            may_share: self.may_share || other.may_share,
            locality_bound: self.locality_bound.join(other.locality_bound),
        }
    }
}

/// Return value information in a memory contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnContract {
    /// Uniqueness of the return value.
    pub uniqueness: Uniqueness,
    /// Whether the function preserves freshness: if all RC'd inputs are
    /// `Unique`, the output is guaranteed `Unique`.
    pub preserves_freshness: bool,
    /// Locality of the returned value (v1: `HeapEscaping` for most).
    pub locality: Locality,
    /// Shape class of the return value.
    pub shape: ShapeClass,
}

impl ReturnContract {
    /// Conservative: return value may be shared, no freshness preservation.
    pub const CONSERVATIVE: Self = Self {
        uniqueness: Uniqueness::MaybeShared,
        preserves_freshness: false,
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
    };

    /// Most-optimistic: unique return, freshness preserved.
    pub const OPTIMISTIC: Self = Self {
        uniqueness: Uniqueness::Unique,
        preserves_freshness: true,
        locality: Locality::BlockLocal,
        // Shape isn't monotonically ordered in a useful way for return values;
        // NonReusable is the safe starting point.
        shape: ShapeClass::NonReusable,
    };

    /// Componentwise join toward conservative.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            uniqueness: self.uniqueness.join(other.uniqueness),
            preserves_freshness: self.preserves_freshness && other.preserves_freshness,
            locality: self.locality.join(other.locality),
            shape: self.shape.join(other.shape),
        }
    }
}

/// Effect summary: what memory effects may the function produce?
///
/// Distinct from [`super::lattice::EffectClass`], which is a per-variable,
/// per-program-point lattice dimension. `EffectSummary` is a per-function
/// summary aggregated across the entire function body.
///
/// # Planned: `may_deallocate` field (Stage 2)
///
/// FP² Theorem 2 requires both sides of the in-place balance: `may_allocate`
/// alone gives FBIP (functional-but-in-place). Full FIP additionally requires
/// `may_deallocate == false` — no unmatched deallocations. This is a post-emission
/// fact computed from `EmitReuseResult.missed_reuses > 0`: if any consumed value
/// with reusable shape was NOT matched by a reuse opportunity, the function
/// deallocates (frees memory without reusing it). When `may_allocate == false &&
/// may_deallocate == false`, the function is fully in-place (FIP).
/// (See: plans/aims-literature-review/section-02-fp2.md)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "4 independent effect flags from FP² paper; enum would add complexity without benefit"
)]
pub struct EffectSummary {
    /// May the function allocate on any code path?
    pub may_allocate: bool,
    /// Allocations are only on slow paths guarded by uniqueness checks.
    ///
    /// When `may_allocate && alloc_only_on_slow_path`, the function is
    /// FIP-eligible with Conditional preconditions.
    pub alloc_only_on_slow_path: bool,
    /// May the function create shared references?
    pub may_share: bool,
    /// May the function throw exceptions/panics?
    pub may_throw: bool,
}

impl EffectSummary {
    /// Conservative: may allocate, share, and throw.
    pub const CONSERVATIVE: Self = Self {
        may_allocate: true,
        alloc_only_on_slow_path: false,
        may_share: true,
        may_throw: true,
    };

    /// Most-optimistic: no effects.
    pub const OPTIMISTIC: Self = Self {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_share: false,
        may_throw: false,
    };

    /// Componentwise join (OR for effect flags, AND for slow-path-only).
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            may_allocate: self.may_allocate || other.may_allocate,
            // Both sides must be slow-path-only for the join to be slow-path-only.
            alloc_only_on_slow_path: self.alloc_only_on_slow_path && other.alloc_only_on_slow_path,
            may_share: self.may_share || other.may_share,
            may_throw: self.may_throw || other.may_throw,
        }
    }
}

/// Constructor-context behavior for TRMC (Stage 3).
///
/// Describes whether a function preserves/consumes constructor contexts.
/// Default (conservative) in Stage 1.
///
/// **Soundness gate (Section 09.2):** In-place TRMC requires
/// `EffectSummary.may_share == false`. When `may_share == true`, the context
/// variable `k` may be captured by an effect handler's resumption and used
/// non-linearly, breaking the unique linear chain invariant. Stage 3
/// `normalize/verify.rs` must gate in-place TRMC behind this check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ContextBehavior {
    /// Does this function preserve a constructor context passed to it?
    pub preserves_context: bool,
    /// Does this function consume a context hole?
    pub consumes_hole: bool,
}

impl ContextBehavior {
    /// Componentwise join: AND (conservative direction for Stage 3).
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            preserves_context: self.preserves_context && other.preserves_context,
            consumes_hole: self.consumes_hole && other.consumes_hole,
        }
    }
}

/// FIP certification status.
///
/// Based on FP² (Lorenzen et al., ICFP 2023): a function is FIP when
/// it can run with no allocation, no deallocation, and constant stack
/// space, provided arguments are unique.
///
/// Ordering: `Certified < Bounded(n) < Conditional < Never`
/// (Certified is most optimistic).
///
/// Section 09.2 Effect Activation: FIP classification is now inferred
/// from the converged effect state, not from a separate certification pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FipContract {
    /// Function cannot be certified FIP.
    Never,
    /// Function is FIP when the specified parameters are unique.
    ///
    /// The `Vec<bool>` is indexed by parameter position in
    /// `MemoryContract.params`. `true` = this parameter must be unique.
    Conditional {
        /// Which parameters must be unique for FIP certification.
        requires_unique_params: Vec<bool>,
    },
    /// Function is unconditionally FIP (all code paths allocation-free).
    Certified,
    /// Function allocates at most `n` constructors beyond what it reuses.
    ///
    /// `FIPTree`'s `fip(n)` pattern — e.g., tree insertion allocates exactly
    /// one node (`Bounded(1)`). Compiler-inferred from allocation balance
    /// tracking (`allocs - reuses = n`), not a user annotation.
    /// (See: plans/aims-literature-review/section-03-fiptree.md)
    Bounded(u16),
}

impl FipContract {
    /// Componentwise join: weakens toward `Never`.
    ///
    /// Ordering: `Certified < Bounded(n) < Conditional < Never`.
    /// For `Bounded`, takes the max (more allocations = weaker).
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,

            // Conditional + Conditional: union of required-unique params.
            (
                Self::Conditional {
                    requires_unique_params: a,
                },
                Self::Conditional {
                    requires_unique_params: b,
                },
            ) => {
                debug_assert_eq!(a.len(), b.len(), "FipContract param counts must match");
                Self::Conditional {
                    requires_unique_params: a.iter().zip(b.iter()).map(|(x, y)| *x || *y).collect(),
                }
            }

            // Conditional absorbs Bounded and Certified.
            // Bounded absorbs Certified. Both: weaker side wins (self.clone()/other.clone()).
            (Self::Conditional { .. }, Self::Certified | Self::Bounded(_))
            | (Self::Bounded(_), Self::Certified) => self.clone(),
            (Self::Certified | Self::Bounded(_), Self::Conditional { .. })
            | (Self::Certified, Self::Bounded(_)) => other.clone(),

            // Bounded + Bounded: take the max allocation count.
            (Self::Bounded(a), Self::Bounded(b)) => Self::Bounded((*a).max(*b)),

            (Self::Certified, Self::Certified) => Self::Certified,
        }
    }
}

/// TRMC constructor-context region metadata (Stage 3).
///
/// Describes a region of code where a self-recursive function builds
/// a constructor in tail context. Empty in Stage 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRegion {
    /// Placeholder — populated in Stage 3.
    _private: (),
}

// Conversion helpers

/// Convert AIMS [`Uniqueness`] to the legacy [`crate::uniqueness::Uniqueness`].
///
/// Both enums have identical variants. This bridge exists because the AIMS
/// lattice defines its own `Uniqueness` (with `PartialOrd`/`Ord`) while the
/// legacy uniqueness analysis has a separate type.
fn aims_to_legacy_uniqueness(u: Uniqueness) -> crate::uniqueness::Uniqueness {
    match u {
        Uniqueness::Unique => crate::uniqueness::Uniqueness::Unique,
        Uniqueness::MaybeShared => crate::uniqueness::Uniqueness::MaybeShared,
        Uniqueness::Shared => crate::uniqueness::Uniqueness::Shared,
    }
}
