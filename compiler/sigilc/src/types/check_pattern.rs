// Pattern expression type checking for Sigil
// Handles fold, map, filter, recurse, and other pattern expressions
//
// Uses helper functions from compat.rs to reduce code duplication:
// - get_list_element_type: for patterns requiring list (fold, filter, count)
// - get_iterable_element_type: for patterns accepting list or range (map, iterate)
// - get_function_return_type: for extracting transform return types

use super::check::{check_expr, check_expr_with_hint};
use super::compat::{
    get_function_return_type, get_iterable_element_type, get_list_element_type, types_compatible,
};
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
            let elem_type = get_list_element_type(&coll_type)
                .map_err(|_| format!("Fold requires a list, got {:?}", coll_type))?;

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
            let elem_type = get_iterable_element_type(&coll_type)
                .map_err(|_| format!("Map requires a list or range, got {:?}", coll_type))?;

            // Map lambda: element -> result
            let expected_lambda_type = TypeExpr::Function(
                Box::new(elem_type),
                Box::new(TypeExpr::Named("_infer_".to_string())),
            );

            let transform_type = check_expr_with_hint(transform, ctx, Some(&expected_lambda_type))?;
            let result_elem_type = get_function_return_type(&transform_type)
                .map_err(|_| format!("Map transform must be a function, got {:?}", transform_type))?;

            Ok(TypeExpr::List(Box::new(result_elem_type)))
        }

        PatternExpr::Filter {
            collection,
            predicate,
        } => {
            let coll_type = check_expr(collection, ctx)?;
            let elem_type = get_list_element_type(&coll_type)
                .map_err(|_| format!("Filter requires a list, got {:?}", coll_type))?;

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
            let elem_type = get_function_return_type(&transform_type)
                .map_err(|_| format!("Collect transform must be a function, got {:?}", transform_type))?;

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
            let elem_type = get_iterable_element_type(&coll_type)
                .map_err(|_| format!("Iterate requires a list or range, got {:?}", coll_type))?;

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
            let elem_type = get_list_element_type(&coll_type)
                .map_err(|_| format!("Count requires a list, got {:?}", coll_type))?;

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
