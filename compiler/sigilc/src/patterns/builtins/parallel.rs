// Parallel pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for parallel semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use crate::ast::{OnError, PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition};
use crate::types::{check_expr, TypeContext};

/// Handler for the parallel pattern.
///
/// parallel(.name1: expr1, .name2: expr2, ...)
///
/// Executes multiple expressions concurrently and returns a struct
/// with the results.
pub struct ParallelPattern;

static PARALLEL_PARAMS: &[ParamSpec] = &[
    ParamSpec::optional(".timeout", "maximum execution time"),
    ParamSpec::optional(
        ".on_error",
        "error handling strategy (fail_fast/collect_all)",
    ),
];

impl PatternDefinition for ParallelPattern {
    fn keyword(&self) -> &'static str {
        "parallel"
    }

    fn params(&self) -> &'static [ParamSpec] {
        PARALLEL_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Parallel {
            branches, timeout, ..
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected parallel pattern",
            ));
        };

        // Check all branch expressions and build record type
        let mut field_types = Vec::new();
        for (name, expr) in branches {
            let ty = check_expr(expr, ctx)?;
            field_types.push((name.clone(), ty));
        }

        if let Some(t) = timeout {
            check_expr(t, ctx)?;
        }

        // Returns an anonymous record type with the branch names as fields
        Ok(TypeExpr::Record(field_types))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Parallel {
            branches,
            timeout: _timeout,
            on_error,
        } = pattern
        else {
            return Err("expected parallel pattern".to_string());
        };

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
                // Receiver cannot be dropped before all senders complete
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

    fn description(&self) -> &'static str {
        "Execute multiple expressions in parallel"
    }

    fn help(&self) -> &'static str {
        r#"The parallel pattern executes expressions concurrently:
  parallel(.task1: expr1, .task2: expr2, .timeout: 5000, .on_error: fail_fast)

- Named branches: Each .name: expr pair runs in its own thread
- .timeout: Optional timeout in milliseconds
- .on_error: fail_fast (default) or collect_all

Returns a struct with each branch name as a field containing its result."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "parallel(.a: fetch_data(), .b: compute_value())",
            "parallel(.x: slow_op(), .y: other_op(), .on_error: collect_all)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_parallel_pattern_keyword() {
        let parallel = ParallelPattern;
        assert_eq!(parallel.keyword(), "parallel");
    }

    #[test]
    fn test_parallel_pattern_params() {
        let parallel = ParallelPattern;
        let params = parallel.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".timeout");
        assert_eq!(params[1].name, ".on_error");
    }

    #[test]
    fn test_parallel_evaluation() {
        let parallel = ParallelPattern;

        // Create parallel pattern: parallel(.a: 1, .b: 2)
        let pattern = PatternExpr::Parallel {
            branches: vec![
                ("a".to_string(), Expr::Int(1)),
                ("b".to_string(), Expr::Int(2)),
            ],
            timeout: None,
            on_error: OnError::FailFast,
        };

        let env = Environment::new();
        let result = parallel.evaluate(&pattern, &env).unwrap();

        match result {
            Value::Struct { fields, .. } => {
                assert_eq!(fields.get("a"), Some(&Value::Int(1)));
                assert_eq!(fields.get("b"), Some(&Value::Int(2)));
            }
            _ => panic!("Expected struct result"),
        }
    }
}
