// Collect pattern evaluation

use crate::ast::*;

use super::super::calls::eval_function_call;
use super::super::expr::eval_expr;
use super::super::value::{Environment, Value};

pub fn eval_collect(range: &Expr, transform: &Expr, env: &Environment) -> Result<Value, String> {
    let range_val = eval_expr(range, env)?;
    let transform_val = eval_expr(transform, env)?;

    let items = match range_val {
        Value::List(items) => items,
        _ => return Err("collect requires a range/list".to_string()),
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
            _ => {
                if let Expr::Ident(name) = transform {
                    if let Some(fd) = env.get_function(name).cloned() {
                        eval_function_call(&fd, vec![item], env)?
                    } else {
                        return Err(format!("Unknown function: {}", name));
                    }
                } else {
                    return Err("collect transform must be a function".to_string());
                }
            }
        };
        results.push(result);
    }
    Ok(Value::List(results))
}
