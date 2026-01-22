// Focused registries for type checking
// Each registry handles a single responsibility (SRP)

use crate::ast::{TypeDef, TypeExpr, WhereBound};
use std::collections::HashMap;

/// Trait bound for a type parameter
/// Maps a type parameter name to the list of trait bounds it must satisfy
#[derive(Clone, Debug)]
pub struct TypeParamBound {
    pub type_param: String,
    pub bounds: Vec<String>, // trait names
}

impl TypeParamBound {
    pub fn new(type_param: String, bounds: Vec<String>) -> Self {
        Self { type_param, bounds }
    }
}

impl From<WhereBound> for TypeParamBound {
    fn from(wb: WhereBound) -> Self {
        Self {
            type_param: wb.type_param,
            bounds: wb.bounds,
        }
    }
}

/// Function signature for type checking
#[derive(Clone, Debug)]
pub struct FunctionSig {
    pub type_params: Vec<String>,
    /// Trait bounds for type parameters (from inline syntax or where clause)
    pub type_param_bounds: Vec<TypeParamBound>,
    pub params: Vec<(String, TypeExpr)>,
    pub return_type: TypeExpr,
    /// Capability requirements declared with `uses` clause
    pub capabilities: Vec<String>,
}

/// Registry for type definitions
#[derive(Clone, Default)]
pub struct TypeRegistry {
    types: HashMap<String, TypeDef>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: String, def: TypeDef) {
        self.types.insert(name, def);
    }

    pub fn lookup(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TypeDef)> {
        self.types.iter()
    }
}

/// Registry for function signatures
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    functions: HashMap<String, FunctionSig>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: String, sig: FunctionSig) {
        self.functions.insert(name, sig);
    }

    pub fn lookup(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &FunctionSig)> {
        self.functions.iter()
    }
}

/// Registry for config variables
#[derive(Clone, Default)]
pub struct ConfigRegistry {
    configs: HashMap<String, TypeExpr>,
}

/// An extension method definition
#[derive(Clone, Debug)]
pub struct ExtensionMethod {
    /// The function signature
    pub sig: FunctionSig,
    /// Where clause constraints (e.g., Self.Item: Add)
    pub where_clause: Vec<WhereBound>,
}

/// Registry for trait extension methods
/// Extension methods are keyed by (trait_name, method_name)
#[derive(Clone, Default)]
pub struct ExtensionRegistry {
    /// Map from trait name to its extension methods
    extensions: HashMap<String, HashMap<String, ExtensionMethod>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Define an extension method for a trait
    pub fn define(
        &mut self,
        trait_name: String,
        method_name: String,
        sig: FunctionSig,
        where_clause: Vec<WhereBound>,
    ) {
        let trait_extensions = self.extensions.entry(trait_name).or_default();
        trait_extensions.insert(method_name, ExtensionMethod { sig, where_clause });
    }

    /// Lookup an extension method for a trait
    pub fn lookup(&self, trait_name: &str, method_name: &str) -> Option<&ExtensionMethod> {
        self.extensions.get(trait_name)?.get(method_name)
    }

    /// Get all extension methods for a trait
    pub fn get_trait_extensions(
        &self,
        trait_name: &str,
    ) -> Option<&HashMap<String, ExtensionMethod>> {
        self.extensions.get(trait_name)
    }

    /// Check if an extension method exists
    pub fn contains(&self, trait_name: &str, method_name: &str) -> bool {
        self.extensions
            .get(trait_name)
            .map_or(false, |methods| methods.contains_key(method_name))
    }

    /// Iterate over all extensions
    pub fn iter(&self) -> impl Iterator<Item = (&String, &HashMap<String, ExtensionMethod>)> {
        self.extensions.iter()
    }
}

impl ConfigRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: String, ty: TypeExpr) {
        self.configs.insert(name, ty);
    }

    pub fn lookup(&self, name: &str) -> Option<&TypeExpr> {
        self.configs.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.configs.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TypeExpr)> {
        self.configs.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TypeDefKind;

    #[test]
    fn test_type_registry() {
        let mut reg = TypeRegistry::new();
        let def = TypeDef {
            name: "Point".to_string(),
            params: vec![],
            kind: TypeDefKind::Struct(vec![]),
            public: false,
            span: 0..0,
        };
        reg.define("Point".to_string(), def);
        assert!(reg.contains("Point"));
        assert!(reg.lookup("Point").is_some());
        assert!(reg.lookup("Unknown").is_none());
    }

    #[test]
    fn test_function_registry() {
        let mut reg = FunctionRegistry::new();
        let sig = FunctionSig {
            type_params: vec![],
            type_param_bounds: vec![],
            params: vec![("x".to_string(), TypeExpr::Named("int".to_string()))],
            return_type: TypeExpr::Named("int".to_string()),
            capabilities: vec![],
        };
        reg.define("foo".to_string(), sig);
        assert!(reg.contains("foo"));
        assert!(reg.lookup("foo").is_some());
    }

    #[test]
    fn test_config_registry() {
        let mut reg = ConfigRegistry::new();
        reg.define("timeout".to_string(), TypeExpr::Named("int".to_string()));
        assert!(reg.contains("timeout"));
        assert!(reg.lookup("timeout").is_some());
    }
}
