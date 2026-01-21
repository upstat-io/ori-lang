// Validate pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for validate semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, TypeContext};

/// Handler for the validate pattern.
///
/// validate(.rules: [ cond | "error", ... ], .then: value)
///
/// Runs ALL validation rules and accumulates errors.
/// Returns Result<T, [str]> where [str] contains all error messages.
pub struct ValidatePattern;

static VALIDATE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(
        ".rules",
        "list of (condition, error_message) pairs",
        TypeConstraint::List,
    ),
    ParamSpec::required(".then", "value to return if all validations pass"),
];

impl PatternDefinition for ValidatePattern {
    fn keyword(&self) -> &'static str {
        "validate"
    }

    fn params(&self) -> &'static [ParamSpec] {
        VALIDATE_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Validate { rules, then_value } = pattern else {
            return Err(Diagnostic::error(ErrorCode::E3009, "expected validate pattern"));
        };

        // Validate each rule
        for (i, (condition, message)) in rules.iter().enumerate() {
            // Condition must be a boolean
            let cond_type = check_expr(condition, ctx)
                .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;
            if !matches!(cond_type, TypeExpr::Named(ref s) if s == "bool") {
                return Err(Diagnostic::error(
                    ErrorCode::E3002,
                    format!("validate rule {} condition must be bool, got {:?}", i + 1, cond_type),
                ));
            }

            // Message must be a string
            let msg_type = check_expr(message, ctx)
                .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;
            if !matches!(msg_type, TypeExpr::Named(ref s) if s == "str") {
                return Err(Diagnostic::error(
                    ErrorCode::E3002,
                    format!("validate rule {} message must be str, got {:?}", i + 1, msg_type),
                ));
            }
        }

        // Check the then_value type
        let then_type = check_expr(then_value, ctx)
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // Return type is Result<T, [str]>
        Ok(TypeExpr::Generic(
            "Result".to_string(),
            vec![then_type, TypeExpr::List(Box::new(TypeExpr::Named("str".to_string())))],
        ))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Validate { rules, then_value } = pattern else {
            return Err("expected validate pattern".to_string());
        };

        // Collect all validation errors (don't short-circuit)
        let mut errors: Vec<String> = Vec::new();

        for (condition, message) in rules {
            // Evaluate the condition
            let cond_result = eval_expr(condition, env)?;
            match cond_result {
                Value::Bool(true) => {
                    // Validation passed, continue
                }
                Value::Bool(false) => {
                    // Validation failed, collect the error message
                    let msg = eval_expr(message, env)?;
                    match msg {
                        Value::String(s) => errors.push(s),
                        _ => return Err("validation message must evaluate to string".to_string()),
                    }
                }
                _ => return Err("validation condition must evaluate to bool".to_string()),
            }
        }

        if errors.is_empty() {
            // All validations passed, return Ok(then_value)
            let value = eval_expr(then_value, env)?;
            Ok(Value::Ok(Box::new(value)))
        } else {
            // Some validations failed, return Err(errors)
            let error_list = Value::List(errors.into_iter().map(Value::String).collect());
            Ok(Value::Err(Box::new(error_list)))
        }
    }

    fn description(&self) -> &'static str {
        "Validate input with error accumulation - runs ALL rules and collects errors"
    }

    fn help(&self) -> &'static str {
        r#"The validate pattern runs all validation rules and accumulates errors:
  validate(.rules: [...], .then: success_value)

Each rule is a (condition, error_message) pair using the `|` syntax:
  validate(
      .rules: [
          age >= 0 | "Age cannot be negative",
          age <= 150 | "Age cannot exceed 150",
          name.len > 0 | "Name cannot be empty",
      ],
      .then: User { name, age },
  )

If all conditions pass, returns Ok(success_value).
If any conditions fail, returns Err([list of error messages]).

Unlike short-circuit validation, this pattern ALWAYS runs all rules,
collecting all error messages for better user feedback."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            r#"validate(
    .rules: [
        x > 0 | "Must be positive",
        x < 100 | "Must be less than 100",
    ],
    .then: x,
)"#,
            r#"validate(
    .rules: [
        user.email.contains("@") | "Invalid email format",
        user.age >= 18 | "Must be 18 or older",
    ],
    .then: create_account(user),
)"#,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_validate_pattern_keyword() {
        let validate_pat = ValidatePattern;
        assert_eq!(validate_pat.keyword(), "validate");
    }

    #[test]
    fn test_validate_pattern_params() {
        let validate_pat = ValidatePattern;
        let params = validate_pat.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".rules");
        assert_eq!(params[1].name, ".then");
    }

    #[test]
    fn test_validate_all_pass() {
        let validate_pat = ValidatePattern;

        // Create validate pattern where all rules pass
        let pattern = PatternExpr::Validate {
            rules: vec![
                (Expr::Bool(true), Expr::String("error1".to_string())),
                (Expr::Bool(true), Expr::String("error2".to_string())),
            ],
            then_value: Box::new(Expr::Int(42)),
        };

        let env = Environment::new();
        let result = validate_pat.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Ok(Box::new(Value::Int(42))));
    }

    #[test]
    fn test_validate_some_fail() {
        let validate_pat = ValidatePattern;

        // Create validate pattern where some rules fail
        let pattern = PatternExpr::Validate {
            rules: vec![
                (Expr::Bool(false), Expr::String("first error".to_string())),
                (Expr::Bool(true), Expr::String("not shown".to_string())),
                (Expr::Bool(false), Expr::String("second error".to_string())),
            ],
            then_value: Box::new(Expr::Int(42)),
        };

        let env = Environment::new();
        let result = validate_pat.evaluate(&pattern, &env).unwrap();

        // Should collect both error messages
        let expected_errors = Value::List(vec![
            Value::String("first error".to_string()),
            Value::String("second error".to_string()),
        ]);
        assert_eq!(result, Value::Err(Box::new(expected_errors)));
    }

    #[test]
    fn test_validate_all_fail() {
        let validate_pat = ValidatePattern;

        // Create validate pattern where all rules fail
        let pattern = PatternExpr::Validate {
            rules: vec![
                (Expr::Bool(false), Expr::String("error 1".to_string())),
                (Expr::Bool(false), Expr::String("error 2".to_string())),
            ],
            then_value: Box::new(Expr::Int(42)),
        };

        let env = Environment::new();
        let result = validate_pat.evaluate(&pattern, &env).unwrap();

        let expected_errors = Value::List(vec![
            Value::String("error 1".to_string()),
            Value::String("error 2".to_string()),
        ]);
        assert_eq!(result, Value::Err(Box::new(expected_errors)));
    }
}
