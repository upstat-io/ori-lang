//! Spawn pattern implementation.
//!
//! `spawn(tasks: [...], max_concurrent: n)` - Fire and forget concurrent execution.
//!
//! Returns `void` - tasks are started but not awaited, errors are discarded.

use std::thread;

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, Value};

#[cfg(test)]
use crate::test_helpers::MockPatternExecutor;

/// The `spawn` pattern executes tasks concurrently without waiting for results.
///
/// Syntax: `spawn(tasks: [...], max_concurrent: n)`
///
/// Type: `spawn(tasks: [() -> T]) -> void`
///
/// Tasks are started but not awaited. Errors are silently discarded.
/// Use for fire-and-forget side effects where results don't matter.
#[derive(Clone, Copy)]
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
                Value::Int(n) => usize::try_from(n.raw()).ok(),
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

#[cfg(test)]
// Tests use unwrap() to panic on unexpected state, making failures immediately visible
#[allow(clippy::unwrap_used, clippy::default_trait_access)]
mod tests {
    use super::*;
    use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

    fn make_ctx<'a>(
        interner: &'a SharedInterner,
        arena: &'a ExprArena,
        props: &'a [NamedExpr],
    ) -> EvalContext<'a> {
        EvalContext::new(interner, arena, props)
    }

    #[test]
    fn spawn_empty_list_returns_void() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let tasks_name = interner.intern("tasks");
        let props = vec![NamedExpr {
            name: tasks_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::list(vec![]));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = SpawnPattern.evaluate(&ctx, &mut exec).unwrap();

        assert!(matches!(result, Value::Void));
    }

    #[test]
    fn spawn_pattern_name() {
        assert_eq!(SpawnPattern.name(), "spawn");
    }

    #[test]
    fn spawn_required_props() {
        assert_eq!(SpawnPattern.required_props(), &["tasks"]);
    }

    #[test]
    fn spawn_does_not_allow_arbitrary_props() {
        assert!(!SpawnPattern.allows_arbitrary_props());
    }

    #[test]
    fn spawn_requires_list_for_tasks() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let tasks_name = interner.intern("tasks");
        let props = vec![NamedExpr {
            name: tasks_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        // Non-list value for tasks
        let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::int(42));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = SpawnPattern.evaluate(&ctx, &mut exec);

        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must be a list"));
    }
}
