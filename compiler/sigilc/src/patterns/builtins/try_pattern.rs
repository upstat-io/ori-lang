// Try pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for try semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, TypeContext};

/// Handler for the try pattern.
///
/// try(.body: expr)
/// try(.body: expr, .catch: (err) -> T)
///
/// Wraps an expression in error handling, returning Result<T, Error>.
/// With .catch handler, catches errors and returns T instead.
pub struct TryPattern;

static TRY_PARAMS: &[ParamSpec] = &[
    ParamSpec::required(".body", "expression to try"),
    ParamSpec::optional_with(
        ".catch",
        "error handler (err) -> T",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for TryPattern {
    fn keyword(&self) -> &'static str {
        "try"
    }

    fn params(&self) -> &'static [ParamSpec] {
        TRY_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Try { body, catch } = pattern else {
            return Err(Diagnostic::error(ErrorCode::E3009, "expected try pattern"));
        };

        // Check body type
        let body_type = check_expr(body, ctx)
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // If catch handler is provided, verify it and return its result type
        if let Some(catch_expr) = catch {
            // Catch handler should return same type as body for unified result
            let expected_catch_type = TypeExpr::Function(
                Box::new(TypeExpr::Named("Error".to_string())),
                Box::new(body_type.clone()),
            );

            check_expr_with_hint(catch_expr, ctx, Some(&expected_catch_type)).map_err(|msg| {
                Diagnostic::error(ErrorCode::E3001, msg)
            })?;

            // With catch, returns body type directly (errors are handled)
            Ok(body_type)
        } else {
            // Without catch, wrap in Result
            Ok(TypeExpr::Generic(
                "Result".to_string(),
                vec![body_type, TypeExpr::Named("Error".to_string())],
            ))
        }
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Try { body, catch } = pattern else {
            return Err("expected try pattern".to_string());
        };

        // Try to evaluate the body
        match eval_expr(body, env) {
            Ok(value) => {
                if catch.is_some() {
                    // With catch, return value directly
                    Ok(value)
                } else {
                    // Without catch, wrap in Ok
                    Ok(Value::Ok(Box::new(value)))
                }
            }
            Err(error_msg) => {
                if let Some(catch_expr) = catch {
                    // Call the catch handler with the error
                    let catch_val = eval_expr(catch_expr, env)?;
                    match catch_val {
                        Value::Function {
                            params,
                            body: catch_body,
                            env: fn_env,
                        } => {
                            if params.len() != 1 {
                                return Err("catch handler must take 1 argument".to_string());
                            }
                            let mut call_env = Environment {
                                configs: env.configs.clone(),
                                current_params: env.current_params.clone(),
                                functions: env.functions.clone(),
                                locals: Environment::locals_from_values(fn_env),
                            };
                            call_env.define(
                                params[0].clone(),
                                Value::String(error_msg),
                                false,
                            );
                            eval_expr(&catch_body, &call_env)
                        }
                        _ => Err("catch must be a function".to_string()),
                    }
                } else {
                    // Without catch, wrap in Err
                    Ok(Value::Err(Box::new(Value::String(error_msg))))
                }
            }
        }
    }

    fn description(&self) -> &'static str {
        "Wrap an expression in error handling, returning Result<T, Error>"
    }

    fn help(&self) -> &'static str {
        r#"The try pattern provides structured error handling:
  try(.body: expression)

Returns Result<T, Error> - Ok(value) on success, Err(error) on failure.

With .catch handler, catches errors and returns a fallback value:
  try(.body: risky_operation, .catch: err -> fallback_value)

The catch variant returns T directly instead of Result<T, Error>."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "try(.body: parse_int(user_input))",
            "try(.body: fetch(url), .catch: err -> default_response)",
            "try(.body: config.get(key), .catch: _ -> \"default\")",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_try_pattern_keyword() {
        let try_pat = TryPattern;
        assert_eq!(try_pat.keyword(), "try");
    }

    #[test]
    fn test_try_pattern_params() {
        let try_pat = TryPattern;
        let params = try_pat.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".body");
        assert_eq!(params[1].name, ".catch");
    }

    #[test]
    fn test_try_evaluation_success() {
        let try_pat = TryPattern;

        // Create try pattern: try(.body: 42)
        let pattern = PatternExpr::Try {
            body: Box::new(Expr::Int(42)),
            catch: None,
        };

        let env = Environment::new();
        let result = try_pat.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Ok(Box::new(Value::Int(42))));
    }

    #[test]
    fn test_try_evaluation_success_with_catch() {
        let try_pat = TryPattern;

        // Create try pattern: try(.body: 42, .catch: _ -> 0)
        let pattern = PatternExpr::Try {
            body: Box::new(Expr::Int(42)),
            catch: Some(Box::new(Expr::Lambda {
                params: vec!["_".to_string()],
                body: Box::new(Expr::Int(0)),
            })),
        };

        let env = Environment::new();
        let result = try_pat.evaluate(&pattern, &env).unwrap();
        // With catch, success returns value directly (not wrapped in Ok)
        assert_eq!(result, Value::Int(42));
    }
}
