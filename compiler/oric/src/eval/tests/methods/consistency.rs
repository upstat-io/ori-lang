//! Tests for consistency between evaluator and type checker builtin methods.

use std::collections::BTreeSet;

use ori_eval::EVAL_BUILTIN_METHODS;
use ori_typeck::infer::builtin_methods::TYPECK_BUILTIN_METHODS;

/// Methods implemented in the evaluator that intentionally lack type
/// checker handlers.
///
/// Operator trait methods (add, sub, mul, etc.) are handled through the trait
/// registry in the type checker, not through builtin method handlers, so they
/// appear here as eval-only. Range methods don't have typeck handlers yet.
const KNOWN_EVAL_ONLY: &[(&str, &str)] = &[
    // Operator trait methods - handled via trait lookup in typeck
    ("bool", "not"),
    // Trait methods for primitives (Clone, Printable, Debug, Hashable, Eq)
    // These are handled via trait lookup in typeck, implemented directly in eval
    ("bool", "clone"),
    ("bool", "debug"),
    ("bool", "equals"),
    ("bool", "hash"),
    ("bool", "to_str"),
    ("byte", "clone"),
    ("byte", "debug"),
    ("byte", "equals"),
    ("byte", "hash"),
    ("byte", "to_str"),
    ("char", "clone"),
    ("char", "debug"),
    ("char", "equals"),
    ("char", "hash"),
    ("char", "to_str"),
    ("duration", "add"),
    ("duration", "div"),
    ("duration", "mul"),
    ("duration", "neg"),
    ("duration", "rem"),
    ("duration", "sub"),
    ("float", "add"),
    ("float", "clone"),
    ("float", "debug"),
    ("float", "div"),
    ("float", "equals"),
    ("float", "hash"),
    ("float", "mul"),
    ("float", "neg"),
    ("float", "sub"),
    ("float", "to_str"),
    ("int", "add"),
    ("int", "bit_and"),
    ("int", "bit_not"),
    ("int", "bit_or"),
    ("int", "bit_xor"),
    ("int", "clone"),
    ("int", "debug"),
    ("int", "div"),
    ("int", "equals"),
    ("int", "floor_div"),
    ("int", "hash"),
    ("int", "mul"),
    ("int", "neg"),
    ("int", "rem"),
    ("int", "shl"),
    ("int", "shr"),
    ("int", "sub"),
    ("int", "to_str"),
    ("list", "add"),
    ("list", "clone"),
    ("list", "debug"),
    ("range", "contains"),
    ("range", "len"),
    ("size", "add"),
    ("size", "div"),
    ("size", "mul"),
    ("size", "rem"),
    ("size", "sub"),
    ("str", "add"),
    ("str", "clone"),
    ("str", "debug"),
    ("str", "equals"),
    ("str", "hash"),
    ("str", "to_str"),
];

/// Every method the evaluator implements must have a type signature in the
/// type checker (unless it's in the known-eval-only list). This test catches
/// drift when methods are added to one crate but not the other.
#[test]
fn eval_methods_are_subset_of_typeck() {
    let typeck: BTreeSet<_> = TYPECK_BUILTIN_METHODS.iter().collect();
    let known: BTreeSet<_> = KNOWN_EVAL_ONLY.iter().collect();

    let mut missing = Vec::new();
    for entry in EVAL_BUILTIN_METHODS {
        if !typeck.contains(entry) && !known.contains(entry) {
            missing.push(entry);
        }
    }

    assert!(
        missing.is_empty(),
        "Evaluator has methods not present in type checker: {missing:?}\n\
         Add type signatures in ori_typeck/src/infer/builtin_methods/ \
         or add to KNOWN_EVAL_ONLY"
    );
}

/// Both constant lists must be sorted (type, then method) for reliable
/// comparison and easy diffing.
#[test]
fn method_lists_are_sorted() {
    for window in EVAL_BUILTIN_METHODS.windows(2) {
        assert!(
            window[0] <= window[1],
            "EVAL_BUILTIN_METHODS not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
    for window in TYPECK_BUILTIN_METHODS.windows(2) {
        assert!(
            window[0] <= window[1],
            "TYPECK_BUILTIN_METHODS not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
}
