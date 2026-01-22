// Fold pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for fold semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{BinaryOp, PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_binary_op, eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_list_element_type, TypeContext};

/// Handler for the fold pattern.
///
/// fold(.over: collection, .init: initial, .with: combiner)
///
/// Reduces a collection to a single value by applying a combining
/// function to each element and an accumulator.
pub struct FoldPattern;

static FOLD_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".over", "collection to fold over", TypeConstraint::List),
    ParamSpec::required(".init", "initial accumulator value"),
    ParamSpec::required_with(
        ".with",
        "combining function (acc, elem) -> acc",
        TypeConstraint::FunctionArity(2),
    ),
];

impl PatternDefinition for FoldPattern {
    fn keyword(&self) -> &'static str {
        "fold"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FOLD_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Fold {
            collection,
            init,
            op,
        } = pattern
        else {
            return Err(Diagnostic::error(ErrorCode::E3009, "expected fold pattern"));
        };

        // Check collection type
        let coll_type = check_expr(collection, ctx)?;

        // Check init type
        let init_type = check_expr(init, ctx)?;

        // Get element type from collection
        let elem_type = get_list_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("fold requires a list, got {:?}", coll_type),
            )
        })?;

        // Fold lambda: (accumulator, element) -> accumulator
        let expected_lambda_type = TypeExpr::Function(
            Box::new(TypeExpr::Tuple(vec![init_type.clone(), elem_type])),
            Box::new(init_type.clone()),
        );

        check_expr_with_hint(op, ctx, Some(&expected_lambda_type))?;

        Ok(init_type)
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Fold {
            collection,
            init,
            op,
        } = pattern
        else {
            return Err("expected fold pattern".to_string());
        };

        let coll = eval_expr(collection, env)?;
        let initial = eval_expr(init, env)?;

        let items: Vec<Value> = match coll {
            Value::List(items) => items,
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            _ => return Err("fold requires a list or string".to_string()),
        };

        let op_val = eval_expr(op, env)?;

        let mut acc = initial;
        for item in items {
            acc = match &op_val {
                Value::BuiltinFunction(name) if name == "+" => {
                    eval_binary_op(&BinaryOp::Add, acc, item)?
                }
                Value::BuiltinFunction(name) if name == "*" => {
                    eval_binary_op(&BinaryOp::Mul, acc, item)?
                }
                Value::Function {
                    params,
                    body,
                    env: fn_env,
                } => {
                    if params.len() != 2 {
                        return Err("fold function must take 2 arguments".to_string());
                    }
                    let mut call_env = Environment {
                        configs: env.configs.clone(),
                        current_params: env.current_params.clone(),
                        functions: env.functions.clone(),
                        locals: Environment::locals_from_values(fn_env.clone()),
                    };
                    call_env.define(params[0].clone(), acc, false);
                    call_env.define(params[1].clone(), item, false);
                    eval_expr(body, &call_env)?
                }
                _ => return Err("Invalid fold operation".to_string()),
            };
        }
        Ok(acc)
    }

    fn description(&self) -> &'static str {
        "Reduce a collection to a single value by repeatedly applying a combining function"
    }

    fn help(&self) -> &'static str {
        r#"The fold pattern accumulates a result by applying a function to each element:
  fold(.over: collection, .init: initial_value, .with: (acc, elem) -> new_acc)

The function receives the current accumulator and next element, returning the new accumulator.
The final result is the accumulator after processing all elements."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "fold(.over: [1, 2, 3], .init: 0, .with: +)",
            "fold(.over: nums, .init: 1, .with: *)",
            "fold(.over: strings, .init: \"\", .with: (acc, s) -> acc ++ s)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_fold_pattern_keyword() {
        let fold = FoldPattern;
        assert_eq!(fold.keyword(), "fold");
    }

    #[test]
    fn test_fold_pattern_params() {
        let fold = FoldPattern;
        let params = fold.params();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, ".over");
        assert_eq!(params[1].name, ".init");
        assert_eq!(params[2].name, ".with");
    }

    #[test]
    fn test_fold_pattern_description() {
        let fold = FoldPattern;
        assert!(fold.description().contains("Reduce"));
    }

    #[test]
    fn test_fold_pattern_help() {
        let fold = FoldPattern;
        assert!(!fold.help().is_empty());
    }

    #[test]
    fn test_fold_pattern_examples() {
        let fold = FoldPattern;
        assert!(!fold.examples().is_empty());
    }

    #[test]
    fn test_fold_evaluation_sum() {
        let fold = FoldPattern;

        // Create fold pattern: fold(.over: [1, 2, 3], .init: 0, .with: +)
        let pattern = PatternExpr::Fold {
            collection: Box::new(Expr::List(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)])),
            init: Box::new(Expr::Int(0)),
            op: Box::new(Expr::Ident("+".to_string())),
        };

        let env = Environment::new();
        let result = fold.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(6));
    }

    #[test]
    fn test_fold_evaluation_product() {
        let fold = FoldPattern;

        // Create fold pattern: fold(.over: [1, 2, 3, 4], .init: 1, .with: *)
        let pattern = PatternExpr::Fold {
            collection: Box::new(Expr::List(vec![
                Expr::Int(1),
                Expr::Int(2),
                Expr::Int(3),
                Expr::Int(4),
            ])),
            init: Box::new(Expr::Int(1)),
            op: Box::new(Expr::Ident("*".to_string())),
        };

        let env = Environment::new();
        let result = fold.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(24));
    }
}
