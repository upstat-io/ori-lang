// Parallel pattern evaluation

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use crate::ast::*;

use super::super::expr::eval_expr;
use super::super::value::{Environment, Value};

pub fn eval_parallel(
    branches: &[(String, Expr)],
    _timeout: &Option<Box<Expr>>,
    on_error: &OnError,
    env: &Environment,
) -> Result<Value, String> {
    // Execute all branches concurrently using threads
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
            // Receiver cannot be dropped before all senders complete;
            // send only fails if receiver is dropped, which doesn't happen
            // until after all threads complete
            let _ = tx.send((name, result));
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
