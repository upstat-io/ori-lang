//! Bridge between `ori_types` internal types and `ori_registry` type tags.
//!
//! This module mediates between the type checker's `Tag` enum (internal representation)
//! and the registry's `TypeTag` enum (builtin type identity). The bridge enables
//! registry-based method lookup from type checker dispatch sites.
//!
//! Two bridge functions:
//! - [`tag_to_type_tag`]: `Tag` → `TypeTag` (enables registry lookup from type checker)
//! - [`return_tag_to_idx`]: `ReturnTag` → `Idx` (converts registry return types to pool handles)
//!
//! This module complements `WellKnownNames` (in `check/well_known/`). The two
//! serve different purposes: `WellKnownNames` maps parsed type *names* (`Name`)
//! to pool constructors for type resolution, while this bridge maps type *tags*
//! to registry queries for method resolution.

use ori_registry::{ReturnTag, TypeProjection, TypeTag};

use crate::{Idx, Tag};

use super::super::InferEngine;

/// Map the type checker's [`Tag`] to the registry's [`TypeTag`].
///
/// Returns `None` for tags that are not builtin types (Named, Applied,
/// Var, Function, etc.) — these are handled by trait/impl dispatch,
/// not the builtin registry.
#[must_use]
pub(crate) fn tag_to_type_tag(tag: Tag) -> Option<TypeTag> {
    match tag {
        // Primitives
        Tag::Int => Some(TypeTag::Int),
        Tag::Float => Some(TypeTag::Float),
        Tag::Bool => Some(TypeTag::Bool),
        Tag::Str => Some(TypeTag::Str),
        Tag::Char => Some(TypeTag::Char),
        Tag::Byte => Some(TypeTag::Byte),

        // Compound types
        Tag::Duration => Some(TypeTag::Duration),
        Tag::Size => Some(TypeTag::Size),
        Tag::Ordering => Some(TypeTag::Ordering),
        Tag::Error => Some(TypeTag::Error),

        // Collections & containers
        Tag::List => Some(TypeTag::List),
        Tag::Option => Some(TypeTag::Option),
        Tag::Result => Some(TypeTag::Result),
        Tag::Map => Some(TypeTag::Map),
        Tag::Set => Some(TypeTag::Set),
        Tag::Channel => Some(TypeTag::Channel),
        Tag::Range => Some(TypeTag::Range),
        Tag::Tuple => Some(TypeTag::Tuple),

        // Iterators
        Tag::Iterator => Some(TypeTag::Iterator),
        Tag::DoubleEndedIterator => Some(TypeTag::DoubleEndedIterator),

        // Not builtin registry types — handled by trait/impl dispatch
        // or have no methods:
        Tag::Named
        | Tag::Applied
        | Tag::Alias
        | Tag::Struct
        | Tag::Enum
        | Tag::Unit
        | Tag::Never
        | Tag::Var
        | Tag::BoundVar
        | Tag::RigidVar
        | Tag::Function
        | Tag::Scheme
        | Tag::Borrowed
        | Tag::Projection
        | Tag::ModuleNs
        | Tag::Infer
        | Tag::SelfType => None,
    }
}

/// Convert a registry [`ReturnTag`] to a pool [`Idx`], using the receiver type
/// to resolve parameterized return types.
///
/// `receiver_ty` is the resolved receiver type (e.g., `List<int>`,
/// `Iterator<str>`). Used to extract inner type parameters when the
/// return type is parameterized.
pub(crate) fn return_tag_to_idx(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    return_tag: ReturnTag,
) -> Idx {
    match return_tag {
        // Concrete types: delegate to TypeTag mapping
        ReturnTag::Concrete(type_tag) => concrete_tag_to_idx(engine, receiver_ty, type_tag),

        // Signature-level types
        ReturnTag::SelfType => receiver_ty,
        ReturnTag::Fresh => engine.fresh_var(),
        ReturnTag::Unit => Idx::UNIT,

        // Direct type-parameter projections
        ReturnTag::ElementType => extract_elem(engine, receiver_ty),
        ReturnTag::KeyType => engine.pool().map_key(receiver_ty),
        ReturnTag::ValueType => engine.pool().map_value(receiver_ty),
        ReturnTag::OkType => engine.pool().result_ok(receiver_ty),
        ReturnTag::ErrType => engine.pool().result_err(receiver_ty),

        // Parameterized wrappers (projection-based)
        ReturnTag::OptionOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().option(inner)
        }
        ReturnTag::ListOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().list(inner)
        }
        ReturnTag::IteratorOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().iterator(inner)
        }
        ReturnTag::DoubleEndedIteratorOf(proj) => {
            let inner = resolve_projection(engine, receiver_ty, proj);
            engine.pool_mut().double_ended_iterator(inner)
        }

        // Fixed-inner wrappers
        ReturnTag::List(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().list(inner)
        }
        ReturnTag::Option(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().option(inner)
        }
        ReturnTag::DoubleEndedIterator(inner_tag) => {
            let inner = type_tag_to_idx(inner_tag);
            engine.pool_mut().double_ended_iterator(inner)
        }

        // Composite returns
        ReturnTag::NextResult => {
            let elem = extract_elem(engine, receiver_ty);
            let option_elem = engine.pool_mut().option(elem);
            engine.pool_mut().tuple(&[option_elem, receiver_ty])
        }
        ReturnTag::ResultOfProjectionFresh(proj) => {
            let ok_ty = resolve_projection(engine, receiver_ty, proj);
            let err_ty = engine.fresh_var();
            engine.pool_mut().result(ok_ty, err_ty)
        }
        ReturnTag::ListKeyValue => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            engine.pool_mut().list(pair)
        }
        ReturnTag::MapIterator => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            engine.pool_mut().iterator(pair)
        }
        ReturnTag::ListOfTupleIntElement => {
            let elem = extract_elem(engine, receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            engine.pool_mut().list(pair)
        }
        ReturnTag::IteratorOfTupleIntElement => {
            let elem = extract_elem(engine, receiver_ty);
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            engine.pool_mut().iterator(pair)
        }
    }
}

/// Map a [`TypeTag`] used as a `ReturnTag::Concrete` to an [`Idx`].
///
/// Primitives and compound types map to fixed constants. Parameterized types
/// (List, Option, etc.) construct in the pool using the receiver's inner type.
fn concrete_tag_to_idx(engine: &mut InferEngine<'_>, receiver_ty: Idx, type_tag: TypeTag) -> Idx {
    match type_tag {
        // Fixed constants (no pool construction needed)
        TypeTag::Int => Idx::INT,
        TypeTag::Float => Idx::FLOAT,
        TypeTag::Bool => Idx::BOOL,
        TypeTag::Str => Idx::STR,
        TypeTag::Char => Idx::CHAR,
        TypeTag::Byte => Idx::BYTE,
        TypeTag::Unit => Idx::UNIT,
        TypeTag::Ordering => Idx::ORDERING,
        TypeTag::Duration => Idx::DURATION,
        TypeTag::Size => Idx::SIZE,
        TypeTag::Error => Idx::ERROR,

        // Parameterized: construct in pool from receiver's inner type
        TypeTag::Option => {
            let elem = extract_elem(engine, receiver_ty);
            engine.pool_mut().option(elem)
        }
        TypeTag::List => {
            let elem = extract_elem(engine, receiver_ty);
            engine.pool_mut().list(elem)
        }
        TypeTag::Set => {
            let elem = extract_elem(engine, receiver_ty);
            engine.pool_mut().set(elem)
        }
        TypeTag::Iterator => {
            let elem = extract_elem(engine, receiver_ty);
            engine.pool_mut().iterator(elem)
        }
        TypeTag::DoubleEndedIterator => {
            let elem = extract_elem(engine, receiver_ty);
            engine.pool_mut().double_ended_iterator(elem)
        }
        TypeTag::Result => {
            let ok_ty = engine.pool().result_ok(receiver_ty);
            let err_ty = engine.pool().result_err(receiver_ty);
            engine.pool_mut().result(ok_ty, err_ty)
        }
        TypeTag::Map => {
            let key_ty = engine.pool().map_key(receiver_ty);
            let value_ty = engine.pool().map_value(receiver_ty);
            engine.pool_mut().map(key_ty, value_ty)
        }

        // Not currently used as method return types
        TypeTag::Never | TypeTag::Channel | TypeTag::Range | TypeTag::Tuple | TypeTag::Function => {
            unreachable!(
                "TypeTag::{type_tag:?} appeared as a method return type — \
                 add pool construction for this tag"
            );
        }
    }
}

/// Extract the primary element type from a container receiver.
///
/// For `List<T>`, `Set<T>`, `Option<T>`, `Iterator<T>`, `DEI<T>`,
/// `Channel<T>`, `Range<T>`: returns `T`.
/// For `Map<K, V>`: returns `K` (the primary element).
/// For `Result<T, E>`: returns `T` (the ok type).
fn extract_elem(engine: &InferEngine<'_>, receiver_ty: Idx) -> Idx {
    let tag = engine.pool().tag(receiver_ty);
    match tag {
        Tag::List => engine.pool().list_elem(receiver_ty),
        Tag::Set => engine.pool().set_elem(receiver_ty),
        Tag::Option => engine.pool().option_inner(receiver_ty),
        Tag::Iterator | Tag::DoubleEndedIterator => engine.pool().iterator_elem(receiver_ty),
        Tag::Channel => engine.pool().channel_elem(receiver_ty),
        Tag::Range => engine.pool().range_elem(receiver_ty),
        Tag::Map => engine.pool().map_key(receiver_ty),
        Tag::Result => engine.pool().result_ok(receiver_ty),
        _ => unreachable!("extract_elem called on non-container type {tag:?}"),
    }
}

/// Map a [`TypeTag`] to a fixed [`Idx`] constant.
///
/// Only handles concrete types with pre-interned Idx constants. Panics on
/// parameterized types — those require pool construction and should not
/// appear inside `ReturnTag::List(TypeTag)` wrappers.
fn type_tag_to_idx(tag: TypeTag) -> Idx {
    match tag {
        TypeTag::Int => Idx::INT,
        TypeTag::Float => Idx::FLOAT,
        TypeTag::Bool => Idx::BOOL,
        TypeTag::Str => Idx::STR,
        TypeTag::Char => Idx::CHAR,
        TypeTag::Byte => Idx::BYTE,
        TypeTag::Unit => Idx::UNIT,
        TypeTag::Ordering => Idx::ORDERING,
        TypeTag::Duration => Idx::DURATION,
        TypeTag::Size => Idx::SIZE,
        TypeTag::Error => Idx::ERROR,
        _ => unreachable!(
            "type_tag_to_idx: TypeTag::{tag:?} is parameterized — \
             cannot appear as a fixed inner type in ReturnTag wrappers"
        ),
    }
}

/// Map a [`TypeProjection`] to a concrete [`Idx`] from the receiver type.
fn resolve_projection(engine: &mut InferEngine<'_>, receiver_ty: Idx, proj: TypeProjection) -> Idx {
    match proj {
        TypeProjection::Element => extract_elem(engine, receiver_ty),
        TypeProjection::Key => engine.pool().map_key(receiver_ty),
        TypeProjection::Value => engine.pool().map_value(receiver_ty),
        TypeProjection::Ok => engine.pool().result_ok(receiver_ty),
        TypeProjection::Err => engine.pool().result_err(receiver_ty),
        TypeProjection::Fixed(tag) => type_tag_to_idx(tag),
    }
}

#[cfg(test)]
mod tests;
