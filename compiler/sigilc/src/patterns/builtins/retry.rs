// Retry pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for retry semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, RetryBackoff, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, TypeContext};

/// Handler for the retry pattern.
///
/// retry(.op: expr, .times: N)
/// retry(.op: expr, .times: N, .backoff: constant, .delay: 100)
///
/// Retries an operation up to N times, optionally with backoff delays.
pub struct RetryPattern;

static RETRY_PARAMS: &[ParamSpec] = &[
    ParamSpec::required(".op", "operation to retry"),
    ParamSpec::required_with(
        ".times",
        "maximum number of attempts",
        TypeConstraint::Numeric,
    ),
    ParamSpec::optional(
        ".backoff",
        "backoff strategy: none, constant, linear, exponential",
    ),
    ParamSpec::optional_with(
        ".delay",
        "base delay in milliseconds",
        TypeConstraint::Numeric,
    ),
];

impl PatternDefinition for RetryPattern {
    fn keyword(&self) -> &'static str {
        "retry"
    }

    fn params(&self) -> &'static [ParamSpec] {
        RETRY_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Retry {
            operation,
            max_attempts,
            delay_ms,
            ..
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected retry pattern",
            ));
        };

        // Check operation type - returns the success type
        let op_type = check_expr(operation, ctx)?;

        // Check max_attempts is numeric
        let attempts_type = check_expr(max_attempts, ctx)?;
        if !matches!(attempts_type, TypeExpr::Named(ref n) if n == "int") {
            return Err(Diagnostic::error(
                ErrorCode::E3001,
                format!("retry .times must be int, got {:?}", attempts_type),
            ));
        }

        // Check delay if provided
        if let Some(delay) = delay_ms {
            let delay_type = check_expr(delay, ctx)?;
            if !matches!(delay_type, TypeExpr::Named(ref n) if n == "int") {
                return Err(Diagnostic::error(
                    ErrorCode::E3001,
                    format!("retry .delay must be int, got {:?}", delay_type),
                ));
            }
        }

        // Retry returns Result<T, Error> where T is the operation's return type
        Ok(TypeExpr::Generic(
            "Result".to_string(),
            vec![op_type, TypeExpr::Named("Error".to_string())],
        ))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Retry {
            operation,
            max_attempts,
            backoff,
            delay_ms,
        } = pattern
        else {
            return Err("expected retry pattern".to_string());
        };

        // Get max attempts
        let max = match eval_expr(max_attempts, env)? {
            Value::Int(n) => n,
            _ => return Err("retry .times must be an integer".to_string()),
        };

        if max <= 0 {
            return Err("retry .times must be positive".to_string());
        }

        // Get base delay
        let base_delay = if let Some(delay) = delay_ms {
            match eval_expr(delay, env)? {
                Value::Int(d) => d as u64,
                _ => return Err("retry .delay must be an integer".to_string()),
            }
        } else {
            0
        };

        // Attempt the operation up to max times
        let mut last_error = String::new();
        for attempt in 0..max {
            match eval_expr(operation, env) {
                Ok(value) => return Ok(Value::Ok(Box::new(value))),
                Err(e) => {
                    last_error = e;
                    // If not the last attempt, apply backoff
                    if attempt < max - 1 && base_delay > 0 {
                        let delay = calculate_delay(*backoff, base_delay, attempt as u64);
                        // In real implementation, we'd sleep here
                        // For now, just acknowledge the delay
                        #[cfg(not(test))]
                        std::thread::sleep(std::time::Duration::from_millis(delay));
                        let _ = delay; // Suppress unused warning in test mode
                    }
                }
            }
        }

        // All attempts failed
        Ok(Value::Err(Box::new(Value::String(last_error))))
    }

    fn description(&self) -> &'static str {
        "Retry an operation with configurable backoff strategy"
    }

    fn help(&self) -> &'static str {
        r#"The retry pattern attempts an operation multiple times:
  retry(.op: risky_operation, .times: 3)

With backoff delays between attempts:
  retry(.op: fetch(url), .times: 5, .backoff: exponential, .delay: 100)

Backoff strategies:
  - none: No delay between retries (default)
  - constant: Same delay each time
  - linear: delay * attempt (100, 200, 300, ...)
  - exponential: delay * 2^attempt (100, 200, 400, 800, ...)

Returns Result<T, Error> - Ok on success, Err with last error message."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "retry(.op: connect(host), .times: 3)",
            "retry(.op: fetch(url), .times: 5, .backoff: constant, .delay: 1000)",
            "retry(.op: send_request(), .times: 10, .backoff: exponential, .delay: 100)",
        ]
    }
}

/// Calculate delay based on backoff strategy
fn calculate_delay(backoff: RetryBackoff, base_delay: u64, attempt: u64) -> u64 {
    match backoff {
        RetryBackoff::None => 0,
        RetryBackoff::Constant => base_delay,
        RetryBackoff::Linear => base_delay * (attempt + 1),
        RetryBackoff::Exponential => base_delay * (1 << attempt), // 2^attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_retry_pattern_keyword() {
        let retry = RetryPattern;
        assert_eq!(retry.keyword(), "retry");
    }

    #[test]
    fn test_retry_pattern_params() {
        let retry = RetryPattern;
        let params = retry.params();
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].name, ".op");
        assert_eq!(params[1].name, ".times");
    }

    #[test]
    fn test_retry_evaluation_success() {
        let retry = RetryPattern;

        // Create retry pattern that succeeds immediately
        let pattern = PatternExpr::Retry {
            operation: Box::new(Expr::Int(42)),
            max_attempts: Box::new(Expr::Int(3)),
            backoff: RetryBackoff::None,
            delay_ms: None,
        };

        let env = Environment::new();
        let result = retry.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Ok(Box::new(Value::Int(42))));
    }

    #[test]
    fn test_calculate_delay_none() {
        assert_eq!(calculate_delay(RetryBackoff::None, 100, 0), 0);
        assert_eq!(calculate_delay(RetryBackoff::None, 100, 5), 0);
    }

    #[test]
    fn test_calculate_delay_constant() {
        assert_eq!(calculate_delay(RetryBackoff::Constant, 100, 0), 100);
        assert_eq!(calculate_delay(RetryBackoff::Constant, 100, 5), 100);
    }

    #[test]
    fn test_calculate_delay_linear() {
        assert_eq!(calculate_delay(RetryBackoff::Linear, 100, 0), 100);
        assert_eq!(calculate_delay(RetryBackoff::Linear, 100, 1), 200);
        assert_eq!(calculate_delay(RetryBackoff::Linear, 100, 2), 300);
    }

    #[test]
    fn test_calculate_delay_exponential() {
        assert_eq!(calculate_delay(RetryBackoff::Exponential, 100, 0), 100);
        assert_eq!(calculate_delay(RetryBackoff::Exponential, 100, 1), 200);
        assert_eq!(calculate_delay(RetryBackoff::Exponential, 100, 2), 400);
        assert_eq!(calculate_delay(RetryBackoff::Exponential, 100, 3), 800);
    }
}
