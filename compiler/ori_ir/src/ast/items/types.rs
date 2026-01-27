//! Type Declaration AST Nodes
//!
//! User-defined types: structs, sum types (enums), and newtypes.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{Name, Span, ParsedType, Spanned};
use super::super::ranges::GenericParamRange;
use super::traits::WhereClause;

/// A user-defined type declaration.
///
/// ```ori
/// type Point = { x: int, y: int }
/// type Status = Pending | Running | Done | Failed(reason: str)
/// type UserId = int
/// pub type Node<T> = { value: T, next: Option<Node<T>> }
/// #[derive(Eq, Clone)] type Config = { name: str }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeDecl {
    pub name: Name,
    /// Generic parameters: `<T, U: Bound>`
    pub generics: GenericParamRange,
    /// Where clauses: `where T: Clone, U: Default`
    pub where_clauses: Vec<WhereClause>,
    /// The kind of type being declared
    pub kind: TypeDeclKind,
    pub span: Span,
    pub is_public: bool,
    /// Derived traits: `#[derive(Eq, Clone)]`
    pub derives: Vec<Name>,
}

impl Spanned for TypeDecl {
    fn span(&self) -> Span {
        self.span
    }
}

/// The kind of user-defined type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeDeclKind {
    /// Struct type: `{ field: Type, ... }`
    Struct(Vec<StructField>),
    /// Sum type: `Variant1 | Variant2(field: Type) | ...`
    Sum(Vec<Variant>),
    /// Newtype (type alias with distinct identity): `ExistingType`
    Newtype(ParsedType),
}

/// A field in a struct type.
///
/// ```ori
/// type Point = { x: int, y: int }
///               ^^^^^^^  ^^^^^^^
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct StructField {
    pub name: Name,
    /// The parsed field type.
    pub ty: ParsedType,
    pub span: Span,
}

impl Spanned for StructField {
    fn span(&self) -> Span {
        self.span
    }
}

/// A variant in a sum type.
///
/// ```ori
/// type Status = Pending | Running | Done | Failed(reason: str)
///               ^^^^^^^   ^^^^^^^   ^^^^   ^^^^^^^^^^^^^^^^^^
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Variant {
    pub name: Name,
    /// Fields for this variant. Empty for unit variants.
    pub fields: Vec<VariantField>,
    pub span: Span,
}

impl Spanned for Variant {
    fn span(&self) -> Span {
        self.span
    }
}

/// A field in a variant.
///
/// Sum type variants can have named fields:
/// ```ori
/// type Message = Text(content: str) | Image(url: str, width: int, height: int)
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct VariantField {
    pub name: Name,
    /// The parsed field type.
    pub ty: ParsedType,
    pub span: Span,
}

impl Spanned for VariantField {
    fn span(&self) -> Span {
        self.span
    }
}
