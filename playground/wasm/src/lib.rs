//! Ori Playground WASM Bindings
//!
//! Exposes the Ori interpreter to JavaScript for browser-based execution.
//! Uses portable crates (ori_eval, ori_parse, etc.) without Salsa dependencies.

use wasm_bindgen::prelude::*;
use ori_ir::{SharedArena, SharedInterner};
use ori_eval::{Environment, FunctionValue, InterpreterBuilder, Value, buffer_handler};
use ori_typeck::type_check;
use serde::Serialize;

// Import console.log from JavaScript
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Result of running Ori code, serialized as JSON for JavaScript.
#[derive(Serialize)]
pub struct RunResult {
    pub success: bool,
    pub output: String,
    pub printed: String,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

/// Initialize the WASM module (called once on load).
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook to log to console
    std::panic::set_hook(Box::new(console_error_panic_hook));
}

fn console_error_panic_hook(info: &std::panic::PanicHookInfo) {
    log(&info.to_string());
}

/// Run Ori source code and return the result as JSON.
///
/// Returns a JSON object with:
/// - `success`: boolean indicating if execution succeeded
/// - `output`: the program output (if successful)
/// - `printed`: any output from print/println calls
/// - `error`: error message (if failed)
/// - `error_type`: "parse", "type", or "runtime" (if failed)
#[wasm_bindgen]
pub fn run_ori(source: &str) -> String {
    let result = run_ori_internal(source);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"success":false,"error":"Serialization error: {}","error_type":"internal"}}"#, e)
    })
}

fn run_ori_internal(source: &str) -> RunResult {
    // Create a shared interner for the session
    let interner = SharedInterner::default();

    // Lex the source
    let tokens = ori_lexer::lex(source, &interner);

    // Parse
    let parse_result = ori_parse::parse(&tokens, &interner);
    if parse_result.has_errors() {
        let errors: Vec<String> = parse_result
            .errors
            .iter()
            .map(|e| format!("At {}: {}", e.span, e.message))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("parse".to_string()),
        };
    }

    // Type check
    let typed_module = type_check(&parse_result, &interner);
    if typed_module.error_guarantee.is_some() {
        let errors: Vec<String> = typed_module
            .errors
            .iter()
            .map(|e| format!("At {}: {}", e.span, e.message))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("type".to_string()),
        };
    }

    // Create interpreter with the parse result's arena and buffer print handler
    let print_handler = buffer_handler();
    let mut interpreter = InterpreterBuilder::new(&interner, &parse_result.arena)
        .print_handler(print_handler.clone())
        .build();

    // Register built-in function_val functions (int, str, float, byte)
    interpreter.register_prelude();

    // Register all functions from the module into the environment
    register_module_functions(&parse_result, interpreter.env_mut());

    // Find @main function and evaluate it
    let main_name = interner.intern("main");
    let main_func = parse_result.module.functions
        .iter()
        .find(|f| f.name == main_name);

    let Some(main_func) = main_func else {
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some("No @main function found".to_string()),
            error_type: Some("runtime".to_string()),
        };
    };

    // Evaluate the main function's body
    match interpreter.eval(main_func.body) {
        Ok(value) => {
            let output = format_value(&value);
            let printed = interpreter.get_print_output();
            RunResult {
                success: true,
                output,
                printed,
                error: None,
                error_type: None,
            }
        }
        Err(e) => {
            // Still capture any print output that occurred before the error
            let printed = interpreter.get_print_output();
            RunResult {
                success: false,
                output: String::new(),
                printed,
                error: Some(e.message),
                error_type: Some("runtime".to_string()),
            }
        }
    }
}

/// Register all functions from a module into the environment.
///
/// This is a simplified version of the register_module_functions from oric,
/// adapted for standalone WASM usage without Salsa dependencies.
fn register_module_functions(
    parse_result: &ori_parse::ParseResult,
    env: &mut Environment,
) {
    // Create a shared arena for all functions in this module
    let shared_arena = SharedArena::new(parse_result.arena.clone());

    for func in &parse_result.module.functions {
        let params: Vec<_> = parse_result.arena.get_params(func.params)
            .iter()
            .map(|p| p.name)
            .collect();
        let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();
        let captures = env.capture();

        let func_value = FunctionValue::with_capabilities(
            params,
            func.body,
            captures,
            shared_arena.clone(),
            capabilities,
        );
        env.define(func.name, Value::Function(func_value), false);
    }
}

/// Format a Value for output display.
fn format_value(value: &Value) -> String {
    match value {
        Value::Void => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => s.to_string(),
        Value::Char(c) => c.to_string(),
        Value::Byte(b) => format!("0x{:02x}", b),
        Value::None => "None".to_string(),
        Value::Some(v) => format!("Some({})", format_value(v)),
        Value::Ok(v) => format!("Ok({})", format_value(v)),
        Value::Err(v) => format!("Err({})", format_value(v)),
        Value::List(items) => {
            let formatted: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", formatted.join(", "))
        }
        Value::Tuple(items) => {
            let formatted: Vec<String> = items.iter().map(format_value).collect();
            format!("({})", formatted.join(", "))
        }
        Value::Map(map) => {
            let entries: Vec<String> = map.iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        Value::Struct(s) => {
            // For struct, we'd need the interner to display field names properly
            // For now, just show the type name
            format!("<struct {:?}>", s.type_name)
        }
        Value::Range(r) => format!("{}..{}", r.start, r.end),
        Value::Function(_) | Value::MemoizedFunction(_) => "<function>".to_string(),
        Value::FunctionVal(_, name) => format!("<builtin {}>", name),
        Value::Duration(ms) => format!("{ms}ms"),
        Value::Size(s) => format!("{}b", s),
        Value::Error(e) => format!("Error({})", e),
    }
}

/// Get version information.
#[wasm_bindgen]
pub fn version() -> String {
    "Ori 0.1.0-alpha".to_string()
}
