//! Top-level items in a module.

use crate::intern::Name;
use super::{Span, ExprId, ParamRange, TypeExprId, expr::TypeExpr};

/// Item identifier (index into module's item list).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct ItemId(u32);

impl ItemId {
    pub const fn new(index: u32) -> Self {
        ItemId(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Top-level item in a module.
#[derive(Clone, Debug)]
pub struct Item {
    pub kind: ItemKind,
    pub span: Span,
}

/// Kinds of top-level items.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// Function: @name (params) -> Type = body
    Function(Function),
    /// Type definition: type Name = ...
    TypeDef(TypeDef),
    /// Config variable: $name = value
    Config(Config),
    /// Import: use './path' { items } or use std.module { items }
    Import(Import),
    /// Test: @test_name tests @target () -> void = ...
    Test(Test),
    /// Trait: trait Name { ... }
    Trait(Trait),
    /// Implementation: impl Type { ... } or impl Trait for Type { ... }
    Impl(Impl),
}

/// Function definition.
#[derive(Clone, Debug)]
pub struct Function {
    /// Function name
    pub name: Name,
    /// Visibility
    pub visibility: Visibility,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Parameters
    pub params: ParamRange,
    /// Return type
    pub return_type: Option<TypeExprId>,
    /// Required capabilities
    pub capabilities: Vec<Name>,
    /// Body expression
    pub body: ExprId,
    /// Whether this is async
    pub is_async: bool,
    /// Span of the function signature (without body)
    pub sig_span: Span,
}

/// Type definition.
#[derive(Clone, Debug)]
pub struct TypeDef {
    /// Type name
    pub name: Name,
    /// Visibility
    pub visibility: Visibility,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Type body
    pub kind: TypeDefKind,
    /// Derived traits
    pub derives: Vec<Name>,
}

/// Type definition kinds.
#[derive(Clone, Debug)]
pub enum TypeDefKind {
    /// Struct: type Point = { x: int, y: int }
    Struct(Vec<StructField>),
    /// Sum type: type Option<T> = Some(T) | None
    Enum(Vec<EnumVariant>),
    /// Newtype/alias: type UserId = int
    Alias(TypeExpr),
}

/// Struct field.
#[derive(Clone, Debug)]
pub struct StructField {
    pub name: Name,
    pub ty: TypeExpr,
    pub visibility: Visibility,
    pub span: Span,
}

/// Enum variant.
#[derive(Clone, Debug)]
pub struct EnumVariant {
    pub name: Name,
    pub fields: Option<Vec<TypeExpr>>,
    pub span: Span,
}

/// Config variable.
#[derive(Clone, Debug)]
pub struct Config {
    /// Config name
    pub name: Name,
    /// Visibility
    pub visibility: Visibility,
    /// Type annotation (optional)
    pub ty: Option<TypeExprId>,
    /// Initial value
    pub value: ExprId,
}

/// Import declaration.
#[derive(Clone, Debug)]
pub struct Import {
    /// Path (relative or module)
    pub path: ImportPath,
    /// Imported items
    pub items: Vec<ImportItem>,
    /// Module alias (for `use std.net.http as http`)
    pub alias: Option<Name>,
    /// Re-export (pub use)
    pub is_pub: bool,
}

/// Import path.
#[derive(Clone, Debug)]
pub enum ImportPath {
    /// Relative path: './math', '../utils'
    Relative(String),
    /// Module path: std.math, std.time
    Module(Vec<Name>),
}

/// Single imported item.
#[derive(Clone, Debug)]
pub struct ImportItem {
    /// Item name
    pub name: Name,
    /// Alias (for `add as plus`)
    pub alias: Option<Name>,
    /// Whether to import private (::)
    pub is_private: bool,
    pub span: Span,
}

/// Test declaration.
#[derive(Clone, Debug)]
pub struct Test {
    /// Test name
    pub name: Name,
    /// Functions being tested
    pub targets: Vec<Name>,
    /// Test body
    pub body: ExprId,
    /// Skip reason (if #[skip("reason")] attribute is present)
    pub skip_reason: Option<Name>,
}

/// Trait definition.
#[derive(Clone, Debug)]
pub struct Trait {
    /// Trait name
    pub name: Name,
    /// Visibility
    pub visibility: Visibility,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Super traits
    pub super_traits: Vec<TypeExpr>,
    /// Trait items (methods)
    pub items: Vec<TraitItem>,
}

/// Trait item (method or associated type).
#[derive(Clone, Debug)]
pub enum TraitItem {
    /// Method: @method (self) -> Type = default_impl
    Method {
        name: Name,
        params: ParamRange,
        return_type: Option<TypeExprId>,
        default: Option<ExprId>,
        span: Span,
    },
    /// Associated type: type Item
    AssociatedType {
        name: Name,
        bounds: Vec<TypeExpr>,
        default: Option<TypeExpr>,
        span: Span,
    },
}

/// Implementation block.
#[derive(Clone, Debug)]
pub struct Impl {
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Trait being implemented (None for inherent impl)
    pub trait_: Option<TypeExpr>,
    /// Type being implemented for
    pub target: TypeExpr,
    /// Where clause constraints
    pub where_clause: Vec<WhereClause>,
    /// Implementation items
    pub items: Vec<ImplItem>,
}

/// Implementation item.
#[derive(Clone, Debug)]
pub enum ImplItem {
    /// Method: @method (self) -> Type = body
    Method {
        name: Name,
        visibility: Visibility,
        params: ParamRange,
        return_type: Option<TypeExprId>,
        body: ExprId,
        span: Span,
    },
    /// Associated type: type Item = ConcreteType
    AssociatedType {
        name: Name,
        value: TypeExpr,
        span: Span,
    },
}

/// Generic type parameter.
#[derive(Clone, Debug)]
pub struct TypeParam {
    pub name: Name,
    pub bounds: Vec<TypeExpr>,
    pub default: Option<TypeExpr>,
    pub span: Span,
}

/// Where clause constraint.
#[derive(Clone, Debug)]
pub struct WhereClause {
    pub ty: TypeExpr,
    pub bounds: Vec<TypeExpr>,
    pub span: Span,
}

/// Visibility modifier.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        self == Visibility::Public
    }
}
