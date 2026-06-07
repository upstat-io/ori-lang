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
#[cfg(test)]
mod tests;

pub use context::{ContextBehavior, ContextRegion};

use super::lattice::{AccessClass, Cardinality, Consumption, Locality, ShapeClass, Uniqueness};
use crate::ir::ArcParam;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::uniqueness::UniquenessSummary;

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

/// Shape of how a parameter aliases the callee's return value.
///
/// Caller-side carrier for BUG-04-090's `apply_result_aliases` side-table
/// population. Distinct in role from `transfers_through_return: bool`:
/// `transfers_through_return` is the callee-side gate (suppress callee's
/// scope-exit dec when the param flows to Return); `return_alias` is the
/// caller-side shape carrier (record what the callee's return aliases so
/// the caller's Apply site can populate the `apply_result_aliases` map).
///
/// Invariant: `transfers_through_return == true` IFF `return_alias ==
/// Some(ReturnAliasShape::Direct)`. The bool is a derived special case of
/// the enum, kept as a separate field to avoid breaking the existing
/// callee-side consumers (`helpers.rs::should_suppress_return_transfer_dec`,
/// `walk.rs:108`, `walk_dec.rs:219`, `dead_cleanup/mod.rs:152/211`,
/// `forward_walk.rs:193`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnAliasShape {
    /// Callee returns the parameter unchanged (e.g., `@id<T>(x: T) -> T = x`).
    /// At the caller, `Apply dst = @callee(arg)` makes `dst` the same
    /// allocation as `arg`.
    Direct,
    /// Callee returns a field projection of the parameter
    /// (e.g., `@unwrap<T>(b: Box<T>) -> T = b.inner`). At the caller,
    /// `Apply dst = @callee(arg)` makes `dst` the same allocation as
    /// `arg.field`. Single field index — nested projections not yet
    /// supported (matches the existing `BorrowSource::Exact { field:
    /// Option<u32> }` precedent).
    Project { field: u32 },
}

impl ReturnAliasShape {
    /// IC-3 join for `Option<ReturnAliasShape>`.
    ///
    /// Lattice chain: `None < Some(Project) < Some(Direct)` (height 2).
    /// `Direct` is TOP — absorbs `Project` (lattice monotonicity requires
    /// `join(TOP, x) = TOP`). When one path returns the whole param
    /// (Direct) and another returns a field projection (Project), the safe
    /// caller-side contract is Direct: the caller cannot rely on
    /// field-level alias info when one path skips the projection.
    /// Incomparable Project paths (different field indices) join to Direct
    /// per the same rationale.
    #[must_use]
    pub fn join(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        match (a, b) {
            (None, None) => None,
            (Some(s), None) | (None, Some(s)) => Some(s),
            // Same Project field on both paths: preserve the field-level shape.
            (Some(Self::Project { field: f1 }), Some(Self::Project { field: f2 })) if f1 == f2 => {
                Some(Self::Project { field: f1 })
            }
            // Direct is TOP (absorbs Project); incomparable Project paths
            // (different field indices) also widen to Direct. Both cases
            // resolve to `Some(Direct)` per the lattice-monotonicity rule
            // `join(TOP, x) = TOP`.
            (Some(Self::Direct), _)
            | (_, Some(Self::Direct))
            | (Some(Self::Project { .. }), Some(Self::Project { .. })) => Some(Self::Direct),
        }
    }
}

/// Per-parameter memory contract.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each bool encodes a distinct AIMS facet not derivable from \
        the others: `may_escape` (legacy escape flag retained until \
        Locality SSOT migration), `may_share` (per-param sharing flag), \
        `transfers_through_return` (Return-flow alias), \
        `return_payload_contains_param` (transitive-drop containment), \
        `iter_consumes` (iter-consume inward-transfer, RL-2). \
        Bundling would obscure the analysis surface."
)]
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
    /// Caller-guaranteed uniqueness at entry.
    ///
    /// When ALL call sites pass this parameter with `Owned + Linear + Once`,
    /// the interprocedural analysis tightens this to `Unique` — the callee
    /// can trust that the argument's runtime RC == 1 at entry. This is the
    /// callee-side dual of COW-aware borrowing (07.3.1).
    ///
    /// Default: `MaybeShared` (no caller guarantee).
    pub uniqueness: Uniqueness,
    /// Whether this parameter flows directly to a `Return { value }`
    /// terminator (BUG-04-090 fix).
    ///
    /// When `true`, ownership of the parameter transfers through the return
    /// to the caller — the callee MUST NOT emit a scope-exit `RcDec` on the
    /// parameter, because the caller will dec the bound result variable
    /// when ITS scope exits. Computed from the structural Return-flow alias
    /// fact in `detect_consumed_params` (NOT from `preserves_freshness`,
    /// which is currently spec-inverted).
    ///
    /// Completes the Lean 4 `ownParamsUsingArgs` pattern: inc/dec
    /// elimination on owned-arg → owned-callee-param transfer applies to
    /// BOTH the caller's inc AND the callee's dec.
    ///
    /// Default: `false` (conservative — emit dec).
    pub transfers_through_return: bool,
    /// Shape of how this parameter aliases the callee's return value
    /// (BUG-04-090 fix — caller-side carrier for `apply_result_aliases`).
    ///
    /// Direct case: callee returns the param unchanged (`@id<T>(x: T) -> T
    /// = x`). Project case: callee returns a field projection
    /// (`@unwrap<T>(b: Box<T>) -> T = b.inner`).
    ///
    /// Consumed by File 12's `apply_result_aliases` population at the
    /// caller's Apply site. Invariant: `transfers_through_return == true`
    /// IFF `return_alias == Some(ReturnAliasShape::Direct)`. Both fields
    /// are derived together in `extract_contract`; downstream callers read
    /// whichever fits their question.
    ///
    /// Default: `None` (conservative — no aliasing claim).
    pub return_alias: Option<ReturnAliasShape>,
    /// Whether the parameter is structurally contained in the function's
    /// returned value as a transitive-drop variant payload.
    ///
    /// Distinct from `return_alias`: `return_alias` covers cases where the
    /// callee returns the param directly (`Direct`) or a field projection
    /// of the param (`Project`). This field covers cases where the callee
    /// constructs a transitive-drop variant containing an alias of the
    /// param and returns that — e.g., `@wrap_ok (m: T) -> Result<T, E> =
    /// Ok(m)`. Here `m` is contained in the returned `Result.payload[0]`
    /// but neither `Direct` nor `Project` matches.
    ///
    /// Consumed by `populate_class_payload_of_with_liveness` in
    /// `intraprocedural/post_convergence.rs`: the Apply/Invoke
    /// edge-recording gate uses `owned || return_payload_contains_param`
    /// so `class_payload_of` edges are recorded for caller-side
    /// transitive-drop containment relationships even when the callee's
    /// param contract has `access: Borrowed`.
    ///
    /// Default: `false` (conservative — no containment claim).
    pub return_payload_contains_param: bool,
    /// Whether the callee iter-consumes this borrowed parameter and frees it
    /// via the iterator drop (RL-2 iter-consume transfer).
    ///
    /// `true` when the parameter (through Let-Var aliases) flows into an
    /// `@iter` / `Iter`-protocol owned-position consume whose resulting
    /// iterator handle is `ori_iter_drop`'d in the callee body — the `for x in
    /// coll` lowering frees the collection INSIDE the callee. The parameter
    /// keeps `access: Borrowed` (the callee neither stores nor returns it), so
    /// neither `transfers_through_return` nor an Owned-access upgrade fires; the
    /// inward-transfer-into-iter-and-drop is a distinct fact this field is the
    /// sole carrier of.
    ///
    /// Consumed on the CALLER side by
    /// `realize::emit_unified::borrowed_arg_release_verdict`: a collection at a
    /// borrowed terminator-`Invoke` arg whose callee param `iter_consumes` is
    /// `true` is classified `ApplyToIterConsumingParam` (RL-2 TRANSFER) and its
    /// caller burden dec is SUPPRESSED — the callee's `ori_iter_drop` is the
    /// single release. Soundness:
    /// `AimsProof.Realization::RL2_iter_consuming_caller_dec_splits`.
    ///
    /// IC-3 join: OR (once `true` on any path, stays `true`).
    ///
    /// Default: `false` (conservative — caller keeps its dec).
    pub iter_consumes: bool,
    /// Whether this borrowed parameter flows ONLY to borrowed positions in the
    /// callee body — no owned-position consumer (no COW-mutation, no transfer).
    ///
    /// `true` when the parameter (through Let-Var / Jump-arg / Select aliases)
    /// appears only at Borrowed arg positions of `Apply`/`Invoke` (a pure
    /// borrow-read such as `xs.length()` / `xs[idx]`), never at an OWNED position
    /// (a COW-mutator like `xs.push(v)` consumes the receiver `[own]`, an iter
    /// consume routes through `@iter [own]`, a transfer hands it inward). Borrow
    /// inference leaves a COW-mutated receiver param `access: Borrowed` (the
    /// runtime manages the COW receiver's RC internally), so `access == Borrowed`
    /// alone does NOT prove non-COW — this is the distinct per-param fact that
    /// does, the affirmative read-only complement of `iter_consumes`.
    ///
    /// Consumed on the CALLER side by
    /// `realize::emit_unified::compute_user_call_arg_lineages`: a fresh-local
    /// COLLECTION at a Borrowed user-call arg whose callee param
    /// `borrowed_read_only` is `true` SURVIVES the call and the caller retains
    /// ownership — it stays ELIGIBLE for the dead-owned alloc-aware net, which
    /// fires ONE RL-2 scope-exit release iff net +1 (the collection analogue of
    /// the borrowed-`str` carve-out). A COW-mutating callee
    /// (`borrowed_read_only == false`) stays EXCLUDED (a caller release would
    /// double-free the COW-shared buffer).
    ///
    /// Soundness: a pure borrow-read of a surviving Borrowed collection is
    /// `ApplyToBorrowedParam` (RL-2 NON-transfer → caller decs)
    /// — `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec` +
    /// `RL2_release_exactly_once`. A COW-mutated / iter-consumed param is a
    /// transfer (NO caller dec) — `RL2_transfer_kinds_no_dec`.
    ///
    /// IC-3 join: AND (the read-only guarantee holds only if it holds on EVERY
    /// path — any owned-position use on any path clears it).
    ///
    /// Scalar params: `false` (no RC, never a release target — the gate also
    /// requires a non-scalar collection arg so the value is irrelevant for them).
    ///
    /// Default: `false` (conservative — caller keeps its exclusion, no release).
    pub borrowed_read_only: bool,
}

impl ParamContract {
    /// Conservative: owned, unrestricted, many uses, may escape/share, unknown locality,
    /// no caller uniqueness guarantee, no return-flow transfer, no return alias.
    pub const CONSERVATIVE: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        may_escape: true,
        may_share: true,
        locality_bound: Locality::Unknown,
        uniqueness: Uniqueness::MaybeShared,
        // Conservative default: emit scope-exit dec. The fixpoint
        // promotes to `true` only when extract_contract proves the
        // param flows to Return.
        transfers_through_return: false,
        // Conservative default: no aliasing claim — caller's Apply dst is
        // treated as a fresh allocation, not aliased to any consumed arg.
        // Promoted by extract_contract when the param flows to Return
        // (Direct) or to a Project that flows to Return (Project).
        return_alias: None,
        // Conservative default: no containment claim. Promoted by
        // extract_contract when the param flows into a transitive-drop
        // variant payload that is returned.
        return_payload_contains_param: false,
        // Conservative default: caller keeps its dec. Promoted by
        // extract_contract when the borrowed param iter-consumes-and-frees.
        iter_consumes: false,
        // Conservative default: caller keeps its exclusion (no read-only claim).
        // AND-join bottom is `false` for the conservative contract — an unknown
        // callee is assumed to consume the param, so the carve-out never fires.
        borrowed_read_only: false,
    };

    /// Most-optimistic: borrowed, dead, absent, no escape/share, block-local, unique,
    /// no return-flow transfer, no return alias.
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
        uniqueness: Uniqueness::Unique,
        // IC-2 starts most-optimistic. Join (OR) promotes to `true` when
        // any path's structural Return-flow alias fact fires.
        transfers_through_return: false,
        // IC-2 starts most-optimistic at the BOTTOM of the chain
        // `None < Some(Project) < Some(Direct)`. Join promotes upward as
        // structural Return-aliasing facts fire across paths.
        return_alias: None,
        // IC-2 starts most-optimistic. Join (OR) promotes to `true` when
        // any path's structural payload-containment fact fires.
        return_payload_contains_param: false,
        // IC-2 starts most-optimistic. Join (OR) promotes to `true` when
        // any path's iter-consume-and-free fact fires.
        iter_consumes: false,
        // IC-2 starts most-optimistic at the AND-join TOP (`true` — assume
        // read-only). Join (AND) demotes to `false` when any path uses the param
        // at an owned position. `extract_contract` overrides per-param from the
        // body scan; OPTIMISTIC is the fixpoint seed only.
        borrowed_read_only: true,
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
            uniqueness: self.uniqueness.join(other.uniqueness),
            // OR (conservative direction): once true on any path, stays true.
            // Matches IC-3 componentwise-max semantics for boolean dimensions.
            transfers_through_return: self.transfers_through_return
                || other.transfers_through_return,
            // Lattice chain `None < Some(Project) < Some(Direct)` (height 2).
            // Direct is TOP — absorbs Project. Incomparable Project paths
            // join to Direct. Per `ReturnAliasShape::join`.
            return_alias: ReturnAliasShape::join(self.return_alias, other.return_alias),
            // OR (conservative direction): once true on any path, stays true.
            // Matches IC-3 componentwise-max semantics for boolean dimensions.
            return_payload_contains_param: self.return_payload_contains_param
                || other.return_payload_contains_param,
            // OR (conservative direction): once true on any path, stays true.
            // Matches IC-3 componentwise-max semantics for boolean dimensions.
            iter_consumes: self.iter_consumes || other.iter_consumes,
            // AND (conservative direction): the read-only guarantee holds only if
            // it holds on EVERY joined path — any owned-position use clears it.
            borrowed_read_only: self.borrowed_read_only && other.borrowed_read_only,
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
                debug_assert_eq!(a.len(), b.len(), "FipContract param counts must match");
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
/// Call sites that previously fell back silently to `MemoryContract::default`
/// produce unsound results: the optimistic default (all-`Borrowed` /
/// `Dead` / `Absent` params, no effects) inflates safety claims through every
/// downstream `refine` call (TF-6) and `EffectSummary` join (IC-5),
/// producing miscompilation rather than a clean panic.
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
