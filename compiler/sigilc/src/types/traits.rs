// Trait system infrastructure for Sigil
// Placeholder for future trait implementation
//
// This module provides the foundation for:
// - Trait definitions
// - Trait implementations (impl blocks)
// - Trait bounds on generics
// - Method resolution

use crate::ast::TypeExpr;
use super::registries::FunctionSig;
use std::collections::HashMap;

/// A trait method signature
#[derive(Clone, Debug)]
pub struct TraitMethod {
    /// Method name
    pub name: String,
    /// Method signature
    pub sig: FunctionSig,
    /// Whether this method has a default implementation
    pub has_default: bool,
}

/// A trait definition
#[derive(Clone, Debug)]
pub struct TraitDef {
    /// Trait name
    pub name: String,
    /// Type parameters (e.g., T in `trait Comparable<T>`)
    pub type_params: Vec<String>,
    /// Supertraits (e.g., `trait Ord: Eq + PartialOrd`)
    pub supertraits: Vec<String>,
    /// Methods defined by this trait
    pub methods: Vec<TraitMethod>,
}

impl TraitDef {
    pub fn new(name: String) -> Self {
        TraitDef {
            name,
            type_params: vec![],
            supertraits: vec![],
            methods: vec![],
        }
    }

    pub fn with_type_params(mut self, params: Vec<String>) -> Self {
        self.type_params = params;
        self
    }

    pub fn with_supertraits(mut self, supertraits: Vec<String>) -> Self {
        self.supertraits = supertraits;
        self
    }

    pub fn add_method(&mut self, method: TraitMethod) {
        self.methods.push(method);
    }

    /// Get a method by name
    pub fn get_method(&self, name: &str) -> Option<&TraitMethod> {
        self.methods.iter().find(|m| m.name == name)
    }

    /// Check if all methods have default implementations
    pub fn is_marker_trait(&self) -> bool {
        self.methods.is_empty()
    }
}

/// A trait bound (e.g., `T: Comparable + Hashable`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitBound {
    /// The type parameter being bounded
    pub type_param: String,
    /// The traits that must be implemented
    pub traits: Vec<String>,
}

impl TraitBound {
    pub fn new(type_param: String) -> Self {
        TraitBound {
            type_param,
            traits: vec![],
        }
    }

    pub fn with_traits(mut self, traits: Vec<String>) -> Self {
        self.traits = traits;
        self
    }

    pub fn add_trait(&mut self, trait_name: String) {
        self.traits.push(trait_name);
    }
}

/// An implementation block (impl Trait for Type)
#[derive(Clone, Debug)]
pub struct ImplDef {
    /// Type parameters for the impl (e.g., `impl<T> Trait for Vec<T>`)
    pub type_params: Vec<String>,
    /// The trait being implemented
    pub trait_name: String,
    /// The type implementing the trait
    pub for_type: TypeExpr,
    /// Where clause bounds
    pub where_clause: Vec<TraitBound>,
    /// Method implementations
    pub methods: Vec<ImplMethod>,
}

impl ImplDef {
    pub fn new(trait_name: String, for_type: TypeExpr) -> Self {
        ImplDef {
            type_params: vec![],
            trait_name,
            for_type,
            where_clause: vec![],
            methods: vec![],
        }
    }

    pub fn with_type_params(mut self, params: Vec<String>) -> Self {
        self.type_params = params;
        self
    }

    pub fn with_where_clause(mut self, bounds: Vec<TraitBound>) -> Self {
        self.where_clause = bounds;
        self
    }

    pub fn add_method(&mut self, method: ImplMethod) {
        self.methods.push(method);
    }
}

/// A method implementation within an impl block
#[derive(Clone, Debug)]
pub struct ImplMethod {
    /// Method name (must match trait method name)
    pub name: String,
    /// Method signature (must be compatible with trait signature)
    pub sig: FunctionSig,
    // Note: Body would be added later when we have AST for function bodies
}

/// Registry for traits
#[derive(Clone, Default)]
pub struct TraitRegistry {
    /// Trait definitions indexed by name
    traits: HashMap<String, TraitDef>,
    /// Implementation blocks
    impls: Vec<ImplDef>,
}

impl TraitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a new trait
    pub fn define_trait(&mut self, def: TraitDef) {
        self.traits.insert(def.name.clone(), def);
    }

    /// Add an implementation
    pub fn add_impl(&mut self, impl_def: ImplDef) {
        self.impls.push(impl_def);
    }

    /// Lookup a trait by name
    pub fn lookup_trait(&self, name: &str) -> Option<&TraitDef> {
        self.traits.get(name)
    }

    /// Check if a trait exists
    pub fn contains_trait(&self, name: &str) -> bool {
        self.traits.contains_key(name)
    }

    /// Get all implementations for a trait
    pub fn get_impls_for_trait(&self, trait_name: &str) -> Vec<&ImplDef> {
        self.impls
            .iter()
            .filter(|i| i.trait_name == trait_name)
            .collect()
    }

    /// Get all implementations for a type
    pub fn get_impls_for_type(&self, type_expr: &TypeExpr) -> Vec<&ImplDef> {
        self.impls
            .iter()
            .filter(|i| &i.for_type == type_expr)
            .collect()
    }

    /// Check if a type implements a trait
    /// Note: This is a simplified check that doesn't handle generics properly yet
    pub fn type_implements_trait(&self, type_expr: &TypeExpr, trait_name: &str) -> bool {
        self.impls
            .iter()
            .any(|i| i.trait_name == trait_name && &i.for_type == type_expr)
    }

    /// Check if all bounds in a where clause are satisfied
    pub fn check_bounds(&self, bounds: &[TraitBound], type_args: &HashMap<String, TypeExpr>) -> bool {
        bounds.iter().all(|bound| {
            if let Some(type_expr) = type_args.get(&bound.type_param) {
                bound.traits.iter().all(|trait_name| {
                    self.type_implements_trait(type_expr, trait_name)
                })
            } else {
                // Type parameter not found in type arguments
                false
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_def_creation() {
        let trait_def = TraitDef::new("Comparable".to_string())
            .with_type_params(vec!["T".to_string()]);

        assert_eq!(trait_def.name, "Comparable");
        assert_eq!(trait_def.type_params, vec!["T".to_string()]);
        assert!(trait_def.methods.is_empty());
    }

    #[test]
    fn test_trait_method() {
        let mut trait_def = TraitDef::new("Hashable".to_string());
        trait_def.add_method(TraitMethod {
            name: "hash".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![("self".to_string(), TypeExpr::Named("Self".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
                capabilities: vec![],
            },
            has_default: false,
        });

        assert_eq!(trait_def.methods.len(), 1);
        assert!(trait_def.get_method("hash").is_some());
        assert!(trait_def.get_method("other").is_none());
    }

    #[test]
    fn test_trait_bound() {
        let bound = TraitBound::new("T".to_string())
            .with_traits(vec!["Comparable".to_string(), "Hashable".to_string()]);

        assert_eq!(bound.type_param, "T");
        assert_eq!(bound.traits.len(), 2);
    }

    #[test]
    fn test_impl_def() {
        let impl_def = ImplDef::new(
            "Comparable".to_string(),
            TypeExpr::Named("int".to_string()),
        );

        assert_eq!(impl_def.trait_name, "Comparable");
        assert_eq!(impl_def.for_type, TypeExpr::Named("int".to_string()));
    }

    #[test]
    fn test_trait_registry() {
        let mut registry = TraitRegistry::new();

        // Define a trait
        let trait_def = TraitDef::new("Hashable".to_string());
        registry.define_trait(trait_def);

        assert!(registry.contains_trait("Hashable"));
        assert!(!registry.contains_trait("Unknown"));

        // Add an implementation
        let impl_def = ImplDef::new(
            "Hashable".to_string(),
            TypeExpr::Named("int".to_string()),
        );
        registry.add_impl(impl_def);

        assert!(registry.type_implements_trait(
            &TypeExpr::Named("int".to_string()),
            "Hashable"
        ));
        assert!(!registry.type_implements_trait(
            &TypeExpr::Named("str".to_string()),
            "Hashable"
        ));
    }

    #[test]
    fn test_marker_trait() {
        let trait_def = TraitDef::new("Send".to_string());
        assert!(trait_def.is_marker_trait());

        let mut trait_with_method = TraitDef::new("Clone".to_string());
        trait_with_method.add_method(TraitMethod {
            name: "clone".to_string(),
            sig: FunctionSig {
                type_params: vec![],
                type_param_bounds: vec![],
                params: vec![],
                return_type: TypeExpr::Named("Self".to_string()),
                capabilities: vec![],
            },
            has_default: false,
        });
        assert!(!trait_with_method.is_marker_trait());
    }

    #[test]
    fn test_supertraits() {
        let trait_def = TraitDef::new("Ord".to_string())
            .with_supertraits(vec!["Eq".to_string(), "PartialOrd".to_string()]);

        assert_eq!(trait_def.supertraits.len(), 2);
        assert!(trait_def.supertraits.contains(&"Eq".to_string()));
    }
}
