//! Method dispatch for variant types (Option, Result, bool, char, byte, newtype).

use super::compare::{compare_option_values, compare_result_values, ordering_to_value};
use super::helpers::{require_args, require_bool_arg, require_byte_arg, require_char_arg};
use ori_ir::StringInterner;
use ori_patterns::{no_such_method, wrong_arg_count, EvalError, EvalResult, Value};

/// Dispatch operator methods on bool values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_bool_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Bool(a) = receiver else {
        unreachable!("dispatch_bool_method called with non-bool receiver")
    };

    match method {
        "not" => {
            require_args("not", 0, args.len())?;
            Ok(Value::Bool(!a))
        }
        // Comparable trait - false < true
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_bool_arg("compare", &args, 0)?;
            Ok(ordering_to_value(a.cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "bool")),
    }
}

/// Dispatch methods on char values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_char_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Char(c) = receiver else {
        unreachable!("dispatch_char_method called with non-char receiver")
    };

    match method {
        // Comparable trait - Unicode codepoint order
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_char_arg("compare", &args, 0)?;
            Ok(ordering_to_value(c.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "char")),
    }
}

/// Dispatch methods on byte values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_byte_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Byte(b) = receiver else {
        unreachable!("dispatch_byte_method called with non-byte receiver")
    };

    match method {
        // Comparable trait - numeric order
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_byte_arg("compare", &args, 0)?;
            Ok(ordering_to_value(b.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "byte")),
    }
}

/// Dispatch methods on newtype values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_newtype_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Newtype { inner, .. } = receiver else {
        unreachable!("dispatch_newtype_method called with non-newtype value");
    };

    match method {
        "unwrap" => {
            if !args.is_empty() {
                return Err(wrong_arg_count("unwrap", 0, args.len()));
            }
            Ok((*inner).clone())
        }
        _ => Err(no_such_method(method, "newtype")),
    }
}

/// Dispatch methods on Option values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_option_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    match (method, &receiver) {
        ("unwrap" | "unwrap_or", Value::Some(v)) => Ok((**v).clone()),
        ("unwrap", Value::None) => Err(EvalError::new("called unwrap on None")),
        ("is_some", Value::Some(_)) | ("is_none", Value::None) => Ok(Value::Bool(true)),
        ("is_some", Value::None) | ("is_none", Value::Some(_)) => Ok(Value::Bool(false)),
        ("unwrap_or", Value::None) => {
            require_args("unwrap_or", 1, args.len())?;
            match args.into_iter().next() {
                Some(default) => Ok(default),
                None => unreachable!("require_args verified length is 1"),
            }
        }
        // Comparable trait - None < Some(_)
        ("compare", _) => {
            require_args("compare", 1, args.len())?;
            let ord = compare_option_values(&receiver, &args[0], interner)?;
            Ok(ordering_to_value(ord, interner))
        }
        _ => Err(no_such_method(method, "Option")),
    }
}

/// Dispatch methods on Result values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_result_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    match method {
        "unwrap" => match &receiver {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Err(e) => Err(EvalError::new(format!("called unwrap on Err: {e:?}"))),
            _ => unreachable!(),
        },
        "is_ok" => Ok(Value::Bool(matches!(&receiver, Value::Ok(_)))),
        "is_err" => Ok(Value::Bool(matches!(&receiver, Value::Err(_)))),
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = &args[0];
            let ord = compare_result_values(&receiver, other, interner)?;
            Ok(ordering_to_value(ord, interner))
        }
        _ => Err(no_such_method(method, "Result")),
    }
}
