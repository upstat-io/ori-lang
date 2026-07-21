//! `ReprPlan` — the central decision document for representation optimization.
//!
//! Each optimization pass writes narrowing decisions into the
//! `ReprPlan` with full provenance tracking. Codegen reads the final
//! plan to determine the machine representation of every type.
//!
//! # Design
//!
//! - **Audit trail**: every decision is recorded in order, even when
//!   overridden — useful for debugging why a type was narrowed.
//! - **Safe defaults**: queries return canonical (un-narrowed) values
//!   when no decision has been recorded — the canonical pass alone causes zero
//!   behavioral change.
//! - **Safe escape defaults**: only AIMS-proven local identities are eligible
//!   for a compiled local allocation mechanism.

use ori_arc::ir::{
    AllocationSiteId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
    YieldAllocationExecution, YieldAllocationFact, YieldAllocationLocality, YieldExtent,
};
use ori_arc::ArcClassification;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::enum_repr::EnumRepr;
use crate::escape::EscapeInfo;
use crate::layout::EnumLayoutInfo;
use crate::range::ValueRange;
use crate::repr::MachineRepr;

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

/// Maximum element bytes admitted to function-lifetime stack storage.
pub const MAX_LOCAL_YIELD_BYTES: u64 = 4 * 1024;

mod decision;
pub(crate) mod query;
mod repr_attr;

/// The central data structure recording all narrowing decisions.
///
/// Computed after type checking and before LLVM codegen. The type checker
/// never sees `ReprPlan`; codegen reads from it but never writes.
///
/// # Salsa Integration
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
    /// Empty until escape analysis populates it.
    escape_info: FxHashMap<Name, EscapeInfo>,
    /// Compiled allocation projections keyed by function and stable site.
    yield_allocations: FxHashMap<(Name, AllocationSiteId), CompiledAllocationDecision>,
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
    pub fn get_enum_repr(&self, idx: Idx) -> Option<&EnumRepr> {
        match self.get_repr(idx)? {
            MachineRepr::Enum(e) => Some(e),
            _ => None,
        }
    }

    /// Record the canonical [`EnumLayoutInfo`] for an enum type.
    ///
    /// Written by the `populate_enum_layouts` pass after all repr-optimization
    /// passes have finalized the type's `EnumRepr`.
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

        if let Some(enum_repr) = self.get_enum_repr(resolved) {
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

    /// Record per-variable range analysis results for a function.
    ///
    /// Called by range analysis after `range_fixpoint()` completes for a function.
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
    /// Used by interprocedural propagation to merge parameter ranges into
    /// existing intraprocedural results.
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

    /// Join an element range into the persistent summary for a collection type.
    ///
    /// Called by `ElementSummaryTable::flush_to_repr_plan()` after the
    /// fixpoint completes for each function. Multiple functions accumulate
    /// evidence by joining (not overwriting).
    pub fn join_element_range(&mut self, collection_idx: Idx, range: ValueRange) {
        self.element_range_summaries
            .entry(collection_idx)
            .and_modify(|existing| *existing = existing.join(range))
            .or_insert(range);
    }

    /// Query the aggregated element range for a collection type.
    ///
    /// Returns `Top` if no construction sites were observed for this collection.
    #[must_use]
    pub fn element_range(&self, collection_idx: Idx) -> ValueRange {
        self.element_range_summaries
            .get(&collection_idx)
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

    /// Check if this specific function (by its ARC-lowered name) is unconstrained.
    ///
    /// Used for analysis-only ARC functions with type-qualified names
    /// Both base names (`__impl_42_index`) and
    /// ordinal-suffixed names (`__impl_42_index_1`) are registered by
    /// `collect_unconstrained_fn_names()`, so exact match is sufficient
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

    /// Freeze AIMS yield facts into conservative compiled allocation choices.
    pub fn freeze_yield_allocations(
        &mut self,
        facts_by_function: &FxHashMap<Name, Vec<YieldAllocationFact>>,
    ) {
        self.yield_allocations.clear();
        for (&function, facts) in facts_by_function {
            for fact in facts {
                let mechanism = select_yield_mechanism(*fact);
                self.yield_allocations.insert(
                    (function, fact.site),
                    CompiledAllocationDecision {
                        site: fact.site,
                        builder: fact.builder,
                        result: fact.result,
                        elem_ty: fact.elem_ty,
                        elem_size: fact.elem_size,
                        extent: fact.extent,
                        mechanism,
                        requires_runtime_header: true,
                    },
                );
            }
        }
    }

    /// Select compact backing only from closed executable call identities.
    ///
    /// ARC contributes neutral extent, locality, execution, and lineage facts.
    /// This compiled-plan step owns the physical header decision and therefore
    /// evaluates the complete realized use set after exact call closure.
    pub(crate) fn close_yield_runtime_header_requirements(
        &mut self,
        functions: &[ArcFunction],
        pool: &Pool,
        mut runtime_call: impl FnMut(Name, ArcVarId) -> Option<YieldLineageRuntimeCall>,
    ) {
        let functions_by_name: FxHashMap<_, _> = functions
            .iter()
            .map(|function| {
                (
                    function.name,
                    (function, ori_arc::YieldLineageIndex::for_function(function)),
                )
            })
            .collect();
        let decisions: Vec<_> = self
            .yield_allocations
            .iter()
            .map(|(&key, &decision)| (key, decision))
            .collect();
        for (key, decision) in decisions {
            let elidable =
                functions_by_name
                    .get(&key.0)
                    .is_some_and(|(function, yield_lineages)| {
                        yield_runtime_header_is_elidable(
                            function,
                            yield_lineages,
                            decision,
                            pool,
                            &mut runtime_call,
                        )
                    });
            if elidable {
                self.yield_allocations
                    .get_mut(&key)
                    .unwrap_or_else(|| unreachable!("compiled allocation disappeared"))
                    .requires_runtime_header = false;
            }
        }
    }

    /// Query the allocation projection by the scratch builder identity.
    #[must_use]
    pub fn yield_allocation_for_builder(
        &self,
        function: Name,
        builder: ArcVarId,
    ) -> Option<CompiledAllocationDecision> {
        self.yield_allocations
            .iter()
            .find_map(|(&(owner, _), decision)| {
                (owner == function && decision.builder == builder).then_some(*decision)
            })
    }

    /// Query the allocation projection by the final list result identity.
    #[must_use]
    pub fn yield_allocation_for_result(
        &self,
        function: Name,
        result: ArcVarId,
    ) -> Option<CompiledAllocationDecision> {
        self.yield_allocations
            .iter()
            .find_map(|(&(owner, _), decision)| {
                (owner == function && decision.result == result).then_some(*decision)
            })
    }

    /// Freeze the closed program's physical length-only clone decisions.
    pub(crate) fn set_length_projections(
        &mut self,
        calls: FxHashMap<(Name, ArcVarId), Name>,
        yields: FxHashMap<Name, ArcVarId>,
    ) {
        self.length_projection_calls = calls;
        self.length_projection_yields = yields;
    }

    /// Iterate qualified call-site redirects selected by the compiled plan.
    pub fn length_projection_calls(&self) -> impl Iterator<Item = ((Name, ArcVarId), Name)> + '_ {
        self.length_projection_calls
            .iter()
            .map(|(&site, &callee)| (site, callee))
    }

    /// Iterate qualified callees and their virtualized yield result.
    pub fn length_projection_yields(&self) -> impl Iterator<Item = (Name, ArcVarId)> + '_ {
        self.length_projection_yields
            .iter()
            .map(|(&callee, &result)| (callee, result))
    }

    /// Record an RC strategy decision for a type.
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
    /// Used by narrowing passes to find struct/tuple types to narrow
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YieldLineageRuntimeCall {
    BorrowedRead,
    StaticUniqueListSet,
}

#[derive(Clone, Copy)]
struct YieldLineageCall<'a> {
    args: &'a [ArcVarId],
    arg_ownership: &'a [ArgOwnership],
    dst: ArcVarId,
    position: (usize, usize),
}

fn yield_runtime_header_is_elidable(
    function: &ArcFunction,
    yield_lineages: &ori_arc::YieldLineageIndex,
    decision: CompiledAllocationDecision,
    pool: &Pool,
    runtime_call: &mut impl FnMut(Name, ArcVarId) -> Option<YieldLineageRuntimeCall>,
) -> bool {
    use ori_registry::TypeTag;

    let classifier = ori_arc::ArcClassifier::new(pool);
    if decision.mechanism != CompiledAllocationMechanism::StackSlot
        || !classifier.is_scalar(decision.elem_ty)
        || !matches!(
            classifier.builtin_type_tag(decision.elem_ty),
            Some(
                TypeTag::Int
                    | TypeTag::Float
                    | TypeTag::Bool
                    | TypeTag::Char
                    | TypeTag::Byte
                    | TypeTag::Unit
                    | TypeTag::Never
                    | TypeTag::Duration
                    | TypeTag::Size
                    | TypeTag::Ordering
            )
        )
    {
        return false;
    }

    let in_lineage = |var| {
        yield_lineages
            .result_for_receiver(var)
            .is_some_and(|result| result == decision.result)
    };
    for (block_idx, block) in function.blocks.iter().enumerate() {
        for (instr_idx, instruction) in block.body.iter().enumerate() {
            if !instruction.used_vars().iter().copied().any(&in_lineage) {
                continue;
            }
            let allowed = match instruction {
                ArcInstr::Let {
                    value: ori_arc::ir::ArcValue::Var(source),
                    ..
                } => in_lineage(*source),
                ArcInstr::Project {
                    value, field: 0, ..
                } => in_lineage(*value),
                ArcInstr::RcDec { var, .. } => in_lineage(*var),
                ArcInstr::Apply {
                    dst,
                    args,
                    arg_ownership,
                    ..
                } => yield_lineage_call_is_header_independent(
                    function,
                    YieldLineageCall {
                        args,
                        arg_ownership,
                        dst: *dst,
                        position: (block_idx, instr_idx),
                    },
                    runtime_call(function.name, *dst),
                    &in_lineage,
                ),
                _ => false,
            };
            if !allowed {
                return false;
            }
        }

        if !block
            .terminator
            .used_vars()
            .iter()
            .copied()
            .any(&in_lineage)
        {
            continue;
        }
        let allowed = match &block.terminator {
            ArcTerminator::Jump { .. } => true,
            ArcTerminator::Invoke {
                dst,
                args,
                arg_ownership,
                ..
            } => yield_lineage_call_is_header_independent(
                function,
                YieldLineageCall {
                    args,
                    arg_ownership,
                    dst: *dst,
                    position: (block_idx, block.body.len()),
                },
                runtime_call(function.name, *dst),
                &in_lineage,
            ),
            _ => false,
        };
        if !allowed {
            return false;
        }
    }
    true
}

fn yield_lineage_call_is_header_independent(
    function: &ArcFunction,
    call: YieldLineageCall<'_>,
    operation: Option<YieldLineageRuntimeCall>,
    in_lineage: &impl Fn(ArcVarId) -> bool,
) -> bool {
    if !call.args.first().copied().is_some_and(in_lineage)
        || call.args.iter().skip(1).copied().any(in_lineage)
    {
        return false;
    }
    match operation {
        Some(YieldLineageRuntimeCall::BorrowedRead) => {
            call.arg_ownership.first() == Some(&ArgOwnership::Borrowed)
        }
        Some(YieldLineageRuntimeCall::StaticUniqueListSet) => {
            in_lineage(call.dst)
                && function
                    .cow_annotations
                    .get(call.position.0, call.position.1)
                    == ori_arc::CowMode::StaticUnique
        }
        None => false,
    }
}

fn select_yield_mechanism(fact: YieldAllocationFact) -> CompiledAllocationMechanism {
    let YieldExtent::StaticExact(capacity) = fact.extent else {
        return CompiledAllocationMechanism::RuntimeHeap;
    };
    let Some(bytes) = capacity.checked_mul(fact.elem_size.max(1)) else {
        return CompiledAllocationMechanism::RuntimeHeap;
    };
    if fact.locality == YieldAllocationLocality::Local
        && fact.execution == YieldAllocationExecution::SingleExecution
        && bytes <= MAX_LOCAL_YIELD_BYTES
    {
        CompiledAllocationMechanism::StackSlot
    } else {
        CompiledAllocationMechanism::RuntimeHeap
    }
}

// Thread safety: compile-time assertion that ReprPlan is Send + Sync.
// ReprPlan has no interior mutability (no RefCell, no Mutex), so &ReprPlan
// can be safely shared across threads. This assertion catches regressions
// if a future field introduces non-Send/Sync types.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ReprPlan>();
};
