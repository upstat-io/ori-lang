//! Type-checked module output.
//!
//! The complete typed IR for a module: per-expression types, function
//! signatures, resolved patterns, monomorphization instances, and the
//! cross-module export sidecars (type descriptors, repr metadata).

use ori_ir::{ExprId, Name, PatternKey, PatternResolution, ReprAttrKind};

use crate::pool::TypeDescriptor;
use crate::registry::burden::UserBurdenSpec;
use crate::registry::TypeEntry;
use crate::{Idx, TypeCheckError, TypeCheckWarning};

use super::mono::{MonoInstance, MonoInstanceId};
use super::sig::FunctionSig;

/// Per-type metadata exported for cross-module repr plan construction.
///
/// Carries `#repr` attributes and visibility information alongside the
/// Merkle hash that identifies the type in the Pool. This metadata is
/// NOT part of the type's structural identity (hash) — it is source-level
/// information needed by `ori_repr` to correctly exempt imported types
/// from integer narrowing.
///
/// Without this sidecar, imported `pub` or `#repr("c")` types lose their
/// protection when an importing module builds its `ReprPlan`, allowing
/// their field layouts to be narrowed in violation of ABI guarantees.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExportedTypeMetadata {
    /// Merkle hash of the type's Pool representation.
    ///
    /// Used to map back to a local `Idx` in the importing module's pool
    /// via `Pool::lookup_by_hash()`.
    pub merkle_hash: u64,

    /// Representation attribute (`#repr("c")`, `#repr("packed")`, etc.).
    ///
    /// `None` means default layout (all optimizations permitted).
    pub repr: Option<ReprAttrKind>,

    /// Whether this type is `pub` in its defining module.
    ///
    /// Public types have ABI contracts that narrowing must not violate.
    pub is_public: bool,
}

/// Type-directed desugar plan for one `ExprKind::AssignTarget` chain.
///
/// `ori_types` resolves the type produced at each level of the chain
/// (`root`, `root` + step 0, `root` + steps 0..1, ...) during
/// `infer_assign_target`; `ori_canon` consumes the plan to synthesize the
/// pure-reassignment form (`root = root.updated(...)` / `{ ...root, f: v }`)
/// in its own `CanArena`. The arena is borrowed immutably during type
/// checking, so the synthesized nodes are minted in `ori_canon`, where the
/// mutable arena lives — `ori_types` records only the resolved types the
/// synthesis needs, keeping the type-direction decision in the type checker
/// while AIMS sees only the pure reassignment.
///
/// `level_types[k]` is the resolved type of the receiver-read after applying
/// the first `k` access steps: `level_types[0]` is `root`'s type,
/// `level_types[k]` is the type of reading `root.step0...step(k-1)`. The
/// vector has `steps.len() + 1` entries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssignDesugar {
    /// Resolved receiver-read type at each chain level (length `steps + 1`).
    pub level_types: Vec<Idx>,
}

/// Pool `Idx` values for the builtin `FormatSpec` struct and its `Option<_>`
/// field types, captured at type-check time.
///
/// `ori_canon` consumes these to type the synthesized `FormatSpec` struct
/// node + its field-value nodes when desugaring a non-primitive `{expr:spec}`
/// interpolation that dispatches a user `Formattable.format(self:, spec:)`.
/// Without precise field types the LLVM backend cannot compute the struct
/// layout. `register_format_spec_type` registers these in every module's pool,
/// so this is always populated after a successful check.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FormatSpecTypes {
    /// The `FormatSpec` struct type.
    pub spec: Idx,
    /// `Option<char>` — the `fill` field type.
    pub opt_char: Idx,
    /// `Option<Alignment>` — the `align` field type.
    pub opt_alignment: Idx,
    /// `Option<Sign>` — the `sign` field type.
    pub opt_sign: Idx,
    /// `Option<int>` — the `width` and `precision` field types.
    pub opt_int: Idx,
    /// `Option<FormatType>` — the `format_type` field type.
    pub opt_format_type: Idx,
    /// The `Alignment` enum type (variant-`Ident` node type).
    pub alignment: Idx,
    /// The `Sign` enum type (variant-`Ident` node type).
    pub sign: Idx,
    /// The `FormatType` enum type (variant-`Ident` node type).
    pub format_type: Idx,
}

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
/// ```ignore
/// let result = type_check_module(db, file);
/// if result.has_errors() {
///     for err in &result.typed.errors {
///         // report error
///     }
/// }
/// // Get type of expression 42
/// let ty = result.typed.expr_type(42);
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
    pub pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    /// Impl method signatures for codegen.
    ///
    /// Each entry maps a method name to its resolved `FunctionSig`. Codegen
    /// needs these to compute ABI (calling convention, sret, parameter passing)
    /// for impl methods, which are compiled separately from top-level functions.
    pub impl_sigs: Vec<(Name, FunctionSig)>,

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
    /// Each entry represents a unique `(fn_name, generic_args)` combination
    /// found at a call site. The LLVM backend uses these to stamp out
    /// concrete specializations of generic functions.
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
    /// `FxHashMap`) for Salsa compatibility per `types.md §SL-3` — the
    /// `TypedModule` struct derives `Eq + Hash` and `FxHashMap` cannot
    /// satisfy them. Mirrors the shape of `pattern_resolutions:
    /// Vec<(PatternKey, PatternResolution)>`. Lookup is binary search via
    /// `dispatch_map_lookup()`.
    ///
    /// Empty for non-generic call sites — only populated when a generic
    /// callee is instantiated with concrete arguments at the call site.
    pub mono_dispatch_map: Vec<(ExprId, MonoInstanceId)>,

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
    /// `FxHashMap`) for Salsa compatibility per `types.md §SL-3` — the
    /// `TypedModule` struct derives `Eq + Hash` and `FxHashMap` cannot satisfy
    /// them. Mirrors `pattern_resolutions` / `mono_dispatch_map`. Lookup is
    /// binary search via [`Self::resolve_assign_desugar`]. Keys are
    /// module-wide AST `ExprId`s (the arena is one-per-module), so body-pass
    /// accumulation extends without re-anchoring.
    ///
    /// Empty when no index/field assignment appears in the module.
    pub assign_desugar_map: Vec<(ExprId, AssignDesugar)>,

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
    /// These instances live in the in-memory `TypeRegistry`'s
    /// `collection_burdens` side-table, which `types` (sourced from
    /// `TypeRegistry::into_entries`) excludes by construction. Exporting them
    /// here lets the ARC codegen pipeline reconstruct the side-table so
    /// collection-instance burden reaches Phase 5 emission. Sorted ascending
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
    /// Spec: 14-expressions.md:1262 + 09-properties-of-types.md:231.
    pub formattable_impl_types: Vec<Idx>,

    /// Pool idxs for the builtin `FormatSpec` struct + its `Option<_>` field
    /// types, for typing the synthesized `FormatSpec` struct in `ori_canon`.
    /// `None` only in synthetic/empty modules where builtin registration ran
    /// without a pool (isolated tests); a real check always populates it.
    pub format_spec_types: Option<FormatSpecTypes>,
}

impl TypedModule {
    /// Create a new empty typed module.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a typed module with pre-allocated capacity.
    pub fn with_capacity(expr_count: usize, function_count: usize) -> Self {
        Self {
            expr_types: Vec::with_capacity(expr_count),
            functions: Vec::with_capacity(function_count),
            types: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: Vec::new(),
            impl_sigs: Vec::new(),
            trait_impl_fn_names: Vec::new(),
            mono_instances: Vec::new(),
            mono_dispatch_map: Vec::new(),
            assign_desugar_map: Vec::new(),
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
    pub fn expr_type(&self, expr_index: usize) -> Option<Idx> {
        self.expr_types.get(expr_index).copied()
    }

    /// Get a function signature by name.
    pub fn function(&self, name: Name) -> Option<&FunctionSig> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Get a type definition by name.
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
    /// Uses O(log n) binary search on the sorted `pattern_resolutions` vec.
    pub fn resolve_pattern(&self, key: PatternKey) -> Option<&PatternResolution> {
        self.pattern_resolutions
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .map(|idx| &self.pattern_resolutions[idx].1)
    }

    /// Look up the type-directed desugar plan for an `AssignTarget` `ExprId`.
    ///
    /// Returns `Some(&AssignDesugar)` if the type checker recorded a plan for
    /// this `AssignTarget` node, `None` otherwise. Uses O(log n) binary search
    /// on the sorted `assign_desugar_map`.
    pub fn resolve_assign_desugar(&self, key: ExprId) -> Option<&AssignDesugar> {
        self.assign_desugar_map
            .binary_search_by_key(&key.raw(), |(eid, _)| eid.raw())
            .ok()
            .map(|idx| &self.assign_desugar_map[idx].1)
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
