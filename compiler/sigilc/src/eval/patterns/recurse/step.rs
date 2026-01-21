// Recurse step evaluation
// Handles eval_recurse_step and eval_recurse_expr

use std::collections::HashMap;

use crate::ast::*;

use super::super::super::builtins::eval_builtin;
use super::super::super::calls::eval_function_call;
use super::super::super::expr::eval_expr;
use super::super::super::operators::eval_binary_op;
use super::super::super::value::{is_truthy, Environment, Value};

use super::analysis::contains_self_call;

/// Evaluate the recursive step of a recurse pattern
/// This handles self() calls within the step expression
#[allow(clippy::too_many_arguments)] // Internal recursive evaluation requires full context
pub fn eval_recurse_step(
    step: &Expr,
    condition: &Expr,
    base_value: &Expr,
    env: &Environment,
    memo: bool,
    parallel_threshold: i64,
    current_n: i64,
    param_names: &[String],
) -> Result<Value, String> {
    // For self() calls, we need to substitute them with recursive evaluations
    eval_recurse_expr(
        step,
        step,
        condition,
        base_value,
        env,
        memo,
        parallel_threshold,
        current_n,
        &mut HashMap::new(),
        param_names,
    )
}

/// Recursively evaluate an expression, handling self() calls
/// `step` is the original step expression (for recursive calls)
/// `expr` is the current expression being evaluated
/// `parallel_threshold` - parallelize when n > threshold (i64::MAX = never)
/// `current_n` - the current value of n (for threshold comparison)
/// `param_names` - the names of the function parameters for binding self() args
#[allow(clippy::too_many_arguments)] // Internal recursive evaluation requires full context
pub fn eval_recurse_expr(
    expr: &Expr,
    step: &Expr,
    condition: &Expr,
    base_value: &Expr,
    env: &Environment,
    memo: bool,
    parallel_threshold: i64,
    current_n: i64,
    cache: &mut HashMap<Vec<i64>, Value>,
    param_names: &[String],
) -> Result<Value, String> {
    match expr {
        // Handle self() calls - this is the recursive invocation
        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "self" {
                    // This is a recursive call
                    // Evaluate the arguments in current environment
                    let arg_values: Result<Vec<Value>, String> = args
                        .iter()
                        .map(|a| {
                            eval_recurse_expr(
                                a,
                                step,
                                condition,
                                base_value,
                                env,
                                memo,
                                parallel_threshold,
                                current_n,
                                cache,
                                param_names,
                            )
                        })
                        .collect();
                    let arg_values = arg_values?;

                    // Create cache key from integer arguments
                    let cache_key: Vec<i64> = if memo {
                        arg_values
                            .iter()
                            .filter_map(|v| {
                                if let Value::Int(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    // Check cache if memoization is enabled
                    if memo && !cache_key.is_empty() {
                        if let Some(cached) = cache.get(&cache_key) {
                            return Ok(cached.clone());
                        }
                    }

                    // Create new environment with updated parameters
                    let mut new_env = Environment {
                        configs: env.configs.clone(),
                        current_params: env.current_params.clone(),
                        functions: env.functions.clone(),
                        locals: env.locals.clone(),
                    };

                    // Bind arguments to parameter names using positional binding
                    // param_names is in the correct order from the function definition
                    for (i, param_name) in param_names.iter().enumerate() {
                        if let Some(arg_val) = arg_values.get(i) {
                            // Parameters are immutable bindings
                            new_env.define(param_name.clone(), arg_val.clone(), false);
                        }
                    }

                    // Get the new n value for threshold comparison (use first int arg)
                    let new_n = arg_values
                        .iter()
                        .find_map(|v| {
                            if let Value::Int(n) = v {
                                Some(*n)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);

                    // Check base condition with new environment
                    let cond_result = eval_expr(condition, &new_env)?;

                    let result = if is_truthy(&cond_result) {
                        eval_expr(base_value, &new_env)?
                    } else {
                        // Recursively evaluate the STEP expression with new environment
                        // Pass the new n value for threshold comparison
                        eval_recurse_expr(
                            step,
                            step,
                            condition,
                            base_value,
                            &new_env,
                            memo,
                            parallel_threshold,
                            new_n,
                            cache,
                            param_names,
                        )?
                    };

                    // Cache the result if memoization is enabled
                    if memo && !cache_key.is_empty() {
                        cache.insert(cache_key, result.clone());
                    }

                    return Ok(result);
                }
            }

            // Regular function call - evaluate normally but recurse into args
            let arg_values: Result<Vec<Value>, String> = args
                .iter()
                .map(|a| {
                    eval_recurse_expr(
                        a,
                        step,
                        condition,
                        base_value,
                        env,
                        memo,
                        parallel_threshold,
                        current_n,
                        cache,
                        param_names,
                    )
                })
                .collect();
            let arg_values = arg_values?;

            if let Expr::Ident(name) = func.as_ref() {
                // Check for builtin functions
                if let Some(result) = eval_builtin(name, &arg_values)? {
                    return Ok(result);
                }

                // User-defined function
                if let Some(fd) = env.get_function(name).cloned() {
                    return eval_function_call(&fd, arg_values, env);
                }

                return Err(format!("Unknown function: {}", name));
            }

            Err("Cannot call non-function".to_string())
        }

        // For binary operations, recurse into both sides
        // If parallel threshold is exceeded, run left and right in separate threads
        Expr::Binary { op, left, right } => {
            // Only parallelize when current_n > parallel_threshold and both sides have self() calls
            if current_n > parallel_threshold
                && contains_self_call(left)
                && contains_self_call(right)
            {
                // Both sides have self() calls - parallelize them
                use std::thread;

                let step_clone = step.clone();
                let condition_clone = condition.clone();
                let base_value_clone = base_value.clone();
                let left_clone = left.as_ref().clone();
                let env_left = Environment {
                    configs: env.configs.clone(),
                    current_params: env.current_params.clone(),
                    functions: env.functions.clone(),
                    locals: env.locals.clone(),
                };
                let threshold = parallel_threshold;
                let n = current_n;

                let param_names_clone: Vec<String> = param_names.to_vec();
                let left_handle = thread::spawn(move || {
                    eval_recurse_expr(
                        &left_clone,
                        &step_clone,
                        &condition_clone,
                        &base_value_clone,
                        &env_left,
                        memo,
                        threshold,
                        n,
                        &mut HashMap::new(),
                        &param_names_clone,
                    )
                });

                // Evaluate right side in current thread
                let r = eval_recurse_expr(
                    right,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;

                // Wait for left side
                let l = left_handle
                    .join()
                    .map_err(|_| "Parallel recursion thread panicked".to_string())??;

                eval_binary_op(op, l, r)
            } else {
                // Sequential evaluation
                let l = eval_recurse_expr(
                    left,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;
                let r = eval_recurse_expr(
                    right,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;
                eval_binary_op(op, l, r)
            }
        }

        // For other expressions, just evaluate normally
        _ => eval_expr(expr, env),
    }
}
