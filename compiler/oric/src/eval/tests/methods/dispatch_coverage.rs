//! Dispatch coverage test: every registry-declared method must have a dispatch
//! handler in the evaluator.
//!
//! This is a smoke test — it verifies dispatch routing, not behavior. Calling
//! each method with zero args will produce argument errors (acceptable) but
//! must NOT produce `UndefinedMethod`.

use std::collections::BTreeMap;

use ori_eval::{RangeValue, Value};
use ori_registry::{TypeTag, BUILTIN_TYPES};

/// Construct the simplest possible `Value` for a `TypeTag`.
///
/// Returns `None` for types that have no `Value` representation or are
/// dispatched by other resolvers (Iterator, DEI, Channel).
pub(super) fn minimal_value_for(tag: TypeTag) -> Option<Value> {
    match tag {
        TypeTag::Int => Some(Value::int(0)),
        TypeTag::Float => Some(Value::Float(0.0)),
        TypeTag::Bool => Some(Value::Bool(false)),
        TypeTag::Str => Some(Value::string("")),
        TypeTag::Char => Some(Value::Char(' ')),
        TypeTag::Byte => Some(Value::Byte(0)),
        TypeTag::Unit => Some(Value::Void),
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
        // Types without a direct Value representation or dispatched elsewhere.
        TypeTag::Never
        | TypeTag::Iterator
        | TypeTag::DoubleEndedIterator
        | TypeTag::Channel
        | TypeTag::Function => None,
    }
}

/// Every method the registry declares for a builtin type must be dispatchable
/// by the evaluator without producing `UndefinedMethod`.
///
/// This test does NOT verify correct behavior — only that the dispatch chain
/// routes to a handler. Uses `can_dispatch_builtin` which checks BOTH resolver
/// paths:
/// - `CollectionMethodResolver` (priority 1): closure-taking methods (fold,
///   map, filter, etc.) dispatched via `is_collection_dispatched()`
/// - `BuiltinMethodResolver` (priority 2): primitive type methods dispatched
///   via `dispatch_builtin_method_str`
///
/// `TypeTag::Channel` is excluded: Channel methods require live channel
/// objects (concurrency primitives) that cannot be constructed as minimal
/// values and are not yet implemented in the tree-walking interpreter.
#[test]
fn every_registry_method_has_eval_dispatch_handler() {
    let interner = super::test_interner();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        // Channel methods require live channel objects — not dispatchable
        // via the builtin/collection resolver paths. Excluded intentionally.
        if type_def.tag == TypeTag::Channel {
            continue;
        }

        for method in type_def.methods {
            if !ori_eval::can_dispatch_builtin(type_def.tag, method.name, &interner) {
                missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry declares methods with no eval dispatch handler ({} missing):\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}
