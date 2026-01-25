//! Module loading methods for the Evaluator.

use std::path::Path;
use crate::ir::{Name, SharedArena};
use crate::parser::ParseResult;
use crate::context::SharedRegistry;
use super::Evaluator;
use super::super::user_methods::{UserMethodRegistry, UserMethod};
use super::super::module::import;

impl Evaluator<'_> {
    /// Find the prelude path by searching for library/std/prelude.si
    pub(super) fn find_prelude_path(current_file: &Path) -> Option<std::path::PathBuf> {
        // Walk up from current file to find project root (contains library/)
        let mut dir = current_file.parent();
        while let Some(d) = dir {
            let prelude_path = d.join("library").join("std").join("prelude.si");
            if prelude_path.exists() {
                return Some(prelude_path);
            }
            dir = d.parent();
        }
        None
    }

    /// Check if a file is the prelude itself.
    pub(super) fn is_prelude_file(file_path: &Path) -> bool {
        file_path.ends_with("library/std/prelude.si")
            || file_path.file_name().is_some_and(|n| n == "prelude.si")
                && file_path.parent().is_some_and(|p| p.ends_with("std"))
    }

    /// Auto-load the prelude (library/std/prelude.si).
    ///
    /// This is called automatically by `load_module` to make prelude functions
    /// available without explicit import (like Rust's `std::prelude`).
    pub(super) fn load_prelude(&mut self, current_file: &Path) -> Result<(), String> {
        // Don't load prelude if we're already loading it (avoid infinite recursion)
        if Self::is_prelude_file(current_file) {
            self.prelude_loaded = true;
            return Ok(());
        }

        // Find the prelude path
        let Some(prelude_path) = Self::find_prelude_path(current_file) else {
            // Prelude not found - this is okay, just skip it
            // (e.g., running tests outside project directory)
            self.prelude_loaded = true;
            return Ok(());
        };

        // Mark as loaded before actually loading to prevent recursion
        self.prelude_loaded = true;

        // Load and parse the prelude
        let prelude_result = import::load_imported_module(&prelude_path, self.interner)
            .map_err(|e| e.message)?;

        let prelude_arena = SharedArena::new(prelude_result.arena.clone());
        let module_functions = import::build_module_functions(&prelude_result, &prelude_arena);

        // Register all public functions from the prelude into the global environment
        for func in &prelude_result.module.functions {
            if func.is_public {
                if let Some(value) = module_functions.get(&func.name) {
                    self.env.define_global(func.name, value.clone());
                }
            }
        }

        Ok(())
    }

    /// Load a module: resolve imports and register all functions.
    ///
    /// This is the core module loading logic used by both the query system
    /// and test runner. It handles:
    /// 1. Auto-loading the prelude (if not already loaded)
    /// 2. Resolving imports and registering imported functions
    /// 3. Registering all local functions
    /// 4. Registering all impl block methods
    ///
    /// After calling this, all functions from the module (and its imports)
    /// are available in the environment for evaluation.
    pub fn load_module(
        &mut self,
        parse_result: &ParseResult,
        file_path: &Path,
    ) -> Result<(), String> {
        // Auto-load prelude if not already loaded and this isn't the prelude itself
        if !self.prelude_loaded {
            self.load_prelude(file_path)?;
        }

        // First, resolve imports
        for imp in &parse_result.module.imports {
            let import_path = import::resolve_import_path(&imp.path, file_path, self.interner)
                .map_err(|e| e.message)?;

            let imported_result = import::load_imported_module(&import_path, self.interner)
                .map_err(|e| e.message)?;

            let imported_arena = SharedArena::new(imported_result.arena.clone());
            let imported_module = import::ImportedModule::new(&imported_result, &imported_arena);

            import::register_imports(
                imp,
                &imported_module,
                &mut self.env,
                self.interner,
                &import_path,
                file_path,
            ).map_err(|e| e.message)?;
        }

        // Then register all local functions
        import::register_module_functions(parse_result, &mut self.env);

        // Build up user method registry from impl and extend blocks
        let mut user_methods = UserMethodRegistry::new();
        self.collect_impl_methods(&parse_result.module, &parse_result.arena, &mut user_methods);
        self.collect_extend_methods(&parse_result.module, &parse_result.arena, &mut user_methods);

        // Replace the shared registry with the built-up one
        self.user_method_registry = SharedRegistry::new(user_methods);

        Ok(())
    }

    /// Collect methods from impl blocks into a registry.
    pub(super) fn collect_impl_methods(&self, module: &crate::ir::Module, arena: &crate::ir::ExprArena, registry: &mut UserMethodRegistry) {
        for impl_def in &module.impls {
            // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
            let type_name = match impl_def.self_path.last() {
                // Use the last segment of the path as the type name
                Some(&name) => self.interner.lookup(name).to_string(),
                None => continue, // Skip if no type path
            };

            // Register each method
            for method in &impl_def.methods {
                let method_name = self.interner.lookup(method.name).to_string();

                // Get parameter names
                let params: Vec<Name> = arena.get_params(method.params)
                    .iter()
                    .map(|p| p.name)
                    .collect();

                // Create user method with captures from current environment
                let user_method = UserMethod::with_captures(
                    params,
                    method.body,
                    self.env.capture(),
                );

                registry.register(type_name.clone(), method_name, user_method);
            }
        }
    }

    /// Collect methods from extend blocks into a registry.
    pub(super) fn collect_extend_methods(&self, module: &crate::ir::Module, arena: &crate::ir::ExprArena, registry: &mut UserMethodRegistry) {
        for extend_def in &module.extends {
            // Get the target type name (e.g., "list" for `extend [T] { ... }`)
            let type_name = self.interner.lookup(extend_def.target_type_name).to_string();

            // Register each method
            for method in &extend_def.methods {
                let method_name = self.interner.lookup(method.name).to_string();

                // Get parameter names
                let params: Vec<Name> = arena.get_params(method.params)
                    .iter()
                    .map(|p| p.name)
                    .collect();

                // Create user method with captures from current environment
                let user_method = UserMethod::with_captures(
                    params,
                    method.body,
                    self.env.capture(),
                );

                registry.register(type_name.clone(), method_name, user_method);
            }
        }
    }
}
