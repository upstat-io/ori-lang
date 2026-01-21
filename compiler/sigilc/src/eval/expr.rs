// Expression evaluation for Sigil
// Core eval_expr function and related helpers

use crate::ast::*;
use std::collections::HashMap;

use super::builtins::{eval_builtin, eval_method_call};
use super::calls::eval_function_call;
use super::match_eval::eval_match;
use super::operators::{eval_binary_op, eval_unary_op};
use super::patterns::eval_pattern;
use super::value::{is_truthy, Environment, Value};

/// Evaluate an expression within a block context (where assignments are allowed)
pub fn eval_block_expr(expr: &Expr, env: &mut Environment) -> Result<Value, String> {
    match expr {
        Expr::Assign { target, value } => {
            let val = eval_expr(value, env)?;
            env.set(target.clone(), val);
            Ok(Value::Nil)
        }
        Expr::For {
            binding,
            iterator,
            body,
        } => {
            let iter_val = eval_expr(iterator, env)?;
            match iter_val {
                Value::List(items) => {
                    for item in items {
                        env.set(binding.clone(), item);
                        eval_block_expr(body, env)?;
                    }
                }
                _ => return Err("For loop requires iterable".to_string()),
            }
            Ok(Value::Nil)
        }
        // For other expressions, delegate to eval_expr with immutable ref
        _ => eval_expr(expr, env),
    }
}

pub fn eval_expr(expr: &Expr, env: &Environment) -> Result<Value, String> {
    match expr {
        // Literals
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Float(f) => Ok(Value::Float(*f)),
        Expr::String(s) => Ok(Value::String(s.clone())),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Nil => Ok(Value::Nil),

        // Identifiers
        Expr::Ident(name) => {
            // Check for operator functions first (used in fold, etc.)
            match name.as_str() {
                "+" | "-" | "*" | "/" => {
                    return Ok(Value::BuiltinFunction(name.clone()));
                }
                _ => {}
            }
            env.get(name)
                .ok_or_else(|| format!("Unknown variable: {}", name))
        }

        Expr::Config(name) => env
            .configs
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Unknown config: ${}", name)),

        // Collections
        Expr::List(items) => {
            let values: Result<Vec<_>, _> = items.iter().map(|e| eval_expr(e, env)).collect();
            Ok(Value::List(values?))
        }

        Expr::Tuple(items) => {
            let values: Result<Vec<_>, _> = items.iter().map(|e| eval_expr(e, env)).collect();
            Ok(Value::Tuple(values?))
        }

        // Operators
        Expr::Binary { op, left, right } => {
            let lval = eval_expr(left, env)?;
            let rval = eval_expr(right, env)?;
            eval_binary_op(op, lval, rval)
        }

        Expr::Unary { op, operand } => {
            let val = eval_expr(operand, env)?;
            eval_unary_op(op, val)
        }

        // Function calls
        Expr::Call { func, args } => {
            let arg_values: Result<Vec<_>, _> = args.iter().map(|a| eval_expr(a, env)).collect();
            let arg_values = arg_values?;

            match func.as_ref() {
                Expr::Ident(name) => {
                    // Check for builtin functions
                    if let Some(result) = eval_builtin(name, &arg_values)? {
                        return Ok(result);
                    }

                    // Check if it's a closure stored in a local variable
                    if let Some(Value::Function {
                        params,
                        body,
                        env: closure_env,
                    }) = env.get(name)
                    {
                        // Create new environment with closure's captured environment
                        let mut call_env = Environment {
                            configs: env.configs.clone(),
                            functions: env.functions.clone(),
                            locals: closure_env.clone(),
                            current_params: params.clone(),
                        };
                        // Bind arguments to parameters
                        for (param, arg) in params.iter().zip(arg_values.iter()) {
                            call_env.set(param.clone(), arg.clone());
                        }
                        return eval_expr(&body, &call_env);
                    }

                    // User-defined function
                    if let Some(fd) = env.get_function(name).cloned() {
                        eval_function_call(&fd, arg_values, env)
                    } else {
                        Err(format!("Unknown function: {}", name))
                    }
                }
                _ => Err("Cannot call non-function".to_string()),
            }
        }

        // Method calls
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let recv = eval_expr(receiver, env)?;

            // Get length for $ substitution in method args (D-style)
            let len = match &recv {
                Value::List(items) => items.len() as i64,
                Value::String(s) => s.len() as i64,
                _ => 0,
            };

            // Evaluate args with $ = length support
            let arg_values: Result<Vec<_>, _> =
                args.iter().map(|a| eval_index_expr(a, env, len)).collect();
            eval_method_call(&recv, method, arg_values?, env)
        }

        // Field access
        Expr::Field(expr, field) => {
            let value = eval_expr(expr, env)?;
            match value {
                Value::Struct { fields, .. } => fields
                    .get(field)
                    .cloned()
                    .ok_or_else(|| format!("Unknown field: {}", field)),
                _ => Err("Cannot access field of non-struct".to_string()),
            }
        }

        // Indexing (with D-style $ support)
        Expr::Index(expr, index) => {
            let value = eval_expr(expr, env)?;

            let len = match &value {
                Value::List(items) => items.len() as i64,
                Value::String(s) => s.len() as i64,
                _ => 0,
            };

            let idx = eval_index_expr(index, env, len)?;

            match (value, idx) {
                (Value::List(items), Value::Int(i)) => items
                    .get(i as usize)
                    .cloned()
                    .ok_or_else(|| "Index out of bounds".to_string()),
                (Value::String(s), Value::Int(i)) => s
                    .chars()
                    .nth(i as usize)
                    .map(|c| Value::String(c.to_string()))
                    .ok_or_else(|| "Index out of bounds".to_string()),
                _ => Err("Cannot index".to_string()),
            }
        }

        // Result/Option types
        Expr::Ok(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Ok(Box::new(value)))
        }

        Expr::Err(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Err(Box::new(value)))
        }

        Expr::Some(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Some(Box::new(value)))
        }

        Expr::None_ => Ok(Value::None_),

        Expr::Coalesce { value, default } => {
            let val = eval_expr(value, env)?;
            match val {
                Value::Nil | Value::None_ => eval_expr(default, env),
                Value::Some(inner) => Ok(*inner),
                other => Ok(other),
            }
        }

        // Control flow
        Expr::Match(m) => {
            let scrutinee = eval_expr(&m.scrutinee, env)?;
            eval_match(&scrutinee, &m.arms, env)
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond = eval_expr(condition, env)?;
            if is_truthy(&cond) {
                eval_expr(then_branch, env)
            } else if let Some(else_expr) = else_branch {
                eval_expr(else_expr, env)
            } else {
                Ok(Value::Nil)
            }
        }

        Expr::Block(exprs) => {
            // Blocks create a local scope where assignments can be made
            let mut local_env = Environment {
                configs: env.configs.clone(),
                functions: env.functions.clone(),
                locals: env.locals.clone(),
                current_params: env.current_params.clone(),
            };
            let mut result = Value::Nil;
            for e in exprs {
                result = eval_block_expr(e, &mut local_env)?;
            }
            Ok(result)
        }

        // Patterns (fold, map, filter, etc.)
        Expr::Pattern(p) => eval_pattern(p, env),

        // Lambda
        Expr::Lambda { params, body } => Ok(Value::Function {
            params: params.clone(),
            body: *body.clone(),
            env: env.locals.clone(),
        }),

        // Struct literal
        Expr::Struct { name, fields } => {
            let mut field_values = HashMap::new();
            for (fname, fexpr) in fields {
                field_values.insert(fname.clone(), eval_expr(fexpr, env)?);
            }
            Ok(Value::Struct {
                name: name.clone(),
                fields: field_values,
            })
        }

        // Range
        Expr::Range { start, end } => {
            let s = eval_expr(start, env)?;
            let e = eval_expr(end, env)?;
            match (s, e) {
                (Value::Int(start), Value::Int(end)) => {
                    let items: Vec<Value> = (start..end).map(Value::Int).collect();
                    Ok(Value::List(items))
                }
                _ => Err("Range requires integer bounds".to_string()),
            }
        }

        _ => Err(format!("Cannot evaluate expression: {:?}", expr)),
    }
}

/// Evaluate index expression with # = array length
pub fn eval_index_expr(expr: &Expr, env: &Environment, length: i64) -> Result<Value, String> {
    match expr {
        Expr::LengthPlaceholder => Ok(Value::Int(length)),
        Expr::Binary { op, left, right } => {
            let l = eval_index_expr(left, env, length)?;
            let r = eval_index_expr(right, env, length)?;
            eval_binary_op(op, l, r)
        }
        Expr::Unary { op, operand } => {
            let val = eval_index_expr(operand, env, length)?;
            eval_unary_op(op, val)
        }
        _ => eval_expr(expr, env),
    }
}
