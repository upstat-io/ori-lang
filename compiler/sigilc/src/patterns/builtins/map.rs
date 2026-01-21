// Map pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for map semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{
    check_expr, check_expr_with_hint, get_function_return_type, get_iterable_element_type,
    TypeContext,
};

/// Handler for the map pattern.
///
/// map(.over: collection, .transform: function)
///
/// Transforms each element of a collection using a function.
pub struct MapPattern;

static MAP_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".over", "collection to map over", TypeConstraint::Iterable),
    ParamSpec::required_with(
        ".transform",
        "transformation function (elem) -> result",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for MapPattern {
    fn keyword(&self) -> &'static str {
        "map"
    }

    fn params(&self) -> &'static [ParamSpec] {
        MAP_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Map {
            collection,
            transform,
        } = pattern
        else {
            return Err(Diagnostic::error(ErrorCode::E3009, "expected map pattern"));
        };

        // Check collection type
        let coll_type = check_expr(collection, ctx).map_err(|msg| {
            Diagnostic::error(ErrorCode::E3001, msg)
        })?;

        // Get element type from collection
        let elem_type = get_iterable_element_type(&coll_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("map requires a list or range, got {:?}", coll_type),
            )
        })?;

        // Map lambda: element -> result
        let expected_lambda_type = TypeExpr::Function(
            Box::new(elem_type),
            Box::new(TypeExpr::Named("_infer_".to_string())),
        );

        let transform_type = check_expr_with_hint(transform, ctx, Some(&expected_lambda_type))
            .map_err(|msg| Diagnostic::error(ErrorCode::E3001, msg))?;

        let result_elem_type = get_function_return_type(&transform_type).map_err(|_| {
            Diagnostic::error(
                ErrorCode::E3001,
                format!("map transform must be a function, got {:?}", transform_type),
            )
        })?;

        Ok(TypeExpr::List(Box::new(result_elem_type)))
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Map {
            collection,
            transform,
        } = pattern
        else {
            return Err("expected map pattern".to_string());
        };

        let coll = eval_expr(collection, env)?;
        let transform_val = eval_expr(transform, env)?;

        let items = match coll {
            Value::List(items) => items,
            _ => return Err("map requires a list".to_string()),
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
                _ => return Err("map requires a function".to_string()),
            };
            results.push(result);
        }
        Ok(Value::List(results))
    }

    fn description(&self) -> &'static str {
        "Transform each element of a collection using a function"
    }

    fn help(&self) -> &'static str {
        r#"The map pattern transforms each element of a collection:
  map(.over: collection, .transform: (elem) -> new_elem)

The function receives each element and returns a transformed value.
The result is a new list with all transformed elements."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "map(.over: [1, 2, 3], .transform: x -> x * 2)",
            "map(.over: names, .transform: name -> name.to_upper())",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_map_pattern_keyword() {
        let map = MapPattern;
        assert_eq!(map.keyword(), "map");
    }

    #[test]
    fn test_map_pattern_params() {
        let map = MapPattern;
        let params = map.params();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, ".over");
        assert_eq!(params[1].name, ".transform");
    }

    #[test]
    fn test_map_pattern_description() {
        let map = MapPattern;
        assert!(map.description().contains("Transform"));
    }

    #[test]
    fn test_map_evaluation() {
        let map = MapPattern;

        // Create map pattern: map(.over: [1, 2, 3], .transform: x -> x * 2)
        let pattern = PatternExpr::Map {
            collection: Box::new(Expr::List(vec![
                Expr::Int(1),
                Expr::Int(2),
                Expr::Int(3),
            ])),
            transform: Box::new(Expr::Lambda {
                params: vec!["x".to_string()],
                body: Box::new(Expr::Binary {
                    op: crate::ast::BinaryOp::Mul,
                    left: Box::new(Expr::Ident("x".to_string())),
                    right: Box::new(Expr::Int(2)),
                }),
            }),
        };

        let env = Environment::new();
        let result = map.evaluate(&pattern, &env).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)]));
    }
}
