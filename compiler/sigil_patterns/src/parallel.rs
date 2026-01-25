//! Parallel pattern implementation.
//!
//! `parallel(tasks: [...], max_concurrent: n, timeout: duration)` - Execute tasks concurrently.
//!
//! Returns `[Result<T, E>]` - all tasks run to completion, errors captured as values.

// Arc and Mutex are required for thread synchronization in parallel execution
#![expect(clippy::disallowed_types, reason = "Arc/Mutex required for thread synchronization")]

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use sigil_types::Type;

use crate::{
    EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext,
    Value,
};

/// The `parallel` pattern executes multiple tasks concurrently with all-settled semantics.
///
/// Syntax: `parallel(tasks: [...], max_concurrent: n, timeout: duration)`
///
/// Type: `parallel(tasks: [() -> T]) -> [Result<T, E>]`
///
/// All tasks run to completion. Errors are captured as `Err` values in the result list.
/// The pattern itself never fails - it always returns a list of results.
pub struct ParallelPattern;

impl PatternDefinition for ParallelPattern {
    fn name(&self) -> &'static str {
        "parallel"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["tasks"]
    }

    fn allows_arbitrary_props(&self) -> bool {
        false
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // parallel(tasks: [() -> T]) -> [Result<T, E>]
        // Get the element type from tasks and wrap in Result, then List
        let tasks_type = ctx.require_prop_type("tasks");
        if let Type::List(elem_type) = tasks_type {
            // elem_type is () -> T, we want Result<T, Error>
            let result_type = if let Type::Function { ret, .. } = elem_type.as_ref() {
                ctx.result_of(*ret.clone(), Type::Error)
            } else {
                ctx.result_of(*elem_type.clone(), Type::Error)
            };
            return ctx.list_of(result_type);
        }
        // Fallback: [Result<Unit, Error>]
        ctx.list_of(ctx.result_of(Type::Unit, Type::Error))
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Intern property names
        let tasks_name = ctx.interner.intern("tasks");
        let timeout_name = ctx.interner.intern("timeout");
        let max_concurrent_name = ctx.interner.intern("max_concurrent");

        // Extract .tasks property (required)
        let tasks_prop = ctx
            .props
            .iter()
            .find(|p| p.name == tasks_name)
            .ok_or_else(|| EvalError::new("parallel requires .tasks property"))?;

        let tasks_value = exec.eval(tasks_prop.value)?;
        let Value::List(task_list) = tasks_value else {
            return Err(EvalError::new("parallel .tasks must be a list"));
        };

        // Extract .timeout (optional, per-task)
        let timeout_ms = ctx
            .props
            .iter()
            .find(|p| p.name == timeout_name)
            .map(|p| exec.eval(p.value))
            .transpose()?
            .and_then(|v| match v {
                Value::Duration(ms) => Some(ms),
                Value::Int(n) => u64::try_from(n).ok(),
                _ => None,
            });

        // Extract .max_concurrent (optional)
        let _max_concurrent = ctx
            .props
            .iter()
            .find(|p| p.name == max_concurrent_name)
            .map(|p| exec.eval(p.value))
            .transpose()?
            .and_then(|v| match v {
                Value::Int(n) => usize::try_from(n).ok(),
                _ => None,
            });
        // Note: max_concurrent is parsed but not yet enforced in this simple impl

        if task_list.is_empty() {
            return Ok(Value::list(vec![]));
        }

        // Check if tasks are callable (function_val)
        let has_callable = task_list
            .iter()
            .any(|v| matches!(v, Value::FunctionVal(_, _)));

        if has_callable && task_list.len() >= 2 {
            // Execute in parallel threads
            let results = Arc::new(Mutex::new(vec![None; task_list.len()]));

            if let Some(timeout_millis) = timeout_ms {
                // Execute with per-task timeout
                let timeout_duration = Duration::from_millis(timeout_millis);
                let (tx, rx) = mpsc::channel();
                let results_clone = Arc::clone(&results);

                thread::scope(|s| {
                    for (i, task) in task_list.iter().enumerate() {
                        let task = task.clone();
                        let results = Arc::clone(&results_clone);
                        let tx = tx.clone();
                        s.spawn(move || {
                            let result = execute_task(task);
                            let mut guard = results.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            guard[i] = Some(result);
                            drop(guard);
                            let _ = tx.send(i);
                        });
                    }
                    drop(tx);

                    // Wait for results with overall timeout
                    let start = std::time::Instant::now();
                    let task_count = results_clone.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
                    let mut completed = 0;

                    while completed < task_count {
                        let remaining = timeout_duration.saturating_sub(start.elapsed());
                        if remaining.is_zero() {
                            break;
                        }
                        match rx.recv_timeout(remaining) {
                            Ok(_) => completed += 1,
                            Err(_) => break,
                        }
                    }
                });

                // Build results - timed out tasks get Err(TimeoutError)
                let guard = results.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let final_results: Vec<Value> = guard
                    .iter()
                    .map(|opt| match opt {
                        Some(v) => v.clone(),
                        None => Value::err(Value::string("TimeoutError")),
                    })
                    .collect();
                Ok(Value::list(final_results))
            } else {
                // No timeout - execute all tasks
                thread::scope(|s| {
                    for (i, task) in task_list.iter().enumerate() {
                        let task = task.clone();
                        let results = Arc::clone(&results);
                        s.spawn(move || {
                            let result = execute_task(task);
                            let mut guard = results.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            guard[i] = Some(result);
                        });
                    }
                });

                let guard = results.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let final_results: Vec<Value> = guard
                    .iter()
                    .map(|opt| {
                        opt.clone()
                            .unwrap_or_else(|| Value::err(Value::string("execution failed")))
                    })
                    .collect();
                Ok(Value::list(final_results))
            }
        } else {
            // Single task or non-callable - execute sequentially
            let results: Vec<Value> = task_list.iter().map(|t| execute_task(t.clone())).collect();
            Ok(Value::list(results))
        }
    }
}

/// Execute a single task and wrap the result in Ok/Err.
fn execute_task(task: Value) -> Value {
    match task {
        Value::FunctionVal(func, _) => match func(&[]) {
            Ok(v) => wrap_in_result(v),
            Err(e) => Value::err(Value::string(e.clone())),
        },
        // If task is already a Result, keep it
        Value::Ok(_) | Value::Err(_) => task,
        // Otherwise wrap the value in Ok
        other => Value::ok(other),
    }
}

/// Wrap a value in Ok, unless it's already a Result.
fn wrap_in_result(value: Value) -> Value {
    match value {
        Value::Ok(_) | Value::Err(_) => value,
        Value::Error(msg) => Value::err(Value::string(&msg)),
        other => Value::ok(other),
    }
}
