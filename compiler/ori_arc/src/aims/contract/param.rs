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
    /// Join two present return-alias shapes.
    #[must_use]
    pub(crate) fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Project { field: left }, Self::Project { field: right }) if left == right => {
                Self::Project { field: left }
            }
            (Self::Direct, _)
            | (_, Self::Direct)
            | (Self::Project { .. }, Self::Project { .. }) => Self::Direct,
        }
    }

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
    pub fn join_optional(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        match (a, b) {
            (None, None) => None,
            (Some(s), None) | (None, Some(s)) => Some(s),
            (Some(left), Some(right)) => Some(left.join(right)),
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
    /// funds one reference per call site — the class-ledger borrowed-boundary
    /// classification consumes this fact and freezes the duplication credit.
    /// An aggregate-STORE of the borrowed param (`Inner { items: xs }`) does
    /// NOT set it (the borrowed-store dup inc + container drop net 0); a
    /// live-past COW consume does NOT set it (the callee's edge release
    /// declines, net 0).
    ///
    /// IC-3 join: OR (a consuming path obligates the caller's funding).
    ///
    /// Default: `false` (conservative — no funding obligation claimed).
    pub borrowed_cow_consumed: bool,

    /// COW-mutator refinement of `borrowed_cow_consumed`, set directly by a
    /// consuming mutator (`push`/`insert`/`set`/...) or transitively by a
    /// callee carrying this fact. The builtin `iter` is excluded because its
    /// ownership uses iterator accounting. The borrowed-`Invoke` lineage gate
    /// keeps the per-call RL-1 funding increment for this demand.
    /// IC-3 join: OR. Default: `false` (no mutator claim).
    pub borrowed_cow_mutated: bool,
}

impl ParamContract {
    /// Freeze the exact owner-credit obligation at a borrowed residual-call
    /// boundary.
    ///
    /// This is the single semantic oracle for closure adapters. Ordinary Owned
    /// access, borrowed COW consumption, and plain RL-2 iterator consumption
    /// require a whole-value credit. Return alias and return containment facts
    /// have their own result accounting and do not add a pre-entry credit here.
    #[must_use]
    pub fn callee_owner_demand(&self) -> CalleeOwnerDemand {
        let whole_value = self.access == AccessClass::Owned
            || self.borrowed_cow_consumed
            || self.is_rl2_consume();
        if whole_value {
            CalleeOwnerDemand::WholeValue
        } else {
            CalleeOwnerDemand::Borrow
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
        // Default false; extraction sets true only for params that flow to Return.
        transfers_through_return: false,
        // Fresh Apply destinations claim no alias until extraction proves Return flow.
        return_alias: None,
        // Extraction claims containment only for returned transitive-drop payloads.
        return_payload_contains_param: false,
        // The caller keeps its dec unless extraction proves iter-consume-and-free.
        iter_consumes: false,
        // Unknown callees make no read-only claim, so the caller keeps its exclusion.
        borrowed_read_only: false,
        // Unknown callees claim no funding obligation for owned-call-arg duplication.
        borrowed_cow_consumed: false,
        // Unknown callees claim no mutation, so lineage gate c3 never declines.
        borrowed_cow_mutated: false,
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
        // IC-2 OR-join starts false and rises on any structural Return-flow fact.
        transfers_through_return: false,
        // IC-2 return-alias BOTTOM rises along None < Project < Direct.
        return_alias: None,
        // IC-2 OR-join rises on any structural payload-containment fact.
        return_payload_contains_param: false,
        // IC-2 OR-join rises on any iter-consume-and-free fact.
        iter_consumes: false,
        // IC-2 AND-join TOP; owned use demotes the body-derived fact to false.
        borrowed_read_only: true,
        // IC-2 OR-join rises when any path consumes COW storage at death.
        borrowed_cow_consumed: false,
        // IC-2 OR-join bottom; promotes when a path's MUTATOR consume fires.
        borrowed_cow_mutated: false,
    };

    /// RL-2 CONSUME classification: `iter_consumes ∧ ¬transfers_through_return`.
    ///
    /// A ttr+iter-consume callee hands the argument back through its own
    /// return, so that overlap keeps its own accounting (see
    /// `compute_ttr_iter_consume_dup_aliases`) and must not double-count as a
    /// plain iter-consume here.
    #[must_use]
    fn is_rl2_consume(&self) -> bool {
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
            // IC-3 boolean max: true on any path remains true.
            transfers_through_return: self.transfers_through_return
                || other.transfers_through_return,
            // ReturnAliasShape::join_optional implements None < Project < Direct.
            return_alias: ReturnAliasShape::join_optional(self.return_alias, other.return_alias),
            // IC-3 boolean max: true on any path remains true.
            return_payload_contains_param: self.return_payload_contains_param
                || other.return_payload_contains_param,
            // IC-3 boolean max: true on any path remains true.
            iter_consumes: self.iter_consumes || other.iter_consumes,
            // Read-only survives only when every joined path preserves it.
            borrowed_read_only: self.borrowed_read_only && other.borrowed_read_only,
            // Any consuming path obligates caller funding.
            borrowed_cow_consumed: self.borrowed_cow_consumed || other.borrowed_cow_consumed,
            // Any mutating path obligates caller funding and declines lineage.
            borrowed_cow_mutated: self.borrowed_cow_mutated || other.borrowed_cow_mutated,
        }
    }
}
