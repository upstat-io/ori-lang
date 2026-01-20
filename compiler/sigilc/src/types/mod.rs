// Type checker for Sigil
// Validates types and produces a typed AST
// STRICT: No unknown types allowed - all types must be determined at compile time

use crate::ast::*;
use std::collections::HashMap;

/// Type-checked AST (same structure, but verified)
pub type TypedModule = Module;

/// Type checking context
pub struct TypeContext {
    /// Type definitions
    types: HashMap<String, TypeDef>,

    /// Function signatures
    functions: HashMap<String, FunctionSig>,

    /// Config variables
    configs: HashMap<String, TypeExpr>,

    /// Local variable types (in current scope)
    locals: HashMap<String, TypeExpr>,

    /// Current function's return type (for `self` calls in recurse)
    current_return_type: Option<TypeExpr>,
}

#[derive(Clone)]
pub struct FunctionSig {
    pub type_params: Vec<String>,
    pub params: Vec<(String, TypeExpr)>,
    pub return_type: TypeExpr,
}

impl TypeContext {
    pub fn new() -> Self {
        let mut ctx = TypeContext {
            types: HashMap::new(),
            functions: HashMap::new(),
            configs: HashMap::new(),
            locals: HashMap::new(),
            current_return_type: None,
        };

        // Register builtin functions
        ctx.register_builtins();
        ctx
    }

    fn register_builtins(&mut self) {
        // print: any -> void
        self.functions.insert(
            "print".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // str: any -> str (conversion)
        self.functions.insert(
            "str".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("str".to_string()),
            },
        );

        // int: any -> int (conversion)
        self.functions.insert(
            "int".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );

        // float: any -> float (conversion)
        self.functions.insert(
            "float".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("float".to_string()),
            },
        );

        // len: any -> int (polymorphic: works on strings and lists)
        self.functions.insert(
            "len".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("value".to_string(), TypeExpr::Named("any".to_string()))],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );

        // assert: bool -> void
        self.functions.insert(
            "assert".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![("condition".to_string(), TypeExpr::Named("bool".to_string()))],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // assert_eq: (T, T) -> void
        self.functions.insert(
            "assert_eq".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("actual".to_string(), TypeExpr::Named("T".to_string())),
                    ("expected".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // assert_err: Result T E -> void
        self.functions.insert(
            "assert_err".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string(), "E".to_string()],
                params: vec![(
                    "result".to_string(),
                    TypeExpr::Generic(
                        "Result".to_string(),
                        vec![
                            TypeExpr::Named("T".to_string()),
                            TypeExpr::Named("E".to_string()),
                        ],
                    ),
                )],
                return_type: TypeExpr::Named("void".to_string()),
            },
        );

        // Arithmetic operators as first-class functions
        // +: (T, T) -> T where T is numeric
        self.functions.insert(
            "+".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // -: (T, T) -> T where T is numeric
        self.functions.insert(
            "-".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // *: (T, T) -> T where T is numeric
        self.functions.insert(
            "*".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // /: (T, T) -> T where T is numeric
        self.functions.insert(
            "/".to_string(),
            FunctionSig {
                type_params: vec!["T".to_string()],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("T".to_string())),
                    ("b".to_string(), TypeExpr::Named("T".to_string())),
                ],
                return_type: TypeExpr::Named("T".to_string()),
            },
        );

        // %: (int, int) -> int
        self.functions.insert(
            "%".to_string(),
            FunctionSig {
                type_params: vec![],
                params: vec![
                    ("a".to_string(), TypeExpr::Named("int".to_string())),
                    ("b".to_string(), TypeExpr::Named("int".to_string())),
                ],
                return_type: TypeExpr::Named("int".to_string()),
            },
        );
    }

    fn set_current_return_type(&mut self, ty: TypeExpr) {
        self.current_return_type = Some(ty);
    }

    fn clear_current_return_type(&mut self) {
        self.current_return_type = None;
    }

    fn define_type(&mut self, name: String, def: TypeDef) {
        self.types.insert(name, def);
    }

    fn define_function(&mut self, name: String, sig: FunctionSig) {
        self.functions.insert(name, sig);
    }

    fn define_config(&mut self, name: String, ty: TypeExpr) {
        self.configs.insert(name, ty);
    }

    fn define_local(&mut self, name: String, ty: TypeExpr) {
        self.locals.insert(name, ty);
    }

    fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.get(name)
    }

    fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        self.configs.get(name)
    }

    fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.locals.get(name)
    }
}

pub fn check(module: Module) -> Result<TypedModule, String> {
    let mut ctx = TypeContext::new();

    // First pass: collect all type and function definitions
    for item in &module.items {
        match item {
            Item::TypeDef(td) => {
                ctx.define_type(td.name.clone(), td.clone());
            }
            Item::Function(fd) => {
                let sig = FunctionSig {
                    type_params: fd.type_params.clone(),
                    params: fd
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), p.ty.clone()))
                        .collect(),
                    return_type: fd.return_type.clone(),
                };
                ctx.define_function(fd.name.clone(), sig);
            }
            Item::Config(cd) => {
                let ty = if let Some(t) = cd.ty.clone() {
                    t
                } else {
                    infer_type(&cd.value).map_err(|e| format!("Config '{}': {}", cd.name, e))?
                };
                ctx.define_config(cd.name.clone(), ty);
            }
            Item::Use(_) => {
                // TODO: Handle imports
            }
            Item::Test(_) => {
                // Tests are checked separately
            }
        }
    }

    // Second pass: type check all expressions
    for item in &module.items {
        match item {
            Item::Function(fd) => {
                check_function(fd, &mut ctx)?;
            }
            Item::Config(cd) => {
                check_config(cd, &ctx)?;
            }
            Item::Test(td) => {
                check_test(td, &ctx)?;
            }
            _ => {}
        }
    }

    Ok(module)
}

fn check_function(fd: &FunctionDef, ctx: &mut TypeContext) -> Result<(), String> {
    // Save old state
    let old_locals = ctx.locals.clone();
    let old_return_type = ctx.current_return_type.clone();

    // Set current return type for self() calls
    ctx.set_current_return_type(fd.return_type.clone());

    // Add parameters to local scope
    for param in &fd.params {
        ctx.define_local(param.name.clone(), param.ty.clone());
    }

    // Check body expression with return type as hint (for lambdas that are directly returned)
    let body_type = check_expr_with_hint(&fd.body, ctx, Some(&fd.return_type))?;

    // Verify return type matches
    if !types_compatible(&body_type, &fd.return_type, ctx) {
        return Err(format!(
            "Function '{}' returns {:?} but body has type {:?}",
            fd.name, fd.return_type, body_type
        ));
    }

    // Restore state
    ctx.locals = old_locals;
    ctx.current_return_type = old_return_type;

    Ok(())
}

fn check_config(cd: &ConfigDef, ctx: &TypeContext) -> Result<(), String> {
    let value_type = check_expr(&cd.value, ctx)?;

    if let Some(ref declared) = cd.ty {
        if !types_compatible(&value_type, declared, ctx) {
            return Err(format!(
                "Config '{}' declared as {:?} but value has type {:?}",
                cd.name, declared, value_type
            ));
        }
    }

    Ok(())
}

/// Check an expression with an optional expected type hint for bidirectional type inference
fn check_expr_with_hint(
    expr: &Expr,
    ctx: &TypeContext,
    expected: Option<&TypeExpr>,
) -> Result<TypeExpr, String> {
    match expr {
        Expr::Lambda { params, body } => check_lambda(params, body, ctx, expected),
        // Empty list can be inferred from expected type
        Expr::List(exprs) if exprs.is_empty() => {
            if let Some(TypeExpr::List(elem_type)) = expected {
                return Ok(TypeExpr::List(elem_type.clone()));
            }
            // Fall through to regular check which will use current_return_type
            check_expr_inner(expr, ctx)
        }
        _ => check_expr_inner(expr, ctx),
    }
}

fn check_expr(expr: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    check_expr_with_hint(expr, ctx, None)
}

/// Check an expression within a block context (where assignments can modify scope)
fn check_block_expr(expr: &Expr, ctx: &mut TypeContext) -> Result<TypeExpr, String> {
    match expr {
        Expr::Assign { target, value } => {
            let value_type = check_expr_with_hint(value, ctx, None)?;
            // Add the variable to the context so subsequent expressions can use it
            ctx.define_local(target.clone(), value_type);
            Ok(TypeExpr::Named("void".to_string()))
        }
        Expr::For {
            binding,
            iterator,
            body,
        } => {
            let iter_type = check_expr(iterator, ctx)?;
            // Get element type from iterator
            let elem_type = match &iter_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                TypeExpr::Named(n) if n == "Range" => TypeExpr::Named("int".to_string()),
                _ => return Err(format!("Cannot iterate over {:?}", iter_type)),
            };
            // Add loop binding to context
            ctx.define_local(binding.clone(), elem_type);
            check_block_expr(body, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }
        // For other expressions, delegate to immutable check
        _ => check_expr_with_hint(expr, ctx, None),
    }
}

/// Check a lambda expression with optional expected function type
fn check_lambda(
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
    let mut child_ctx = TypeContext {
        types: ctx.types.clone(),
        functions: ctx.functions.clone(),
        configs: ctx.configs.clone(),
        locals: ctx.locals.clone(),
        current_return_type: ctx.current_return_type.clone(),
    };

    // Add lambda parameters to the child context
    for (name, ty) in params.iter().zip(param_types.iter()) {
        child_ctx.locals.insert(name.clone(), ty.clone());
    }

    // Check the body with the child context
    let body_type = check_expr_inner(body, &child_ctx)?;

    // Build the function type
    if params.len() == 1 {
        Ok(TypeExpr::Function(
            Box::new(param_types.into_iter().next().unwrap()),
            Box::new(body_type),
        ))
    } else {
        Ok(TypeExpr::Function(
            Box::new(TypeExpr::Tuple(param_types)),
            Box::new(body_type),
        ))
    }
}

fn check_expr_inner(expr: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    match expr {
        Expr::Int(_) => Ok(TypeExpr::Named("int".to_string())),
        Expr::Float(_) => Ok(TypeExpr::Named("float".to_string())),
        Expr::String(_) => Ok(TypeExpr::Named("str".to_string())),
        Expr::Bool(_) => Ok(TypeExpr::Named("bool".to_string())),
        Expr::Nil => Ok(TypeExpr::Named("nil".to_string())),

        Expr::Ident(name) => {
            if let Some(ty) = ctx.lookup_local(name) {
                Ok(ty.clone())
            } else if let Some(sig) = ctx.lookup_function(name) {
                // Return function type
                Ok(sig.return_type.clone())
            } else {
                Err(format!("Unknown identifier: {}", name))
            }
        }

        Expr::Config(name) => ctx
            .lookup_config(name)
            .cloned()
            .ok_or_else(|| format!("Unknown config: ${}", name)),

        Expr::List(exprs) => {
            if exprs.is_empty() {
                // Empty list gets type from context (function return type)
                if let Some(ref ret_type) = ctx.current_return_type {
                    if let TypeExpr::List(elem_type) = ret_type {
                        return Ok(TypeExpr::List(elem_type.clone()));
                    }
                }
                // For empty lists in other contexts, we need to infer from usage
                // For now, allow it if we're in a context where the type is clear
                Err("Cannot infer type of empty list. Add a type annotation or ensure context provides the type.".to_string())
            } else {
                let elem_type = check_expr(&exprs[0], ctx)?;
                // Check all elements have the same type
                for (i, e) in exprs.iter().enumerate().skip(1) {
                    let t = check_expr(e, ctx)?;
                    if !types_compatible(&t, &elem_type, ctx) {
                        return Err(format!(
                            "List element {} has type {:?} but expected {:?}",
                            i, t, elem_type
                        ));
                    }
                }
                Ok(TypeExpr::List(Box::new(elem_type)))
            }
        }

        Expr::Binary { op, left, right } => {
            // For equality/comparison, use left type as hint for right
            match op {
                BinaryOp::Eq | BinaryOp::NotEq => {
                    let left_type = check_expr(left, ctx)?;
                    // Use left type as hint for right side (helps with empty lists)
                    check_expr_with_hint(right, ctx, Some(&left_type))?;
                    Ok(TypeExpr::Named("bool".to_string()))
                }
                BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                    check_expr(left, ctx)?;
                    check_expr(right, ctx)?;
                    Ok(TypeExpr::Named("bool".to_string()))
                }
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::IntDiv
                | BinaryOp::Mod => {
                    let left_type = check_expr(left, ctx)?;
                    let right_type = check_expr(right, ctx)?;
                    if is_numeric(&left_type) && is_numeric(&right_type) {
                        Ok(left_type)
                    } else if matches!((&left_type, op), (TypeExpr::Named(n), BinaryOp::Add) if n == "str")
                    {
                        Ok(TypeExpr::Named("str".to_string()))
                    } else if matches!(
                        (&left_type, &right_type, op),
                        (TypeExpr::List(_), TypeExpr::List(_), BinaryOp::Add)
                    ) {
                        Ok(left_type)
                    } else {
                        Err(format!(
                            "Cannot apply {:?} to {:?} and {:?}",
                            op, left_type, right_type
                        ))
                    }
                }
                BinaryOp::And | BinaryOp::Or => {
                    check_expr(left, ctx)?;
                    check_expr(right, ctx)?;
                    Ok(TypeExpr::Named("bool".to_string()))
                }
                BinaryOp::Pipe => {
                    check_expr(left, ctx)?;
                    let right_type = check_expr(right, ctx)?;
                    Ok(right_type)
                }
            }
        }

        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                // `self` is a special recursive call - use current function's return type
                if name == "self" {
                    // Check args without type hints for self calls
                    for arg in args {
                        check_expr(arg, ctx)?;
                    }
                    return ctx
                        .current_return_type
                        .clone()
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

                    // For generic functions like assert_eq, infer type param from first arg
                    // then use it for subsequent args with the same type param
                    let mut inferred_types: HashMap<String, TypeExpr> = HashMap::new();

                    for (i, arg) in args.iter().enumerate() {
                        if let Some((param_name, param_type)) = sig.params.get(i) {
                            // If param type is a type parameter, check if we've inferred it
                            if let TypeExpr::Named(type_name) = param_type {
                                if sig.type_params.contains(type_name) {
                                    // It's a type parameter
                                    let arg_type = check_expr_with_hint(
                                        arg,
                                        ctx,
                                        inferred_types.get(type_name),
                                    )?;
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

        Expr::Ok(inner) => {
            let inner_type = check_expr(inner, ctx)?;
            // For Ok, we know the success type but error type comes from context
            if let Some(ref ret_type) = ctx.current_return_type {
                if let TypeExpr::Generic(name, args) = ret_type {
                    if name == "Result" && args.len() == 2 {
                        return Ok(TypeExpr::Generic(
                            "Result".to_string(),
                            vec![inner_type, args[1].clone()],
                        ));
                    }
                }
            }
            Ok(TypeExpr::Generic(
                "Result".to_string(),
                vec![inner_type, TypeExpr::Named("void".to_string())],
            ))
        }

        Expr::Err(inner) => {
            let inner_type = check_expr(inner, ctx)?;
            // For Err, we know the error type but success type comes from context
            if let Some(ref ret_type) = ctx.current_return_type {
                if let TypeExpr::Generic(name, args) = ret_type {
                    if name == "Result" && args.len() == 2 {
                        return Ok(TypeExpr::Generic(
                            "Result".to_string(),
                            vec![args[0].clone(), inner_type],
                        ));
                    }
                }
            }
            Ok(TypeExpr::Generic(
                "Result".to_string(),
                vec![TypeExpr::Named("void".to_string()), inner_type],
            ))
        }

        Expr::Some(inner) => {
            let inner_type = check_expr(inner, ctx)?;
            Ok(TypeExpr::Optional(Box::new(inner_type)))
        }

        Expr::None_ => {
            // None needs context to determine the inner type
            if let Some(ref ret_type) = ctx.current_return_type {
                if let TypeExpr::Optional(inner) = ret_type {
                    return Ok(TypeExpr::Optional(inner.clone()));
                }
            }
            Err(
                "Cannot infer type of None. Use in a context where the optional type is clear."
                    .to_string(),
            )
        }

        Expr::Match(m) => {
            // Check scrutinee
            check_expr(&m.scrutinee, ctx)?;

            // All arms must have the same type
            if m.arms.is_empty() {
                return Err("Match expression has no arms".to_string());
            }

            let first_type = check_expr(&m.arms[0].body, ctx)?;
            for (i, arm) in m.arms.iter().enumerate().skip(1) {
                let arm_type = check_expr(&arm.body, ctx)?;
                if !types_compatible(&arm_type, &first_type, ctx) {
                    return Err(format!(
                        "Match arm {} has type {:?} but expected {:?}",
                        i, arm_type, first_type
                    ));
                }
            }
            Ok(first_type)
        }

        Expr::Block(exprs) => {
            if exprs.is_empty() {
                return Ok(TypeExpr::Named("void".to_string()));
            }
            // Create a child context for block scope
            let mut block_ctx = TypeContext {
                types: ctx.types.clone(),
                functions: ctx.functions.clone(),
                configs: ctx.configs.clone(),
                locals: ctx.locals.clone(),
                current_return_type: ctx.current_return_type.clone(),
            };
            // Check all expressions, tracking assignments
            let mut last_type = TypeExpr::Named("void".to_string());
            for expr in exprs.iter() {
                last_type = check_block_expr(expr, &mut block_ctx)?;
            }
            Ok(last_type)
        }

        Expr::Pattern(p) => check_pattern_expr(p, ctx),

        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let receiver_type = check_expr(receiver, ctx)?;
            for arg in args {
                check_expr(arg, ctx)?;
            }

            // Handle list methods
            if let TypeExpr::List(elem_type) = &receiver_type {
                match method.as_str() {
                    "push" | "pop" | "slice" => Ok(receiver_type.clone()),
                    "first" | "last" => Ok(TypeExpr::Optional(elem_type.clone())),
                    "len" => Ok(TypeExpr::Named("int".to_string())),
                    _ => Err(format!("Unknown list method: {}", method)),
                }
            } else if let TypeExpr::Named(name) = &receiver_type {
                if name == "str" {
                    match method.as_str() {
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

        Expr::Index(arr, _index) => {
            let arr_type = check_expr(arr, ctx)?;
            if let TypeExpr::List(elem_type) = arr_type {
                Ok(*elem_type)
            } else if let TypeExpr::Named(name) = &arr_type {
                if name == "str" {
                    Ok(TypeExpr::Named("str".to_string()))
                } else {
                    Err(format!("Cannot index into type {:?}", arr_type))
                }
            } else {
                Err(format!("Cannot index into type {:?}", arr_type))
            }
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_type = check_expr(condition, ctx)?;
            if !types_compatible(&cond_type, &TypeExpr::Named("bool".to_string()), ctx) {
                return Err(format!("If condition must be bool, got {:?}", cond_type));
            }

            let then_type = check_expr(then_branch, ctx)?;

            if let Some(else_expr) = else_branch {
                let else_type = check_expr(else_expr, ctx)?;
                if !types_compatible(&then_type, &else_type, ctx) {
                    return Err(format!(
                        "If branches have different types: then={:?}, else={:?}",
                        then_type, else_type
                    ));
                }
            }
            Ok(then_type)
        }

        Expr::LengthPlaceholder => Ok(TypeExpr::Named("int".to_string())),

        Expr::Range { start, end } => {
            let start_type = check_expr(start, ctx)?;
            let end_type = check_expr(end, ctx)?;
            if !is_numeric(&start_type) || !is_numeric(&end_type) {
                return Err(format!(
                    "Range bounds must be numeric, got {:?}..{:?}",
                    start_type, end_type
                ));
            }
            // Range is a special type that can be iterated
            Ok(TypeExpr::Named("Range".to_string()))
        }

        Expr::Lambda { params, body } => {
            // Lambdas without context must be checked via check_lambda with a type hint
            // If we get here, no type hint was provided
            check_lambda(params, body, ctx, None)
        }

        Expr::Tuple(exprs) => {
            let types: Result<Vec<_>, _> = exprs.iter().map(|e| check_expr(e, ctx)).collect();
            Ok(TypeExpr::Tuple(types?))
        }

        Expr::Field(expr, field) => {
            let expr_type = check_expr(expr, ctx)?;
            match &expr_type {
                // Anonymous record type - look up field directly
                TypeExpr::Record(fields) => {
                    if let Some((_, field_type)) = fields.iter().find(|(n, _)| n == field) {
                        Ok(field_type.clone())
                    } else {
                        Err(format!("Record has no field '{}'", field))
                    }
                }
                // Named struct type - look up struct definition
                TypeExpr::Named(type_name) => {
                    if let Some(type_def) = ctx.lookup_type(type_name) {
                        if let TypeDefKind::Struct(struct_fields) = &type_def.kind {
                            if let Some(f) = struct_fields.iter().find(|f| &f.name == field) {
                                Ok(f.ty.clone())
                            } else {
                                Err(format!("Struct '{}' has no field '{}'", type_name, field))
                            }
                        } else {
                            Err(format!("Type '{}' is not a struct", type_name))
                        }
                    } else {
                        Err(format!(
                            "Cannot access field '{}' on type {:?}",
                            field, expr_type
                        ))
                    }
                }
                _ => Err(format!(
                    "Cannot access field '{}' on type {:?}",
                    field, expr_type
                )),
            }
        }

        Expr::Struct { name, fields } => {
            // Check field expressions
            for (_, expr) in fields {
                check_expr(expr, ctx)?;
            }
            Ok(TypeExpr::Named(name.clone()))
        }

        Expr::Coalesce { value, default } => {
            let value_type = check_expr(value, ctx)?;
            let default_type = check_expr(default, ctx)?;

            // value should be Optional<T>, default should be T
            if let TypeExpr::Optional(inner) = value_type {
                if types_compatible(&default_type, &inner, ctx) {
                    Ok(*inner)
                } else {
                    Err(format!(
                        "Coalesce default type {:?} doesn't match optional inner type {:?}",
                        default_type, inner
                    ))
                }
            } else {
                Err(format!(
                    "Coalesce (??) requires optional type, got {:?}",
                    value_type
                ))
            }
        }

        Expr::For {
            binding: _,
            iterator,
            body,
        } => {
            check_expr(iterator, ctx)?;
            check_expr(body, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }

        Expr::Assign { target: _, value } => {
            check_expr(value, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }

        Expr::MapLiteral(entries) => {
            if entries.is_empty() {
                return Err("Cannot infer type of empty map literal".to_string());
            }
            let (key, value) = &entries[0];
            let key_type = check_expr(key, ctx)?;
            let value_type = check_expr(value, ctx)?;
            Ok(TypeExpr::Map(Box::new(key_type), Box::new(value_type)))
        }

        Expr::Unwrap(inner) => {
            let inner_type = check_expr(inner, ctx)?;
            match inner_type {
                TypeExpr::Optional(t) => Ok(*t),
                TypeExpr::Generic(name, args) if name == "Result" && args.len() >= 1 => {
                    Ok(args[0].clone())
                }
                _ => Err(format!(
                    "Cannot unwrap non-optional/non-result type: {:?}",
                    inner_type
                )),
            }
        }

        Expr::Unary { op, operand } => {
            let operand_type = check_expr(operand, ctx)?;
            match op {
                UnaryOp::Neg => {
                    if is_numeric(&operand_type) {
                        Ok(operand_type)
                    } else {
                        Err(format!(
                            "Cannot negate non-numeric type: {:?}",
                            operand_type
                        ))
                    }
                }
                UnaryOp::Not => {
                    if types_compatible(&operand_type, &TypeExpr::Named("bool".to_string()), ctx) {
                        Ok(TypeExpr::Named("bool".to_string()))
                    } else {
                        Err(format!(
                            "Cannot apply ! to non-bool type: {:?}",
                            operand_type
                        ))
                    }
                }
            }
        }
    }
}

fn check_pattern_expr(p: &PatternExpr, ctx: &TypeContext) -> Result<TypeExpr, String> {
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

fn types_compatible(actual: &TypeExpr, expected: &TypeExpr, _ctx: &TypeContext) -> bool {
    match (actual, expected) {
        // 'any' is compatible with everything (for builtins)
        (_, TypeExpr::Named(e)) if e == "any" => true,
        (TypeExpr::Named(a), _) if a == "any" => true,

        // Type parameters (single uppercase letter) match anything
        // This handles generic function definitions like len: [T] -> int
        (_, TypeExpr::Named(e)) if is_type_parameter(e) => true,
        (TypeExpr::Named(a), _) if is_type_parameter(a) => true,

        // void is compatible with itself
        (TypeExpr::Named(a), TypeExpr::Named(e)) if a == "void" && e == "void" => true,

        // Named types must match exactly
        (TypeExpr::Named(a), TypeExpr::Named(e)) => a == e,

        (TypeExpr::Optional(a), TypeExpr::Optional(e)) => types_compatible(a, e, _ctx),
        (TypeExpr::List(a), TypeExpr::List(e)) => types_compatible(a, e, _ctx),
        (TypeExpr::Generic(na, aa), TypeExpr::Generic(ne, ae)) => {
            na == ne
                && aa.len() == ae.len()
                && aa
                    .iter()
                    .zip(ae.iter())
                    .all(|(a, e)| types_compatible(a, e, _ctx))
        }
        (TypeExpr::Function(a_param, a_ret), TypeExpr::Function(e_param, e_ret)) => {
            types_compatible(a_param, e_param, _ctx) && types_compatible(a_ret, e_ret, _ctx)
        }
        (TypeExpr::Tuple(a), TypeExpr::Tuple(e)) => {
            a.len() == e.len()
                && a.iter()
                    .zip(e.iter())
                    .all(|(a, e)| types_compatible(a, e, _ctx))
        }
        (TypeExpr::Map(ak, av), TypeExpr::Map(ek, ev)) => {
            types_compatible(ak, ek, _ctx) && types_compatible(av, ev, _ctx)
        }
        // Record types are compatible if they have the same fields with compatible types
        (TypeExpr::Record(a_fields), TypeExpr::Record(e_fields)) => {
            if a_fields.len() != e_fields.len() {
                return false;
            }
            for (a_name, a_type) in a_fields {
                if let Some((_, e_type)) = e_fields.iter().find(|(n, _)| n == a_name) {
                    if !types_compatible(a_type, e_type, _ctx) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

fn is_numeric(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Named(n) if n == "int" || n == "float")
}

/// Check if a type name is a type parameter (single uppercase letter like T, E, K, V)
fn is_type_parameter(name: &str) -> bool {
    name.len() == 1
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
}

fn infer_type(expr: &Expr) -> Result<TypeExpr, String> {
    match expr {
        Expr::Int(_) => Ok(TypeExpr::Named("int".to_string())),
        Expr::Float(_) => Ok(TypeExpr::Named("float".to_string())),
        Expr::String(_) => Ok(TypeExpr::Named("str".to_string())),
        Expr::Bool(_) => Ok(TypeExpr::Named("bool".to_string())),
        Expr::List(items) if !items.is_empty() => {
            let elem_type = infer_type(&items[0])?;
            Ok(TypeExpr::List(Box::new(elem_type)))
        }
        _ => Err("Cannot infer type - explicit type annotation required".to_string()),
    }
}

fn check_test(td: &TestDef, ctx: &TypeContext) -> Result<(), String> {
    // Check that the target function exists
    if ctx.lookup_function(&td.target).is_none() {
        return Err(format!(
            "Test '{}' references unknown function '@{}'",
            td.name, td.target
        ));
    }

    // Type check the test body
    check_expr(&td.body, ctx)?;

    Ok(())
}

use crate::ast::TestDef;
