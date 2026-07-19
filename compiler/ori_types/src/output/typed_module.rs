//! Type-checked module output.
//!
//! The complete typed IR for a module: per-expression types, function
//! signatures, resolved patterns, monomorphization instances, and the
//! cross-module export sidecars (type descriptors, repr metadata).

mod impl_methods;
mod sidecars;

pub use impl_methods::{ImplMethodId, ImplMethodRole, ImplSig, ImportedImplSig};
pub use sidecars::{
    AssignDesugar, AssignStepRoute, CapabilityCallSite, CapabilityProvider,
    CapabilityProviderSource, ExportedTypeMetadata, FormatSpecTypes, IterMethodRoute,
};

use ori_ir::{ExprId, Name, PatternKey, PatternResolution, SparseSideTable};

use crate::pool::TypeDescriptor;
use crate::registry::burden::UserBurdenSpec;
use crate::registry::TypeEntry;
use crate::{Idx, TypeCheckError, TypeCheckWarning};

use super::mono::{MonoInstance, MonoInstanceId};
use super::sig::FunctionSig;

/// Type-checked module.
///
/// Contains all type information computed by the inference engine.
/// Uses `Idx` for O(1) type comparisons via the unified Pool.
///
/// # Salsa Compatibility
///
/// Derives all traits required for Salsa query results.
///
/// # Example
///
/// ```rust
/// use ori_types::{Idx, TypedModule};
///
/// let mut typed = TypedModule::new();
/// typed.expr_types.push(Idx::INT);
/// assert_eq!(typed.expr_type(0), Some(Idx::INT));
/// assert_eq!(typed.expr_count(), 1);
/// assert!(!typed.has_errors());
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct TypedModule {
    /// Type of each expression, indexed by expression ID.
    ///
    /// This is stored as a Vec for O(1) access. Expression IDs are
    /// sequential starting from 0 in each module.
    pub expr_types: Vec<Idx>,

    /// Function signatures by name.
    ///
    /// Sorted by name for deterministic output.
    pub functions: Vec<FunctionSig>,

    /// User-defined type definitions (structs, enums, newtypes, aliases).
    ///
    /// Exported from the module's `TypeRegistry` for cross-module type
    /// resolution. Sorted by name (from `BTreeMap` iteration order).
    pub types: Vec<TypeEntry>,

    /// Type errors accumulated during type checking.
    pub errors: Vec<TypeCheckError>,

    /// Type warnings accumulated during type checking.
    ///
    /// Warnings indicate suspicious but valid code (e.g., infinite iterator
    /// consumed without `.take()`). They do not prevent compilation.
    pub warnings: Vec<TypeCheckWarning>,

    /// Resolved patterns: `Binding` names disambiguated to unit variants.
    ///
    /// Sorted by `PatternKey` for O(log n) binary search via `resolve_pattern()`.
    /// Only patterns that were resolved are stored — unresolved bindings are
    /// normal variable bindings and have no entry.
    pub pattern_resolutions: SparseSideTable<PatternKey, PatternResolution>,

    /// Impl method signatures for codegen.
    ///
    /// Each entry maps an impl method's owning receiver type + name to its
    /// resolved `FunctionSig`. Codegen needs these to compute ABI (calling
    /// convention, sret, parameter passing) for impl methods (compiled
    /// separately from top-level functions) AND to key mono-collection dispatch
    /// on the owning receiver rather than the first value param.
    pub impl_sigs: Vec<ImplSig>,

    /// Imported impl templates reachable through this module's imports.
    ///
    /// These are semantic lookup/codegen inputs, not locally-owned bodies.
    pub imported_impl_sigs: Vec<ImportedImplSig>,

    /// Derived implementations accepted by validation and coherence.
    ///
    /// This is the sole authority for compiler-generated derive
    /// bodies. Raw source attributes are not an executable inventory.
    pub accepted_derives: Vec<super::AcceptedDerivedImpl>,

    /// Concrete nested-call selections for accepted generated bodies.
    ///
    /// Each plan is frozen while trait coherence and concrete substitutions
    /// coexist. Realization consumes these facts without redispatching by
    /// receiver shape or method spelling.
    pub derived_call_plans: Vec<super::DerivedCallPlan>,

    /// Trait impl method identities: `(self_type_idx, method_name)`.
    ///
    /// Trait impl methods may be called via dynamic dispatch with unknown
    /// arguments — their parameter ranges must stay Top in
    /// interprocedural range analysis. Inherent impl methods are NOT
    /// included (they have known call sites).
    ///
    /// The `Idx` is the self-type index (e.g., `Idx` for `Color` in
    /// `impl Color: Printable { @to_str }`), disambiguating same-named
    /// methods across different types.
    pub trait_impl_fn_names: Vec<(Idx, Name)>,

    /// Monomorphization instances discovered during type checking.
    ///
    /// Each entry represents one unique concrete generic callable identity.
    /// Most originate at call sites; operator-selected generic methods also
    /// appear in this list without a corresponding `mono_dispatch_map` entry. Physical
    /// backends use the inventory to stamp out concrete specializations.
    pub mono_instances: Vec<MonoInstance>,

    /// Map from AST `ExprId` of a generic call site to its resolved
    /// monomorphic instance index.
    ///
    /// Populated during inference at every `Call` / `MethodCall` site that
    /// resolves to a generic function or method instantiated with concrete
    /// arguments at this call site. `ori_canon` reads this side-table while
    /// lowering each `CanExpr::Call` / `CanExpr::MethodCall` and writes the
    /// resolved [`MonoInstanceId`] onto the canonical-IR node, from where
    /// it propagates through ARC IR (`ArcInstr::Apply` / `ArcTerminator::Invoke`)
    /// to LLVM dispatch + `ori_eval` dispatch — the same-index handoff shape
    /// preserves dual-execution parity per.
    ///
    /// Key is the AST `ExprId` of the call expression itself (NOT the
    /// receiver, NOT a sub-argument). Value is an index into
    /// [`Self::mono_instances`].
    ///
    /// Stored as `Vec<(ExprId, MonoInstanceId)>` sorted by `ExprId` (NOT
    /// `FxHashMap`) for Salsa compatibility — the
    /// `TypedModule` struct derives `Eq + Hash` and `FxHashMap` cannot
    /// satisfy them. Lookup is binary search via
    /// `dispatch_map_lookup()`.
    ///
    /// Empty for non-generic call sites — only populated when a generic
    /// callee is instantiated with concrete arguments at the call site.
    pub mono_dispatch_map: SparseSideTable<ExprId, MonoInstanceId>,

    /// Exact semantic method producers selected at source call sites.
    ///
    /// Canonical IR carries only a [`crate::MethodProducerId`] into this dense
    /// table, keeping the leaf `ori_ir` crate independent of type checking.
    /// Realization resolves the handle before generic/local/imported callable
    /// target closure; no backend reconstructs a producer from method spelling.
    pub method_producers: Vec<crate::MethodProducer>,

    /// Exact dispatch route selected for every index expression.
    ///
    /// Selected routes index [`Self::method_producers`]; builtin, deferred, and
    /// invalid routes remain explicit through canonical and ARC lowering.
    pub index_dispatch_map: SparseSideTable<ExprId, ori_ir::canon::IndexDispatch>,

    /// Ordered provider selections for capability-bearing free calls.
    ///
    /// Canon consumes this sidecar to append source-erased implicit provider
    /// arguments. The call's matching `mono_dispatch_map` entry selects code
    /// specialized by provider type, while this table preserves the lexical
    /// provider value passed by that particular call site.
    pub capability_call_map: SparseSideTable<ExprId, CapabilityCallSite>,

    /// Type-directed desugar plans for `ExprKind::AssignTarget` chains.
    ///
    /// Each entry maps the AST `ExprId` of an `AssignTarget` node to its
    /// [`AssignDesugar`] plan (the resolved receiver-read type at every chain
    /// level). `ori_canon` reads this side-table while lowering each
    /// `ExprKind::AssignTarget`, synthesizing the pure-reassignment form
    /// (`root = root.updated(...)` / `root = { ...root, f: v }`) so AIMS never
    /// sees a `CanExpr::Assign { target: Index/Field }`.
    ///
    /// Stored as `Vec<(ExprId, AssignDesugar)>` sorted by `ExprId` (NOT
    /// `FxHashMap`) for Salsa compatibility — the
    /// `TypedModule` struct derives `Eq + Hash` and `FxHashMap` cannot satisfy
    /// them. Lookup is
    /// binary search via [`Self::resolve_assign_desugar`]. Keys are
    /// module-wide AST `ExprId`s (the arena is one-per-module), so body-pass
    /// accumulation extends without re-anchoring.
    ///
    /// Empty when no index/field assignment appears in the module.
    pub assign_desugar_map: SparseSideTable<ExprId, AssignDesugar>,

    /// Module-alias qualified-call rewrite targets, keyed by call `ExprId`.
    ///
    /// Each entry maps the AST `ExprId` of a `MethodCall` / `MethodCallNamed`
    /// that resolved to a module-alias qualified call (`alias.func(args)`) to
    /// the qualified imported-function `Name` (`"alias.func"`). `ori_canon`
    /// reads this side-table at both `lower_method_call` (positional) and
    /// `desugar_method_call_named` (named) and rewrites the namespace
    /// `CanExpr::MethodCall` into a free `CanExpr::Call { func: FunctionRef }`
    /// so both backends see one shape and the receiver namespace is never
    /// threaded as a `self` argument. Sorted `Vec<(ExprId, Name)>` like
    /// `assign_desugar_map` (Salsa `Eq + Hash`). Lookup via
    /// [`Self::resolve_module_alias_call`]. Empty when no alias-qualified call
    /// appears in the module.
    pub module_alias_call_map: SparseSideTable<ExprId, Name>,

    /// Iterable->Iterator routed method calls, keyed by the exact source call
    /// `ExprId`. `ori_canon` reads each type-directed route at
    /// `lower_method_call` / `desugar_method_call_named` and rewrites
    /// `recv.method(args)` through a materialized `recv.iter()` node. An eager
    /// adapter route additionally synthesizes the terminal `.collect()` used by
    /// Annex C's direct `Range.map` / `Range.filter` list semantics. Lookup via
    /// [`Self::resolve_iter_route`].
    pub iter_route_map: SparseSideTable<ExprId, IterMethodRoute>,

    /// Portable type descriptors for all types referenced in exported signatures.
    ///
    /// Topologically sorted: leaves first. Each entry is `(merkle_hash, descriptor)`.
    /// Importing modules can reconstruct any exported type from these descriptors
    /// without accessing the originating Pool or AST.
    ///
    /// Only includes types from public function signatures to minimize size.
    pub type_descriptors: Vec<(u64, TypeDescriptor)>,

    /// Repr and visibility metadata for user-defined types in this module.
    ///
    /// Enables importing modules to correctly exempt `pub` and `#repr(...)`
    /// types from integer narrowing when building their `ReprPlan`.
    /// Only includes types with non-default metadata (repr attribute or public).
    pub exported_type_metadata: Vec<ExportedTypeMetadata>,

    /// Merkle hashes of collection types (List, Set) reachable from public
    /// function signatures.
    ///
    /// Enables importing modules to protect collection element layouts from
    /// narrowing when the collection appears in an exported ABI surface.
    /// Merged with imported collection surfaces for transitive forwarding
    /// (A→B→C propagation).
    pub exported_collection_surfaces: Vec<u64>,

    /// Per-instance burden specs for monomorphized generic-builtin collection
    /// instances (`[T]`, `{K: V}`, `Set<T>`, `Option<T>`, `Result<T, E>`,
    /// `Range<T>`) that carry no nominal `TypeEntry`.
    ///
    /// The in-memory `TypeRegistry` stores these instances in its
    /// `collection_burdens` side-table, which `types` excludes because
    /// `TypeRegistry::into_entries` contains only nominal entries. Storing these
    /// specs preserves collection-instance burden for ARC emission. Sorted ascending
    /// by `Idx` for Salsa-deterministic output.
    /// Spec: Annex E §AIMS.
    pub collection_burdens: Vec<(Idx, UserBurdenSpec)>,

    /// Self-type `Idx` values carrying an explicit `impl T: Formattable`.
    ///
    /// The blanket `impl<T: Printable> T: Formattable` is NOT registered, so
    /// this set contains ONLY types with a user-written `Formattable` impl.
    /// `ori_canon` queries `has_formattable_impl` while desugaring a
    /// non-primitive `{expr:spec}` interpolation to decide between a
    /// `Formattable.format(self:, spec:)` `MethodCall` (explicit impl present)
    /// and a `to_str()` + `FormatWith` re-route (Printable-only).
    /// Sorted ascending for binary-search lookup.
    /// Spec: Clause 14 (string interpolation) + Clause 9 (Printable/Formattable).
    pub formattable_impl_types: Vec<Idx>,

    /// Pool idxs for the builtin `FormatSpec` struct + its `Option<_>` field
    /// types, for typing the synthesized `FormatSpec` struct in `ori_canon`.
    /// `None` only in synthetic/empty modules where builtin registration ran
    /// without a pool (isolated tests); a real check always populates it.
    pub format_spec_types: Option<FormatSpecTypes>,
}

impl TypedModule {
    /// Create a new empty typed module.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a typed module with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(expr_count: usize, function_count: usize) -> Self {
        Self {
            expr_types: Vec::with_capacity(expr_count),
            functions: Vec::with_capacity(function_count),
            types: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: SparseSideTable::new(),
            impl_sigs: Vec::new(),
            imported_impl_sigs: Vec::new(),
            accepted_derives: Vec::new(),
            derived_call_plans: Vec::new(),
            trait_impl_fn_names: Vec::new(),
            mono_instances: Vec::new(),
            mono_dispatch_map: SparseSideTable::new(),
            method_producers: Vec::new(),
            index_dispatch_map: SparseSideTable::new(),
            capability_call_map: SparseSideTable::new(),
            assign_desugar_map: SparseSideTable::new(),
            module_alias_call_map: SparseSideTable::new(),
            iter_route_map: SparseSideTable::new(),
            type_descriptors: Vec::new(),
            exported_type_metadata: Vec::new(),
            exported_collection_surfaces: Vec::new(),
            collection_burdens: Vec::new(),
            formattable_impl_types: Vec::new(),
            format_spec_types: None,
        }
    }

    /// Check if this module has type errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the type of an expression by index.
    ///
    /// Returns `None` if the expression index is out of bounds.
    #[must_use = "the absence of a value must be handled"]
    pub fn expr_type(&self, expr_index: usize) -> Option<Idx> {
        self.expr_types.get(expr_index).copied()
    }

    /// Get a function signature by name.
    #[must_use = "the absence of a value must be handled"]
    pub fn function(&self, name: Name) -> Option<&FunctionSig> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Get a type definition by name.
    #[must_use = "the absence of a value must be handled"]
    pub fn type_def(&self, name: Name) -> Option<&TypeEntry> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Get the number of type definitions.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Get the number of typed expressions.
    pub fn expr_count(&self) -> usize {
        self.expr_types.len()
    }

    /// Get the number of functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Look up a pattern resolution by key.
    ///
    /// Returns `Some(&PatternResolution)` if the pattern was resolved to a
    /// unit variant, `None` if it's a normal variable binding.
    ///
    /// Uses O(log n) binary search on the sorted `pattern_resolutions` table.
    #[must_use = "the absence of a value must be handled"]
    pub fn resolve_pattern(&self, key: PatternKey) -> Option<&PatternResolution> {
        self.pattern_resolutions.get(key)
    }

    /// Look up the type-directed desugar plan for an `AssignTarget` `ExprId`.
    ///
    /// Returns `Some(&AssignDesugar)` if the type checker recorded a plan for
    /// this `AssignTarget` node, `None` otherwise. Uses O(log n) binary search
    /// on the sorted `assign_desugar_map`.
    #[must_use = "the absence of a value must be handled"]
    pub fn resolve_assign_desugar(&self, key: ExprId) -> Option<&AssignDesugar> {
        self.assign_desugar_map.get(key)
    }

    /// Look up the type-directed Iterable->Iterator route for a method call,
    /// keyed by the exact source call `ExprId`.
    ///
    /// Returns the materialized iterator type and, for eager Range adapters,
    /// the intermediate adapter type that must be collected.
    #[must_use = "the absence of a value must be handled"]
    pub fn resolve_iter_route(&self, key: ExprId) -> Option<IterMethodRoute> {
        self.iter_route_map.get(key).copied()
    }

    /// Resolve the frozen capability-provider selection for one source call.
    #[must_use = "the absence of a value must be handled"]
    pub fn resolve_capability_call(&self, key: ExprId) -> Option<&CapabilityCallSite> {
        self.capability_call_map.get(key)
    }

    /// Look up the qualified imported-function `Name` a module-alias qualified
    /// call (`alias.func(args)`) at this call `ExprId` rewrites to.
    ///
    /// Returns `Some(name)` when the type checker recorded this call as an
    /// alias-qualified call, `None` otherwise. `ori_canon` rewrites the
    /// namespace `MethodCall` to a free `CanExpr::Call { func: FunctionRef(name) }`
    /// on a hit. O(log n) binary search on `module_alias_call_map`.
    #[must_use = "the absence of a value must be handled"]
    pub fn resolve_module_alias_call(&self, key: ExprId) -> Option<Name> {
        self.module_alias_call_map.get(key).copied()
    }

    /// Whether `ty` carries an explicit `impl T: Formattable`.
    ///
    /// Returns `false` for the blanket-Printable case (no registered impl).
    /// Uses O(log n) binary search on the sorted `formattable_impl_types` vec.
    pub fn has_formattable_impl(&self, ty: Idx) -> bool {
        self.formattable_impl_types
            .binary_search_by_key(&ty.raw(), |i| i.raw())
            .is_ok()
    }
}
