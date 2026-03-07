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
mod resolve_by_type;

use ori_registry::ReturnTag;

use crate::infer::InferEngine;
use crate::{Idx, Tag};

use super::registry_bridge;
use resolve_by_type::resolve_named_type_method;

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
    // Named/Applied types: user-defined, not in registry
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
