// Filter pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for filter semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, is_truthy, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_list_element_type, TypeContext};

/// Handler for the filter pattern.
///
/// filter(.over: collection, .where: predicate)
///
/// Selects elements from a collection that match a predicate.
pub struct FilterPattern;

static FILTER_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".over", "collection to filter", TypeConstraint::List),
    ParamSpec::required_with(
        ".where",
        "predicate function (elem) -> bool",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for FilterPattern {
    fn keyword(&self) -> &'static str {
        "filter"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FILTER_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Filter {
            collection,
            predicate,
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected filter pattern",
            ));
        };

        // Check collection type
        let coll_type = check_expr(collection, ctx)
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // Get element type from collection
        let elem_type = get_list_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("filter requires a list, got {:?}", coll_type),
            )
        })?;

        // Filter predicate: element -> bool
        let expected_lambda_type = TypeExpr::Function(
            Box::new(elem_type),
            Box::new(TypeExpr::Named("bool".to_string())),
        );

        check_expr_with_hint(predicate, ctx, Some(&expected_lambda_type))
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // Filter returns the same type as input collection
        Ok(coll_type)
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Filter {
            collection,
            predicate,
        } = pattern
        else {
            return Err("expected filter pattern".to_string());
        };

        let coll = eval_expr(collection, env)?;
        let pred_val = eval_expr(predicate, env)?;

        let items = match coll {
            Value::List(items) => items,
            _ => return Err("filter requires a list".to_string()),
        };

        let mut results = Vec::new();
        for item in items {
            let keep = match &pred_val {
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
                        call_env.define(param.clone(), item.clone(), false);
                    }
                    is_truthy(&eval_expr(body, &call_env)?)
                }
                _ => return Err("filter requires a function".to_string()),
            };
            if keep {
                results.push(item);
            }
        }
        Ok(Value::List(results))
    }

    fn description(&self) -> &'static str {
        "Select elements from a collection that match a predicate"
    }

    fn help(&self) -> &'static str {
        r#"The filter pattern selects elements matching a predicate:
  filter(.over: collection, .where: (elem) -> bool)

The predicate function receives each element and returns true to keep it.
The result is a new list containing only matching elements."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "filter(.over: [1, 2, 3, 4], .where: x -> x > 2)",
            "filter(.over: users, .where: u -> u.active)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, Expr};

    #[test]
    fn test_filter_pattern_keyword() {
        let filter = FilterPattern;
        assert_eq!(filter.keyword(), "filter");
    }

    #[test]
    fn test_filter_pattern_params() {
        let filter = FilterPattern;
        let params = filter.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".over");
        assert_eq!(params[1].name, ".where");
    }

    #[test]
    fn test_filter_evaluation() {
        let filter = FilterPattern;

        // Create filter pattern: filter(.over: [1, 2, 3, 4], .where: x -> x > 2)
        let pattern = PatternExpr::Filter {
            collection: Box::new(Expr::List(vec![
                Expr::Int(1),
                Expr::Int(2),
                Expr::Int(3),
                Expr::Int(4),
            ])),
            predicate: Box::new(Expr::Lambda {
                params: vec!["x".to_string()],
                body: Box::new(Expr::Binary {
                    op: BinaryOp::Gt,
                    left: Box::new(Expr::Ident("x".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            }),
        };

        let env = Environment::new();
        let result = filter.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(3), Value::Int(4)]));
    }
}
