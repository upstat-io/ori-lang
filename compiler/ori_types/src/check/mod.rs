//! Module-level type checker.
//!
//! The `ModuleChecker` orchestrates type checking of an entire module,
//! coordinating the `InferEngine`, registries, and output generation.
//!
//! # Architecture
//!
//! Type checking follows a multi-pass approach:
//!
//! ```text
//! Pass 0: Registration
//!   0a: Built-in types (Ordering, etc.)
//!   0b: User-defined types (structs, enums, newtypes)
//!   0c: Traits and implementations
//!   0d: Derived implementations
//!   0e: Config variables
//!
//! Pass 1: Function Signatures
//!   - Collect all function signatures before body checking
//!   - Enables mutual recursion and forward references
//!   - Create type schemes for polymorphic functions
//!   - Freeze base environment
//!
//! Pass 2: Function Bodies
//!   - Type check function bodies against signatures
//!   - Handle let bindings with let-polymorphism
//!
//! Pass 3: Test Bodies
//!   - Type check test bodies (implicit void return)
//!
//! Pass 4: Impl Method Bodies
//!   - Type check implementation method bodies
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use ori_types::check::check_module;
//!
//! let result = check_module(&parse_output, &interner);
//! if result.has_errors() {
//!     for error in result.errors() {
//!         // report error
//!     }
//! }
//! ```
//!
//! # Design Notes
//!
//! Key design decisions:
//! - Uses `Idx` for type handles (compact u32 pool indices)
//! - Uses `Pool` for interned type storage
//! - Uses `InferEngine` for Hindley-Milner inference

use ori_ir::{ExprArena, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    FunctionSig, Idx, MethodRegistry, PatternKey, PatternResolution, Pool, TraitRegistry,
    TypeCheckResult, TypeCheckWarning, TypeEnv, TypeRegistry, TypedModule,
};

// Re-export main API
pub use api::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
};

mod accessors;
mod api;
mod bodies;
mod exports;
mod imports;
mod object_safety;
mod registration;
mod scope;
mod signatures;
mod well_known;

// Re-export for use in sibling modules (e.g., infer::expr::type_resolution).
pub(crate) use object_safety::{check_parsed_type_object_safety, ObjectSafetyChecker};
pub(crate) use well_known::{is_concrete_named_type, resolve_well_known_generic, WellKnownNames};

#[cfg(test)]
mod integration_tests;

/// Module-level type checker.
///
/// Orchestrates all passes of type checking for a single module,
/// producing a `TypedModule` with expression types and any errors.
///
/// # Component Structure
///
/// ```text
/// ModuleChecker
/// ├── Immutable Context
/// │   ├── arena: &ExprArena     (expression lookup)
/// │   └── interner: &StringInterner (name resolution)
/// │
/// ├── Type Storage
/// │   └── pool: Pool            (unified type pool)
/// │
/// ├── Registries
/// │   ├── types: TypeRegistry   (structs, enums)
/// │   ├── traits: TraitRegistry (traits, impls)
/// │   └── methods: MethodRegistry (built-in methods)
/// │
/// ├── Function State
/// │   ├── signatures: HashMap<Name, FunctionSig>
/// │   └── base_env: Option<TypeEnv>
/// │
/// ├── Scope Context
/// │   ├── current_function: Option<Idx>
/// │   ├── current_impl_self: Option<Idx>
/// │   ├── current_capabilities: HashSet<Name>
/// │   └── provided_capabilities: HashSet<Name>
/// │
/// └── Diagnostics
///     └── errors: Vec<TypeCheckError>
/// ```
pub struct ModuleChecker<'a> {
    // === Immutable Context ===
    /// Expression arena for looking up expressions.
    arena: &'a ExprArena,
    /// String interner for name resolution.
    interner: &'a StringInterner,

    // === Type Storage ===
    /// Unified type pool (becomes part of output).
    pool: Pool,

    // === Name Cache ===
    /// Pre-interned primitive and well-known type names for O(1) resolution.
    well_known: WellKnownNames,

    // === Registries ===
    /// Registry for user-defined types (structs, enums).
    types: TypeRegistry,
    /// Registry for traits and implementations.
    traits: TraitRegistry,
    /// Registry for method resolution (built-ins + user).
    methods: MethodRegistry,

    // === Import State ===
    /// Environment with imported function bindings.
    ///
    /// Populated by `register_imported_function()` before signature collection.
    /// `collect_signatures()` creates a child of this to include local functions,
    /// so imports are visible as the grandparent scope.
    import_env: TypeEnv,
    /// Module alias imports for qualified access (e.g., `http.get(...)`).
    ///
    /// Maps alias names to the signatures of all public functions in that module.
    /// Full qualified-access resolution is deferred to inference engine changes.
    module_aliases: FxHashMap<Name, Vec<FunctionSig>>,

    // === Function Signatures ===
    /// Collected function signatures for call resolution.
    signatures: FxHashMap<Name, FunctionSig>,
    /// Frozen base environment (after signature collection).
    base_env: Option<TypeEnv>,

    // === Expression Types ===
    /// Inferred type for each expression (expr index → type).
    expr_types: Vec<Idx>,

    // === Scope Context ===
    /// Current function's type (for `recurse` pattern).
    current_function: Option<Idx>,
    /// Current impl's self type (for `self` resolution).
    current_impl_self: Option<Idx>,
    /// Capabilities declared by current function (`uses` clause).
    current_capabilities: FxHashSet<Name>,
    /// Capabilities provided in scope (`with...in`).
    provided_capabilities: FxHashSet<Name>,
    /// Constant types.
    const_types: FxHashMap<Name, Idx>,

    // === Diagnostics ===
    /// Accumulated type check errors.
    errors: Vec<crate::TypeCheckError>,
    /// Accumulated type check warnings.
    warnings: Vec<TypeCheckWarning>,

    // === Pattern Resolutions ===
    /// Accumulated pattern resolutions from all checked bodies.
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    // === Impl Method Signatures ===
    /// Accumulated impl method signatures for codegen.
    ///
    /// Built during `check_impl_bodies` — each `(Name, FunctionSig)` pair
    /// maps an impl method name to its resolved signature. Codegen needs
    /// these to compute ABI (calling convention, sret, parameter passing).
    impl_sigs: Vec<(Name, FunctionSig)>,
    /// Trait impl method identities (for unconstrained function detection).
    /// Each entry is `(self_type_idx, method_name)` for disambiguation.
    trait_impl_fn_names: Vec<(Idx, Name)>,

    // === Monomorphization ===
    /// Concrete generic function instantiations discovered during type checking.
    ///
    /// Accumulated from `InferEngine` after each function body is checked.
    /// Deduped by `(fn_name, generic_args)` before inclusion in `TypedModule`.
    mono_instances: Vec<crate::MonoInstance>,

    /// Deferred mono calls (generic calling generic).
    ///
    /// Accumulated from `InferEngine` after each function body is checked.
    /// Resolved in `finish_with_pool()` using direct `MonoInstance` body type maps.
    deferred_mono_calls: Vec<crate::DeferredMonoCall>,

    /// Imported modules' exported type metadata for transitive forwarding.
    ///
    /// Set by the caller (e.g., `register_resolved_imports` in `oric/src/typeck.rs`)
    /// before `finish_with_pool()`. When generating this module's
    /// `exported_type_metadata`, these entries are merged in so that re-exported
    /// types propagate transitively through module chains (A→B→C). Without this,
    /// A would lose C's `pub`/`#repr` metadata when importing only B.
    imported_type_metadata: Vec<crate::output::ExportedTypeMetadata>,
    imported_collection_surfaces: Vec<u64>,
}

impl<'a> ModuleChecker<'a> {
    // Constructors

    /// Create a new module checker.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        let well_known = WellKnownNames::new(interner);
        Self {
            arena,
            interner,
            pool: Pool::new(),
            well_known,
            types: TypeRegistry::new(),
            traits: TraitRegistry::new(),
            methods: MethodRegistry::new(),
            import_env: TypeEnv::new(),
            module_aliases: FxHashMap::default(),
            signatures: FxHashMap::default(),
            base_env: None,
            expr_types: Vec::new(),
            current_function: None,
            current_impl_self: None,
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            const_types: FxHashMap::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: Vec::new(),
            impl_sigs: Vec::new(),
            trait_impl_fn_names: Vec::new(),
            mono_instances: Vec::new(),
            deferred_mono_calls: Vec::new(),
            imported_type_metadata: Vec::new(),
            imported_collection_surfaces: Vec::new(),
        }
    }

    /// Create a module checker with pre-populated registries.
    ///
    /// Use this when imports have already been resolved and you need
    /// to register imported types/traits before checking.
    pub fn with_registries(
        arena: &'a ExprArena,
        interner: &'a StringInterner,
        types: TypeRegistry,
        traits: TraitRegistry,
    ) -> Self {
        let well_known = WellKnownNames::new(interner);
        Self {
            arena,
            interner,
            pool: Pool::new(),
            well_known,
            types,
            traits,
            methods: MethodRegistry::new(),
            import_env: TypeEnv::new(),
            module_aliases: FxHashMap::default(),
            signatures: FxHashMap::default(),
            base_env: None,
            expr_types: Vec::new(),
            current_function: None,
            current_impl_self: None,
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            const_types: FxHashMap::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: Vec::new(),
            impl_sigs: Vec::new(),
            trait_impl_fn_names: Vec::new(),
            mono_instances: Vec::new(),
            deferred_mono_calls: Vec::new(),
            imported_type_metadata: Vec::new(),
            imported_collection_surfaces: Vec::new(),
        }
    }

    // Import/Setup Setters

    /// Set imported type metadata for transitive forwarding.
    ///
    /// Called during the `register_fn` closure in [`check_module_with_imports`]
    /// to provide metadata from imported modules. When this module finishes type
    /// checking, its `exported_type_metadata` will include both local types and
    /// forwarded imported entries (deduped by Merkle hash, local priority).
    pub fn set_imported_type_metadata(
        &mut self,
        metadata: Vec<crate::output::ExportedTypeMetadata>,
    ) {
        self.imported_type_metadata = metadata;
    }

    /// Set imported collection surface hashes from imported modules.
    ///
    /// These merkle hashes identify collection types (List, Set) that appear
    /// in imported public function signatures. They are merged with local
    /// collection surfaces during export for transitive forwarding.
    pub fn set_imported_collection_surfaces(&mut self, surfaces: Vec<u64>) {
        self.imported_collection_surfaces = surfaces;
    }

    // Output Generation

    /// Finalize checking and produce the result.
    ///
    /// Consumes the checker and returns the typed module with any errors.
    pub fn finish(self) -> TypeCheckResult {
        self.finish_with_pool().0
    }

    /// Consume the checker and return the pool along with the result.
    ///
    /// Use this when you need access to the pool for type resolution
    /// after checking is complete.
    pub fn finish_with_pool(self) -> (TypeCheckResult, Pool) {
        let mut pool = self.pool;
        let deferred_mono_calls = self.deferred_mono_calls;

        // Sort functions by name for deterministic output regardless of
        // FxHashMap iteration order. Required for Salsa's Eq comparison.
        let mut functions: Vec<FunctionSig> = self.signatures.into_values().collect();
        functions.sort_by_key(|f| f.name);

        // Pre-intern tuple types for multi-parameter functions. Multi-clause
        // function lowering (ori_canon) creates a synthetic Tuple scrutinee
        // from the parameters, and needs the tuple type interned in the pool.
        // The pool is immutable by that point, so we intern here.
        for sig in &functions {
            if sig.param_types.len() > 1 {
                pool.tuple(&sig.param_types);
            }
        }

        // Extract type definitions (already sorted by name via BTreeMap).
        let types = self.types.into_entries();

        // Sort and dedup pattern resolutions for O(log n) binary search.
        let mut pattern_resolutions = self.pattern_resolutions;
        pattern_resolutions.sort_by_key(|(k, _)| *k);
        pattern_resolutions.dedup_by_key(|(k, _)| *k);

        // Resolve transitive mono calls (generic calling generic) before dedup.
        let mut mono_instances = self.mono_instances;
        if !deferred_mono_calls.is_empty() {
            exports::resolve_deferred_mono_calls(
                &mut pool,
                &mut mono_instances,
                &deferred_mono_calls,
            );
        }

        // Dedup mono instances by (fn_name, generic_args).
        // Sort by fn_name for determinism, then dedup adjacent entries.
        mono_instances.sort_by_key(|m| m.fn_name);
        mono_instances.dedup_by(|a, b| a.fn_name == b.fn_name && a.generic_args == b.generic_args);

        // Generate portable type descriptors for all public function signatures.
        // These enable cross-module type reconstruction without AST access.
        let type_descriptors = exports::generate_export_descriptors(&pool, &functions);

        // Generate exported type metadata for cross-module repr plan construction.
        // Merges local types (repr/public) with forwarded imported metadata so that
        // transitive chains (A→B→C) propagate correctly.
        let exported_type_metadata =
            exports::generate_exported_type_metadata(&types, &self.imported_type_metadata);

        // Generate collection surface hashes for cross-module ABI protection.
        // Walks public function signatures to find List/Set types, merges with
        // imported surfaces for transitive forwarding.
        let exported_collection_surfaces = exports::generate_exported_collection_surfaces(
            &pool,
            &functions,
            &self.imported_collection_surfaces,
        );

        let typed = TypedModule {
            expr_types: self.expr_types,
            functions,
            types,
            errors: self.errors,
            warnings: self.warnings,
            pattern_resolutions,
            impl_sigs: self.impl_sigs,
            trait_impl_fn_names: self.trait_impl_fn_names,
            mono_instances,
            type_descriptors,
            exported_type_metadata,
            exported_collection_surfaces,
        };

        (TypeCheckResult::from_typed(typed), pool)
    }
}

#[cfg(test)]
mod tests;
