// Iterate pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for iterate semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{IterDirection, PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_iterable_element_type, TypeContext};

/// Handler for the iterate pattern.
///
/// iterate(.over: collection, .direction: dir, .into: init, .with: combiner)
///
/// Iterates over a collection with direction control, similar to fold
/// but with more control over iteration direction.
pub struct IteratePattern;

static ITERATE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".over", "collection to iterate", TypeConstraint::Iterable),
    ParamSpec::optional(".direction", "iteration direction (forward/backward)"),
    ParamSpec::required(".into", "initial accumulator value"),
    ParamSpec::required_with(
        ".with",
        "combining function (acc, elem) -> acc",
        TypeConstraint::FunctionArity(2),
    ),
];

impl PatternDefinition for IteratePattern {
    fn keyword(&self) -> &'static str {
        "iterate"
    }

    fn params(&self) -> &'static [ParamSpec] {
        ITERATE_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Iterate {
            over, into, with, ..
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected iterate pattern",
            ));
        };

        // Check collection type
        let coll_type = check_expr(over, ctx)?;

        // Check init type
        let into_type = check_expr(into, ctx)?;

        // Get element type from collection
        let elem_type = get_iterable_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("iterate requires a list or range, got {:?}", coll_type),
            )
        })?;

        // Iterate lambda: (accumulator, element) -> accumulator
        let expected_lambda_type = TypeExpr::Function(
            Box::new(TypeExpr::Tuple(vec![into_type.clone(), elem_type])),
            Box::new(into_type.clone()),
        );

        check_expr_with_hint(with, ctx, Some(&expected_lambda_type))?;

        Ok(into_type)
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Iterate {
            over,
            direction,
            into,
            with,
        } = pattern
        else {
            return Err("expected iterate pattern".to_string());
        };

        let collection = eval_expr(over, env)?;
        let initial = eval_expr(into, env)?;
        let op_val = eval_expr(with, env)?;

        let items: Vec<Value> = match collection {
            Value::List(items) => items,
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            _ => return Err("iterate requires a list or string".to_string()),
        };

        // Apply direction
        let items: Vec<Value> = match direction {
            IterDirection::Forward => items,
            IterDirection::Backward => items.into_iter().rev().collect(),
        };

        let mut acc = initial;
        for (i, item) in items.into_iter().enumerate() {
            acc = match &op_val {
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
                    // Bind 'acc' and 'item' and 'i' for the operation
                    if !params.is_empty() {
                        call_env.define(params[0].clone(), acc.clone(), false);
                    }
                    if params.len() >= 2 {
                        call_env.define(params[1].clone(), item.clone(), false);
                    }
                    // Also bind common names used in iterate
                    call_env.define("acc".to_string(), acc, false);
                    call_env.define("char".to_string(), item.clone(), false);
                    call_env.define("item".to_string(), item, false);
                    call_env.define("i".to_string(), Value::Int(i as i64), false);
                    eval_expr(body, &call_env)?
                }
                _ => return Err("iterate requires a function for 'with'".to_string()),
            };
        }
        Ok(acc)
    }

    fn description(&self) -> &'static str {
        "Iterate over a collection with direction control"
    }

    fn help(&self) -> &'static str {
        r#"The iterate pattern provides fold-like behavior with direction control:
  iterate(.over: collection, .direction: backward, .into: init, .with: (acc, elem) -> new_acc)

- .over: Collection to iterate over
- .direction: forward (default) or backward
- .into: Initial accumulator value
- .with: Combining function receiving accumulator and element"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "iterate(.over: \"hello\", .direction: backward, .into: \"\", .with: (acc, c) -> acc ++ c)",
            "iterate(.over: nums, .into: 0, .with: (sum, n) -> sum + n)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_iterate_pattern_keyword() {
        let iterate = IteratePattern;
        assert_eq!(iterate.keyword(), "iterate");
    }

    #[test]
    fn test_iterate_pattern_params() {
        let iterate = IteratePattern;
        let params = iterate.params();
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].name, ".over");
        assert_eq!(params[1].name, ".direction");
        assert_eq!(params[2].name, ".into");
        assert_eq!(params[3].name, ".with");
    }

    #[test]
    fn test_iterate_evaluation_forward() {
        let iterate = IteratePattern;

        // Create iterate pattern: iterate(.over: [1, 2, 3], .direction: forward, .into: 0, .with: (acc, n) -> acc + n)
        let pattern = PatternExpr::Iterate {
            over: Box::new(Expr::List(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)])),
            direction: IterDirection::Forward,
            into: Box::new(Expr::Int(0)),
            with: Box::new(Expr::Lambda {
                params: vec!["acc".to_string(), "n".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Add,
                    left: Box::new(Expr::Ident("acc".to_string())),
                    right: Box::new(Expr::Ident("n".to_string())),
                }),
            }),
        };

        let env = Environment::new();
        let result = iterate.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(6)); // 1 + 2 + 3 = 6
    }
}
