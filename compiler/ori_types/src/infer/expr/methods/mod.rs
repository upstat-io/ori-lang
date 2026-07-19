//! Builtin method resolution for primitives and collections.
//!
//! [`ori_registry`] supplies method existence and return tags; computed-return
//! handlers materialize context-dependent types. User-defined receivers bypass
//! this path except for the registered error type's backend-supported methods.

mod computed_returns;

use ori_registry::ReturnTag;

use crate::infer::InferEngine;
use crate::{Idx, Tag, TypeKind};

use super::registry_bridge;

/// Check if a Range method requires iteration (and is thus invalid on `Range<float>`).
///
/// Dispatch rejects every method on `Range<float>`. This classification
/// distinguishes iteration diagnostics from the generic missing-method error.
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
    ) || method
        .params
        .iter()
        .any(|param| param.ty == ReturnTag::Fresh)
}

/// Resolve a built-in method call on a known type tag.
///
/// Returns `Some(return_type)` if the method is a known built-in,
/// `None` if the method is not recognized for this type tag.
///
/// Uses the `ori_registry` for method lookup and return type resolution.
/// Named/Applied types (user-defined) bypass the registry, except the
/// registered error struct's backend-required methods, which route through
/// the registry's allow-list first.
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    // INVARIANT: only registry-declared backend methods bypass named-type lookup.
    if matches!(tag, Tag::Named | Tag::Applied) {
        if engine.pool().is_error_struct_receiver(receiver_ty) {
            if let Some(method_def) =
                ori_registry::find_method(ori_registry::TypeTag::Error, method_name)
            {
                if method_def.backend_required {
                    // Effective Tag::Error into the computed-return call —
                    // NEVER the receiver's raw Tag::Named — exactly as a
                    // genuine Tag::Error receiver resolves.
                    let return_ty = if method_def.returns == ReturnTag::Fresh {
                        computed_returns::resolve_computed_return(
                            engine,
                            receiver_ty,
                            Tag::Error,
                            method_name,
                        )
                    } else {
                        registry_bridge::return_tag_to_idx(engine, receiver_ty, method_def.returns)
                    };
                    return Some(return_ty);
                }
            }
        }
        // Supports newtype `.unwrap()`/`.inner()`/`.value()` and common trait
        // methods; also the error-struct fall-through on registry miss or a
        // non-allow-listed method name (e.g. `.message()`, still poisons).
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
/// Exhaustive matching forces method-resolution updates for every new
/// `ori_registry::TypeTag` variant.
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
