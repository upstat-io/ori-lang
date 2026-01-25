//! Trait Bound Checking
//!
//! Verifies that types satisfy trait bounds at call sites.

use crate::ir::{Name, Span};
use crate::types::Type;
use crate::diagnostic::ErrorCode;
use super::{TypeChecker, TypeCheckError};

/// Check if a primitive type has a built-in trait implementation.
fn primitive_implements_trait(ty: &Type, trait_name: &str) -> bool {
    match ty {
        Type::Int => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Default" | "Printable"),
        Type::Float => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Default" | "Printable"),
        Type::Bool => matches!(trait_name, "Eq" | "Clone" | "Hashable" | "Default" | "Printable"),
        Type::Str => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Default" | "Printable"),
        Type::Char => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Hashable" | "Printable"),
        Type::Byte => matches!(trait_name, "Eq" | "Clone" | "Hashable" | "Printable"),
        Type::Unit => matches!(trait_name, "Eq" | "Clone" | "Default"),
        Type::Duration => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Printable"),
        Type::Size => matches!(trait_name, "Eq" | "Comparable" | "Clone" | "Printable"),

        // Option<T> is Eq if T is Eq, Clone if T is Clone, etc.
        Type::Option(inner) => {
            if matches!(trait_name, "Clone" | "Eq" | "Default") {
                // For now, assume inner type satisfies the bound
                // Full checking would recursively verify inner type
                let _ = inner;
                true
            } else {
                false
            }
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

        // Lists, Maps, Tuples implement Clone if their elements do
        Type::List(_) | Type::Map { .. } | Type::Tuple(_) | Type::Set(_) => {
            matches!(trait_name, "Clone" | "Eq")
        }

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
        if self.trait_registry.implements(ty, trait_name) {
            return true;
        }

        // Then check built-in trait implementations for primitive types
        let trait_str = self.interner.lookup(trait_name);
        primitive_implements_trait(ty, trait_str)
    }

    /// Check trait bounds for a function call.
    ///
    /// Given a function name and the call site span, verifies that the resolved
    /// types for each generic parameter satisfy all required trait bounds.
    pub(crate) fn check_function_bounds(&mut self, func_name: Name, span: Span) {
        // Look up the function's signature to get its generic bounds
        let func_sig = match self.function_sigs.get(&func_name) {
            Some(sig) => sig.clone(),
            None => return, // Not a known function (might be a closure or imported)
        };

        // Check each generic parameter's bounds
        for generic in &func_sig.generics {
            if generic.bounds.is_empty() {
                continue; // No bounds to check
            }

            // Resolve the type variable to get the actual type
            let resolved_type = self.ctx.resolve(&generic.type_var);

            // Skip unresolved type variables - bounds can't be checked yet
            if matches!(resolved_type, Type::Var(_)) {
                continue;
            }

            // Check each bound for this generic parameter
            for bound_path in &generic.bounds {
                if !self.type_satisfies_bound(&resolved_type, bound_path) {
                    let bound_name = bound_path.iter()
                        .map(|n| self.interner.lookup(*n).to_string())
                        .collect::<Vec<_>>()
                        .join(".");

                    let type_name = resolved_type.display(self.interner);
                    let generic_name = self.interner.lookup(generic.param);

                    self.errors.push(TypeCheckError {
                        message: format!(
                            "type `{type_name}` does not satisfy trait bound `{bound_name}` required by generic parameter `{generic_name}`"
                        ),
                        span,
                        code: ErrorCode::E2009,
                    });
                }
            }
        }
    }
}
