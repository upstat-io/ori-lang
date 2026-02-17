//! Built-in method resolution for primitives and collections.

use super::super::InferEngine;
use crate::{Idx, Tag};

/// Methods that are only available on `DoubleEndedIterator`, not plain `Iterator`.
///
/// Used by `resolve_iterator_method()` (the `if is_dei` guards) and by
/// `infer_method_call()` / `infer_method_call_named()` (diagnostic fallback for
/// non-DEI receivers). Single source of truth to prevent drift.
pub const DEI_ONLY_METHODS: &[&str] = &["last", "next_back", "rev", "rfind", "rfold"];

/// All built-in methods recognized by the type checker's `resolve_builtin_method()`.
///
/// Used by cross-crate consistency tests to verify the type checker, evaluator,
/// and IR registry agree on which methods exist. Each entry is `(type_name, method_name)`.
/// Sorted by type then method for deterministic comparison.
///
/// Type names match `EVAL_BUILTIN_METHODS` convention: lowercase for primitives
/// (`"int"`, `"str"`), proper-case for special types (`"Duration"`, `"Ordering"`).
///
/// **Named/Applied** types are excluded (user-defined, not builtin).
pub const TYPECK_BUILTIN_METHODS: &[(&str, &str)] = &[
    // Proper-cased types sort before lowercase in ASCII (A-Z < a-z).
    //
    // Channel
    ("Channel", "close"),
    ("Channel", "is_closed"),
    ("Channel", "is_empty"),
    ("Channel", "len"),
    ("Channel", "receive"),
    ("Channel", "recv"),
    ("Channel", "send"),
    ("Channel", "try_receive"),
    ("Channel", "try_recv"),
    // DoubleEndedIterator (methods only available on DoubleEndedIterator)
    ("DoubleEndedIterator", "last"),
    ("DoubleEndedIterator", "next_back"),
    ("DoubleEndedIterator", "rev"),
    ("DoubleEndedIterator", "rfind"),
    ("DoubleEndedIterator", "rfold"),
    // Duration
    ("Duration", "abs"),
    ("Duration", "as_micros"),
    ("Duration", "as_millis"),
    ("Duration", "as_nanos"),
    ("Duration", "as_seconds"),
    ("Duration", "clone"),
    ("Duration", "compare"),
    ("Duration", "equals"),
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
    ("Duration", "hash"),
    ("Duration", "hours"),
    ("Duration", "is_negative"),
    ("Duration", "is_positive"),
    ("Duration", "is_zero"),
    ("Duration", "microseconds"),
    ("Duration", "milliseconds"),
    ("Duration", "minutes"),
    ("Duration", "nanoseconds"),
    ("Duration", "seconds"),
    ("Duration", "to_micros"),
    ("Duration", "to_millis"),
    ("Duration", "to_nanos"),
    ("Duration", "to_seconds"),
    ("Duration", "to_str"),
    ("Duration", "zero"),
    // Iterator (methods available on both Iterator and DoubleEndedIterator)
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
    ("Iterator", "map"),
    ("Iterator", "next"),
    ("Iterator", "skip"),
    ("Iterator", "take"),
    ("Iterator", "zip"),
    // Option
    ("Option", "and_then"),
    ("Option", "clone"),
    ("Option", "expect"),
    ("Option", "filter"),
    ("Option", "flat_map"),
    ("Option", "is_none"),
    ("Option", "is_some"),
    ("Option", "iter"),
    ("Option", "map"),
    ("Option", "ok_or"),
    ("Option", "or"),
    ("Option", "or_else"),
    ("Option", "unwrap"),
    ("Option", "unwrap_or"),
    // Ordering
    ("Ordering", "clone"),
    ("Ordering", "compare"),
    ("Ordering", "debug"),
    ("Ordering", "equals"),
    ("Ordering", "hash"),
    ("Ordering", "is_equal"),
    ("Ordering", "is_greater"),
    ("Ordering", "is_greater_or_equal"),
    ("Ordering", "is_less"),
    ("Ordering", "is_less_or_equal"),
    ("Ordering", "reverse"),
    ("Ordering", "then"),
    ("Ordering", "to_str"),
    // Result
    ("Result", "and_then"),
    ("Result", "clone"),
    ("Result", "err"),
    ("Result", "expect"),
    ("Result", "expect_err"),
    ("Result", "is_err"),
    ("Result", "is_ok"),
    ("Result", "map"),
    ("Result", "map_err"),
    ("Result", "ok"),
    ("Result", "or_else"),
    ("Result", "unwrap"),
    ("Result", "unwrap_err"),
    ("Result", "unwrap_or"),
    // Set
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
    // Size
    ("Size", "as_bytes"),
    ("Size", "clone"),
    ("Size", "compare"),
    ("Size", "equals"),
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
    ("Size", "hash"),
    ("Size", "is_zero"),
    ("Size", "to_bytes"),
    ("Size", "to_gb"),
    ("Size", "to_kb"),
    ("Size", "to_mb"),
    ("Size", "to_str"),
    ("Size", "to_tb"),
    ("Size", "zero"),
    // bool
    ("bool", "clone"),
    ("bool", "compare"),
    ("bool", "equals"),
    ("bool", "hash"),
    ("bool", "to_int"),
    ("bool", "to_str"),
    // byte
    ("byte", "clone"),
    ("byte", "compare"),
    ("byte", "equals"),
    ("byte", "hash"),
    ("byte", "is_ascii"),
    ("byte", "is_ascii_alpha"),
    ("byte", "is_ascii_digit"),
    ("byte", "is_ascii_whitespace"),
    ("byte", "to_char"),
    ("byte", "to_int"),
    ("byte", "to_str"),
    // char
    ("char", "clone"),
    ("char", "compare"),
    ("char", "equals"),
    ("char", "hash"),
    ("char", "is_alpha"),
    ("char", "is_ascii"),
    ("char", "is_digit"),
    ("char", "is_lowercase"),
    ("char", "is_uppercase"),
    ("char", "is_whitespace"),
    ("char", "to_byte"),
    ("char", "to_int"),
    ("char", "to_lowercase"),
    ("char", "to_str"),
    ("char", "to_uppercase"),
    // float
    ("float", "abs"),
    ("float", "acos"),
    ("float", "asin"),
    ("float", "atan"),
    ("float", "atan2"),
    ("float", "cbrt"),
    ("float", "ceil"),
    ("float", "clamp"),
    ("float", "clone"),
    ("float", "compare"),
    ("float", "cos"),
    ("float", "equals"),
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
    ("float", "to_str"),
    ("float", "trunc"),
    // int
    ("int", "abs"),
    ("int", "clamp"),
    ("int", "clone"),
    ("int", "compare"),
    ("int", "equals"),
    ("int", "hash"),
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
    ("int", "to_str"),
    // list
    ("list", "all"),
    ("list", "any"),
    ("list", "append"),
    ("list", "chunk"),
    ("list", "clone"),
    ("list", "contains"),
    ("list", "count"),
    ("list", "enumerate"),
    ("list", "filter"),
    ("list", "find"),
    ("list", "first"),
    ("list", "flat_map"),
    ("list", "flatten"),
    ("list", "fold"),
    ("list", "for_each"),
    ("list", "get"),
    ("list", "group_by"),
    ("list", "is_empty"),
    ("list", "iter"),
    ("list", "join"),
    ("list", "last"),
    ("list", "len"),
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
    // map
    ("map", "clone"),
    ("map", "contains"),
    ("map", "contains_key"),
    ("map", "entries"),
    ("map", "get"),
    ("map", "insert"),
    ("map", "is_empty"),
    ("map", "iter"),
    ("map", "keys"),
    ("map", "len"),
    ("map", "merge"),
    ("map", "remove"),
    ("map", "update"),
    ("map", "values"),
    // range
    ("range", "collect"),
    ("range", "contains"),
    ("range", "count"),
    ("range", "is_empty"),
    ("range", "iter"),
    ("range", "len"),
    ("range", "step_by"),
    ("range", "to_list"),
    // str
    ("str", "byte_len"),
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "clone"),
    ("str", "compare"),
    ("str", "contains"),
    ("str", "ends_with"),
    ("str", "equals"),
    ("str", "hash"),
    ("str", "index_of"),
    ("str", "is_empty"),
    ("str", "iter"),
    ("str", "last_index_of"),
    ("str", "len"),
    ("str", "lines"),
    ("str", "pad_end"),
    ("str", "pad_start"),
    ("str", "parse_float"),
    ("str", "parse_int"),
    ("str", "repeat"),
    ("str", "replace"),
    ("str", "slice"),
    ("str", "split"),
    ("str", "starts_with"),
    ("str", "substring"),
    ("str", "to_float"),
    ("str", "to_int"),
    ("str", "to_lowercase"),
    ("str", "to_uppercase"),
    ("str", "trim"),
    ("str", "trim_end"),
    ("str", "trim_start"),
    // tuple
    ("tuple", "clone"),
    ("tuple", "len"),
];

/// Resolve a built-in method call on a known type tag.
///
/// Returns `Some(return_type)` if the method is a known built-in,
/// `None` if the method is not recognized for this type tag.
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Option<Idx> {
    match tag {
        Tag::List => resolve_list_method(engine, receiver_ty, method_name),
        Tag::Option => resolve_option_method(engine, receiver_ty, method_name),
        Tag::Result => resolve_result_method(engine, receiver_ty, method_name),
        Tag::Map => resolve_map_method(engine, receiver_ty, method_name),
        Tag::Set => resolve_set_method(engine, receiver_ty, method_name),
        Tag::Str => resolve_str_method(engine, method_name),
        Tag::Int => resolve_int_method(method_name),
        Tag::Float => resolve_float_method(method_name),
        Tag::Duration => resolve_duration_method(method_name),
        Tag::Size => resolve_size_method(method_name),
        Tag::Channel => resolve_channel_method(engine, receiver_ty, method_name),
        Tag::Range => resolve_range_method(engine, receiver_ty, method_name),
        Tag::Iterator | Tag::DoubleEndedIterator => {
            resolve_iterator_method(engine, receiver_ty, method_name)
        }
        Tag::Named | Tag::Applied => resolve_named_type_method(engine, receiver_ty, method_name),
        Tag::Bool => resolve_bool_method(method_name),
        Tag::Byte => resolve_byte_method(method_name),
        Tag::Char => resolve_char_method(method_name),
        Tag::Ordering => resolve_ordering_method(method_name),
        Tag::Tuple => resolve_tuple_method(receiver_ty, method_name),
        _ => None,
    }
}

fn resolve_list_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().list_elem(receiver_ty);
    match method {
        "len" | "count" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "first" | "last" | "pop" | "get" => Some(engine.pool_mut().option(elem)),
        "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
        "reverse" | "sort" | "sorted" | "unique" | "flatten" | "push" | "append" | "prepend"
        | "clone" => Some(receiver_ty),
        "join" => Some(Idx::STR),
        "enumerate" => {
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().list(pair))
        }
        "zip" => {
            // zip takes another list and returns list of tuples
            // Without knowing the other list's element type, return fresh var
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().list(pair))
        }
        "map" | "filter" | "flat_map" | "find" | "any" | "all" | "fold" | "reduce" | "for_each"
        | "take" | "skip" | "take_while" | "skip_while" | "chunk" | "window" | "min" | "max"
        | "sum" | "product" | "min_by" | "max_by" | "sort_by" | "group_by" | "partition" => {
            // Higher-order methods — return type depends on closure argument.
            // For now return fresh var; proper HO method inference is a follow-up.
            Some(engine.pool_mut().fresh_var())
        }
        _ => None,
    }
}

fn resolve_option_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let inner = engine.pool().option_inner(receiver_ty);
    match method {
        "is_some" | "is_none" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(inner),
        "ok_or" => {
            let err_ty = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().result(inner, err_ty))
        }
        "iter" => Some(engine.pool_mut().iterator(inner)),
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            Some(engine.pool_mut().fresh_var())
        }
        "or" | "clone" => Some(receiver_ty),
        _ => None,
    }
}

fn resolve_result_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let ok_ty = engine.pool().result_ok(receiver_ty);
    let err_ty = engine.pool().result_err(receiver_ty);
    match method {
        "is_ok" | "is_err" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(ok_ty),
        "unwrap_err" | "expect_err" => Some(err_ty),
        "ok" => Some(engine.pool_mut().option(ok_ty)),
        "err" => Some(engine.pool_mut().option(err_ty)),
        "map" | "map_err" | "and_then" | "or_else" => Some(engine.pool_mut().fresh_var()),
        "clone" => Some(receiver_ty),
        _ => None,
    }
}

fn resolve_map_method(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Option<Idx> {
    let key_ty = engine.pool().map_key(receiver_ty);
    let value_ty = engine.pool().map_value(receiver_ty);
    match method {
        "len" => Some(Idx::INT),
        "is_empty" | "contains_key" | "contains" => Some(Idx::BOOL),
        "get" => Some(engine.pool_mut().option(value_ty)),
        "iter" => {
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().iterator(pair))
        }
        "keys" => Some(engine.pool_mut().list(key_ty)),
        "values" => Some(engine.pool_mut().list(value_ty)),
        "entries" => {
            let pair = engine.pool_mut().tuple(&[key_ty, value_ty]);
            Some(engine.pool_mut().list(pair))
        }
        "insert" | "remove" | "update" | "merge" | "clone" => Some(receiver_ty),
        _ => None,
    }
}

fn resolve_set_method(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Option<Idx> {
    let elem = engine.pool().set_elem(receiver_ty);
    match method {
        "len" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "iter" => Some(engine.pool_mut().iterator(elem)),
        "insert" | "remove" | "union" | "intersection" | "difference" | "clone" => {
            Some(receiver_ty)
        }
        "to_list" => Some(engine.pool_mut().list(elem)),
        _ => None,
    }
}

fn resolve_str_method(engine: &mut InferEngine<'_>, method: &str) -> Option<Idx> {
    match method {
        "len" | "byte_len" | "hash" => Some(Idx::INT),
        "iter" => Some(engine.pool_mut().double_ended_iterator(Idx::CHAR)),
        "is_empty" | "starts_with" | "ends_with" | "contains" | "equals" => Some(Idx::BOOL),
        "to_uppercase" | "to_lowercase" | "trim" | "trim_start" | "trim_end" | "replace"
        | "repeat" | "pad_start" | "pad_end" | "slice" | "substring" | "clone" => Some(Idx::STR),
        "chars" => Some(engine.pool_mut().list(Idx::CHAR)),
        "bytes" => Some(engine.pool_mut().list(Idx::BYTE)),
        "split" | "lines" => Some(engine.pool_mut().list(Idx::STR)),
        "index_of" | "last_index_of" | "to_int" | "parse_int" => {
            Some(engine.pool_mut().option(Idx::INT))
        }
        "to_float" | "parse_float" => Some(engine.pool_mut().option(Idx::FLOAT)),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_int_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "min" | "max" | "clamp" | "pow" | "signum" | "clone" | "hash" => Some(Idx::INT),
        "to_float" => Some(Idx::FLOAT),
        "to_str" => Some(Idx::STR),
        "to_byte" => Some(Idx::BYTE),
        "is_positive" | "is_negative" | "is_zero" | "is_even" | "is_odd" | "equals" => {
            Some(Idx::BOOL)
        }
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_float_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "sqrt" | "cbrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
        | "ln" | "log2" | "log10" | "exp" | "pow" | "min" | "max" | "clamp" | "signum"
        | "clone" => Some(Idx::FLOAT),
        "floor" | "ceil" | "round" | "trunc" | "to_int" => Some(Idx::INT),
        "to_str" => Some(Idx::STR),
        "is_nan" | "is_infinite" | "is_finite" | "is_normal" | "is_positive" | "is_negative"
        | "is_zero" | "equals" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_duration_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_seconds" | "to_millis" | "to_micros" | "to_nanos" | "as_seconds" | "as_millis"
        | "as_micros" | "as_nanos" => Some(Idx::FLOAT),
        "to_str" | "format" => Some(Idx::STR),
        "abs" | "from_nanoseconds" | "from_microseconds" | "from_milliseconds" | "from_seconds"
        | "from_minutes" | "from_hours" | "from_nanos" | "from_micros" | "from_millis" | "zero"
        | "clone" => Some(Idx::DURATION),
        "is_zero" | "is_negative" | "is_positive" | "equals" => Some(Idx::BOOL),
        "nanoseconds" | "microseconds" | "milliseconds" | "seconds" | "minutes" | "hours"
        | "hash" => Some(Idx::INT),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_size_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_bytes" | "as_bytes" | "to_kb" | "to_mb" | "to_gb" | "to_tb" | "hash" => Some(Idx::INT),
        "to_str" | "format" => Some(Idx::STR),
        "is_zero" | "equals" => Some(Idx::BOOL),
        // Associated functions (static constructors): Size.from_bytes(b: 100)
        "from_bytes" | "from_kilobytes" | "from_megabytes" | "from_gigabytes"
        | "from_terabytes" | "from_kb" | "from_mb" | "from_gb" | "from_tb" | "zero" | "clone" => {
            Some(Idx::SIZE)
        }
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_channel_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().channel_elem(receiver_ty);
    match method {
        "send" | "close" => Some(Idx::UNIT),
        "recv" | "receive" | "try_recv" | "try_receive" => Some(engine.pool_mut().option(elem)),
        "is_closed" | "is_empty" => Some(Idx::BOOL),
        "len" => Some(Idx::INT),
        _ => None,
    }
}

fn resolve_range_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().range_elem(receiver_ty);
    // Range<float> does not implement Iterable — reject iteration methods.
    // Non-iteration methods (len, is_empty, contains) are still valid.
    let is_float = elem == Idx::FLOAT;
    match method {
        "len" | "count" => Some(Idx::INT),
        "is_empty" | "contains" => Some(Idx::BOOL),
        "iter" | "to_list" | "collect" if is_float => None,
        "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
        "to_list" | "collect" => Some(engine.pool_mut().list(elem)),
        "step_by" => Some(receiver_ty),
        _ => None,
    }
}

/// Resolve methods on Named/Applied types (user-defined structs, enums, newtypes).
///
/// For newtypes, supports `.unwrap()` to extract the inner value.
fn resolve_named_type_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method_name: &str,
) -> Option<Idx> {
    // Check type registry for newtype unwrap
    if method_name == "unwrap" || method_name == "inner" || method_name == "value" {
        if let Some(type_registry) = engine.type_registry() {
            if let Some(entry) = type_registry.get_by_idx(receiver_ty) {
                if let crate::TypeKind::Newtype { underlying } = &entry.kind {
                    return Some(*underlying);
                }
            }
        }
    }

    // Common methods on any user-defined type
    match method_name {
        "to_str" => Some(Idx::STR),
        _ => None,
    }
}

/// Ordering methods: predicates, reverse, equality, and trait methods.
fn resolve_ordering_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "is_less"
        | "is_equal"
        | "is_greater"
        | "is_less_or_equal"
        | "is_greater_or_equal"
        | "equals" => Some(Idx::BOOL),
        "reverse" | "clone" | "compare" | "then" => Some(Idx::ORDERING),
        "hash" => Some(Idx::INT),
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}

fn resolve_bool_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" => Some(Idx::STR),
        "to_int" | "hash" => Some(Idx::INT),
        "clone" | "equals" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_byte_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_int" | "hash" => Some(Idx::INT),
        "to_char" => Some(Idx::CHAR),
        "to_str" => Some(Idx::STR),
        "is_ascii" | "is_ascii_digit" | "is_ascii_alpha" | "is_ascii_whitespace" | "equals" => {
            Some(Idx::BOOL)
        }
        "clone" => Some(Idx::BYTE),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

fn resolve_char_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" => Some(Idx::STR),
        "to_int" | "to_byte" | "hash" => Some(Idx::INT),
        "is_digit" | "is_alpha" | "is_whitespace" | "is_uppercase" | "is_lowercase"
        | "is_ascii" | "equals" => Some(Idx::BOOL),
        "to_uppercase" | "to_lowercase" | "clone" => Some(Idx::CHAR),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

/// Resolve methods on `Iterator<T>` and `DoubleEndedIterator<T>`.
///
/// Three categories:
/// - **Adapters** return a new iterator (may propagate or downgrade double-endedness)
/// - **Consumers** eagerly consume the iterator and return a final value
/// - **Double-ended only** (`next_back`, `rev`, `last`, `rfind`, `rfold`) — only
///   available on `DoubleEndedIterator<T>`, rejected on plain `Iterator<T>`
fn resolve_iterator_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().iterator_elem(receiver_ty);
    let is_dei = engine.pool().tag(receiver_ty) == Tag::DoubleEndedIterator;

    match method {
        // next() — available on all iterators
        "next" => {
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }

        // === Double-ended-only methods ===
        "next_back" if is_dei => {
            let option_elem = engine.pool_mut().option(elem);
            Some(engine.pool_mut().tuple(&[option_elem, receiver_ty]))
        }
        "rev" if is_dei => Some(receiver_ty),
        "last" | "rfind" if is_dei => Some(engine.pool_mut().option(elem)),
        "rfold" if is_dei => Some(engine.pool_mut().fresh_var()),
        // Non-DEI receiver: next_back/rev/last/rfind/rfold fall through to wildcard (None)

        // === Adapters that PROPAGATE double-endedness ===
        "filter" => Some(receiver_ty),
        "map" => {
            let new_elem = engine.pool_mut().fresh_var();
            if is_dei {
                Some(engine.pool_mut().double_ended_iterator(new_elem))
            } else {
                Some(engine.pool_mut().iterator(new_elem))
            }
        }

        // === Adapters that DOWNGRADE to plain Iterator ===
        "take" | "skip" | "chain" | "cycle" => Some(engine.pool_mut().iterator(elem)),
        "flatten" | "flat_map" => {
            let new_elem = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().iterator(new_elem))
        }
        "enumerate" => {
            let pair = engine.pool_mut().tuple(&[Idx::INT, elem]);
            Some(engine.pool_mut().iterator(pair))
        }
        "zip" => {
            let other_elem = engine.pool_mut().fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            Some(engine.pool_mut().iterator(pair))
        }

        // === Consumers (available on all iterators) ===
        "fold" => Some(engine.pool_mut().fresh_var()),
        "count" => Some(Idx::INT),
        "find" => Some(engine.pool_mut().option(elem)),
        "any" | "all" => Some(Idx::BOOL),
        "for_each" => Some(Idx::UNIT),
        "collect" => Some(engine.pool_mut().list(elem)),
        _ => None,
    }
}

fn resolve_tuple_method(receiver_ty: Idx, method_name: &str) -> Option<Idx> {
    match method_name {
        "len" => Some(Idx::INT),
        "clone" => Some(receiver_ty),
        _ => None,
    }
}
