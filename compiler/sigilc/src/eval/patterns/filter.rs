// Filter pattern evaluation

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{is_truthy, Environment, Value};

pub fn eval_filter(
    collection: &Expr,
    predicate: &Expr,
    env: &Environment,
) -> Result<Value, String> {
    let coll = eval_expr(collection, env)?;
    let pred_val = eval_expr(predicate, env)?;

    let items = match coll {
        Value::List(items) => items,
        _ => return Err("filter requires a list".to_string()),
    };

    let mut results = Vec::new();
    for item in items {
        let keep = match &pred_val {
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
                    call_env.set(param.clone(), item.clone());
                }
                is_truthy(&eval_expr(body, &call_env)?)
            }
            _ => return Err("filter requires a function".to_string()),
        };
        if keep {
            results.push(item);
        }
    }
    Ok(Value::List(results))
}
