// Trait method resolution for Sigil
//
// Provides method lookup and resolution for trait methods.
// This handles:
// - Finding which trait a method belongs to
// - Resolving method calls on trait objects
// - Checking method compatibility

use crate::ast::TypeExpr;
use crate::types::traits::{ImplDef, TraitDef, TraitMethod, TraitRegistry};
use crate::types::FunctionSig;

/// Result of method resolution.
#[derive(Debug, Clone)]
pub struct ResolvedMethod {
    /// The trait that defines this method
    pub trait_name: String,
    /// The method signature
    pub method: TraitMethod,
    /// The implementation being used (if concrete type)
    pub impl_def: Option<ImplDef>,
}

/// Method resolver for trait-based dispatch.
pub struct MethodResolver<'a> {
    registry: &'a TraitRegistry,
}

impl<'a> MethodResolver<'a> {
    /// Create a new method resolver.
    pub fn new(registry: &'a TraitRegistry) -> Self {
        MethodResolver { registry }
    }

    /// Resolve a method call on a type.
    ///
    /// Returns the resolved method if found, or None if the type doesn't
    /// implement any trait with that method.
    pub fn resolve_method(
        &self,
        type_expr: &TypeExpr,
        method_name: &str,
    ) -> Option<ResolvedMethod> {
        // Get all implementations for this type
        let impls = self.registry.get_impls_for_type(type_expr);

        for impl_def in impls {
            // Look up the trait
            if let Some(trait_def) = self.registry.lookup_trait(&impl_def.trait_name) {
                // Check if the trait has this method
                if let Some(method) = trait_def.get_method(method_name) {
                    return Some(ResolvedMethod {
                        trait_name: impl_def.trait_name.clone(),
                        method: method.clone(),
                        impl_def: Some(impl_def.clone()),
                    });
                }
            }
        }

        None
    }

    /// Find all traits that have a method with the given name.
    pub fn find_traits_with_method(&self, method_name: &str) -> Vec<&TraitDef> {
        // This would iterate over all traits - for now, we don't have that API
        // in TraitRegistry, so this is a placeholder
        Vec::new()
    }

    /// Check if a method implementation is compatible with the trait signature.
    pub fn check_method_compatibility(
        &self,
        trait_sig: &FunctionSig,
        impl_sig: &FunctionSig,
        type_args: &std::collections::HashMap<String, TypeExpr>,
    ) -> Result<(), String> {
        // Check parameter count (allowing for Self parameter)
        if trait_sig.params.len() != impl_sig.params.len() {
            return Err(format!(
                "Method has {} parameters, but trait expects {}",
                impl_sig.params.len(),
                trait_sig.params.len()
            ));
        }

        // Check parameter types
        for (i, ((trait_name, trait_ty), (impl_name, impl_ty))) in
            trait_sig.params.iter().zip(&impl_sig.params).enumerate()
        {
            if !self.types_compatible(trait_ty, impl_ty, type_args) {
                return Err(format!(
                    "Parameter {} type mismatch: trait expects {:?}, impl provides {:?}",
                    i, trait_ty, impl_ty
                ));
            }
        }

        // Check return type
        if !self.types_compatible(&trait_sig.return_type, &impl_sig.return_type, type_args) {
            return Err(format!(
                "Return type mismatch: trait expects {:?}, impl provides {:?}",
                trait_sig.return_type, impl_sig.return_type
            ));
        }

        Ok(())
    }

    /// Check if two types are compatible, considering Self and type parameters.
    fn types_compatible(
        &self,
        trait_ty: &TypeExpr,
        impl_ty: &TypeExpr,
        type_args: &std::collections::HashMap<String, TypeExpr>,
    ) -> bool {
        match (trait_ty, impl_ty) {
            // Self in trait matches the implementing type
            (TypeExpr::Named(n), _) if n == "Self" => true,

            // Type parameters are substituted
            (TypeExpr::Named(n), actual) => {
                if let Some(substituted) = type_args.get(n) {
                    substituted == actual
                } else {
                    trait_ty == impl_ty
                }
            }

            // Exact match
            _ => trait_ty == impl_ty,
        }
    }
}

/// Information about a trait object (dyn Trait).
#[derive(Debug, Clone)]
pub struct TraitObject {
    /// The trait being used as an object
    pub trait_name: String,
    /// Methods available through the vtable
    pub methods: Vec<TraitMethod>,
}

impl TraitObject {
    /// Create a trait object from a trait definition.
    pub fn from_trait(trait_def: &TraitDef) -> Result<Self, String> {
        // Check object safety - all methods must be object-safe
        for method in &trait_def.methods {
            // Simple check: no generic methods for now
            if !method.sig.type_params.is_empty() {
                return Err(format!(
                    "Trait '{}' is not object-safe: method '{}' has generic parameters",
                    trait_def.name, method.name
                ));
            }
        }

        Ok(TraitObject {
            trait_name: trait_def.name.clone(),
            methods: trait_def.methods.clone(),
        })
    }

    /// Get a method by name.
    pub fn get_method(&self, name: &str) -> Option<&TraitMethod> {
        self.methods.iter().find(|m| m.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registry() -> TraitRegistry {
        let mut registry = TraitRegistry::new();

        // Define a simple trait
        let mut comparable = TraitDef::new("Comparable".to_string());
        comparable.add_method(TraitMethod {
            name: "compare".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![
                    ("self".to_string(), TypeExpr::Named("Self".to_string())),
                    ("other".to_string(), TypeExpr::Named("Self".to_string())),
                ],
                return_type: TypeExpr::Named("int".to_string()),
                capabilities: vec![],
            },
            has_default: false,
        });
        registry.define_trait(comparable);

        // Add implementation for int
        let impl_def = ImplDef::new(
            "Comparable".to_string(),
            TypeExpr::Named("int".to_string()),
        );
        registry.add_impl(impl_def);

        registry
    }

    #[test]
    fn test_method_resolution() {
        let registry = create_test_registry();
        let resolver = MethodResolver::new(&registry);

        let result = resolver.resolve_method(
            &TypeExpr::Named("int".to_string()),
            "compare",
        );

        assert!(result.is_some());
        let resolved = result.unwrap();
        assert_eq!(resolved.trait_name, "Comparable");
        assert_eq!(resolved.method.name, "compare");
    }

    #[test]
    fn test_method_not_found() {
        let registry = create_test_registry();
        let resolver = MethodResolver::new(&registry);

        let result = resolver.resolve_method(
            &TypeExpr::Named("int".to_string()),
            "nonexistent",
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_trait_object_creation() {
        let trait_def = TraitDef::new("Display".to_string());
        let obj = TraitObject::from_trait(&trait_def);

        assert!(obj.is_ok());
        let obj = obj.unwrap();
        assert_eq!(obj.trait_name, "Display");
    }

    #[test]
    fn test_trait_object_not_object_safe() {
        let mut trait_def = TraitDef::new("Generic".to_string());
        trait_def.add_method(TraitMethod {
            name: "generic_method".to_string(),
            sig: FunctionSig {
                type_params: vec!["T".to_string()], // Generic!
                type_param_bounds: vec![],
                params: vec![],
                return_type: TypeExpr::Named("void".to_string()),
                capabilities: vec![],
            },
            has_default: false,
        });

        let obj = TraitObject::from_trait(&trait_def);
        assert!(obj.is_err());
    }
}
