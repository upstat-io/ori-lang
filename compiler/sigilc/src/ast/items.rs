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

/// Function definition
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub public: bool,
    pub name: String,
    pub type_params: Vec<String>, // Generic type parameters
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
