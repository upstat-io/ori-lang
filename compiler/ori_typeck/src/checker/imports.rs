//! Imported function and module alias support for type checking.
//!
//! Provides types and methods for registering imported function signatures
//! and module aliases in the type checker before checking local function bodies.
//!
//! # Usage
//!
//! 1. Resolve imports to get `ImportedFunction` from each imported module
//! 2. Call `TypeChecker::register_imported_functions()` before `check_module()`
//! 3. Imported functions will be available for call type checking
//!
//! For module aliases (`use std.http as http`):
//! 1. Create `ImportedModuleAlias` with the alias name and exported functions
//! 2. Call `TypeChecker::register_module_alias()` before `check_module()`
//! 3. Qualified access like `http.get(...)` will be type checked

use super::types::{FunctionType, GenericBound};
use super::TypeChecker;
use ori_ir::Name;
use ori_types::Type;

/// A generic parameter with its trait bounds for imported functions.
///
/// This is a portable representation that doesn't depend on `TypeIds`.
#[derive(Clone, Debug)]
pub struct ImportedGeneric {
    /// The generic parameter name (e.g., `T` in `<T: Eq>`)
    pub param: Name,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`)
    pub bounds: Vec<Vec<Name>>,
}

/// An imported function signature for type checking.
///
/// This is a portable representation that uses `Type` instead of `TypeId`,
/// allowing it to be used across different type checking contexts.
///
/// # Creating `ImportedFunction`
///
/// Convert from a typed module's `FunctionType`:
///
/// ```text
/// let imported = ImportedFunction::from_function_type(&func_type, interner);
/// ```
#[derive(Clone, Debug)]
pub struct ImportedFunction {
    /// Function name
    pub name: Name,
    /// Parameter types (portable Type, not `TypeId`)
    pub params: Vec<Type>,
    /// Return type (portable Type, not `TypeId`)
    pub return_type: Type,
    /// Generic parameters with their bounds
    pub generics: Vec<ImportedGeneric>,
    /// Capabilities required by this function
    pub capabilities: Vec<Name>,
}

impl ImportedFunction {
    /// Create an `ImportedFunction` from a `FunctionType` and its type interner.
    ///
    /// This converts `TypeIds` to Types, making the result portable across
    /// different type checking contexts.
    pub fn from_function_type(
        func_type: &FunctionType,
        interner: &ori_types::TypeInterner,
    ) -> Self {
        // Convert param TypeIds to Types
        let params: Vec<Type> = func_type
            .params
            .iter()
            .map(|&type_id| interner.to_type(type_id))
            .collect();

        // Convert return type
        let return_type = interner.to_type(func_type.return_type);

        // Convert generics (bounds are already Names, just copy them)
        let generics: Vec<ImportedGeneric> = func_type
            .generics
            .iter()
            .map(|g| ImportedGeneric {
                param: g.param,
                bounds: g.bounds.clone(),
            })
            .collect();

        ImportedFunction {
            name: func_type.name,
            params,
            return_type,
            generics,
            capabilities: func_type.capabilities.clone(),
        }
    }
}

/// A module alias import for type checking.
///
/// This represents a `use std.http as http` style import where the entire
/// module is imported under an alias name, enabling qualified access like
/// `http.get(...)`.
#[derive(Clone, Debug)]
pub struct ImportedModuleAlias {
    /// The alias name (e.g., `http` in `use std.http as http`)
    pub alias: Name,
    /// The exported functions from the module
    pub functions: Vec<ImportedFunction>,
}

impl TypeChecker<'_> {
    /// Register a module alias for type checking.
    ///
    /// This should be called before `check_module()` to make module aliases
    /// available for qualified access type checking.
    ///
    /// # Example
    ///
    /// ```text
    /// // For: use std.http as http
    /// let alias = ImportedModuleAlias {
    ///     alias: http_name,
    ///     functions: vec![get_fn, post_fn, ...],
    /// };
    /// checker.register_module_alias(&alias);
    /// // Now `http.get(...)` can be type checked
    /// ```
    pub fn register_module_alias(&mut self, module_alias: &ImportedModuleAlias) {
        // Build the module namespace type with all exported functions
        let mut items: Vec<(Name, Type)> = module_alias
            .functions
            .iter()
            .map(|func| {
                let fn_type = Type::Function {
                    params: func.params.clone(),
                    ret: Box::new(func.return_type.clone()),
                };
                (func.name, fn_type)
            })
            .collect();

        // Sort by Name for O(log n) binary search lookup (ModuleNamespace invariant)
        items.sort_by_key(|(name, _)| *name);

        let namespace_type = Type::ModuleNamespace { items };

        // Bind the module alias to the namespace type in the environment
        self.inference.env.bind(module_alias.alias, namespace_type);
    }

    /// Register imported function signatures for type checking.
    ///
    /// This should be called before `check_module()` to make imported
    /// functions available during type checking of the current module.
    ///
    /// The imported functions will be:
    /// 1. Added to the type environment so calls can be type checked
    /// 2. Added to `function_sigs` for generic constraint checking
    ///
    /// # Example
    ///
    /// ```text
    /// let checker = TypeChecker::new(arena, interner);
    /// checker.register_imported_functions(&imported_functions);
    /// let typed = checker.check_module(&module);
    /// ```
    pub fn register_imported_functions(&mut self, imports: &[ImportedFunction]) {
        // First pass: collect all data that needs the interner (immutable borrow)
        struct PreparedImport {
            name: Name,
            params: Vec<ori_ir::TypeId>,
            return_type: ori_ir::TypeId,
            fn_type: Type,
            capabilities: Vec<Name>,
        }

        let prepared: Vec<PreparedImport> = {
            let interner = self.inference.env.interner();
            imports
                .iter()
                .map(|import| {
                    // Convert portable Types to TypeIds
                    let params: Vec<ori_ir::TypeId> = import
                        .params
                        .iter()
                        .map(|t| t.to_type_id(interner))
                        .collect();
                    let return_type = import.return_type.to_type_id(interner);

                    // Create Type::Function for environment binding
                    let fn_type = Type::Function {
                        params: import.params.clone(),
                        ret: Box::new(import.return_type.clone()),
                    };

                    PreparedImport {
                        name: import.name,
                        params,
                        return_type,
                        fn_type,
                        capabilities: import.capabilities.clone(),
                    }
                })
                .collect()
        };

        // Second pass: create fresh type vars and do mutations
        for (import, prep) in imports.iter().zip(prepared.into_iter()) {
            // Create generics with fresh type variables for this context
            let mut generics = Vec::new();
            for g in &import.generics {
                let type_var_id = self.inference.ctx.fresh_var_id();
                generics.push(GenericBound {
                    param: g.param,
                    bounds: g.bounds.clone(),
                    type_var: type_var_id,
                });
            }

            // Create FunctionType for scope context
            let func_type = FunctionType {
                name: prep.name,
                generics: generics.clone(),
                where_constraints: Vec::new(), // TODO: Support where clauses on imports
                params: prep.params,
                return_type: prep.return_type,
                capabilities: prep.capabilities,
            };

            // Store in function_sigs for constraint checking during calls
            self.scope.function_sigs.insert(prep.name, func_type);

            // Bind to environment
            // For generic functions, create a polymorphic type scheme
            let interner = self.inference.env.interner();
            let type_vars: Vec<_> = generics
                .iter()
                .filter_map(|g| {
                    if let ori_types::TypeData::Var(tv) = interner.lookup(g.type_var) {
                        Some(tv)
                    } else {
                        None
                    }
                })
                .collect();

            if type_vars.is_empty() {
                self.inference.env.bind(prep.name, prep.fn_type);
            } else {
                let scheme = ori_types::TypeScheme::poly(type_vars, prep.fn_type);
                self.inference.env.bind_scheme(prep.name, scheme);
            }
        }
    }
}
