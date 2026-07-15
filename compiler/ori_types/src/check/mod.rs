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

use ori_ir::{ExprArena, ExprId, Name, SparseSideTable, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    FunctionSig, Idx, MethodRegistry, PatternKey, PatternResolution, Pool, TraitRegistry,
    TypeCheckResult, TypeCheckWarning, TypeEnv, TypeRegistry, TypedModule,
};

/// Identity tuple for `MonoInstance` deduplication at `finish_with_pool()`.
///
/// Encodes the full distinguishing identity per the `MonoInstance`
/// invariant: `(fn_name, generic_args, impl_args, method_args,
/// concrete_param_types, receiver_type)`. Two instances are duplicates iff
/// every field of the tuple matches; the dedup at finish-time uses an
/// `FxHashSet<MonoIdentityKey>` retain to collapse them.
type MonoIdentityKey = (
    Name,
    Vec<crate::GenericArg>,
    Vec<crate::GenericArg>,
    Vec<crate::GenericArg>,
    Vec<crate::Idx>,
    Option<crate::Idx>,
);

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
    /// Keys are module-wide AST receiver `ExprId`s; sorted in `finish_with_pool`,
    /// then stored in [`crate::TypedModule::iter_route_map`].
    iter_route_desugars: Vec<(ExprId, crate::Idx)>,

    // Impl Method Signatures
    /// Accumulated impl method signatures for codegen.
    ///
    /// Built during `check_impl_bodies` — each [`crate::ImplSig`] maps an impl
    /// method's owning receiver type + name to its resolved signature. Codegen
    /// needs these to compute ABI (calling convention, sret, parameter
    /// passing) AND to key mono-collection dispatch on the receiver.
    impl_sigs: Vec<crate::ImplSig>,
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
    /// Concrete generic function instantiations discovered during type checking.
    ///
    /// Accumulated from `InferEngine` after each function body is checked.
    /// Deduped by `(fn_name, generic_args)` before inclusion in `TypedModule`.
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
        Self::build(arena, interner, TypeRegistry::new(), TraitRegistry::new())
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
        Self::build(arena, interner, types, traits)
    }

    /// Shared constructor body for [`Self::new`] and [`Self::with_registries`].
    /// The two public constructors differ ONLY in `types`/`traits` (fresh vs
    /// pre-populated); every other field is initialized identically here — the
    /// single source of truth for the `ModuleChecker` field set.
    fn build(
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
    /// Capture the explicit-`Formattable` impl set + builtin `FormatSpec` type
    /// idxs for `ori_canon`'s non-primitive `{expr:spec}` desugar. The blanket
    /// `impl<T: Printable> T: Formattable` is not a registered impl, so the impl
    /// set is exactly the user-written `Formattable` impls. Self types are
    /// resolved through the pool's chains so the canon-side query (which
    /// normalizes the `FormatWith` receiver type the same way) matches regardless
    /// of Named-vs-resolved divergence.
    fn collect_formattable_metadata(
        &mut self,
    ) -> (Vec<Idx>, Option<crate::output::FormatSpecTypes>) {
        // Collect self types under an immutable `traits` borrow, then resolve
        // them under a mutable `pool` borrow — the two stay disjoint.
        let formattable_name = self.interner.intern("Formattable");
        let raw_self_types: Vec<Idx> = self
            .traits
            .get_trait_by_name(formattable_name)
            .map(|t| t.idx)
            .map(|formattable_idx| {
                self.traits
                    .impls_of_trait(formattable_idx)
                    .map(|e| e.self_type)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut formattable_impl_types: Vec<Idx> = raw_self_types
            .into_iter()
            .map(|st| self.pool.resolve_fully(st))
            .collect();
        formattable_impl_types.sort_unstable_by_key(|i| i.raw());
        formattable_impl_types.dedup_by_key(|i| i.raw());

        // Builtin `FormatSpec` struct idx + its `Option<_>` field idxs
        // (idempotent interning returns the entries `register_format_spec_type`
        // already created) so ori_canon can type the synthesized FormatSpec
        // struct + field-value nodes.
        let spec_name = self.interner.intern("FormatSpec");
        let alignment_name = self.interner.intern("Alignment");
        let sign_name = self.interner.intern("Sign");
        let format_type_name = self.interner.intern("FormatType");
        let spec = self.pool.named(spec_name);
        let alignment_idx = self.pool.named(alignment_name);
        let sign_idx = self.pool.named(sign_name);
        let ft_idx = self.pool.named(format_type_name);
        let format_spec_types = Some(crate::output::FormatSpecTypes {
            spec,
            opt_char: self.pool.option(Idx::CHAR),
            opt_alignment: self.pool.option(alignment_idx),
            opt_sign: self.pool.option(sign_idx),
            opt_int: self.pool.option(Idx::INT),
            opt_format_type: self.pool.option(ft_idx),
            alignment: alignment_idx,
            sign: sign_idx,
            format_type: ft_idx,
        });

        (formattable_impl_types, format_spec_types)
    }

    pub fn finish_with_pool(mut self) -> (TypeCheckResult, Pool) {
        // The explicit-Formattable impl set + builtin FormatSpec type idxs
        // ori_canon consumes to desugar non-primitive `{expr:spec}`. Computed
        // before `pool` is moved out.
        let (formattable_impl_types, format_spec_types) = self.collect_formattable_metadata();
        let mut pool = self.pool;
        let deferred_mono_calls = self.deferred_mono_calls;

        // Sort functions by name for deterministic output regardless of
        // FxHashMap iteration order. Required for Salsa's Eq comparison.
        let mut functions: Vec<FunctionSig> = self.signatures.into_values().collect();
        functions.sort_by_key(|f| f.name);

        // Drain the monomorphized-collection burden side-table before
        // `into_entries` consumes the registry — these instances carry no
        // nominal `TypeEntry`, so `types` (below) excludes them by
        // construction. Exporting them lets the ARC pipeline reconstruct the
        // side-table for Phase 5 burden emission (Spec: Annex E §AIMS).
        let collection_burdens = self.types.drain_collection_burdens();

        // Generic-composite type-param map (`Name → [param names]`) for refreshing method
        // mono `body_type_map`s below — built BEFORE `into_entries` consumes the
        // registry. SSOT mirror of `monomorphization::collect_generic_type_params`.
        let generic_type_params: rustc_hash::FxHashMap<Name, Vec<Name>> = self
            .types
            .iter()
            .filter(|entry| !entry.type_params.is_empty())
            .map(|entry| (entry.name, entry.type_params.clone()))
            .collect();

        // Extract type definitions (already sorted by name via BTreeMap).
        let types = self.types.into_entries();

        // Dedup pattern resolutions (sorted by key first so duplicate keys are
        // adjacent); the final `SparseSideTable::from_unsorted` re-sorts for the
        // O(log n) binary-search shape. `assign_desugar_map` is sorted by the
        // table; the `AssignTarget` desugar plans carry unique `ExprId` keys.
        let mut pattern_resolutions = self.pattern_resolutions;
        pattern_resolutions.sort_by_key(|(k, _)| *k);
        pattern_resolutions.dedup_by_key(|(k, _)| *k);
        let assign_desugar_map = self.assign_desugars;
        let module_alias_call_map = self.module_alias_calls;
        let iter_route_map = self.iter_route_desugars;

        // Resolve transitive mono calls (generic calling generic) before dedup.
        // The deferred resolver publishes dispatch entries into
        // `mono_dispatch_pre_dedup` for each successfully-resolved deferred
        // call, using the `DeferredMonoCall.call_expr_id` recorded at inference
        // time. Pre-dedup ids are remapped through the same dedup pipeline as
        // eager-path entries below.
        let mut mono_instances = self.mono_instances;
        if !deferred_mono_calls.is_empty() {
            exports::resolve_deferred_mono_calls(
                &mut pool,
                &mut mono_instances,
                &mut self.mono_dispatch_pre_dedup,
                &deferred_mono_calls,
            );
        }

        // Complete each method instance's `body_type_map` against the now-fully-
        // interned pool. The eager method-mono path builds the map at the call
        // site (Pass 3), before a generic-impl method body interns its own
        // composite ctor types (Pass 4) — e.g. a `Pair<B, A>` constructor inside
        // `swap`. This pass captures those post-recording body composites so they
        // reach codegen substituted to concrete instead of carrying impl
        // `RigidVar`s. Runs before dedup so refreshed maps participate in the
        // identity tuple unchanged (`body_type_map` is not a dedup key).
        exports::refresh_method_mono_body_type_maps(
            &mut pool,
            &mut mono_instances,
            &generic_type_params,
        );

        // Dedup mono instances by the full identity tuple, then sort by
        // `fn_name` and remap the dispatch entries through the composed
        // `pre-dedup → dedup → sorted` permutation. A free function sidesteps
        // the partial-move of `self.pool` earlier in this method.
        let (mono_instances, mono_dispatch_map) =
            dedup_and_remap_mono_instances(mono_instances, self.mono_dispatch_pre_dedup);

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

        let mut accepted_derives = self.accepted_derives;
        accepted_derives.sort_unstable_by_key(|accepted| accepted.id);
        debug_assert!(
            accepted_derives
                .windows(2)
                .all(|pair| pair[0].id != pair[1].id),
            "accepted derived implementation identities must be unique"
        );

        let typed = TypedModule {
            expr_types: self.expr_types,
            functions,
            types,
            errors: self.errors,
            warnings: self.warnings,
            pattern_resolutions: SparseSideTable::from_unsorted(pattern_resolutions),
            impl_sigs: self.impl_sigs,
            accepted_derives,
            trait_impl_fn_names: self.trait_impl_fn_names,
            mono_instances,
            // Populated from `mono_dispatch_pre_dedup` after remapping
            // pre-dedup `MonoInstanceId`s through dedup + sort, then sorted
            // by `ExprId` for binary-search lookup. The deferred-resolution
            // path `exports::resolve_deferred_mono_calls` publishes pre-dedup
            // entries via `DeferredMonoCall.call_expr_id`, so transitive
            // (generic-calls-generic) instantiations land in this map
            // alongside eager-path instantiations. Both flow through the
            // same dedup-remap pipeline.
            mono_dispatch_map: SparseSideTable::from_unsorted(mono_dispatch_map),
            type_descriptors,
            exported_type_metadata,
            exported_collection_surfaces,
            collection_burdens,
            formattable_impl_types,
            format_spec_types,
            assign_desugar_map: SparseSideTable::from_unsorted(assign_desugar_map),
            module_alias_call_map: SparseSideTable::from_unsorted(module_alias_call_map),
            iter_route_map: SparseSideTable::from_unsorted(iter_route_map),
        };

        (TypeCheckResult::from_typed(typed), pool)
    }
}

/// Dedup `mono_instances` by the full identity tuple, sort the survivors by
/// `fn_name`, and remap `mono_dispatch_pre_dedup` entries through the composed
/// `pre-dedup → dedup → sorted` permutation.
///
/// Returns the deduped+sorted instances and the remapped dispatch map sorted by
/// `ExprId` for binary-search lookup.
fn dedup_and_remap_mono_instances(
    mut mono_instances: Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) -> (
    Vec<crate::MonoInstance>,
    Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) {
    // Dedup mono instances by the full identity tuple — `fn_name` alone
    // is insufficient once method instances flow through this list:
    //
    // - Method-level args collision: `Foo<int>::bar<U>` instantiated via
    //   typed-binding inference at `U = str` vs `U = int` share `fn_name`
    //   AND empty `generic_args`; only `method_args` differ.
    // - Receiver-type collision: `Box<int>::map<U>` and `Option<int>::map<U>`
    //   both have `fn_name = "map"` and identical `impl_args = [int]`;
    //   only `receiver_type` (or `concrete_param_types[0]`) discriminates.
    // - Trait-method-from-different-impls: two impls of the same trait
    //   method on different self types — distinguished by `receiver_type`.
    //
    // Identity tuple: (fn_name, generic_args, impl_args, method_args,
    // concrete_param_types, receiver_type) — see `MonoIdentityKey` alias.
    //
    // Dedup tracks `old_idx → new_idx` so the pre-dedup
    // `mono_dispatch_map` entries (which carry pre-dedup `MonoInstanceId`s)
    // can be remapped to point at the same canonical instance after
    // non-adjacent duplicates collapse. FxHashMap stays deterministic
    // (FxHasher has no per-process random seed), satisfying Salsa SL-1
    // (query purity).
    let pre_dedup_len = mono_instances.len();
    let mut seen: rustc_hash::FxHashMap<MonoIdentityKey, u32> = rustc_hash::FxHashMap::default();
    let mut deduped: Vec<crate::MonoInstance> = Vec::with_capacity(pre_dedup_len);
    // `old_to_dedup[old_position]` = position in `deduped` after retain.
    let mut old_to_dedup: Vec<u32> = Vec::with_capacity(pre_dedup_len);
    for inst in mono_instances.drain(..) {
        let key: MonoIdentityKey = (
            inst.fn_name,
            inst.generic_args.clone(),
            inst.impl_args.clone(),
            inst.method_args.clone(),
            inst.concrete_param_types.clone(),
            inst.receiver_type,
        );
        if let Some(&existing) = seen.get(&key) {
            old_to_dedup.push(existing);
        } else {
            // Saturating `Vec::len() → u32` matches `pool/substitute/mod.rs`
            // — strict workspace clippy denies bare `as` truncation casts and
            // `expect`/`unwrap`. 4-billion-instance overflow is structurally
            // unreachable for any single module.
            let new_idx = u32::try_from(deduped.len()).unwrap_or(u32::MAX);
            seen.insert(key, new_idx);
            deduped.push(inst);
            old_to_dedup.push(new_idx);
        }
    }

    // Sort by fn_name for deterministic output ordering, tracking the
    // permutation so dispatch entries can be re-anchored. Pairing
    // each instance with its pre-sort index via `enumerate` and then
    // sorting the pair vector avoids the placeholder-`Option` dance
    // that an in-place permutation would require.
    let n_dedup = deduped.len();
    // Saturating `usize → u32` casts match `pool/substitute/mod.rs`'s
    // `unwrap_or(u32::MAX)` pattern (strict workspace clippy denies
    // `cast_possible_truncation`). Per the dedup-loop comment above,
    // `deduped.len()` is structurally bounded well below `u32::MAX`.
    let mut indexed: Vec<(u32, crate::MonoInstance)> = deduped
        .into_iter()
        .enumerate()
        .map(|(i, inst)| (u32::try_from(i).unwrap_or(u32::MAX), inst))
        .collect();
    indexed.sort_by_key(|(_, inst)| inst.fn_name);
    let mut dedup_to_sorted: Vec<u32> = vec![0; n_dedup];
    for (sorted_pos, (dedup_pos, _)) in indexed.iter().enumerate() {
        dedup_to_sorted[*dedup_pos as usize] = u32::try_from(sorted_pos).unwrap_or(u32::MAX);
    }
    let mono_instances: Vec<crate::MonoInstance> =
        indexed.into_iter().map(|(_, inst)| inst).collect();

    // Apply the composed `pre-dedup → dedup → sorted` remap to the dispatch
    // entries. The caller wraps the result in `SparseSideTable::from_unsorted`,
    // which sorts by `ExprId` for the O(log n) binary-search shape.
    let mono_dispatch_map: Vec<(ori_ir::ExprId, crate::MonoInstanceId)> = mono_dispatch_pre_dedup
        .into_iter()
        .map(|(eid, id)| {
            let dedup_idx = old_to_dedup[id.index()];
            let final_idx = dedup_to_sorted[dedup_idx as usize];
            (eid, crate::MonoInstanceId::new(final_idx))
        })
        .collect();

    (mono_instances, mono_dispatch_map)
}

#[cfg(test)]
mod tests;
