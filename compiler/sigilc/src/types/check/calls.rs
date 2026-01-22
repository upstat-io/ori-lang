// Function call type checking

use std::collections::HashMap;

use crate::ast::{Expr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::compat::types_compatible;
use super::super::context::TypeContext;
use super::{check_expr, check_expr_with_hint};

/// Check a function call expression
pub fn check_call(func: &Expr, args: &[Expr], ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    if let Expr::Ident(name) = func {
        // `self` is a special recursive call - use current function's return type
        if name == "self" {
            // Check args without type hints for self calls
            for arg in args {
                check_expr(arg, ctx)?;
            }
            return ctx.current_return_type().ok_or_else(|| {
                Diagnostic::error(
                    ErrorCode::E3002,
                    "self() called outside of a function context",
                )
                .with_label(ctx.make_span(0..0), "not in a function")
            });
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
            return Err(Diagnostic::error(
                ErrorCode::E3006,
                format!("variable '{}' is not callable: {:?}", name, local_type),
            )
            .with_label(ctx.make_span(0..0), "not a function"));
        }

        // Check if it's a defined function
        if let Some(sig) = ctx.lookup_function(name) {
            // Check argument count
            if args.len() != sig.params.len() {
                return Err(Diagnostic::error(
                    ErrorCode::E3004,
                    format!(
                        "function '{}' expects {} arguments, got {}",
                        name,
                        sig.params.len(),
                        args.len()
                    ),
                )
                .with_label(ctx.make_span(0..0), "wrong number of arguments"));
            }

            // Check capability requirements
            if !sig.capabilities.is_empty() {
                let missing = ctx.check_capabilities(&sig.capabilities);
                if !missing.is_empty() {
                    return Err(Diagnostic::error(
                        ErrorCode::E3006,
                        format!(
                            "function '{}' requires capabilities [{}] but [{}] are not available",
                            name,
                            sig.capabilities.join(", "),
                            missing.join(", ")
                        ),
                    )
                    .with_label(ctx.make_span(0..0), "missing capabilities")
                    .with_help(format!(
                        "add 'uses {}' to the calling function or use 'with {} = impl in ...' to provide them",
                        missing.join(", "),
                        missing.first().unwrap_or(&String::new())
                    )));
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
                                    return Err(Diagnostic::error(
                                        ErrorCode::E3001,
                                        format!(
                                            "argument '{}' has type {:?} but expected {:?}",
                                            param_name, arg_type, inferred
                                        ),
                                    )
                                    .with_label(
                                        ctx.make_span(0..0),
                                        format!("expected {:?}", inferred),
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
                        return Err(Diagnostic::error(
                            ErrorCode::E3001,
                            format!(
                                "argument '{}' has type {:?} but expected {:?}",
                                param_name, arg_type, param_type
                            ),
                        )
                        .with_label(ctx.make_span(0..0), format!("expected {:?}", param_type)));
                    }
                } else {
                    check_expr(arg, ctx)?;
                }
            }
            return Ok(sig.return_type.clone());
        }

        return Err(
            Diagnostic::error(ErrorCode::E3002, format!("unknown function '{}'", name))
                .with_label(ctx.make_span(0..0), "not found"),
        );
    }

    // Lambda call or other callable - check args first without hints
    for arg in args {
        check_expr(arg, ctx)?;
    }
    let func_type = check_expr(func, ctx)?;
    if let TypeExpr::Function(_, ret) = func_type {
        return Ok(*ret);
    }

    Err(Diagnostic::error(
        ErrorCode::E3006,
        format!("expression is not callable: {:?}", func),
    )
    .with_label(ctx.make_span(0..0), "not a function"))
}

/// Check a method call expression
pub fn check_method_call(
    receiver: &Expr,
    method: &str,
    args: &[Expr],
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    let receiver_type = check_expr(receiver, ctx)?;
    for arg in args {
        check_expr(arg, ctx)?;
    }

    // Handle list methods
    if let TypeExpr::List(elem_type) = &receiver_type {
        match method {
            "push" | "pop" | "slice" => return Ok(receiver_type.clone()),
            "first" | "last" => return Ok(TypeExpr::Optional(elem_type.clone())),
            "len" => return Ok(TypeExpr::Named("int".to_string())),
            "join" => return Ok(TypeExpr::Named("str".to_string())),
            _ => {} // Fall through to extension method check
        }
    } else if let TypeExpr::Named(name) = &receiver_type {
        if name == "str" {
            match method {
                "len" => return Ok(TypeExpr::Named("int".to_string())),
                "slice" => return Ok(TypeExpr::Named("str".to_string())),
                "split" => return Ok(TypeExpr::List(Box::new(TypeExpr::Named("str".to_string())))),
                "trim" | "upper" | "lower" => return Ok(TypeExpr::Named("str".to_string())),
                _ => {} // Fall through to extension method check
            }
        }
    }

    // Check for imported extension methods
    // Look through all traits to find if any have this method imported
    if let Some((trait_name, ext_method)) = find_imported_extension_method(method, ctx) {
        // Found an imported extension method - return its return type
        // TODO: Verify the receiver type implements the trait
        let _ = trait_name; // Will be used for trait impl verification
        return Ok(ext_method.sig.return_type.clone());
    }

    // Check if there's an unimported extension method that could help
    if let Some((trait_name, _)) = find_any_extension_method(method, ctx) {
        return Err(Diagnostic::error(
            ErrorCode::E3008,
            format!(
                "method '{}' is an extension method on trait '{}'",
                method, trait_name
            ),
        )
        .with_label(ctx.make_span(0..0), "extension method not imported")
        .with_help(format!(
            "add: extension <module> {{ {}.{} }}",
            trait_name, method
        )));
    }

    Err(Diagnostic::error(
        ErrorCode::E3008,
        format!(
            "cannot call method '{}' on type {:?}",
            method, receiver_type
        ),
    )
    .with_label(ctx.make_span(0..0), "method not found"))
}

/// Find an imported extension method by name
fn find_imported_extension_method(
    method_name: &str,
    ctx: &TypeContext,
) -> Option<(String, crate::types::registries::ExtensionMethod)> {
    // Iterate through all extension registrations to find imported methods
    for (trait_name, methods) in ctx.extensions.iter() {
        if let Some(ext_method) = methods.get(method_name) {
            if ctx.is_extension_imported(trait_name, method_name) {
                return Some((trait_name.clone(), ext_method.clone()));
            }
        }
    }
    None
}

/// Find any extension method by name (for error messages)
fn find_any_extension_method(
    method_name: &str,
    ctx: &TypeContext,
) -> Option<(String, crate::types::registries::ExtensionMethod)> {
    for (trait_name, methods) in ctx.extensions.iter() {
        if let Some(ext_method) = methods.get(method_name) {
            return Some((trait_name.clone(), ext_method.clone()));
        }
    }
    None
}
