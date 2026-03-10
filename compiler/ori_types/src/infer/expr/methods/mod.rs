//! Built-in method resolution for primitives and collections.
//!
//! Resolution uses the [`ori_registry`] as the single source of truth for
//! method existence and return types. The flow:
//!
//! 1. [`registry_bridge::tag_to_type_tag`] — convert `Tag` → `TypeTag`
//! 2. [`ori_registry::find_method`] — look up method by `(TypeTag, name)`
//! 3. [`registry_bridge::return_tag_to_idx`] — convert `ReturnTag` → `Idx`
//! 4. [`computed_returns`] — handle `ReturnTag::Fresh` methods needing
//!    specific type construction (DEI propagation, tuple pairs)
//!
//! Named/Applied types (user-defined) bypass the registry and use
//! [`resolve_named_type_method`] directly.

mod computed_returns;

use ori_registry::ReturnTag;

use crate::infer::InferEngine;
use crate::{Idx, Tag, TypeKind};

use super::registry_bridge;

/// Methods that require iteration and are therefore invalid on `Range<float>`.
///
/// Float ranges are not `Iterable` — these methods must be rejected even though
/// the registry defines them (the registry is type-parameter agnostic).
pub(crate) const RANGE_FLOAT_ITERATION_METHODS: &[&str] = &["collect", "iter", "to_list"];

/// Resolve a built-in method call on a known type tag.
///
/// Returns `Some(return_type)` if the method is a known built-in,
/// `None` if the method is not recognized for this type tag.
///
/// Uses the `ori_registry` for method lookup and return type resolution.
/// Named/Applied types (user-defined) bypass the registry.
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    // Named/Applied types: user-defined, not in registry.
    // Supports newtype `.unwrap()`/`.inner()`/`.value()` and common trait methods.
    if matches!(tag, Tag::Named | Tag::Applied) {
        return resolve_named_type_method(engine, receiver_ty, method_name);
    }

    // Convert to registry TypeTag
    let type_tag = registry_bridge::tag_to_type_tag(tag)?;

    // Look up method in registry (DEI-filtering handled by find_method)
    let method_def = ori_registry::find_method(type_tag, method_name)?;

    // Range<float> iteration rejection: iter/to_list/collect not available on float ranges
    if tag == Tag::Range && is_float_range_iteration(engine, receiver_ty, method_name) {
        return None;
    }

    // Convert return type to pool Idx
    let return_ty = if method_def.returns == ReturnTag::Fresh {
        computed_returns::resolve_computed_return(engine, receiver_ty, tag, method_name)
    } else {
        registry_bridge::return_tag_to_idx(engine, receiver_ty, method_def.returns)
    };

    Some(return_ty)
}

/// Returns `true` for Range iteration methods when the range element type is float.
///
/// `Range<float>` does not implement `Iterable` — iteration methods must be
/// rejected even though the registry defines them (the registry is type-parameter
/// agnostic).
fn is_float_range_iteration(engine: &InferEngine<'_>, receiver_ty: Idx, method: &str) -> bool {
    RANGE_FLOAT_ITERATION_METHODS.contains(&method)
        && engine.pool().range_elem(receiver_ty) == Idx::FLOAT
}

/// Resolve methods on Named/Applied types (user-defined structs, enums, newtypes).
///
/// For newtypes, supports `.unwrap()` to extract the inner value.
fn resolve_named_type_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method_name: &str,
) -> Option<Idx> {
    // Check type registry for newtype unwrap
    if method_name == "unwrap" || method_name == "inner" || method_name == "value" {
        if let Some(type_registry) = engine.type_registry() {
            if let Some(entry) = type_registry.get_by_idx(receiver_ty) {
                if let TypeKind::Newtype { underlying } = &entry.kind {
                    return Some(*underlying);
                }
            }
        }
    }

    // Common methods on any user-defined type
    match method_name {
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating the type checker's method resolution.
// Compile-time exhaustiveness guard (Roc pattern): adding a TypeTag variant without
// updating this match = compile error. See plans/type_strategy_registry/section-14.
#[allow(
    dead_code,
    unreachable_code,
    reason = "compile-time exhaustiveness guard — never called"
)]
fn _enforce_type_tag_exhaustiveness(tag: ori_registry::TypeTag) {
    match tag {
        // All 23 TypeTag variants — resolved via registry lookup in resolve_named_type_method()
        ori_registry::TypeTag::Int
        | ori_registry::TypeTag::Float
        | ori_registry::TypeTag::Bool
        | ori_registry::TypeTag::Char
        | ori_registry::TypeTag::Byte
        | ori_registry::TypeTag::Duration
        | ori_registry::TypeTag::Size
        | ori_registry::TypeTag::Ordering
        | ori_registry::TypeTag::Str
        | ori_registry::TypeTag::Error
        | ori_registry::TypeTag::List
        | ori_registry::TypeTag::Map
        | ori_registry::TypeTag::Set
        | ori_registry::TypeTag::Range
        | ori_registry::TypeTag::Tuple
        | ori_registry::TypeTag::Option
        | ori_registry::TypeTag::Result
        | ori_registry::TypeTag::Channel
        | ori_registry::TypeTag::Iterator
        | ori_registry::TypeTag::DoubleEndedIterator
        | ori_registry::TypeTag::Function
        | ori_registry::TypeTag::Unit
        | ori_registry::TypeTag::Never => {}
    }
}
