//! Iterator helpers and `TypeTag` exhaustiveness guards.
//!
//! Extracted from `builtins/mod.rs` to keep it under the 500-line limit.

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

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating the LLVM backend's builtin codegen.
// Compile-time exhaustiveness guard (Roc pattern).
#[allow(
    dead_code,
    unreachable_code,
    reason = "compile-time exhaustiveness guard — never called"
)]
fn _enforce_type_tag_exhaustiveness(tag: ori_registry::TypeTag) {
    match tag {
        // All 23 TypeTag variants — handled in primitives.rs, collections/,
        // compound_type_impls.rs, option_result.rs, iterator.rs
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
