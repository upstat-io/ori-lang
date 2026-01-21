// Pattern evaluation for Sigil
// Handles fold, map, filter, recurse, parallel, etc.

use crate::ast::*;
use std::collections::HashMap;

use super::builtins::eval_builtin;
use super::calls::eval_function_call;
use super::expr::eval_expr;
use super::operators::eval_binary_op;
use super::value::{is_truthy, Environment, Value};

pub fn eval_pattern(pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
    match pattern {
        PatternExpr::Fold {
            collection,
            init,
            op,
        } => eval_fold(collection, init, op, env),

        PatternExpr::Map {
            collection,
            transform,
        } => eval_map(collection, transform, env),

        PatternExpr::Filter {
            collection,
            predicate,
        } => eval_filter(collection, predicate, env),

        PatternExpr::Collect { range, transform } => eval_collect(range, transform, env),

        PatternExpr::Count {
            collection,
            predicate,
        } => eval_count(collection, predicate, env),

        PatternExpr::Recurse {
            condition,
            base_value,
            step,
            memo,
            parallel_threshold,
        } => eval_recurse(condition, base_value, step, *memo, *parallel_threshold, env),

        PatternExpr::Iterate {
            over,
            direction,
            into,
            with,
        } => eval_iterate(over, direction, into, with, env),

        PatternExpr::Transform { input, steps } => eval_transform(input, steps, env),

        PatternExpr::Parallel {
            branches,
            timeout,
            on_error,
        } => eval_parallel(branches, timeout, on_error, env),
    }
}

fn eval_fold(
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

fn eval_map(collection: &Expr, transform: &Expr, env: &Environment) -> Result<Value, String> {
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

fn eval_filter(collection: &Expr, predicate: &Expr, env: &Environment) -> Result<Value, String> {
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

fn eval_collect(range: &Expr, transform: &Expr, env: &Environment) -> Result<Value, String> {
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

fn eval_count(collection: &Expr, predicate: &Expr, env: &Environment) -> Result<Value, String> {
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

fn eval_recurse(
    condition: &Expr,
    base_value: &Expr,
    step: &Expr,
    memo: bool,
    parallel_threshold: i64,
    env: &Environment,
) -> Result<Value, String> {
    // For recurse, we need to be inside a function context
    // The recurse pattern creates a recursive function that:
    // 1. Checks condition - if true, returns base_value
    // 2. Otherwise evaluates step with self() for recursive calls

    // Use parameter names in order from the function call context
    let param_names = &env.current_params;

    // First evaluate condition
    let cond_result = eval_expr(condition, env)?;

    if is_truthy(&cond_result) {
        // Base case: return base_value
        eval_expr(base_value, env)
    } else {
        // Recursive case: evaluate step
        // Get current n value to check against parallel threshold (use first int param)
        let current_n = param_names
            .iter()
            .find_map(|name| {
                if let Some(Value::Int(n)) = env.get(name) {
                    Some(n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        eval_recurse_step(
            step,
            condition,
            base_value,
            env,
            memo,
            parallel_threshold,
            current_n,
            param_names,
        )
    }
}

fn eval_iterate(
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
                if params.len() >= 1 {
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

fn eval_transform(input: &Expr, steps: &[Expr], env: &Environment) -> Result<Value, String> {
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

fn eval_parallel(
    branches: &[(String, Expr)],
    _timeout: &Option<Box<Expr>>,
    on_error: &OnError,
    env: &Environment,
) -> Result<Value, String> {
    // Execute all branches concurrently using threads
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();

    // Clone what we need for threads
    let configs = env.configs.clone();
    let functions = env.functions.clone();
    let locals = env.locals.clone();

    let mut handles = Vec::new();
    let branch_count = branches.len();

    for (name, expr) in branches.iter().cloned() {
        let tx = tx.clone();
        let configs = configs.clone();
        let functions = functions.clone();
        let locals = locals.clone();
        let current_params = env.current_params.clone();

        let handle = thread::spawn(move || {
            let thread_env = Environment {
                configs,
                functions,
                locals,
                current_params,
            };
            let result = eval_expr(&expr, &thread_env);
            tx.send((name, result)).unwrap();
        });
        handles.push(handle);
    }

    // Drop the original sender so rx knows when all threads are done
    drop(tx);

    // Collect results
    let mut results: HashMap<String, Value> = HashMap::new();
    let mut first_error: Option<String> = None;

    for _ in 0..branch_count {
        match rx.recv() {
            Ok((name, Ok(value))) => {
                results.insert(name, value);
            }
            Ok((name, Err(e))) => match on_error {
                OnError::FailFast => {
                    if first_error.is_none() {
                        first_error = Some(format!("parallel branch '{}' failed: {}", name, e));
                    }
                }
                OnError::CollectAll => {
                    results.insert(name, Value::Err(Box::new(Value::String(e))));
                }
            },
            Err(_) => break,
        }
    }

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }

    if let Some(err) = first_error {
        if matches!(on_error, OnError::FailFast) {
            return Err(err);
        }
    }

    // Return as anonymous struct
    Ok(Value::Struct {
        name: "parallel".to_string(),
        fields: results,
    })
}

// ============================================================================
// Recurse Helper Functions
// ============================================================================

/// Evaluate the recursive step of a recurse pattern
/// This handles self() calls within the step expression
fn eval_recurse_step(
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
fn eval_recurse_expr(
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
                            new_env.set(param_name.clone(), arg_val.clone());
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

/// Check if an expression contains a self() call
fn contains_self_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "self" {
                    return true;
                }
            }
            args.iter().any(contains_self_call)
        }
        Expr::Binary { left, right, .. } => contains_self_call(left) || contains_self_call(right),
        Expr::Unary { operand, .. } => contains_self_call(operand),
        _ => false,
    }
}
