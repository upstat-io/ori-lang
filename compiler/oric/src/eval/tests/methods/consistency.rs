//! Tests for consistency between evaluator and type checker builtin methods.

use std::collections::BTreeSet;

use ori_eval::EVAL_BUILTIN_METHODS;
use ori_typeck::infer::builtin_methods::TYPECK_BUILTIN_METHODS;

/// Methods implemented in the evaluator that intentionally lack type
/// checker handlers (e.g., range has no typeck handler yet).
const KNOWN_EVAL_ONLY: &[(&str, &str)] = &[("range", "contains"), ("range", "len")];

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
