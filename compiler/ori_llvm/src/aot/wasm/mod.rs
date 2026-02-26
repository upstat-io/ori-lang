//! WebAssembly-specific configuration and code generation.
//!
//! This module provides WASM-specific functionality beyond basic target support:
//! - Memory configuration (import/export, initial/max size)
//! - JavaScript binding generation
//! - TypeScript declaration generation
//! - WASI support
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │   LLVM IR   │───▶│  WASM Emit  │───▶│  .wasm file │
//! │  (Module)   │    │  (wasm-ld)  │    │             │
//! └─────────────┘    └──────┬──────┘    └──────┬──────┘
//!                           │                  │
//!                    ┌──────▼──────┐    ┌──────▼──────┐
//!                    │  .js glue   │    │  .d.ts decl │
//!                    │  (optional) │    │  (optional) │
//!                    └─────────────┘    └─────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::wasm::{WasmConfig, WasmMemoryConfig, JsBindingGenerator};
//!
//! let config = WasmConfig::default()
//!     .with_memory(WasmMemoryConfig::default().with_initial_pages(16))
//!     .with_js_bindings(true);
//!
//! // Generate WASM with JS bindings
//! let js_gen = JsBindingGenerator::new("my_module", &exports);
//! js_gen.generate_js(Path::new("my_module.js"))?;
//! js_gen.generate_dts(Path::new("my_module.d.ts"))?;
//! ```

pub mod config;
pub mod optimize;
pub mod wasi;

pub use config::{WasmConfig, WasmFeatures, WasmMemoryConfig, WasmOutputOptions, WasmStackConfig};
pub use optimize::{WasmOptLevel, WasmOptRunner};
pub use wasi::{WasiConfig, WasiPreopen, WasiVersion};

use std::fmt::{self, Write as _};
use std::fs;
use std::path::Path;

/// Error type for WASM-specific operations.
#[derive(Debug, Clone)]
pub enum WasmError {
    /// Failed to generate JavaScript bindings.
    JsBindingGeneration { message: String },
    /// Failed to generate TypeScript declarations.
    DtsGeneration { message: String },
    /// Failed to write output file.
    WriteError { path: String, message: String },
    /// Invalid WASM configuration.
    InvalidConfig { message: String },
}

impl fmt::Display for WasmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsBindingGeneration { message } => {
                write!(f, "failed to generate JavaScript bindings: {message}")
            }
            Self::DtsGeneration { message } => {
                write!(f, "failed to generate TypeScript declarations: {message}")
            }
            Self::WriteError { path, message } => {
                write!(f, "failed to write '{path}': {message}")
            }
            Self::InvalidConfig { message } => {
                write!(f, "invalid WASM configuration: {message}")
            }
        }
    }
}

impl std::error::Error for WasmError {}

/// Information about an exported WASM function for binding generation.
#[derive(Debug, Clone)]
pub struct WasmExport {
    /// Ori function name (e.g., "add").
    pub ori_name: String,
    /// WASM export name (e.g., `_ori_add_ii`).
    pub wasm_name: String,
    /// Parameter types for documentation/TypeScript.
    pub params: Vec<WasmType>,
    /// Return type for documentation/TypeScript.
    pub return_type: WasmType,
    /// Whether this function is async (returns Promise in JS).
    pub is_async: bool,
}

/// WASM-level type representation for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmType {
    /// 32-bit integer (Ori: int on WASM32).
    I32,
    /// 64-bit integer (Ori: int on native, not typically used on WASM).
    I64,
    /// 32-bit float.
    F32,
    /// 64-bit float (Ori: float).
    F64,
    /// Pointer to string data (i32 offset + i32 length).
    String,
    /// Pointer to list data (i32 offset + i32 length).
    List(Box<WasmType>),
    /// Void (no return value).
    Void,
    /// Opaque pointer (for complex types).
    Pointer,
}

impl WasmType {
    /// Get the TypeScript type representation.
    #[must_use]
    pub fn typescript_type(&self) -> &'static str {
        match self {
            Self::I32 | Self::I64 | Self::F32 | Self::F64 | Self::Pointer => "number",
            Self::String => "string",
            Self::List(_) => "Array<any>", // Could be more specific
            Self::Void => "void",
        }
    }

    /// Get the JavaScript `JSDoc` type annotation.
    #[must_use]
    pub fn jsdoc_type(&self) -> &'static str {
        match self {
            Self::I32 | Self::I64 | Self::F32 | Self::F64 | Self::Pointer => "number",
            Self::String => "string",
            Self::List(_) => "Array",
            Self::Void => "void",
        }
    }
}

/// Generator for JavaScript bindings and TypeScript declarations.
pub struct JsBindingGenerator {
    /// Module name (used for naming).
    pub module_name: String,
    /// Exported functions.
    pub exports: Vec<WasmExport>,
}

impl JsBindingGenerator {
    /// Create a new binding generator.
    #[must_use]
    pub fn new(module_name: &str, exports: Vec<WasmExport>) -> Self {
        Self {
            module_name: module_name.to_string(),
            exports,
        }
    }

    /// Generate JavaScript glue code.
    ///
    /// The generated code handles:
    /// - Loading and instantiating the WASM module
    /// - String marshalling (TextEncoder/TextDecoder)
    /// - Memory management helpers
    /// - Clean wrapper functions for each export
    pub fn generate_js(&self, output: &Path) -> Result<(), WasmError> {
        let mut content = String::new();

        // Header
        let _ = write!(
            content,
            r"// Auto-generated JavaScript bindings for {module_name}.wasm
// Generated by Ori compiler

/**
 * @typedef {{{{
 *   memory: WebAssembly.Memory,
 *   instance: WebAssembly.Instance,
{export_types}
 * }}}} {module_name_pascal}Module
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

let instance = null;
let memory = null;

/**
 * Allocate memory in the WASM heap.
 * @param {{number}} size - Size in bytes
 * @returns {{number}} Pointer to allocated memory
 */
function alloc(size) {{
    return instance.exports.ori_alloc(size);
}}

/**
 * Free memory in the WASM heap.
 * @param {{number}} ptr - Pointer to free
 */
function free(ptr) {{
    instance.exports.ori_free(ptr);
}}

/**
 * Encode a string to WASM memory.
 * @param {{string}} str - String to encode
 * @returns {{{{ptr: number, len: number}}}} Pointer and length
 */
function encodeString(str) {{
    const bytes = encoder.encode(str);
    const ptr = alloc(bytes.length);
    const view = new Uint8Array(memory.buffer, ptr, bytes.length);
    view.set(bytes);
    return {{ ptr, len: bytes.length }};
}}

/**
 * Decode a string from WASM memory.
 * @param {{number}} ptr - Pointer to string data
 * @param {{number}} len - Length in bytes
 * @returns {{string}} Decoded string
 */
function decodeString(ptr, len) {{
    const bytes = new Uint8Array(memory.buffer, ptr, len);
    return decoder.decode(bytes);
}}

",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
            export_types = self.generate_jsdoc_export_types(),
        );

        // Generate wrapper functions
        for export in &self.exports {
            content.push_str(&Self::generate_js_wrapper(export));
            content.push('\n');
        }

        // Init function
        content.push_str(&self.generate_js_init());

        // Export
        let _ = write!(
            content,
            r"
export {{ init, {exports} }};
export default {{ init, {exports} }};
",
            exports = self
                .exports
                .iter()
                .map(|e| e.ori_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Write file
        fs::write(output, content).map_err(|e| WasmError::WriteError {
            path: output.to_string_lossy().into_owned(),
            message: e.to_string(),
        })
    }

    /// Generate TypeScript declaration file.
    pub fn generate_dts(&self, output: &Path) -> Result<(), WasmError> {
        let mut content = String::new();

        // Header
        let _ = write!(
            content,
            r"// Auto-generated TypeScript declarations for {module_name}.wasm
// Generated by Ori compiler

export interface {module_name_pascal}Module {{
    memory: WebAssembly.Memory;
    instance: WebAssembly.Instance;
}}

/**
 * Initialize the WASM module.
 * @param url - URL or path to the .wasm file
 * @param imports - Optional additional imports
 */
export function init(
    url?: string | URL | Response | BufferSource,
    imports?: WebAssembly.Imports
): Promise<{module_name_pascal}Module>;

",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
        );

        // Generate function declarations
        for export in &self.exports {
            content.push_str(&Self::generate_dts_function(export));
            content.push('\n');
        }

        // Write file
        fs::write(output, content).map_err(|e| WasmError::WriteError {
            path: output.to_string_lossy().into_owned(),
            message: e.to_string(),
        })
    }

    fn generate_jsdoc_export_types(&self) -> String {
        self.exports
            .iter()
            .map(|e| {
                let params = e
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("arg{}: {}", i, t.jsdoc_type()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    " *   {}: ({}) => {}",
                    e.ori_name,
                    params,
                    e.return_type.jsdoc_type()
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    fn generate_js_wrapper(export: &WasmExport) -> String {
        let mut code = String::new();

        // JSDoc
        code.push_str("/**\n");
        for (i, param) in export.params.iter().enumerate() {
            let _ = writeln!(code, " * @param {{{}}} arg{}", param.jsdoc_type(), i);
        }
        let _ = writeln!(code, " * @returns {{{}}}", export.return_type.jsdoc_type());
        code.push_str(" */\n");

        // Function signature
        let params = (0..export.params.len())
            .map(|i| format!("arg{i}"))
            .collect::<Vec<_>>()
            .join(", ");

        let _ = writeln!(code, "export function {}({}) {{", export.ori_name, params);

        // Check initialization
        code.push_str(
            "    if (!instance) throw new Error('Module not initialized. Call init() first.');\n",
        );

        // Handle string parameters
        let mut cleanup = Vec::new();
        let mut call_args = Vec::new();

        for (i, param) in export.params.iter().enumerate() {
            if *param == WasmType::String {
                let _ = writeln!(code, "    const _str{i} = encodeString(arg{i});");
                call_args.push(format!("_str{i}.ptr, _str{i}.len"));
                cleanup.push(format!("_str{i}.ptr"));
            } else {
                call_args.push(format!("arg{i}"));
            }
        }

        // Make the call
        let call = format!(
            "instance.exports.{}({})",
            export.wasm_name,
            call_args.join(", ")
        );

        if export.return_type == WasmType::Void {
            let _ = writeln!(code, "    {call};");
        } else if export.return_type == WasmType::String {
            let _ = writeln!(code, "    const _result = {call};");
            code.push_str("    // TODO: Decode string result from WASM memory\n");
            code.push_str("    return _result;\n");
        } else {
            let _ = writeln!(code, "    const _result = {call};");
            // Cleanup allocated strings
            for ptr in &cleanup {
                let _ = writeln!(code, "    free({ptr});");
            }
            code.push_str("    return _result;\n");
        }

        code.push_str("}\n");
        code
    }

    fn generate_js_init(&self) -> String {
        format!(
            r"
/**
 * Initialize the WASM module.
 * @param {{string | URL | Response | BufferSource}} [url='{module_name}.wasm'] - URL or source
 * @param {{WebAssembly.Imports}} [imports={{}}] - Additional imports
 * @returns {{Promise<{module_name_pascal}Module>}}
 */
export async function init(url = '{module_name}.wasm', imports = {{}}) {{
    let source;

    if (url instanceof Response) {{
        source = url;
    }} else if (url instanceof ArrayBuffer || ArrayBuffer.isView(url)) {{
        source = url;
    }} else {{
        source = fetch(url);
    }}

    const wasmImports = {{
        env: {{
            ...imports.env,
        }},
        ...imports,
    }};

    let result;
    if (source instanceof Response || source instanceof Promise) {{
        result = await WebAssembly.instantiateStreaming(source, wasmImports);
    }} else {{
        result = await WebAssembly.instantiate(source, wasmImports);
    }}

    instance = result.instance;
    memory = instance.exports.memory;

    return {{
        memory,
        instance,
{export_props}
    }};
}}
",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
            export_props = self
                .exports
                .iter()
                .map(|e| format!("        {}: {},", e.ori_name, e.ori_name))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    fn generate_dts_function(export: &WasmExport) -> String {
        let params = export
            .params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{}: {}", i, t.typescript_type()))
            .collect::<Vec<_>>()
            .join(", ");

        let ret_type = if export.is_async {
            format!("Promise<{}>", export.return_type.typescript_type())
        } else {
            export.return_type.typescript_type().to_string()
        };

        format!(
            "/**\n * {}\n */\nexport function {}({}): {};\n",
            export.ori_name, export.ori_name, params, ret_type
        )
    }
}

/// Convert a string to `PascalCase`.
fn pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

// Tests extracted to: compiler/oric/tests/phases/codegen/wasm.rs
