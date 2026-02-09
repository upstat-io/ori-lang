//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.
//!
//! # Module Structure
//!
//! - [`helpers`]: Argument validation and shared utilities
//! - [`numeric`]: Methods on `int` and `float` types
//! - [`collections`]: Methods on `list`, `str`, `map`, and `range` types
//! - [`variants`]: Methods on `Option`, `Result`, `bool`, `char`, `byte`, and `newtype`
//! - [`units`]: Methods on `Duration` and `Size` types
//! - [`ordering`]: Methods on `Ordering` type
//! - [`compare`]: Value comparison utilities

mod collections;
mod compare;
mod helpers;
mod numeric;
mod ordering;
mod units;
mod variants;

pub use helpers::EVAL_BUILTIN_METHODS;

use ori_ir::StringInterner;
use ori_patterns::{no_such_method, EvalResult, Value};

/// Dispatch an associated function call (static method without receiver instance).
///
/// Associated functions are called on type names rather than instances,
/// e.g., `Duration.from_seconds(s: 10)`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature with other dispatch functions"
)]
pub fn dispatch_associated_function(type_name: &str, method: &str, args: Vec<Value>) -> EvalResult {
    match type_name {
        "Duration" => units::dispatch_duration_associated(method, &args),
        "Size" => units::dispatch_size_associated(method, &args),
        _ => Err(no_such_method(method, type_name).into()),
    }
}

/// Dispatch a built-in method call using direct pattern matching.
///
/// This is the preferred entry point for built-in method calls. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
///
/// Handles operator trait methods (add, sub, mul, etc.) uniformly for all types.
pub fn dispatch_builtin_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    match &receiver {
        Value::Int(_) => numeric::dispatch_int_method(receiver, method, args, interner),
        Value::Float(_) => numeric::dispatch_float_method(receiver, method, args, interner),
        Value::Bool(_) => variants::dispatch_bool_method(receiver, method, args, interner),
        Value::Char(_) => variants::dispatch_char_method(receiver, method, args, interner),
        Value::Byte(_) => variants::dispatch_byte_method(receiver, method, args, interner),
        Value::List(_) => collections::dispatch_list_method(receiver, method, args, interner),
        Value::Str(_) => collections::dispatch_string_method(receiver, method, args, interner),
        Value::Map(_) => collections::dispatch_map_method(receiver, method, args),
        Value::Range(_) => collections::dispatch_range_method(receiver, method, args),
        Value::Some(_) | Value::None => {
            variants::dispatch_option_method(receiver, method, args, interner)
        }
        Value::Ok(_) | Value::Err(_) => {
            variants::dispatch_result_method(receiver, method, args, interner)
        }
        Value::Newtype { .. } => variants::dispatch_newtype_method(receiver, method, args),
        Value::Duration(_) => units::dispatch_duration_method(receiver, method, args, interner),
        Value::Size(_) => units::dispatch_size_method(receiver, method, args, interner),
        Value::Ordering(_) => ordering::dispatch_ordering_method(receiver, method, args, interner),
        _ => Err(no_such_method(method, receiver.type_name()).into()),
    }
}
