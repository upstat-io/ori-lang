// Match expression evaluation for Sigil

use crate::ast::{MatchArm, Pattern};

use super::expr::eval_expr;
use super::value::{is_truthy, Environment, Value};

/// Evaluate a match expression
pub fn eval_match(
    scrutinee: &Value,
    arms: &[MatchArm],
    env: &Environment,
) -> Result<Value, String> {
    for arm in arms {
        if let Some(bindings) = match_pattern(&arm.pattern, scrutinee, env)? {
            let mut new_env = Environment {
                configs: env.configs.clone(),
                current_params: env.current_params.clone(),
                functions: env.functions.clone(),
                locals: env.locals.clone(),
            };
            for (name, value) in bindings {
                new_env.set(name, value);
            }
            return eval_expr(&arm.body, &new_env);
        }
    }
    Err("No matching pattern".to_string())
}

/// Match a pattern against a value, returning bindings if successful
pub fn match_pattern(
    pattern: &Pattern,
    value: &Value,
    env: &Environment,
) -> Result<Option<Vec<(String, Value)>>, String> {
    match pattern {
        Pattern::Wildcard => Ok(Some(Vec::new())),

        Pattern::Binding(name) => Ok(Some(vec![(name.clone(), value.clone())])),

        Pattern::Literal(expr) => {
            let expected = eval_expr(expr, env)?;
            if expected == *value {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }

        Pattern::Variant { name, fields } => match (name.as_str(), value) {
            ("Ok", Value::Ok(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "value" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("Err", Value::Err(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "error" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("Some", Value::Some(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "value" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("None", Value::None_) => Ok(Some(Vec::new())),
            _ => Ok(None),
        },

        Pattern::Condition(expr) => {
            let result = eval_expr(expr, env)?;
            if is_truthy(&result) {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }
    }
}
