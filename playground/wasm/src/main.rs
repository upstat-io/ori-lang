//! Ori Playground WASI Binary
//!
//! Reads Ori source from stdin, executes it, prints result to stdout.
//! Designed to run in browser via @wasmer/wasi.

use ori_ir::StringInterner;
use ori_lexer::lex;
use ori_parse::parse;
use ori_typeck::type_check;
use ori_eval::{Environment, Value};
use ori_patterns::PatternRegistry;
use std::io::{self, Read};
use std::sync::Arc;

fn main() {
    // Read source code from stdin
    let mut source = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut source) {
        eprintln!("Error reading input: {e}");
        std::process::exit(1);
    }

    // Run the code
    match run_ori(&source) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_ori(source: &str) -> Result<String, String> {
    let interner = Arc::new(StringInterner::new());

    // Lex
    let tokens = lex(source, &interner);

    // Parse
    let parse_result = parse(&tokens, &interner);
    if parse_result.has_errors() {
        let errors: Vec<String> = parse_result
            .errors
            .iter()
            .map(|e| format!("Parse error at {}: {}", e.span, e.message))
            .collect();
        return Err(errors.join("\n"));
    }

    // Type check
    let type_result = type_check(&parse_result, &interner);
    if type_result.has_errors() {
        let errors: Vec<String> = type_result
            .errors
            .iter()
            .map(|e| format!("Type error: {:?}", e))
            .collect();
        return Err(errors.join("\n"));
    }

    // Evaluate
    let registry = PatternRegistry::new();
    let mut env = Environment::new();

    // Register functions
    for func in &parse_result.module.functions {
        let func_value = Value::Function(ori_eval::FunctionValue {
            params: parse_result.arena.get_params(func.params).to_vec(),
            body: func.body,
            env: env.capture(),
        });
        env.define(func.name, func_value);
    }

    // Look for main
    let main_name = interner.intern("main");
    let main_func = parse_result.module.functions.iter().find(|f| f.name == main_name);

    if let Some(func) = main_func {
        // Simple evaluation - just evaluate the body
        match eval_expr(&parse_result.arena, func.body, &mut env, &registry, &interner) {
            Ok(value) => {
                match value {
                    Value::Void => Ok(String::new()),
                    _ => Ok(format!("→ {}", value.display_value())),
                }
            }
            Err(e) => Err(format!("Runtime error: {e}")),
        }
    } else {
        Ok(String::new())
    }
}

fn eval_expr(
    arena: &ori_ir::ExprArena,
    expr_id: ori_ir::ExprId,
    env: &mut Environment,
    registry: &PatternRegistry,
    interner: &Arc<StringInterner>,
) -> Result<Value, String> {
    use ori_ir::ExprKind;
    use ori_eval::*;

    let expr = arena.get(expr_id);
    match &expr.kind {
        ExprKind::Int(n) => Ok(Value::Int(*n)),
        ExprKind::Float(f) => Ok(Value::Float(*f)),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Str(s) => Ok(Value::Str(s.clone())),
        ExprKind::Void => Ok(Value::Void),
        _ => Err("Expression type not yet supported in playground".to_string()),
    }
}
