// Interpreter/Evaluator for Sigil
//
// Module structure following Rust's pattern:
// - mod.rs: Main eval logic and public API
// - value.rs: Value type, Environment, is_truthy
// - operators.rs: Binary and unary operator evaluation
// - builtins.rs: Builtin functions and method calls

mod builtins;
mod operators;
mod value;

use crate::ast::*;
use std::collections::HashMap;

// Re-export core types for external use
pub use value::{is_truthy, Environment, Value};

// Internal imports from submodules
use builtins::{eval_builtin, eval_method_call};
use operators::{eval_binary_op, eval_unary_op};

/// Run a module (find and execute main function)
pub fn run(module: Module) -> Result<(), String> {
    let mut env = Environment::new();

    // First pass: collect configs and functions
    for item in &module.items {
        match item {
            Item::Config(cd) => {
                let value = eval_expr(&cd.value, &env)?;
                env.set_config(cd.name.clone(), value);
            }
            Item::Function(fd) => {
                env.define_function(fd.name.clone(), fd.clone());
            }
            _ => {}
        }
    }

    // Find and run main function
    if let Some(main_fn) = env.get_function("main").cloned() {
        eval_expr(&main_fn.body, &env)?;
    }

    Ok(())
}

/// Run a single test
pub fn run_test(module: &Module, test: &TestDef) -> Result<(), String> {
    let mut env = Environment::new();

    // First pass: collect configs and functions
    for item in &module.items {
        match item {
            Item::Config(cd) => {
                let value = eval_expr(&cd.value, &env)?;
                env.set_config(cd.name.clone(), value);
            }
            Item::Function(fd) => {
                env.define_function(fd.name.clone(), fd.clone());
            }
            _ => {}
        }
    }

    // Run the test body
    eval_expr(&test.body, &env)?;

    Ok(())
}

/// REPL support: evaluate a single line
pub fn eval_line(input: &str, env: &mut Environment) -> Result<String, String> {
    let tokens = crate::lexer::tokenize(input, "<repl>")?;
    let module = crate::parser::parse(tokens, "<repl>")?;

    for item in module.items {
        match item {
            Item::Config(cd) => {
                let value = eval_expr(&cd.value, env)?;
                env.set_config(cd.name.clone(), value.clone());
                return Ok(format!("${} = {}", cd.name, value));
            }
            Item::Function(fd) => {
                env.define_function(fd.name.clone(), fd.clone());
                return Ok(format!("@{} defined", fd.name));
            }
            _ => {}
        }
    }

    Ok(String::new())
}

/// Get type of expression (for REPL)
pub fn type_of(_expr: &str, _env: &Environment) -> Result<String, String> {
    // TODO: Implement type inference for REPL
    Ok("unknown".to_string())
}

// ============================================================================
// Expression Evaluation
// ============================================================================

/// Evaluate an expression within a block context (where assignments are allowed)
fn eval_block_expr(expr: &Expr, env: &mut Environment) -> Result<Value, String> {
    match expr {
        Expr::Assign { target, value } => {
            let val = eval_expr(value, env)?;
            env.set(target.clone(), val);
            Ok(Value::Nil)
        }
        Expr::For {
            binding,
            iterator,
            body,
        } => {
            let iter_val = eval_expr(iterator, env)?;
            match iter_val {
                Value::List(items) => {
                    for item in items {
                        env.set(binding.clone(), item);
                        eval_block_expr(body, env)?;
                    }
                }
                _ => return Err("For loop requires iterable".to_string()),
            }
            Ok(Value::Nil)
        }
        // For other expressions, delegate to eval_expr with immutable ref
        _ => eval_expr(expr, env),
    }
}

fn eval_expr(expr: &Expr, env: &Environment) -> Result<Value, String> {
    match expr {
        // Literals
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Float(f) => Ok(Value::Float(*f)),
        Expr::String(s) => Ok(Value::String(s.clone())),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Nil => Ok(Value::Nil),

        // Identifiers
        Expr::Ident(name) => {
            // Check for operator functions first (used in fold, etc.)
            match name.as_str() {
                "+" | "-" | "*" | "/" => {
                    return Ok(Value::BuiltinFunction(name.clone()));
                }
                _ => {}
            }
            env.get(name)
                .ok_or_else(|| format!("Unknown variable: {}", name))
        }

        Expr::Config(name) => env
            .configs
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Unknown config: ${}", name)),

        // Collections
        Expr::List(items) => {
            let values: Result<Vec<_>, _> = items.iter().map(|e| eval_expr(e, env)).collect();
            Ok(Value::List(values?))
        }

        Expr::Tuple(items) => {
            let values: Result<Vec<_>, _> = items.iter().map(|e| eval_expr(e, env)).collect();
            Ok(Value::Tuple(values?))
        }

        // Operators
        Expr::Binary { op, left, right } => {
            let lval = eval_expr(left, env)?;
            let rval = eval_expr(right, env)?;
            eval_binary_op(op, lval, rval)
        }

        Expr::Unary { op, operand } => {
            let val = eval_expr(operand, env)?;
            eval_unary_op(op, val)
        }

        // Function calls
        Expr::Call { func, args } => {
            let arg_values: Result<Vec<_>, _> = args.iter().map(|a| eval_expr(a, env)).collect();
            let arg_values = arg_values?;

            match func.as_ref() {
                Expr::Ident(name) => {
                    // Check for builtin functions
                    if let Some(result) = eval_builtin(name, &arg_values)? {
                        return Ok(result);
                    }

                    // Check if it's a closure stored in a local variable
                    if let Some(Value::Function {
                        params,
                        body,
                        env: closure_env,
                    }) = env.get(name)
                    {
                        // Create new environment with closure's captured environment
                        let mut call_env = Environment {
                            configs: env.configs.clone(),
                            functions: env.functions.clone(),
                            locals: closure_env.clone(),
                            current_params: params.clone(),
                        };
                        // Bind arguments to parameters
                        for (param, arg) in params.iter().zip(arg_values.iter()) {
                            call_env.set(param.clone(), arg.clone());
                        }
                        return eval_expr(&body, &call_env);
                    }

                    // User-defined function
                    if let Some(fd) = env.get_function(name).cloned() {
                        eval_function_call(&fd, arg_values, env)
                    } else {
                        Err(format!("Unknown function: {}", name))
                    }
                }
                _ => Err("Cannot call non-function".to_string()),
            }
        }

        // Method calls
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let recv = eval_expr(receiver, env)?;

            // Get length for $ substitution in method args (D-style)
            let len = match &recv {
                Value::List(items) => items.len() as i64,
                Value::String(s) => s.len() as i64,
                _ => 0,
            };

            // Evaluate args with $ = length support
            let arg_values: Result<Vec<_>, _> =
                args.iter().map(|a| eval_index_expr(a, env, len)).collect();
            eval_method_call(&recv, method, arg_values?, env)
        }

        // Field access
        Expr::Field(expr, field) => {
            let value = eval_expr(expr, env)?;
            match value {
                Value::Struct { fields, .. } => fields
                    .get(field)
                    .cloned()
                    .ok_or_else(|| format!("Unknown field: {}", field)),
                _ => Err("Cannot access field of non-struct".to_string()),
            }
        }

        // Indexing (with D-style $ support)
        Expr::Index(expr, index) => {
            let value = eval_expr(expr, env)?;

            let len = match &value {
                Value::List(items) => items.len() as i64,
                Value::String(s) => s.len() as i64,
                _ => 0,
            };

            let idx = eval_index_expr(index, env, len)?;

            match (value, idx) {
                (Value::List(items), Value::Int(i)) => items
                    .get(i as usize)
                    .cloned()
                    .ok_or_else(|| "Index out of bounds".to_string()),
                (Value::String(s), Value::Int(i)) => s
                    .chars()
                    .nth(i as usize)
                    .map(|c| Value::String(c.to_string()))
                    .ok_or_else(|| "Index out of bounds".to_string()),
                _ => Err("Cannot index".to_string()),
            }
        }

        // Result/Option types
        Expr::Ok(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Ok(Box::new(value)))
        }

        Expr::Err(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Err(Box::new(value)))
        }

        Expr::Some(inner) => {
            let value = eval_expr(inner, env)?;
            Ok(Value::Some(Box::new(value)))
        }

        Expr::None_ => Ok(Value::None_),

        Expr::Coalesce { value, default } => {
            let val = eval_expr(value, env)?;
            match val {
                Value::Nil | Value::None_ => eval_expr(default, env),
                Value::Some(inner) => Ok(*inner),
                other => Ok(other),
            }
        }

        // Control flow
        Expr::Match(m) => {
            let scrutinee = eval_expr(&m.scrutinee, env)?;
            eval_match(&scrutinee, &m.arms, env)
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond = eval_expr(condition, env)?;
            if is_truthy(&cond) {
                eval_expr(then_branch, env)
            } else if let Some(else_expr) = else_branch {
                eval_expr(else_expr, env)
            } else {
                Ok(Value::Nil)
            }
        }

        Expr::Block(exprs) => {
            // Blocks create a local scope where assignments can be made
            let mut local_env = Environment {
                configs: env.configs.clone(),
                functions: env.functions.clone(),
                locals: env.locals.clone(),
                current_params: env.current_params.clone(),
            };
            let mut result = Value::Nil;
            for e in exprs {
                result = eval_block_expr(e, &mut local_env)?;
            }
            Ok(result)
        }

        // Patterns (fold, map, filter, etc.)
        Expr::Pattern(p) => eval_pattern(p, env),

        // Lambda
        Expr::Lambda { params, body } => Ok(Value::Function {
            params: params.clone(),
            body: *body.clone(),
            env: env.locals.clone(),
        }),

        // Struct literal
        Expr::Struct { name, fields } => {
            let mut field_values = HashMap::new();
            for (fname, fexpr) in fields {
                field_values.insert(fname.clone(), eval_expr(fexpr, env)?);
            }
            Ok(Value::Struct {
                name: name.clone(),
                fields: field_values,
            })
        }

        // Range
        Expr::Range { start, end } => {
            let s = eval_expr(start, env)?;
            let e = eval_expr(end, env)?;
            match (s, e) {
                (Value::Int(start), Value::Int(end)) => {
                    let items: Vec<Value> = (start..end).map(Value::Int).collect();
                    Ok(Value::List(items))
                }
                _ => Err("Range requires integer bounds".to_string()),
            }
        }

        _ => Err(format!("Cannot evaluate expression: {:?}", expr)),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Evaluate index expression with # = array length
fn eval_index_expr(expr: &Expr, env: &Environment, length: i64) -> Result<Value, String> {
    match expr {
        Expr::LengthPlaceholder => Ok(Value::Int(length)),
        Expr::Binary { op, left, right } => {
            let l = eval_index_expr(left, env, length)?;
            let r = eval_index_expr(right, env, length)?;
            eval_binary_op(op, l, r)
        }
        Expr::Unary { op, operand } => {
            let val = eval_index_expr(operand, env, length)?;
            eval_unary_op(op, val)
        }
        _ => eval_expr(expr, env),
    }
}

/// Evaluate a function call
fn eval_function_call(
    fd: &FunctionDef,
    args: Vec<Value>,
    env: &Environment,
) -> Result<Value, String> {
    // Capture parameter names in order for recursion support
    let param_names: Vec<String> = fd.params.iter().map(|p| p.name.clone()).collect();

    let mut new_env = Environment {
        configs: env.configs.clone(),
        functions: env.functions.clone(),
        locals: HashMap::new(),      // Start with fresh locals
        current_params: param_names, // Store param names in order
    };

    for (param, value) in fd.params.iter().zip(args.into_iter()) {
        new_env.set(param.name.clone(), value);
    }

    eval_expr(&fd.body, &new_env)
}

/// Evaluate a match expression
fn eval_match(scrutinee: &Value, arms: &[MatchArm], env: &Environment) -> Result<Value, String> {
    for arm in arms {
        if let Some(bindings) = match_pattern(&arm.pattern, scrutinee, env)? {
            let mut new_env = Environment {
                configs: env.configs.clone(),
                current_params: env.current_params.clone(),
                functions: env.functions.clone(),
                locals: env.locals.clone(),
            };
            for (name, value) in bindings {
                new_env.set(name, value);
            }
            return eval_expr(&arm.body, &new_env);
        }
    }
    Err("No matching pattern".to_string())
}

/// Match a pattern against a value, returning bindings if successful
fn match_pattern(
    pattern: &Pattern,
    value: &Value,
    env: &Environment,
) -> Result<Option<Vec<(String, Value)>>, String> {
    match pattern {
        Pattern::Wildcard => Ok(Some(Vec::new())),

        Pattern::Binding(name) => Ok(Some(vec![(name.clone(), value.clone())])),

        Pattern::Literal(expr) => {
            let expected = eval_expr(expr, env)?;
            if expected == *value {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }

        Pattern::Variant { name, fields } => match (name.as_str(), value) {
            ("Ok", Value::Ok(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "value" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("Err", Value::Err(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "error" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("Some", Value::Some(inner)) => {
                let mut bindings = Vec::new();
                for (fname, fpat) in fields {
                    if fname == "value" {
                        if let Some(bs) = match_pattern(fpat, inner, env)? {
                            bindings.extend(bs);
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Ok(Some(bindings))
            }
            ("None", Value::None_) => Ok(Some(Vec::new())),
            _ => Ok(None),
        },

        Pattern::Condition(expr) => {
            let result = eval_expr(expr, env)?;
            if is_truthy(&result) {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }
    }
}

// ============================================================================
// Pattern Evaluation (fold, map, filter, etc.)
// ============================================================================

fn eval_pattern(pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
    match pattern {
        PatternExpr::Fold {
            collection,
            init,
            op,
        } => {
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
                            locals: fn_env.clone(),
                        };
                        call_env.set(params[0].clone(), acc);
                        call_env.set(params[1].clone(), item);
                        eval_expr(body, &call_env)?
                    }
                    _ => return Err("Invalid fold operation".to_string()),
                };
            }
            Ok(acc)
        }

        PatternExpr::Map {
            collection,
            transform,
        } => {
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
                            locals: fn_env.clone(),
                        };
                        if let Some(param) = params.first() {
                            call_env.set(param.clone(), item);
                        }
                        eval_expr(body, &call_env)?
                    }
                    _ => return Err("map requires a function".to_string()),
                };
                results.push(result);
            }
            Ok(Value::List(results))
        }

        PatternExpr::Filter {
            collection,
            predicate,
        } => {
            let coll = eval_expr(collection, env)?;
            let pred_val = eval_expr(predicate, env)?;

            let items = match coll {
                Value::List(items) => items,
                _ => return Err("filter requires a list".to_string()),
            };

            let mut results = Vec::new();
            for item in items {
                let keep = match &pred_val {
                    Value::Function {
                        params,
                        body,
                        env: fn_env,
                    } => {
                        let mut call_env = Environment {
                            configs: env.configs.clone(),
                            current_params: env.current_params.clone(),
                            functions: env.functions.clone(),
                            locals: fn_env.clone(),
                        };
                        if let Some(param) = params.first() {
                            call_env.set(param.clone(), item.clone());
                        }
                        is_truthy(&eval_expr(body, &call_env)?)
                    }
                    _ => return Err("filter requires a function".to_string()),
                };
                if keep {
                    results.push(item);
                }
            }
            Ok(Value::List(results))
        }

        PatternExpr::Collect { range, transform } => {
            let range_val = eval_expr(range, env)?;
            let transform_val = eval_expr(transform, env)?;

            let items = match range_val {
                Value::List(items) => items,
                _ => return Err("collect requires a range/list".to_string()),
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
                            locals: fn_env.clone(),
                        };
                        if let Some(param) = params.first() {
                            call_env.set(param.clone(), item);
                        }
                        eval_expr(body, &call_env)?
                    }
                    _ => {
                        if let Expr::Ident(name) = transform.as_ref() {
                            if let Some(fd) = env.get_function(name).cloned() {
                                eval_function_call(&fd, vec![item], env)?
                            } else {
                                return Err(format!("Unknown function: {}", name));
                            }
                        } else {
                            return Err("collect transform must be a function".to_string());
                        }
                    }
                };
                results.push(result);
            }
            Ok(Value::List(results))
        }

        PatternExpr::Count {
            collection,
            predicate,
        } => {
            let coll = eval_expr(collection, env)?;
            let pred_val = eval_expr(predicate, env)?;

            let items = match coll {
                Value::List(items) => items,
                Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                _ => return Err("count requires a collection".to_string()),
            };

            let mut count = 0;
            for item in items {
                let matches = match &pred_val {
                    Value::Function {
                        params,
                        body,
                        env: fn_env,
                    } => {
                        let mut call_env = Environment {
                            configs: env.configs.clone(),
                            current_params: env.current_params.clone(),
                            functions: env.functions.clone(),
                            locals: fn_env.clone(),
                        };
                        if let Some(param) = params.first() {
                            call_env.set(param.clone(), item);
                        }
                        is_truthy(&eval_expr(body, &call_env)?)
                    }
                    _ => return Err("count requires a predicate function".to_string()),
                };
                if matches {
                    count += 1;
                }
            }
            Ok(Value::Int(count))
        }

        PatternExpr::Recurse {
            condition,
            base_value,
            step,
            memo,
            parallel_threshold,
        } => {
            // For recurse, we need to be inside a function context
            // The recurse pattern creates a recursive function that:
            // 1. Checks condition - if true, returns base_value
            // 2. Otherwise evaluates step with self() for recursive calls

            // Use parameter names in order from the function call context
            let param_names = &env.current_params;

            // First evaluate condition
            let cond_result = eval_expr(condition, env)?;

            if is_truthy(&cond_result) {
                // Base case: return base_value
                eval_expr(base_value, env)
            } else {
                // Recursive case: evaluate step
                // Get current n value to check against parallel threshold (use first int param)
                let current_n = param_names
                    .iter()
                    .find_map(|name| {
                        if let Some(Value::Int(n)) = env.get(name) {
                            Some(n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                eval_recurse_step(
                    step,
                    condition,
                    base_value,
                    env,
                    *memo,
                    *parallel_threshold,
                    current_n,
                    param_names,
                )
            }
        }

        PatternExpr::Iterate {
            over,
            direction,
            into,
            with,
        } => {
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
                            locals: fn_env.clone(),
                        };
                        // Bind 'acc' and 'char'/'item' and 'i' for the operation
                        if params.len() >= 1 {
                            call_env.set(params[0].clone(), acc);
                        }
                        if params.len() >= 2 {
                            call_env.set(params[1].clone(), item.clone());
                        }
                        // Also bind common names used in iterate
                        call_env.set("acc".to_string(), call_env.get("acc").unwrap_or(Value::Nil));
                        call_env.set("char".to_string(), item.clone());
                        call_env.set("item".to_string(), item);
                        call_env.set("i".to_string(), Value::Int(i as i64));
                        eval_expr(body, &call_env)?
                    }
                    _ => return Err("iterate requires a function for 'with'".to_string()),
                };
            }
            Ok(acc)
        }

        PatternExpr::Transform { input, steps } => {
            // Transform passes a value through a series of transformation steps
            let mut value = eval_expr(input, env)?;

            for step_expr in steps {
                // Each step can be a function or a lambda
                let step_val = eval_expr(step_expr, env)?;
                value = match step_val {
                    Value::Function {
                        params,
                        body,
                        env: fn_env,
                    } => {
                        let mut call_env = Environment {
                            configs: env.configs.clone(),
                            current_params: env.current_params.clone(),
                            functions: env.functions.clone(),
                            locals: fn_env.clone(),
                        };
                        if let Some(param) = params.first() {
                            call_env.set(param.clone(), value);
                        }
                        // Also bind 'x' as common transform variable
                        call_env.set(
                            "x".to_string(),
                            call_env
                                .locals
                                .get(params.first().unwrap_or(&"x".to_string()))
                                .cloned()
                                .unwrap_or(Value::Nil),
                        );
                        eval_expr(&body, &call_env)?
                    }
                    _ => {
                        // If it's an identifier, try to call it as a function
                        if let Expr::Ident(name) = step_expr {
                            if let Some(fd) = env.get_function(name).cloned() {
                                eval_function_call(&fd, vec![value], env)?
                            } else {
                                return Err(format!("Unknown transform function: {}", name));
                            }
                        } else {
                            return Err("Transform step must be a function".to_string());
                        }
                    }
                };
            }
            Ok(value)
        }

        PatternExpr::Parallel {
            branches,
            timeout: _,
            on_error,
        } => {
            // Execute all branches concurrently using threads
            use std::sync::mpsc;
            use std::thread;

            let (tx, rx) = mpsc::channel();

            // Clone what we need for threads
            let configs = env.configs.clone();
            let functions = env.functions.clone();
            let locals = env.locals.clone();

            let mut handles = Vec::new();
            let branch_count = branches.len();

            for (name, expr) in branches.clone() {
                let tx = tx.clone();
                let configs = configs.clone();
                let functions = functions.clone();
                let locals = locals.clone();
                let current_params = env.current_params.clone();

                let handle = thread::spawn(move || {
                    let thread_env = Environment {
                        configs,
                        functions,
                        locals,
                        current_params,
                    };
                    let result = eval_expr(&expr, &thread_env);
                    tx.send((name, result)).unwrap();
                });
                handles.push(handle);
            }

            // Drop the original sender so rx knows when all threads are done
            drop(tx);

            // Collect results
            let mut results: HashMap<String, Value> = HashMap::new();
            let mut first_error: Option<String> = None;

            for _ in 0..branch_count {
                match rx.recv() {
                    Ok((name, Ok(value))) => {
                        results.insert(name, value);
                    }
                    Ok((name, Err(e))) => match on_error {
                        OnError::FailFast => {
                            if first_error.is_none() {
                                first_error =
                                    Some(format!("parallel branch '{}' failed: {}", name, e));
                            }
                        }
                        OnError::CollectAll => {
                            results.insert(name, Value::Err(Box::new(Value::String(e))));
                        }
                    },
                    Err(_) => break,
                }
            }

            // Wait for all threads
            for handle in handles {
                let _ = handle.join();
            }

            if let Some(err) = first_error {
                if matches!(on_error, OnError::FailFast) {
                    return Err(err);
                }
            }

            // Return as anonymous struct
            Ok(Value::Struct {
                name: "parallel".to_string(),
                fields: results,
            })
        }
    }
}

/// Evaluate the recursive step of a recurse pattern
/// This handles self() calls within the step expression
fn eval_recurse_step(
    step: &Expr,
    condition: &Expr,
    base_value: &Expr,
    env: &Environment,
    memo: bool,
    parallel_threshold: i64,
    current_n: i64,
    param_names: &[String],
) -> Result<Value, String> {
    // For self() calls, we need to substitute them with recursive evaluations
    eval_recurse_expr(
        step,
        step,
        condition,
        base_value,
        env,
        memo,
        parallel_threshold,
        current_n,
        &mut HashMap::new(),
        param_names,
    )
}

/// Recursively evaluate an expression, handling self() calls
/// `step` is the original step expression (for recursive calls)
/// `expr` is the current expression being evaluated
/// `parallel_threshold` - parallelize when n > threshold (i64::MAX = never)
/// `current_n` - the current value of n (for threshold comparison)
/// `param_names` - the names of the function parameters for binding self() args
fn eval_recurse_expr(
    expr: &Expr,
    step: &Expr,
    condition: &Expr,
    base_value: &Expr,
    env: &Environment,
    memo: bool,
    parallel_threshold: i64,
    current_n: i64,
    cache: &mut HashMap<Vec<i64>, Value>,
    param_names: &[String],
) -> Result<Value, String> {
    match expr {
        // Handle self() calls - this is the recursive invocation
        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "self" {
                    // This is a recursive call
                    // Evaluate the arguments in current environment
                    let arg_values: Result<Vec<Value>, String> = args
                        .iter()
                        .map(|a| {
                            eval_recurse_expr(
                                a,
                                step,
                                condition,
                                base_value,
                                env,
                                memo,
                                parallel_threshold,
                                current_n,
                                cache,
                                param_names,
                            )
                        })
                        .collect();
                    let arg_values = arg_values?;

                    // Create cache key from integer arguments
                    let cache_key: Vec<i64> = if memo {
                        arg_values
                            .iter()
                            .filter_map(|v| {
                                if let Value::Int(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    // Check cache if memoization is enabled
                    if memo && !cache_key.is_empty() {
                        if let Some(cached) = cache.get(&cache_key) {
                            return Ok(cached.clone());
                        }
                    }

                    // Create new environment with updated parameters
                    let mut new_env = Environment {
                        configs: env.configs.clone(),
                        current_params: env.current_params.clone(),
                        functions: env.functions.clone(),
                        locals: env.locals.clone(),
                    };

                    // Bind arguments to parameter names using positional binding
                    // param_names is in the correct order from the function definition
                    for (i, param_name) in param_names.iter().enumerate() {
                        if let Some(arg_val) = arg_values.get(i) {
                            new_env.set(param_name.clone(), arg_val.clone());
                        }
                    }

                    // Get the new n value for threshold comparison (use first int arg)
                    let new_n = arg_values
                        .iter()
                        .find_map(|v| {
                            if let Value::Int(n) = v {
                                Some(*n)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);

                    // Check base condition with new environment
                    let cond_result = eval_expr(condition, &new_env)?;

                    let result = if is_truthy(&cond_result) {
                        eval_expr(base_value, &new_env)?
                    } else {
                        // Recursively evaluate the STEP expression with new environment
                        // Pass the new n value for threshold comparison
                        eval_recurse_expr(
                            step,
                            step,
                            condition,
                            base_value,
                            &new_env,
                            memo,
                            parallel_threshold,
                            new_n,
                            cache,
                            param_names,
                        )?
                    };

                    // Cache the result if memoization is enabled
                    if memo && !cache_key.is_empty() {
                        cache.insert(cache_key, result.clone());
                    }

                    return Ok(result);
                }
            }

            // Regular function call - evaluate normally but recurse into args
            let arg_values: Result<Vec<Value>, String> = args
                .iter()
                .map(|a| {
                    eval_recurse_expr(
                        a,
                        step,
                        condition,
                        base_value,
                        env,
                        memo,
                        parallel_threshold,
                        current_n,
                        cache,
                        param_names,
                    )
                })
                .collect();
            let arg_values = arg_values?;

            if let Expr::Ident(name) = func.as_ref() {
                // Check for builtin functions
                if let Some(result) = eval_builtin(name, &arg_values)? {
                    return Ok(result);
                }

                // User-defined function
                if let Some(fd) = env.get_function(name).cloned() {
                    return eval_function_call(&fd, arg_values, env);
                }

                return Err(format!("Unknown function: {}", name));
            }

            Err("Cannot call non-function".to_string())
        }

        // For binary operations, recurse into both sides
        // If parallel threshold is exceeded, run left and right in separate threads
        Expr::Binary { op, left, right } => {
            // Only parallelize when current_n > parallel_threshold and both sides have self() calls
            if current_n > parallel_threshold
                && contains_self_call(left)
                && contains_self_call(right)
            {
                // Both sides have self() calls - parallelize them
                use std::thread;

                let step_clone = step.clone();
                let condition_clone = condition.clone();
                let base_value_clone = base_value.clone();
                let left_clone = left.as_ref().clone();
                let env_left = Environment {
                    configs: env.configs.clone(),
                    current_params: env.current_params.clone(),
                    functions: env.functions.clone(),
                    locals: env.locals.clone(),
                };
                let threshold = parallel_threshold;
                let n = current_n;

                let param_names_clone: Vec<String> = param_names.to_vec();
                let left_handle = thread::spawn(move || {
                    eval_recurse_expr(
                        &left_clone,
                        &step_clone,
                        &condition_clone,
                        &base_value_clone,
                        &env_left,
                        memo,
                        threshold,
                        n,
                        &mut HashMap::new(),
                        &param_names_clone,
                    )
                });

                // Evaluate right side in current thread
                let r = eval_recurse_expr(
                    right,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;

                // Wait for left side
                let l = left_handle
                    .join()
                    .map_err(|_| "Parallel recursion thread panicked".to_string())??;

                eval_binary_op(op, l, r)
            } else {
                // Sequential evaluation
                let l = eval_recurse_expr(
                    left,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;
                let r = eval_recurse_expr(
                    right,
                    step,
                    condition,
                    base_value,
                    env,
                    memo,
                    parallel_threshold,
                    current_n,
                    cache,
                    param_names,
                )?;
                eval_binary_op(op, l, r)
            }
        }

        // For other expressions, just evaluate normally
        _ => eval_expr(expr, env),
    }
}

/// Extract the primary parameter name from an expression
/// For `target`, returns Some("target")
/// For `current * 2`, returns Some("current") if "current" is in param_names
/// For complex expressions, tries to find the dominant parameter reference
fn extract_primary_param<'a>(expr: &Expr, param_names: &'a [String]) -> Option<&'a String> {
    match expr {
        Expr::Ident(name) => {
            // Direct identifier - check if it's a parameter
            param_names.iter().find(|p| *p == name)
        }
        Expr::Binary { left, right, .. } => {
            // Check left side first, then right
            extract_primary_param(left, param_names)
                .or_else(|| extract_primary_param(right, param_names))
        }
        Expr::Unary { operand, .. } => extract_primary_param(operand, param_names),
        Expr::Call { args, .. } => {
            // Check arguments
            for arg in args {
                if let Some(p) = extract_primary_param(arg, param_names) {
                    return Some(p);
                }
            }
            None
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => extract_primary_param(condition, param_names)
            .or_else(|| extract_primary_param(then_branch, param_names))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|e| extract_primary_param(e, param_names))
            }),
        _ => None,
    }
}

/// Check if an expression contains a self() call
fn contains_self_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "self" {
                    return true;
                }
            }
            args.iter().any(contains_self_call)
        }
        Expr::Binary { left, right, .. } => contains_self_call(left) || contains_self_call(right),
        Expr::Unary { operand, .. } => contains_self_call(operand),
        _ => false,
    }
}
