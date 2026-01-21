// Transform pattern evaluation

use crate::ast::*;

use super::super::calls::eval_function_call;
use super::super::expr::eval_expr;
use super::super::value::{Environment, Value};

pub fn eval_transform(input: &Expr, steps: &[Expr], env: &Environment) -> Result<Value, String> {
    // Transform passes a value through a series of transformation steps
    let mut value = eval_expr(input, env)?;

    for step_expr in steps {
        // Each step can be a function or a lambda
        let step_val = eval_expr(step_expr, env)?;
        value = match step_val {
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
                    call_env.set(param.clone(), value);
                }
                // Also bind 'x' as common transform variable
                call_env.set(
                    "x".to_string(),
                    call_env
                        .locals
                        .get(params.first().unwrap_or(&"x".to_string()))
                        .cloned()
                        .unwrap_or(Value::Nil),
                );
                eval_expr(&body, &call_env)?
            }
            _ => {
                // If it's an identifier, try to call it as a function
                if let Expr::Ident(name) = step_expr {
                    if let Some(fd) = env.get_function(name).cloned() {
                        eval_function_call(&fd, vec![value], env)?
                    } else {
                        return Err(format!("Unknown transform function: {}", name));
                    }
                } else {
                    return Err("Transform step must be a function".to_string());
                }
            }
        };
    }
    Ok(value)
}
