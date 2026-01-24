//! Expression evaluation for literals, operators, and variables.
//!
//! This module handles the core expression evaluation that doesn't involve
//! control flow or function calls. It's designed to be called from the
//! main Evaluator.

use crate::ir::{ExprId, ExprKind, BinaryOp, UnaryOp, StringInterner, Name};
use crate::eval::{Value, EvalResult, EvalError, RangeValue};
use crate::eval::errors;
use crate::eval::environment::Environment;
use crate::eval::operators::OperatorRegistry;
use crate::eval::unary_operators::UnaryOperatorRegistry;
use crate::context::SharedRegistry;

/// Evaluate a literal expression.
///
/// Returns the Value for the given literal ExprKind, or None if not a literal.
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
        errors::undefined_variable(name_str)
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

/// Get the length of a collection for HashLength resolution.
pub fn get_collection_length(value: &Value) -> Result<i64, EvalError> {
    match value {
        Value::List(items) => Ok(items.len() as i64),
        Value::Str(s) => Ok(s.chars().count() as i64),
        Value::Map(map) => Ok(map.len() as i64),
        Value::Tuple(items) => Ok(items.len() as i64),
        _ => Err(errors::cannot_get_length(value.type_name())),
    }
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

/// Evaluate index access.
pub fn eval_index(value: Value, index: Value) -> EvalResult {
    match (value, index) {
        (Value::List(items), Value::Int(i)) => {
            let idx = if i < 0 {
                (items.len() as i64 + i) as usize
            } else {
                i as usize
            };
            items.get(idx).cloned().ok_or_else(|| errors::index_out_of_bounds(i))
        }
        (Value::Str(s), Value::Int(i)) => {
            let idx = if i < 0 {
                (s.len() as i64 + i) as usize
            } else {
                i as usize
            };
            s.chars().nth(idx)
                .map(Value::Char)
                .ok_or_else(|| errors::index_out_of_bounds(i))
        }
        (Value::Map(map), Value::Str(key)) => {
            map.get(key.as_str()).cloned()
                .ok_or_else(|| errors::key_not_found(&key))
        }
        (value, index) => Err(errors::cannot_index(value.type_name(), index.type_name())),
    }
}

/// Evaluate field access.
pub fn eval_field_access(value: Value, field: Name, interner: &StringInterner) -> EvalResult {
    match value {
        Value::Struct(s) => {
            s.get_field(field).cloned().ok_or_else(|| {
                let field_name = interner.lookup(field);
                errors::no_field_on_struct(field_name)
            })
        }
        Value::Tuple(items) => {
            // Tuple field access like t.0, t.1
            let field_name = interner.lookup(field);
            if let Ok(idx) = field_name.parse::<usize>() {
                items.get(idx).cloned().ok_or_else(|| errors::tuple_index_out_of_bounds(idx))
            } else {
                Err(errors::invalid_tuple_field(field_name))
            }
        }
        value => Err(errors::cannot_access_field(value.type_name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;

    #[test]
    fn test_eval_literal_int() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Int(42), &interner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_literal_bool() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Bool(true), &interner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_literal_unit() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Unit, &interner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), Value::Void);
    }

    #[test]
    fn test_eval_literal_non_literal() {
        let interner = SharedInterner::default();
        let result = eval_literal(&ExprKind::Error, &interner);
        assert!(result.is_none());
    }

    #[test]
    fn test_eval_binary_values_add() {
        let result = eval_binary_values(Value::Int(2), BinaryOp::Add, Value::Int(3));
        assert_eq!(result.unwrap(), Value::Int(5));
    }

    #[test]
    fn test_eval_binary_values_div_by_zero() {
        let result = eval_binary_values(Value::Int(10), BinaryOp::Div, Value::Int(0));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("division by zero"));
    }

    #[test]
    fn test_get_collection_length_list() {
        let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(get_collection_length(&list).unwrap(), 3);
    }

    #[test]
    fn test_get_collection_length_string() {
        let s = Value::string("hello");
        assert_eq!(get_collection_length(&s).unwrap(), 5);
    }

    #[test]
    fn test_eval_index_list() {
        let list = Value::list(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
        assert_eq!(eval_index(list.clone(), Value::Int(1)).unwrap(), Value::Int(20));
        assert_eq!(eval_index(list, Value::Int(-1)).unwrap(), Value::Int(30));
    }

    #[test]
    fn test_eval_index_string() {
        let s = Value::string("hello");
        assert_eq!(eval_index(s, Value::Int(0)).unwrap(), Value::Char('h'));
    }

    #[test]
    fn test_eval_index_out_of_bounds() {
        let list = Value::list(vec![Value::Int(1)]);
        let result = eval_index(list, Value::Int(5));
        assert!(result.is_err());
    }
}
