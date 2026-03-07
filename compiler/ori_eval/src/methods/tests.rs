//! Tests for builtin method names and dispatch.

use super::BuiltinMethodNames;

/// All method-name strings interned by `BuiltinMethodNames`, excluding
/// non-method fields (`duration`, `size` are type names for associated
/// function dispatch).
///
/// Must be kept in sync with `assert_exhaustive` below — adding a field
/// to `BuiltinMethodNames` without adding it to both places causes a
/// compile error (from the destructure) or a test failure (from the
/// registry comparison).
const FIELD_NAMES: &[&str] = &[
    "add",
    "all",
    "any",
    "bit_and",
    "bit_not",
    "bit_or",
    "bit_xor",
    "bytes",
    "clone",
    "compare",
    "concat",
    "contains",
    "contains_key",
    "debug",
    "difference",
    "div",
    "divide",
    "drop",
    "ends_with",
    "entries",
    "equals",
    "escape",
    "first",
    "floor_div",
    "get",
    "gigabytes",
    "has_trace",
    "hash",
    "hours",
    "insert",
    "intersection",
    "into",
    "is_empty",
    "is_equal",
    "is_err",
    "is_greater",
    "is_greater_or_equal",
    "is_less",
    "is_less_or_equal",
    "is_none",
    "is_ok",
    "is_some",
    "iter",
    "keys",
    "kilobytes",
    "last",
    "len",
    "length",
    "megabytes",
    "message",
    "microseconds",
    "milliseconds",
    "minutes",
    "mul",
    "multiply",
    "nanoseconds",
    "neg",
    "negate",
    "not",
    "ok_or",
    "pop",
    "push",
    "remainder",
    "rem",
    "remove",
    "repeat",
    "replace",
    "reverse",
    "seconds",
    "set",
    "shl",
    "shr",
    "skip",
    "slice",
    "sort",
    "sort_stable",
    "split",
    "starts_with",
    "sub",
    "substring",
    "subtract",
    "take",
    "terabytes",
    "then",
    "to_list",
    "to_lowercase",
    "to_str",
    "to_uppercase",
    "trace",
    "trace_entries",
    "trim",
    "union",
    "unwrap",
    "unwrap_err",
    "unwrap_or",
    "values",
    "with_trace",
];

/// Compile-time exhaustive destructure of `BuiltinMethodNames`.
///
/// Adding a field to the struct without updating this function causes a
/// compile error. The field values are unused — this is purely a
/// completeness gate.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive destructure of 97-field struct — cannot be shortened"
)]
fn assert_exhaustive(names: &BuiltinMethodNames) {
    let BuiltinMethodNames {
        add,
        sub,
        mul,
        div,
        rem,
        neg,
        compare,
        equals,
        clone_,
        to_str,
        debug,
        hash,
        contains,
        len,
        length,
        is_empty,
        not,
        unwrap,
        concat,
        floor_div,
        bit_and,
        bit_or,
        bit_xor,
        bit_not,
        shl,
        shr,
        to_uppercase,
        to_lowercase,
        trim,
        starts_with,
        ends_with,
        escape,
        replace,
        repeat,
        split,
        substring,
        first,
        last,
        pop,
        push,
        set,
        slice,
        sort,
        sort_stable,
        take,
        skip,
        drop_,
        contains_key,
        get,
        keys,
        values,
        entries,
        insert,
        remove,
        union,
        intersection,
        difference,
        to_list,
        unwrap_or,
        is_some,
        is_none,
        ok_or,
        is_ok,
        is_err,
        unwrap_err,
        is_less,
        is_equal,
        is_greater,
        is_less_or_equal,
        is_greater_or_equal,
        reverse,
        then,
        duration,
        size,
        subtract,
        multiply,
        divide,
        remainder,
        negate,
        nanoseconds,
        microseconds,
        milliseconds,
        seconds,
        minutes,
        hours,
        bytes,
        kilobytes,
        megabytes,
        gigabytes,
        terabytes,
        iter,
        into,
        trace,
        trace_entries,
        has_trace,
        with_trace,
        message,
    } = names;

    let _ = (
        add,
        sub,
        mul,
        div,
        rem,
        neg,
        compare,
        equals,
        clone_,
        to_str,
        debug,
        hash,
        contains,
        len,
        length,
        is_empty,
        not,
        unwrap,
        concat,
        floor_div,
        bit_and,
        bit_or,
        bit_xor,
        bit_not,
        shl,
        shr,
        to_uppercase,
        to_lowercase,
        trim,
        starts_with,
        ends_with,
        escape,
        replace,
        repeat,
        split,
        substring,
        first,
        last,
        pop,
        push,
        set,
        slice,
        sort,
        sort_stable,
        take,
        skip,
        drop_,
        contains_key,
        get,
        keys,
        values,
        entries,
        insert,
        remove,
        union,
        intersection,
        difference,
        to_list,
        unwrap_or,
        is_some,
        is_none,
        ok_or,
        is_ok,
        is_err,
        unwrap_err,
        is_less,
        is_equal,
        is_greater,
        is_less_or_equal,
        is_greater_or_equal,
        reverse,
        then,
        duration,
        size,
        subtract,
        multiply,
        divide,
        remainder,
        negate,
        nanoseconds,
        microseconds,
        milliseconds,
        seconds,
        minutes,
        hours,
        bytes,
        kilobytes,
        megabytes,
        gigabytes,
        terabytes,
        iter,
        into,
        trace,
        trace_entries,
        has_trace,
        with_trace,
        message,
    );
}

/// Validates that every field in `BuiltinMethodNames` corresponds to a method
/// declared in `ori_registry::BUILTIN_TYPES`.
///
/// Two-layer enforcement:
/// 1. **Compile-time**: `assert_exhaustive` destructures every field — adding
///    a field without updating it causes a compile error.
/// 2. **Runtime**: Every method-name field (excluding known non-method fields)
///    must appear in the registry's method declarations.
#[test]
fn builtin_method_names_match_registry() {
    use ori_registry::BUILTIN_TYPES;
    use std::collections::BTreeSet;

    let interner = ori_ir::SharedInterner::default();
    let names = BuiltinMethodNames::new(&interner);

    // Compile-time exhaustiveness check
    assert_exhaustive(&names);

    let field_names: BTreeSet<&str> = FIELD_NAMES.iter().copied().collect();

    // Eval-only operator trait aliases. The registry declares the short
    // names (sub, mul, div, rem, neg); the evaluator also accepts the
    // verbose trait method names as fallback dispatch for Duration/Size.
    let eval_only_aliases: BTreeSet<&str> =
        ["subtract", "multiply", "divide", "remainder", "negate"]
            .into_iter()
            .collect();

    let registry_method_names: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Every field name (minus known eval-only aliases) must exist in registry
    let method_fields: BTreeSet<&str> = field_names
        .difference(&eval_only_aliases)
        .copied()
        .collect();
    let missing: Vec<&&str> = method_fields.difference(&registry_method_names).collect();
    assert!(
        missing.is_empty(),
        "BuiltinMethodNames has fields not in registry: {missing:?}\n\
         These interned names don't correspond to any method in BUILTIN_TYPES.\n\
         Either add the method to the registry, add to eval_only_aliases, or remove the field."
    );
}
