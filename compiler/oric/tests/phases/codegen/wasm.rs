//! Tests for WASM-specific AOT configuration (`ori_llvm::aot::wasm`).
//!
//! These tests verify:
//! - Memory configuration (pages, import/export)
//! - Stack configuration
//! - WASM feature flags (bulk memory, SIMD, etc.)
//! - wasm-opt runner
//! - JavaScript/TypeScript binding generation
//! - WASI configuration

use ori_llvm::aot::wasm::{
    JsBindingGenerator, WasiConfig, WasiPreopen, WasiVersion, WasmConfig, WasmError, WasmExport,
    WasmMemoryConfig, WasmOptLevel, WasmOptRunner, WasmStackConfig, WasmType,
};
use std::path::Path;

// Helper function for pascal_case (reimplemented for testing since it's module-private)
fn pascal_case(s: &str) -> String {
    s.split(|c| c == '_' || c == '-')
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

#[test]
fn test_memory_config_default() {
    let config = WasmMemoryConfig::default();
    assert_eq!(config.initial_pages, 16);
    assert_eq!(config.max_pages, Some(256));
    assert!(!config.import_memory);
    assert!(config.export_memory);
}

#[test]
fn test_memory_config_bytes() {
    let config = WasmMemoryConfig::default();
    assert_eq!(config.initial_bytes(), 16 * 65536); // 1MB
    assert_eq!(config.max_bytes(), Some(256 * 65536)); // 16MB
}

#[test]
fn test_memory_config_linker_args() {
    let config = WasmMemoryConfig::default();
    let args = config.linker_args();
    assert!(args.contains(&"--initial-memory=1048576".to_string()));
    assert!(args.contains(&"--max-memory=16777216".to_string()));
    assert!(args.contains(&"--export-memory".to_string()));
}

#[test]
fn test_memory_config_import() {
    let config = WasmMemoryConfig::default().with_import("env", "memory");
    assert!(config.import_memory);
    assert!(!config.export_memory);
    let args = config.linker_args();
    assert!(args.contains(&"--import-memory".to_string()));
}

#[test]
fn test_stack_config_default() {
    let config = WasmStackConfig::default();
    assert_eq!(config.size, 1024 * 1024); // 1MB
}

#[test]
fn test_stack_config_linker_args() {
    let config = WasmStackConfig::default().with_size_kb(512);
    let args = config.linker_args();
    assert!(args.contains(&format!("--stack-size={}", 512 * 1024)));
}

#[test]
fn test_wasm_config_standalone() {
    let config = WasmConfig::standalone();
    assert!(!config.wasi);
    assert!(!config.generate_js_bindings());
    assert!(config.bulk_memory());
}

#[test]
fn test_wasm_config_wasi() {
    let config = WasmConfig::wasi();
    assert!(config.wasi);
}

#[test]
fn test_wasm_config_browser() {
    let config = WasmConfig::browser();
    assert!(config.generate_js_bindings());
    assert!(config.generate_dts());
}

#[test]
fn test_wasm_config_linker_args() {
    let config = WasmConfig::default().with_bulk_memory(true).with_simd(true);
    let args = config.linker_args();
    assert!(args.contains(&"--enable-bulk-memory".to_string()));
    assert!(args.contains(&"--enable-simd".to_string()));
}

#[test]
fn test_wasm_opt_level_flags() {
    assert_eq!(WasmOptLevel::O0.flag(), "-O0");
    assert_eq!(WasmOptLevel::O2.flag(), "-O2");
    assert_eq!(WasmOptLevel::Os.flag(), "-Os");
    assert_eq!(WasmOptLevel::Oz.flag(), "-Oz");
}

// wasm-opt Runner Tests

#[test]
fn test_wasm_opt_runner_default() {
    let runner = WasmOptRunner::default();
    assert_eq!(runner.level, WasmOptLevel::O2);
    assert!(!runner.debug_names);
    assert!(!runner.source_map);
}

#[test]
fn test_wasm_opt_runner_with_level() {
    let runner = WasmOptRunner::with_level(WasmOptLevel::Oz);
    assert_eq!(runner.level, WasmOptLevel::Oz);
}

#[test]
fn test_wasm_opt_runner_builder() {
    let runner = WasmOptRunner::new()
        .with_opt_level(WasmOptLevel::O3)
        .with_debug_names(true)
        .with_source_map(true)
        .with_feature("bulk-memory")
        .with_feature("simd");

    assert_eq!(runner.level, WasmOptLevel::O3);
    assert!(runner.debug_names);
    assert!(runner.source_map);
    assert_eq!(runner.features.len(), 2);
}

#[test]
fn test_wasm_opt_runner_with_path() {
    let runner = WasmOptRunner::new().with_path("/usr/local/bin/wasm-opt");
    assert_eq!(
        runner.wasm_opt_path,
        std::path::PathBuf::from("/usr/local/bin/wasm-opt")
    );
}

#[test]
fn test_wasm_opt_runner_build_command() {
    let runner = WasmOptRunner::new()
        .with_opt_level(WasmOptLevel::Oz)
        .with_debug_names(true)
        .with_feature("bulk-memory");

    let cmd = runner.build_command(Path::new("input.wasm"), Path::new("output.wasm"));
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

    assert!(args.contains(&"-Oz".into()));
    assert!(args.contains(&"input.wasm".into()));
    assert!(args.contains(&"-o".into()));
    assert!(args.contains(&"output.wasm".into()));
    assert!(args.contains(&"--debuginfo".into()));
    assert!(args.contains(&"--enable-bulk-memory".into()));
}

#[test]
fn test_wasm_opt_runner_build_command_with_source_map() {
    let runner = WasmOptRunner::new().with_source_map(true);

    let cmd = runner.build_command(Path::new("in.wasm"), Path::new("out.wasm"));
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

    assert!(args.contains(&"--source-map".into()));
}

#[test]
fn test_wasm_type_typescript() {
    assert_eq!(WasmType::I32.typescript_type(), "number");
    assert_eq!(WasmType::String.typescript_type(), "string");
    assert_eq!(WasmType::Void.typescript_type(), "void");
}

#[test]
fn test_pascal_case() {
    assert_eq!(pascal_case("my_module"), "MyModule");
    assert_eq!(pascal_case("hello-world"), "HelloWorld");
    assert_eq!(pascal_case("test"), "Test");
    assert_eq!(pascal_case("a_b_c"), "ABC");
}

#[test]
fn test_js_binding_generator_new() {
    let exports = vec![WasmExport {
        ori_name: "add".to_string(),
        wasm_name: "_ori_add_ii".to_string(),
        params: vec![WasmType::I32, WasmType::I32],
        return_type: WasmType::I32,
        is_async: false,
    }];
    let gen = JsBindingGenerator::new("my_module", exports);
    assert_eq!(gen.module_name, "my_module");
    assert_eq!(gen.exports.len(), 1);
}

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
}

// WASI Configuration Tests

#[test]
fn test_wasi_version_default() {
    let version = WasiVersion::default();
    assert_eq!(version, WasiVersion::Preview1);
    assert_eq!(version.target_suffix(), "wasi");
}

#[test]
fn test_wasi_version_preview2() {
    let version = WasiVersion::Preview2;
    assert_eq!(version.target_suffix(), "wasip2");
}

#[test]
fn test_wasi_config_default() {
    let config = WasiConfig::default();
    assert!(config.filesystem);
    assert!(config.clock);
    assert!(config.random);
    assert!(config.env);
    assert!(config.args);
    assert!(config.preopens.is_empty());
}

#[test]
fn test_wasi_config_minimal() {
    let config = WasiConfig::minimal();
    assert!(!config.filesystem);
    assert!(config.clock);
    assert!(config.random);
    assert!(!config.env);
    assert!(!config.args);
}

#[test]
fn test_wasi_config_cli() {
    let config = WasiConfig::cli();
    assert!(config.filesystem);
    assert!(config.clock);
    assert!(config.random);
    assert!(config.env);
    assert!(config.args);
}

#[test]
fn test_wasi_config_with_preopen() {
    let config = WasiConfig::default()
        .with_preopen("/app", "/home/user/app")
        .with_preopen("/data", "/var/data");
    assert_eq!(config.preopens.len(), 2);
    assert_eq!(config.preopens[0].guest_path, "/app");
    assert_eq!(config.preopens[0].host_path, "/home/user/app");
}

#[test]
fn test_wasi_config_with_env() {
    let config = WasiConfig::default()
        .with_env("HOME", "/app")
        .with_env("DEBUG", "1");
    assert_eq!(config.env_vars.len(), 2);
    assert_eq!(config.env_vars[0], ("HOME".to_string(), "/app".to_string()));
}

#[test]
fn test_wasi_config_with_args() {
    let config = WasiConfig::default().with_args(vec!["arg1".to_string(), "arg2".to_string()]);
    assert_eq!(config.argv.len(), 2);
}

#[test]
fn test_wasi_config_undefined_symbols() {
    let config = WasiConfig::default();
    let symbols = config.undefined_symbols();
    // Should contain core WASI imports
    assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
    assert!(symbols.contains(&"wasi_snapshot_preview1.fd_write"));
    // Should contain filesystem imports (enabled by default)
    assert!(symbols.contains(&"wasi_snapshot_preview1.path_open"));
    // Should contain clock imports
    assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));
    // Should contain random imports
    assert!(symbols.contains(&"wasi_snapshot_preview1.random_get"));
}

#[test]
fn test_wasi_config_minimal_symbols() {
    let config = WasiConfig::minimal();
    let symbols = config.undefined_symbols();
    // Should contain core imports
    assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
    // Should NOT contain filesystem imports
    assert!(!symbols.contains(&"wasi_snapshot_preview1.path_open"));
    // Should contain clock imports
    assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));
}

#[test]
fn test_wasm_config_wasi_cli() {
    let config = WasmConfig::wasi_cli();
    assert!(config.wasi);
    assert!(config.wasi_config.is_some());
    let wasi = config.wasi_config.unwrap();
    assert!(wasi.filesystem);
    assert!(wasi.args);
}

#[test]
fn test_wasm_config_wasi_minimal() {
    let config = WasmConfig::wasi_minimal();
    assert!(config.wasi);
    assert!(config.wasi_config.is_some());
    let wasi = config.wasi_config.unwrap();
    assert!(!wasi.filesystem);
}

#[test]
fn test_wasm_config_with_wasi_config() {
    let wasi = WasiConfig::default().with_preopen("/", "/tmp");
    let config = WasmConfig::default().with_wasi_config(wasi);
    assert!(config.wasi);
    assert!(config.wasi_config.is_some());
    assert_eq!(config.wasi_config.unwrap().preopens.len(), 1);
}

#[test]
fn test_wasm_config_with_wasi_enables_wasi() {
    let config = WasmConfig::default().with_wasi(true);
    assert!(config.wasi);
    // Should auto-create default WASI config
    assert!(config.wasi_config.is_some());
}

// Additional coverage tests

#[test]
fn test_wasm_error_invalid_config() {
    let err = WasmError::InvalidConfig {
        message: "bad config".to_string(),
    };
    assert!(err.to_string().contains("invalid WASM configuration"));
    assert!(err.to_string().contains("bad config"));
}

#[test]
fn test_wasm_type_all_variants() {
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

    // JSDoc types
    assert_eq!(WasmType::I32.jsdoc_type(), "number");
    assert_eq!(WasmType::String.jsdoc_type(), "string");
    assert_eq!(WasmType::Void.jsdoc_type(), "void");
    assert_eq!(WasmType::Pointer.jsdoc_type(), "number");
    assert_eq!(
        WasmType::List(Box::new(WasmType::I32)).jsdoc_type(),
        "Array"
    );
}

#[test]
fn test_memory_config_with_export() {
    let config = WasmMemoryConfig::default().with_export("mem");
    assert!(config.export_memory);
    assert_eq!(config.export_name, Some("mem".to_string()));
}

#[test]
fn test_memory_config_without_export() {
    let config = WasmMemoryConfig::default().without_export();
    assert!(!config.export_memory);
    assert!(config.export_name.is_none());
}

#[test]
fn test_memory_config_with_shared() {
    let config = WasmMemoryConfig::default().with_shared(true);
    assert!(config.shared);
    let args = config.linker_args();
    assert!(args.contains(&"--shared-memory".to_string()));
}

#[test]
fn test_memory_config_no_max() {
    let config = WasmMemoryConfig::default().with_max_pages(None);
    assert!(config.max_pages.is_none());
    assert!(config.max_bytes().is_none());
    let args = config.linker_args();
    assert!(!args.iter().any(|a| a.contains("--max-memory")));
}

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
}

#[test]
fn test_wasm_config_with_reference_types() {
    let config = WasmConfig::default().with_reference_types(true);
    assert!(config.reference_types());
}

#[test]
fn test_wasm_config_with_exception_handling() {
    let config = WasmConfig::default().with_exception_handling(true);
    assert!(config.exception_handling());
}

#[test]
fn test_js_binding_generator_generate_js() {
    use std::fs;

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
    let js_path = temp_dir.join("test_ori_wasm.js");

    let result = gen.generate_js(&js_path);
    assert!(result.is_ok(), "generate_js failed: {result:?}");

    // Verify file was created and has content
    let content = fs::read_to_string(&js_path).unwrap();
    assert!(content.contains("test_module.wasm"));
    assert!(content.contains("TextEncoder"));
    assert!(content.contains("TextDecoder"));
    assert!(content.contains("function add"));
    assert!(content.contains("function greet"));
    assert!(content.contains("export { init"));

    // Clean up
    let _ = fs::remove_file(&js_path);
}

#[test]
fn test_js_binding_generator_generate_dts() {
    use std::fs;

    let exports = vec![WasmExport {
        ori_name: "calculate".to_string(),
        wasm_name: "_ori_calc".to_string(),
        params: vec![WasmType::F64],
        return_type: WasmType::F64,
        is_async: true,
    }];
    let gen = JsBindingGenerator::new("calc_module", exports);

    let temp_dir = std::env::temp_dir();
    let dts_path = temp_dir.join("test_ori_wasm.d.ts");

    let result = gen.generate_dts(&dts_path);
    assert!(result.is_ok(), "generate_dts failed: {result:?}");

    // Verify file was created and has content
    let content = fs::read_to_string(&dts_path).unwrap();
    assert!(content.contains("CalcModule"));
    assert!(content.contains("WebAssembly.Memory"));
    assert!(content.contains("export function init"));
    assert!(content.contains("export function calculate"));
    assert!(content.contains("Promise<number>")); // async function

    // Clean up
    let _ = fs::remove_file(&dts_path);
}

#[test]
fn test_js_binding_generator_string_return() {
    use std::fs;

    let exports = vec![WasmExport {
        ori_name: "get_name".to_string(),
        wasm_name: "_ori_get_name".to_string(),
        params: vec![],
        return_type: WasmType::String,
        is_async: false,
    }];
    let gen = JsBindingGenerator::new("name_module", exports);

    let temp_dir = std::env::temp_dir();
    let js_path = temp_dir.join("test_ori_wasm_string.js");

    let result = gen.generate_js(&js_path);
    assert!(result.is_ok());

    let content = fs::read_to_string(&js_path).unwrap();
    assert!(content.contains("get_name"));

    let _ = fs::remove_file(&js_path);
}

#[test]
fn test_wasi_config_write_undefined_symbols() {
    use std::fs;

    let config = WasiConfig::default();
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_wasi_symbols.txt");

    let result = config.write_undefined_symbols(&path);
    assert!(result.is_ok(), "write_undefined_symbols failed: {result:?}");

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("wasi_snapshot_preview1.proc_exit"));
    assert!(content.contains("wasi_snapshot_preview1.fd_write"));
    assert!(content.contains("wasi_snapshot_preview1.path_open"));

    let _ = fs::remove_file(&path);
}

#[test]
fn test_wasm_opt_runner_is_available() {
    // wasm-opt may or may not be available in the test environment
    let runner = WasmOptRunner::new();
    let _available = runner.is_available(); // Just verify it doesn't panic
}

#[test]
fn test_wasm_opt_all_levels() {
    assert_eq!(WasmOptLevel::O1.flag(), "-O1");
    assert_eq!(WasmOptLevel::O3.flag(), "-O3");
    assert_eq!(WasmOptLevel::O4.flag(), "-O4");
}

#[test]
fn test_pascal_case_empty() {
    assert_eq!(pascal_case(""), "");
}

#[test]
fn test_pascal_case_single_char() {
    assert_eq!(pascal_case("a"), "A");
}

#[test]
fn test_pascal_case_multiple_separators() {
    assert_eq!(pascal_case("a_b-c_d"), "ABCD");
}

#[test]
fn test_wasm_export_clone() {
    let export = WasmExport {
        ori_name: "test".to_string(),
        wasm_name: "_test".to_string(),
        params: vec![WasmType::I32],
        return_type: WasmType::Void,
        is_async: false,
    };
    let cloned = export.clone();
    assert_eq!(cloned.ori_name, "test");
}

#[test]
fn test_wasi_preopen_clone() {
    let preopen = WasiPreopen {
        guest_path: "/app".to_string(),
        host_path: "/home/user/app".to_string(),
    };
    let cloned = preopen.clone();
    assert_eq!(cloned.guest_path, "/app");
    assert_eq!(cloned.host_path, "/home/user/app");
}

#[test]
fn test_wasm_config_clone() {
    let config = WasmConfig::browser();
    let cloned = config.clone();
    assert!(cloned.generate_js_bindings());
    assert!(cloned.generate_dts());
}

#[test]
fn test_wasi_config_clone() {
    let config = WasiConfig::cli()
        .with_preopen("/app", "/tmp")
        .with_env("KEY", "value");
    let cloned = config.clone();
    assert_eq!(cloned.preopens.len(), 1);
    assert_eq!(cloned.env_vars.len(), 1);
}

#[test]
fn test_js_binding_empty_exports() {
    use std::fs;

    let gen = JsBindingGenerator::new("empty_module", vec![]);
    let temp_dir = std::env::temp_dir();
    let js_path = temp_dir.join("test_ori_empty.js");

    let result = gen.generate_js(&js_path);
    assert!(result.is_ok());

    let content = fs::read_to_string(&js_path).unwrap();
    assert!(content.contains("EmptyModule"));

    let _ = fs::remove_file(&js_path);
}
