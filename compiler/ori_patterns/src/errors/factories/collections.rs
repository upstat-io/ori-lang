//! Error-factory functions for collection, pattern, index-context, and
//! miscellaneous evaluation failures.

use crate::errors::{EvalError, EvalErrorKind};
use crate::value::Value;

// Miscellaneous Errors

/// Self used outside of method context.
#[cold]
pub fn self_outside_method() -> EvalError {
    EvalError::new("'self' used outside of method context")
}

/// Parse error placeholder.
#[cold]
pub fn parse_error() -> EvalError {
    EvalError::new("parse error")
}

/// Hash length used outside index brackets.
#[cold]
pub fn hash_outside_index() -> EvalError {
    EvalError::new("# can only be used inside index brackets")
}

/// Await not supported.
#[cold]
pub fn await_not_supported() -> EvalError {
    EvalError::new("await not supported in interpreter")
}

/// Invalid literal pattern.
#[cold]
pub fn invalid_literal_pattern() -> EvalError {
    EvalError::new("invalid literal pattern")
}

// Collection Method Errors

/// Map requires a collection (list or range).
#[cold]
pub fn map_requires_collection() -> EvalError {
    EvalError::new("map requires a collection")
}

/// Filter requires a collection (list or range).
#[cold]
pub fn filter_requires_collection() -> EvalError {
    EvalError::new("filter requires a collection")
}

/// Fold requires a collection (list or range).
#[cold]
pub fn fold_requires_collection() -> EvalError {
    EvalError::new("fold requires a collection")
}

/// Find requires a list.
#[cold]
pub fn find_requires_list() -> EvalError {
    EvalError::new("find requires a list")
}

/// Collect requires a range.
#[cold]
pub fn collect_requires_range() -> EvalError {
    EvalError::new("collect requires a range")
}

/// Any requires a list.
#[cold]
pub fn any_requires_list() -> EvalError {
    EvalError::new("any requires a list")
}

/// All requires a list.
#[cold]
pub fn all_requires_list() -> EvalError {
    EvalError::new("all requires a list")
}

/// Join requires a list.
#[cold]
pub fn join_requires_list() -> EvalError {
    EvalError::new("join requires a list")
}

/// Map entries requires a map.
#[cold]
pub fn map_entries_requires_map() -> EvalError {
    EvalError::new("map entries requires a map")
}

/// Filter entries requires a map.
#[cold]
pub fn filter_entries_requires_map() -> EvalError {
    EvalError::new("filter entries requires a map")
}

// Not Implemented Errors

/// Map entries not yet implemented.
#[cold]
pub fn map_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "map_entries() is not yet implemented".to_string(),
        suggestion: "use map() with entry destructuring: map.entries().map((k, v) -> ...)"
            .to_string(),
    })
}

/// Filter entries not yet implemented.
#[cold]
pub fn filter_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "filter_entries() is not yet implemented".to_string(),
        suggestion: "use filter() with entry destructuring: map.entries().filter((k, v) -> ...)"
            .to_string(),
    })
}

/// Index assignment (`list[i] = x`) is not supported.
#[cold]
pub fn index_assignment_not_supported() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "index assignment (list[i] = x) is not supported".to_string(),
        suggestion: "use functional update patterns instead".to_string(),
    })
}

/// Field assignment not yet implemented.
#[cold]
pub fn field_assignment_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "field assignment (obj.field = x) is not yet implemented".to_string(),
        suggestion: "use spread syntax: { ...obj, field: x }".to_string(),
    })
}

/// Default requires type context.
#[cold]
pub fn default_requires_type_context() -> EvalError {
    EvalError::new("default() requires type context; use explicit construction instead")
}

// Index Context Errors

/// Operator not supported in index context.
#[cold]
pub fn operator_not_supported_in_index() -> EvalError {
    EvalError::new("operator not supported in index context")
}

/// Non-integer in index context.
#[cold]
pub fn non_integer_in_index() -> EvalError {
    EvalError::new("non-integer in index context")
}

/// Collection too large for indexing.
#[cold]
pub fn collection_too_large() -> EvalError {
    EvalError::new("collection too large")
}

// Pattern Errors

/// Unknown pattern kind.
#[cold]
pub fn unknown_pattern(kind: &str) -> EvalError {
    EvalError::new(format!("unknown pattern: {kind}"))
}

/// For pattern requires a list.
#[cold]
pub fn for_pattern_requires_list(actual: &str) -> EvalError {
    EvalError::new(format!("for pattern requires a list, got {actual}"))
}

// Propagation Helpers

/// Create a standardized propagated error message.
///
/// This ensures consistent formatting of propagated errors across all call sites.
#[cold]
pub fn propagated_error_message(value: &Value) -> String {
    format!("propagated error: {value:?}")
}
