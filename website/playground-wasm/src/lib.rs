//! Ori Playground WASM Bindings
//!
//! Thin wrapper around `ori_compiler` that exposes the Ori compilation pipeline
//! to JavaScript via `wasm_bindgen`. All compilation logic lives in the portable
//! `ori_compiler` crate; this module only handles WASM-specific serialization
//! and entry points.

use ori_compiler::{compile_and_run, format_source, render_diagnostics, CompileConfig, ErrorPhase};
use ori_diagnostic::emitter::ColorMode;
use ori_eval::EvalMode;
use serde::Serialize;
use wasm_bindgen::prelude::*;

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

/// Result of formatting Ori code, serialized as JSON for JavaScript.
#[derive(Serialize)]
struct FormatResult {
    success: bool,
    formatted: Option<String>,
    error: Option<String>,
}

/// Initialize the WASM module (called once on load).
#[wasm_bindgen(start)]
pub fn init() {
    std::panic::set_hook(Box::new(console_error_panic_hook));
}

fn console_error_panic_hook(info: &std::panic::PanicHookInfo) {
    log(&info.to_string());
}

/// Run Ori source code and return the result as JSON.
#[wasm_bindgen]
pub fn run_ori(source: &str, _max_call_depth: Option<usize>) -> String {
    let result = run_ori_internal(source);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"success":false,"error":"Serialization error: {}","error_type":"internal"}}"#,
            e
        )
    })
}

/// Get the default maximum call depth for WASM.
#[wasm_bindgen]
pub fn default_max_call_depth() -> usize {
    EvalMode::Interpret.max_recursion_depth().unwrap_or(200)
}

fn run_ori_internal(source: &str) -> RunResult {
    let config = CompileConfig {
        file_path: "playground.ori".to_string(),
    };

    let output = compile_and_run(source, &config);

    if output.success {
        RunResult {
            success: true,
            output: output.output,
            printed: output.printed,
            error: None,
            error_type: None,
        }
    } else {
        let error_type = output.error_phase.map(|phase| match phase {
            ErrorPhase::Parse => "parse",
            ErrorPhase::Type => "type",
            ErrorPhase::Runtime => "runtime",
        });

        // Render diagnostics with source context for rich error messages
        let error_text = render_diagnostics(
            source,
            "playground.ori",
            &output.diagnostics,
            ColorMode::Never,
        );

        RunResult {
            success: false,
            output: String::new(),
            printed: output.printed,
            error: Some(error_text),
            error_type: error_type.map(String::from),
        }
    }
}

/// Format Ori source code and return the result as JSON.
#[wasm_bindgen]
pub fn format_ori(source: &str, max_width: Option<usize>) -> String {
    let result = format_ori_internal(source, max_width);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"success":false,"error":"Serialization error: {}"}}"#, e)
    })
}

fn format_ori_internal(source: &str, max_width: Option<usize>) -> FormatResult {
    let output = format_source(source, max_width);

    if output.success {
        FormatResult {
            success: true,
            formatted: output.formatted,
            error: None,
        }
    } else {
        let error_text = render_diagnostics(
            source,
            "playground.ori",
            &output.diagnostics,
            ColorMode::Never,
        );
        FormatResult {
            success: false,
            formatted: None,
            error: Some(error_text),
        }
    }
}

/// Get version information for the playground footer.
#[wasm_bindgen]
pub fn version() -> String {
    format!("Ori build {}", include_str!("../../../BUILD_NUMBER").trim())
}
