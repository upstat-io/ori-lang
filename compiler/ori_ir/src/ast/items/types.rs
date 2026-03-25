//! Type Declaration AST Nodes
//!
//! User-defined types: structs, sum types (enums), and newtypes.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{Name, ParsedType, Span, Spanned};

use super::super::ranges::GenericParamRange;
use super::super::Visibility;
use super::traits::WhereClause;

/// User-specified representation attribute (`#repr(...)`).
///
/// Parsed by `ori_parse` and stored in the AST as a data-only enum.
/// Converted to `ori_repr::ReprAttribute` during representation planning.
///
/// Defined in `ori_ir` (not `ori_parse`) to keep the dependency direction
/// `ori_parse → ori_ir` — `TypeDecl` stores this, so it must live in `ori_ir`.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum ReprAttrKind {
    /// `#repr("c")` — C-compatible layout (no field reordering).
    C,
    /// `#repr("packed")` — no padding between fields.
    Packed,
    /// `#repr("transparent")` — same layout as the single field.
    Transparent,
    /// `#repr("aligned", N)` — minimum alignment (power of two).
    Aligned(u64),
    /// `#repr("c")` + `#repr("aligned", N)` — C layout with enforced alignment.
    ///
    /// Produced by type checking when stacked `#repr("c")` and `#repr("aligned", N)`
    /// are combined. Not directly parseable — always merged from two separate attrs.
    CAligned(u64),
}

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
    /// The kind of type being declared
    pub kind: TypeDeclKind,
    /// Generic parameters: `<T, U: Bound>`
    pub generics: GenericParamRange,
    /// Where clauses: `where T: Clone, U: Default`
    pub where_clauses: Vec<WhereClause>,
    pub span: Span,
    pub visibility: Visibility,
    /// Derived traits: `#[derive(Eq, Clone)]`
    pub derives: Vec<Name>,
    /// Representation attributes: `#repr("c")`, `#repr("packed")`, etc.
    ///
    /// Multiple `#repr` attributes may be stacked (e.g., `#repr("c") #repr("aligned", 16)`).
    /// Combination validation and merging happens in type checking, not parsing.
    pub repr_attrs: Vec<ReprAttrKind>,
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
