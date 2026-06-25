//! Method dispatch for wrapper variant types (Option, Result).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalError, EvalResult, Value};

use super::super::compare::{
    compare_option_values, compare_result_values, equals_values, hash_value, ordering_to_value,
};
use super::super::helpers::{debug_value, require_args};
use super::super::DispatchCtx;

/// Dispatch methods on Option values.
pub fn dispatch_option_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.unwrap || method == n.unwrap_or {
        // Both unwrap and unwrap_or return inner value for Some
        if let Value::Some(v) = &receiver {
            return Ok((**v).clone());
        }
        // None: unwrap errors, unwrap_or returns default
        if method == n.unwrap {
            return Err(EvalError::new("called unwrap on None").into());
        }
        require_args("unwrap_or", 1, args.len())?;
        match args.into_iter().next() {
            Some(default) => Ok(default),
            None => unreachable!("require_args verified length is 1"),
        }
    } else if method == n.is_some {
        Ok(Value::Bool(matches!(&receiver, Value::Some(_))))
    } else if method == n.is_none {
        Ok(Value::Bool(matches!(&receiver, Value::None)))
    // ok_or: Convert Option to Result
    } else if method == n.ok_or {
        require_args("ok_or", 1, args.len())?;
        match &receiver {
            Value::Some(v) => Ok(Value::ok((**v).clone())),
            _ => match args.into_iter().next() {
                Some(error) => Ok(Value::err(error)),
                None => unreachable!("require_args verified length is 1"),
            },
        }
    // Comparable trait - None < Some(_)
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let ord = compare_option_values(&receiver, &args[0], ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - recursive hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Iterable: Some(x) → 1-element list iterator, None → empty iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        // from_value handles Some → 1-element and None → empty iterator
        match ori_patterns::IteratorValue::from_value(&receiver) {
            Some(iter) => Ok(Value::iterator(iter)),
            None => unreachable!("Option values are always iterable"),
        }
    // Debug trait - structural representation
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver, ctx.interner)))
    // Higher-order methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_option_method_str(receiver, method_str, args)
    }
}

/// String-based dispatch for Option higher-order methods.
///
/// These methods require closures that can only be evaluated with a full
/// interpreter context. The dispatch handler validates argument count so
/// the method is recognized (not `UndefinedMethod`).
fn dispatch_option_method_str(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match method {
        "map" | "and_then" | "flat_map" | "filter" | "or_else" => {
            require_args(method, 1, args.len())?;
            // These require a closure — wrong_arg_type when called without interpreter
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        "expect" => {
            require_args("expect", 1, args.len())?;
            if let Value::Some(v) = &receiver {
                Ok((**v).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect failed on None".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        "or" => {
            require_args("or", 1, args.len())?;
            match &receiver {
                Value::Some(_) => Ok(receiver),
                _ => Ok(args.into_iter().next().unwrap_or(Value::None)),
            }
        }
        _ => Err(no_such_method(method, "Option").into()),
    }
}

/// Dispatch methods on Result values.
pub fn dispatch_result_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.unwrap || method == n.unwrap_or {
        match &receiver {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Err(e) => {
                if method == n.unwrap {
                    Err(EvalError::new(format!("called unwrap on Err: {e:?}")).into())
                } else {
                    require_args("unwrap_or", 1, args.len())?;
                    match args.into_iter().next() {
                        Some(default) => Ok(default),
                        None => unreachable!("require_args verified length is 1"),
                    }
                }
            }
            _ => unreachable!(),
        }
    } else if method == n.unwrap_err {
        match &receiver {
            Value::Err(e) => Ok((**e).clone()),
            Value::Ok(v) => Err(EvalError::new(format!("called unwrap_err on Ok: {v:?}")).into()),
            _ => unreachable!(),
        }
    } else if method == n.is_ok {
        Ok(Value::Bool(matches!(&receiver, Value::Ok(_))))
    } else if method == n.is_err {
        Ok(Value::Bool(matches!(&receiver, Value::Err(_))))
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = &args[0];
        let ord = compare_result_values(&receiver, other, ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - recursive hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Debug trait - structural representation
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver, ctx.interner)))
    // Traceable delegation: forward to inner Error if present
    } else if method == n.trace {
        require_args("trace", 0, args.len())?;
        Ok(Value::string(result_error_trace(&receiver)))
    } else if method == n.trace_entries {
        require_args("trace_entries", 0, args.len())?;
        match result_inner_error(&receiver) {
            Some(ev) => {
                let entries: Vec<Value> = ev
                    .trace()
                    .iter()
                    .map(|entry| super::super::error::trace_entry_to_struct(entry, ctx))
                    .collect();
                Ok(Value::list(entries))
            }
            None => Ok(Value::list(vec![])),
        }
    } else if method == n.has_trace {
        require_args("has_trace", 0, args.len())?;
        let has = result_inner_error(&receiver).is_some_and(ori_patterns::ErrorValue::has_trace);
        Ok(Value::Bool(has))
    // Higher-order and projection methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_result_method_str(&receiver, method_str, &args)
    }
}

/// String-based dispatch for Result methods not covered by Name-based dispatch.
fn dispatch_result_method_str(receiver: &Value, method: &str, args: &[Value]) -> EvalResult {
    match method {
        "map" | "map_err" | "and_then" | "or_else" => {
            require_args(method, 1, args.len())?;
            // These require a closure — wrong_arg_type when called without interpreter
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        "ok" => {
            require_args("ok", 0, args.len())?;
            if let Value::Ok(v) = receiver {
                Ok(Value::some((**v).clone()))
            } else {
                Ok(Value::None)
            }
        }
        "err" => {
            require_args("err", 0, args.len())?;
            if let Value::Err(e) = receiver {
                Ok(Value::some((**e).clone()))
            } else {
                Ok(Value::None)
            }
        }
        "expect" => {
            require_args("expect", 1, args.len())?;
            if let Value::Ok(v) = receiver {
                Ok((**v).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect failed on Err".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        "expect_err" => {
            require_args("expect_err", 1, args.len())?;
            if let Value::Err(e) = receiver {
                Ok((**e).clone())
            } else {
                let msg = match &args[0] {
                    Value::Str(s) => s.to_string(),
                    _ => "expect_err failed on Ok".to_string(),
                };
                Err(EvalError::new(msg).into())
            }
        }
        _ => Err(no_such_method(method, "Result").into()),
    }
}

/// Extract the inner `ErrorValue` from a Result's Err variant, if present.
fn result_inner_error(value: &Value) -> Option<&ori_patterns::ErrorValue> {
    match value {
        Value::Err(inner) => inner.as_error(),
        _ => None,
    }
}

/// Get the trace string from a Result's inner Error, or empty string.
fn result_error_trace(value: &Value) -> String {
    result_inner_error(value).map_or_else(String::new, ori_patterns::ErrorValue::format_trace)
}
