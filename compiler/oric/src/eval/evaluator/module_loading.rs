//! Module loading methods for the Evaluator.
//!
//! Provides Salsa-integrated module loading with proper dependency tracking.
//! Import resolution is handled by `imports::resolve_imports()` (unified pipeline);
//! this module consumes the resolved data to build interpreter-specific
//! `FunctionValue` objects and register them in the environment.

use super::super::module::import;
use super::Evaluator;
use crate::imports;
use crate::parser::ParseOutput;
use ori_eval::{
    collect_def_impl_methods_with_config, collect_extend_methods_with_config,
    collect_impl_methods_with_config, process_derives, register_module_bindings,
    DefaultFieldTypeRegistry, MethodCollectionConfig, UserMethodRegistry,
};
use ori_ir::canon::SharedCanonResult;
use std::path::Path;

impl Evaluator<'_> {
    /// Load a module: resolve imports and register all functions.
    ///
    /// This is the core module loading logic used by both the query system
    /// and test runner. It handles:
    /// 1. Auto-loading the prelude (if not already loaded)
    /// 2. Resolving imports and registering imported functions and methods
    /// 3. Registering all local functions
    /// 4. Registering all local impl block methods
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
        let resolved = imports::resolve_imports(self.db, parse_result, file_path);
        let interner = self.db.interner();

        if !self.prelude_loaded {
            self.prelude_loaded = true;
            if let Some(ref prelude) = resolved.prelude {
                let prelude_arena = prelude.parse_output.arena.clone();

                // Type-check and canonicalize prelude for canonical function dispatch.
                let prelude_canon = crate::query::canonicalize_module(
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

        // INVARIANT: all use-statement errors accumulate before returning.
        let mut import_errors = resolved.errors.clone();
        let mut user_methods = UserMethodRegistry::new();
        for imp_module in &resolved.modules {
            let imp = &parse_result.module.imports[imp_module.import_index];

            let imported_arena = imp_module.parse_output.arena.clone();

            // Type-check and canonicalize the imported module for canonical dispatch.
            let imp_canon = crate::query::canonicalize_module(
                self.db,
                &imp_module.parse_output,
                &imp_module.module_path,
                imp_module.source_file,
            );

            let imported_module = match import::ImportedModule::new(
                self.db,
                &imp_module.parse_output,
                &imported_arena,
                &imp_module.module_path,
                imp_canon.as_ref(),
            ) {
                Ok(imported) => imported,
                Err(errors) => {
                    import_errors.extend(errors);
                    continue;
                }
            };

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

            // INVARIANT: imported methods retain their defining arena, IR, and captures.
            let config = MethodCollectionConfig {
                module: &imp_module.parse_output.module,
                arena: &imported_arena,
                captures: imported_module.shared_captures(self.env()),
                canon: imp_canon.as_ref(),
                interner,
            };
            collect_module_methods(&config, &mut user_methods);
        }

        if !import_errors.is_empty() {
            return Err(import_errors);
        }

        // Clone the shared arena (O(1) Arc::clone) for methods in this module.
        // Methods carry their arena reference for correct evaluation
        // when called from different contexts (e.g., from within a prelude function).
        let shared_arena = parse_result.arena.clone();

        // INVARIANT: local constructors shadow same-named prelude captures.
        register_module_bindings(&parse_result.module, &shared_arena, self.env_mut(), canon);

        // Add this module's impl and extend blocks to the methods collected from
        // its imported providers.
        let config = MethodCollectionConfig {
            module: &parse_result.module,
            arena: &shared_arena,
            captures: std::sync::Arc::new(self.env().capture()),
            canon,
            interner: self.interner(),
        };
        collect_module_methods(&config, &mut user_methods);

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
}

fn collect_module_methods(config: &MethodCollectionConfig<'_>, registry: &mut UserMethodRegistry) {
    collect_impl_methods_with_config(config, registry);
    collect_extend_methods_with_config(config, registry);
    collect_def_impl_methods_with_config(config, registry);
}
