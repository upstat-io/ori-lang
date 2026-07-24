//! Test Utilities for AOT Integration Tests
//!
//! Provides shared helpers for:
//! - Compiling and running Ori programs through the AOT pipeline
//! - Creating test fixtures (WASM modules, object files)
//! - Binary format verification
//! - Target configuration helpers
//! - Command execution utilities
//! - IR capture and inspection

mod binary;
mod commands;
mod compile;
mod ir_capture;
mod object;
mod targets;
mod wasm;

// Re-export everything for transparent access via `crate::util::*`
pub use binary::{ir_capture_binary, ori_binary, stdlib_path};
pub use commands::{
    clang_available, command_args, command_has_arg, command_has_arg_before, llvm_objdump_available,
    tool_available, wasm_ld_available, wasm_opt_available,
};
pub use compile::{
    assert_aot_success, assert_cell_output, assert_multifile_aot_success,
    assert_multifile_cell_output, assert_no_signal_crash, assert_panic_exit, compile_and_run,
    compile_and_run_capture, compile_and_run_valgrind_with_args, compile_and_run_with_args,
    compile_and_run_with_build_env, compile_and_run_with_env, compile_multifile_and_run_capture,
};
pub use ir_capture::{
    compile_and_capture_ir, compile_and_capture_ir_no_repr_opt, compile_to_llvm_ir,
    compile_to_llvm_ir_for_target, count_bridge_blocks, count_dead_phis, count_single_pred_phis,
    extract_function_ir, resolve_derived_function_name, resolve_function_attrs,
};
pub use object::{
    object_has_section, object_has_symbol, parse_object, ObjectFormat, ObjectVerification,
    SymbolKind,
};
pub use targets::{
    linux_target, macos_arm_target, macos_target, wasm32_target, wasm32_wasi_target,
    windows_gnu_target, windows_msvc_target,
};
pub use wasm::{
    browser_wasm_config, default_wasm_config, minimal_wasm_config, parse_wasm, wasi_config,
    wasm_has_custom_section, wasm_has_export, wasm_has_export_of_kind, wasm_has_import_from,
    wasm_module_with_exports, WasmExportInfo, WasmExportKind, WasmFeatures, WasmImportInfo,
    WasmImportKind, WasmMemoryInfo, WasmVerification, MINIMAL_WASM_MODULE,
};
