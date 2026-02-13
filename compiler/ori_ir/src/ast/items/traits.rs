//! Trait and Implementation Types
//!
//! Generic parameters, trait definitions, impl blocks, and extension methods.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::ranges::{GenericParamRange, ParamRange};
use super::super::Visibility;
use crate::{ExprId, Name, ParsedType, ParsedTypeRange, Span, Spanned};

/// Generic parameter: type param (`T`, `T: Bound`) or const param (`$N: int`).
///
/// Default type parameters allow trait definitions like `trait Add<Rhs = Self>`.
/// Const generics allow compile-time values: `@f<$N: int>`, `@f<$B: bool = true>`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct GenericParam {
    pub name: Name,
    /// Trait bounds for type parameters (e.g., `T: Eq + Clone`).
    /// Empty for const parameters.
    pub bounds: Vec<TraitBound>,
    /// Default type for this parameter (e.g., `Self` in `trait Add<Rhs = Self>`).
    /// When present, this type is used if the impl omits the type argument.
    pub default_type: Option<ParsedType>,
    /// If true, this is a const generic parameter (`$N: int`).
    /// The type is stored in `const_type`, not `bounds`.
    pub is_const: bool,
    /// For const parameters: the type (e.g., `int` in `$N: int`).
    /// None for type parameters.
    pub const_type: Option<ParsedType>,
    /// For const params: the default value (e.g., `10` in `$N: int = 10`).
    /// None for type parameters (which use `default_type` instead).
    pub default_value: Option<ExprId>,
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
    pub visibility: Visibility,
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

impl TraitItem {
    /// Get the span of the trait item.
    pub fn span(&self) -> Span {
        match self {
            TraitItem::MethodSig(sig) => sig.span,
            TraitItem::DefaultMethod(method) => method.span,
            TraitItem::AssocType(assoc) => assoc.span,
        }
    }
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
///
/// ```ori
/// trait Iterator {
///     type Item  // No default - must be specified
/// }
///
/// trait Add<Rhs = Self> {
///     type Output = Self  // Default to Self
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitAssocType {
    pub name: Name,
    /// Default type for this associated type (e.g., `Self` in `type Output = Self`).
    /// When present, this type is used if the impl omits the associated type.
    /// Stored as `ParsedType` (not resolved) because defaults may contain `Self`
    /// which must be resolved at impl registration time, not trait registration time.
    pub default_type: Option<ParsedType>,
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
///
/// // Trait impl with type arguments
/// impl Add<int> for Point {
///     @add (self, rhs: int) -> Point = ...
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplDef {
    pub generics: GenericParamRange,
    /// The trait being implemented (None for inherent impl).
    pub trait_path: Option<Vec<Name>>,
    /// Type arguments for the trait (e.g., `[int]` in `impl Add<int> for Point`).
    /// Empty if no type arguments specified or if this is an inherent impl.
    pub trait_type_args: ParsedTypeRange,
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

impl From<&TraitDefaultMethod> for ImplMethod {
    fn from(default: &TraitDefaultMethod) -> Self {
        Self {
            name: default.name,
            params: default.params,
            return_ty: default.return_ty.clone(),
            body: default.body,
            span: default.span,
        }
    }
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

/// Default implementation for a trait.
///
/// Provides the standard behavior for a trait when imported. Unlike regular `impl`,
/// `def impl` methods don't have a `self` parameter (stateless) and there's no
/// `for Type` clause (anonymous implementation).
///
/// ```ori
/// pub def impl Http {
///     @get (url: str) -> Result<Response, Error> = ...
///     @post (url: str, body: str) -> Result<Response, Error> = ...
/// }
/// ```
///
/// When a module exports both a trait and its `def impl`, importing the trait
/// automatically binds the default implementation.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct DefImplDef {
    /// The trait this is a default implementation for.
    pub trait_name: Name,
    /// Methods implementing the trait (must not have `self` parameter).
    pub methods: Vec<ImplMethod>,
    pub span: Span,
    pub visibility: Visibility,
}

impl Spanned for DefImplDef {
    fn span(&self) -> Span {
        self.span
    }
}
