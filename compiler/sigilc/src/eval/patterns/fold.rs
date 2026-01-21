// Fold pattern evaluation

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::operators::eval_binary_op;
use super::super::value::{Environment, Value};

pub fn eval_fold(
    collection: &Expr,
    init: &Expr,
    op: &Expr,
    env: &Environment,
) -> Result<Value, String> {
    let coll = eval_expr(collection, env)?;
    let initial = eval_expr(init, env)?;

    let items: Vec<Value> = match coll {
        Value::List(items) => items,
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        _ => return Err("fold requires a list or string".to_string()),
    };

    let op_val = eval_expr(op, env)?;

    let mut acc = initial;
    for item in items {
        acc = match &op_val {
            Value::BuiltinFunction(name) if name == "+" => {
                eval_binary_op(&BinaryOp::Add, acc, item)?
            }
            Value::BuiltinFunction(name) if name == "*" => {
                eval_binary_op(&BinaryOp::Mul, acc, item)?
            }
            Value::Function {
                params,
                body,
                env: fn_env,
            } => {
                if params.len() != 2 {
                    return Err("fold function must take 2 arguments".to_string());
                }
                let mut call_env = Environment {
                    configs: env.configs.clone(),
                    current_params: env.current_params.clone(),
                    functions: env.functions.clone(),
                    locals: fn_env.clone(),
                };
                call_env.set(params[0].clone(), acc);
                call_env.set(params[1].clone(), item);
                eval_expr(body, &call_env)?
            }
            _ => return Err("Invalid fold operation".to_string()),
        };
    }
    Ok(acc)
}
