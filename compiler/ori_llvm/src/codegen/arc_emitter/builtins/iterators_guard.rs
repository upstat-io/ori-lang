//! Iterator helpers and `TypeTag` exhaustiveness guards.

use super::TypeInfo;

/// Known iterator method names for auto-iter promotion.
///
/// Driven by the registry — queries `ori_registry::has_method()` for both
/// `Iterator` and `DoubleEndedIterator` type tags. `__iter_next` is NOT in
/// the registry (compiler-internal protocol) and is handled separately by
/// `try_emit_protocol`.
pub(super) fn is_iterator_method(name: &str) -> bool {
    ori_registry::has_method(ori_registry::TypeTag::Iterator, name)
        || ori_registry::has_method(ori_registry::TypeTag::DoubleEndedIterator, name)
}

/// Extract the element type from a collection's `TypeInfo` for iterator
/// dispatch after auto-iter promotion.
pub(super) fn auto_iter_element_type(type_info: &TypeInfo) -> ori_types::Idx {
    match type_info {
        TypeInfo::List { element } | TypeInfo::Set { element } => *element,
        TypeInfo::Map { key, .. } => *key,
        _ => ori_types::Idx::INT, // fallback for str (char→int), range (int)
    }
}

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating this crate's builtin codegen.
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
