//! Spawn pattern implementation.
//!
//! `spawn(.tasks: [...], .max_concurrent: n)` - Fire and forget concurrent execution.
//!
//! Returns `void` - tasks are started but not awaited, errors are discarded.

use std::thread;
use crate::types::Type;
use crate::eval::{Value, EvalResult};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `spawn` pattern executes tasks concurrently without waiting for results.
///
/// Syntax: `spawn(.tasks: [...], .max_concurrent: n)`
///
/// Type: `spawn(.tasks: [() -> T]) -> void`
///
/// Tasks are started but not awaited. Errors are silently discarded.
/// Use for fire-and-forget side effects where results don't matter.
pub struct SpawnPattern;

impl PatternDefinition for SpawnPattern {
    fn name(&self) -> &'static str {
        "spawn"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["tasks"]
    }

    fn allows_arbitrary_props(&self) -> bool {
        false
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        // spawn always returns unit (void)
        Type::Unit
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        // Intern property names
        let tasks_name = ctx.interner.intern("tasks");
        let max_concurrent_name = ctx.interner.intern("max_concurrent");

        // Extract .tasks property (required)
        let tasks_prop = ctx.props.iter()
            .find(|p| p.name == tasks_name)
            .ok_or_else(|| crate::eval::EvalError::new("spawn requires .tasks property"))?;

        let tasks_value = exec.eval(tasks_prop.value)?;
        let task_list = match tasks_value {
            Value::List(items) => items,
            _ => return Err(crate::eval::EvalError::new("spawn .tasks must be a list")),
        };

        // Extract .max_concurrent (optional)
        let _max_concurrent = ctx.props.iter()
            .find(|p| p.name == max_concurrent_name)
            .map(|p| exec.eval(p.value))
            .transpose()?
            .and_then(|v| match v {
                Value::Int(n) if n > 0 => Some(n as usize),
                _ => None,
            });
        // Note: max_concurrent is parsed but not yet enforced in this simple impl

        if task_list.is_empty() {
            return Ok(Value::Void);
        }

        // Fire and forget - spawn threads and don't wait
        // Note: In a real implementation, we'd use a thread pool or async runtime
        // For now, we use scoped threads which will wait, but discard results
        thread::scope(|s| {
            for task in task_list.iter() {
                let task = task.clone();
                s.spawn(move || {
                    // Execute task, discard result (fire and forget)
                    let _ = execute_task_silently(task);
                });
            }
        });

        Ok(Value::Void)
    }
}

/// Execute a task and discard the result.
fn execute_task_silently(task: Value) {
    match task {
        Value::FunctionVal(func, _) => {
            let _ = func(&[]);
        }
        _ => {
            // Non-callable values are no-ops in spawn
        }
    }
}
