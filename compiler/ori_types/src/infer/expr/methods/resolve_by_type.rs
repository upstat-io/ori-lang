//! Per-type method resolution functions.
//!
//! Each function handles method lookup for a specific type tag,
//! returning `Some(return_type)` for recognized methods or `None`
//! to fall through to trait-based dispatch.

use crate::infer::InferEngine;
use crate::{Idx, Tag};

pub(super) fn resolve_list_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().list_elem(receiver_ty);
    match method {
        "len" | "length" | "count" | "hash" => Some(Idx::INT),
        "is_empty" | "contains" | "equals" => Some(Idx::BOOL),
        "first" | "last" | "pop" | "get" => Some(engine.pool_mut().option(elem)),
        "iter" => Some(engine.pool_mut().double_ended_iterator(elem)),
        "reverse" | "sort" | "sort_stable" | "sorted" | "unique" | "flatten" | "push"
        | "append" | "prepend" | "concat" | "clone" | "set" | "insert" | "remove" => {
            Some(receiver_ty)
        }
        "compare" => Some(Idx::ORDERING),
        "join" | "debug" => Some(Idx::STR),
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

pub(super) fn resolve_option_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let inner = engine.pool().option_inner(receiver_ty);
    match method {
        "is_some" | "is_none" | "equals" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(inner),
        "compare" => Some(Idx::ORDERING),
        "hash" => Some(Idx::INT),
        "ok_or" => {
            let err_ty = engine.pool_mut().fresh_var();
            Some(engine.pool_mut().result(inner, err_ty))
        }
        "iter" => Some(engine.pool_mut().iterator(inner)),
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            Some(engine.pool_mut().fresh_var())
        }
        "or" | "clone" => Some(receiver_ty),
        "debug" => Some(Idx::STR),
        _ => None,
    }
}

pub(super) fn resolve_result_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let ok_ty = engine.pool().result_ok(receiver_ty);
    let err_ty = engine.pool().result_err(receiver_ty);
    match method {
        "is_ok" | "is_err" | "has_trace" | "equals" => Some(Idx::BOOL),
        "unwrap" | "expect" | "unwrap_or" => Some(ok_ty),
        "compare" => Some(Idx::ORDERING),
        "hash" => Some(Idx::INT),
        "unwrap_err" | "expect_err" => Some(err_ty),
        "ok" => Some(engine.pool_mut().option(ok_ty)),
        "err" => Some(engine.pool_mut().option(err_ty)),
        "map" | "map_err" | "and_then" | "or_else" | "trace_entries" => {
            Some(engine.pool_mut().fresh_var())
        }
        "clone" => Some(receiver_ty),
        "debug" | "trace" => Some(Idx::STR),
        _ => None,
    }
}

pub(super) fn resolve_map_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let key_ty = engine.pool().map_key(receiver_ty);
    let value_ty = engine.pool().map_value(receiver_ty);
    match method {
        "len" | "length" | "hash" => Some(Idx::INT),
        "is_empty" | "contains_key" | "contains" | "equals" => Some(Idx::BOOL),
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
        "debug" => Some(Idx::STR),
        _ => None,
    }
}

pub(super) fn resolve_set_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: &str,
) -> Option<Idx> {
    let elem = engine.pool().set_elem(receiver_ty);
    match method {
        "len" | "length" | "hash" => Some(Idx::INT),
        "is_empty" | "contains" | "equals" => Some(Idx::BOOL),
        "iter" => Some(engine.pool_mut().iterator(elem)),
        "insert" | "remove" | "union" | "intersection" | "difference" | "clone" => {
            Some(receiver_ty)
        }
        "to_list" | "into" => Some(engine.pool_mut().list(elem)),
        "debug" => Some(Idx::STR),
        _ => None,
    }
}

pub(super) fn resolve_str_method(engine: &mut InferEngine<'_>, method: &str) -> Option<Idx> {
    match method {
        "into" => {
            // str.into() -> Error (wraps string as error message)
            Some(Idx::ERROR)
        }
        "len" | "byte_len" | "hash" | "length" => Some(Idx::INT),
        "iter" => Some(engine.pool_mut().double_ended_iterator(Idx::CHAR)),
        "is_empty" | "starts_with" | "ends_with" | "contains" | "equals" => Some(Idx::BOOL),
        "to_uppercase" | "to_lowercase" | "trim" | "trim_start" | "trim_end" | "replace"
        | "repeat" | "pad_start" | "pad_end" | "slice" | "substring" | "clone" | "debug"
        | "escape" | "concat" | "to_str" => Some(Idx::STR),
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

pub(super) fn resolve_int_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "min" | "max" | "clamp" | "pow" | "signum" | "clone" | "hash" => Some(Idx::INT),
        "to_float" | "into" | "f" => Some(Idx::FLOAT),
        "to_str" | "debug" => Some(Idx::STR),
        "to_byte" | "byte" => Some(Idx::BYTE),
        "is_positive" | "is_negative" | "is_zero" | "is_even" | "is_odd" | "equals" => {
            Some(Idx::BOOL)
        }
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

pub(super) fn resolve_float_method(method: &str) -> Option<Idx> {
    match method {
        "abs" | "sqrt" | "cbrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
        | "ln" | "log2" | "log10" | "exp" | "pow" | "min" | "max" | "clamp" | "signum"
        | "clone" => Some(Idx::FLOAT),
        "floor" | "ceil" | "round" | "trunc" | "to_int" | "hash" => Some(Idx::INT),
        "to_str" | "debug" => Some(Idx::STR),
        "is_nan" | "is_infinite" | "is_finite" | "is_normal" | "is_positive" | "is_negative"
        | "is_zero" | "equals" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

pub(super) fn resolve_duration_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_seconds" | "to_millis" | "to_micros" | "to_nanos" | "as_seconds" | "as_millis"
        | "as_micros" | "as_nanos" => Some(Idx::FLOAT),
        "to_str" | "format" | "debug" => Some(Idx::STR),
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

pub(super) fn resolve_size_method(method: &str) -> Option<Idx> {
    match method {
        // Instance methods
        "to_bytes" | "as_bytes" | "to_kb" | "to_mb" | "to_gb" | "to_tb" | "hash" => Some(Idx::INT),
        "to_str" | "format" | "debug" => Some(Idx::STR),
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

pub(super) fn resolve_channel_method(
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

pub(super) fn resolve_range_method(
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
pub(super) fn resolve_named_type_method(
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
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}

/// Ordering methods: predicates, reverse, equality, and trait methods.
pub(super) fn resolve_ordering_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "is_less"
        | "is_equal"
        | "is_greater"
        | "is_less_or_equal"
        | "is_greater_or_equal"
        | "equals" => Some(Idx::BOOL),
        "reverse" | "clone" | "compare" | "then" | "then_with" => Some(Idx::ORDERING),
        "hash" => Some(Idx::INT),
        "to_str" | "debug" => Some(Idx::STR),
        _ => None,
    }
}

pub(super) fn resolve_error_method(engine: &mut InferEngine<'_>, method_name: &str) -> Option<Idx> {
    match method_name {
        "message" | "to_str" | "debug" | "trace" => Some(Idx::STR),
        "has_trace" => Some(Idx::BOOL),
        "clone" | "with_trace" => Some(Idx::ERROR),
        "trace_entries" => Some(engine.pool_mut().fresh_var()),
        _ => None,
    }
}

pub(super) fn resolve_bool_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" | "debug" => Some(Idx::STR),
        "to_int" | "hash" => Some(Idx::INT),
        "clone" | "equals" => Some(Idx::BOOL),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

pub(super) fn resolve_byte_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_int" | "hash" => Some(Idx::INT),
        "to_char" => Some(Idx::CHAR),
        "to_str" | "debug" => Some(Idx::STR),
        "is_ascii" | "is_ascii_digit" | "is_ascii_alpha" | "is_ascii_whitespace" | "equals" => {
            Some(Idx::BOOL)
        }
        "clone" => Some(Idx::BYTE),
        "compare" => Some(Idx::ORDERING),
        _ => None,
    }
}

pub(super) fn resolve_char_method(method_name: &str) -> Option<Idx> {
    match method_name {
        "to_str" | "debug" => Some(Idx::STR),
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
/// - **Double-ended only** (`next_back`, `rev`, `last`, `rfind`, `rfold`) -- only
///   available on `DoubleEndedIterator<T>`, rejected on plain `Iterator<T>`
pub(super) fn resolve_iterator_method(
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
        "join" => Some(Idx::STR),
        "find" => Some(engine.pool_mut().option(elem)),
        "any" | "all" => Some(Idx::BOOL),
        "for_each" => Some(Idx::UNIT),
        "collect" => Some(engine.pool_mut().list(elem)),
        _ => None,
    }
}

pub(super) fn resolve_tuple_method(receiver_ty: Idx, method_name: &str) -> Option<Idx> {
    match method_name {
        "len" | "hash" => Some(Idx::INT),
        "compare" => Some(Idx::ORDERING),
        "equals" => Some(Idx::BOOL),
        "clone" => Some(receiver_ty),
        "debug" => Some(Idx::STR),
        _ => None,
    }
}
