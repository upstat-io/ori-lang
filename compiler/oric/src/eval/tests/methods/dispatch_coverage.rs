//! Dispatch coverage test: every registry-declared method must have a dispatch
//! handler in the evaluator.
//!
//! This is a smoke test — it verifies dispatch routing, not behavior. Calling
//! each method with zero args will produce argument errors (acceptable) but
//! must NOT produce `UndefinedMethod`.

use std::collections::BTreeMap;

use ori_eval::{RangeValue, Value};
use ori_patterns::EvalErrorKind;
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
        TypeTag::Unit
        | TypeTag::Never
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
/// routes to a handler. Argument errors (wrong count, wrong type) are acceptable;
/// `UndefinedMethod` is not.
///
/// Types without a `Value` representation (Channel, Iterator, DEI) are skipped
/// — their methods are dispatched by specialized resolvers, not the builtin path.
#[test]
fn every_registry_method_has_eval_dispatch_handler() {
    let interner = super::test_interner();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        let Some(receiver) = minimal_value_for(type_def.tag) else {
            // Skip types without Value representation (Channel, Iterator, etc.)
            continue;
        };

        for method in type_def.methods {
            let result = ori_eval::dispatch_builtin_method_str(
                receiver.clone(),
                method.name,
                vec![],
                &interner,
            );

            let is_undefined = match &result {
                Err(action) => {
                    if let ori_patterns::ControlAction::Error(e) = action {
                        matches!(e.kind, EvalErrorKind::UndefinedMethod { .. })
                    } else {
                        false
                    }
                }
                Ok(_) => false,
            };

            if is_undefined {
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
