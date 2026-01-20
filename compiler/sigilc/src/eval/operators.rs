// Binary and unary operator evaluation

use crate::ast::{BinaryOp, UnaryOp};
use super::value::Value;

/// Evaluate a binary operation
pub fn eval_binary_op(op: &BinaryOp, left: Value, right: Value) -> Result<Value, String> {
    match (op, left, right) {
        (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinaryOp::Add, Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
        (BinaryOp::Add, Value::String(a), Value::Int(b)) => Ok(Value::String(format!("{}{}", a, b))),
        (BinaryOp::Add, Value::List(mut a), Value::List(b)) => {
            a.extend(b);
            Ok(Value::List(a))
        }

        (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),

        (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),

        (BinaryOp::Div, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                Err("Division by zero".to_string())
            } else {
                // Integer division returns integer (like Rust/Java)
                Ok(Value::Int(a / b))
            }
        }
        (BinaryOp::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (BinaryOp::Div, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
        (BinaryOp::Div, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / b as f64)),

        (BinaryOp::IntDiv, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                Err("Division by zero".to_string())
            } else {
                Ok(Value::Int(a / b))
            }
        }

        (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),

        (BinaryOp::Eq, a, b) => Ok(Value::Bool(a == b)),
        (BinaryOp::NotEq, a, b) => Ok(Value::Bool(a != b)),

        (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        (BinaryOp::LtEq, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (BinaryOp::GtEq, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),

        (BinaryOp::And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
        (BinaryOp::Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),

        _ => Err("Invalid binary operation".to_string()),
    }
}

/// Evaluate a unary operation
pub fn eval_unary_op(op: &UnaryOp, val: Value) -> Result<Value, String> {
    match (op, val) {
        (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
        (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
        (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        _ => Err("Invalid unary operation".to_string()),
    }
}
