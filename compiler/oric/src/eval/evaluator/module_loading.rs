//! Module loading methods for the Evaluator.
//!
//! Provides Salsa-integrated module loading with proper dependency tracking.
//! All file access goes through `db.load_file()`.

use super::super::module::import;
use super::Evaluator;
use crate::ir::{Name, SharedArena, TypeDeclKind};
use crate::parser::ParseResult;
use crate::query::parsed;
use crate::typeck::derives::process_derives;
use crate::typeck::type_registry::TypeRegistry;
use ori_eval::{UserMethod, UserMethodRegistry, Value};
use std::path::{Path, PathBuf};

impl Evaluator<'_> {
    /// Generate candidate paths for the prelude.
    fn prelude_candidates(current_file: &Path) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let mut dir = current_file.parent();
        while let Some(d) = dir {
            candidates.push(d.join("library").join("std").join("prelude.ori"));
            dir = d.parent();
        }
        candidates
    }

    /// Check if a file is the prelude itself.
    pub(super) fn is_prelude_file(file_path: &Path) -> bool {
        file_path.ends_with("library/std/prelude.ori")
            || file_path.file_name().is_some_and(|n| n == "prelude.ori")
                && file_path.parent().is_some_and(|p| p.ends_with("std"))
    }

    /// Auto-load the prelude (library/std/prelude.ori).
    ///
    /// This is called automatically by `load_module` to make prelude functions
    /// available without explicit import. All file access goes through
    /// `db.load_file()` for proper Salsa tracking.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Result return type maintained for API consistency with load_module"
    )]
    pub(super) fn load_prelude(&mut self, current_file: &Path) -> Result<(), String> {
        // Don't load prelude if we're already loading it (avoid infinite recursion)
        if Self::is_prelude_file(current_file) {
            self.prelude_loaded = true;
            return Ok(());
        }

        // Mark as loaded before actually loading to prevent recursion
        self.prelude_loaded = true;

        // Find and load prelude via Salsa-tracked file loading
        let prelude_file = Self::prelude_candidates(current_file)
            .iter()
            .find_map(|candidate| self.db.load_file(candidate));

        let Some(prelude_file) = prelude_file else {
            // Prelude not found - this is okay (e.g., tests outside project)
            return Ok(());
        };

        let prelude_result = parsed(self.db, prelude_file);
        let prelude_arena = SharedArena::new(prelude_result.arena.clone());
        let module_functions = import::build_module_functions(&prelude_result, &prelude_arena);

        // Register all public functions from the prelude into the global environment
        for func in &prelude_result.module.functions {
            if func.is_public {
                if let Some(value) = module_functions.get(&func.name) {
                    self.env_mut().define_global(func.name, value.clone());
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
    /// All file access goes through `db.load_file()` for proper Salsa tracking.
    ///
    /// After calling this, all functions from the module (and its imports)
    /// are available in the environment for evaluation.
    ///
    /// Note: Type checking should be done by the caller before calling this method.
    /// The type checker doesn't resolve imports, so it must be called on the resolved
    /// module context, not on individual files in isolation.
    pub fn load_module(
        &mut self,
        parse_result: &ParseResult,
        file_path: &Path,
    ) -> Result<(), String> {
        // Auto-load prelude if not already loaded and this isn't the prelude itself
        if !self.prelude_loaded {
            self.load_prelude(file_path)?;
        }

        // Resolve and load imports via Salsa-tracked resolution
        for imp in &parse_result.module.imports {
            let resolved =
                import::resolve_import(self.db, &imp.path, file_path).map_err(|e| e.message)?;
            let imported_result = parsed(self.db, resolved.file);

            let imported_arena = SharedArena::new(imported_result.arena.clone());
            let imported_module = import::ImportedModule::new(&imported_result, &imported_arena);

            // Access interner directly from interpreter to avoid borrow conflict
            let interner = self.interpreter.interner;
            import::register_imports(
                imp,
                &imported_module,
                &mut self.interpreter.env,
                interner,
                &resolved.path,
                file_path,
            )
            .map_err(|e| e.message)?;
        }

        // Then register all local functions
        import::register_module_functions(parse_result, self.env_mut());

        // Register variant constructors from type declarations
        self.register_variant_constructors(&parse_result.module);

        // Create a shared arena for all methods in this module
        // This ensures methods carry their arena reference for correct evaluation
        // when called from different contexts (e.g., from within a prelude function)
        let shared_arena = SharedArena::new(parse_result.arena.clone());

        // Build up user method registry from impl and extend blocks
        let mut user_methods = UserMethodRegistry::new();
        self.collect_impl_methods(&parse_result.module, &shared_arena, &mut user_methods);
        self.collect_extend_methods(&parse_result.module, &shared_arena, &mut user_methods);

        // Process derived traits (Eq, Clone, Hashable, Printable, Default)
        // Note: We use an empty TypeRegistry here since derive processing doesn't need it
        // (field information comes from the AST, not the type registry)
        let type_registry = TypeRegistry::new();
        process_derives(
            &parse_result.module,
            &type_registry,
            &mut user_methods,
            self.interner(),
        );

        // Merge the collected methods into the existing registry.
        // Using merge() instead of replacing allows the cached MethodDispatcher
        // to see the new methods (since SharedMutableRegistry provides interior mutability).
        self.user_method_registry().write().merge(user_methods);

        Ok(())
    }

    /// Collect methods from impl blocks into a registry.
    ///
    /// Takes a `SharedArena` so that methods carry their arena reference for
    /// correct evaluation when called from different contexts.
    pub(super) fn collect_impl_methods(
        &self,
        module: &crate::ir::Module,
        arena: &SharedArena,
        registry: &mut UserMethodRegistry,
    ) {
        // First, build a map of trait names to their definitions for default method lookup
        let mut trait_map: std::collections::HashMap<Name, &crate::ir::TraitDef> =
            std::collections::HashMap::new();
        for trait_def in &module.traits {
            trait_map.insert(trait_def.name, trait_def);
        }

        for impl_def in &module.impls {
            // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
            let Some(&type_name) = impl_def.self_path.last() else {
                continue; // Skip if no type path
            };

            // Collect names of methods explicitly defined in this impl
            let mut overridden_methods: std::collections::HashSet<Name> =
                std::collections::HashSet::new();

            // Register each explicitly defined method
            for method in &impl_def.methods {
                overridden_methods.insert(method.name);

                // Get parameter names
                let params = arena.get_param_names(method.params);

                // Create user method with captures and arena
                let user_method =
                    UserMethod::new(params, method.body, self.env().capture(), arena.clone());

                registry.register(type_name, method.name, user_method);
            }

            // For trait impls, also register default trait methods that weren't overridden
            if let Some(trait_path) = &impl_def.trait_path {
                if let Some(&trait_name) = trait_path.last() {
                    if let Some(trait_def) = trait_map.get(&trait_name) {
                        for item in &trait_def.items {
                            if let crate::ir::TraitItem::DefaultMethod(default_method) = item {
                                // Only register if not overridden
                                if !overridden_methods.contains(&default_method.name) {
                                    let params = arena.get_param_names(default_method.params);

                                    let user_method = UserMethod::new(
                                        params,
                                        default_method.body,
                                        self.env().capture(),
                                        arena.clone(),
                                    );

                                    registry.register(type_name, default_method.name, user_method);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Collect methods from extend blocks into a registry.
    ///
    /// Takes a `SharedArena` so that methods carry their arena reference for
    /// correct evaluation when called from different contexts.
    pub(super) fn collect_extend_methods(
        &self,
        module: &crate::ir::Module,
        arena: &SharedArena,
        registry: &mut UserMethodRegistry,
    ) {
        for extend_def in &module.extends {
            // Get the target type name (e.g., "list" for `extend [T] { ... }`)
            let type_name = extend_def.target_type_name;

            // Register each method
            for method in &extend_def.methods {
                // Get parameter names
                let params = arena.get_param_names(method.params);

                // Create user method with captures and arena
                let user_method =
                    UserMethod::new(params, method.body, self.env().capture(), arena.clone());

                registry.register(type_name, method.name, user_method);
            }
        }
    }

    /// Register variant constructors from sum type declarations.
    ///
    /// For each sum type (enum), registers each variant as a constructor:
    /// - Unit variants (no fields) are bound directly as `Value::Variant`
    /// - Variants with fields are bound as constructor functions
    fn register_variant_constructors(&mut self, module: &crate::ir::Module) {
        for type_decl in &module.types {
            if let TypeDeclKind::Sum(variants) = &type_decl.kind {
                let type_name = type_decl.name;

                for variant in variants {
                    if variant.fields.is_empty() {
                        // Unit variant: bind directly as Value::Variant
                        let value = Value::variant(type_name, variant.name, vec![]);
                        self.env_mut().define_global(variant.name, value);
                    } else {
                        // Variant with fields: create a constructor function
                        // For now, we'll use a special VariantConstructor value type
                        // that the evaluator recognizes during function calls
                        let value = Value::variant_constructor(
                            type_name,
                            variant.name,
                            variant.fields.len(),
                        );
                        self.env_mut().define_global(variant.name, value);
                    }
                }
            }
        }
    }
}
