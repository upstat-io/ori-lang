//! Import and signature management for `ModuleChecker`.
//!
//! Handles registering imported functions, traits, built-in functions/values,
//! module aliases, function signature storage, and monomorphization accumulation.
//! Supports both hash-first resolution (O(1) per type) and AST fallback for
//! generic or missing types.

use ori_ir::{ExprArena, Name};
use rustc_hash::FxHashMap;

use super::signatures;
use super::ModuleChecker;
use crate::{FunctionSig, Idx, TypeEnv};

impl ModuleChecker<'_> {
    // Signature Registration

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

    /// Record a trait impl method name for unconstrained function detection.
    ///
    /// Trait impl methods may be called via dynamic dispatch — their parameter
    /// ranges must stay Top in interprocedural range analysis. Inherent impl
    /// methods are NOT recorded (they have known call sites).
    pub fn register_trait_impl_fn_name(&mut self, self_type: Idx, name: Name) {
        self.trait_impl_fn_names.push((self_type, name));
    }

    /// Accumulate mono instances from an inference engine pass.
    pub fn accumulate_mono_instances(&mut self, instances: Vec<crate::MonoInstance>) {
        self.mono_instances.extend(instances);
    }

    /// Accumulate deferred mono calls from an inference engine pass.
    pub fn accumulate_deferred_mono_calls(&mut self, calls: Vec<crate::DeferredMonoCall>) {
        self.deferred_mono_calls.extend(calls);
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

    // Import Registration

    /// Register an imported function for cross-module type checking.
    ///
    /// If `imported_sig` is provided (from the source module's `TypeCheckResult`),
    /// attempts hash-first resolution: looks up each parameter/return type by
    /// Merkle hash in the local pool (O(1) per type). Falls back to AST
    /// re-walking if any type is missing from the local pool or the function
    /// is generic (type variables are pool-local).
    ///
    /// Call this before `collect_signatures()` / `check_module_impl()` so
    /// that imported bindings are visible as the parent scope of local
    /// function signatures.
    pub fn register_imported_function(
        &mut self,
        func: &ori_ir::Function,
        foreign_arena: &ExprArena,
        imported_sig: Option<&FunctionSig>,
    ) {
        // Fast path: resolve non-generic signatures by Merkle hash
        if let Some(ext_sig) = imported_sig {
            if let Some(local_sig) = self.try_resolve_sig_by_hash(ext_sig) {
                tracing::debug!(
                    name = ?self.interner.lookup(local_sig.name),
                    "import: hash-first hit"
                );
                self.bind_imported_sig(local_sig);
                return;
            }
            tracing::debug!(
                name = ?self.interner.lookup(ext_sig.name),
                generic = !ext_sig.scheme_var_ids.is_empty(),
                "import: hash-first miss → AST fallback"
            );
        }

        // Slow path: AST re-walking (creates types in local pool)
        let (sig, var_ids) = signatures::infer_function_signature_from(self, func, foreign_arena);
        self.bind_imported_sig_with_vars(sig, &var_ids);
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
        imported_sig: Option<&FunctionSig>,
    ) {
        // Fast path: resolve non-generic signatures by Merkle hash
        if let Some(ext_sig) = imported_sig {
            if let Some(mut local_sig) = self.try_resolve_sig_by_hash(ext_sig) {
                tracing::debug!(
                    name = ?self.interner.lookup(ext_sig.name),
                    alias = ?self.interner.lookup(alias),
                    "import: hash-first hit (aliased)"
                );
                local_sig.name = alias;
                self.bind_imported_sig(local_sig);
                return;
            }
            tracing::debug!(
                name = ?self.interner.lookup(ext_sig.name),
                alias = ?self.interner.lookup(alias),
                generic = !ext_sig.scheme_var_ids.is_empty(),
                "import: hash-first miss → AST fallback (aliased)"
            );
        }

        // Slow path: AST re-walking
        let (mut sig, var_ids) =
            signatures::infer_function_signature_from(self, func, foreign_arena);
        sig.name = alias;
        self.bind_imported_sig_with_vars(sig, &var_ids);
    }

    /// Try to resolve all types in an imported signature by Merkle hash lookup.
    ///
    /// Returns `Some(local_sig)` if every param/return type already exists in
    /// the local pool. Returns `None` if any lookup misses (caller should fall
    /// back to AST re-walking).
    ///
    /// Generic functions (non-empty `scheme_var_ids`) always return `None`
    /// because type variable hashes are pool-local and won't match across pools.
    fn try_resolve_sig_by_hash(&self, ext_sig: &FunctionSig) -> Option<FunctionSig> {
        // Generic functions: type variables are pool-local, hash won't match
        if !ext_sig.scheme_var_ids.is_empty() {
            return None;
        }

        // Try to resolve every param type by hash
        let mut local_param_types = Vec::with_capacity(ext_sig.param_types.len());
        for &hash in &ext_sig.param_hashes {
            local_param_types.push(self.pool.lookup_by_hash(hash)?);
        }

        // Try to resolve return type by hash
        let local_return_type = self.pool.lookup_by_hash(ext_sig.return_hash)?;

        // All types resolved — build local FunctionSig
        Some(FunctionSig {
            name: ext_sig.name,
            type_params: ext_sig.type_params.clone(),
            const_params: ext_sig.const_params.clone(),
            param_names: ext_sig.param_names.clone(),
            param_types: local_param_types,
            return_type: local_return_type,
            capabilities: ext_sig.capabilities.clone(),
            is_public: ext_sig.is_public,
            is_test: ext_sig.is_test,
            is_main: ext_sig.is_main,
            is_fbip: ext_sig.is_fbip,
            type_param_bounds: ext_sig.type_param_bounds.clone(),
            where_clauses: ext_sig.where_clauses.clone(),
            generic_param_mapping: ext_sig.generic_param_mapping.clone(),
            scheme_var_ids: ext_sig.scheme_var_ids.clone(),
            required_params: ext_sig.required_params,
            param_defaults: ext_sig.param_defaults.clone(),
            param_hashes: ext_sig.param_hashes.clone(),
            return_hash: ext_sig.return_hash,
        })
    }

    /// Bind an imported signature into the import environment (non-generic path).
    ///
    /// Used by the hash-first resolution path where we know `scheme_var_ids` is empty.
    fn bind_imported_sig(&mut self, sig: FunctionSig) {
        let fn_type = self.pool.function(&sig.param_types, sig.return_type);
        self.import_env.bind(sig.name, fn_type);
        self.signatures.insert(sig.name, sig);
    }

    /// Bind an imported signature, wrapping generic functions in a type scheme.
    ///
    /// Used by the AST fallback path where we have explicit var IDs.
    fn bind_imported_sig_with_vars(&mut self, sig: FunctionSig, var_ids: &[u32]) {
        let fn_type = self.pool.function(&sig.param_types, sig.return_type);

        // Wrap generic functions in a type scheme so each call gets fresh
        // type variables via instantiation (prevents shared-variable pollution).
        let bound_type = if var_ids.is_empty() {
            fn_type
        } else {
            self.pool.scheme(var_ids, fn_type)
        };

        self.import_env.bind(sig.name, bound_type);
        self.signatures.insert(sig.name, sig);
    }

    /// Register traits from an imported module (e.g., prelude).
    ///
    /// Uses the foreign module's arena to resolve generic params and method
    /// signatures. Only public traits are registered.
    pub fn register_imported_traits(&mut self, module: &ori_ir::Module, foreign_arena: &ExprArena) {
        super::registration::register_imported_traits(self, module, foreign_arena);
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
        param_types: &[crate::Idx],
        return_type: crate::Idx,
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
    pub fn register_builtin_value(&mut self, name: Name, ty: crate::Idx) {
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
}
