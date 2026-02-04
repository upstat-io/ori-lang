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
use ori_types::{Type, TypeFolder, TypeVar};
use rustc_hash::FxHashMap;

/// A generic parameter with its trait bounds for imported functions.
///
/// This is a portable representation that doesn't depend on `TypeIds`.
#[derive(Clone, Debug)]
pub struct ImportedGeneric {
    /// The generic parameter name (e.g., `T` in `<T: Eq>`)
    pub param: Name,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`)
    pub bounds: Vec<Vec<Name>>,
    /// The original type variable used in the function's type signature.
    /// Used to substitute with fresh type variables when importing.
    pub type_var: TypeVar,
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

        // Convert generics (bounds are already Names, extract TypeVar for substitution)
        let generics: Vec<ImportedGeneric> = func_type
            .generics
            .iter()
            .filter_map(|g| {
                // Extract the TypeVar from the TypeId
                if let ori_types::TypeData::Var(tv) = interner.lookup(g.type_var) {
                    Some(ImportedGeneric {
                        param: g.param,
                        bounds: g.bounds.clone(),
                        type_var: tv,
                    })
                } else {
                    // Should not happen: generic bounds should always have TypeVar
                    None
                }
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
        for import in imports {
            // Create fresh type variables for each generic parameter and build
            // a substitution map from original TypeVars to fresh ones.
            let mut substitution: FxHashMap<TypeVar, TypeVar> = FxHashMap::default();
            let mut generics = Vec::new();

            for g in &import.generics {
                let fresh_type_id = self.inference.ctx.fresh_var_id();
                // Use ctx's interner since fresh_var_id() creates TypeId there
                let interner = self.inference.ctx.interner();
                if let ori_types::TypeData::Var(fresh_tv) = interner.lookup(fresh_type_id) {
                    // Map original TypeVar to fresh TypeVar
                    substitution.insert(g.type_var, fresh_tv);
                    generics.push(GenericBound {
                        param: g.param,
                        bounds: g.bounds.clone(),
                        type_var: fresh_type_id,
                    });
                }
            }

            // Substitute original type vars with fresh ones in the function type.
            // This ensures the TypeScheme's quantified vars match the vars in the type.
            let fn_type = Type::Function {
                params: import.params.clone(),
                ret: Box::new(import.return_type.clone()),
            };

            let substituted_fn_type = if substitution.is_empty() {
                fn_type
            } else {
                struct TypeVarSubstituter<'a> {
                    substitution: &'a FxHashMap<TypeVar, TypeVar>,
                }
                impl TypeFolder for TypeVarSubstituter<'_> {
                    fn fold_var(&mut self, var: TypeVar) -> Type {
                        if let Some(&fresh) = self.substitution.get(&var) {
                            Type::Var(fresh)
                        } else {
                            Type::Var(var)
                        }
                    }
                }
                let mut folder = TypeVarSubstituter {
                    substitution: &substitution,
                };
                folder.fold(&fn_type)
            };

            // Convert to TypeIds for FunctionType (use ctx's interner for consistency)
            let interner = self.inference.ctx.interner();
            let params: Vec<ori_ir::TypeId> = match &substituted_fn_type {
                Type::Function { params, .. } => {
                    params.iter().map(|t| t.to_type_id(interner)).collect()
                }
                _ => unreachable!(),
            };
            let return_type = match &substituted_fn_type {
                Type::Function { ret, .. } => ret.to_type_id(interner),
                _ => unreachable!(),
            };

            // Create FunctionType for scope context (for constraint checking)
            let func_type = FunctionType {
                name: import.name,
                generics: generics.clone(),
                where_constraints: Vec::new(), // TODO: Support where clauses on imports
                params,
                return_type,
                capabilities: import.capabilities.clone(),
            };

            // Store in function_sigs for constraint checking during calls
            self.scope.function_sigs.insert(import.name, func_type);

            // Bind to environment as a type scheme
            let interner = self.inference.ctx.interner();
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
                self.inference.env.bind(import.name, substituted_fn_type);
            } else {
                let scheme = ori_types::TypeScheme::poly(type_vars, substituted_fn_type);
                self.inference.env.bind_scheme(import.name, scheme);
            }
        }
    }
}
