// Iterate pattern evaluation

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{Environment, Value};

pub fn eval_iterate(
    over: &Expr,
    direction: &IterDirection,
    into: &Expr,
    with: &Expr,
    env: &Environment,
) -> Result<Value, String> {
    let collection = eval_expr(over, env)?;
    let initial = eval_expr(into, env)?;
    let op_val = eval_expr(with, env)?;

    let items: Vec<Value> = match collection {
        Value::List(items) => items,
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        _ => return Err("iterate requires a list or string".to_string()),
    };

    // Apply direction
    let items: Vec<Value> = match direction {
        IterDirection::Forward => items,
        IterDirection::Backward => items.into_iter().rev().collect(),
    };

    let mut acc = initial;
    for (i, item) in items.into_iter().enumerate() {
        acc = match &op_val {
            Value::Function {
                params,
                body,
                env: fn_env,
            } => {
                let mut call_env = Environment {
                    configs: env.configs.clone(),
                    current_params: env.current_params.clone(),
                    functions: env.functions.clone(),
                    locals: fn_env.clone(),
                };
                // Bind 'acc' and 'char'/'item' and 'i' for the operation
                if !params.is_empty() {
                    call_env.set(params[0].clone(), acc);
                }
                if params.len() >= 2 {
                    call_env.set(params[1].clone(), item.clone());
                }
                // Also bind common names used in iterate
                call_env.set("acc".to_string(), call_env.get("acc").unwrap_or(Value::Nil));
                call_env.set("char".to_string(), item.clone());
                call_env.set("item".to_string(), item);
                call_env.set("i".to_string(), Value::Int(i as i64));
                eval_expr(body, &call_env)?
            }
            _ => return Err("iterate requires a function for 'with'".to_string()),
        };
    }
    Ok(acc)
}
