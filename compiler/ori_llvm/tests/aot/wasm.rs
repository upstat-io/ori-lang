//! WebAssembly AOT Compilation Tests
//!
//! Test scenarios inspired by:
//! - Rust: `tests/run-make/wasm-export-all-symbols/`, `wasm-custom-section/`
//! - Zig: `test/link/wasm/export/`, `test/link/wasm/basic-features/`
//!
//! These tests verify:
//! - Function export control
//! - Memory import/export configuration
//! - WASM feature flags
//! - WASI support
//! - wasm-opt integration

use ori_llvm::aot::wasm::{
    JsBindingGenerator, WasiConfig, WasiVersion, WasmConfig, WasmError, WasmExport,
    WasmMemoryConfig, WasmOptLevel, WasmOptRunner, WasmStackConfig, WasmType,
};
use ori_llvm::aot::{LinkOutput, WasmLinker};

use super::util::{
    parse_wasm, wasm32_target, wasm32_wasi_target, wasm_has_export, wasm_has_export_of_kind,
    WasmExportKind, MINIMAL_WASM_MODULE,
};
use crate::assert_command_args;

// ============================================================================
// WASM Module Parsing Tests
// ============================================================================

/// Test: Parse a minimal valid WASM module
///
/// Scenario from Rust `wasm-export-all-symbols`:
/// Verify that we can parse WASM exports correctly.
#[test]
fn test_parse_minimal_wasm_module() {
    let verification = parse_wasm(MINIMAL_WASM_MODULE).expect("Failed to parse WASM module");

    // Verify exports
    assert!(
        wasm_has_export(&verification, "_start"),
        "Expected '_start' export"
    );
    assert!(
        wasm_has_export(&verification, "memory"),
        "Expected 'memory' export"
    );

    // Verify export kinds
    assert!(wasm_has_export_of_kind(
        &verification,
        "_start",
        WasmExportKind::Function
    ));
    assert!(wasm_has_export_of_kind(
        &verification,
        "memory",
        WasmExportKind::Memory
    ));

    // Verify memory configuration
    assert_eq!(verification.memories.len(), 1);
    assert_eq!(verification.memories[0].initial_pages, 1);
}

/// Test: WASM module with no exports
///
/// Scenario: Library module without entry point.
#[test]
fn test_parse_wasm_detects_empty_exports() {
    // Minimal WASM with no exports
    let wasm_no_exports: &[u8] = &[
        0x00, 0x61, 0x73, 0x6D, // magic
        0x01, 0x00, 0x00, 0x00, // version
    ];

    let verification = parse_wasm(wasm_no_exports).expect("Failed to parse WASM");
    assert!(verification.exports.is_empty());
}

/// Test: Invalid WASM binary detection
///
/// Scenario: Corrupted or non-WASM data should fail.
#[test]
fn test_parse_invalid_wasm_fails() {
    let invalid = b"not a wasm module";
    let result = parse_wasm(invalid);
    assert!(result.is_err());
}

// ============================================================================
// Memory Configuration Tests
// ============================================================================

/// Test: Default memory configuration
///
/// Scenario from Zig `wasm/basic-features`:
/// Verify default memory settings.
#[test]
fn test_memory_config_default() {
    let config = WasmMemoryConfig::default();

    // 1MB initial (16 pages * 64KB)
    assert_eq!(config.initial_pages, 16);
    assert_eq!(config.initial_bytes(), 1_048_576);

    // 16MB max (256 pages * 64KB)
    assert_eq!(config.max_pages, Some(256));
    assert_eq!(config.max_bytes(), Some(16_777_216));

    // Export by default, don't import
    assert!(config.export_memory);
    assert!(!config.import_memory);
    assert!(!config.shared);
}

/// Test: Memory import configuration
///
/// Scenario: JavaScript host provides memory.
#[test]
fn test_memory_config_import() {
    let config = WasmMemoryConfig::default().with_import("env", "memory");

    assert!(config.import_memory);
    assert!(!config.export_memory);
    assert_eq!(
        config.import_name,
        Some(("env".to_string(), "memory".to_string()))
    );

    let args = config.linker_args();
    assert!(args.contains(&"--import-memory".to_string()));
    assert!(!args.contains(&"--export-memory".to_string()));
}

/// Test: Shared memory for threading
///
/// Scenario from Zig `wasm/shared-memory`:
/// Enable shared memory for web workers.
#[test]
fn test_memory_config_shared() {
    let config = WasmMemoryConfig::default().with_shared(true);

    assert!(config.shared);

    let args = config.linker_args();
    assert!(args.contains(&"--shared-memory".to_string()));
}

/// Test: Custom memory limits
///
/// Scenario: Constrained environment with limited memory.
#[test]
fn test_memory_config_custom_limits() {
    let config = WasmMemoryConfig::default()
        .with_initial_pages(4) // 256KB
        .with_max_pages(Some(16)); // 1MB max

    assert_eq!(config.initial_pages, 4);
    assert_eq!(config.initial_bytes(), 262_144);
    assert_eq!(config.max_pages, Some(16));
    assert_eq!(config.max_bytes(), Some(1_048_576));
}

/// Test: Unlimited memory (no max)
///
/// Scenario: Allow memory to grow without limit.
#[test]
fn test_memory_config_unlimited() {
    let config = WasmMemoryConfig::default().with_max_pages(None);

    assert!(config.max_pages.is_none());
    assert!(config.max_bytes().is_none());

    let args = config.linker_args();
    assert!(!args.iter().any(|a| a.contains("--max-memory")));
}

// ============================================================================
// Stack Configuration Tests
// ============================================================================

/// Test: Default stack configuration
#[test]
fn test_stack_config_default() {
    let config = WasmStackConfig::default();

    // 1MB stack
    assert_eq!(config.size, 1_048_576);
}

/// Test: Custom stack size
#[test]
fn test_stack_config_custom() {
    let config = WasmStackConfig::default().with_size_kb(512);

    assert_eq!(config.size, 524_288);

    let args = config.linker_args();
    assert!(args.contains(&"--stack-size=524288".to_string()));
}

// ============================================================================
// WASM Feature Flag Tests
// ============================================================================

/// Test: Enable bulk memory operations
///
/// Scenario from Zig `wasm/basic-features`:
/// Faster memcpy/memset using WASM bulk memory ops.
#[test]
fn test_wasm_config_bulk_memory() {
    let config = WasmConfig::default().with_bulk_memory(true);

    assert!(config.bulk_memory);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-bulk-memory".to_string()));
}

/// Test: Enable SIMD instructions
///
/// Scenario: Performance-critical code using SIMD.
#[test]
fn test_wasm_config_simd() {
    let config = WasmConfig::default().with_simd(true);

    assert!(config.simd);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-simd".to_string()));
}

/// Test: Enable reference types
///
/// Scenario: Using externref for JS interop.
#[test]
fn test_wasm_config_reference_types() {
    let config = WasmConfig::default().with_reference_types(true);

    assert!(config.reference_types);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-reference-types".to_string()));
}

/// Test: Enable multi-value returns
///
/// Scenario: Functions returning multiple values.
#[test]
fn test_wasm_config_multivalue() {
    let config = WasmConfig::default();

    // Enabled by default
    assert!(config.multi_value);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-multivalue".to_string()));
}

/// Test: Enable exception handling
///
/// Scenario: Using WASM exceptions proposal.
#[test]
fn test_wasm_config_exception_handling() {
    let config = WasmConfig::default().with_exception_handling(true);

    assert!(config.exception_handling);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-exception-handling".to_string()));
}

/// Test: All features enabled
///
/// Scenario: Maximum feature set for modern browsers.
#[test]
fn test_wasm_config_all_features() {
    let config = WasmConfig::default()
        .with_bulk_memory(true)
        .with_simd(true)
        .with_reference_types(true)
        .with_exception_handling(true);

    let args = config.linker_args();
    assert!(args.contains(&"--enable-bulk-memory".to_string()));
    assert!(args.contains(&"--enable-simd".to_string()));
    assert!(args.contains(&"--enable-reference-types".to_string()));
    assert!(args.contains(&"--enable-exception-handling".to_string()));
    assert!(args.contains(&"--enable-multivalue".to_string()));
}

// ============================================================================
// WASM Configuration Presets Tests
// ============================================================================

/// Test: Standalone WASM configuration
///
/// Scenario: WASM module without WASI or JS bindings.
#[test]
fn test_wasm_config_standalone() {
    let config = WasmConfig::standalone();

    assert!(!config.wasi);
    assert!(!config.generate_js_bindings);
    assert!(!config.generate_dts);
    assert!(config.bulk_memory);
    assert!(!config.simd);
}

/// Test: Browser WASM configuration
///
/// Scenario: WASM module for browser embedding.
#[test]
fn test_wasm_config_browser() {
    let config = WasmConfig::browser();

    assert!(!config.wasi);
    assert!(config.generate_js_bindings);
    assert!(config.generate_dts);
    assert!(config.bulk_memory);
}

/// Test: WASI WASM configuration
///
/// Scenario: WASM module using WASI system interface.
#[test]
fn test_wasm_config_wasi() {
    let config = WasmConfig::wasi();

    assert!(config.wasi);
    assert!(config.wasi_config.is_some());
}

/// Test: WASI CLI configuration
///
/// Scenario: Command-line WASI application.
#[test]
fn test_wasm_config_wasi_cli() {
    let config = WasmConfig::wasi_cli();

    assert!(config.wasi);
    let wasi = config.wasi_config.unwrap();
    assert!(wasi.filesystem);
    assert!(wasi.env);
    assert!(wasi.args);
}

/// Test: Minimal WASI configuration
///
/// Scenario: Sandboxed WASI without filesystem access.
#[test]
fn test_wasm_config_wasi_minimal() {
    let config = WasmConfig::wasi_minimal();

    assert!(config.wasi);
    let wasi = config.wasi_config.unwrap();
    assert!(!wasi.filesystem);
    assert!(!wasi.env);
    assert!(!wasi.args);
    assert!(wasi.clock);
    assert!(wasi.random);
}

// ============================================================================
// WASI Configuration Tests
// ============================================================================

/// Test: WASI version configuration
#[test]
fn test_wasi_version() {
    assert_eq!(WasiVersion::default(), WasiVersion::Preview1);
    assert_eq!(WasiVersion::Preview1.target_suffix(), "wasi");
    assert_eq!(WasiVersion::Preview2.target_suffix(), "wasip2");
}

/// Test: WASI preopened directories
///
/// Scenario from WASI spec: Mapping host paths to guest paths.
#[test]
fn test_wasi_preopens() {
    let config = WasiConfig::default()
        .with_preopen("/app", "/home/user/app")
        .with_preopen("/data", "/var/data");

    assert_eq!(config.preopens.len(), 2);
    assert_eq!(config.preopens[0].guest_path, "/app");
    assert_eq!(config.preopens[0].host_path, "/home/user/app");
}

/// Test: WASI environment variables
///
/// Scenario: Passing configuration to WASI module.
#[test]
fn test_wasi_env_vars() {
    let config = WasiConfig::default()
        .with_env("HOME", "/app")
        .with_env("DEBUG", "1");

    assert_eq!(config.env_vars.len(), 2);
    assert_eq!(config.env_vars[0], ("HOME".to_string(), "/app".to_string()));
}

/// Test: WASI command-line arguments
#[test]
fn test_wasi_argv() {
    let config = WasiConfig::default().with_args(vec!["arg1".to_string(), "--flag".to_string()]);

    assert_eq!(config.argv.len(), 2);
}

/// Test: WASI undefined symbols list
///
/// Scenario from Rust `wasm-symbols`:
/// Verify WASI imports are generated correctly.
#[test]
fn test_wasi_undefined_symbols() {
    let config = WasiConfig::default();
    let symbols = config.undefined_symbols();

    // Core WASI symbols
    assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.fd_write"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.fd_read"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.fd_close"));

    // Filesystem (enabled by default)
    assert!(symbols.contains(&"wasi_snapshot_preview1.path_open"));

    // Clock (enabled by default)
    assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));

    // Random (enabled by default)
    assert!(symbols.contains(&"wasi_snapshot_preview1.random_get"));
}

/// Test: Minimal WASI undefined symbols
///
/// Scenario: Verify minimal config has fewer imports.
#[test]
fn test_wasi_minimal_undefined_symbols() {
    let config = WasiConfig::minimal();
    let symbols = config.undefined_symbols();

    // Core symbols still present
    assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.fd_write"));

    // Filesystem NOT present
    assert!(!symbols.contains(&"wasi_snapshot_preview1.path_open"));

    // Clock and random still present
    assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.random_get"));
}

// ============================================================================
// wasm-opt Integration Tests
// ============================================================================

/// Test: wasm-opt optimization levels
#[test]
fn test_wasm_opt_levels() {
    assert_eq!(WasmOptLevel::O0.flag(), "-O0");
    assert_eq!(WasmOptLevel::O1.flag(), "-O1");
    assert_eq!(WasmOptLevel::O2.flag(), "-O2");
    assert_eq!(WasmOptLevel::O3.flag(), "-O3");
    assert_eq!(WasmOptLevel::O4.flag(), "-O4");
    assert_eq!(WasmOptLevel::Os.flag(), "-Os");
    assert_eq!(WasmOptLevel::Oz.flag(), "-Oz");
}

/// Test: wasm-opt runner configuration
#[test]
fn test_wasm_opt_runner_config() {
    let runner = WasmOptRunner::new()
        .with_opt_level(WasmOptLevel::Oz)
        .with_debug_names(true)
        .with_source_map(true)
        .with_feature("bulk-memory")
        .with_feature("simd");

    let cmd = runner.build_command(
        std::path::Path::new("input.wasm"),
        std::path::Path::new("output.wasm"),
    );

    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

    assert!(args.contains(&"-Oz".into()));
    assert!(args.contains(&"--debuginfo".into()));
    assert!(args.contains(&"--source-map".into()));
    assert!(args.contains(&"--enable-bulk-memory".into()));
    assert!(args.contains(&"--enable-simd".into()));
}

/// Test: wasm-opt with custom path
#[test]
fn test_wasm_opt_runner_custom_path() {
    let runner = WasmOptRunner::new().with_path("/opt/binaryen/bin/wasm-opt");

    let cmd = runner.build_command(
        std::path::Path::new("in.wasm"),
        std::path::Path::new("out.wasm"),
    );

    assert_eq!(
        cmd.get_program().to_string_lossy(),
        "/opt/binaryen/bin/wasm-opt"
    );
}

// ============================================================================
// WASM Linker Command Tests
// ============================================================================

/// Test: WASM linker executable output
///
/// Scenario from Zig `wasm/export`:
/// Entry point configuration for executables.
#[test]
fn test_wasm_linker_executable() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.set_output_kind(LinkOutput::Executable);
    linker.set_output(std::path::Path::new("output.wasm"));

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--entry=_start", "-o", "output.wasm");
}

/// Test: WASM linker shared library output
///
/// Scenario: Library module without entry point.
#[test]
fn test_wasm_linker_shared_library() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.set_output_kind(LinkOutput::SharedLibrary);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--no-entry", "--export-dynamic");
}

/// Test: WASM linker export symbols
///
/// Scenario from Rust `wasm-export-all-symbols`:
/// Explicit symbol export control.
#[test]
fn test_wasm_linker_export_symbols() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.export_symbols(&[
        "main".to_string(),
        "add".to_string(),
        "multiply".to_string(),
    ]);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--export=main", "--export=add", "--export=multiply");
}

/// Test: WASM linker memory configuration
///
/// Scenario from Zig `wasm/basic-features`:
/// Memory limits and import/export.
#[test]
fn test_wasm_linker_memory_config() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.set_memory(1_048_576, Some(16_777_216)); // 1MB initial, 16MB max
    linker.export_memory(true);

    let cmd = linker.finalize();
    assert_command_args!(
        cmd,
        "--initial-memory=1048576",
        "--max-memory=16777216",
        "--export-memory"
    );
}

/// Test: WASM linker import memory
///
/// Scenario: JavaScript host provides memory.
#[test]
fn test_wasm_linker_import_memory() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.import_memory(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--import-memory");
}

/// Test: WASM linker feature flags
///
/// Scenario: Enable modern WASM features.
#[test]
fn test_wasm_linker_features() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.enable_bulk_memory(true);
    linker.enable_simd(true);
    linker.enable_multivalue(true);
    linker.enable_reference_types(true);
    linker.enable_exception_handling(true);

    let cmd = linker.finalize();
    assert_command_args!(
        cmd,
        "--enable-bulk-memory",
        "--enable-simd",
        "--enable-multivalue",
        "--enable-reference-types",
        "--enable-exception-handling"
    );
}

/// Test: WASM linker with WasmConfig
///
/// Scenario: Apply comprehensive configuration.
#[test]
fn test_wasm_linker_apply_config() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    let config = WasmConfig::default()
        .with_memory(
            WasmMemoryConfig::default()
                .with_initial_pages(32)
                .with_max_pages(Some(128)),
        )
        .with_stack(WasmStackConfig::default().with_size_kb(256))
        .with_bulk_memory(true)
        .with_simd(true);

    linker.apply_config(&config);

    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

    // Memory config
    assert!(args.iter().any(|a| a.contains("--initial-memory=")));
    assert!(args.iter().any(|a| a.contains("--max-memory=")));
    assert!(args.iter().any(|a| a.contains("--stack-size=")));

    // Features
    assert!(args.contains(&"--enable-bulk-memory".into()));
    assert!(args.contains(&"--enable-simd".into()));
}

/// Test: WASM linker GC and strip
///
/// Scenario from Rust `wasm-export-all-symbols`:
/// Size optimization via dead code elimination.
#[test]
fn test_wasm_linker_gc_and_strip() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.gc_sections(true);
    linker.strip_symbols(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--gc-sections", "--strip-all");
}

/// Test: WASM linker allow undefined
///
/// Scenario: WASI modules with host imports.
#[test]
fn test_wasm_linker_allow_undefined() {
    let target = wasm32_wasi_target();
    let mut linker = WasmLinker::new(&target);

    linker.allow_undefined(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--allow-undefined");
}

/// Test: WASM linker custom entry point
///
/// Scenario: Non-standard entry function name.
#[test]
fn test_wasm_linker_custom_entry() {
    let target = wasm32_target();
    let mut linker = WasmLinker::new(&target);

    linker.set_entry("main");

    let cmd = linker.finalize();
    assert_command_args!(cmd, "--entry=main");
}

// ============================================================================
// JavaScript Binding Generation Tests
// ============================================================================

/// Test: JS binding generator creates valid output
#[test]
fn test_js_binding_generator() {
    let exports = vec![
        WasmExport {
            ori_name: "add".to_string(),
            wasm_name: "_ori_add_ii".to_string(),
            params: vec![WasmType::I32, WasmType::I32],
            return_type: WasmType::I32,
            is_async: false,
        },
        WasmExport {
            ori_name: "greet".to_string(),
            wasm_name: "_ori_greet_s".to_string(),
            params: vec![WasmType::String],
            return_type: WasmType::Void,
            is_async: false,
        },
    ];

    let gen = JsBindingGenerator::new("test_module", exports);
    let temp_dir = std::env::temp_dir();
    let js_path = temp_dir.join("test_bindings.js");
    let dts_path = temp_dir.join("test_bindings.d.ts");

    // Generate JS
    gen.generate_js(&js_path).expect("Failed to generate JS");

    // Verify JS content
    let js_content = std::fs::read_to_string(&js_path).unwrap();
    assert!(js_content.contains("function add"));
    assert!(js_content.contains("function greet"));
    assert!(js_content.contains("TextEncoder"));
    assert!(js_content.contains("TextDecoder"));
    assert!(js_content.contains("export { init"));

    // Generate TypeScript declarations
    gen.generate_dts(&dts_path).expect("Failed to generate dts");

    // Verify dts content
    let dts_content = std::fs::read_to_string(&dts_path).unwrap();
    assert!(dts_content.contains("export function add"));
    assert!(dts_content.contains("export function greet"));
    assert!(dts_content.contains("WebAssembly.Memory"));

    // Cleanup
    let _ = std::fs::remove_file(&js_path);
    let _ = std::fs::remove_file(&dts_path);
}

/// Test: WasmType TypeScript mappings
#[test]
fn test_wasm_type_mappings() {
    assert_eq!(WasmType::I32.typescript_type(), "number");
    assert_eq!(WasmType::I64.typescript_type(), "number");
    assert_eq!(WasmType::F32.typescript_type(), "number");
    assert_eq!(WasmType::F64.typescript_type(), "number");
    assert_eq!(WasmType::String.typescript_type(), "string");
    assert_eq!(WasmType::Void.typescript_type(), "void");
    assert_eq!(WasmType::Pointer.typescript_type(), "number");
    assert_eq!(
        WasmType::List(Box::new(WasmType::I32)).typescript_type(),
        "Array<any>"
    );
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test: WASM error display
#[test]
fn test_wasm_error_display() {
    let err = WasmError::JsBindingGeneration {
        message: "test error".to_string(),
    };
    assert!(err.to_string().contains("JavaScript bindings"));

    let err = WasmError::DtsGeneration {
        message: "test error".to_string(),
    };
    assert!(err.to_string().contains("TypeScript declarations"));

    let err = WasmError::WriteError {
        path: "/tmp/test.js".to_string(),
        message: "permission denied".to_string(),
    };
    assert!(err.to_string().contains("/tmp/test.js"));

    let err = WasmError::InvalidConfig {
        message: "bad config".to_string(),
    };
    assert!(err.to_string().contains("invalid WASM configuration"));
}
