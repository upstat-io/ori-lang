// Recurse pattern evaluation
// Entry point for recursive pattern evaluation

mod analysis;
mod step;

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{is_truthy, Environment, Value};

pub use step::eval_recurse_step;

pub fn eval_recurse(
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
