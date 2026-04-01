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

/// Check if a Range method requires iteration (and is thus invalid on `Range<float>`).
///
/// Note: ALL methods on `Range<float>` are now rejected at dispatch time in
/// `resolve_builtin_method()`. This function is retained for the diagnostic
/// path in `method_call.rs` which needs to distinguish "iteration method on
/// float range" (specific error E2xxx) from "any method on float range"
/// (generic "no such method" error).
pub(in crate::infer::expr) fn range_method_requires_iteration(method_name: &str) -> bool {
    use ori_registry::{ReturnTag, TypeTag};
    let Some(method) = ori_registry::find_method(TypeTag::Range, method_name) else {
        return false;
    };
    matches!(
        method.returns,
        ReturnTag::IteratorOf(_)
            | ReturnTag::DoubleEndedIteratorOf(_)
            | ReturnTag::ListOf(_)
            | ReturnTag::ListOfTupleIntElement
            | ReturnTag::IteratorOfTupleIntElement
    )
}

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

    // Range<float> rejection: ALL methods are unavailable because the evaluator
    // only supports integer ranges. Float range creation (0.0..10.0) is rejected
    // at runtime — method dispatch must be rejected at type-check time too.
    if tag == Tag::Range && engine.pool().range_elem(receiver_ty) == Idx::FLOAT {
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
/// to `ori_registry` without updating this crate's method resolution.
fn _enforce_exhaustiveness(tag: ori_registry::TypeTag) {
    use ori_registry::TypeTag;
    #[expect(
        clippy::match_same_arms,
        reason = "exhaustiveness guard — each arm must be explicit"
    )]
    match tag {
        TypeTag::Int | TypeTag::Float | TypeTag::Bool | TypeTag::Char | TypeTag::Byte => {}
        TypeTag::Unit | TypeTag::Never => {}
        TypeTag::Duration | TypeTag::Size | TypeTag::Ordering => {}
        TypeTag::Str | TypeTag::Error => {}
        TypeTag::List | TypeTag::Map | TypeTag::Set | TypeTag::Range => {}
        TypeTag::Tuple | TypeTag::Option | TypeTag::Result | TypeTag::Channel => {}
        TypeTag::Function | TypeTag::Iterator | TypeTag::DoubleEndedIterator => {}
    }
}

#[cfg(test)]
mod tests;
