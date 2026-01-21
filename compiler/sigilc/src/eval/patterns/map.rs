// Map pattern evaluation

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{Environment, Value};

pub fn eval_map(collection: &Expr, transform: &Expr, env: &Environment) -> Result<Value, String> {
    let coll = eval_expr(collection, env)?;
    let transform_val = eval_expr(transform, env)?;

    let items = match coll {
        Value::List(items) => items,
        _ => return Err("map requires a list".to_string()),
    };

    let mut results = Vec::new();
    for item in items {
        let result = match &transform_val {
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
                eval_expr(body, &call_env)?
            }
            _ => return Err("map requires a function".to_string()),
        };
        results.push(result);
    }
    Ok(Value::List(results))
}
