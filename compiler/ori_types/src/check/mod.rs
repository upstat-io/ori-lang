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
//! ```rust
//! use ori_ir::StringInterner;
//!
//! let interner = StringInterner::new();
//! let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/doc_main.ori"));
//! let tokens = ori_lexer::lex(source, &interner);
//! let parsed = ori_parse::parse(&tokens, &interner);
//! let result = ori_types::check_module(&parsed.module, &parsed.arena, &interner);
//! assert!(!result.has_errors(), "{:?}", result.errors());
//! ```
//!
//! # Design Notes
//!
//! Key design decisions:
//! - Uses `Idx` for type handles (compact u32 pool indices)
//! - Uses `Pool` for interned type storage
//! - Uses `InferEngine` for Hindley-Milner inference

use ori_ir::{ExprArena, ExprId, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    FunctionSig, Idx, MethodRegistry, PatternKey, PatternResolution, Pool, TraitRegistry,
    TypeCheckWarning, TypeEnv, TypeRegistry,
};

// Re-export main API
pub use api::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
};

mod accessors;
mod api;
mod bodies;
mod derived_call_plans;
mod exports;
mod finish;
mod imports;
mod object_safety;
pub(crate) mod registration;
mod scope;
mod signatures;
pub(crate) mod validators;
mod well_known;

// Re-export for use in sibling modules (e.g., infer::expr::type_resolution).
pub(crate) use object_safety::{check_parsed_type_object_safety, ObjectSafetyChecker};
pub(crate) use well_known::{is_concrete_named_type, resolve_well_known_generic, WellKnownNames};

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod test_utils;

/// Module-level type checker.
///
/// Orchestrates all passes of type checking for a single module,
/// producing a `TypedModule` with expression types and any errors.
///
/// # Component Structure
///
/// ```text
/// ModuleChecker
/// - Immutable Context
///   - arena: &ExprArena     (expression lookup)
///   - interner: &StringInterner (name resolution)
/// - Type Storage
///   - pool: Pool            (unified type pool)
/// - Registries
///   - types: TypeRegistry   (structs, enums)
///   - traits: TraitRegistry (traits, impls)
///   - methods: MethodRegistry (built-in methods)
/// - Function State
///   - signatures: HashMap<Name, FunctionSig>
///   - base_env: Option<TypeEnv>
/// - Scope Context
///   - current_function: Option<Idx>
///   - current_impl_self: Option<Idx>
///   - current_capabilities: HashSet<Name>
///   - provided_capabilities: HashSet<Name>
/// - Diagnostics
///   - errors: Vec<TypeCheckError>
/// ```
pub struct ModuleChecker<'a> {
    // Immutable Context
    /// Expression arena for looking up expressions.
    arena: &'a ExprArena,
    /// String interner for name resolution.
    interner: &'a StringInterner,

    // Type Storage
    /// Unified type pool (becomes part of output).
    pool: Pool,

    // Name Cache
    /// Pre-interned primitive and well-known type names for O(1) resolution.
    well_known: WellKnownNames,

    // Registries
    /// Registry for user-defined types (structs, enums).
    types: TypeRegistry,
    /// Registry for traits and implementations.
    traits: TraitRegistry,
    /// Registry for method resolution (built-ins + user).
    methods: MethodRegistry,

    // Import State
    /// Environment with imported function bindings.
    ///
    /// Populated by `register_imported_function()` before signature collection.
    /// `collect_signatures()` creates a child of this to include local functions,
    /// so imports are visible as the grandparent scope.
    import_env: TypeEnv,
    /// Module alias imports for qualified access (e.g., `http.get(...)`).
    ///
    /// Maps alias names to the signatures of all public functions in that module.
    /// Consumed by `try_infer_module_alias_call` to resolve `alias.func(args)`
    /// against the named signature (arity-checked, args checked against params).
    module_aliases: FxHashMap<Name, Vec<FunctionSig>>,

    // Function Signatures
    /// Collected function signatures for call resolution.
    signatures: FxHashMap<Name, FunctionSig>,
    /// Frozen base environment (after signature collection).
    base_env: Option<TypeEnv>,

    // Expression Types
    /// Inferred type for each expression (expr index → type).
    expr_types: Vec<Idx>,

    // Scope Context
    /// Current function's type (for `recurse` pattern).
    current_function: Option<Idx>,
    /// Current impl's self type (for `self` resolution).
    current_impl_self: Option<Idx>,
    /// Current impl's associated-type projection context, set transiently while
    /// resolving an impl block's method signatures (registration Pass 0c +
    /// body-check Pass 4). The map is the impl's in-scope `type Item = …`
    /// bindings (`assoc_name → resolved Idx`), available BEFORE the `ImplEntry`
    /// is registered; the `Option<Idx>` is the impl's `trait_idx` for cross-impl
    /// projection lookups. Read by `resolve_type_with_overlay_inner`'s
    /// `ParsedType::AssociatedType` arm to resolve `Self.Item` in a method's
    /// declared param/return type.
    current_impl_assoc: Option<(FxHashMap<Name, Idx>, Option<Idx>)>,
    /// Capabilities declared by current function (`uses` clause).
    current_capabilities: FxHashSet<Name>,
    /// Capabilities provided in scope (`with...in`).
    provided_capabilities: FxHashSet<Name>,
    /// Constant types.
    const_types: FxHashMap<Name, Idx>,
    /// Builtin-type extension methods keyed by `TypeTag`, from `module.extends`.
    /// Set on each inference engine so an `extend <builtin> { @m }` method is not
    /// false-rejected as unknown.
    builtin_extensions: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>>,

    /// Impl-level `RigidVar` substitution maps, one per `module.impls` entry in
    /// registration order. Allocated at `register_impls` (before any body pass)
    /// so a method-mono recording at a pass-3 test/function call site finds the
    /// impl's `RigidVar`s in `var_states`; `check_impl_block` (pass 4) REUSES the
    /// same map so the method body's generic types reference the identical
    /// `RigidVar` Idxs the recording scan substitutes. Without early allocation,
    /// the body's `RigidVar`s are created after the recording and the generic-
    /// struct ctor composite never resolves (`swap` facet).
    impl_rigid_var_maps: Vec<FxHashMap<Name, Idx>>,

    /// Method-level `RigidVar` substitution maps keyed by the method body's
    /// `ExprId`. Allocated at Pass 0c (`register_impls`) so a method's generic
    /// `RigidVar`s exist before any pass-3 call-site records a method mono;
    /// `check_impl_method` (pass 4) REUSES the stored map via `prealloc` so the
    /// body's `RigidVar` Idxs match the recording scan (method-level
    /// generic facet). Mirror of `impl_rigid_var_maps` for method binders.
    method_rigid_var_maps: FxHashMap<ExprId, FxHashMap<Name, Idx>>,

    // Diagnostics
    /// Accumulated type check errors.
    errors: Vec<crate::TypeCheckError>,
    /// Accumulated type check warnings.
    warnings: Vec<TypeCheckWarning>,

    // Pattern Resolutions
    /// Accumulated pattern resolutions from all checked bodies.
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    // Assignment-Target Desugar Plans
    /// Accumulated type-directed desugar plans for `ExprKind::AssignTarget`
    /// chains from all checked bodies. Keys are module-wide AST `ExprId`s;
    /// sorted by `ExprId` in `finish_with_pool` for binary-search lookup, then
    /// stored in [`crate::TypedModule::assign_desugar_map`].
    assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,

    /// Module-alias qualified-call rewrite entries from all checked bodies.
    /// Keys are module-wide AST `ExprId`s; sorted in `finish_with_pool`, then
    /// stored in [`crate::TypedModule::module_alias_call_map`].
    module_alias_calls: Vec<(ExprId, ori_ir::Name)>,

    /// Iterable->Iterator route entries from all checked bodies.
    /// Keys are module-wide AST call `ExprId`s; sorted in `finish_with_pool`,
    /// then stored in [`crate::TypedModule::iter_route_map`].
    iter_route_desugars: Vec<(ExprId, crate::IterMethodRoute)>,

    // Impl Method Signatures
    /// Accumulated impl method signatures for codegen.
    ///
    /// Built during `check_impl_bodies` — each [`crate::ImplSig`] maps an impl
    /// method's owning receiver type + name to its resolved signature. Codegen
    /// needs these to compute ABI (calling convention, sret, parameter
    /// passing) AND to key mono-collection dispatch on the receiver.
    impl_sigs: Vec<crate::ImplSig>,
    /// Imported generic impl templates registered before local checking.
    ///
    /// Coordinates are importer-pool types; producer identities remain stable
    /// across foreign arena and registry index spaces.
    imported_impl_sigs: Vec<crate::ImportedImplSig>,
    /// Compiler-generated implementations accepted by validation and
    /// coherence. Downstream generated-body construction consumes this exact
    /// inventory rather than rescanning source attributes.
    accepted_derives: Vec<crate::AcceptedDerivedImpl>,
    /// Frontend-owned semantic roles keyed by exact impl-method identity.
    /// Populated during impl registration, where the resolved trait identity
    /// and logical burden operation coexist; consumed unchanged by body export.
    impl_method_roles: FxHashMap<crate::ImplMethodId, crate::ImplMethodRole>,
    /// Trait impl method identities (for unconstrained function detection).
    /// Each entry is `(self_type_idx, method_name)` for disambiguation.
    trait_impl_fn_names: Vec<(Idx, Name)>,

    // Monomorphization
    /// Concrete generic callable demands discovered during type checking.
    ///
    /// Accumulated from `InferEngine` after each function body is checked.
    /// Deduped by the full free-function/method identity before inclusion in
    /// `TypedModule`.
    mono_instances: Vec<crate::MonoInstance>,

    /// Pre-dedup `(call_expr_id, MonoInstanceId)` entries accumulated from
    /// each engine session via `accumulate_mono_session`. The
    /// [`crate::MonoInstanceId`] values reference positions in
    /// `mono_instances` AT THE TIME of accumulation (already module-wide
    /// per-session offset adjustment, but pre-dedup). [`finish_with_pool`]
    /// builds an `old_idx → new_idx` remap when it dedups + sorts
    /// `mono_instances`, applies it to these entries, sorts by `ExprId`,
    /// and stores the result in [`crate::TypedModule::mono_dispatch_map`].
    mono_dispatch_pre_dedup: Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,

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
        Self::with_registries(arena, interner, TypeRegistry::new(), TraitRegistry::new())
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
            current_impl_assoc: None,
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            const_types: FxHashMap::default(),
            builtin_extensions: FxHashMap::default(),
            impl_rigid_var_maps: Vec::new(),
            method_rigid_var_maps: FxHashMap::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: Vec::new(),
            assign_desugars: Vec::new(),
            module_alias_calls: Vec::new(),
            iter_route_desugars: Vec::new(),
            impl_sigs: Vec::new(),
            imported_impl_sigs: Vec::new(),
            accepted_derives: Vec::new(),
            impl_method_roles: FxHashMap::default(),
            trait_impl_fn_names: Vec::new(),
            mono_instances: Vec::new(),
            mono_dispatch_pre_dedup: Vec::new(),
            deferred_mono_calls: Vec::new(),
            imported_type_metadata: Vec::new(),
            imported_collection_surfaces: Vec::new(),
        }
    }

    /// Store the impl-level `RigidVar` substitution map for the next
    /// `module.impls` entry (push order == `module.impls` iteration order).
    /// Called once per impl by `register_impls`, before any body pass.
    pub(crate) fn push_impl_rigid_var_map(&mut self, map: FxHashMap<Name, Idx>) {
        self.impl_rigid_var_maps.push(map);
    }

    /// The impl-level `RigidVar` substitution map for the impl at `module.impls`
    /// position `idx`, or `None` when out of range. Consumed by
    /// `check_impl_block` to REUSE the registration-time `RigidVar`s.
    pub(crate) fn impl_rigid_var_map(&self, idx: usize) -> Option<&FxHashMap<Name, Idx>> {
        self.impl_rigid_var_maps.get(idx)
    }

    /// Store the method-level `RigidVar` substitution map for the method whose
    /// body is `body`. Called once per impl method by `register_impls` (Pass 0c)
    /// so the binders exist before any pass-3 call-site records a method mono.
    pub(crate) fn set_method_rigid_var_map(&mut self, body: ExprId, map: FxHashMap<Name, Idx>) {
        self.method_rigid_var_maps.insert(body, map);
    }

    /// The method-level `RigidVar` map for the method body `body`, or `None`
    /// when the method has no registered map. Consumed by `check_impl_method`
    /// (Pass 4) to REUSE the registration-time `RigidVar`s via `prealloc`.
    pub(crate) fn method_rigid_var_map_for(&self, body: ExprId) -> Option<&FxHashMap<Name, Idx>> {
        self.method_rigid_var_maps.get(&body)
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
}

#[cfg(test)]
mod tests;
