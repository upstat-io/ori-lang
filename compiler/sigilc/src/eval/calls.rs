// Function call evaluation for Sigil

use crate::ast::FunctionDef;
use std::collections::HashMap;

use super::expr::eval_expr;
use super::value::{Environment, Value};

/// Evaluate a function call
pub fn eval_function_call(
    fd: &FunctionDef,
    args: Vec<Value>,
    env: &Environment,
) -> Result<Value, String> {
    // Capture parameter names in order for recursion support
    let param_names: Vec<String> = fd.params.iter().map(|p| p.name.clone()).collect();

    let mut new_env = Environment {
        configs: env.configs.clone(),
        functions: env.functions.clone(),
        locals: HashMap::new(),      // Start with fresh locals
        current_params: param_names, // Store param names in order
    };

    for (param, value) in fd.params.iter().zip(args.into_iter()) {
        // Parameters are immutable bindings
        new_env.define(param.name.clone(), value, false);
    }

    eval_expr(&fd.body.expr, &new_env)
}
