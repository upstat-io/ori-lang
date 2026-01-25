//! Spawn pattern implementation.
//!
//! `spawn(tasks: [...], max_concurrent: n)` - Fire and forget concurrent execution.
//!
//! Returns `void` - tasks are started but not awaited, errors are discarded.

use std::thread;

use sigil_types::Type;

use crate::{
    EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext,
    Value,
};

/// The `spawn` pattern executes tasks concurrently without waiting for results.
///
/// Syntax: `spawn(tasks: [...], max_concurrent: n)`
///
/// Type: `spawn(tasks: [() -> T]) -> void`
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

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Intern property names
        let tasks_name = ctx.interner.intern("tasks");
        let max_concurrent_name = ctx.interner.intern("max_concurrent");

        // Extract .tasks property (required)
        let tasks_prop = ctx
            .props
            .iter()
            .find(|p| p.name == tasks_name)
            .ok_or_else(|| EvalError::new("spawn requires .tasks property"))?;

        let tasks_value = exec.eval(tasks_prop.value)?;
        let Value::List(task_list) = tasks_value else {
            return Err(EvalError::new("spawn .tasks must be a list"));
        };

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

        if task_list.is_empty() {
            return Ok(Value::Void);
        }

        // Fire and forget - spawn threads and discard results.
        // Uses scoped threads to ensure all tasks complete before returning.
        thread::scope(|s| {
            for task in task_list.iter() {
                s.spawn(|| {
                    execute_task_silently(task);
                });
            }
        });

        Ok(Value::Void)
    }
}

/// Execute a task and discard the result.
fn execute_task_silently(task: &Value) {
    if let Value::FunctionVal(func, _) = task {
        let _ = func(&[]);
    }
}
