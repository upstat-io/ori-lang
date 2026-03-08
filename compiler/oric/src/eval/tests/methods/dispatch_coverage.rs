//! Dispatch coverage test: every registry-declared method must have a dispatch
//! handler in the evaluator.
//!
//! This is a smoke test — it verifies dispatch routing, not behavior. Calling
//! each method with zero args will produce argument errors (acceptable) but
//! must NOT produce `UndefinedMethod`.

use std::collections::{BTreeMap, BTreeSet};

use ori_eval::{RangeValue, Value};
use ori_patterns::EvalErrorKind;
use ori_registry::{TypeTag, BUILTIN_TYPES};

/// Methods declared in the registry but not yet implemented in eval dispatch.
///
/// This list must shrink monotonically. Adding entries requires justification.
/// Removing entries (= implementing methods) is always welcome.
///
/// **Cross-reference:** `TYPECK_METHODS_NOT_IN_IR` in `consistency.rs` covers
/// the narrower registry-vs-IR gap. Both allowlists will be eliminated by
/// `plans/type_strategy_registry/` Section 13.
///
/// Type names use **registry convention** (`PascalCase` for composite types).
const METHODS_NOT_YET_IN_EVAL: &[(&str, &str)] = &[
    // Channel — no Value representation yet
    ("Channel", "close"),
    ("Channel", "is_closed"),
    ("Channel", "is_empty"),
    ("Channel", "len"),
    ("Channel", "receive"),
    ("Channel", "recv"),
    ("Channel", "send"),
    ("Channel", "try_receive"),
    ("Channel", "try_recv"),
    // Duration — factory and conversion methods
    ("Duration", "abs"),
    ("Duration", "as_micros"),
    ("Duration", "as_millis"),
    ("Duration", "as_nanos"),
    ("Duration", "as_seconds"),
    ("Duration", "format"),
    ("Duration", "from_hours"),
    ("Duration", "from_micros"),
    ("Duration", "from_microseconds"),
    ("Duration", "from_millis"),
    ("Duration", "from_milliseconds"),
    ("Duration", "from_minutes"),
    ("Duration", "from_nanos"),
    ("Duration", "from_nanoseconds"),
    ("Duration", "from_seconds"),
    ("Duration", "is_negative"),
    ("Duration", "is_positive"),
    ("Duration", "is_zero"),
    ("Duration", "to_micros"),
    ("Duration", "to_millis"),
    ("Duration", "to_nanos"),
    ("Duration", "to_seconds"),
    ("Duration", "zero"),
    // List — methods recognized by typeck but not in eval direct dispatch
    ("List", "append"),
    ("List", "chunk"),
    ("List", "count"),
    ("List", "enumerate"),
    ("List", "flat_map"),
    ("List", "flatten"),
    ("List", "for_each"),
    ("List", "get"),
    ("List", "group_by"),
    ("List", "max"),
    ("List", "max_by"),
    ("List", "min"),
    ("List", "min_by"),
    ("List", "partition"),
    ("List", "prepend"),
    ("List", "product"),
    ("List", "reduce"),
    ("List", "skip_while"),
    ("List", "sort_by"),
    ("List", "sorted"),
    ("List", "sum"),
    ("List", "take_while"),
    ("List", "unique"),
    ("List", "window"),
    ("List", "zip"),
    // Map — methods not in eval
    ("Map", "merge"),
    ("Map", "update"),
    // Option — higher-order methods not in eval
    ("Option", "and_then"),
    ("Option", "expect"),
    ("Option", "filter"),
    ("Option", "flat_map"),
    ("Option", "map"),
    ("Option", "or"),
    ("Option", "or_else"),
    // Range — methods not in eval direct dispatch
    ("Range", "count"),
    ("Range", "is_empty"),
    ("Range", "step_by"),
    ("Range", "to_list"),
    // Result — methods not in eval
    ("Result", "and_then"),
    ("Result", "err"),
    ("Result", "expect"),
    ("Result", "expect_err"),
    ("Result", "map"),
    ("Result", "map_err"),
    ("Result", "ok"),
    ("Result", "or_else"),
    // Size — factory and conversion methods
    ("Size", "as_bytes"),
    ("Size", "format"),
    ("Size", "from_bytes"),
    ("Size", "from_gb"),
    ("Size", "from_gigabytes"),
    ("Size", "from_kb"),
    ("Size", "from_kilobytes"),
    ("Size", "from_mb"),
    ("Size", "from_megabytes"),
    ("Size", "from_tb"),
    ("Size", "from_terabytes"),
    ("Size", "is_zero"),
    ("Size", "to_bytes"),
    ("Size", "to_gb"),
    ("Size", "to_kb"),
    ("Size", "to_mb"),
    ("Size", "to_tb"),
    ("Size", "zero"),
    // Tuple — no gaps
    // bool
    ("bool", "to_int"),
    // byte — arithmetic operators and predicates
    ("byte", "add"),
    ("byte", "bit_and"),
    ("byte", "bit_not"),
    ("byte", "bit_or"),
    ("byte", "bit_xor"),
    ("byte", "div"),
    ("byte", "is_ascii"),
    ("byte", "is_ascii_alpha"),
    ("byte", "is_ascii_digit"),
    ("byte", "is_ascii_whitespace"),
    ("byte", "mul"),
    ("byte", "rem"),
    ("byte", "shl"),
    ("byte", "shr"),
    ("byte", "sub"),
    ("byte", "to_char"),
    ("byte", "to_int"),
    // char — predicates and conversions
    ("char", "is_alpha"),
    ("char", "is_ascii"),
    ("char", "is_digit"),
    ("char", "is_lowercase"),
    ("char", "is_uppercase"),
    ("char", "is_whitespace"),
    ("char", "to_byte"),
    ("char", "to_int"),
    ("char", "to_lowercase"),
    ("char", "to_uppercase"),
    // float — methods not in eval direct dispatch
    ("float", "abs"),
    ("float", "acos"),
    ("float", "asin"),
    ("float", "atan"),
    ("float", "atan2"),
    ("float", "cbrt"),
    ("float", "ceil"),
    ("float", "clamp"),
    ("float", "cos"),
    ("float", "exp"),
    ("float", "floor"),
    ("float", "is_finite"),
    ("float", "is_infinite"),
    ("float", "is_nan"),
    ("float", "is_negative"),
    ("float", "is_normal"),
    ("float", "is_positive"),
    ("float", "is_zero"),
    ("float", "ln"),
    ("float", "log10"),
    ("float", "log2"),
    ("float", "max"),
    ("float", "min"),
    ("float", "pow"),
    ("float", "rem"),
    ("float", "round"),
    ("float", "signum"),
    ("float", "sin"),
    ("float", "sqrt"),
    ("float", "tan"),
    ("float", "to_int"),
    ("float", "trunc"),
    // int — methods not in eval direct dispatch
    ("int", "abs"),
    ("int", "byte"),
    ("int", "clamp"),
    ("int", "f"),
    ("int", "is_even"),
    ("int", "is_negative"),
    ("int", "is_odd"),
    ("int", "is_positive"),
    ("int", "is_zero"),
    ("int", "max"),
    ("int", "min"),
    ("int", "pow"),
    ("int", "signum"),
    ("int", "to_byte"),
    ("int", "to_float"),
    // str — methods not in eval direct dispatch
    ("str", "as_bytes"),
    ("str", "byte_len"),
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "from_utf8"),
    ("str", "from_utf8_unchecked"),
    ("str", "index_of"),
    ("str", "last_index_of"),
    ("str", "lines"),
    ("str", "pad_end"),
    ("str", "pad_start"),
    ("str", "parse_float"),
    ("str", "parse_int"),
    ("str", "to_bytes"),
    ("str", "to_float"),
    ("str", "to_int"),
    ("str", "trim_end"),
    ("str", "trim_start"),
];

/// Methods dispatched by `CollectionMethodResolver` for non-Iterator types.
///
/// These produce `UndefinedMethod` via `dispatch_builtin_method_str` because
/// it only exercises the `BuiltinMethodResolver` path (priority 2), not the
/// `CollectionMethodResolver` (priority 1).
const COLLECTION_RESOLVER_METHODS: &[(&str, &str)] = &[
    // List: map, filter, fold, find, any, all, join
    ("List", "all"),
    ("List", "any"),
    ("List", "filter"),
    ("List", "find"),
    ("List", "fold"),
    ("List", "join"),
    ("List", "map"),
    // Map: map, filter
    ("Map", "filter"),
    ("Map", "map"),
    // Ordering: then_with
    ("Ordering", "then_with"),
    // Range: map, filter, fold, find, any, all, collect
    ("Range", "all"),
    ("Range", "any"),
    ("Range", "collect"),
    ("Range", "filter"),
    ("Range", "find"),
    ("Range", "fold"),
    ("Range", "map"),
];

/// Construct the simplest possible `Value` for a `TypeTag`.
///
/// Returns `None` for types that have no `Value` representation or are
/// dispatched by other resolvers (Iterator, DEI, Channel).
fn minimal_value_for(tag: TypeTag) -> Option<Value> {
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
/// `UndefinedMethod` is not (unless the method is in a known allowlist).
#[test]
fn every_registry_method_has_eval_dispatch_handler() {
    let interner = super::test_interner();

    let not_yet: BTreeSet<(&str, &str)> = METHODS_NOT_YET_IN_EVAL.iter().copied().collect();
    let collection: BTreeSet<(&str, &str)> = COLLECTION_RESOLVER_METHODS.iter().copied().collect();

    let mut unexpected_missing = Vec::new();
    let mut unexpectedly_present = Vec::new();

    for type_def in BUILTIN_TYPES {
        let receiver = minimal_value_for(type_def.tag);
        let Some(receiver) = receiver else {
            // Skip types without Value representation (Channel, Iterator, etc.)
            continue;
        };

        for method in type_def.methods {
            let pair = (type_def.name, method.name);

            // Skip methods we know aren't yet implemented or are dispatched elsewhere
            let is_allowlisted = not_yet.contains(&pair) || collection.contains(&pair);

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

            if is_undefined && !is_allowlisted {
                unexpected_missing.push(format!("{}.{}", type_def.name, method.name));
            } else if !is_undefined && is_allowlisted {
                unexpectedly_present.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        unexpected_missing.is_empty(),
        "Registry declares methods with no eval dispatch handler \
         (add handler or add to METHODS_NOT_YET_IN_EVAL): {unexpected_missing:#?}"
    );

    assert!(
        unexpectedly_present.is_empty(),
        "Methods in allowlists now have dispatch handlers — \
         remove from METHODS_NOT_YET_IN_EVAL or COLLECTION_RESOLVER_METHODS: \
         {unexpectedly_present:#?}"
    );
}

/// Ensure the not-yet-implemented allowlist does not grow.
///
/// If you are adding a method to the allowlist, this test will fail.
/// Implement the method instead, or get explicit approval to raise
/// the ceiling.
#[test]
fn methods_not_yet_in_eval_does_not_grow() {
    assert!(
        METHODS_NOT_YET_IN_EVAL.len() <= 189,
        "METHODS_NOT_YET_IN_EVAL grew to {} entries (ceiling: 189). \
         Implement the method or get approval to raise the ceiling.",
        METHODS_NOT_YET_IN_EVAL.len()
    );
}
