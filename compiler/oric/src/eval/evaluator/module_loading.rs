//! Module loading methods for the Evaluator.
//!
//! Provides Salsa-integrated module loading with proper dependency tracking.
//! Import resolution is handled by `imports::resolve_imports()` (unified pipeline);
//! this module consumes the resolved data to build interpreter-specific
//! `FunctionValue` objects and register them in the environment.

use super::super::module::import;
use super::Evaluator;
use crate::imports;
use crate::input::SourceFile;
use crate::parser::ParseOutput;
use ori_eval::{
    collect_def_impl_methods_with_config, collect_extend_methods_with_config,
    collect_impl_methods_with_config, process_derives, register_module_functions,
    register_newtype_constructors, register_variant_constructors, DefaultFieldTypeRegistry,
    MethodCollectionConfig, UserMethodRegistry,
};
use ori_ir::canon::SharedCanonResult;
use std::path::Path;

impl Evaluator<'_> {
    /// Load a module: resolve imports and register all functions.
    ///
    /// This is the core module loading logic used by both the query system
    /// and test runner. It handles:
    /// 1. Auto-loading the prelude (if not already loaded)
    /// 2. Resolving imports and registering imported functions
    /// 3. Registering all local functions
    /// 4. Registering all impl block methods
    ///
    /// Import resolution uses the unified `imports::resolve_imports()` pipeline,
    /// which handles prelude discovery and `use` statement resolution via Salsa.
    /// The interpreter consumes the resolved data to build `FunctionValue` objects
    /// with captures and register them in the environment.
    ///
    /// When canonical IR is available (via `canon`), imported modules are also
    /// type-checked and canonicalized so that imported functions have canonical
    /// bodies. This ensures the evaluator uses `eval_can(CanId)` for all function
    /// calls, including cross-module ones.
    pub(crate) fn load_module(
        &mut self,
        parse_result: &ParseOutput,
        file_path: &Path,
        canon: Option<&SharedCanonResult>,
    ) -> Result<(), Vec<imports::ImportError>> {
        // Resolve all imports via the unified pipeline (prelude + explicit use statements).
        let resolved = imports::resolve_imports(self.db, parse_result, file_path);
        let interner = self.db.interner();

        // Register prelude functions (if not already loaded)
        if !self.prelude_loaded {
            self.prelude_loaded = true;
            if let Some(ref prelude) = resolved.prelude {
                let prelude_arena = prelude.parse_output.arena.clone();

                // Type-check and canonicalize prelude for canonical function dispatch.
                let prelude_canon = Self::canonicalize_module(
                    self.db,
                    &prelude.parse_output,
                    &prelude.module_path,
                    prelude.source_file,
                );

                let module_functions = import::build_module_functions(
                    &prelude.parse_output,
                    &prelude_arena,
                    prelude_canon.as_ref(),
                );

                for func in &prelude.parse_output.module.functions {
                    if func.visibility.is_public() {
                        if let Some(value) = module_functions.get(&func.name) {
                            self.env_mut().define_global(func.name, value.clone());
                        }
                    }
                }
            }
        }

        // Report all import resolution errors (accumulate, don't bail on the first).
        // Returns structured errors so callers can report each individually with spans.
        // Clone is acceptable: cold path (error-only), small strings, and `resolved`
        // is Arc<ResolvedImports> — avoiding the clone would require changing the
        // return type to carry the Arc, rippling through all callers for minimal gain.
        if !resolved.errors.is_empty() {
            return Err(resolved.errors.clone());
        }

        // Register explicitly imported functions.
        // Each resolved module carries its import_index so we can find
        // the corresponding UseDef for visibility/alias handling.
        // Accumulate errors across all use statements so the user sees every
        // problem at once, not just the first failing import.
        let mut import_errors = Vec::new();
        for imp_module in &resolved.modules {
            let imp = &parse_result.module.imports[imp_module.import_index];

            let imported_arena = imp_module.parse_output.arena.clone();

            // Type-check and canonicalize the imported module for canonical dispatch.
            let imp_canon = Self::canonicalize_module(
                self.db,
                &imp_module.parse_output,
                &imp_module.module_path,
                imp_module.source_file,
            );

            let imported_module = import::ImportedModule::new(
                &imp_module.parse_output,
                &imported_arena,
                imp_canon.as_ref(),
            );

            if let Err(errs) = import::register_imports(
                imp,
                &imported_module,
                self.env_mut(),
                interner,
                &imp_module.module_path,
                file_path,
                imp_canon.as_ref(),
            ) {
                import_errors.extend(errs);
            }
        }

        if !import_errors.is_empty() {
            return Err(import_errors);
        }

        // Clone the shared arena (O(1) Arc::clone) for methods in this module.
        // Methods carry their arena reference for correct evaluation
        // when called from different contexts (e.g., from within a prelude function).
        let shared_arena = parse_result.arena.clone();

        // Then register all local functions (with canonical IR when available)
        register_module_functions(&parse_result.module, &shared_arena, self.env_mut(), canon);

        // Register variant constructors from type declarations
        register_variant_constructors(&parse_result.module, self.env_mut());

        // Register newtype constructors from type declarations
        register_newtype_constructors(&parse_result.module, self.env_mut());

        // Build up user method registry from impl and extend blocks
        let mut user_methods = UserMethodRegistry::new();
        let config = MethodCollectionConfig {
            module: &parse_result.module,
            arena: &shared_arena,
            captures: std::sync::Arc::new(self.env().capture()),
            canon,
            interner: self.interner(),
        };
        collect_impl_methods_with_config(&config, &mut user_methods);
        collect_extend_methods_with_config(&config, &mut user_methods);
        collect_def_impl_methods_with_config(&config, &mut user_methods);

        // Process derived traits (Eq, Clone, Hashable, Printable, Default)
        let mut default_ft = DefaultFieldTypeRegistry::new();
        process_derives(
            &parse_result.module,
            &mut user_methods,
            &mut default_ft,
            self.interner(),
        );

        // Merge the collected methods into the existing registry.
        // Using merge() instead of replacing allows the cached MethodDispatcher
        // to see the new methods (since SharedMutableRegistry provides interior mutability).
        self.user_method_registry().write().merge(user_methods);
        self.default_field_types().write().merge(default_ft);

        Ok(())
    }

    /// Type-check and canonicalize a module, returning its `SharedCanonResult`.
    ///
    /// This enables imported functions to carry canonical IR for `eval_can()`
    /// dispatch. Results are cached in the session-scoped `CanonCache` so that
    /// repeated calls for the same module (e.g., the prelude across multiple
    /// Evaluator instances in the test runner) avoid redundant work.
    ///
    /// When a `SourceFile` is available, type checking goes through Salsa queries
    /// (`typed()` + `typed_pool()`), ensuring results are cached in Salsa's
    /// dependency graph and the Pool is stored in `PoolCache`. Falls back to
    /// direct `type_check_with_imports_and_pool()` only when no `SourceFile`
    /// exists (e.g., synthetic modules not loaded through Salsa).
    ///
    /// Skips canonicalization (returns `None`) when type errors are present,
    /// unlike `canonicalize_cached()` which always canonicalizes. This is
    /// intentional: imported modules with type errors should not produce
    /// incomplete canonical IR that could cause evaluator crashes, whereas
    /// the `check` command needs canon IR even with errors to detect pattern
    /// exhaustiveness problems.
    fn canonicalize_module(
        db: &dyn crate::db::Db,
        parse_output: &ParseOutput,
        module_path: &std::path::Path,
        source_file: Option<SourceFile>,
    ) -> Option<SharedCanonResult> {
        // Check cache first — avoids re-type-checking + re-canonicalizing
        // the same module across Evaluator instances.
        if let Some(cached) = db.canon_cache().get(module_path) {
            return Some(cached);
        }

        // Type-check via shared helper (Salsa queries when SourceFile is available,
        // direct type checking otherwise).
        let (type_result, pool) =
            crate::query::type_check_module(db, parse_output, module_path, source_file)?;

        // Only canonicalize if there are no type errors — otherwise the
        // canonical IR may be incomplete or inconsistent.
        if type_result.has_errors() {
            tracing::debug!(
                module = %module_path.display(),
                errors = type_result.errors().len(),
                "skipping canonicalization due to type errors"
            );
            return None;
        }

        // Delegate to the shared cache→compute→store helper.
        Some(crate::query::canonicalize_cached_by_path(
            db,
            module_path,
            parse_output,
            &type_result,
            &pool,
        ))
    }
}
