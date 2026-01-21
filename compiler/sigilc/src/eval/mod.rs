// Interpreter/Evaluator for Sigil
//
// Module structure:
// - mod.rs: Public API (run, run_test, eval_line)
// - expr.rs: Expression evaluation (eval_expr, eval_block_expr)
// - calls.rs: Function call evaluation
// - match_eval.rs: Match expression evaluation
// - patterns/: Pattern evaluation (fold, map, filter, recurse, etc.)
// - value.rs: Value type, Environment
// - operators.rs: Binary and unary operator evaluation
// - builtins.rs: Builtin functions and method calls

mod builtins;
mod calls;
mod expr;
mod match_eval;
mod operators;
pub mod patterns;
pub mod value;

#[cfg(test)]
mod tests;

use crate::ast::*;

// Re-export core types for external use
pub use value::{is_truthy, Environment, Value};

// Re-export expression evaluation for pattern handlers
pub use calls::eval_function_call;
pub use expr::eval_expr;
pub use operators::eval_binary_op;

/// Internal: Initialize environment from a module (load configs and functions)
fn initialize_environment(module: &Module) -> Result<Environment, String> {
    let mut env = Environment::new();

    for item in &module.items {
        match item {
            Item::Config(cd) => {
                let value = eval_expr(&cd.value.expr, &env)?;
                env.set_config(cd.name.clone(), value);
            }
            Item::Function(fd) => {
                env.define_function(fd.name.clone(), fd.clone());
            }
            _ => {}
        }
    }

    Ok(env)
}

/// Run a module (find and execute main function)
pub fn run(module: Module) -> Result<Value, String> {
    let env = initialize_environment(&module)?;

    if let Some(main_fn) = env.get_function("main").cloned() {
        eval_expr(&main_fn.body.expr, &env)
    } else {
        Ok(Value::Nil)
    }
}

/// Run a single test
pub fn run_test(module: &Module, test: &TestDef) -> Result<(), String> {
    let env = initialize_environment(module)?;
    eval_expr(&test.body.expr, &env)?;
    Ok(())
}

/// REPL support: evaluate a single line
pub fn eval_line(input: &str, env: &mut Environment) -> Result<String, String> {
    let tokens = crate::lexer::tokenize(input, "<repl>")?;
    let module = crate::parser::parse(tokens, "<repl>")?;

    for item in module.items {
        match item {
            Item::Config(cd) => {
                let value = eval_expr(&cd.value.expr, env)?;
                env.set_config(cd.name.clone(), value.clone());
                return Ok(format!("${} = {}", cd.name, value));
            }
            Item::Function(fd) => {
                env.define_function(fd.name.clone(), fd.clone());
                return Ok(format!("@{} defined", fd.name));
            }
            _ => {}
        }
    }

    Ok(String::new())
}

/// Get type of expression (for REPL)
pub fn type_of(_expr: &str, _env: &Environment) -> Result<String, String> {
    // TODO: Implement type inference for REPL
    Ok("unknown".to_string())
}
