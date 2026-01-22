// Transform pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for transform semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{Expr, PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, eval_function_call, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition};
use crate::types::{check_expr, check_expr_with_hint, TypeContext};

/// Handler for the transform pattern.
///
/// transform(.input: value, .steps: [step1, step2, ...])
///
/// Applies a pipeline of transformations to a value.
pub struct TransformPattern;

static TRANSFORM_PARAMS: &[ParamSpec] = &[
    ParamSpec::required(".input", "initial value"),
    ParamSpec::required(".steps", "transformation steps to apply"),
];

impl PatternDefinition for TransformPattern {
    fn keyword(&self) -> &'static str {
        "transform"
    }

    fn params(&self) -> &'static [ParamSpec] {
        TRANSFORM_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Transform { input, steps } = pattern else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected transform pattern",
            ));
        };

        // Check input type
        let mut current_type = check_expr(input, ctx)?;

        // Check each step and chain types
        for step in steps {
            // Each step takes the current type as input
            let expected_lambda_type = TypeExpr::Function(
                Box::new(current_type.clone()),
                Box::new(TypeExpr::Named("_infer_".to_string())),
            );

            let step_type = check_expr_with_hint(step, ctx, Some(&expected_lambda_type))?;

            if let TypeExpr::Function(_, ret) = step_type {
                current_type = *ret;
            }
        }

        Ok(current_type)
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Transform { input, steps } = pattern else {
            return Err("expected transform pattern".to_string());
        };

        // Transform passes a value through a series of transformation steps
        let mut value = eval_expr(input, env)?;

        for step_expr in steps {
            // Each step can be a function or a lambda
            let step_val = eval_expr(step_expr, env)?;
            value = match step_val {
                Value::Function {
                    params,
                    body,
                    env: fn_env,
                } => {
                    let mut call_env = Environment {
                        configs: env.configs.clone(),
                        current_params: env.current_params.clone(),
                        functions: env.functions.clone(),
                        locals: Environment::locals_from_values(fn_env.clone()),
                    };
                    if let Some(param) = params.first() {
                        call_env.define(param.clone(), value.clone(), false);
                    }
                    // Also bind 'x' as common transform variable
                    call_env.define(
                        "x".to_string(),
                        call_env
                            .get(params.first().unwrap_or(&"x".to_string()))
                            .unwrap_or(value.clone()),
                        false,
                    );
                    eval_expr(&body, &call_env)?
                }
                _ => {
                    // If it's an identifier, try to call it as a function
                    if let Expr::Ident(name) = step_expr {
                        if let Some(fd) = env.get_function(name).cloned() {
                            eval_function_call(&fd, vec![value], env)?
                        } else {
                            return Err(format!("Unknown transform function: {}", name));
                        }
                    } else {
                        return Err("Transform step must be a function".to_string());
                    }
                }
            };
        }
        Ok(value)
    }

    fn description(&self) -> &'static str {
        "Apply a pipeline of transformations to a value"
    }

    fn help(&self) -> &'static str {
        r#"The transform pattern chains multiple transformations:
  transform(.input: value, .steps: [step1, step2, step3])

Each step is a function that takes the current value and returns a new value.
The result of each step becomes the input to the next step."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "transform(.input: \"hello\", .steps: [to_upper, reverse])",
            "transform(.input: 5, .steps: [x -> x * 2, x -> x + 1])",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::BinaryOp;

    #[test]
    fn test_transform_pattern_keyword() {
        let transform = TransformPattern;
        assert_eq!(transform.keyword(), "transform");
    }

    #[test]
    fn test_transform_pattern_params() {
        let transform = TransformPattern;
        let params = transform.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".input");
        assert_eq!(params[1].name, ".steps");
    }

    #[test]
    fn test_transform_evaluation() {
        let transform = TransformPattern;

        // Create transform pattern: transform(.input: 5, .steps: [x -> x * 2, x -> x + 1])
        let pattern = PatternExpr::Transform {
            input: Box::new(Expr::Int(5)),
            steps: vec![
                Expr::Lambda {
                    params: vec!["x".to_string()],
                    body: Box::new(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr::Ident("x".to_string())),
                        right: Box::new(Expr::Int(2)),
                    }),
                },
                Expr::Lambda {
                    params: vec!["x".to_string()],
                    body: Box::new(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Ident("x".to_string())),
                        right: Box::new(Expr::Int(1)),
                    }),
                },
            ],
        };

        let env = Environment::new();
        let result = transform.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(11)); // 5 * 2 + 1 = 11
    }
}
