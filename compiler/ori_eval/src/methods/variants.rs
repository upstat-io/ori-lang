//! Method dispatch for variant types (Option, Result, bool, char, byte, newtype).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalError, EvalResult, Value};

use super::compare::{compare_option_values, compare_result_values, ordering_to_value};
use super::helpers::{
    debug_value, escape_debug_char, require_args, require_bool_arg, require_byte_arg,
    require_char_arg,
};
use super::DispatchCtx;

/// Dispatch operator methods on bool values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_bool_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Bool(a) = receiver else {
        unreachable!("dispatch_bool_method called with non-bool receiver")
    };

    let n = ctx.names;

    if method == n.not {
        require_args("not", 0, args.len())?;
        Ok(Value::Bool(!a))
    // Comparable trait - false < true
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let b = require_bool_arg("compare", &args, 0)?;
        Ok(ordering_to_value(a.cmp(&b)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let b = require_bool_arg("equals", &args, 0)?;
        Ok(Value::Bool(a == b))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(if a { "true" } else { "false" }))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(a)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "bool").into())
    }
}

/// Dispatch methods on char values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_char_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Char(c) = receiver else {
        unreachable!("dispatch_char_method called with non-char receiver")
    };

    let n = ctx.names;

    // Comparable trait - Unicode codepoint order
    if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_char_arg("compare", &args, 0)?;
        Ok(ordering_to_value(c.cmp(&other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_char_arg("equals", &args, 0)?;
        Ok(Value::Bool(c == other))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable trait
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(c.to_string()))
    // Debug trait - shows escaped char with quotes
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(format!("'{}'", escape_debug_char(c))))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(c as u32)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "char").into())
    }
}

/// Dispatch methods on byte values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_byte_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Byte(b) = receiver else {
        unreachable!("dispatch_byte_method called with non-byte receiver")
    };

    let n = ctx.names;

    // Comparable trait - numeric order
    if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_byte_arg("compare", &args, 0)?;
        Ok(ordering_to_value(b.cmp(&other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_byte_arg("equals", &args, 0)?;
        Ok(Value::Bool(b == other))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Printable and Debug traits
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(format!("0x{b:02x}")))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(i64::from(b)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "byte").into())
    }
}

/// Dispatch methods on newtype values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_newtype_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Newtype { inner, .. } = receiver else {
        unreachable!("dispatch_newtype_method called with non-newtype value");
    };

    let n = ctx.names;

    if method == n.unwrap {
        require_args("unwrap", 0, args.len())?;
        Ok((*inner).clone())
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "newtype").into())
    }
}

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
        Ok(Value::string(debug_value(&receiver)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Option").into())
    }
}

/// Dispatch methods on Result values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_result_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.unwrap {
        match &receiver {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Err(e) => Err(EvalError::new(format!("called unwrap on Err: {e:?}")).into()),
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
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Debug trait - structural representation
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Result").into())
    }
}
