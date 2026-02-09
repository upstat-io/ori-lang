//! Module loading methods for the Evaluator.
//!
//! Provides Salsa-integrated module loading with proper dependency tracking.
//! Import resolution is handled by `imports::resolve_imports()` (unified pipeline);
//! this module consumes the resolved data to build interpreter-specific
//! `FunctionValue` objects and register them in the environment.

use super::super::module::import;
use super::Evaluator;
use crate::imports;
use crate::ir::SharedArena;
use crate::parser::ParseOutput;
use ori_eval::{
    collect_def_impl_methods, collect_extend_methods, collect_impl_methods, process_derives,
    register_module_functions, register_newtype_constructors, register_variant_constructors,
    UserMethodRegistry,
};
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
    pub fn load_module(
        &mut self,
        parse_result: &ParseOutput,
        file_path: &Path,
    ) -> Result<(), String> {
        // Resolve all imports via the unified pipeline (prelude + explicit use statements).
        let resolved = imports::resolve_imports(self.db, parse_result, file_path);

        // Register prelude functions (if not already loaded)
        if !self.prelude_loaded {
            self.prelude_loaded = true;
            if let Some(ref prelude) = resolved.prelude {
                let prelude_arena = SharedArena::new(prelude.parse_output.arena.clone());
                let module_functions =
                    import::build_module_functions(&prelude.parse_output, &prelude_arena);

                for func in &prelude.parse_output.module.functions {
                    if func.visibility.is_public() {
                        if let Some(value) = module_functions.get(&func.name) {
                            self.env_mut().define_global(func.name, value.clone());
                        }
                    }
                }
            }
        }

        // Report import resolution errors
        if let Some(error) = resolved.errors.first() {
            return Err(error.message.clone());
        }

        // Register explicitly imported functions.
        // Each resolved module carries its import_index so we can find
        // the corresponding UseDef for visibility/alias handling.
        for imp_module in &resolved.modules {
            let imp = &parse_result.module.imports[imp_module.import_index];

            let imported_arena = SharedArena::new(imp_module.parse_output.arena.clone());
            let imported_module =
                import::ImportedModule::new(&imp_module.parse_output, &imported_arena);

            let interner = self.interpreter.interner;
            let import_path = std::path::Path::new(&imp_module.module_path);
            import::register_imports(
                imp,
                &imported_module,
                &mut self.interpreter.env,
                interner,
                import_path,
                file_path,
            )
            .map_err(|e| e.message)?;
        }

        // Create a shared arena for all methods in this module
        // This ensures methods carry their arena reference for correct evaluation
        // when called from different contexts (e.g., from within a prelude function)
        let shared_arena = SharedArena::new(parse_result.arena.clone());

        // Then register all local functions
        register_module_functions(&parse_result.module, &shared_arena, self.env_mut());

        // Register variant constructors from type declarations
        register_variant_constructors(&parse_result.module, self.env_mut());

        // Register newtype constructors from type declarations
        register_newtype_constructors(&parse_result.module, self.env_mut());

        // Build up user method registry from impl and extend blocks
        // Wrap captures in Arc once for efficient sharing across all collect_* calls
        let mut user_methods = UserMethodRegistry::new();
        let captures = std::sync::Arc::new(self.env().capture());
        collect_impl_methods(
            &parse_result.module,
            &shared_arena,
            &captures,
            &mut user_methods,
        );
        collect_extend_methods(
            &parse_result.module,
            &shared_arena,
            &captures,
            &mut user_methods,
        );
        collect_def_impl_methods(
            &parse_result.module,
            &shared_arena,
            &captures,
            &mut user_methods,
        );

        // Process derived traits (Eq, Clone, Hashable, Printable, Default)
        process_derives(&parse_result.module, &mut user_methods, self.interner());

        // Merge the collected methods into the existing registry.
        // Using merge() instead of replacing allows the cached MethodDispatcher
        // to see the new methods (since SharedMutableRegistry provides interior mutability).
        self.user_method_registry().write().merge(user_methods);

        Ok(())
    }
}
