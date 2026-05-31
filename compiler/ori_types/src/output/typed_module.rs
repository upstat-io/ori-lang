//! Type-checked module output.
//!
//! The complete typed IR for a module: per-expression types, function
//! signatures, resolved patterns, monomorphization instances, and the
//! cross-module export sidecars (type descriptors, repr metadata).

use ori_ir::{ExprId, Name, PatternKey, PatternResolution, ReprAttrKind};

use crate::pool::TypeDescriptor;
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
            type_descriptors: Vec::new(),
            exported_type_metadata: Vec::new(),
            exported_collection_surfaces: Vec::new(),
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
}
