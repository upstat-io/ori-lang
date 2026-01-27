//! Trait and Implementation Types
//!
//! Generic parameters, trait definitions, impl blocks, and extension methods.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::{GenericParamRange, ParamRange};
use crate::{ExprId, Name, ParsedType, Span, Spanned};

/// Generic parameter: `T` or `T: Bound` or `T: A + B`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct GenericParam {
    pub name: Name,
    pub bounds: Vec<TraitBound>,
    pub span: Span,
}

/// A trait bound: `Eq`, `Comparable`, or path like `std.collections.Iterator`.
///
/// The path is guaranteed non-empty by construction: `first` is always present,
/// and `rest` contains any additional segments.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitBound {
    /// The first segment of the path (required).
    pub first: Name,
    /// Additional path segments (e.g., for `std.collections.Iterator`, this would be `["collections", "Iterator"]`).
    pub rest: Vec<Name>,
    pub span: Span,
}

impl TraitBound {
    /// Get the simple name (last segment) of the trait bound.
    pub fn name(&self) -> Name {
        self.rest.last().copied().unwrap_or(self.first)
    }

    /// Get the full path as a vector.
    pub fn path(&self) -> Vec<Name> {
        let mut path = vec![self.first];
        path.extend(&self.rest);
        path
    }
}

/// Where clause constraint: `T: Clone`, `U: Default`, or `T.Item: Eq`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WhereClause {
    /// The type parameter being constrained (e.g., `T` in `T: Clone` or `T.Item: Eq`).
    pub param: Name,
    /// Optional associated type projection (e.g., `Item` in `T.Item: Eq`).
    /// If `Some`, this is a constraint on an associated type: `param.projection: bounds`.
    pub projection: Option<Name>,
    /// The trait bounds that must be satisfied.
    pub bounds: Vec<TraitBound>,
    pub span: Span,
}

/// Trait definition.
///
/// ```ori
/// trait Printable {
///     @to_string (self) -> str
/// }
///
/// trait Comparable: Eq {
///     @compare (self, other: Self) -> Ordering
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitDef {
    pub name: Name,
    pub generics: GenericParamRange,
    /// Super-traits (inheritance): `trait Child: Parent`
    pub super_traits: Vec<TraitBound>,
    pub items: Vec<TraitItem>,
    pub span: Span,
    pub is_public: bool,
}

impl Spanned for TraitDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Item inside a trait definition.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TraitItem {
    /// Required method signature: `@method (self) -> Type`
    MethodSig(TraitMethodSig),
    /// Method with default implementation: `@method (self) -> Type = expr`
    DefaultMethod(TraitDefaultMethod),
    /// Associated type: `type Item`
    AssocType(TraitAssocType),
}

/// Required method signature in a trait.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitMethodSig {
    pub name: Name,
    pub params: ParamRange,
    /// The parsed return type.
    pub return_ty: ParsedType,
    pub span: Span,
}

impl Spanned for TraitMethodSig {
    fn span(&self) -> Span {
        self.span
    }
}

/// Method with default implementation in a trait.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitDefaultMethod {
    pub name: Name,
    pub params: ParamRange,
    /// The parsed return type.
    pub return_ty: ParsedType,
    pub body: ExprId,
    pub span: Span,
}

impl Spanned for TraitDefaultMethod {
    fn span(&self) -> Span {
        self.span
    }
}

/// Associated type in a trait.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitAssocType {
    pub name: Name,
    pub span: Span,
}

impl Spanned for TraitAssocType {
    fn span(&self) -> Span {
        self.span
    }
}

/// Implementation block.
///
/// ```ori
/// // Inherent impl
/// impl Point {
///     @new (x: int, y: int) -> Point = Point { x, y }
/// }
///
/// // Trait impl
/// impl Printable for Point {
///     @to_string (self) -> str = "..."
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplDef {
    pub generics: GenericParamRange,
    /// The trait being implemented (None for inherent impl).
    pub trait_path: Option<Vec<Name>>,
    /// The type path being implemented (e.g., `["Point"]` for `impl Point { ... }`).
    /// Used for method dispatch lookup.
    pub self_path: Vec<Name>,
    /// The parsed type implementing the trait (or receiving inherent methods).
    pub self_ty: ParsedType,
    pub where_clauses: Vec<WhereClause>,
    pub methods: Vec<ImplMethod>,
    /// Associated type definitions (e.g., `type Item = T`).
    pub assoc_types: Vec<ImplAssocType>,
    pub span: Span,
}

impl ImplDef {
    /// Returns true if this is an inherent impl (no trait).
    pub fn is_inherent(&self) -> bool {
        self.trait_path.is_none()
    }

    /// Returns true if this is a trait impl.
    pub fn is_trait_impl(&self) -> bool {
        self.trait_path.is_some()
    }
}

impl Spanned for ImplDef {
    fn span(&self) -> Span {
        self.span
    }
}

/// Method in an impl block.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplMethod {
    pub name: Name,
    pub params: ParamRange,
    /// The parsed return type.
    pub return_ty: ParsedType,
    pub body: ExprId,
    pub span: Span,
}

/// Associated type definition in an impl block.
///
/// ```ori
/// impl Iterator for List<T> {
///     type Item = T
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplAssocType {
    /// The associated type name (e.g., `Item`).
    pub name: Name,
    /// The concrete type being assigned (e.g., `T` in `type Item = T`).
    pub ty: ParsedType,
    pub span: Span,
}

impl Spanned for ImplAssocType {
    fn span(&self) -> Span {
        self.span
    }
}

/// Extension method definition.
/// Syntax: `extend Type { @method (self, ...) -> ReturnType = body }`
///
/// Extensions add methods to existing types without modifying their definition.
/// Used to add methods like `map`, `filter` to built-in types like `[T]`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExtendDef {
    /// Generic parameters: `extend<T> [T] { ... }`
    pub generics: GenericParamRange,
    /// The parsed type being extended (e.g., `[T]`, `Option<T>`, `str`)
    pub target_ty: ParsedType,
    /// String representation of the target type for method dispatch
    /// e.g., "list" for `[T]`, "Option" for `Option<T>`
    pub target_type_name: Name,
    /// Where clauses for constraints
    pub where_clauses: Vec<WhereClause>,
    /// Methods being added
    pub methods: Vec<ImplMethod>,
    pub span: Span,
}

impl Spanned for ExtendDef {
    fn span(&self) -> Span {
        self.span
    }
}
