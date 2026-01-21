// AST type expressions for Sigil
// Contains TypeExpr enum representing type annotations

/// Type expressions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    /// Named type: int, str, User, etc.
    Named(String),

    /// Generic type application: Result T E, List T
    Generic(String, Vec<TypeExpr>),

    /// Optional type: ?T
    Optional(Box<TypeExpr>),

    /// List type: `[T]`
    List(Box<TypeExpr>),

    /// Map type: {K: V}
    Map(Box<TypeExpr>, Box<TypeExpr>),

    /// Tuple type: (T, U)
    Tuple(Vec<TypeExpr>),

    /// Function type: T -> U
    Function(Box<TypeExpr>, Box<TypeExpr>),

    /// Anonymous record/struct type: { field1: T1, field2: T2 }
    Record(Vec<(String, TypeExpr)>),

    /// Dynamic trait object: dyn Trait
    /// Represents a type-erased reference to any type implementing the trait
    DynTrait(String),
}
