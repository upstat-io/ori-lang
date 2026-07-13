//! Memory contracts for interprocedural analysis.
//!
//! [`MemoryContract`] describes a function's memory behavior: how it uses
//! parameters, what it returns, and what effects it has. Produced by the
//! interprocedural fixpoint, consumed by intraprocedural analysis at call
//! sites and by AIMS realization emission.
//!
//! # Current State
//!
//! All contract fields are active and refined by interprocedural
//! analysis:
//! - Core fields (`access`, `consumption`, `cardinality`, `uniqueness`)
//!   are refined by SCC fixed-point iteration
//! - `EffectSummary` fields are computed from function body instructions
//! - `FipContract` is inferred from converged effect state and token
//!   balance (`extract_contract` in `interprocedural/extract.rs`)
//! - `ContextBehavior` is computed from `ContextRegion` metadata during
//!   contract extraction; `default` for non-TRMC functions
//! - `is_fbip` is `!effects.may_allocate` (inferred metadata)

mod context;
mod param;
#[cfg(test)]
mod tests;

pub use context::{ContextBehavior, ContextRegion};
pub use param::{ParamContract, ReturnAliasShape};

use super::lattice::{AccessClass, Consumption, Locality, ShapeClass, Uniqueness};
use crate::ir::ArcParam;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_ir::Name;
use ori_types::Idx;

/// Memory contract for a function.
///
/// Describes parameter ownership, return value uniqueness, effect summary,
/// constructor-context behavior, and FIP certification. Produced by the
/// interprocedural fixpoint, consumed by intraprocedural analysis at call
/// sites and by AIMS realization emission.
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
    /// and does NOT change `is_auto_fbip` behavior. It makes FBIP status
    /// visible to interprocedural analysis without running the post-pipeline check.
    pub is_fbip: bool,
}

impl MemoryContract {
    /// Most-optimistic contract for fixed-point initialization.
    ///
    /// All params start as borrowed (most optimistic for RC — fewer ops needed).
    /// The fixed-point promotes parameters toward `Owned` as call sites demand it.
    ///
    /// `fip_initial` controls FIP behavior:
    /// - Pass `FipContract::Never` to disable FIP inference
    /// - Pass `FipContract::Certified` for optimistic start (refined downward)
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
        assert_eq!(
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
        assert_eq!(
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
    /// Whether the function returns a FRESH self-allocated rc=1 buffer on every
    /// path — a buffer allocated INSIDE this function (a `Construct`/`Reuse`/
    /// `CollectionReuse` collection, or the `for_yield` `@ori_list_take`
    /// finalizer that moves a fresh scratch buffer out), distinct from any
    /// caller-visible value. Stronger than `preserves_freshness ∧ uniqueness`:
    /// it certifies the returned allocation is a fresh self-alloc the caller
    /// receives at rc=1 (no upstream alias), which licenses caller-side analyses
    /// to admit the Invoke/Apply result as a fresh collection root. Orthogonal to
    /// `uniqueness`/`preserves_freshness` — set independently, consumed only by
    /// the fresh-collection-root admission, so it never perturbs the
    /// uniqueness/freshness-driven store-dup accounting.
    /// Spec: Annex E §AIMS RL-1 + RL-29 (fresh + Unique non-aliasing).
    pub returns_fresh_self_alloc: bool,
    /// Whether the return value is a SHARING VIEW of a borrowed input's
    /// backing buffer on every path — a seamless-slice co-reference
    /// (`slice`/`substring`/`take`/`drop`) whose runtime self-inc mints the
    /// view's own `+1` on the SHARED allocation. The typed buffer-provenance
    /// CREDIT fact: the call boundary classifies the result as a CREDIT on
    /// the source's allocation class (on top of the borrow-read of the
    /// source), never a fresh birth. Orthogonal to `uniqueness` /
    /// `returns_fresh_self_alloc` (a view is NEVER a fresh self-alloc);
    /// carried for the provenance-ledger emitter, consumed by no emission
    /// path here. Spec: Annex E §AIMS §12 (sharing-view producer = CREDIT).
    pub returns_sharing_view: bool,
}

impl ReturnContract {
    /// Conservative: return value may be shared, no freshness preservation.
    pub const CONSERVATIVE: Self = Self {
        uniqueness: Uniqueness::MaybeShared,
        preserves_freshness: false,
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
        returns_fresh_self_alloc: false,
        returns_sharing_view: false,
    };

    /// Most-optimistic: unique return, freshness preserved.
    pub const OPTIMISTIC: Self = Self {
        uniqueness: Uniqueness::Unique,
        preserves_freshness: true,
        locality: Locality::BlockLocal,
        // Shape isn't monotonically ordered in a useful way for return values;
        // NonReusable is the safe starting point.
        shape: ShapeClass::NonReusable,
        // Optimistic-init: assume fresh self-alloc so the monotone SCC fixpoint
        // join (AND) only ever CLEARS it. The extractor recomputes the true
        // value from the body each pass; `true ∧ extractor` lets a consistently
        // fresh-self-alloc return survive, `true ∧ false` clears a non-fresh one.
        returns_fresh_self_alloc: true,
        // Same monotone AND-join shape as `returns_fresh_self_alloc`: the
        // extractor's per-pass value is authoritative; `true ∧ false` clears.
        returns_sharing_view: true,
    };

    /// Componentwise join toward conservative.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            uniqueness: self.uniqueness.join(other.uniqueness),
            preserves_freshness: self.preserves_freshness && other.preserves_freshness,
            locality: self.locality.join(other.locality),
            shape: self.shape.join(other.shape),
            returns_fresh_self_alloc: self.returns_fresh_self_alloc
                && other.returns_fresh_self_alloc,
            returns_sharing_view: self.returns_sharing_view && other.returns_sharing_view,
        }
    }
}

/// Effect summary: what memory effects may the function produce?
///
/// Distinct from [`super::lattice::EffectClass`], which is a per-variable,
/// per-program-point lattice dimension. `EffectSummary` is a per-function
/// summary aggregated across the entire function body.
///
/// FP² Theorem 2 (Lorenzen et al., ICFP 2023) requires:
/// - `may_allocate == false` → FBIP (no fresh allocations)
/// - `may_allocate == false && may_deallocate == false` → fully in-place (FIP)
///
/// `may_deallocate` is a post-emission fact: computed from
/// `FipEvidence.missed_reuses > 0` after realization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "6 independent effect flags from FP² paper (may_allocate, alloc_only_on_slow_path, \
              may_deallocate, may_share, may_throw, has_unbounded_stack); \
              enum would add complexity without benefit"
)]
pub struct EffectSummary {
    /// May the function allocate on any code path?
    pub may_allocate: bool,
    /// Allocations are only on slow paths guarded by uniqueness checks.
    ///
    /// When `may_allocate && alloc_only_on_slow_path`, the function is
    /// FIP-eligible with Conditional preconditions.
    pub alloc_only_on_slow_path: bool,
    /// May the function deallocate on any code path?
    ///
    /// `true` if any consumed value with reusable shape was NOT matched
    /// by a reuse opportunity — the function frees memory without reusing
    /// it. Computed post-emission from `FipEvidence.missed_reuses > 0`.
    ///
    /// When `may_allocate == false && may_deallocate == false`, the
    /// function is fully in-place (FIP per FP² Theorem 2).
    pub may_deallocate: bool,
    /// May the function create shared references?
    pub may_share: bool,
    /// May the function throw exceptions/panics?
    pub may_throw: bool,
    /// Does this function have unbounded stack growth?
    ///
    /// `true` if the function contains non-tail-recursive calls to itself
    /// or to mutual-recursion partners. Functions where all recursive calls
    /// are in tail position (rewritten to loops by the tail-call pass) are
    /// considered constant-stack.
    ///
    /// Unlike `may_allocate`/`may_share`/`may_throw`, this is NOT accumulated
    /// per-block during analysis. It is set once in `extract_contract` from
    /// SCC membership and syntactic tail-position checks.
    pub has_unbounded_stack: bool,
}

impl EffectSummary {
    /// Conservative: may allocate, deallocate, share, throw, and unbounded stack.
    pub const CONSERVATIVE: Self = Self {
        may_allocate: true,
        alloc_only_on_slow_path: false,
        may_deallocate: true,
        may_share: true,
        may_throw: true,
        has_unbounded_stack: true,
    };

    /// Most-optimistic: no effects.
    pub const OPTIMISTIC: Self = Self {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_deallocate: false,
        may_share: false,
        may_throw: false,
        has_unbounded_stack: false,
    };

    /// Componentwise join (OR for effect flags, AND for slow-path-only).
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self {
            may_allocate: self.may_allocate || other.may_allocate,
            // Both sides must be slow-path-only for the join to be slow-path-only.
            alloc_only_on_slow_path: self.alloc_only_on_slow_path && other.alloc_only_on_slow_path,
            may_deallocate: self.may_deallocate || other.may_deallocate,
            may_share: self.may_share || other.may_share,
            may_throw: self.may_throw || other.may_throw,
            // Either side unbounded → joined is unbounded.
            has_unbounded_stack: self.has_unbounded_stack || other.has_unbounded_stack,
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
/// FIP classification is inferred from the converged effect state, not
/// from a separate certification pass.
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
    /// Function is unconditionally FIP (all allocations matched by reuses,
    /// or no allocations at all). `may_allocate` may be `true` for
    /// token-balanced functions; FBIP (allocation-free) is tracked separately
    /// by `MemoryContract::is_fbip`.
    Certified,
    /// Function allocates at most `n` constructors beyond what it reuses.
    ///
    /// `FIPTree`'s `fip(n)` pattern — e.g., tree insertion allocates exactly
    /// one node (`Bounded(1)`). Compiler-inferred from allocation balance
    /// tracking (`allocs - reuses = n`), not a user annotation.
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
                assert_eq!(a.len(), b.len(), "FipContract param counts must match");
                Self::Conditional {
                    requires_unique_params: a.iter().zip(b.iter()).map(|(x, y)| *x || *y).collect(),
                }
            }

            // Conditional absorbs Bounded and Certified.
            // Bounded absorbs Certified. Both: weaker side wins (self.clone/other.clone).
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

/// Extension trait asserting the AIMS Invariant IC-1 required-lookup contract
/// on the interprocedural contract map (`HashMap<Name, MemoryContract, S>` for
/// any `S: BuildHasher`).
///
/// IC-1 mandates that every `MemoryContract` queried by intraprocedural / RC /
/// realization passes MUST exist — the SCC fixpoint (PL-1a) orders callees
/// before callers, so by the time any caller queries a callee's contract, the
/// callee has been fully analyzed and inserted into the map. A missing entry
/// is an internal pipeline-ordering invariant violation, never a recoverable
/// runtime condition.
///
/// A silent fallback to `MemoryContract::default` would produce unsound
/// results: the optimistic default (all-`Borrowed` / `Dead` / `Absent`
/// params, no effects) inflates safety claims through every downstream
/// `refine` call (TF-6) and `EffectSummary` join (IC-5), producing
/// miscompilation rather than a clean panic.
pub trait ContractMapExt {
    /// Look up the contract for `name`, panicking with an attributed
    /// IC-1-violation message when absent. `site` identifies the lookup site
    /// (callee module + function) so the panic message points at the pipeline
    /// edge that broke the invariant.
    fn get_required(&self, name: &Name, site: &'static str) -> &MemoryContract;

    /// Mutable variant of [`get_required`](Self::get_required).
    fn get_mut_required(&mut self, name: &Name, site: &'static str) -> &mut MemoryContract;
}

impl<S: BuildHasher> ContractMapExt for HashMap<Name, MemoryContract, S> {
    #[inline]
    fn get_required(&self, name: &Name, site: &'static str) -> &MemoryContract {
        self.get(name).unwrap_or_else(|| {
            unreachable!(
                "AIMS Invariant IC-1 violation at {site}: contract for {name:?} not in map. \
                 SCC ordering (PL-1a) should guarantee callee analyzed before caller."
            )
        })
    }

    #[inline]
    fn get_mut_required(&mut self, name: &Name, site: &'static str) -> &mut MemoryContract {
        self.get_mut(name).unwrap_or_else(|| {
            unreachable!(
                "AIMS Invariant IC-1 violation at {site}: contract for {name:?} not in map. \
                 SCC ordering (PL-1a) should guarantee callee analyzed before caller."
            )
        })
    }
}
