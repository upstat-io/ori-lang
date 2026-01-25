//! Expression evaluation for literals, operators, and variables.
//!
//! This module handles the core expression evaluation that doesn't involve
//! control flow or function calls. It's designed to be called from the
//! main Evaluator.

use crate::ir::{ExprId, ExprKind, BinaryOp, UnaryOp, StringInterner, Name};
use crate::eval::{Value, RangeValue, EvalResult, EvalError};
use sigil_eval::{
    Environment, OperatorRegistry, UnaryOperatorRegistry,
    undefined_variable, cannot_get_length, index_out_of_bounds, key_not_found,
    cannot_index, no_field_on_struct, tuple_index_out_of_bounds, invalid_tuple_field,
    cannot_access_field,
};
use crate::context::SharedRegistry;

/// Evaluate a literal expression.
///
/// Returns the Value for the given literal `ExprKind`, or None if not a literal.
pub fn eval_literal(kind: &ExprKind, interner: &StringInterner) -> Option<EvalResult> {
    match kind {
        ExprKind::Int(n) => Some(Ok(Value::Int(*n))),
        ExprKind::Float(bits) => Some(Ok(Value::Float(f64::from_bits(*bits)))),
        ExprKind::Bool(b) => Some(Ok(Value::Bool(*b))),
        ExprKind::String(s) => {
            let string = interner.lookup(*s).to_string();
            Some(Ok(Value::string(string)))
        }
        ExprKind::Char(c) => Some(Ok(Value::Char(*c))),
        ExprKind::Unit => Some(Ok(Value::Void)),
        ExprKind::Duration { value, unit } => {
            Some(Ok(Value::Duration(unit.to_millis(*value))))
        }
        ExprKind::Size { value, unit } => {
            Some(Ok(Value::Size(unit.to_bytes(*value))))
        }
        _ => None,
    }
}

/// Evaluate an identifier lookup.
pub fn eval_ident(name: Name, env: &Environment, interner: &StringInterner) -> EvalResult {
    env.lookup(name).ok_or_else(|| {
        let name_str = interner.lookup(name);
        undefined_variable(name_str)
    })
}

/// Evaluate a binary operation with short-circuit logic for && and ||.
///
/// The `eval_fn` callback is used to evaluate the operands, allowing
/// lazy evaluation for short-circuit operators.
pub fn eval_binary<F>(
    left: ExprId,
    op: BinaryOp,
    right: ExprId,
    operator_registry: &SharedRegistry<OperatorRegistry>,
    mut eval_fn: F,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
{
    let left_val = eval_fn(left)?;

    // Short-circuit for && and ||
    match op {
        BinaryOp::And => {
            if !left_val.is_truthy() {
                return Ok(Value::Bool(false));
            }
            let right_val = eval_fn(right)?;
            return Ok(Value::Bool(right_val.is_truthy()));
        }
        BinaryOp::Or => {
            if left_val.is_truthy() {
                return Ok(Value::Bool(true));
            }
            let right_val = eval_fn(right)?;
            return Ok(Value::Bool(right_val.is_truthy()));
        }
        _ => {}
    }

    let right_val = eval_fn(right)?;

    // Delegate to operator registry
    operator_registry.evaluate(left_val, right_val, op)
}

/// Evaluate a unary operation.
pub fn eval_unary<F>(
    op: UnaryOp,
    operand: ExprId,
    unary_registry: &SharedRegistry<UnaryOperatorRegistry>,
    mut eval_fn: F,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
{
    let value = eval_fn(operand)?;
    unary_registry.evaluate(value, op)
}

/// Evaluate binary operation on already-evaluated values (for index context).
pub fn eval_binary_values(left_val: Value, op: BinaryOp, right_val: Value) -> EvalResult {
    match (left_val, right_val) {
        (Value::Int(a), Value::Int(b)) => match op {
            BinaryOp::Add => Ok(Value::Int(a + b)),
            BinaryOp::Sub => Ok(Value::Int(a - b)),
            BinaryOp::Mul => Ok(Value::Int(a * b)),
            BinaryOp::Div => {
                if b == 0 {
                    Err(EvalError::new("division by zero"))
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            _ => Err(EvalError::new("operator not supported in index context")),
        },
        _ => Err(EvalError::new("non-integer in index context")),
    }
}

/// Get the length of a collection for `HashLength` resolution.
pub fn get_collection_length(value: &Value) -> Result<i64, EvalError> {
    let len = match value {
        Value::List(items) | Value::Tuple(items) => items.len(),
        Value::Str(s) => s.chars().count(),
        Value::Map(map) => map.len(),
        _ => return Err(cannot_get_length(value.type_name())),
    };
    i64::try_from(len).map_err(|_| EvalError::new("collection too large"))
}

/// Evaluate a range expression.
pub fn eval_range<F>(
    start: Option<ExprId>,
    end: Option<ExprId>,
    inclusive: bool,
    mut eval_fn: F,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
{
    let start_val = if let Some(s) = start {
        eval_fn(s)?.as_int()
            .ok_or_else(|| EvalError::new("range start must be an integer"))?
    } else {
        0
    };
    let end_val = if let Some(e) = end {
        eval_fn(e)?.as_int()
            .ok_or_else(|| EvalError::new("range end must be an integer"))?
    } else {
        return Err(EvalError::new("unbounded range end"));
    };

    if inclusive {
        Ok(Value::Range(RangeValue::inclusive(start_val, end_val)))
    } else {
        Ok(Value::Range(RangeValue::exclusive(start_val, end_val)))
    }
}

/// Convert a signed index to unsigned, handling negative indices from the end.
fn resolve_index(i: i64, len: usize) -> Option<usize> {
    if i >= 0 {
        let idx = usize::try_from(i).ok()?;
        if idx < len { Some(idx) } else { None }
    } else {
        // Negative index: count from end
        // -i is positive since i < 0, safe to convert
        let positive = usize::try_from(-i).ok()?;
        if positive <= len { Some(len - positive) } else { None }
    }
}

/// Evaluate index access.
pub fn eval_index(value: Value, index: Value) -> EvalResult {
    match (value, index) {
        (Value::List(items), Value::Int(i)) => {
            let idx = resolve_index(i, items.len())
                .ok_or_else(|| index_out_of_bounds(i))?;
            items.get(idx).cloned().ok_or_else(|| index_out_of_bounds(i))
        }
        (Value::Str(s), Value::Int(i)) => {
            let char_count = s.chars().count();
            let idx = resolve_index(i, char_count)
                .ok_or_else(|| index_out_of_bounds(i))?;
            s.chars().nth(idx)
                .map(Value::Char)
                .ok_or_else(|| index_out_of_bounds(i))
        }
        (Value::Map(map), Value::Str(key)) => {
            map.get(key.as_str()).cloned()
                .ok_or_else(|| key_not_found(&key))
        }
        (value, index) => Err(cannot_index(value.type_name(), index.type_name())),
    }
}

/// Evaluate field access.
pub fn eval_field_access(value: Value, field: Name, interner: &StringInterner) -> EvalResult {
    match value {
        Value::Struct(s) => {
            s.get_field(field).cloned().ok_or_else(|| {
                let field_name = interner.lookup(field);
                no_field_on_struct(field_name)
            })
        }
        Value::Tuple(items) => {
            // Tuple field access like t.0, t.1
            let field_name = interner.lookup(field);
            if let Ok(idx) = field_name.parse::<usize>() {
                items.get(idx).cloned().ok_or_else(|| tuple_index_out_of_bounds(idx))
            } else {
                Err(invalid_tuple_field(field_name))
            }
        }
        value => Err(cannot_access_field(value.type_name())),
    }
}
