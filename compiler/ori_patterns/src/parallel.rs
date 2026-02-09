//! Parallel pattern implementation.
//!
//! `parallel(tasks: [...], max_concurrent: n, timeout: duration)` - Execute tasks concurrently.
//!
//! Returns `[Result<T, E>]` - all tasks run to completion, errors captured as values.

// Arc and Mutex are required for thread synchronization in parallel execution
#![expect(
    clippy::disallowed_types,
    reason = "Arc/Mutex required for thread synchronization"
)]

use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, Value};

/// The `parallel` pattern executes multiple tasks concurrently with all-settled semantics.
///
/// Syntax: `parallel(tasks: [...], max_concurrent: n, timeout: duration)`
///
/// Type: `parallel(tasks: [() -> T]) -> [Result<T, E>]`
///
/// All tasks run to completion. Errors are captured as `Err` values in the result list.
/// The pattern itself never fails - it always returns a list of results.
#[derive(Clone, Copy)]
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

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Intern property names
        let tasks_name = ctx.interner.intern("tasks");
        let timeout_name = ctx.interner.intern("timeout");
        let max_concurrent_name = ctx.interner.intern("max_concurrent");

        // Single-pass property extraction for O(n) instead of O(3n)
        let (tasks_prop, timeout_prop, max_concurrent_prop) = {
            let mut tasks = None;
            let mut timeout = None;
            let mut max_concurrent = None;
            for prop in ctx.props {
                if prop.name == tasks_name {
                    tasks = Some(prop);
                } else if prop.name == timeout_name {
                    timeout = Some(prop);
                } else if prop.name == max_concurrent_name {
                    max_concurrent = Some(prop);
                }
            }
            (tasks, timeout, max_concurrent)
        };

        // Extract .tasks property (required)
        let tasks_prop =
            tasks_prop.ok_or_else(|| EvalError::new("parallel requires .tasks property"))?;
        let tasks_value = exec.eval(tasks_prop.value)?;
        let Value::List(task_list) = tasks_value else {
            return Err(EvalError::new("parallel .tasks must be a list").into());
        };

        // Extract .timeout (optional) - convert nanoseconds to milliseconds
        let timeout_ms = timeout_prop
            .map(|p| exec.eval(p.value))
            .transpose()?
            .and_then(|v| match v {
                Value::Duration(ns) if ns > 0 => Some((ns / 1_000_000).cast_unsigned()),
                Value::Int(n) => u64::try_from(n.raw()).ok(),
                _ => None,
            });

        // Extract .max_concurrent (optional, defaults to unlimited)
        let max_concurrent = max_concurrent_prop
            .map(|p| exec.eval(p.value))
            .transpose()?
            .and_then(|v| match v {
                Value::Int(n) if n.raw() > 0 => usize::try_from(n.raw()).ok(),
                _ => None,
            });

        if task_list.is_empty() {
            return Ok(Value::list(vec![]));
        }

        // Check if tasks are callable (function_val)
        let has_callable = task_list
            .iter()
            .any(|v| matches!(v, Value::FunctionVal(_, _)));

        if has_callable && task_list.len() >= 2 {
            execute_parallel(&task_list, max_concurrent, timeout_ms)
        } else {
            // Single task or non-callable - execute sequentially.
            // Note: clone() is cheap here - Value uses Arc for heap types (O(1) ref count).
            let results: Vec<Value> = task_list.iter().map(|t| execute_task(t.clone())).collect();
            Ok(Value::list(results))
        }
    }
}

/// A simple semaphore for limiting concurrent execution.
pub struct Semaphore {
    count: Mutex<usize>,
    condvar: Condvar,
    max: usize,
}

impl Semaphore {
    /// Create a new semaphore with the given maximum concurrent count.
    pub fn new(max: usize) -> Self {
        Semaphore {
            count: Mutex::new(0),
            condvar: Condvar::new(),
            max,
        }
    }

    /// Acquire a slot from the semaphore, blocking if at capacity.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "counting semaphore bounded by max"
    )]
    pub fn acquire(&self) {
        let mut count = self
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *count >= self.max {
            count = self
                .condvar
                .wait(count)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *count += 1;
    }

    /// Release a slot back to the semaphore.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "counting semaphore bounded by max"
    )]
    pub fn release(&self) {
        let mut count = self
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *count -= 1;
        self.condvar.notify_one();
    }
}

/// Execute tasks in parallel with optional concurrency limit and timeout.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "completion counter bounded by task count"
)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "returns EvalResult to match PatternDefinition::evaluate interface"
)]
#[allow(clippy::result_large_err)] // EvalError is fundamental error type
fn execute_parallel(
    task_list: &[Value],
    max_concurrent: Option<usize>,
    timeout_ms: Option<u64>,
) -> EvalResult {
    let results = Arc::new(Mutex::new(vec![None; task_list.len()]));
    let semaphore = max_concurrent.map(|n| Arc::new(Semaphore::new(n)));

    if let Some(timeout_millis) = timeout_ms {
        // Execute with timeout
        let timeout_duration = Duration::from_millis(timeout_millis);
        let (tx, rx) = mpsc::channel();
        let results_clone = Arc::clone(&results);

        thread::scope(|s| {
            for (i, task) in task_list.iter().enumerate() {
                // Clone task for thread ownership. Cheap: Value uses Arc for heap types.
                let task = task.clone();
                let results = Arc::clone(&results_clone);
                let tx = tx.clone();
                let sem = semaphore.clone();

                s.spawn(move || {
                    // Acquire semaphore slot if concurrency is limited
                    if let Some(ref sem) = sem {
                        sem.acquire();
                    }

                    let result = execute_task(task);

                    // Release semaphore slot
                    if let Some(ref sem) = sem {
                        sem.release();
                    }

                    let mut guard = results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard[i] = Some(result);
                    drop(guard);
                    let _ = tx.send(i);
                });
            }
            drop(tx);

            // Wait for results with overall timeout
            let start = std::time::Instant::now();
            let task_count = results_clone
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len();
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
        let guard = results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
                // Clone task for thread ownership. Cheap: Value uses Arc for heap types.
                let task = task.clone();
                let results = Arc::clone(&results);
                let sem = semaphore.clone();

                s.spawn(move || {
                    // Acquire semaphore slot if concurrency is limited
                    if let Some(ref sem) = sem {
                        sem.acquire();
                    }

                    let result = execute_task(task);

                    // Release semaphore slot
                    if let Some(ref sem) = sem {
                        sem.release();
                    }

                    let mut guard = results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard[i] = Some(result);
                });
            }
        });

        let guard = results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let final_results: Vec<Value> = guard
            .iter()
            .map(|opt| {
                opt.clone()
                    .unwrap_or_else(|| Value::err(Value::string("execution failed")))
            })
            .collect();
        Ok(Value::list(final_results))
    }
}

/// Execute a single task and wrap the result in Ok/Err.
pub fn execute_task(task: Value) -> Value {
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
pub fn wrap_in_result(value: Value) -> Value {
    match value {
        Value::Ok(_) | Value::Err(_) => value,
        Value::Error(msg) => Value::err(Value::string(&msg)),
        other => Value::ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod execute_task_tests {
        use super::*;

        #[test]
        fn function_val_success_wraps_in_ok() {
            let task = Value::FunctionVal(|_| Ok(Value::Int(42.into())), "test_fn");
            let result = execute_task(task);
            assert!(matches!(result, Value::Ok(_)));
            if let Value::Ok(inner) = result {
                assert!(matches!(*inner, Value::Int(n) if n.raw() == 42));
            }
        }

        #[test]
        fn function_val_error_wraps_in_err() {
            let task = Value::FunctionVal(|_| Err("test error".to_string()), "test_fn");
            let result = execute_task(task);
            assert!(matches!(result, Value::Err(_)));
        }

        #[test]
        fn ok_passthrough() {
            let task = Value::ok(Value::Int(42.into()));
            let result = execute_task(task.clone());
            assert!(matches!(result, Value::Ok(_)));
        }

        #[test]
        fn err_passthrough() {
            let task = Value::err(Value::string("error"));
            let result = execute_task(task.clone());
            assert!(matches!(result, Value::Err(_)));
        }

        #[test]
        fn plain_value_wraps_in_ok() {
            let task = Value::Int(100.into());
            let result = execute_task(task);
            assert!(matches!(result, Value::Ok(_)));
            if let Value::Ok(inner) = result {
                assert!(matches!(*inner, Value::Int(n) if n.raw() == 100));
            }
        }
    }

    mod wrap_in_result_tests {
        use super::*;

        #[test]
        fn ok_passthrough() {
            let value = Value::ok(Value::Int(42.into()));
            let result = wrap_in_result(value);
            assert!(matches!(result, Value::Ok(_)));
        }

        #[test]
        fn err_passthrough() {
            let value = Value::err(Value::string("error"));
            let result = wrap_in_result(value);
            assert!(matches!(result, Value::Err(_)));
        }

        #[test]
        fn error_converts_to_err() {
            let value = Value::Error("some error".to_string());
            let result = wrap_in_result(value);
            assert!(matches!(result, Value::Err(_)));
        }

        #[test]
        fn plain_value_wraps_in_ok() {
            let value = Value::Bool(true);
            let result = wrap_in_result(value);
            assert!(matches!(result, Value::Ok(_)));
            if let Value::Ok(inner) = result {
                assert!(matches!(*inner, Value::Bool(true)));
            }
        }
    }
}
