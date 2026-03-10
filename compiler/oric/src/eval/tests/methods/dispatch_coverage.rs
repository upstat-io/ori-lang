//! Eval dispatch gap tracking: methods declared in the registry but not yet
//! implemented in the evaluator.
//!
//! `METHODS_NOT_YET_IN_EVAL` is a **shrinking allowlist**. Adding entries
//! requires justification. Removing entries (= implementing methods) is always
//! welcome. The ceiling test enforces monotonic decrease.
//!
//! Cross-phase enforcement (verifying every registry method has a handler)
//! lives in `consistency.rs`. This file only tracks the known eval gaps.

use ori_registry::BUILTIN_TYPES;

/// Methods declared in the registry but not yet implemented in eval dispatch.
///
/// This list must shrink monotonically. Adding entries requires justification.
/// Removing entries (= implementing methods) is always welcome.
///
/// Type names use **registry convention** (`PascalCase` for composite types).
///
/// Referenced by `consistency.rs::every_registry_method_has_eval_handler`.
pub(super) const METHODS_NOT_YET_IN_EVAL: &[(&str, &str)] = &[
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

/// Total registry method count for sanity checking.
///
/// Ensures the enforcement test in consistency.rs is actually iterating
/// over a non-trivial number of methods.
#[test]
fn registry_has_expected_method_count() {
    let total: usize = BUILTIN_TYPES.iter().map(|td| td.methods.len()).sum();

    // Registry should have 350+ methods across all types.
    // If this drops significantly, the registry was likely corrupted.
    assert!(
        total >= 350,
        "Registry has only {total} methods — expected 350+. \
         Was a TypeDef accidentally emptied?"
    );
}
