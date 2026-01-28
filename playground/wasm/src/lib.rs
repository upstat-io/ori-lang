//! Ori Playground WASM Bindings
//!
//! Exposes the Ori interpreter to JavaScript for browser-based execution.
//! Uses portable crates (ori_eval, ori_parse, etc.) without Salsa dependencies.

use wasm_bindgen::prelude::*;
use ori_ir::{SharedArena, SharedInterner};
use ori_eval::{
    buffer_handler, collect_extend_methods, collect_impl_methods, register_module_functions,
    register_newtype_constructors, register_variant_constructors, InterpreterBuilder,
    UserMethodRegistry, Value,
};
use ori_typeck::derives::process_derives;
use ori_typeck::type_check;
use ori_typeck::TypeRegistry;
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

    // Create a shared arena for all methods in this module
    let shared_arena = SharedArena::new(parse_result.arena.clone());

    // Build user method registry from impl and extend blocks
    let mut user_methods = UserMethodRegistry::new();
    let captures = interpreter.env().capture();
    collect_impl_methods(&parse_result.module, &shared_arena, &captures, &mut user_methods);
    collect_extend_methods(&parse_result.module, &shared_arena, &captures, &mut user_methods);

    // Process derived traits (Eq, Clone, Hashable, Printable, Default)
    let type_registry = TypeRegistry::new();
    process_derives(
        &parse_result.module,
        &type_registry,
        &mut user_methods,
        &interner,
    );

    // Merge the collected methods into the interpreter's registry
    interpreter.user_method_registry.write().merge(user_methods);

    // Register all functions from the module into the environment
    register_module_functions(&parse_result.module, &shared_arena, interpreter.env_mut());

    // Register variant constructors from sum type declarations
    register_variant_constructors(&parse_result.module, interpreter.env_mut());

    // Register newtype constructors from type declarations
    register_newtype_constructors(&parse_result.module, interpreter.env_mut());

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
        Value::Variant { variant_name, fields, .. } => {
            if fields.is_empty() {
                format!("{variant_name:?}")
            } else {
                let formatted: Vec<String> = fields.iter().map(format_value).collect();
                format!("{:?}({})", variant_name, formatted.join(", "))
            }
        }
        Value::VariantConstructor { variant_name, .. } => {
            format!("<variant constructor {:?}>", variant_name)
        }
        Value::Newtype { inner, .. } => format_value(inner),
        Value::NewtypeConstructor { type_name } => {
            format!("<newtype constructor {:?}>", type_name)
        }
    }
}

/// Get version information.
#[wasm_bindgen]
pub fn version() -> String {
    "Ori 0.1.0-alpha".to_string()
}
