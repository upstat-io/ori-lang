// Trait system for Sigil
//
// This module provides the infrastructure for Sigil's trait system:
// - Trait definitions and bounds
// - Trait implementations (impl blocks)
// - Method resolution
// - Object safety checking
//
// The core types (TraitDef, ImplDef, TraitRegistry) are defined in
// types/traits.rs. This module provides higher-level functionality.

mod resolution;

// Re-export core types from types/traits
pub use crate::types::traits::{
    ImplDef, ImplMethod, TraitBound, TraitDef, TraitMethod, TraitRegistry,
};

// Export resolution types
pub use resolution::{MethodResolver, ResolvedMethod, TraitObject};

/// Built-in traits that are always available.
pub mod builtins {
    use super::*;
    use crate::types::FunctionSig;
    use crate::ast::TypeExpr;

    /// Create the Copy trait definition.
    pub fn copy_trait() -> TraitDef {
        TraitDef::new("Copy".to_string())
    }

    /// Create the Clone trait definition.
    pub fn clone_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Clone".to_string());
        trait_def.add_method(TraitMethod {
            name: "clone".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![("self".to_string(), TypeExpr::Named("Self".to_string()))],
                return_type: TypeExpr::Named("Self".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Create the Eq trait definition.
    pub fn eq_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Eq".to_string());
        trait_def.add_method(TraitMethod {
            name: "eq".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![
                    ("self".to_string(), TypeExpr::Named("Self".to_string())),
                    ("other".to_string(), TypeExpr::Named("Self".to_string())),
                ],
                return_type: TypeExpr::Named("bool".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Create the Ord trait definition.
    pub fn ord_trait() -> TraitDef {
        let trait_def = TraitDef::new("Ord".to_string())
            .with_supertraits(vec!["Eq".to_string()]);
        // Note: methods would include compare, lt, gt, etc.
        trait_def
    }

    /// Create the Hash trait definition.
    pub fn hash_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Hash".to_string());
        trait_def.add_method(TraitMethod {
            name: "hash".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![("self".to_string(), TypeExpr::Named("Self".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Create the Display trait definition.
    pub fn display_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Display".to_string());
        trait_def.add_method(TraitMethod {
            name: "to_string".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![("self".to_string(), TypeExpr::Named("Self".to_string()))],
                return_type: TypeExpr::Named("str".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Create the Debug trait definition.
    pub fn debug_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Debug".to_string());
        trait_def.add_method(TraitMethod {
            name: "debug".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![("self".to_string(), TypeExpr::Named("Self".to_string()))],
                return_type: TypeExpr::Named("str".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Create the Default trait definition.
    pub fn default_trait() -> TraitDef {
        let mut trait_def = TraitDef::new("Default".to_string());
        trait_def.add_method(TraitMethod {
            name: "default".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![], // No self parameter - static method
                return_type: TypeExpr::Named("Self".to_string()),
            },
            has_default: false,
        });
        trait_def
    }

    /// Register all built-in traits in a registry.
    pub fn register_builtins(registry: &mut TraitRegistry) {
        registry.define_trait(copy_trait());
        registry.define_trait(clone_trait());
        registry.define_trait(eq_trait());
        registry.define_trait(ord_trait());
        registry.define_trait(hash_trait());
        registry.define_trait(display_trait());
        registry.define_trait(debug_trait());
        registry.define_trait(default_trait());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_copy_trait() {
        let copy = builtins::copy_trait();
        assert_eq!(copy.name, "Copy");
        assert!(copy.is_marker_trait()); // No methods
    }

    #[test]
    fn test_builtin_clone_trait() {
        let clone = builtins::clone_trait();
        assert_eq!(clone.name, "Clone");
        assert!(!clone.is_marker_trait()); // Has clone method
        assert!(clone.get_method("clone").is_some());
    }

    #[test]
    fn test_builtin_eq_trait() {
        let eq = builtins::eq_trait();
        assert_eq!(eq.name, "Eq");
        assert!(eq.get_method("eq").is_some());
    }

    #[test]
    fn test_builtin_ord_has_supertraits() {
        let ord = builtins::ord_trait();
        assert!(ord.supertraits.contains(&"Eq".to_string()));
    }

    #[test]
    fn test_register_builtins() {
        let mut registry = TraitRegistry::new();
        builtins::register_builtins(&mut registry);

        assert!(registry.contains_trait("Copy"));
        assert!(registry.contains_trait("Clone"));
        assert!(registry.contains_trait("Eq"));
        assert!(registry.contains_trait("Ord"));
        assert!(registry.contains_trait("Hash"));
        assert!(registry.contains_trait("Display"));
        assert!(registry.contains_trait("Debug"));
        assert!(registry.contains_trait("Default"));
    }
}
