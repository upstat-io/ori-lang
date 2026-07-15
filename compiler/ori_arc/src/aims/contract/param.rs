//! Per-parameter memory contract ([`ParamContract`]) and the caller-side
//! return-alias shape it carries ([`ReturnAliasShape`]).
//!
//! Split out of [`super`] to keep the contract module under the 500-line
//! hygiene cap.

use super::super::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};

/// Independent-owner credit required before entering a callee through a
/// borrowed residual-call boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalleeOwnerDemand {
    /// The callee observes the value without consuming an ownership credit.
    Borrow,
    /// The callee consumes one credit for the complete value.
    WholeValue,
    /// The callee consumes one credit for exactly one projected field.
    ProjectedField(u32),
}

/// Contradictory final contract facts that demand both whole-value and
/// projected-field credits at one parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalleeOwnerDemandConflict {
    pub field: u32,
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
/// `walk.rs`, `walk_dec.rs`, `dead_cleanup/mod.rs`, `forward_walk.rs`).
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
    /// May the callee introduce or observe multiple logical owners of this
    /// parameter's value?
    pub may_share: bool,
    /// Locality lower bound: the callee guarantees this parameter stays at
    /// least this local (v1: always `Unknown`).
    pub locality_bound: Locality,
    /// Caller-guaranteed uniqueness at entry.
    ///
    /// When ALL call sites pass this parameter with `Owned + Linear + Once`,
    /// the interprocedural analysis tightens this to `Unique` — the callee can
    /// trust that exactly one logical owner exists at entry. This is the
    /// callee-side dual of COW-aware borrowing (07.3.1); it does not prescribe
    /// a runtime counter.
    ///
    /// Default: `MaybeShared` (no caller guarantee).
    pub uniqueness: Uniqueness,
    /// Whether this parameter flows directly to a `Return { value }`
    /// terminator (BUG-04-090 fix).
    ///
    /// When `true`, ownership of the parameter transfers through the return
    /// to the caller — the callee MUST NOT discharge the parameter's ownership
    /// credit at scope exit, because the caller owns the bound result's eventual
    /// release. The current carrier spells that release `RcDec`. Computed from the structural Return-flow alias
    /// fact in `detect_consumed_params` (NOT from `preserves_freshness`,
    /// which is currently spec-inverted).
    ///
    /// Completes the logical `ownParamsUsingArgs` pattern: event-pair
    /// elimination on owned-arg → owned-callee-param transfer applies to
    /// both the caller's additional credit and the callee's release.
    ///
    /// Default: `false` (conservative — retain the release obligation).
    pub transfers_through_return: bool,
    /// Shape of how this parameter aliases the callee's return value
    /// (BUG-04-090 fix — caller-side carrier for `apply_result_aliases`).
    ///
    /// Direct case: callee returns the param unchanged (`@id<T>(x: T) -> T
    /// = x`). Project case: callee returns a field projection
    /// (`@unwrap<T>(b: Box<T>) -> T = b.inner`).
    ///
    /// Consumed by the caller-side `apply_result_aliases` population at the
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
    /// Consumed by the burden-path transitive-drop alias machinery —
    /// `intraprocedural/apply_aliases.rs` (the `aliasing_params` filter) and
    /// `intraprocedural/post_convergence.rs::materialize_transitive_drop_singleton_classes`
    /// — via `owned || return_payload_contains_param`, so caller-side
    /// transitive-drop containment relationships are admitted even when the
    /// callee's param contract has `access: Borrowed`.
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
    /// caller release is suppressed — the callee's iterator cleanup is the
    /// single logical release. Soundness:
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
    /// physical runtime manages the receiver's sharing mechanics), so
    /// `access == Borrowed` alone does NOT prove non-COW — this is the distinct
    /// per-param fact that does, the affirmative read-only complement of
    /// `iter_consumes`.
    ///
    /// Consumed on the CALLER side by
    /// `realize::emit_unified::compute_user_call_arg_lineages`: a fresh-local
    /// COLLECTION at a Borrowed user-call arg whose callee param
    /// `borrowed_read_only` is `true` SURVIVES the call and the caller retains
    /// ownership — it stays ELIGIBLE for the dead-owned alloc-aware net, which
    /// fires one RL-2 scope-exit release iff one logical credit remains (the
    /// collection analogue of the borrowed-`str` carve-out). A COW-mutating callee
    /// (`borrowed_read_only == false`) stays excluded because the transfer and
    /// cleanup facts already assign the one release.
    ///
    /// Soundness: a pure borrow-read of a surviving Borrowed collection is
    /// `ApplyToBorrowedParam` (RL-2 non-transfer → caller releases)
    /// — `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec` +
    /// `RL2_release_exactly_once`. A COW-mutated / iter-consumed param is a
    /// transfer (no caller release) — historical theorem
    /// `RL2_transfer_kinds_no_dec`.
    ///
    /// IC-3 join: AND (the read-only guarantee holds only if it holds on EVERY
    /// path — any owned-position use on any path clears it).
    ///
    /// Scalar params: `false` (no managed ownership obligation, never a release target — the gate also
    /// requires a non-scalar collection arg so the value is irrelevant for them).
    ///
    /// Default: `false` (conservative — caller keeps its exclusion, no release).
    pub borrowed_read_only: bool,

    /// RL-1 + RL-2 borrowed-COW-consume fact: the Borrowed param's lineage is
    /// consumed at a COW-MUTATOR owned position (builtin consuming receiver /
    /// consuming second arg, or transitively a callee param carrying this
    /// fact) AS the lineage's LAST use in the body. The callee's COW-inc edge
    /// release then nets -1 on the caller's allocation (the realization
    /// convention treats the call as an effective consume), so the CALLER
    /// funds one reference per call site — the owned-call-arg duplication-inc
    /// admission (`compute_genuine_dup_call_arg_aliases`) consumes this fact.
    /// An aggregate-STORE of the borrowed param (`Inner { items: xs }`) does
    /// NOT set it (the borrowed-store dup inc + container drop net 0); a
    /// live-past COW consume does NOT set it (the callee's edge release
    /// declines, net 0).
    ///
    /// IC-3 join: OR (a consuming path obligates the caller's funding).
    ///
    /// Default: `false` (conservative — no funding obligation claimed).
    pub borrowed_cow_consumed: bool,

    /// MUTATOR-only refinement of `borrowed_cow_consumed`: the consuming
    /// position is a genuine COW MUTATOR (`push`/`insert`/`set`/... — the
    /// consuming-receiver set MINUS the builtin `iter`, whose consumption is
    /// iterator machinery, not a COW mutation), or transitively a callee param
    /// carrying THIS fact. Consumed by the borrowed-`Invoke` lineage gate (c3):
    /// a lineage member borrowed into a COW-mutating callee needs its per-call
    /// RL-1 funding inc kept (the callee nets -1), while an iter-consuming
    /// callee's accounting stays with the iter-consume machinery.
    ///
    /// IC-3 join: OR (a mutating path obligates the caller's funding).
    ///
    /// Default: `false` (conservative — no mutator claim).
    pub borrowed_cow_mutated: bool,

    /// Field-grained iter-consume record (RL-2 iter-consume inward-transfer,
    /// the per-field refinement of `iter_consumes`).
    ///
    /// `Some(field)` when this BORROWED aggregate param has its `Project
    /// param.field` projection iter-consumed (`@iter [own]` → `ori_iter_drop`
    /// frees the projected collection INSIDE the callee), while the param itself
    /// is NOT directly iter-consumed (the whole-param `iter_consumes` covers the
    /// param-IS-the-collection case; this covers the field-IS-the-collection
    /// case). Exactly one consumed field; multiple distinct consumed fields, or a
    /// field also read past the consume, poison to `None`.
    ///
    /// At a call where a fresh/dead OWNED aggregate is passed at this borrowed
    /// argument position, the callee transfers ownership of `field` inward
    /// (RL-2). Any caller-side aggregate release must therefore skip that field,
    /// releasing the shell and its other owned fields without double-freeing the
    /// iter-consumed field.
    ///
    /// IC-3 join: equal `Some` survives; disagreement (different field index)
    /// poisons to `None`. A single callee computes this once intraprocedurally.
    ///
    /// Default: `None` (conservative — no field-grained iter-consume claim).
    pub iter_consumes_projected_field: Option<u32>,
}

impl ParamContract {
    /// Freeze the exact owner-credit obligation at a borrowed residual-call
    /// boundary.
    ///
    /// This is the single semantic oracle for closure adapters. Ordinary Owned
    /// access, borrowed COW consumption, and plain RL-2 iterator consumption
    /// require a whole-value credit. Field-grained iterator consumption needs
    /// only that field's credit; retaining every field would leak. Return alias
    /// and return containment facts have their own result accounting and do not
    /// add a pre-entry credit here.
    pub fn callee_owner_demand(&self) -> Result<CalleeOwnerDemand, CalleeOwnerDemandConflict> {
        let whole_value = self.access == AccessClass::Owned
            || self.borrowed_cow_consumed
            || self.is_rl2_consume();
        match (whole_value, self.iter_consumes_projected_field) {
            (true, Some(field)) => Err(CalleeOwnerDemandConflict { field }),
            (true, None) => Ok(CalleeOwnerDemand::WholeValue),
            (false, Some(field)) => Ok(CalleeOwnerDemand::ProjectedField(field)),
            (false, None) => Ok(CalleeOwnerDemand::Borrow),
        }
    }

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
        // Conservative default: no funding obligation claimed — the
        // owned-call-arg duplication admission never fires on an unknown
        // callee.
        borrowed_cow_consumed: false,
        // Conservative default: no mutator claim — the lineage gate (c3) never
        // declines on an unknown callee.
        borrowed_cow_mutated: false,
        // Conservative default: no field-grained iter-consume claim — an unknown
        // callee never feeds the caller-side aggregate-field iter-consume scan.
        iter_consumes_projected_field: None,
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
        // IC-2 starts most-optimistic at the OR-join bottom (`false`). Join
        // (OR) promotes when a path's COW-consume-at-death fact fires.
        borrowed_cow_consumed: false,
        // IC-2 OR-join bottom; promotes when a path's MUTATOR consume fires.
        borrowed_cow_mutated: false,
        // IC-2 seed: no claim. `extract_contract` overrides per-param from the
        // body's projected-field iter-consume scan; a single callee computes it once.
        iter_consumes_projected_field: None,
    };

    /// RL-2 CONSUME classification: `iter_consumes ∧ ¬transfers_through_return`.
    ///
    /// A ttr+iter-consume callee hands the argument back through its own
    /// return, so that overlap keeps its own accounting (see
    /// `compute_ttr_iter_consume_dup_aliases`) and must not double-count as a
    /// plain iter-consume here.
    #[must_use]
    pub fn is_rl2_consume(&self) -> bool {
        self.iter_consumes && !self.transfers_through_return
    }

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
            // OR (conservative direction): a consuming path obligates the
            // caller's funding.
            borrowed_cow_consumed: self.borrowed_cow_consumed || other.borrowed_cow_consumed,
            // OR (conservative direction): a mutating path obligates the
            // caller's funding (the lineage gate declines).
            borrowed_cow_mutated: self.borrowed_cow_mutated || other.borrowed_cow_mutated,
            // Equal `Some` survives; disagreement (different consumed field)
            // poisons to `None`. A single callee computes its own field-grained
            // iter-consume shape once.
            iter_consumes_projected_field: match (
                self.iter_consumes_projected_field,
                other.iter_consumes_projected_field,
            ) {
                (None, x) | (x, None) => x,
                (Some(a), Some(b)) if a == b => Some(a),
                (Some(_), Some(_)) => None,
            },
        }
    }
}
