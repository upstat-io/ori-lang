// AST item definitions for Sigil
// Contains top-level definition types: functions, tests, configs, types, imports

use super::types::TypeExpr;
use super::Span;
use super::SpannedExpr;

/// Test definition
#[derive(Debug, Clone)]
pub struct TestDef {
    pub name: String,
    pub target: String, // The function being tested
    pub body: SpannedExpr,
    pub span: Span,
}

/// Config definition: $name: Type = value
#[derive(Debug, Clone)]
pub struct ConfigDef {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub value: SpannedExpr,
    pub span: Span,
}

/// Type definition
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub public: bool,
    pub name: String,
    pub params: Vec<String>, // Generic parameters
    pub kind: TypeDefKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeDefKind {
    /// Newtype alias: type UserId = str
    Alias(TypeExpr),

    /// Struct: type User { id: UserId, name: str }
    Struct(Vec<Field>),

    /// Enum/Sum type: type Error = NotFound | Invalid { msg: str }
    Enum(Vec<Variant>),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>, // Empty for unit variants
}

/// A type parameter with optional trait bounds
/// e.g., T or T: Comparable + Eq
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<String>, // Trait bounds (can be empty)
}

impl TypeParam {
    /// Create an unbounded type parameter
    pub fn unbounded(name: String) -> Self {
        Self { name, bounds: vec![] }
    }

    /// Create a bounded type parameter
    pub fn bounded(name: String, bounds: Vec<String>) -> Self {
        Self { name, bounds }
    }
}

/// Function definition
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub public: bool,
    pub name: String,
    pub type_params: Vec<String>, // Generic type parameters (names only, for compatibility)
    pub type_param_bounds: Vec<TypeParam>, // Type parameters with bounds
    pub where_clause: Vec<WhereBound>, // Additional where clause constraints
    pub uses_clause: Vec<String>, // Capability requirements: uses Http, FileSystem
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub body: SpannedExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

/// Use/import definition
#[derive(Debug, Clone)]
pub struct UseDef {
    pub path: Vec<String>,
    pub items: Vec<UseItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseItem {
    pub name: String,
    pub alias: Option<String>,
}

/// Trait definition
/// trait Name<Params>: Supertraits { methods }
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub public: bool,
    pub name: String,
    pub type_params: Vec<String>,
    pub supertraits: Vec<String>,
    pub associated_types: Vec<AssociatedType>,
    pub methods: Vec<TraitMethodDef>,
    pub span: Span,
}

/// Associated type in a trait: type Item: Bound
#[derive(Debug, Clone)]
pub struct AssociatedType {
    pub name: String,
    pub bounds: Vec<String>, // Trait bounds on the associated type
    pub default: Option<TypeExpr>, // Optional default type
}

/// Method in a trait definition
#[derive(Debug, Clone)]
pub struct TraitMethodDef {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub default_body: Option<SpannedExpr>, // Default implementation
    pub span: Span,
}

/// Implementation block
/// impl<Params> Trait for Type where Bounds { methods }
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub type_params: Vec<String>,
    pub trait_name: Option<String>, // None for inherent impl
    pub for_type: TypeExpr,
    pub where_clause: Vec<WhereBound>,
    pub associated_types: Vec<AssociatedTypeImpl>,
    pub methods: Vec<FunctionDef>,
    pub span: Span,
}

/// Where clause bound: T: Trait + Trait2
#[derive(Debug, Clone)]
pub struct WhereBound {
    pub type_param: String,
    pub bounds: Vec<String>,
}

/// Associated type implementation: type Item = ConcreteType
#[derive(Debug, Clone)]
pub struct AssociatedTypeImpl {
    pub name: String,
    pub ty: TypeExpr,
}
