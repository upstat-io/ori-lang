// Function call type checking

use std::collections::HashMap;

use crate::ast::{Expr, TypeExpr};

use super::super::compat::types_compatible;
use super::super::context::TypeContext;
use super::{check_expr, check_expr_with_hint};

/// Check a function call expression
pub fn check_call(func: &Expr, args: &[Expr], ctx: &TypeContext) -> Result<TypeExpr, String> {
    if let Expr::Ident(name) = func {
        // `self` is a special recursive call - use current function's return type
        if name == "self" {
            // Check args without type hints for self calls
            for arg in args {
                check_expr(arg, ctx)?;
            }
            return ctx
                .current_return_type()
                .ok_or_else(|| "self() called outside of a function context".to_string());
        }

        // Check if it's a local variable holding a function
        if let Some(local_type) = ctx.lookup_local(name) {
            if let TypeExpr::Function(param_type, ret) = local_type {
                // Check args with expected param types
                let expected_types = match param_type.as_ref() {
                    TypeExpr::Tuple(types) => types.clone(),
                    single => vec![single.clone()],
                };
                for (i, arg) in args.iter().enumerate() {
                    let expected = expected_types.get(i);
                    check_expr_with_hint(arg, ctx, expected)?;
                }
                return Ok(*ret.clone());
            }
            return Err(format!(
                "Variable '{}' is not callable: {:?}",
                name, local_type
            ));
        }

        // Check if it's a defined function
        if let Some(sig) = ctx.lookup_function(name) {
            // Check argument count
            if args.len() != sig.params.len() {
                return Err(format!(
                    "Function '{}' expects {} arguments, got {}",
                    name,
                    sig.params.len(),
                    args.len()
                ));
            }

            // Check capability requirements
            if !sig.capabilities.is_empty() {
                let missing = ctx.check_capabilities(&sig.capabilities);
                if !missing.is_empty() {
                    return Err(format!(
                        "Function '{}' requires capabilities [{}] but [{}] are not available. \
                         Add 'uses {}' to the calling function or use 'with {} = impl in ...' to provide them.",
                        name,
                        sig.capabilities.join(", "),
                        missing.join(", "),
                        missing.join(", "),
                        missing.first().unwrap_or(&String::new())
                    ));
                }
            }

            // For generic functions like assert_eq, infer type param from first arg
            // then use it for subsequent args with the same type param
            let mut inferred_types: HashMap<String, TypeExpr> = HashMap::new();

            for (i, arg) in args.iter().enumerate() {
                if let Some((param_name, param_type)) = sig.params.get(i) {
                    // If param type is a type parameter, check if we've inferred it
                    if let TypeExpr::Named(type_name) = param_type {
                        if sig.type_params.contains(type_name) {
                            // It's a type parameter
                            let arg_type =
                                check_expr_with_hint(arg, ctx, inferred_types.get(type_name))?;
                            if let Some(inferred) = inferred_types.get(type_name) {
                                // Verify type matches
                                if !types_compatible(&arg_type, inferred, ctx) {
                                    return Err(format!(
                                        "Argument '{}' has type {:?} but expected {:?}",
                                        param_name, arg_type, inferred
                                    ));
                                }
                            } else {
                                // First time seeing this type param - infer from arg
                                inferred_types.insert(type_name.clone(), arg_type);
                            }
                            continue;
                        }
                    }
                    // Not a type parameter - check with declared type and verify
                    let arg_type = check_expr_with_hint(arg, ctx, Some(param_type))?;
                    if !types_compatible(&arg_type, param_type, ctx) {
                        return Err(format!(
                            "Argument '{}' has type {:?} but expected {:?}",
                            param_name, arg_type, param_type
                        ));
                    }
                } else {
                    check_expr(arg, ctx)?;
                }
            }
            return Ok(sig.return_type.clone());
        }

        return Err(format!("Unknown function: {}", name));
    }

    // Lambda call or other callable - check args first without hints
    for arg in args {
        check_expr(arg, ctx)?;
    }
    let func_type = check_expr(func, ctx)?;
    if let TypeExpr::Function(_, ret) = func_type {
        return Ok(*ret);
    }

    Err(format!("Expression is not callable: {:?}", func))
}

/// Check a method call expression
pub fn check_method_call(
    receiver: &Expr,
    method: &str,
    args: &[Expr],
    ctx: &TypeContext,
) -> Result<TypeExpr, String> {
    let receiver_type = check_expr(receiver, ctx)?;
    for arg in args {
        check_expr(arg, ctx)?;
    }

    // Handle list methods
    if let TypeExpr::List(elem_type) = &receiver_type {
        match method {
            "push" | "pop" | "slice" => Ok(receiver_type.clone()),
            "first" | "last" => Ok(TypeExpr::Optional(elem_type.clone())),
            "len" => Ok(TypeExpr::Named("int".to_string())),
            "join" => Ok(TypeExpr::Named("str".to_string())),
            _ => Err(format!("Unknown list method: {}", method)),
        }
    } else if let TypeExpr::Named(name) = &receiver_type {
        if name == "str" {
            match method {
                "len" => Ok(TypeExpr::Named("int".to_string())),
                "slice" => Ok(TypeExpr::Named("str".to_string())),
                "split" => Ok(TypeExpr::List(Box::new(TypeExpr::Named("str".to_string())))),
                "trim" | "upper" | "lower" => Ok(TypeExpr::Named("str".to_string())),
                _ => Err(format!("Unknown string method: {}", method)),
            }
        } else {
            Err(format!(
                "Cannot call method '{}' on type {:?}",
                method, receiver_type
            ))
        }
    } else {
        Err(format!(
            "Cannot call method '{}' on type {:?}",
            method, receiver_type
        ))
    }
}
