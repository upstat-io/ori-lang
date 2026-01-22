//! Run command - parse and evaluate a Sigil file

use std::fs;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind, ExprArena};
use sigilc_v2::eval::{Evaluator, Value, FunctionValue, Environment};
use sigilc_v2::errors::Diagnostic;

/// Result of running a file
pub struct RunResult {
    pub value: Option<Value>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Run a Sigil source file
pub fn run_file(path: &str) -> Result<RunResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading file '{}': {}", path, e))?;

    run_source(&source, path)
}

/// Register all module functions in the environment.
pub fn register_functions(
    env: &mut Environment,
    items: &[sigilc_v2::syntax::Item],
    arena: &ExprArena,
) {
    for item in items {
        if let ItemKind::Function(func) = &item.kind {
            // Extract parameter names
            let params: Vec<_> = arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();

            // Create function value
            let func_val = Value::Function(FunctionValue {
                params,
                body: func.body,
                captures: Rc::new(RefCell::new(HashMap::new())),
            });

            // Register in environment (not mutable)
            env.define(func.name, func_val, false);
        }
    }
}

/// Register built-in functions in the environment.
pub fn register_builtins(env: &mut Environment, interner: &StringInterner) {
    // print
    let print_name = interner.intern("print");
    env.define(print_name, Value::Builtin(builtin_print, "print"), false);

    // len
    let len_name = interner.intern("len");
    env.define(len_name, Value::Builtin(builtin_len, "len"), false);

    // str
    let str_name = interner.intern("str");
    env.define(str_name, Value::Builtin(builtin_str, "str"), false);

    // int
    let int_name = interner.intern("int");
    env.define(int_name, Value::Builtin(builtin_int, "int"), false);

    // float
    let float_name = interner.intern("float");
    env.define(float_name, Value::Builtin(builtin_float, "float"), false);

    // assert
    let assert_name = interner.intern("assert");
    env.define(assert_name, Value::Builtin(builtin_assert, "assert"), false);

    // assert_eq
    let assert_eq_name = interner.intern("assert_eq");
    env.define(assert_eq_name, Value::Builtin(builtin_assert_eq, "assert_eq"), false);

    // panic
    let panic_name = interner.intern("panic");
    env.define(panic_name, Value::Builtin(builtin_panic, "panic"), false);
}

fn builtin_print(args: &[Value]) -> Result<Value, String> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 { print!(" "); }
        print!("{}", arg.display_value());
    }
    println!();
    Ok(Value::Void)
}

fn builtin_len(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("len() takes exactly 1 argument".to_string());
    }
    match &args[0] {
        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
        Value::List(l) => Ok(Value::Int(l.len() as i64)),
        Value::Map(m) => Ok(Value::Int(m.len() as i64)),
        _ => Err(format!("len() not supported for {}", args[0].type_name())),
    }
}

fn builtin_str(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("str() takes exactly 1 argument".to_string());
    }
    Ok(Value::Str(Rc::new(args[0].display_value())))
}

fn builtin_int(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("int() takes exactly 1 argument".to_string());
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Str(s) => s.parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("cannot convert '{}' to int", s)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        _ => Err(format!("int() not supported for {}", args[0].type_name())),
    }
}

fn builtin_float(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("float() takes exactly 1 argument".to_string());
    }
    match &args[0] {
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Str(s) => s.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| format!("cannot convert '{}' to float", s)),
        _ => Err(format!("float() not supported for {}", args[0].type_name())),
    }
}

fn builtin_assert(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("assert() takes 1 or 2 arguments".to_string());
    }
    if !args[0].is_truthy() {
        let msg = args.get(1)
            .map(|v| v.display_value())
            .unwrap_or_else(|| "assertion failed".to_string());
        return Err(msg);
    }
    Ok(Value::Void)
}

fn builtin_assert_eq(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("assert_eq() takes 2 or 3 arguments".to_string());
    }
    if !args[0].equals(&args[1]) {
        let msg = args.get(2)
            .map(|v| v.display_value())
            .unwrap_or_else(|| format!(
                "assertion failed: {} != {}",
                args[0].display_value(),
                args[1].display_value()
            ));
        return Err(msg);
    }
    Ok(Value::Void)
}

fn builtin_panic(args: &[Value]) -> Result<Value, String> {
    let msg = args.first()
        .map(|v| v.display_value())
        .unwrap_or_else(|| "panic".to_string());
    Err(msg)
}

/// Run Sigil source code
pub fn run_source(source: &str, _filename: &str) -> Result<RunResult, String> {
    let interner = StringInterner::new();

    // Step 1: Lex
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();

    // Step 2: Parse
    let parser = Parser::new(&tokens, &interner);
    let parse_result = parser.parse_module();

    // Check for parse errors
    if !parse_result.diagnostics.is_empty() {
        return Ok(RunResult {
            value: None,
            diagnostics: parse_result.diagnostics,
        });
    }

    // Step 3: Set up environment with functions and builtins
    let mut env = Environment::new();
    register_builtins(&mut env, &interner);
    register_functions(&mut env, &parse_result.items, &parse_result.arena);

    // Step 4: Find and evaluate @main function
    let mut evaluator = Evaluator::with_env(&interner, &parse_result.arena, env);

    // Look for @main function
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            let name = interner.lookup(func.name);
            if name == "main" {
                // Evaluate the function body
                match evaluator.eval(func.body) {
                    Ok(value) => {
                        return Ok(RunResult {
                            value: Some(value),
                            diagnostics: vec![],
                        });
                    }
                    Err(e) => {
                        return Err(format!("Runtime error: {}", e.message));
                    }
                }
            }
        }
    }

    // No @main found - just return success with no value
    Ok(RunResult {
        value: None,
        diagnostics: vec![],
    })
}

/// Run file and print results
pub fn run_file_and_print(path: &str) {
    match run_file(path) {
        Ok(result) => {
            // Print any diagnostics
            for diag in &result.diagnostics {
                eprintln!("{:?}", diag);
            }

            if result.diagnostics.iter().any(|d| d.is_error()) {
                std::process::exit(1);
            }

            // Print result value if present
            if let Some(value) = result.value {
                if !matches!(value, Value::Void) {
                    println!("{:?}", value);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
