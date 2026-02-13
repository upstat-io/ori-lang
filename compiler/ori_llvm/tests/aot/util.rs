//! Test Utilities for AOT Integration Tests
//!
//! Provides shared helpers for:
//! - Compiling and running Ori programs through the AOT pipeline
//! - Creating test fixtures (WASM modules, object files)
//! - Binary format verification
//! - Target configuration helpers
//! - Command execution utilities

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

use ori_llvm::aot::{
    TargetConfig, TargetTripleComponents, WasmConfig, WasmMemoryConfig, WasmStackConfig,
};

/// Create a Linux `x86_64` target for testing.
#[must_use]
pub fn linux_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    TargetConfig::from_components(components)
}

/// Create a macOS `x86_64` target for testing.
#[must_use]
pub fn macos_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    TargetConfig::from_components(components)
}

/// Create a macOS ARM64 target for testing.
#[must_use]
pub fn macos_arm_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    TargetConfig::from_components(components)
}

/// Create a Windows MSVC target for testing.
#[must_use]
pub fn windows_msvc_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    TargetConfig::from_components(components)
}

/// Create a Windows GNU target for testing.
#[must_use]
pub fn windows_gnu_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    TargetConfig::from_components(components)
}

/// Create a WASM32 standalone target for testing.
#[must_use]
pub fn wasm32_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    TargetConfig::from_components(components)
}

/// Create a WASM32 WASI target for testing.
#[must_use]
pub fn wasm32_wasi_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("wasm32-unknown-wasi").unwrap();
    TargetConfig::from_components(components)
}

/// Create a default WASM configuration for testing.
#[must_use]
pub fn default_wasm_config() -> WasmConfig {
    WasmConfig::default()
}

/// Create a browser-oriented WASM configuration for testing.
#[must_use]
pub fn browser_wasm_config() -> WasmConfig {
    WasmConfig::browser()
}

/// Create a WASI configuration for testing.
#[must_use]
pub fn wasi_config() -> WasmConfig {
    WasmConfig::wasi()
}

/// Create a minimal WASM configuration for testing.
#[must_use]
pub fn minimal_wasm_config() -> WasmConfig {
    WasmConfig::default()
        .with_memory(
            WasmMemoryConfig::default()
                .with_initial_pages(1)
                .with_max_pages(Some(1)),
        )
        .with_stack(WasmStackConfig::default().with_size_kb(64))
}

/// WASM binary verification result.
#[derive(Debug, Default)]
pub struct WasmVerification {
    /// Exported functions.
    pub exports: Vec<WasmExportInfo>,
    /// Imported functions.
    pub imports: Vec<WasmImportInfo>,
    /// Memory configuration.
    pub memories: Vec<WasmMemoryInfo>,
    /// Custom sections.
    pub custom_sections: Vec<String>,
    /// Enabled features.
    pub features: WasmFeatures,
    /// Total code size in bytes.
    pub code_size: usize,
}

#[derive(Debug, Clone)]
pub struct WasmExportInfo {
    pub name: String,
    pub kind: WasmExportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmExportKind {
    Function,
    Memory,
    Table,
    Global,
}

#[derive(Debug, Clone)]
pub struct WasmImportInfo {
    pub module: String,
    pub name: String,
    pub kind: WasmImportKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmImportKind {
    Function,
    Memory,
    Table,
    Global,
}

#[derive(Debug, Clone)]
pub struct WasmMemoryInfo {
    pub initial_pages: u64,
    pub max_pages: Option<u64>,
    pub shared: bool,
}

// These boolean fields represent distinct WASM feature flags that are independent
// and cannot be meaningfully combined into a state machine or two-variant enums.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent WASM feature flags; each is a genuine on/off toggle"
)]
#[derive(Debug, Default, Clone)]
pub struct WasmFeatures {
    pub bulk_memory: bool,
    pub simd: bool,
    pub reference_types: bool,
    pub multi_value: bool,
    pub exception_handling: bool,
    pub threads: bool,
}

/// Parse a WASM binary and extract verification information.
///
/// # Errors
///
/// Returns an error if the WASM binary is invalid.
pub fn parse_wasm(bytes: &[u8]) -> Result<WasmVerification, String> {
    use wasmparser::{Parser, Payload};

    let mut result = WasmVerification::default();
    let parser = Parser::new(0);

    for payload in parser.parse_all(bytes) {
        let payload = payload.map_err(|e| format!("WASM parse error: {e}"))?;

        match payload {
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|e| format!("Export parse error: {e}"))?;
                    result.exports.push(WasmExportInfo {
                        name: export.name.to_string(),
                        kind: match export.kind {
                            wasmparser::ExternalKind::Func => WasmExportKind::Function,
                            wasmparser::ExternalKind::Memory => WasmExportKind::Memory,
                            wasmparser::ExternalKind::Table => WasmExportKind::Table,
                            wasmparser::ExternalKind::Global => WasmExportKind::Global,
                            wasmparser::ExternalKind::Tag => continue,
                        },
                    });
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader {
                    let import = import.map_err(|e| format!("Import parse error: {e}"))?;
                    result.imports.push(WasmImportInfo {
                        module: import.module.to_string(),
                        name: import.name.to_string(),
                        kind: match import.ty {
                            wasmparser::TypeRef::Func(_) => WasmImportKind::Function,
                            wasmparser::TypeRef::Memory(_) => WasmImportKind::Memory,
                            wasmparser::TypeRef::Table(_) => WasmImportKind::Table,
                            wasmparser::TypeRef::Global(_) => WasmImportKind::Global,
                            wasmparser::TypeRef::Tag(_) => continue,
                        },
                    });
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|e| format!("Memory parse error: {e}"))?;
                    result.memories.push(WasmMemoryInfo {
                        initial_pages: memory.initial,
                        max_pages: memory.maximum,
                        shared: memory.shared,
                    });
                }
            }
            Payload::CustomSection(section) => {
                result.custom_sections.push(section.name().to_string());
            }
            Payload::CodeSectionStart { size, .. } => {
                result.code_size = size as usize;
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Verify that a WASM binary exports a specific function.
pub fn wasm_has_export(verification: &WasmVerification, name: &str) -> bool {
    verification.exports.iter().any(|e| e.name == name)
}

/// Verify that a WASM binary exports a function with a specific kind.
pub fn wasm_has_export_of_kind(
    verification: &WasmVerification,
    name: &str,
    kind: WasmExportKind,
) -> bool {
    verification
        .exports
        .iter()
        .any(|e| e.name == name && e.kind == kind)
}

/// Verify that a WASM binary imports from a specific module.
pub fn wasm_has_import_from(verification: &WasmVerification, module: &str) -> bool {
    verification.imports.iter().any(|i| i.module == module)
}

/// Verify that a WASM binary has a custom section.
pub fn wasm_has_custom_section(verification: &WasmVerification, name: &str) -> bool {
    verification.custom_sections.iter().any(|s| s == name)
}

/// Object file verification result.
#[derive(Debug, Default)]
pub struct ObjectVerification {
    /// Object file format.
    pub format: ObjectFormat,
    /// Architecture.
    pub architecture: String,
    /// Symbols (name, kind).
    pub symbols: Vec<(String, SymbolKind)>,
    /// Sections.
    pub sections: Vec<String>,
    /// Whether the object contains debug info.
    pub has_debug_info: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum ObjectFormat {
    #[default]
    Unknown,
    Elf,
    MachO,
    Coff,
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Text,
    Data,
    Bss,
    Unknown,
}

/// Parse an object file and extract verification information.
///
/// # Errors
///
/// Returns an error if the object file is invalid.
pub fn parse_object(bytes: &[u8]) -> Result<ObjectVerification, String> {
    use object::{Object, ObjectSection, ObjectSymbol};

    let obj = object::File::parse(bytes).map_err(|e| format!("Object parse error: {e}"))?;

    let format = match obj.format() {
        object::BinaryFormat::Elf => ObjectFormat::Elf,
        object::BinaryFormat::MachO => ObjectFormat::MachO,
        object::BinaryFormat::Coff | object::BinaryFormat::Pe => ObjectFormat::Coff,
        object::BinaryFormat::Wasm => ObjectFormat::Wasm,
        _ => ObjectFormat::Unknown,
    };

    let architecture = format!("{:?}", obj.architecture());

    let symbols: Vec<_> = obj
        .symbols()
        .filter_map(|sym| {
            let name = sym.name().ok()?.to_string();
            let kind = match sym.section() {
                object::SymbolSection::Section(idx) => {
                    if let Ok(section) = obj.section_by_index(idx) {
                        if section.name().ok()?.contains("text") {
                            SymbolKind::Text
                        } else if section.name().ok()?.contains("data") {
                            SymbolKind::Data
                        } else if section.name().ok()?.contains("bss") {
                            SymbolKind::Bss
                        } else {
                            SymbolKind::Unknown
                        }
                    } else {
                        SymbolKind::Unknown
                    }
                }
                _ => SymbolKind::Unknown,
            };
            Some((name, kind))
        })
        .collect();

    let sections: Vec<_> = obj
        .sections()
        .filter_map(|s| s.name().ok().map(ToString::to_string))
        .collect();

    let has_debug_info = sections
        .iter()
        .any(|s| s.contains("debug") || s.starts_with(".debug") || s.starts_with("__debug"));

    Ok(ObjectVerification {
        format,
        architecture,
        symbols,
        sections,
        has_debug_info,
    })
}

/// Check if an object file contains a symbol with the given name.
pub fn object_has_symbol(verification: &ObjectVerification, name: &str) -> bool {
    verification.symbols.iter().any(|(n, _)| n.contains(name))
}

/// Check if an object file contains a section with the given name.
pub fn object_has_section(verification: &ObjectVerification, name: &str) -> bool {
    verification.sections.iter().any(|s| s.contains(name))
}

/// Extract arguments from a Command for verification.
pub fn command_args(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect()
}

/// Check if command contains an argument.
pub fn command_has_arg(cmd: &Command, arg: &str) -> bool {
    command_args(cmd).iter().any(|a| a.contains(arg))
}

/// Check if command has an argument at a specific position relative to another.
pub fn command_has_arg_before(cmd: &Command, arg: &str, before: &str) -> bool {
    let args = command_args(cmd);
    let arg_pos = args.iter().position(|a| a.contains(arg));
    let before_pos = args.iter().position(|a| a.contains(before));

    match (arg_pos, before_pos) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// Check if a tool is available on the system.
pub fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if wasm-ld is available.
pub fn wasm_ld_available() -> bool {
    tool_available("wasm-ld")
}

/// Check if clang is available.
pub fn clang_available() -> bool {
    tool_available("clang")
}

/// Check if llvm-objdump is available.
pub fn llvm_objdump_available() -> bool {
    tool_available("llvm-objdump")
}

/// Check if wasm-opt is available.
pub fn wasm_opt_available() -> bool {
    tool_available("wasm-opt")
}

// AOT compile-and-run helpers

/// Get the path to the `ori` binary.
pub fn ori_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("compiler").exists())
        .map_or_else(|| PathBuf::from("/workspace"), Path::to_path_buf);

    let release_path = workspace_root.join("target/release/ori");
    if release_path.exists() {
        return release_path;
    }

    let debug_path = workspace_root.join("target/debug/ori");
    if debug_path.exists() {
        return debug_path;
    }

    PathBuf::from("ori")
}

/// Compile and run an Ori program, returning the exit code.
///
/// Returns 0 on success, non-zero on failure, -1 if compilation fails.
pub fn compile_and_run(source: &str) -> i32 {
    let (exit_code, _, stderr) = compile_and_run_capture(source);
    if exit_code < 0 && !stderr.is_empty() {
        eprintln!("Compilation failed:\n{stderr}");
    }
    exit_code
}

/// Assert that a program compiles and runs with exit code 0.
pub fn assert_aot_success(source: &str, test_name: &str) {
    let exit_code = compile_and_run(source);
    assert_eq!(
        exit_code, 0,
        "{test_name} failed with exit code {exit_code}"
    );
}

/// Compile and run an Ori program, capturing stdout output.
///
/// Returns `(exit_code, stdout, stderr)`.
pub fn compile_and_run_capture(source: &str) -> (i32, String, String) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_{id}.ori"));
    let binary_path = temp_dir.path().join(format!("test_{id}"));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return (-1, String::new(), stderr);
    }

    let run_result = Command::new(&binary_path)
        .output()
        .expect("Failed to execute binary");

    let exit_code = run_result.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&run_result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
    (exit_code, stdout, stderr)
}

/// Create a minimal valid WASM module for testing.
///
/// This creates a WASM module with:
/// - A single exported function `_start` that returns immediately
/// - Memory export
pub const MINIMAL_WASM_MODULE: &[u8] = &[
    // WASM magic number and version
    0x00, 0x61, 0x73, 0x6D, // \0asm
    0x01, 0x00, 0x00, 0x00, // version 1
    // Type section (1 type: () -> ())
    0x01, // section id
    0x04, // section size
    0x01, // num types
    0x60, // func type
    0x00, // 0 params
    0x00, // 0 results
    // Function section (1 function with type 0)
    0x03, // section id
    0x02, // section size
    0x01, // num functions
    0x00, // function 0 uses type 0
    // Memory section (1 memory: initial 1 page)
    0x05, // section id
    0x03, // section size
    0x01, // num memories
    0x00, // flags (no max)
    0x01, // initial pages
    // Export section (2 exports: _start and memory)
    0x07, // section id
    0x13, // section size (19 bytes: 1 + 9 + 9)
    0x02, // num exports
    // Export 1: _start (function 0)
    0x06, // name length
    0x5F, 0x73, 0x74, 0x61, 0x72, 0x74, // "_start"
    0x00, // export kind (func)
    0x00, // function index
    // Export 2: memory (memory 0)
    0x06, // name length
    0x6D, 0x65, 0x6D, 0x6F, 0x72, 0x79, // "memory"
    0x02, // export kind (memory)
    0x00, // memory index
    // Code section (1 function body)
    0x0A, // section id
    0x04, // section size
    0x01, // num function bodies
    0x02, // body size
    0x00, // local count
    0x0B, // end
];

/// Create a WASM module with custom exports for testing.
pub fn wasm_module_with_exports(exports: &[(&str, &str)]) -> Vec<u8> {
    let module = MINIMAL_WASM_MODULE.to_vec();
    // For simplicity, return the minimal module
    // Real implementation would construct custom exports
    let _ = exports;
    module
}

/// Assert that a WASM verification has specific exports.
#[macro_export]
macro_rules! assert_wasm_exports {
    ($verification:expr, $($export:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::wasm_has_export(&$verification, $export),
                "Expected WASM export '{}' not found. Exports: {:?}",
                $export,
                $verification.exports.iter().map(|e| &e.name).collect::<Vec<_>>()
            );
        )+
    };
}

/// Assert that a WASM verification has specific imports.
#[macro_export]
macro_rules! assert_wasm_imports_from {
    ($verification:expr, $($module:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::wasm_has_import_from(&$verification, $module),
                "Expected WASM import from module '{}' not found. Imports: {:?}",
                $module,
                $verification.imports.iter().map(|i| &i.module).collect::<Vec<_>>()
            );
        )+
    };
}

/// Assert that an object file has specific symbols.
#[macro_export]
macro_rules! assert_object_symbols {
    ($verification:expr, $($symbol:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::object_has_symbol(&$verification, $symbol),
                "Expected symbol '{}' not found in object. Symbols: {:?}",
                $symbol,
                $verification.symbols.iter().map(|(n, _)| n).collect::<Vec<_>>()
            );
        )+
    };
}

/// Assert that a command contains specific arguments.
#[macro_export]
macro_rules! assert_command_args {
    ($cmd:expr, $($arg:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::command_has_arg(&$cmd, $arg),
                "Expected argument '{}' not found in command. Args: {:?}",
                $arg,
                $crate::util::command_args(&$cmd)
            );
        )+
    };
}
