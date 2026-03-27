//! `ReprPlan` — the central decision document for representation optimization.
//!
//! Each optimization pass (§02–§11) writes narrowing decisions into the
//! `ReprPlan` with full provenance tracking. Codegen reads the final
//! plan to determine the machine representation of every type.
//!
//! # Design
//!
//! - **Audit trail**: every decision is recorded in order, even when
//!   overridden — useful for debugging why a type was narrowed.
//! - **Safe defaults**: queries return canonical (un-narrowed) values
//!   when no decision has been recorded — §01 alone causes zero
//!   behavioral change.
//! - **Placeholder fields**: `escape_info` and `function_var_ranges`
//!   use stub types until §03 and §08 replace them.

use ori_arc::ArcVarId;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::escape::EscapeInfo;
use crate::range::ValueRange;
use crate::repr::MachineRepr;

// Re-export sub-types for plan consumers.
pub use self::decision::{DecisionReason, DecisionSource, ReprDecision};
pub use self::query::{NarrowingPolicy, RcStrategy};
pub use self::repr_attr::ReprAttribute;

mod decision;
pub(crate) mod query;
mod repr_attr;

/// The central data structure recording all narrowing decisions.
///
/// Computed after type checking and before LLVM codegen. The type checker
/// never sees `ReprPlan`; codegen reads from it but never writes.
///
/// # Salsa Integration (§01.6)
///
/// `ReprPlan` is **not** a Salsa tracked struct. It is computed imperatively
/// by [`compute_repr_plan()`] as a forward pass that mutates state across
/// multiple analysis phases (triviality → range → narrowing → layout).
/// Making each phase a Salsa query would create artificial dependencies and
/// complicate the multi-pass mutation pattern.
///
/// Instead, `ReprPlan` is computed once per compilation and passed as
/// `&ReprPlan` to codegen — the same model as [`TypeInfoStore`] in
/// `ori_llvm`, but without interior mutability.
///
/// **Invalidation:** Recomputed on every compilation. Future optimization:
/// if the Pool is unchanged (Salsa cache hit on type checking), the
/// previous `ReprPlan` can be reused via a Salsa query keyed on Pool
/// identity.
///
/// **JIT compatibility:** The JIT path recomputes the entire `ReprPlan`
/// per invocation, matching `TypeInfoStore`'s current behavior. Future
/// optimization: incremental updates keyed by function-level changes.
///
/// **Thread safety:** All fields are plain `FxHashMap`/`Vec` — no
/// `RefCell`, `Mutex`, or interior mutability. After construction,
/// `&ReprPlan` is `Send + Sync` by the implicit auto-trait rules.
/// This contrasts with `TypeInfoStore` which uses `RefCell` for lazy
/// population.
///
/// [`compute_repr_plan()`]: crate::compute_repr_plan
/// [`TypeInfoStore`]: https://docs.rs/ori_llvm (internal)
#[derive(Debug)]
#[expect(
    clippy::zero_sized_map_values,
    reason = "EscapeInfo is placeholder ZST — replaced by §08"
)]
pub struct ReprPlan {
    /// Per-type decisions (indexed by Pool `Idx`).
    decisions: FxHashMap<Idx, ReprDecision>,
    /// Per-type `#repr` attributes (only for structs/enums with explicit attrs).
    repr_attrs: FxHashMap<Idx, ReprAttribute>,
    /// Per-type RC strategy decisions (indexed by Pool `Idx`).
    ///
    /// Stored **separately** from `decisions` because RC strategy is metadata
    /// about how a value is reference-counted, not a replacement for its
    /// layout. Writing an RC decision must not destroy the type's
    /// `MachineRepr` (TPR-01-022). Populated by §09 (ARC header compression)
    /// and §10 (thread-local ARC).
    rc_strategies: FxHashMap<Idx, RcStrategy>,
    /// Per-function escape info (indexed by function `Name`).
    ///
    /// Empty until §08 populates it.
    escape_info: FxHashMap<Name, EscapeInfo>,
    /// Per-function, per-variable ranges from §03 range analysis.
    ///
    /// Key: function `Name` → (`ArcVarId` → `ValueRange`).
    /// Populated by §03, consumed by §04.
    function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
    /// Per-type-field range summaries from §03 field-summary analysis.
    ///
    /// Key: `(struct/tuple Idx, field_index)` → joined `ValueRange`.
    /// Populated by §03.2b (`FieldSummaryTable::flush_to_repr_plan`),
    /// consumed by §04 (struct field narrowing).
    field_range_summaries: FxHashMap<(Idx, u32), ValueRange>,
    /// Audit trail — all decisions in insertion order.
    audit: Vec<ReprDecision>,
    /// Narrowing policy controlling optimization aggressiveness.
    narrowing_policy: NarrowingPolicy,
}

impl ReprPlan {
    /// Create a new empty `ReprPlan` with the given narrowing policy.
    #[must_use]
    #[expect(
        clippy::zero_sized_map_values,
        reason = "EscapeInfo is placeholder ZST — replaced by §08"
    )]
    pub fn new(policy: NarrowingPolicy) -> Self {
        Self {
            decisions: FxHashMap::default(),
            repr_attrs: FxHashMap::default(),
            rc_strategies: FxHashMap::default(),
            escape_info: FxHashMap::default(),
            function_var_ranges: FxHashMap::default(),
            field_range_summaries: FxHashMap::default(),
            audit: Vec::new(),
            narrowing_policy: policy,
        }
    }

    /// Record a narrowing decision for a type.
    ///
    /// Later decisions override earlier ones for the same `Idx`, but the
    /// audit trail preserves both entries in insertion order.
    pub fn set_repr(&mut self, idx: Idx, decision: ReprDecision) {
        self.audit.push(decision.clone());
        self.decisions.insert(idx, decision);
    }

    /// Query the representation decision for a type.
    ///
    /// Returns `None` if no decision has been recorded — callers should
    /// fall back to `TypeInfoStore` (Phase A migration).
    #[must_use]
    pub fn get_repr(&self, idx: Idx) -> Option<&MachineRepr> {
        self.decisions.get(&idx).map(|d| &d.repr)
    }

    /// Record per-variable range analysis results for a function.
    ///
    /// Called by §03 after `range_fixpoint()` completes for a function.
    pub fn set_var_ranges(&mut self, func: Name, ranges: FxHashMap<ArcVarId, ValueRange>) {
        self.function_var_ranges.insert(func, ranges);
    }

    /// Get the range for a variable in a function.
    ///
    /// Returns the default `ValueRange` (unconstrained) if no range was
    /// recorded for this function or variable.
    #[must_use]
    pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange {
        self.function_var_ranges
            .get(&func)
            .and_then(|m| m.get(&var))
            .copied()
            .unwrap_or_default()
    }

    /// Get mutable access to a function's per-variable range map.
    ///
    /// Returns `None` if no ranges have been recorded for this function.
    /// Used by §03.5 to merge interprocedural parameter ranges into existing
    /// intraprocedural results.
    pub fn function_var_ranges_mut(
        &mut self,
        func: Name,
    ) -> Option<&mut FxHashMap<ArcVarId, ValueRange>> {
        self.function_var_ranges.get_mut(&func)
    }

    /// Join a field range into the persistent summary.
    ///
    /// Called by `FieldSummaryTable::flush_to_repr_plan()` after the
    /// fixpoint completes for each function. Multiple functions accumulate
    /// evidence by joining (not overwriting).
    pub fn join_field_range(&mut self, idx: Idx, field: u32, range: ValueRange) {
        self.field_range_summaries
            .entry((idx, field))
            .and_modify(|existing| *existing = existing.join(range))
            .or_insert(range);
    }

    /// Query the aggregated field range for a struct/tuple field.
    ///
    /// Returns `Top` if no construction sites were observed for this field.
    #[must_use]
    pub fn field_range(&self, idx: Idx, field: u32) -> ValueRange {
        self.field_range_summaries
            .get(&(idx, field))
            .copied()
            .unwrap_or_default()
    }

    /// Store a `#repr` attribute for a type.
    pub fn set_repr_attr(&mut self, idx: Idx, attr: ReprAttribute) {
        self.repr_attrs.insert(idx, attr);
    }

    /// Query the `#repr` attribute for a type.
    #[must_use]
    pub fn repr_attr(&self, idx: Idx) -> Option<&ReprAttribute> {
        self.repr_attrs.get(&idx)
    }

    /// Record per-function escape analysis info (§08 output).
    pub fn set_escape_info(&mut self, func: Name, info: EscapeInfo) {
        self.escape_info.insert(func, info);
    }

    /// Record an RC strategy decision for a type (§09/§10 output).
    ///
    /// Stores the strategy in a **separate map** so the type's `MachineRepr`
    /// layout is preserved. The audit trail records the decision for debugging.
    pub fn set_rc_strategy(&mut self, idx: Idx, strategy: RcStrategy, source: DecisionSource) {
        let reason = match strategy {
            RcStrategy::None => DecisionReason::TransitivelyTrivial,
            RcStrategy::NonAtomic { .. } => DecisionReason::Custom("thread-local".into()),
            RcStrategy::Atomic { .. } => DecisionReason::Canonical,
        };
        self.rc_strategies.insert(idx, strategy);
        // Record in audit trail for debugging without overwriting the repr.
        self.audit.push(ReprDecision {
            source,
            type_idx: idx,
            repr: self
                .get_repr(idx)
                .cloned()
                .unwrap_or(MachineRepr::OpaquePtr),
            reason,
        });
    }

    /// Iterate over all type indices that have a stored representation decision.
    ///
    /// Used by narrowing passes (§04) to find struct/tuple types to narrow
    /// without depending on pool iteration order.
    pub fn decision_indices(&self) -> impl Iterator<Item = Idx> + '_ {
        self.decisions.keys().copied()
    }

    /// Dump the audit trail for debugging.
    ///
    /// Returns a human-readable string with all decisions in insertion order.
    #[must_use]
    pub fn dump_audit(&self, pool: &Pool) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        for (i, d) in self.audit.iter().enumerate() {
            let tag = pool.tag(d.type_idx);
            let _ = writeln!(out, "[{i}] {tag:?} <- {:?}: {:?}", d.source, d.reason);
        }
        out
    }
}

// §01.6 Thread safety: compile-time assertion that ReprPlan is Send + Sync.
// ReprPlan has no interior mutability (no RefCell, no Mutex), so &ReprPlan
// can be safely shared across threads. This assertion catches regressions
// if a future field introduces non-Send/Sync types.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ReprPlan>();
};
