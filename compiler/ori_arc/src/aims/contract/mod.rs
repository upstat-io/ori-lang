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
mod effect;
mod param;
#[cfg(test)]
mod tests;

pub use context::{ContextBehavior, ContextRegion};
pub use effect::{
    EffectSummary, FipContract, FreshSelfAllocationFacts, FunctionEffectFacts, MemoryAccessClass,
};
pub use param::{CalleeOwnerDemand, ParamContract, ReturnAliasShape};

use super::lattice::{AccessClass, Cardinality, Consumption, Locality, ShapeClass, Uniqueness};
use crate::ir::{ArcFunction, ArcInstr, ArcParam, ArcTerminator};
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
    pub(crate) fn join(&self, other: &Self) -> Self {
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
            return_info: self.return_info.join(other.return_info),
            effects: self.effects.join(other.effects),
            context_behavior: self.context_behavior.join(&other.context_behavior),
            fip: self.fip.join(&other.fip),
            // is_fbip: AND (conservative direction — if either side allocates,
            // the joined contract is not FBIP).
            is_fbip: self.is_fbip && other.is_fbip,
        }
    }

    /// Realize the backend-neutral memory-access classification for RL-30.
    ///
    /// This consumes the final post-AIMS body as well as its final contract.
    /// `EffectSummary` describes ownership/FIP effects, not arbitrary runtime
    /// or I/O writes, so a contract alone cannot prove a whole-function memory
    /// attribute. Until IC-5 carries typed inaccessible-memory effects, every
    /// call is untyped and therefore fails closed. This includes apparently
    /// known calls and runtime I/O, panic, and thread-local-state operations;
    /// their names never substitute for typed write effects.
    #[must_use]
    pub fn function_effect_facts(&self, function: &ArcFunction) -> FunctionEffectFacts {
        let may_write_inaccessible = function.blocks.iter().any(|block| {
            block.body.iter().any(|instruction| {
                matches!(
                    instruction,
                    ArcInstr::Apply { .. } | ArcInstr::ApplyIndirect { .. }
                )
            }) || matches!(
                block.terminator,
                ArcTerminator::Invoke { .. }
                    | ArcTerminator::InvokeIndirect { .. }
                    | ArcTerminator::Resume
            )
        });
        let structurally_read_only = function.blocks.iter().all(|block| {
            block.body.iter().all(|instruction| {
                matches!(
                    instruction,
                    ArcInstr::Let { .. }
                        | ArcInstr::Project { .. }
                        | ArcInstr::Construct { .. }
                        | ArcInstr::Select { .. }
                )
            }) && matches!(
                block.terminator,
                ArcTerminator::Return { .. }
                    | ArcTerminator::Jump { .. }
                    | ArcTerminator::Branch { .. }
                    | ArcTerminator::Switch { .. }
                    | ArcTerminator::Unreachable
            )
        });
        let no_writes = !self.effects.may_allocate
            && !self.effects.may_deallocate
            && !self.effects.may_share
            && !self.effects.may_throw
            && !may_write_inaccessible
            && structurally_read_only
            && self.params.iter().all(|param| {
                param.cardinality == Cardinality::Absent
                    || (param.access == AccessClass::Borrowed && !param.may_share)
            });
        FunctionEffectFacts::new(
            self.effects,
            may_write_inaccessible,
            if no_writes {
                MemoryAccessClass::ReadOnly
            } else {
                MemoryAccessClass::ReadWrite
            },
        )
    }

    /// Freeze the backend-neutral RL-29 return-allocation fact.
    ///
    /// `preserves_freshness` is deliberately insufficient: a result may remain
    /// unique while forwarding caller-owned or consumed storage. Only the
    /// stronger path-universal self-allocation proof excludes upstream aliases.
    #[must_use]
    pub const fn fresh_self_allocation_facts(&self) -> FreshSelfAllocationFacts {
        FreshSelfAllocationFacts::new(self.return_info.returns_fresh_self_alloc)
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
    /// Logical lifetime bound of the returned value. The shipped carrier uses
    /// the legacy `HeapEscaping` label for most escaping results; it does not
    /// select heap placement.
    pub locality: Locality,
    /// Shape class of the return value.
    pub shape: ShapeClass,
    /// Whether every path returns a fresh, independently owned buffer identity
    /// with no upstream alias.
    /// The buffer may be produced directly by this body (`Construct`/`Reuse`/
    /// `CollectionReuse` or `@ori_list_take`) or by a callee carrying the same
    /// path-universal proof; it is never caller-provided or consumed-input
    /// storage. Stronger than `preserves_freshness ∧ uniqueness`, this licenses
    /// caller-side analyses to admit the Invoke/Apply result as a fresh collection
    /// root. Orthogonal to
    /// `uniqueness`/`preserves_freshness` — set independently, consumed only by
    /// the fresh-collection-root admission, so it never perturbs the
    /// uniqueness/freshness-driven store-dup accounting.
    /// Spec: Annex E §AIMS RL-1 + RL-29 (fresh + Unique non-aliasing).
    pub returns_fresh_self_alloc: bool,
    /// Whether the return value is a sharing view of a borrowed input's
    /// backing identity on every path — a seamless-slice co-reference
    /// (`slice`/`substring`/`take`/`drop`) whose producer creates the view's
    /// independent logical owner. The typed provenance CREDIT fact classifies
    /// that result on the source's identity class in addition to the source
    /// borrow-read, never as a fresh birth. Orthogonal to `uniqueness` /
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
    pub(crate) fn join(self, other: Self) -> Self {
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
