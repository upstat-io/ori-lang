//! Test Utilities for AOT Integration Tests
//!
//! Provides shared helpers for:
//! - Compiling and running Ori programs through the AOT pipeline
//! - Creating test fixtures (WASM modules, object files)
//! - Binary format verification
//! - Target configuration helpers
//! - Command execution utilities

mod aot;
mod object;
mod wasm;

// Re-export everything for transparent access via `crate::util::*`
pub use aot::{
    assert_aot_success, compile_and_capture_ir, compile_and_run, compile_and_run_capture,
    count_bridge_blocks, count_dead_phis, count_single_pred_phis, extract_function_ir, ori_binary,
    stdlib_path,
};
pub use object::{
    object_has_section, object_has_symbol, parse_object, ObjectFormat, ObjectVerification,
    SymbolKind,
};
pub use wasm::{
    browser_wasm_config, default_wasm_config, minimal_wasm_config, parse_wasm, wasi_config,
    wasm_has_custom_section, wasm_has_export, wasm_has_export_of_kind, wasm_has_import_from,
    wasm_module_with_exports, WasmExportInfo, WasmExportKind, WasmFeatures, WasmImportInfo,
    WasmImportKind, WasmMemoryInfo, WasmVerification, MINIMAL_WASM_MODULE,
};

use std::process::Command;

use ori_llvm::aot::{TargetConfig, TargetTripleComponents};

// Target helpers

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

// Command utilities

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
