//! Ori Playground WASM Bindings
//!
//! Exposes the Ori interpreter to JavaScript for browser-based execution.

use wasm_bindgen::prelude::*;
use sigilc::{CompilerDb, SourceFile};
use sigilc::query::{parsed, typed, evaluated};
use std::path::PathBuf;
use std::cell::RefCell;
use serde::Serialize;

// Import console.log from JavaScript
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Thread-local buffer to capture print output
thread_local! {
    static PRINT_BUFFER: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Clear the print buffer
fn clear_print_buffer() {
    PRINT_BUFFER.with(|buf| {
        buf.borrow_mut().clear();
    });
}

/// Get captured print output
fn get_print_output() -> String {
    PRINT_BUFFER.with(|buf| {
        buf.borrow().join("\n")
    })
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
    // Set up print hook - redirect Rust's stdout to our buffer
    std::panic::set_hook(Box::new(console_error_panic_hook));
}

fn console_error_panic_hook(info: &std::panic::PanicInfo) {
    log(&info.to_string());
}

/// Run Ori source code and return the result as JSON.
///
/// Returns a JSON object with:
/// - `success`: boolean indicating if execution succeeded
/// - `output`: the program output (if successful)
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
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from("playground.ori"), source.to_string());

    // Parse
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        let errors: Vec<String> = parse_result
            .errors
            .iter()
            .map(|e| format!("At {}: {}", e.span, e.message))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("parse".to_string()),
        };
    }

    // Type check
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        let errors: Vec<String> = type_result
            .errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect();
        return RunResult {
            success: false,
            output: String::new(),
            error: Some(errors.join("\n")),
            error_type: Some("type".to_string()),
        };
    }

    // Evaluate
    let eval_result = evaluated(&db, file);
    if eval_result.is_failure() {
        return RunResult {
            success: false,
            output: String::new(),
            error: Some(eval_result.error.unwrap_or_else(|| "Unknown error".to_string())),
            error_type: Some("runtime".to_string()),
        };
    }

    // Format output
    let output = if let Some(result) = eval_result.result {
        use sigilc::EvalOutput;
        match result {
            EvalOutput::Void => String::new(),
            _ => result.display(),
        }
    } else {
        String::new()
    };

    RunResult {
        success: true,
        output,
        error: None,
        error_type: None,
    }
}

/// Get version information.
#[wasm_bindgen]
pub fn version() -> String {
    "Ori 0.1.0-alpha".to_string()
}
