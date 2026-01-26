//! Trait Bound Checking
//!
//! Verifies that types satisfy trait bounds at call sites.

use std::collections::HashMap;
use crate::ir::{Name, Span};
use crate::types::Type;
use crate::diagnostic::ErrorCode;
use super::{TypeChecker, TypeCheckError, FunctionType};

/// Check if a primitive type has a built-in trait implementation.
///
/// This is used to check both generic trait bounds and capability trait implementations.
pub fn primitive_implements_trait(ty: &Type, trait_name: &str) -> bool {
    match ty {
        Type::Int => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Default" | "Printable"),
        Type::Float => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Default" | "Printable"),
        Type::Bool => matches!(trait_name, "Eq" | "Clone" | "Hashable" | "Default" | "Printable"),
        Type::Str => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Default" | "Printable" | "Len" | "IsEmpty"),
        Type::Char => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Printable"),
        Type::Byte => matches!(trait_name, "Eq" | "Clone" | "Hashable" | "Printable"),
        Type::Unit => matches!(trait_name, "Eq" | "Clone" | "Default"),
        Type::Duration => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Printable"),
        Type::Size => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Printable"),

        // Option<T> is Eq if T is Eq, Clone if T is Clone, etc.
        Type::Option(inner) => {
            // Option<T> satisfies Clone/Eq/Default if T does.
            // Recursive verification deferred to trait solving.
            let _ = inner;
            matches!(trait_name, "Clone" | "Eq" | "Default")
        }

        // Result<T, E> is Eq if both T and E are Eq, etc.
        Type::Result { ok, err } => {
            if matches!(trait_name, "Clone" | "Eq") {
                let _ = (ok, err);
                true
            } else {
                false
            }
        }

        // Lists implement Len, IsEmpty, Clone, Eq
        Type::List(_) => matches!(trait_name, "Clone" | "Eq" | "Len" | "IsEmpty"),

        // Maps implement Len, IsEmpty, Clone, Eq
        Type::Map { .. } => matches!(trait_name, "Clone" | "Eq" | "Len" | "IsEmpty"),

        // Sets implement Len, IsEmpty, Clone, Eq
        Type::Set(_) => matches!(trait_name, "Clone" | "Eq" | "Len" | "IsEmpty"),

        // Tuples implement Clone, Eq
        Type::Tuple(_) => matches!(trait_name, "Clone" | "Eq"),

        // Ranges implement Len
        Type::Range(_) => matches!(trait_name, "Len"),

        _ => false,
    }
}

impl TypeChecker<'_> {
    /// Check if a type satisfies a trait bound.
    ///
    /// Returns true if the type implements the trait, false otherwise.
    /// This uses the trait registry to check for implementations.
    pub(crate) fn type_satisfies_bound(&self, ty: &Type, trait_path: &[Name]) -> bool {
        // Get the trait name (last segment of path)
        let trait_name = match trait_path.last() {
            Some(name) => *name,
            None => return false,
        };

        // First check the trait registry for registered implementations
        if self.registries.traits.implements(ty, trait_name) {
            return true;
        }

        // Then check built-in trait implementations for primitive types
        let trait_str = self.context.interner.lookup(trait_name);
        primitive_implements_trait(ty, trait_str)
    }

    /// Check trait bounds for a function call.
    ///
    /// Given a function name and the call site span, verifies that the resolved
    /// types for each generic parameter satisfy all required trait bounds.
    ///
    /// `resolved_params` contains the resolved parameter types from the call site,
    /// used to determine the concrete types for generic parameters.
    pub(crate) fn check_function_bounds(
        &mut self,
        func_name: Name,
        resolved_params: Option<&[Type]>,
        span: Span,
    ) {
        // Look up the function's signature to get its generic bounds
        let func_sig = match self.scope.function_sigs.get(&func_name) {
            Some(sig) => sig.clone(),
            None => return, // Not a known function (might be a closure or imported)
        };

        let resolved_params = match resolved_params {
            Some(params) => params,
            None => return, // No params to check
        };

        // Build a mapping from generic param name to its resolved type.
        // This is done by finding which params in the signature use type vars
        // that correspond to generic parameters.
        let generic_types = self.build_generic_type_map(&func_sig, resolved_params);

        // Check each generic parameter's bounds
        for generic in &func_sig.generics {
            if generic.bounds.is_empty() {
                continue; // No bounds to check
            }

            // Look up the resolved type for this generic parameter
            let resolved_type = match generic_types.get(&generic.param) {
                Some(ty) => ty.clone(),
                None => continue, // Generic param not used in params
            };

            // Skip unresolved type variables - bounds can't be checked yet
            if matches!(resolved_type, Type::Var(_)) {
                continue;
            }

            // Check each bound for this generic parameter
            for bound_path in &generic.bounds {
                if !self.type_satisfies_bound(&resolved_type, bound_path) {
                    let bound_name = bound_path.iter()
                        .map(|n| self.context.interner.lookup(*n).to_string())
                        .collect::<Vec<_>>()
                        .join(".");

                    let type_name = resolved_type.display(self.context.interner);
                    let generic_name = self.context.interner.lookup(generic.param);

                    self.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "type `{type_name}` does not satisfy trait bound `{bound_name}` required by generic parameter `{generic_name}`"
                        ),
                        span,
                        code: ErrorCode::E2009,
                    });
                }
            }
        }

        // Check where clause constraints with projections (e.g., where C.Item: Eq)
        for constraint in &func_sig.where_constraints {
            // Look up the resolved type for the base parameter
            let resolved_base = match generic_types.get(&constraint.param) {
                Some(ty) => ty.clone(),
                None => continue, // Base param not found
            };

            // Skip unresolved type variables
            if matches!(resolved_base, Type::Var(_)) {
                continue;
            }

            // Get the type to check: either the base type or its associated type projection
            let type_to_check = if let Some(assoc_name) = constraint.projection {
                // Look up the associated type for the resolved base type
                match self.resolve_associated_type(&resolved_base, assoc_name) {
                    Some(assoc_ty) => assoc_ty,
                    None => {
                        // Associated type not found - error already reported elsewhere
                        continue;
                    }
                }
            } else {
                resolved_base.clone()
            };

            // Check each bound
            for bound_path in &constraint.bounds {
                if !self.type_satisfies_bound(&type_to_check, bound_path) {
                    let bound_name = bound_path.iter()
                        .map(|n| self.context.interner.lookup(*n).to_string())
                        .collect::<Vec<_>>()
                        .join(".");

                    let type_name = type_to_check.display(self.context.interner);
                    let constraint_desc = if let Some(proj) = constraint.projection {
                        let param_name = self.context.interner.lookup(constraint.param);
                        let proj_name = self.context.interner.lookup(proj);
                        format!("{param_name}.{proj_name}")
                    } else {
                        self.context.interner.lookup(constraint.param).to_string()
                    };

                    self.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "type `{type_name}` (from `{constraint_desc}`) does not satisfy trait bound `{bound_name}`"
                        ),
                        span,
                        code: ErrorCode::E2009,
                    });
                }
            }
        }
    }

    /// Build a mapping from generic parameter names to their resolved types.
    ///
    /// This maps each generic parameter to the concrete type that was passed
    /// at the call site, by matching the signature's parameter types (which
    /// reference the generic type vars) against the resolved parameter types.
    fn build_generic_type_map(
        &self,
        func_sig: &FunctionType,
        resolved_params: &[Type],
    ) -> HashMap<Name, Type> {
        let mut map = HashMap::new();

        // Build a map from type var ID to generic param name
        let mut type_var_to_generic: HashMap<u32, Name> = HashMap::new();
        for generic in &func_sig.generics {
            if let Type::Var(tv) = &generic.type_var {
                type_var_to_generic.insert(tv.0, generic.param);
            }
        }
        // Also add type vars from where constraints
        for constraint in &func_sig.where_constraints {
            if let Type::Var(tv) = &constraint.type_var {
                type_var_to_generic.insert(tv.0, constraint.param);
            }
        }

        // Match signature params to resolved params
        for (sig_param, resolved_param) in func_sig.params.iter().zip(resolved_params.iter()) {
            // If the sig param is a type var, look up which generic it corresponds to
            if let Type::Var(tv) = sig_param {
                if let Some(generic_name) = type_var_to_generic.get(&tv.0) {
                    map.insert(*generic_name, resolved_param.clone());
                }
            }
        }

        map
    }

    /// Resolve an associated type for a given base type.
    ///
    /// Given `IntBox` and `Item`, looks up what `IntBox.Item` resolves to
    /// by checking the trait registry for impl blocks that define `type Item = ...`.
    fn resolve_associated_type(&self, base_ty: &Type, assoc_name: Name) -> Option<Type> {
        // Get the type name from the base type
        let type_name = match base_ty {
            Type::Named(name) => *name,
            Type::Applied { name, .. } => *name,
            _ => return None,
        };

        // Look up the associated type in the trait registry
        self.registries.traits.lookup_assoc_type_by_name(type_name, assoc_name)
    }
}
