//! Return type and type projection tags for method signatures.
//!
//! [`ReturnTag`] describes how a method's return type (or parameter type)
//! relates to the receiver — concrete, projected from type parameters,
//! or computed at inference time. [`TypeProjection`] identifies which
//! type parameter to extract from the receiver.

use super::TypeTag;

/// A type reference used in method parameter and return type positions.
///
/// Most parameters have concrete types (`TypeTag`), but some are generic
/// (e.g., the element type of a list method, or a closure parameter).
/// `ReturnTag` handles both cases.
///
/// # Naming
///
/// **Canonical name is `ReturnTag`** — not `ReturnSpec`, `ReturnType`,
/// or `ParamType`. The name applies to both parameter and return
/// positions because the same abstract type references work in both.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReturnTag {
    // Concrete types
    /// A concrete builtin type.
    Concrete(TypeTag),
    /// The receiver's own type (for methods returning `Self`).
    SelfType,
    /// Unit type (for void-returning methods like `for_each`).
    Unit,

    // Direct type-parameter projections
    /// The element type of the receiver (`T` in `List<T>`, `Option<T>`, etc.).
    ElementType,
    /// The key type (`K` in `Map<K, V>`).
    KeyType,
    /// The value type (`V` in `Map<K, V>`).
    ValueType,
    /// The ok-type (`T` in `Result<T, E>`).
    OkType,
    /// The err-type (`E` in `Result<T, E>`).
    ErrType,

    // Parameterized wrappers (fixed inner type)
    /// `[T]` for a fixed `T`. Example: `str.split(sep:) -> [str]`.
    List(TypeTag),
    /// `Option<T>` for a fixed `T`. Example: `str.to_int() -> Option<int>`.
    Option(TypeTag),
    /// `DoubleEndedIterator<T>` for a fixed `T`. Example: `str.iter() -> DEI<char>`.
    DoubleEndedIterator(TypeTag),

    // Parameterized wrappers (projection-based)
    /// `Option<P>` where P is a type projection from the receiver.
    OptionOf(TypeProjection),
    /// `[P]` where P is a type projection.
    ListOf(TypeProjection),
    /// `Iterator<P>` where P is a type projection.
    IteratorOf(TypeProjection),
    /// `DoubleEndedIterator<P>` where P is a type projection.
    DoubleEndedIteratorOf(TypeProjection),

    // Composite returns
    /// `[(K, V)]` from a Map. Example: `map.entries()`.
    ListKeyValue,
    /// `[(int, T)]` where T is the element type. Example: `list.enumerate()`.
    ListOfTupleIntElement,
    /// `Iterator<(K, V)>` from a Map. Example: `map.iter()`.
    MapIterator,
    /// `Iterator<(int, T)>` where T is the element type.
    IteratorOfTupleIntElement,

    // Tuple returns
    /// `(Option<T>, Self)` — the iterator `next()` protocol.
    NextResult,
    /// `Result<P, E>` where P is a projection and E is fresh.
    ResultOfProjectionFresh(TypeProjection),

    // Higher-order
    /// A fresh type variable — return type depends on closure argument.
    Fresh,
}

impl From<TypeTag> for ReturnTag {
    fn from(tag: TypeTag) -> Self {
        Self::Concrete(tag)
    }
}

/// Which type parameter to project from the receiver.
///
/// Used by `ReturnTag::OptionOf`, `ListOf`, `IteratorOf`,
/// `DoubleEndedIteratorOf` to express generic return types relative
/// to the receiver's type parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeProjection {
    /// The single element type (`T` in `List<T>`, `Option<T>`, `Iterator<T>`, etc.).
    Element,
    /// The key type (`K` in `Map<K, V>`).
    Key,
    /// The value type (`V` in `Map<K, V>`).
    Value,
    /// The ok-type (`T` in `Result<T, E>`).
    Ok,
    /// The err-type (`E` in `Result<T, E>`).
    Err,
    /// A fixed concrete type (not a projection from receiver params).
    Fixed(TypeTag),
}
