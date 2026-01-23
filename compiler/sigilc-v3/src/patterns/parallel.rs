//! Parallel pattern implementation.
//!
//! `parallel(.task1: expr1, .task2: expr2, ...)` - Execute tasks concurrently.

use std::rc::Rc;
use crate::types::Type;
use crate::eval::{Value, EvalResult};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `parallel` pattern executes multiple tasks concurrently.
///
/// Syntax: `parallel(.task1: expr1, .task2: expr2, ...)`
///
/// Type: `parallel(.task1: T1, .task2: T2, ...) -> (T1, T2, ...)`
///
/// Note: In the interpreter, tasks run sequentially. True parallelism
/// is implemented in the compiled output.
pub struct ParallelPattern;

impl PatternDefinition for ParallelPattern {
    fn name(&self) -> &'static str {
        "parallel"
    }

    fn required_props(&self) -> &'static [&'static str] {
        // No fixed required props - accepts arbitrary task names
        &[]
    }

    fn allows_arbitrary_props(&self) -> bool {
        true
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // parallel(.task1: T1, .task2: T2, ...) -> (T1, T2, ...)
        // Collect all property types into a tuple
        let types: Vec<Type> = ctx.prop_types.values().cloned().collect();
        Type::Tuple(types)
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        // Execute all tasks (sequentially in interpreter)
        let mut results = Vec::new();
        for prop in ctx.props {
            let value = exec.eval(prop.value)?;
            results.push(value);
        }
        Ok(Value::Tuple(Rc::new(results)))
    }

}
