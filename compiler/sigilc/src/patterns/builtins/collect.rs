// Collect pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for collect semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{Expr, PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, eval_function_call, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_function_return_type, TypeContext};

/// Handler for the collect pattern.
///
/// collect(.range: range, .into: transform)
///
/// Builds a list by applying a transform to each element of a range.
pub struct CollectPattern;

static COLLECT_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".range", "range to iterate over", TypeConstraint::Iterable),
    ParamSpec::required_with(
        ".into",
        "transformation function (i) -> elem",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for CollectPattern {
    fn keyword(&self) -> &'static str {
        "collect"
    }

    fn params(&self) -> &'static [ParamSpec] {
        COLLECT_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Collect { range, transform } = pattern else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected collect pattern",
            ));
        };

        // Check range type
        check_expr(range, ctx)?;

        // Collect iterates over a range (integers)
        let expected_lambda_type = TypeExpr::Function(
            Box::new(TypeExpr::Named("int".to_string())),
            Box::new(TypeExpr::Named("_infer_".to_string())),
        );

        let transform_type = check_expr_with_hint(transform, ctx, Some(&expected_lambda_type))?;

        let elem_type = get_function_return_type(&transform_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!(
                    "collect transform must be a function, got {:?}",
                    transform_type
                ),
            )
        })?;

        Ok(TypeExpr::List(Box::new(elem_type)))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Collect { range, transform } = pattern else {
            return Err("expected collect pattern".to_string());
        };

        let range_val = eval_expr(range, env)?;
        let transform_val = eval_expr(transform, env)?;

        let items = match range_val {
            Value::List(items) => items,
            _ => return Err("collect requires a range/list".to_string()),
        };

        let mut results = Vec::new();
        for item in items {
            let result = match &transform_val {
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
                        call_env.define(param.clone(), item, false);
                    }
                    eval_expr(body, &call_env)?
                }
                _ => {
                    if let Expr::Ident(name) = transform.as_ref() {
                        if let Some(fd) = env.get_function(name).cloned() {
                            eval_function_call(&fd, vec![item], env)?
                        } else {
                            return Err(format!("Unknown function: {}", name));
                        }
                    } else {
                        return Err("collect transform must be a function".to_string());
                    }
                }
            };
            results.push(result);
        }
        Ok(Value::List(results))
    }

    fn description(&self) -> &'static str {
        "Build a list by applying a transform to each element of a range"
    }

    fn help(&self) -> &'static str {
        r#"The collect pattern builds a list from a range:
  collect(.range: 0..10, .into: (i) -> i * 2)

The transform function receives each index and returns the element.
Useful for generating sequences based on indices."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "collect(.range: 0..5, .into: i -> i * i)",
            "collect(.range: 1..10, .into: i -> \"item_\" ++ str(i))",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_pattern_keyword() {
        let collect = CollectPattern;
        assert_eq!(collect.keyword(), "collect");
    }

    #[test]
    fn test_collect_pattern_params() {
        let collect = CollectPattern;
        let params = collect.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".range");
        assert_eq!(params[1].name, ".into");
    }

    #[test]
    fn test_collect_evaluation() {
        let collect = CollectPattern;

        // Create collect pattern: collect(.range: [0, 1, 2], .into: i -> i * 2)
        let pattern = PatternExpr::Collect {
            range: Box::new(Expr::List(vec![Expr::Int(0), Expr::Int(1), Expr::Int(2)])),
            transform: Box::new(Expr::Lambda {
                params: vec!["i".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Mul,
                    left: Box::new(Expr::Ident("i".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            }),
        };

        let env = Environment::new();
        let result = collect.evaluate(&pattern, &env).unwrap();
        assert_eq!(
            result,
            Value::List(vec![Value::Int(0), Value::Int(2), Value::Int(4)])
        );
    }
}
