// Find pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for find semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, check_expr_with_hint, get_list_element_type, TypeContext};

/// Handler for the find pattern.
///
/// find(.in: collection, .where: predicate)
/// find(.in: collection, .where: predicate, .default: value)
///
/// Finds the first element in a collection matching the predicate.
/// Returns Option<T> without .default, or T with .default.
pub struct FindPattern;

static FIND_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".in", "collection to search", TypeConstraint::List),
    ParamSpec::required_with(
        ".where",
        "predicate function (elem) -> bool",
        TypeConstraint::FunctionArity(1),
    ),
    ParamSpec::optional(".default", "default value if not found"),
];

impl PatternDefinition for FindPattern {
    fn keyword(&self) -> &'static str {
        "find"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FIND_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Find {
            collection,
            predicate,
            default,
        } = pattern
        else {
            return Err(Diagnostic::error(ErrorCode::E3009, "expected find pattern"));
        };

        // Check collection type
        let coll_type = check_expr(collection, ctx)?;

        // Get element type from collection
        let elem_type = get_list_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("find requires a list, got {:?}", coll_type),
            )
        })?;

        // Predicate should be (elem) -> bool
        let expected_pred_type = TypeExpr::Function(
            Box::new(elem_type.clone()),
            Box::new(TypeExpr::Named("bool".to_string())),
        );

        check_expr_with_hint(predicate, ctx, Some(&expected_pred_type))?;

        // If default is provided, return element type; otherwise return Option<elem_type>
        if let Some(default_expr) = default {
            let default_type = check_expr(default_expr, ctx)?;

            // Default must match element type
            if default_type != elem_type {
                return Err(Diagnostic::error(
                    ErrorCode::E3001,
                    format!(
                        "find default type {:?} doesn't match element type {:?}",
                        default_type, elem_type
                    ),
                ));
            }

            Ok(elem_type)
        } else {
            Ok(TypeExpr::Optional(Box::new(elem_type)))
        }
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Find {
            collection,
            predicate,
            default,
        } = pattern
        else {
            return Err("expected find pattern".to_string());
        };

        let coll = eval_expr(collection, env)?;
        let pred_val = eval_expr(predicate, env)?;

        let items: Vec<Value> = match coll {
            Value::List(items) => items,
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            _ => return Err("find requires a list or string".to_string()),
        };

        // Find first matching element
        for item in items {
            let matches = match &pred_val {
                Value::Function {
                    params,
                    body,
                    env: fn_env,
                } => {
                    if params.len() != 1 {
                        return Err("find predicate must take 1 argument".to_string());
                    }
                    let mut call_env = Environment {
                        configs: env.configs.clone(),
                        current_params: env.current_params.clone(),
                        functions: env.functions.clone(),
                        locals: Environment::locals_from_values(fn_env.clone()),
                    };
                    call_env.define(params[0].clone(), item.clone(), false);
                    let result = eval_expr(body, &call_env)?;
                    matches!(result, Value::Bool(true))
                }
                _ => return Err("find predicate must be a function".to_string()),
            };

            if matches {
                // Found a match
                return if default.is_some() {
                    Ok(item)
                } else {
                    Ok(Value::Some(Box::new(item)))
                };
            }
        }

        // No match found
        if let Some(default_expr) = default {
            eval_expr(default_expr, env)
        } else {
            Ok(Value::None_)
        }
    }

    fn description(&self) -> &'static str {
        "Find the first element in a collection matching a predicate"
    }

    fn help(&self) -> &'static str {
        r#"The find pattern searches a collection for the first matching element:
  find(.in: collection, .where: (elem) -> bool)

Returns Option<T> - Some(elem) if found, None otherwise.

With .default, returns the element type directly:
  find(.in: collection, .where: predicate, .default: fallback_value)"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "find(.in: [1, 2, 3, 4], .where: x -> x > 2)",
            "find(.in: users, .where: u -> u.name == target, .default: default_user)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_find_pattern_keyword() {
        let find = FindPattern;
        assert_eq!(find.keyword(), "find");
    }

    #[test]
    fn test_find_pattern_params() {
        let find = FindPattern;
        let params = find.params();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, ".in");
        assert_eq!(params[1].name, ".where");
        assert_eq!(params[2].name, ".default");
    }

    #[test]
    fn test_find_evaluation_found() {
        let find = FindPattern;

        // Create find pattern: find(.in: [1, 2, 3, 4], .where: x -> x > 2)
        let pattern = PatternExpr::Find {
            collection: Box::new(Expr::List(vec![
                Expr::Int(1),
                Expr::Int(2),
                Expr::Int(3),
                Expr::Int(4),
            ])),
            predicate: Box::new(Expr::Lambda {
                params: vec!["x".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Gt,
                    left: Box::new(Expr::Ident("x".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            }),
            default: None,
        };

        let env = Environment::new();
        let result = find.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Some(Box::new(Value::Int(3))));
    }

    #[test]
    fn test_find_evaluation_not_found() {
        let find = FindPattern;

        // Create find pattern: find(.in: [1, 2], .where: x -> x > 10)
        let pattern = PatternExpr::Find {
            collection: Box::new(Expr::List(vec![Expr::Int(1), Expr::Int(2)])),
            predicate: Box::new(Expr::Lambda {
                params: vec!["x".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Gt,
                    left: Box::new(Expr::Ident("x".to_string())),
                    right: Box::new(Expr::Int(10)),
                }),
            }),
            default: None,
        };

        let env = Environment::new();
        let result = find.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::None_);
    }

    #[test]
    fn test_find_evaluation_with_default() {
        let find = FindPattern;

        // Create find pattern: find(.in: [1, 2], .where: x -> x > 10, .default: 0)
        let pattern = PatternExpr::Find {
            collection: Box::new(Expr::List(vec![Expr::Int(1), Expr::Int(2)])),
            predicate: Box::new(Expr::Lambda {
                params: vec!["x".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Gt,
                    left: Box::new(Expr::Ident("x".to_string())),
                    right: Box::new(Expr::Int(10)),
                }),
            }),
            default: Some(Box::new(Expr::Int(0))),
        };

        let env = Environment::new();
        let result = find.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::Int(0));
    }
}
