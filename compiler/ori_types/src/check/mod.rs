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

use ori_ir::{ExprArena, Name, Span, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    FunctionSig, Idx, InferEngine, MethodRegistry, PatternKey, PatternResolution, Pool,
    TraitRegistry, TypeCheckError, TypeCheckResult, TypeCheckWarning, TypeEnv, TypeRegistry,
    TypedModule,
};

// Re-export main API
pub use api::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
};

mod api;
mod bodies;
mod object_safety;
mod registration;
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
    errors: Vec<TypeCheckError>,
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
}

impl<'a> ModuleChecker<'a> {
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
        }
    }

    // ========================================
    // Accessors
    // ========================================

    /// Get the expression arena.
    ///
    /// Returns with the original `'a` lifetime to avoid borrowing `self`.
    /// This allows using the arena while mutably borrowing other checker fields.
    #[inline]
    pub fn arena(&self) -> &'a ExprArena {
        self.arena
    }

    /// Get the string interner.
    ///
    /// Returns with the original `'a` lifetime to avoid borrowing `self`.
    #[inline]
    pub fn interner(&self) -> &'a StringInterner {
        self.interner
    }

    /// Get the pre-interned well-known type names cache.
    #[inline]
    pub(crate) fn well_known(&self) -> &WellKnownNames {
        &self.well_known
    }

    /// Resolve a primitive type name to its fixed `Idx` via the name cache.
    #[inline]
    pub fn resolve_primitive_name(&self, name: Name) -> Option<Idx> {
        self.well_known.resolve_primitive(name)
    }

    /// Resolve a well-known generic type name via the name cache.
    ///
    /// Split borrow: reads `well_known` (immutable) and writes `pool` (mutable)
    /// from the same `&mut self`. This is safe because they're independent fields.
    #[inline]
    pub fn resolve_well_known_generic_cached(&mut self, name: Name, args: &[Idx]) -> Option<Idx> {
        self.well_known.resolve_generic(&mut self.pool, name, args)
    }

    /// Check if a name is a well-known concrete type (not a trait object).
    #[inline]
    pub fn is_well_known_concrete_cached(&self, name: Name, num_args: usize) -> bool {
        self.well_known.is_concrete(name, num_args)
    }

    /// Resolve a registration-phase primitive (Ordering, Duration, Size).
    #[inline]
    pub fn resolve_registration_primitive(&self, name: Name) -> Option<Idx> {
        self.well_known.resolve_registration_primitive(name)
    }

    /// Get the type pool.
    #[inline]
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Get mutable access to the type pool.
    #[inline]
    pub fn pool_mut(&mut self) -> &mut Pool {
        &mut self.pool
    }

    /// Get the type registry.
    #[inline]
    pub fn type_registry(&self) -> &TypeRegistry {
        &self.types
    }

    /// Get mutable access to the type registry.
    #[inline]
    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.types
    }

    /// Get the trait registry.
    #[inline]
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.traits
    }

    /// Get mutable access to the trait registry.
    #[inline]
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.traits
    }

    /// Get the method registry.
    #[inline]
    pub fn method_registry(&self) -> &MethodRegistry {
        &self.methods
    }

    /// Get a function signature by name.
    pub fn get_signature(&self, name: Name) -> Option<&FunctionSig> {
        self.signatures.get(&name)
    }

    /// Register a function signature.
    ///
    /// Called during Pass 1 (signature collection) to store signatures
    /// for later call resolution during body checking.
    pub fn register_signature(&mut self, sig: FunctionSig) {
        self.signatures.insert(sig.name, sig);
    }

    /// Store an impl method signature for codegen.
    pub fn register_impl_sig(&mut self, name: Name, sig: FunctionSig) {
        self.impl_sigs.push((name, sig));
    }

    /// Get all registered signatures.
    pub fn signatures(&self) -> &FxHashMap<Name, FunctionSig> {
        &self.signatures
    }

    /// Get the import environment.
    ///
    /// Contains bindings for imported functions, populated before signature
    /// collection via `register_imported_function()`.
    pub fn import_env(&self) -> &TypeEnv {
        &self.import_env
    }

    /// Get the module alias map.
    ///
    /// Maps alias names to the public function signatures from the aliased module.
    pub fn module_aliases(&self) -> &FxHashMap<Name, Vec<FunctionSig>> {
        &self.module_aliases
    }

    // ========================================
    // Import Registration
    // ========================================

    /// Register an imported function for cross-module type checking.
    ///
    /// Infers the function's signature using the foreign module's arena,
    /// creates the function type in the local pool, and binds it in
    /// the import environment.
    ///
    /// Call this before `collect_signatures()` / `check_module_impl()` so
    /// that imported bindings are visible as the parent scope of local
    /// function signatures.
    pub fn register_imported_function(
        &mut self,
        func: &ori_ir::Function,
        foreign_arena: &ExprArena,
    ) {
        let (sig, var_ids) = signatures::infer_function_signature_from(self, func, foreign_arena);
        let fn_type = self.pool.function(&sig.param_types, sig.return_type);

        // Wrap generic functions in a type scheme so each call gets fresh
        // type variables via instantiation (prevents shared-variable pollution).
        let bound_type = if var_ids.is_empty() {
            fn_type
        } else {
            self.pool.scheme(&var_ids, fn_type)
        };

        self.import_env.bind(sig.name, bound_type);
        self.signatures.insert(sig.name, sig);
    }

    /// Register an imported function under a different local name.
    ///
    /// Like [`register_imported_function`], but overrides the name used for
    /// binding in the import environment and signature map. Used for aliased
    /// imports (`use './mod' { foo as bar }`) — avoids cloning the entire
    /// `Function` AST node just to change its name.
    pub fn register_imported_function_as(
        &mut self,
        func: &ori_ir::Function,
        foreign_arena: &ExprArena,
        alias: Name,
    ) {
        let (mut sig, var_ids) =
            signatures::infer_function_signature_from(self, func, foreign_arena);
        sig.name = alias;
        let fn_type = self.pool.function(&sig.param_types, sig.return_type);

        let bound_type = if var_ids.is_empty() {
            fn_type
        } else {
            self.pool.scheme(&var_ids, fn_type)
        };

        self.import_env.bind(alias, bound_type);
        self.signatures.insert(alias, sig);
    }

    /// Register traits from an imported module (e.g., prelude).
    ///
    /// Uses the foreign module's arena to resolve generic params and method
    /// signatures. Only public traits are registered.
    pub fn register_imported_traits(&mut self, module: &ori_ir::Module, foreign_arena: &ExprArena) {
        registration::register_imported_traits(self, module, foreign_arena);
    }

    /// Register a built-in function directly by type signature.
    ///
    /// Used for native functions (like `int()`, `str()`, `float()`) that are
    /// implemented in the evaluator but need type information during checking.
    ///
    /// `generic_var_ids` lists the var IDs of type parameters that should be
    /// quantified. Pass empty slice for monomorphic functions.
    pub fn register_builtin_function(
        &mut self,
        name: Name,
        param_types: &[Idx],
        return_type: Idx,
        generic_var_ids: &[u32],
    ) {
        let fn_type = self.pool.function(param_types, return_type);
        let bound_type = if generic_var_ids.is_empty() {
            fn_type
        } else {
            self.pool.scheme(generic_var_ids, fn_type)
        };
        self.import_env.bind(name, bound_type);
    }

    /// Register a built-in value (like `Less`, `Equal`, `Greater`).
    pub fn register_builtin_value(&mut self, name: Name, ty: Idx) {
        self.import_env.bind(name, ty);
    }

    /// Register a module alias for qualified access.
    ///
    /// Collects signatures for all public functions in the given module and
    /// stores them under the alias name. Also binds the alias in the import
    /// environment as a named type placeholder.
    ///
    /// **Note:** Full qualified-access resolution (`alias.func(...)`) is deferred —
    /// it requires inference engine changes for `ExprKind::FieldAccess` on
    /// namespace types. The data storage is in place for when that's needed.
    pub fn register_module_alias(
        &mut self,
        alias: Name,
        module: &ori_ir::Module,
        foreign_arena: &ExprArena,
    ) {
        let sigs: Vec<FunctionSig> = module
            .functions
            .iter()
            .filter(|f| f.visibility == ori_ir::Visibility::Public)
            .map(|f| {
                let (sig, _var_ids) =
                    signatures::infer_function_signature_from(self, f, foreign_arena);
                sig
            })
            .collect();

        self.module_aliases.insert(alias, sigs);

        // Bind alias as a named type placeholder in the import env.
        // This makes the alias name resolvable (as a namespace marker).
        let alias_ty = self.pool.named(alias);
        self.import_env.bind(alias, alias_ty);
    }

    // ========================================
    // Scope Context
    // ========================================

    /// Get the current function type (for `recurse`).
    #[inline]
    pub fn current_function(&self) -> Option<Idx> {
        self.current_function
    }

    /// Get the current impl self type.
    #[inline]
    pub fn current_impl_self(&self) -> Option<Idx> {
        self.current_impl_self
    }

    /// Check if a capability is available (declared or provided).
    pub fn has_capability(&self, cap: Name) -> bool {
        self.current_capabilities.contains(&cap) || self.provided_capabilities.contains(&cap)
    }

    /// Get the type of a constant.
    pub fn const_type(&self, name: Name) -> Option<Idx> {
        self.const_types.get(&name).copied()
    }

    /// Register a constant type.
    pub fn register_const_type(&mut self, name: Name, ty: Idx) {
        self.const_types.insert(name, ty);
    }

    // ========================================
    // Environment Management
    // ========================================

    /// Freeze the current environment as the base.
    ///
    /// Called after signature collection to preserve function bindings.
    /// Function body checking creates child environments from this base.
    pub fn freeze_base_env(&mut self, env: TypeEnv) {
        self.base_env = Some(env);
    }

    /// Get a child of the frozen base environment.
    ///
    /// Returns `None` if the base hasn't been frozen yet.
    pub fn child_of_base(&self) -> Option<TypeEnv> {
        self.base_env.as_ref().map(TypeEnv::child)
    }

    /// Get the frozen base environment.
    pub fn base_env(&self) -> Option<&TypeEnv> {
        self.base_env.as_ref()
    }

    // ========================================
    // Error Management
    // ========================================

    /// Check if any errors have been accumulated.
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get accumulated errors.
    #[inline]
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.errors
    }

    /// Push a type check error.
    pub fn push_error(&mut self, error: TypeCheckError) {
        self.errors.push(error);
    }

    /// Push a type check warning.
    pub fn push_warning(&mut self, warning: TypeCheckWarning) {
        self.warnings.push(warning);
    }

    /// Report an undefined identifier error.
    pub fn error_undefined(&mut self, name: Name, span: Span) {
        self.errors
            .push(TypeCheckError::undefined_identifier(name, span));
    }

    // ========================================
    // Expression Types
    // ========================================

    /// Store the inferred type for an expression.
    ///
    /// Expression indices are assumed to be sequential starting from 0.
    /// If the index exceeds current capacity, the vector is extended.
    pub fn store_expr_type(&mut self, expr_index: usize, ty: Idx) {
        if expr_index >= self.expr_types.len() {
            self.expr_types.resize(expr_index + 1, Idx::ERROR);
        }
        self.expr_types[expr_index] = ty;
    }

    /// Get the inferred type for an expression.
    pub fn get_expr_type(&self, expr_index: usize) -> Option<Idx> {
        self.expr_types.get(expr_index).copied()
    }

    // ========================================
    // Inference Engine Creation
    // ========================================

    /// Create an inference engine for checking a scope.
    ///
    /// The engine borrows the pool mutably and starts with a fresh environment.
    /// Propagates capability state so the engine can validate call-site capabilities.
    pub fn create_engine(&mut self) -> InferEngine<'_> {
        let interner = self.interner;
        let well_known = &self.well_known;
        // Split borrow: pool (mut) + traits, signatures, types, consts (shared)
        let traits = &self.traits;
        let sigs = &self.signatures;
        let types = &self.types;
        let consts = &self.const_types;
        let impl_self = self.current_impl_self;
        let current_caps = self.current_capabilities.clone();
        let provided_caps = self.provided_capabilities.clone();
        let mut engine = InferEngine::new(&mut self.pool);
        engine.set_interner(interner);
        engine.set_well_known(well_known);
        engine.set_trait_registry(traits);
        engine.set_signatures(sigs);
        engine.set_type_registry(types);
        engine.set_const_types(consts);
        engine.set_capabilities(current_caps, provided_caps);
        if let Some(self_ty) = impl_self {
            engine.set_impl_self_type(self_ty);
        }
        engine
    }

    /// Create an inference engine with a specific environment.
    ///
    /// Use this when you need to start with pre-bound variables
    /// (e.g., function parameters).
    /// Propagates capability state so the engine can validate call-site capabilities.
    pub fn create_engine_with_env(&mut self, env: TypeEnv) -> InferEngine<'_> {
        let interner = self.interner;
        let well_known = &self.well_known;
        // Split borrow: pool (mut) + traits, signatures, types, consts (shared)
        let traits = &self.traits;
        let sigs = &self.signatures;
        let types = &self.types;
        let consts = &self.const_types;
        let impl_self = self.current_impl_self;
        let current_caps = self.current_capabilities.clone();
        let provided_caps = self.provided_capabilities.clone();
        let mut engine = InferEngine::with_env(&mut self.pool, env);
        engine.set_interner(interner);
        engine.set_well_known(well_known);
        engine.set_trait_registry(traits);
        engine.set_signatures(sigs);
        engine.set_type_registry(types);
        engine.set_const_types(consts);
        engine.set_capabilities(current_caps, provided_caps);
        if let Some(self_ty) = impl_self {
            engine.set_impl_self_type(self_ty);
        }
        engine
    }

    // ========================================
    // Context Management (RAII-style)
    // ========================================

    /// Execute a closure with a function scope.
    ///
    /// Sets up `current_function` and `current_capabilities` for the duration.
    pub fn with_function_scope<T, F>(
        &mut self,
        fn_type: Idx,
        capabilities: FxHashSet<Name>,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved_fn = self.current_function.replace(fn_type);
        let saved_caps = std::mem::replace(&mut self.current_capabilities, capabilities);

        let result = f(self);

        self.current_function = saved_fn;
        self.current_capabilities = saved_caps;

        result
    }

    /// Execute a closure with an impl scope.
    ///
    /// Sets up `current_impl_self` for the duration.
    pub fn with_impl_scope<T, F>(&mut self, self_ty: Idx, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved = self.current_impl_self.replace(self_ty);
        let result = f(self);
        self.current_impl_self = saved;
        result
    }

    /// Execute a closure with additional provided capabilities.
    ///
    /// Used for `with...in` expressions.
    pub fn with_provided_capabilities<T, F>(&mut self, caps: FxHashSet<Name>, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved = std::mem::take(&mut self.provided_capabilities);
        self.provided_capabilities = caps;
        let result = f(self);
        self.provided_capabilities = saved;
        result
    }

    // ========================================
    // Output Generation
    // ========================================

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
        let pool = self.pool;

        // Sort functions by name for deterministic output regardless of
        // FxHashMap iteration order. Required for Salsa's Eq comparison.
        let mut functions: Vec<FunctionSig> = self.signatures.into_values().collect();
        functions.sort_by_key(|f| f.name);

        // Extract type definitions (already sorted by name via BTreeMap).
        let types = self.types.into_entries();

        // Sort and dedup pattern resolutions for O(log n) binary search.
        let mut pattern_resolutions = self.pattern_resolutions;
        pattern_resolutions.sort_by_key(|(k, _)| *k);
        pattern_resolutions.dedup_by_key(|(k, _)| *k);

        let typed = TypedModule {
            expr_types: self.expr_types,
            functions,
            types,
            errors: self.errors,
            warnings: self.warnings,
            pattern_resolutions,
            impl_sigs: self.impl_sigs,
        };

        (TypeCheckResult::from_typed(typed), pool)
    }
}

#[cfg(test)]
mod tests;
