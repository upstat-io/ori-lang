//! Static dispatch capability check for enforcement testing.
//!
//! Provides [`can_dispatch_builtin`] which determines whether the evaluator
//! can handle a given (`TypeTag`, method name) pair without actually executing
//! the method. This enables cross-phase enforcement tests to verify every
//! registry-declared method has an eval dispatch handler.

use std::collections::BTreeMap;

use ori_ir::StringInterner;
use ori_patterns::{ControlAction, EvalErrorKind, EvalResult, RangeValue, Value};
use ori_registry::TypeTag;

use super::dispatch_builtin_method_str;

/// Check if the evaluator can dispatch a builtin method for the given type.
///
/// Queries both resolver paths:
/// - [`CollectionMethodResolver`](crate::interpreter::resolvers::collection::CollectionMethodResolver)
///   (priority 1): handles iterator, collection, and closure-taking methods
/// - [`BuiltinMethodResolver`](crate::interpreter::resolvers::builtin::BuiltinMethodResolver)
///   (priority 2): handles primitive type methods
///
/// Returns `true` if a handler exists (even if it would fail with wrong args).
/// Returns `false` if the method would produce `UndefinedMethod`.
///
/// # Usage
///
/// This is an enforcement-testing API. It verifies dispatch routing, not
/// behavioral correctness. Used by cross-phase enforcement tests in `oric`.
///
/// ```ignore
/// for type_def in ori_registry::BUILTIN_TYPES {
///     for method in type_def.methods {
///         assert!(can_dispatch_builtin(type_def.tag, method.name, &interner));
///     }
/// }
/// ```
pub fn can_dispatch_builtin(tag: TypeTag, method: &str, interner: &StringInterner) -> bool {
    // CollectionMethodResolver path — static method set check.
    // These methods are dispatched by CollectionMethodResolver (priority 1),
    // which `dispatch_builtin_method_str` does NOT exercise.
    if is_collection_dispatched(tag, method) {
        return true;
    }

    // BuiltinMethodResolver path — try dispatch with minimal value.
    // Returns true if the handler exists (any result except UndefinedMethod).
    let Some(receiver) = minimal_value_for_tag(tag) else {
        return false;
    };

    let result = dispatch_builtin_method_str(receiver, method, vec![], interner);
    !is_undefined_method_error(&result)
}

/// Check if a method is handled by the `CollectionMethodResolver`.
///
/// The `CollectionMethodResolver` dispatches methods that take function arguments
/// (closures) and need evaluator access. These methods are NOT reachable via
/// `dispatch_builtin_method_str` because that only exercises the
/// `BuiltinMethodResolver` (priority 2).
///
/// Must stay in sync with `CollectionMethodResolver::resolve()` in
/// `interpreter/resolvers/collection/mod.rs`.
fn is_collection_dispatched(tag: TypeTag, method: &str) -> bool {
    match tag {
        // Iterator gets ALL iterator methods from resolve_iterator_method()
        TypeTag::Iterator | TypeTag::DoubleEndedIterator => matches!(
            method,
            "next"
                | "map"
                | "filter"
                | "take"
                | "skip"
                | "fold"
                | "count"
                | "find"
                | "any"
                | "all"
                | "for_each"
                | "collect"
                | "enumerate"
                | "zip"
                | "chain"
                | "flatten"
                | "flat_map"
                | "cycle"
                | "next_back"
                | "rev"
                | "last"
                | "rfind"
                | "rfold"
                | "join"
                | "collect_set"
        ),
        // List gets join directly + iterable methods (map, filter, fold, find, any, all)
        TypeTag::List => matches!(
            method,
            "map" | "filter" | "fold" | "find" | "any" | "all" | "join"
        ),
        // Range gets collect directly + iterable methods
        TypeTag::Range => matches!(
            method,
            "map" | "filter" | "fold" | "find" | "any" | "all" | "collect"
        ),
        // Map gets map/filter via MapEntries/FilterEntries
        TypeTag::Map => matches!(method, "map" | "filter"),
        // Ordering gets then_with
        TypeTag::Ordering => method == "then_with",
        _ => false,
    }
}

/// Construct the simplest possible `Value` for a `TypeTag`.
///
/// Returns `None` for types without a direct `Value` representation
/// (Unit, Never, Iterator, DEI, Channel, Function). Iterator and DEI
/// are handled entirely by `CollectionMethodResolver` and checked via
/// `is_collection_dispatched`.
fn minimal_value_for_tag(tag: TypeTag) -> Option<Value> {
    match tag {
        TypeTag::Int => Some(Value::int(0)),
        TypeTag::Float => Some(Value::Float(0.0)),
        TypeTag::Bool => Some(Value::Bool(false)),
        TypeTag::Str => Some(Value::string("")),
        TypeTag::Char => Some(Value::Char(' ')),
        TypeTag::Byte => Some(Value::Byte(0)),
        TypeTag::Duration => Some(Value::Duration(0)),
        TypeTag::Size => Some(Value::Size(0)),
        TypeTag::Ordering => Some(Value::ordering_equal()),
        TypeTag::Option => Some(Value::None),
        TypeTag::Result => Some(Value::ok(Value::Void)),
        TypeTag::List => Some(Value::list(vec![])),
        TypeTag::Map => Some(Value::map(BTreeMap::default())),
        TypeTag::Set => Some(Value::set(BTreeMap::default())),
        TypeTag::Range => Some(Value::Range(RangeValue::exclusive(0, 0))),
        TypeTag::Tuple => Some(Value::tuple(vec![])),
        TypeTag::Error => Some(Value::error("test")),
        // No Value representation or dispatched entirely by CollectionMethodResolver
        TypeTag::Unit
        | TypeTag::Never
        | TypeTag::Iterator
        | TypeTag::DoubleEndedIterator
        | TypeTag::Channel
        | TypeTag::Function => None,
    }
}

/// Check if an `EvalResult` represents an `UndefinedMethod` error.
fn is_undefined_method_error(result: &EvalResult) -> bool {
    match result {
        Err(action) => {
            if let ControlAction::Error(e) = action {
                matches!(e.kind, EvalErrorKind::UndefinedMethod { .. })
            } else {
                false
            }
        }
        Ok(_) => false,
    }
}

#[cfg(test)]
mod tests;
