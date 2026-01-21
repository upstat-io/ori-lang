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
