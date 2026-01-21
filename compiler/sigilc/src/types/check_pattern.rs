// Pattern expression type checking for Sigil
// Handles fold, map, filter, recurse, and other pattern expressions

use super::check_expr::{check_expr, check_expr_with_hint};
use super::compat::types_compatible;
use super::context::TypeContext;
use crate::ast::*;

pub fn check_pattern_expr(p: &PatternExpr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    match p {
        PatternExpr::Fold {
            collection,
            init,
            op,
        } => {
            let coll_type = check_expr(collection, ctx)?;
            let init_type = check_expr(init, ctx)?;

            // Get element type from collection
            let elem_type = match &coll_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                _ => return Err(format!("Fold requires a list, got {:?}", coll_type)),
            };

            // Fold lambda: (accumulator, element) -> accumulator
            let expected_lambda_type = TypeExpr::Function(
                Box::new(TypeExpr::Tuple(vec![init_type.clone(), elem_type])),
                Box::new(init_type.clone()),
            );

            check_expr_with_hint(op, ctx, Some(&expected_lambda_type))?;
            Ok(init_type)
        }

        PatternExpr::Map {
            collection,
            transform,
        } => {
            let coll_type = check_expr(collection, ctx)?;

            // Get element type from collection
            let elem_type = match &coll_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                TypeExpr::Named(n) if n == "Range" => TypeExpr::Named("int".to_string()),
                _ => return Err(format!("Map requires a list or range, got {:?}", coll_type)),
            };

            // Map lambda: element -> result (we don't know result yet, so use a placeholder)
            // Check the transform with the expected input type
            let expected_lambda_type = TypeExpr::Function(
                Box::new(elem_type),
                Box::new(TypeExpr::Named("_infer_".to_string())),
            );

            let transform_type = check_expr_with_hint(transform, ctx, Some(&expected_lambda_type))?;

            // Extract return type from the checked transform
            let result_elem_type = if let TypeExpr::Function(_, ret) = transform_type {
                *ret
            } else {
                return Err(format!(
                    "Map transform must be a function, got {:?}",
                    transform_type
                ));
            };

            Ok(TypeExpr::List(Box::new(result_elem_type)))
        }

        PatternExpr::Filter {
            collection,
            predicate,
        } => {
            let coll_type = check_expr(collection, ctx)?;

            // Get element type from collection
            let elem_type = match &coll_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                _ => return Err(format!("Filter requires a list, got {:?}", coll_type)),
            };

            // Filter predicate: element -> bool
            let expected_lambda_type = TypeExpr::Function(
                Box::new(elem_type),
                Box::new(TypeExpr::Named("bool".to_string())),
            );

            check_expr_with_hint(predicate, ctx, Some(&expected_lambda_type))?;
            Ok(coll_type)
        }

        PatternExpr::Collect { range, transform } => {
            check_expr(range, ctx)?;

            // Collect iterates over a range (integers)
            let expected_lambda_type = TypeExpr::Function(
                Box::new(TypeExpr::Named("int".to_string())),
                Box::new(TypeExpr::Named("_infer_".to_string())),
            );

            let transform_type = check_expr_with_hint(transform, ctx, Some(&expected_lambda_type))?;

            // Extract return type from the checked transform
            let elem_type = if let TypeExpr::Function(_, ret) = transform_type {
                *ret
            } else {
                return Err(format!(
                    "Collect transform must be a function, got {:?}",
                    transform_type
                ));
            };

            Ok(TypeExpr::List(Box::new(elem_type)))
        }

        PatternExpr::Recurse {
            condition,
            base_value,
            step,
            ..
        } => {
            check_expr(condition, ctx)?;
            let base_type = check_expr(base_value, ctx)?;
            let step_type = check_expr(step, ctx)?;

            // Base and step should have compatible types
            if !types_compatible(&base_type, &step_type, ctx) {
                return Err(format!(
                    "Recurse base type {:?} doesn't match step type {:?}",
                    base_type, step_type
                ));
            }

            Ok(base_type)
        }

        PatternExpr::Iterate {
            over, into, with, ..
        } => {
            let coll_type = check_expr(over, ctx)?;
            let into_type = check_expr(into, ctx)?;

            // Get element type from collection
            let elem_type = match &coll_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                TypeExpr::Named(n) if n == "Range" => TypeExpr::Named("int".to_string()),
                _ => {
                    return Err(format!(
                        "Iterate requires a list or range, got {:?}",
                        coll_type
                    ))
                }
            };

            // Iterate lambda: (accumulator, element) -> accumulator
            let expected_lambda_type = TypeExpr::Function(
                Box::new(TypeExpr::Tuple(vec![into_type.clone(), elem_type])),
                Box::new(into_type.clone()),
            );

            check_expr_with_hint(with, ctx, Some(&expected_lambda_type))?;
            Ok(into_type)
        }

        PatternExpr::Transform { input, steps } => {
            let mut current_type = check_expr(input, ctx)?;
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

        PatternExpr::Count {
            collection,
            predicate,
        } => {
            let coll_type = check_expr(collection, ctx)?;

            // Get element type from collection
            let elem_type = match &coll_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                _ => return Err(format!("Count requires a list, got {:?}", coll_type)),
            };

            // Count predicate: element -> bool
            let expected_lambda_type = TypeExpr::Function(
                Box::new(elem_type),
                Box::new(TypeExpr::Named("bool".to_string())),
            );

            check_expr_with_hint(predicate, ctx, Some(&expected_lambda_type))?;
            Ok(TypeExpr::Named("int".to_string()))
        }

        PatternExpr::Parallel {
            branches, timeout, ..
        } => {
            // Check all branch expressions and build record type
            let mut field_types = Vec::new();
            for (name, expr) in branches {
                let ty = check_expr(expr, ctx)?;
                field_types.push((name.clone(), ty));
            }
            if let Some(t) = timeout {
                check_expr(t, ctx)?;
            }
            // Returns an anonymous record type with the branch names as fields
            Ok(TypeExpr::Record(field_types))
        }
    }
}
