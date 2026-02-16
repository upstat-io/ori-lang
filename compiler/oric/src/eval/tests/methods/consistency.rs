//! Tests for consistency between evaluator builtin methods, type checker
//! builtin methods, and the `ori_ir` builtin method registry.

use std::collections::BTreeSet;

use ori_eval::{EVAL_BUILTIN_METHODS, ITERATOR_METHOD_NAMES};
use ori_ir::builtin_methods::BUILTIN_METHODS;
use ori_types::TYPECK_BUILTIN_METHODS;

/// Collection types that have eval/typeck methods but are not yet in the
/// `ori_ir` builtin method registry. These are tracked as a gap to fix.
/// Names match `EVAL_BUILTIN_METHODS`/`TYPECK_BUILTIN_METHODS` convention.
const COLLECTION_TYPES: &[&str] = &[
    "Channel", "Iterator", "Option", "Result", "Set", "list", "map", "range", "tuple",
];

/// IR registry methods that are implemented in the evaluator through method
/// resolvers (`UserRegistryResolver`, `CollectionMethodResolver`, etc.) rather
/// than through direct dispatch in `dispatch_builtin_method`.
///
/// These are NOT missing from eval — they work at runtime. They're just
/// dispatched through a different mechanism than `EVAL_BUILTIN_METHODS`.
const IR_METHODS_DISPATCHED_VIA_RESOLVERS: &[(&str, &str)] = &[
    // float — numeric methods dispatched via method resolvers
    ("float", "abs"),
    ("float", "ceil"),
    ("float", "floor"),
    ("float", "max"),
    ("float", "min"),
    ("float", "round"),
    ("float", "sqrt"),
    // int — numeric methods dispatched via method resolvers
    ("int", "abs"),
    ("int", "max"),
    ("int", "min"),
];

/// Eval methods for primitive types that are not yet in the IR builtin
/// method registry. These need to be added to `ori_ir/src/builtin_methods.rs`.
const EVAL_METHODS_NOT_IN_IR: &[(&str, &str)] = &[
    // Duration/Size operator aliases — eval accepts both short and long forms
    // (e.g., "sub" and "subtract"), but IR only registers the short form.
    ("Duration", "divide"),
    ("Duration", "multiply"),
    ("Duration", "negate"),
    ("Duration", "remainder"),
    ("Duration", "subtract"),
    ("Size", "divide"),
    ("Size", "multiply"),
    ("Size", "remainder"),
    ("Size", "subtract"),
    // Debug trait — implemented in eval, not yet in IR for all types
    ("bool", "debug"),
    ("byte", "debug"),
    ("char", "debug"),
    ("float", "debug"),
    ("int", "debug"),
    ("str", "debug"),
    // Printable for str (str.to_str returns itself)
    ("str", "to_str"),
    // Iterable — iter() returns Iterator<T>, not expressible in current IR ReturnSpec
    ("str", "iter"),
];

/// Build the set of `(type_name, method_name)` from the IR registry.
fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}

/// Every method in the IR builtin registry should be implemented in the
/// evaluator (either via direct dispatch or method resolvers).
#[test]
fn ir_methods_implemented_in_eval() {
    let eval_set: BTreeSet<_> = EVAL_BUILTIN_METHODS.iter().copied().collect();
    let resolver_set: BTreeSet<_> = IR_METHODS_DISPATCHED_VIA_RESOLVERS
        .iter()
        .copied()
        .collect();
    let ir_set = ir_method_set();

    let mut missing = Vec::new();
    for &(ty, method) in &ir_set {
        if !eval_set.contains(&(ty, method)) && !resolver_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "IR registry has methods not accounted for in evaluator: {missing:?}\n\
         Either add to EVAL_BUILTIN_METHODS (direct dispatch) or \
         IR_METHODS_DISPATCHED_VIA_RESOLVERS (method resolver dispatch)"
    );
}

/// Every eval method for primitive types should be in the IR registry
/// (the single source of truth for builtin method signatures).
#[test]
fn eval_primitive_methods_in_ir() {
    let ir_set = ir_method_set();
    let known_set: BTreeSet<_> = EVAL_METHODS_NOT_IN_IR.iter().copied().collect();

    let mut missing = Vec::new();
    for &(ty, method) in EVAL_BUILTIN_METHODS {
        // Skip collection types (not yet in IR registry)
        if COLLECTION_TYPES.contains(&ty) {
            continue;
        }
        if !ir_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Evaluator has primitive methods not in IR registry: {missing:?}\n\
         Add method definitions in ori_ir/src/builtin_methods.rs or \
         add to EVAL_METHODS_NOT_IN_IR"
    );
}

/// The eval method list must be sorted (type, then method) for reliable
/// comparison and easy diffing.
#[test]
fn eval_method_list_is_sorted() {
    for window in EVAL_BUILTIN_METHODS.windows(2) {
        assert!(
            window[0] <= window[1],
            "EVAL_BUILTIN_METHODS not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
}

// ── Typeck consistency tests ──────────────────────────────────────────

/// Eval methods that typeck doesn't recognize because operators are handled
/// separately by operator inference, not method resolution. Also includes
/// trait methods not yet in the typeck string-match dispatch.
const EVAL_METHODS_NOT_IN_TYPECK: &[(&str, &str)] = &[
    // Duration/Size operators — eval dispatches via operator trait methods,
    // typeck handles operators through operator inference
    ("Duration", "add"),
    ("Duration", "div"),
    ("Duration", "divide"),
    ("Duration", "mul"),
    ("Duration", "multiply"),
    ("Duration", "neg"),
    ("Duration", "negate"),
    ("Duration", "rem"),
    ("Duration", "remainder"),
    ("Duration", "sub"),
    ("Duration", "subtract"),
    // Option — eval has trait methods typeck resolves via traits
    ("Option", "compare"),
    ("Option", "ok_or"),
    // Ordering — no extras needed (all in typeck)
    // Result — eval has trait methods typeck resolves via traits
    ("Result", "compare"),
    // Size — operators and accessor methods
    ("Size", "add"),
    ("Size", "bytes"),
    ("Size", "div"),
    ("Size", "divide"),
    ("Size", "gigabytes"),
    ("Size", "kilobytes"),
    ("Size", "megabytes"),
    ("Size", "mul"),
    ("Size", "multiply"),
    ("Size", "rem"),
    ("Size", "remainder"),
    ("Size", "sub"),
    ("Size", "subtract"),
    ("Size", "terabytes"),
    // Operator methods — typeck resolves operators via operator inference,
    // not through resolve_builtin_method()
    ("bool", "debug"),
    ("bool", "not"),
    ("byte", "debug"),
    ("char", "debug"),
    ("float", "add"),
    ("float", "debug"),
    ("float", "div"),
    ("float", "mul"),
    ("float", "neg"),
    ("float", "sub"),
    ("int", "add"),
    ("int", "bit_and"),
    ("int", "bit_not"),
    ("int", "bit_or"),
    ("int", "bit_xor"),
    ("int", "debug"),
    ("int", "div"),
    ("int", "floor_div"),
    ("int", "mul"),
    ("int", "neg"),
    ("int", "rem"),
    ("int", "shl"),
    ("int", "shr"),
    ("int", "sub"),
    ("list", "add"),
    ("list", "compare"),
    ("list", "concat"),
    ("list", "debug"),
    ("str", "add"),
    ("str", "concat"),
    ("str", "debug"),
    ("str", "to_str"),
];

/// Typeck methods for primitive types not yet in the IR registry.
/// These need to be added to `ori_ir/src/builtin_methods/mod.rs`.
const TYPECK_METHODS_NOT_IN_IR: &[(&str, &str)] = &[
    // Duration — conversion aliases and factory methods
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
    // Ordering — debug trait
    ("Ordering", "debug"),
    // Size — conversion aliases and factory methods
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
    ("Size", "to_str"),
    ("Size", "to_tb"),
    ("Size", "zero"),
    // bool — typeck has conversions that IR doesn't list yet
    ("bool", "to_int"),
    // byte — typeck has conversions and predicates not in IR
    ("byte", "is_ascii"),
    ("byte", "is_ascii_alpha"),
    ("byte", "is_ascii_digit"),
    ("byte", "is_ascii_whitespace"),
    ("byte", "to_char"),
    ("byte", "to_int"),
    // char — typeck has conversions and predicates not in IR
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
    // float — typeck has many math methods not in IR
    ("float", "acos"),
    ("float", "asin"),
    ("float", "atan"),
    ("float", "atan2"),
    ("float", "cbrt"),
    ("float", "clamp"),
    ("float", "cos"),
    ("float", "exp"),
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
    ("float", "pow"),
    ("float", "signum"),
    ("float", "sin"),
    ("float", "tan"),
    ("float", "to_int"),
    ("float", "trunc"),
    // int — typeck has methods not in IR
    ("int", "clamp"),
    ("int", "is_even"),
    ("int", "is_negative"),
    ("int", "is_odd"),
    ("int", "is_positive"),
    ("int", "is_zero"),
    ("int", "pow"),
    ("int", "signum"),
    ("int", "to_byte"),
    ("int", "to_float"),
    // str — typeck has many methods not in IR
    ("str", "byte_len"),
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "index_of"),
    ("str", "iter"),
    ("str", "last_index_of"),
    ("str", "lines"),
    ("str", "pad_end"),
    ("str", "pad_start"),
    ("str", "parse_float"),
    ("str", "parse_int"),
    ("str", "repeat"),
    ("str", "replace"),
    ("str", "slice"),
    ("str", "split"),
    ("str", "substring"),
    ("str", "to_float"),
    ("str", "to_int"),
    ("str", "to_str"),
    ("str", "trim_end"),
    ("str", "trim_start"),
];

/// Typeck methods for all types (including collection types) that are NOT
/// yet implemented in the evaluator. These type-check successfully but
/// would fail at runtime with "no such method".
const TYPECK_METHODS_NOT_IN_EVAL: &[(&str, &str)] = &[
    // Channel — not in eval at all yet (no Channel value type)
    ("Channel", "close"),
    ("Channel", "is_closed"),
    ("Channel", "is_empty"),
    ("Channel", "len"),
    ("Channel", "receive"),
    ("Channel", "recv"),
    ("Channel", "send"),
    ("Channel", "try_receive"),
    ("Channel", "try_recv"),
    // Iterator — dispatched via CollectionMethodResolver, not EVAL_BUILTIN_METHODS
    ("Iterator", "all"),
    ("Iterator", "any"),
    ("Iterator", "chain"),
    ("Iterator", "collect"),
    ("Iterator", "count"),
    ("Iterator", "cycle"),
    ("Iterator", "enumerate"),
    ("Iterator", "filter"),
    ("Iterator", "find"),
    ("Iterator", "flat_map"),
    ("Iterator", "flatten"),
    ("Iterator", "fold"),
    ("Iterator", "for_each"),
    ("Iterator", "last"),
    ("Iterator", "map"),
    ("Iterator", "next"),
    ("Iterator", "next_back"),
    ("Iterator", "rev"),
    ("Iterator", "rfind"),
    ("Iterator", "rfold"),
    ("Iterator", "skip"),
    ("Iterator", "take"),
    ("Iterator", "zip"),
    // Duration — factory and conversion methods not in eval
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
    // Ordering — typeck has debug, to_str
    ("Ordering", "to_str"),
    // Option — higher-order methods not in eval
    ("Option", "and_then"),
    ("Option", "expect"),
    ("Option", "filter"),
    ("Option", "flat_map"),
    ("Option", "map"),
    ("Option", "or"),
    ("Option", "or_else"),
    // Result — methods not in eval
    ("Result", "and_then"),
    ("Result", "err"),
    ("Result", "expect"),
    ("Result", "expect_err"),
    ("Result", "map"),
    ("Result", "map_err"),
    ("Result", "ok"),
    ("Result", "or_else"),
    ("Result", "unwrap_err"),
    ("Result", "unwrap_or"),
    // Set — not in eval at all yet
    ("Set", "clone"),
    ("Set", "contains"),
    ("Set", "difference"),
    ("Set", "insert"),
    ("Set", "intersection"),
    ("Set", "is_empty"),
    ("Set", "iter"),
    ("Set", "len"),
    ("Set", "remove"),
    ("Set", "to_list"),
    ("Set", "union"),
    // Size — factory and conversion methods not in eval
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
    ("Size", "to_str"),
    ("Size", "to_tb"),
    ("Size", "zero"),
    // bool
    ("bool", "to_int"),
    // byte — predicates and conversions
    ("byte", "is_ascii"),
    ("byte", "is_ascii_alpha"),
    ("byte", "is_ascii_digit"),
    ("byte", "is_ascii_whitespace"),
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
    // (abs, ceil, floor, max, min, round, sqrt are via method resolvers)
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
    ("float", "round"),
    ("float", "signum"),
    ("float", "sin"),
    ("float", "sqrt"),
    ("float", "tan"),
    ("float", "to_int"),
    ("float", "trunc"),
    // int — methods not in eval direct dispatch
    // (abs, max, min are via method resolvers)
    ("int", "abs"),
    ("int", "clamp"),
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
    // list — many methods recognized by typeck but not in eval
    ("list", "all"),
    ("list", "any"),
    ("list", "append"),
    ("list", "chunk"),
    ("list", "count"),
    ("list", "enumerate"),
    ("list", "filter"),
    ("list", "find"),
    ("list", "flat_map"),
    ("list", "flatten"),
    ("list", "fold"),
    ("list", "for_each"),
    ("list", "get"),
    ("list", "group_by"),
    ("list", "join"),
    ("list", "map"),
    ("list", "max"),
    ("list", "max_by"),
    ("list", "min"),
    ("list", "min_by"),
    ("list", "partition"),
    ("list", "pop"),
    ("list", "prepend"),
    ("list", "product"),
    ("list", "push"),
    ("list", "reduce"),
    ("list", "reverse"),
    ("list", "skip"),
    ("list", "skip_while"),
    ("list", "sort"),
    ("list", "sort_by"),
    ("list", "sorted"),
    ("list", "sum"),
    ("list", "take"),
    ("list", "take_while"),
    ("list", "unique"),
    ("list", "window"),
    ("list", "zip"),
    // map — methods not in eval
    ("map", "contains"),
    ("map", "entries"),
    ("map", "get"),
    ("map", "insert"),
    ("map", "merge"),
    ("map", "remove"),
    ("map", "update"),
    // range — methods not in eval
    ("range", "collect"),
    ("range", "count"),
    ("range", "is_empty"),
    ("range", "step_by"),
    ("range", "to_list"),
    // str — many methods not in eval
    ("str", "byte_len"),
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "index_of"),
    ("str", "last_index_of"),
    ("str", "lines"),
    ("str", "pad_end"),
    ("str", "pad_start"),
    ("str", "parse_float"),
    ("str", "parse_int"),
    ("str", "repeat"),
    ("str", "replace"),
    ("str", "slice"),
    ("str", "split"),
    ("str", "substring"),
    ("str", "to_float"),
    ("str", "to_int"),
    ("str", "trim_end"),
    ("str", "trim_start"),
    // tuple — len not in eval
    ("tuple", "len"),
];

/// The typeck method list must be sorted for reliable comparison.
#[test]
fn typeck_method_list_is_sorted() {
    for window in TYPECK_BUILTIN_METHODS.windows(2) {
        assert!(
            window[0] <= window[1],
            "TYPECK_BUILTIN_METHODS not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
}

/// Every typeck method for primitive types should be in the IR registry.
#[test]
fn typeck_primitive_methods_in_ir() {
    let ir_set = ir_method_set();
    let known_set: BTreeSet<_> = TYPECK_METHODS_NOT_IN_IR.iter().copied().collect();

    let mut missing = Vec::new();
    for &(ty, method) in TYPECK_BUILTIN_METHODS {
        // Skip collection types (not yet in IR registry)
        if COLLECTION_TYPES.contains(&ty) {
            continue;
        }
        if !ir_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Type checker has primitive methods not in IR registry: {missing:?}\n\
         Add method definitions in ori_ir/src/builtin_methods/mod.rs or \
         add to TYPECK_METHODS_NOT_IN_IR"
    );
}

/// Every eval method should be recognized by the type checker.
/// If typeck doesn't recognize a method, it will report a type error
/// for code that would actually work at runtime.
#[test]
fn eval_methods_recognized_by_typeck() {
    let typeck_set: BTreeSet<_> = TYPECK_BUILTIN_METHODS.iter().copied().collect();
    let known_set: BTreeSet<_> = EVAL_METHODS_NOT_IN_TYPECK.iter().copied().collect();

    let mut missing = Vec::new();
    for &(ty, method) in EVAL_BUILTIN_METHODS {
        if !typeck_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Evaluator has methods not recognized by type checker: {missing:?}\n\
         Add to TYPECK_BUILTIN_METHODS in ori_types/src/infer/expr/methods.rs or \
         add to EVAL_METHODS_NOT_IN_TYPECK"
    );
}

/// Every typeck method should be implemented in the evaluator (or explicitly
/// listed as not-yet-implemented). This catches methods that type-check
/// successfully but fail at runtime with "no such method".
#[test]
fn typeck_methods_implemented_in_eval() {
    let eval_set: BTreeSet<_> = EVAL_BUILTIN_METHODS.iter().copied().collect();
    let known_set: BTreeSet<_> = TYPECK_METHODS_NOT_IN_EVAL.iter().copied().collect();

    let mut missing = Vec::new();
    for &(ty, method) in TYPECK_BUILTIN_METHODS {
        if !eval_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Type checker recognizes methods not implemented in evaluator: {missing:?}\n\
         Either implement in ori_eval or add to TYPECK_METHODS_NOT_IN_EVAL"
    );
}

// ── Iterator cross-crate consistency ─────────────────────────────────

/// Every Iterator method in typeck must have a corresponding eval resolver
/// entry, and vice versa. This closes the gap where Iterator methods were
/// exempted from all consistency checks via `COLLECTION_TYPES` and
/// `TYPECK_METHODS_NOT_IN_EVAL`.
#[test]
fn iterator_typeck_methods_match_eval_resolver() {
    let typeck_iter_methods: BTreeSet<&str> = TYPECK_BUILTIN_METHODS
        .iter()
        .filter(|(ty, _)| *ty == "Iterator")
        .map(|(_, method)| *method)
        .collect();

    let eval_iter_methods: BTreeSet<&str> = ITERATOR_METHOD_NAMES.iter().copied().collect();

    let in_typeck_not_eval: Vec<_> = typeck_iter_methods.difference(&eval_iter_methods).collect();
    let in_eval_not_typeck: Vec<_> = eval_iter_methods.difference(&typeck_iter_methods).collect();

    assert!(
        in_typeck_not_eval.is_empty(),
        "Iterator methods in typeck but missing from eval resolver: {in_typeck_not_eval:?}\n\
         Add to ITERATOR_METHOD_NAMES in ori_eval/src/interpreter/resolvers/mod.rs"
    );
    assert!(
        in_eval_not_typeck.is_empty(),
        "Iterator methods in eval resolver but missing from typeck: {in_eval_not_typeck:?}\n\
         Add to TYPECK_BUILTIN_METHODS in ori_types/src/infer/expr/methods.rs"
    );
}

/// The eval iterator method name list must be sorted for reliable comparison.
#[test]
fn eval_iterator_method_names_sorted() {
    for window in ITERATOR_METHOD_NAMES.windows(2) {
        assert!(
            window[0] <= window[1],
            "ITERATOR_METHOD_NAMES not sorted: {:?} > {:?}",
            window[0],
            window[1]
        );
    }
}
