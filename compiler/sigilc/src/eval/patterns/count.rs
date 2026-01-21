// Count pattern evaluation

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{is_truthy, Environment, Value};

pub fn eval_count(
    collection: &Expr,
    predicate: &Expr,
    env: &Environment,
) -> Result<Value, String> {
    let coll = eval_expr(collection, env)?;
    let pred_val = eval_expr(predicate, env)?;

    let items = match coll {
        Value::List(items) => items,
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        _ => return Err("count requires a collection".to_string()),
    };

    let mut count = 0;
    for item in items {
        let matches = match &pred_val {
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
                if let Some(param) = params.first() {
                    call_env.set(param.clone(), item);
                }
                is_truthy(&eval_expr(body, &call_env)?)
            }
            _ => return Err("count requires a predicate function".to_string()),
        };
        if matches {
            count += 1;
        }
    }
    Ok(Value::Int(count))
}
