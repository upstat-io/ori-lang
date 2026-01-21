// Count pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for count semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, is_truthy, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_list_element_type, TypeContext};

/// Handler for the count pattern.
///
/// count(.over: collection, .where: predicate)
///
/// Counts elements in a collection that match a predicate.
pub struct CountPattern;

static COUNT_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".over", "collection to count", TypeConstraint::List),
    ParamSpec::required_with(
        ".where",
        "predicate function (elem) -> bool",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for CountPattern {
    fn keyword(&self) -> &'static str {
        "count"
    }

    fn params(&self) -> &'static [ParamSpec] {
        COUNT_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Count {
            collection,
            predicate,
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected count pattern",
            ));
        };

        // Check collection type
        let coll_type = check_expr(collection, ctx)
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // Get element type from collection
        let elem_type = get_list_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("count requires a list, got {:?}", coll_type),
            )
        })?;

        // Count predicate: element -> bool
        let expected_lambda_type = TypeExpr::Function(
            Box::new(elem_type),
            Box::new(TypeExpr::Named("bool".to_string())),
        );

        check_expr_with_hint(predicate, ctx, Some(&expected_lambda_type))
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        // Count always returns int
        Ok(TypeExpr::Named("int".to_string()))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Count {
            collection,
            predicate,
        } = pattern
        else {
            return Err("expected count pattern".to_string());
        };

        let coll = eval_expr(collection, env)?;
        let pred_val = eval_expr(predicate, env)?;

        let items = match coll {
            Value::List(items) => items,
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            _ => return Err("count requires a collection".to_string()),
        };

        let mut count = 0;
        for item in items {
            let matches = match &pred_val {
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
                    is_truthy(&eval_expr(body, &call_env)?)
                }
                _ => return Err("count requires a predicate function".to_string()),
            };
            if matches {
                count += 1;
            }
        }
        Ok(Value::Int(count))
    }

    fn description(&self) -> &'static str {
        "Count elements in a collection that match a predicate"
    }

    fn help(&self) -> &'static str {
        r#"The count pattern counts matching elements:
  count(.over: collection, .where: (elem) -> bool)

The predicate function receives each element and returns true to count it.
Returns an integer count of matching elements."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "count(.over: [1, 2, 3, 4, 5], .where: x -> x > 3)",
            "count(.over: users, .where: u -> u.active)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, Expr};

    #[test]
    fn test_count_pattern_keyword() {
        let count = CountPattern;
        assert_eq!(count.keyword(), "count");
    }

    #[test]
    fn test_count_pattern_params() {
        let count = CountPattern;
        let params = count.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".over");
        assert_eq!(params[1].name, ".where");
    }

    #[test]
    fn test_count_evaluation() {
        let count = CountPattern;

        // Create count pattern: count(.over: [1, 2, 3, 4, 5], .where: x -> x > 2)
        let pattern = PatternExpr::Count {
            collection: Box::new(Expr::List(vec![
                Expr::Int(1),
                Expr::Int(2),
                Expr::Int(3),
                Expr::Int(4),
                Expr::Int(5),
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
        let result = count.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(3)); // 3, 4, 5 are > 2
    }
}
