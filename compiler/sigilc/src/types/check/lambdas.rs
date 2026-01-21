// Lambda type checking

use crate::ast::{Expr, TypeExpr};

use super::super::context::TypeContext;
use super::check_expr_inner;

/// Check a lambda expression with optional expected function type
pub fn check_lambda(
    params: &[String],
    body: &Expr,
    ctx: &TypeContext,
    expected: Option<&TypeExpr>,
) -> Result<TypeExpr, String> {
    // Unwrap the expected type - handle single-element tuples containing function types
    // This happens because (int -> int) is parsed as Tuple([Function(int, int)])
    let unwrapped_expected: Option<&TypeExpr> = match expected {
        Some(TypeExpr::Tuple(types)) if types.len() == 1 => {
            if let TypeExpr::Function(_, _) = &types[0] {
                Some(&types[0])
            } else {
                expected
            }
        }
        other => other,
    };

    // Determine parameter types from expected type hint
    let param_types: Vec<TypeExpr> = if let Some(TypeExpr::Function(param_type, _)) =
        unwrapped_expected
    {
        // Extract param types from expected function type
        match param_type.as_ref() {
            TypeExpr::Tuple(types) => types.clone(),
            single_type => vec![single_type.clone()],
        }
    } else {
        // No type hint - this is an error in strict mode
        return Err(format!(
            "Cannot infer types for lambda parameters {:?}. Lambda must be used in a context that provides type information (e.g., map, filter, fold).",
            params
        ));
    };

    if param_types.len() != params.len() {
        return Err(format!(
            "Lambda expects {} parameters but context provides {} parameter types",
            params.len(),
            param_types.len()
        ));
    }

    // Create a child context with lambda parameters
    let child_ctx = TypeContext::child_with_locals(ctx, |locals| {
        for (name, ty) in params.iter().zip(param_types.iter()) {
            locals.insert(name.clone(), ty.clone());
        }
    });

    // Check the body with the child context
    let body_type = check_expr_inner(body, &child_ctx)?;

    // Build the function type
    let param_type = if params.len() == 1 {
        // Safe: we just checked params.len() == 1, so param_types has exactly one element
        param_types
            .into_iter()
            .next()
            .unwrap_or_else(|| TypeExpr::Named("void".to_string()))
    } else {
        TypeExpr::Tuple(param_types)
    };
    Ok(TypeExpr::Function(
        Box::new(param_type),
        Box::new(body_type),
    ))
}
