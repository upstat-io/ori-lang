//! Definition registries for functions, types, configs, and traits.
//!
//! These registries store the signatures and definitions that are
//! available for name resolution and type checking.

use crate::intern::{Name, TypeId, TypeRange};
use crate::syntax::Span;
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Function signature.
#[derive(Clone, Debug)]
pub struct FunctionSig {
    /// Function name.
    pub name: Name,
    /// Generic type parameters.
    pub type_params: Vec<TypeParam>,
    /// Parameter types.
    pub params: Vec<ParamSig>,
    /// Return type.
    pub return_type: TypeId,
    /// Required capabilities.
    pub capabilities: Vec<Name>,
    /// Whether this is async.
    pub is_async: bool,
    /// Definition span.
    pub span: Span,
}

/// A type parameter with bounds.
#[derive(Clone, Debug)]
pub struct TypeParam {
    pub name: Name,
    pub bounds: Vec<TypeId>,
}

/// Parameter signature.
#[derive(Clone, Debug)]
pub struct ParamSig {
    pub name: Name,
    pub ty: TypeId,
    pub has_default: bool,
}

/// Type definition (struct, enum, alias).
#[derive(Clone, Debug)]
pub struct TypeDef {
    /// Type name.
    pub name: Name,
    /// Generic type parameters.
    pub type_params: Vec<TypeParam>,
    /// Type kind.
    pub kind: TypeDefKind,
    /// Definition span.
    pub span: Span,
}

/// Kind of type definition.
#[derive(Clone, Debug)]
pub enum TypeDefKind {
    /// Struct with named fields.
    Struct(Vec<FieldDef>),
    /// Enum with variants.
    Enum(Vec<VariantDef>),
    /// Type alias.
    Alias(TypeId),
}

/// Struct field definition.
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: Name,
    pub ty: TypeId,
    pub is_public: bool,
}

/// Enum variant definition.
#[derive(Clone, Debug)]
pub struct VariantDef {
    pub name: Name,
    pub fields: Option<Vec<TypeId>>,
}

/// Config variable definition.
#[derive(Clone, Debug)]
pub struct ConfigDef {
    pub name: Name,
    pub ty: TypeId,
    pub span: Span,
}

/// Trait definition.
#[derive(Clone, Debug)]
pub struct TraitDef {
    pub name: Name,
    pub type_params: Vec<TypeParam>,
    pub super_traits: Vec<TypeId>,
    pub methods: Vec<TraitMethod>,
    pub span: Span,
}

/// Trait method signature.
#[derive(Clone, Debug)]
pub struct TraitMethod {
    pub name: Name,
    pub params: Vec<ParamSig>,
    pub return_type: TypeId,
    pub has_default: bool,
}

/// Implementation block.
#[derive(Clone, Debug)]
pub struct ImplDef {
    /// Type parameters for this impl.
    pub type_params: Vec<TypeParam>,
    /// Trait being implemented (None for inherent impl).
    pub trait_: Option<TypeId>,
    /// Type being implemented for.
    pub target: TypeId,
    /// Methods in this impl.
    pub methods: Vec<ImplMethod>,
    pub span: Span,
}

/// Method in an impl block.
#[derive(Clone, Debug)]
pub struct ImplMethod {
    pub name: Name,
    pub sig: FunctionSig,
}

/// Registry of all definitions in a module.
#[derive(Clone, Debug, Default)]
pub struct DefinitionRegistry {
    /// Functions by name.
    pub functions: FxHashMap<Name, Arc<FunctionSig>>,
    /// Types by name.
    pub types: FxHashMap<Name, Arc<TypeDef>>,
    /// Configs by name.
    pub configs: FxHashMap<Name, Arc<ConfigDef>>,
    /// Traits by name.
    pub traits: FxHashMap<Name, Arc<TraitDef>>,
    /// Impls (keyed by target type).
    pub impls: Vec<Arc<ImplDef>>,
}

impl DefinitionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a function.
    pub fn register_function(&mut self, sig: FunctionSig) {
        self.functions.insert(sig.name, Arc::new(sig));
    }

    /// Register a type definition.
    pub fn register_type(&mut self, def: TypeDef) {
        self.types.insert(def.name, Arc::new(def));
    }

    /// Register a config.
    pub fn register_config(&mut self, def: ConfigDef) {
        self.configs.insert(def.name, Arc::new(def));
    }

    /// Register a trait.
    pub fn register_trait(&mut self, def: TraitDef) {
        self.traits.insert(def.name, Arc::new(def));
    }

    /// Register an impl block.
    pub fn register_impl(&mut self, def: ImplDef) {
        self.impls.push(Arc::new(def));
    }

    /// Look up a function by name.
    pub fn get_function(&self, name: Name) -> Option<&Arc<FunctionSig>> {
        self.functions.get(&name)
    }

    /// Look up a type by name.
    pub fn get_type(&self, name: Name) -> Option<&Arc<TypeDef>> {
        self.types.get(&name)
    }

    /// Look up a config by name.
    pub fn get_config(&self, name: Name) -> Option<&Arc<ConfigDef>> {
        self.configs.get(&name)
    }

    /// Look up a trait by name.
    pub fn get_trait(&self, name: Name) -> Option<&Arc<TraitDef>> {
        self.traits.get(&name)
    }

    /// Find impls for a given type.
    pub fn get_impls_for(&self, target: TypeId) -> Vec<&Arc<ImplDef>> {
        self.impls.iter().filter(|i| i.target == target).collect()
    }

    /// Find trait impl for a type.
    pub fn get_trait_impl(&self, trait_: TypeId, target: TypeId) -> Option<&Arc<ImplDef>> {
        self.impls
            .iter()
            .find(|i| i.trait_ == Some(trait_) && i.target == target)
    }

    /// Merge another registry into this one.
    pub fn merge(&mut self, other: &DefinitionRegistry) {
        for (name, sig) in &other.functions {
            self.functions.insert(*name, Arc::clone(sig));
        }
        for (name, def) in &other.types {
            self.types.insert(*name, Arc::clone(def));
        }
        for (name, def) in &other.configs {
            self.configs.insert(*name, Arc::clone(def));
        }
        for (name, def) in &other.traits {
            self.traits.insert(*name, Arc::clone(def));
        }
        for def in &other.impls {
            self.impls.push(Arc::clone(def));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::StringInterner;

    #[test]
    fn test_registry_functions() {
        let interner = StringInterner::new();
        let mut registry = DefinitionRegistry::new();

        let add = interner.intern("add");
        let sig = FunctionSig {
            name: add,
            type_params: vec![],
            params: vec![
                ParamSig { name: interner.intern("a"), ty: TypeId::INT, has_default: false },
                ParamSig { name: interner.intern("b"), ty: TypeId::INT, has_default: false },
            ],
            return_type: TypeId::INT,
            capabilities: vec![],
            is_async: false,
            span: Span::DUMMY,
        };

        registry.register_function(sig);

        let found = registry.get_function(add).unwrap();
        assert_eq!(found.name, add);
        assert_eq!(found.params.len(), 2);
        assert_eq!(found.return_type, TypeId::INT);
    }

    #[test]
    fn test_registry_types() {
        let interner = StringInterner::new();
        let mut registry = DefinitionRegistry::new();

        let point = interner.intern("Point");
        let def = TypeDef {
            name: point,
            type_params: vec![],
            kind: TypeDefKind::Struct(vec![
                FieldDef { name: interner.intern("x"), ty: TypeId::INT, is_public: true },
                FieldDef { name: interner.intern("y"), ty: TypeId::INT, is_public: true },
            ]),
            span: Span::DUMMY,
        };

        registry.register_type(def);

        let found = registry.get_type(point).unwrap();
        assert_eq!(found.name, point);
        if let TypeDefKind::Struct(fields) = &found.kind {
            assert_eq!(fields.len(), 2);
        } else {
            panic!("Expected struct");
        }
    }
}
