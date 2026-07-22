//! `ReprPlan` — the central decision document for representation optimization.
//!
//! Optimization passes record narrowing decisions with provenance; codegen
//! reads the closed plan. Missing decisions stay canonical, and only
//! AIMS-proven local identities admit local allocation mechanisms.

use crate::enum_repr::EnumRepr;
use crate::escape::EscapeInfo;
use crate::layout::EnumLayoutInfo;
use crate::range::ValueRange;
use crate::repr::MachineRepr;
use ori_arc::ir::{AllocationSiteId, ArcVarId, YieldExtent};
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

// Re-export sub-types for plan consumers.
pub use self::decision::{DecisionReason, DecisionSource, ReprDecision};
pub use self::query::{NarrowingPolicy, RcStrategy};
pub use self::repr_attr::ReprAttribute;

/// Concrete storage mechanism selected for one yield allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompiledAllocationMechanism {
    /// Existing growable RC-managed runtime allocation.
    RuntimeHeap,
    /// Bounded function-lifetime storage emitted in the owning stack frame.
    StackSlot,
}

/// Representation-layer allocation projection for one stable site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompiledAllocationDecision {
    pub site: AllocationSiteId,
    pub builder: ArcVarId,
    pub result: ArcVarId,
    pub elem_ty: Idx,
    pub elem_size: u64,
    pub extent: YieldExtent,
    pub mechanism: CompiledAllocationMechanism,
    /// Preserve the runtime RC header immediately before the element data.
    pub requires_runtime_header: bool,
}

impl CompiledAllocationDecision {
    /// Maximum element bytes admitted to function-lifetime stack storage.
    pub const MAX_LOCAL_BYTES: u64 = 4 * 1024;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum YieldAllocationIdentity {
    Builder(ArcVarId),
    Result(ArcVarId),
}

mod decision;
pub(crate) mod query;
mod range_plan;
mod repr_attr;
mod yield_plan;

pub(crate) use yield_plan::YieldLineageRuntimeCall;

/// The central data structure recording all narrowing decisions.
///
/// Computed after type checking and before LLVM codegen. The type checker
/// never sees `ReprPlan`; codegen reads from it but never writes.
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
/// **Invalidation:** Recomputed on every compilation. Reuse requires a Salsa
/// query keyed on Pool identity.
///
/// **JIT compatibility:** Each JIT invocation recomputes the entire `ReprPlan`.
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
    /// `MachineRepr`. Populated by ARC header compression
    /// and thread-local ARC passes.
    rc_strategies: FxHashMap<Idx, RcStrategy>,
    /// Per-function escape info (indexed by function `Name`).
    ///
    /// Absence of a function key conservatively means that its values escape.
    escape_info: FxHashMap<Name, EscapeInfo>,
    /// Compiled allocation projections keyed by function and stable site.
    yield_allocations: FxHashMap<(Name, AllocationSiteId), CompiledAllocationDecision>,
    /// Stable allocation sites indexed by the ARC identities used during emission.
    yield_allocation_sites: FxHashMap<(Name, YieldAllocationIdentity), AllocationSiteId>,
    /// Qualified call sites redirected to private length-only physical clones.
    length_projection_calls: FxHashMap<(Name, ArcVarId), Name>,
    /// Qualified callees and the returned yield virtualized by their clones.
    length_projection_yields: FxHashMap<Name, ArcVarId>,
    /// Per-function, per-variable ranges from range analysis.
    ///
    /// Key: function `Name` → (`ArcVarId` → `ValueRange`).
    /// Populated by range analysis, consumed by integer narrowing.
    function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
    /// Per-type-field range summaries from field-summary analysis.
    ///
    /// Key: `(struct/tuple Idx, field_index)` → joined `ValueRange`.
    /// Populated by `FieldSummaryTable::flush_to_repr_plan`,
    /// consumed by struct field narrowing.
    field_range_summaries: FxHashMap<(Idx, u32), ValueRange>,
    /// Per-collection-type element range summaries from element analysis.
    ///
    /// Key: collection type `Idx` (e.g., `[int]`) → joined `ValueRange` of
    /// all observed element values across `Construct(ListLiteral)` and
    /// `CollectionReuse` sites. Consumed by collection element narrowing.
    element_range_summaries: FxHashMap<Idx, ValueRange>,
    /// Canonical enum layout facts, keyed by enum type `Idx`.
    ///
    /// Populated from the final `EnumRepr` after all repr-optimization passes.
    /// Consumers query `enum_layout_info()` instead of computing layout ad-hoc.
    enum_layouts: FxHashMap<Idx, EnumLayoutInfo>,
    /// Audit trail — all decisions in insertion order.
    audit: Vec<ReprDecision>,
    /// Narrowing policy controlling optimization aggressiveness.
    narrowing_policy: NarrowingPolicy,
    /// Type indices declared `pub` — their layout is part of the ABI
    /// contract and must NOT be narrowed.
    ///
    /// Populated at plan construction from type checker visibility info.
    pub_type_indices: FxHashSet<Idx>,
    /// Function identities whose parameters must NOT be narrowed by
    /// interprocedural range analysis.
    ///
    /// Entries are `(Option<self_type_idx>, method_name)`:
    /// - `(None, name)` — pub top-level function
    /// - `(Some(idx), name)` — trait impl method on type `idx`
    ///
    /// Using `(Option<Idx>, Name)` prevents bare-Name collisions where a
    /// trait impl method and an unrelated inherent method share a name
    ///
    /// Closures are handled separately via `ArcFunction::num_captures > 0`.
    unconstrained_fn_names: FxHashSet<(Option<Idx>, Name)>,
    /// Whether the analysis set includes functions not fully integrated into
    /// the codegen pipeline (e.g., impl methods ARC-lowered for range analysis
    /// only, not for borrow inference or LLVM emission).
    ///
    /// When true, integer narrowing is suppressed because the field-range
    /// summaries from analysis-only functions may trigger narrowing for structs
    /// that cross ABI boundaries without proper widening.
    has_analysis_only_functions: bool,
}

impl ReprPlan {
    /// Create a new empty `ReprPlan` with the given narrowing policy.
    #[must_use]
    pub fn new(policy: NarrowingPolicy) -> Self {
        Self {
            decisions: FxHashMap::default(),
            repr_attrs: FxHashMap::default(),
            rc_strategies: FxHashMap::default(),
            escape_info: FxHashMap::default(),
            yield_allocations: FxHashMap::default(),
            yield_allocation_sites: FxHashMap::default(),
            length_projection_calls: FxHashMap::default(),
            length_projection_yields: FxHashMap::default(),
            function_var_ranges: FxHashMap::default(),
            field_range_summaries: FxHashMap::default(),
            element_range_summaries: FxHashMap::default(),
            enum_layouts: FxHashMap::default(),
            audit: Vec::new(),
            narrowing_policy: policy,
            pub_type_indices: FxHashSet::default(),
            unconstrained_fn_names: FxHashSet::default(),
            has_analysis_only_functions: false,
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

    /// Query the enum representation for a type.
    ///
    /// Returns `None` if no decision is recorded or the type is not an enum.
    /// This is the canonical query — all consumers should use this instead
    /// of pattern-matching `get_repr()` into `MachineRepr::Enum`.
    #[must_use]
    pub fn enum_repr(&self, idx: Idx) -> Option<&EnumRepr> {
        match self.get_repr(idx)? {
            MachineRepr::Enum(e) => Some(e),
            _ => None,
        }
    }

    /// Record the canonical [`EnumLayoutInfo`] for an enum type.
    pub fn set_enum_layout(&mut self, idx: Idx, info: EnumLayoutInfo) {
        self.enum_layouts.insert(idx, info);
    }

    /// Query the canonical enum layout facts for a type.
    ///
    /// Returns `None` if `idx` is not an enum or no layout was recorded. This
    /// is the canonical query — consumers read layout facts here instead of
    /// recomputing tag/GEP/offset logic ad-hoc.
    #[must_use]
    pub fn enum_layout_info(&self, idx: Idx) -> Option<&EnumLayoutInfo> {
        self.enum_layouts.get(&idx)
    }

    /// Resolve the `EnumRepr` for `idx` — plan-first, with on-the-fly
    /// canonical recomputation for enum-shaped types with variable residue
    /// (e.g., `Option<Var(T resolved to str)>`) that were not in the plan when it
    /// was computed.
    ///
    /// SSOT for the plan-lookup + canonical-fallback ladder. Every consumer
    /// (ABI sizing, type-info layout resolution, ARC emission) routes through
    /// this so a variable-residue enum cannot answer differently across
    /// emission surfaces. The fallback delegates to
    /// [`crate::canonical_enum_for_type`], keeping layout authority here.
    #[must_use]
    pub fn enum_repr_with_fallback<'p>(
        &'p self,
        pool: &Pool,
        idx: Idx,
    ) -> Option<std::borrow::Cow<'p, EnumRepr>> {
        let resolved = pool.resolve_fully(idx);

        if let Some(enum_repr) = self.enum_repr(resolved) {
            return Some(std::borrow::Cow::Borrowed(enum_repr));
        }

        if matches!(
            pool.tag(resolved),
            ori_types::Tag::Option | ori_types::Tag::Result | ori_types::Tag::Enum
        ) {
            if let Some(enum_repr) = crate::canonical::canonical_enum_for_type(pool, resolved) {
                return Some(std::borrow::Cow::Owned(enum_repr));
            }
        }

        None
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

    /// Register type indices that are declared `pub`.
    ///
    /// Public types have ABI contracts with external code — their field
    /// layouts must not be narrowed by integer narrowing.
    pub fn set_pub_type_indices(&mut self, indices: impl IntoIterator<Item = Idx>) {
        self.pub_type_indices.extend(indices);
    }

    /// Check if a type index is declared `pub`.
    ///
    /// Public types must not have their fields narrowed — external callers
    /// may construct them with full-range values.
    #[must_use]
    pub fn is_public_type(&self, idx: Idx) -> bool {
        self.pub_type_indices.contains(&idx)
    }

    /// Register unconstrained function identities (pub, trait impl).
    ///
    /// Each entry is `(Option<self_type_idx>, method_name)`:
    /// - `(None, name)` for pub top-level functions
    /// - `(Some(idx), name)` for trait impl methods on type `idx`
    ///
    /// Unconstrained functions may be called from external code or via
    /// dynamic dispatch — their parameter ranges must not be narrowed
    /// by interprocedural range analysis. Closures are handled
    /// separately via `ArcFunction::num_captures`.
    pub fn set_unconstrained_fn_names(
        &mut self,
        names: impl IntoIterator<Item = (Option<Idx>, Name)>,
    ) {
        self.unconstrained_fn_names.extend(names);
    }

    /// Check if a function is unconstrained (pub or trait impl).
    ///
    /// `self_type` is the first parameter's type for impl methods, or `None`
    /// for top-level functions. This disambiguates same-named methods across
    /// different types.
    ///
    /// A function is unconstrained if:
    /// - It's a pub top-level function: `(None, name)` is in the set, OR
    /// - It's a trait impl method: `(Some(self_type), name)` is in the set
    #[must_use]
    pub fn is_unconstrained_fn(&self, self_type: Option<Idx>, name: Name) -> bool {
        // Exact match only: (None, name) for pub top-level, (Some(idx), name) for
        // trait impl methods. No wildcard fallback — a pub top-level `foo` must NOT
        // make an unrelated impl method `Type.foo` unconstrained.
        self.unconstrained_fn_names.contains(&(self_type, name))
    }

    /// Reports whether an ARC-qualified function has no representation constraints.
    ///
    /// Base (`__impl_<type-hash>_index`) and ordinal-suffixed
    /// (`__impl_<type-hash>_index_1`) names are stored as exact keys.
    #[must_use]
    pub fn is_qualified_unconstrained(&self, qualified_name: Name) -> bool {
        self.unconstrained_fn_names
            .contains(&(None, qualified_name))
    }

    /// Whether integer narrowing is safe to apply at the codegen level.
    ///
    /// Returns `false` when the analysis set includes functions not fully
    /// integrated into the codegen pipeline (e.g., impl methods ARC-lowered
    /// for range analysis only). Their field-range summaries could trigger
    /// narrowing for structs that cross ABI boundaries between the ARC-emitted
    /// path (narrowed) and the `compile_impls` path (canonical), causing
    /// layout mismatches.
    ///
    /// When no analysis-only functions are present, narrowing is safe because
    /// all analyzed functions go through the same codegen path.
    #[must_use]
    pub fn is_narrowing_safe_for_codegen(&self) -> bool {
        !self.has_analysis_only_functions
    }

    /// Mark that the analysis set includes functions not fully integrated
    /// into the codegen pipeline.
    ///
    /// When set, integer/float narrowing and per-variable range storage are
    /// suppressed to prevent ABI-mismatched struct layouts.
    pub fn set_has_analysis_only_functions(&mut self) {
        self.has_analysis_only_functions = true;
    }

    /// Record per-function escape analysis info.
    pub fn set_escape_info(&mut self, func: Name, info: EscapeInfo) {
        self.escape_info.insert(func, info);
    }

    /// Record an RC strategy decision for a type.
    /// Stores the strategy in a **separate map** so the type's `MachineRepr`
    /// layout is preserved. The audit trail records the decision for debugging.
    pub fn set_rc_strategy(&mut self, idx: Idx, strategy: RcStrategy, source: DecisionSource) {
        let reason = match strategy {
            RcStrategy::None => DecisionReason::TransitivelyTrivial,
            RcStrategy::NonAtomic { .. } => DecisionReason::Custom("thread-local".into()),
            RcStrategy::Atomic { .. } => DecisionReason::Canonical,
        };
        self.rc_strategies.insert(idx, strategy);
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

    /// Yields decided type indices in the plan's deterministic map order.
    pub fn decision_indices(&self) -> impl Iterator<Item = Idx> + '_ {
        self.decisions.keys().copied()
    }

    /// Dump the audit trail for debugging.
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

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ReprPlan>();
};
