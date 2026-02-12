//! Ori Playground WASM Bindings
//!
//! Exposes the Ori interpreter to JavaScript for browser-based execution.
//! Uses portable crates (ori_eval, ori_parse, etc.) without Salsa dependencies.

use wasm_bindgen::prelude::*;
use ori_ir::SharedInterner;
use ori_eval::{
    buffer_handler, collect_extend_methods_with_config, collect_impl_methods_with_config,
    process_derives, register_module_functions, register_newtype_constructors,
    register_variant_constructors, EvalMode, InterpreterBuilder, MethodCollectionConfig,
    UserMethodRegistry, Value,
};
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
/// Parameters:
/// - `source`: The Ori source code to run
/// - `max_call_depth`: Optional maximum recursion depth (defaults to 200)
///
/// Returns a JSON object with:
/// - `success`: boolean indicating if execution succeeded
/// - `output`: the program output (if successful)
/// - `printed`: any output from print/println calls
/// - `error`: error message (if failed)
/// - `error_type`: "parse", "type", or "runtime" (if failed)
#[wasm_bindgen]
pub fn run_ori(source: &str, max_call_depth: Option<usize>) -> String {
    let result = run_ori_internal(source, max_call_depth);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"success":false,"error":"Serialization error: {}","error_type":"internal"}}"#, e)
    })
}

/// Get the default maximum call depth for WASM.
#[wasm_bindgen]
pub fn default_max_call_depth() -> usize {
    // EvalMode::Interpret returns Some(200) on WASM
    EvalMode::Interpret.max_recursion_depth().unwrap_or(200)
}

fn run_ori_internal(source: &str, max_call_depth: Option<usize>) -> RunResult {
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
            .map(|e| format!("At {}: {}", e.span(), e.message()))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("parse".to_string()),
        };
    }

    // Type check (V2 pipeline)
    let (type_result, pool) = ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        &interner,
        |_checker| {},
    );
    if type_result.has_errors() {
        let errors: Vec<String> = type_result
            .errors()
            .iter()
            .map(|e| format!("At {}: {}", e.span, e.format_with(&pool, &interner)))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("type".to_string()),
        };
    }

    // Canonicalize: AST + types → self-contained canonical IR
    let canon_result = ori_canon::lower_module(
        &parse_result.module,
        &parse_result.arena,
        &type_result,
        &pool,
        &interner,
    );
    let shared_canon = ori_ir::canon::SharedCanonResult::new(canon_result);

    // Create interpreter with the parse result's arena and buffer print handler.
    // EvalMode::Interpret on WASM enforces a 200-depth recursion limit.
    let _ = max_call_depth; // Reserved for future per-session depth override
    let print_handler = buffer_handler();
    let mut interpreter = InterpreterBuilder::new(&interner, &parse_result.arena)
        .print_handler(print_handler.clone())
        .canon(shared_canon.clone())
        .build();

    // Register built-in function_val functions (int, str, float, byte)
    interpreter.register_prelude();

    // Clone the shared arena (O(1) Arc::clone) for methods in this module
    let shared_arena = parse_result.arena.clone();

    // Build user method registry from impl and extend blocks
    let mut user_methods = UserMethodRegistry::new();
    let config = MethodCollectionConfig {
        module: &parse_result.module,
        arena: &shared_arena,
        captures: std::sync::Arc::new(interpreter.env().capture()),
        canon: Some(&shared_canon),
    };
    collect_impl_methods_with_config(&config, &mut user_methods);
    collect_extend_methods_with_config(&config, &mut user_methods);

    // Process derived traits (Eq, Clone, Hashable, Printable, Default)
    process_derives(
        &parse_result.module,
        &mut user_methods,
        &interner,
    );

    // Merge the collected methods into the interpreter's registry
    interpreter.user_method_registry().write().merge(user_methods);

    // Register all functions from the module into the environment
    register_module_functions(&parse_result.module, &shared_arena, interpreter.env_mut(), Some(&shared_canon));

    // Register variant constructors from sum type declarations
    register_variant_constructors(&parse_result.module, interpreter.env_mut());

    // Register newtype constructors from type declarations
    register_newtype_constructors(&parse_result.module, interpreter.env_mut());

    // Find @main function's canonical root and evaluate it
    let main_name = interner.intern("main");
    let Some(can_id) = shared_canon.root_for(main_name) else {
        return RunResult {
            success: false,
            output: String::new(),
            printed: String::new(),
            error: Some("No @main function found".to_string()),
            error_type: Some("runtime".to_string()),
        };
    };

    // Evaluate main via canonical path
    match interpreter.eval_can(can_id) {
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
                error: Some(e.into_eval_error().message),
                error_type: Some("runtime".to_string()),
            }
        }
    }
}

/// Format a Value for output display.
fn format_value(value: &Value) -> String {
    // Special case: Void produces no output
    if matches!(value, Value::Void) {
        return String::new();
    }
    value.display_value()
}

/// Get version information for the playground footer.
#[wasm_bindgen]
pub fn version() -> String {
    format!("Ori build {}", include_str!("../../../BUILD_NUMBER").trim())
}

// TODO: Switch to LSP-based formatting once ori_lsp is implemented.
// This direct integration is a temporary solution for the playground.

/// Format Ori source code and return the result as JSON.
///
/// Parameters:
/// - `source`: The Ori source code to format
/// - `max_width`: Optional maximum line width (defaults to 100)
///
/// Returns a JSON object with:
/// - `success`: boolean indicating if formatting succeeded
/// - `formatted`: the formatted code (if successful)
/// - `error`: error message (if failed)
#[wasm_bindgen]
pub fn format_ori(source: &str, max_width: Option<usize>) -> String {
    let result = format_ori_internal(source, max_width);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"success":false,"error":"Serialization error: {}"}}"#, e)
    })
}

#[derive(Serialize)]
struct FormatResult {
    success: bool,
    formatted: Option<String>,
    error: Option<String>,
}

fn format_ori_internal(source: &str, max_width: Option<usize>) -> FormatResult {
    let interner = SharedInterner::default();

    // Lex with comments for comment-preserving formatting
    let lex_output = ori_lexer::lex_with_comments(source, &interner);

    // Parse
    let parse_result = ori_parse::parse(&lex_output.tokens, &interner);
    if parse_result.has_errors() {
        let errors: Vec<String> = parse_result
            .errors
            .iter()
            .map(|e| e.message().to_string())
            .collect();
        return FormatResult {
            success: false,
            formatted: None,
            error: Some(errors.join("\n")),
        };
    }

    // Build config
    let config = match max_width {
        Some(width) => ori_fmt::FormatConfig::with_max_width(width),
        None => ori_fmt::FormatConfig::default(),
    };

    // Format with comment preservation and config
    let formatted = ori_fmt::format_module_with_comments_and_config(
        &parse_result.module,
        &lex_output.comments,
        &parse_result.arena,
        &*interner,
        config,
    );

    FormatResult {
        success: true,
        formatted: Some(formatted),
        error: None,
    }
}
