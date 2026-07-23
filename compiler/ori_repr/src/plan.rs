//! `ReprPlan` — the central decision document for representation optimization.
//!
//! Optimization passes record narrowing decisions with provenance; codegen
//! reads the closed plan. Missing decisions stay canonical, and only
//! AIMS-proven local identities admit local allocation mechanisms.

mod decision;
pub(crate) mod query;
mod range_plan;
mod repr_attr;
mod yield_plan;

use ori_arc::ir::{AllocationSiteId, ArcVarId, YieldExtent};
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::enum_repr::EnumRepr;
use crate::escape::EscapeInfo;
use crate::layout::EnumLayoutInfo;
use crate::range::ValueRange;
use crate::repr::MachineRepr;

pub use self::decision::{DecisionReason, DecisionSource, ReprDecision};
pub use self::query::{NarrowingPolicy, RcStrategy};
pub use self::repr_attr::ReprAttribute;
pub(crate) use yield_plan::YieldLineageRuntimeCall;

/// Concrete storage mechanism selected for one yield allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompiledAllocationMechanism {
    /// Existing growable RC-managed runtime allocation.
    RuntimeHeap {
        /// Proven allocation extent, when available.
        extent: YieldExtent,
    },
    /// Bounded function-lifetime storage with a runtime-compatible header.
    ManagedStack {
        /// Exact element capacity.
        capacity: u64,
    },
    /// Bounded function-lifetime storage without a runtime header.
    CompactStack {
        /// Exact element capacity.
        capacity: u64,
    },
}

/// Representation-layer allocation projection for one stable site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompiledAllocationDecision {
    /// Stable identity of the source allocation.
    pub site: AllocationSiteId,
    /// ARC variable holding the allocation builder.
    pub builder: ArcVarId,
    /// ARC variable holding the completed collection.
    pub result: ArcVarId,
    /// Element type stored by the allocation.
    pub elem_ty: Idx,
    /// Physical size of one element in bytes.
    pub elem_size: u64,
    /// Physical storage selected for the allocation.
    pub mechanism: CompiledAllocationMechanism,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum YieldAllocationIdentity {
    Builder(ArcVarId),
    Result(ArcVarId),
}

/// Representation and narrowing decisions consumed by physical backends.
/// [`compute_repr_plan()`] builds the plan imperatively after type checking;
/// codegen reads it without mutation. Every compilation and JIT invocation
/// recomputes it. Plain collections provide immutable `Send + Sync` access
/// without interior mutability.
///
/// [`compute_repr_plan()`]: crate::compute_repr_plan
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
    /// `MachineRepr`.
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
    /// Key: function `Name` maps to (`ArcVarId`, `ValueRange`) entries.
    function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
    /// Per-type-field range summaries from field-summary analysis.
    ///
    /// Key: `(struct/tuple Idx, field_index)` maps to the joined `ValueRange`.
    field_range_summaries: FxHashMap<(Idx, u32), ValueRange>,
    /// Per-collection-type element range summaries from element analysis.
    ///
    /// Key: collection type `Idx` (e.g., `[int]`) maps to the joined `ValueRange` of
    /// all observed element values across `Construct(ListLiteral)` and
    /// `CollectionReuse` sites.
    element_range_summaries: FxHashMap<Idx, ValueRange>,
    /// Canonical enum layout facts, keyed by enum type `Idx`.
    ///
    /// Each entry is derived from the final `EnumRepr` after all representation
    /// optimization passes.
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
    /// Function identities whose parameters remain unconstrained by range analysis.
    /// `None` identifies public top-level functions; `Some(idx)` qualifies trait
    /// methods by receiver type. Closures use `ArcFunction::num_captures`.
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

impl CompiledAllocationDecision {
    /// Maximum element bytes admitted to function-lifetime stack storage.
    pub const MAX_LOCAL_BYTES: u64 = 4 * 1024;
}

impl CompiledAllocationMechanism {
    /// Proven allocation extent.
    #[must_use]
    pub const fn extent(self) -> YieldExtent {
        match self {
            Self::RuntimeHeap { extent } => extent,

            Self::ManagedStack { capacity } | Self::CompactStack { capacity } => {
                YieldExtent::StaticExact(capacity)
            }
        }
    }

    /// Whether the allocation is emitted in the owning stack frame.
    #[must_use]
    pub const fn is_stack(self) -> bool {
        match self {
            Self::RuntimeHeap { .. } => false,
            Self::ManagedStack { .. } | Self::CompactStack { .. } => true,
        }
    }

    /// Whether storage retains the runtime-compatible header.
    #[must_use]
    pub const fn requires_runtime_header(self) -> bool {
        match self {
            Self::RuntimeHeap { .. } | Self::ManagedStack { .. } => true,
            Self::CompactStack { .. } => false,
        }
    }
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
    /// fall back to the canonical `TypeInfoStore` layout.
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

    /// Resolves `idx` from the plan, then canonically recomputes enum-shaped
    /// types with unresolved residue. ABI, type-info, and ARC consumers share
    /// this fallback so they cannot select different layouts.
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

    /// Registers public and trait-method identities whose parameter ranges must
    /// remain unconstrained. `None` identifies top-level functions; `Some(idx)`
    /// identifies methods by receiver type. Closures use `ArcFunction::num_captures`.
    pub fn set_unconstrained_fn_names(
        &mut self,
        names: impl IntoIterator<Item = (Option<Idx>, Name)>,
    ) {
        self.unconstrained_fn_names.extend(names);
    }

    /// Reports whether the exact top-level or receiver-qualified identity is
    /// externally unconstrained. `self_type` disambiguates same-named methods.
    #[must_use]
    pub fn is_unconstrained_fn(&self, self_type: Option<Idx>, name: Name) -> bool {
        // Why: Receiver qualification keeps public top-level names from constraining methods.
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

    /// Reports whether every analyzed function shares the narrowing-aware
    /// codegen path. Analysis-only functions make field summaries unsafe for
    /// ABI-crossing layout decisions.
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
            assert!(
                writeln!(out, "[{i}] {tag:?} <- {:?}: {:?}", d.source, d.reason).is_ok(),
                "writing ReprPlan audit text to String cannot fail"
            );
        }
        out
    }
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ReprPlan>();
};
