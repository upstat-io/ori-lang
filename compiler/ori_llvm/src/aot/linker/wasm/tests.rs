use super::*;
use crate::aot::target::TargetTripleComponents;

fn test_target() -> TargetConfig {
    let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    TargetConfig::from_components(components)
}

#[test]
fn test_wasm_linker_new() {
    let target = test_target();
    let linker = WasmLinker::new(&target);
    assert!(linker.target().is_wasm());
}

#[test]
fn test_wasm_linker_output() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_output(Path::new("output.wasm"));
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"-o".into()));
    assert!(args.contains(&"output.wasm".into()));
}

#[test]
fn test_wasm_linker_executable() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_output_kind(LinkOutput::Executable);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--entry=_start".into()));
}

#[test]
fn test_wasm_linker_shared_library() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_output_kind(LinkOutput::SharedLibrary);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--no-entry".into()));
    assert!(args.contains(&"--export-dynamic".into()));
}

#[test]
fn test_wasm_linker_memory_config() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_memory(1024 * 1024, Some(16 * 1024 * 1024));
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--initial-memory=1048576".into()));
    assert!(args.contains(&"--max-memory=16777216".into()));
}

#[test]
fn test_wasm_linker_stack_size() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_stack_size(512 * 1024);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--stack-size=524288".into()));
}

#[test]
fn test_wasm_linker_import_export_memory() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.import_memory(true);
    linker.export_memory(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--import-memory".into()));
    assert!(args.contains(&"--export-memory".into()));
}

#[test]
fn test_wasm_linker_features() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.enable_bulk_memory(true);
    linker.enable_simd(true);
    linker.enable_multivalue(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--enable-bulk-memory".into()));
    assert!(args.contains(&"--enable-simd".into()));
    assert!(args.contains(&"--enable-multivalue".into()));
}

#[test]
fn test_wasm_linker_gc_and_strip() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.gc_sections(true);
    linker.strip_symbols(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--gc-sections".into()));
    assert!(args.contains(&"--strip-all".into()));
}

#[test]
fn test_wasm_linker_export_symbols() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.export_symbols(&["foo".to_string(), "bar".to_string()]);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--export=foo".into()));
    assert!(args.contains(&"--export=bar".into()));
}

#[test]
fn test_wasm_linker_library_path() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.add_library_path(Path::new("/usr/lib/wasm"));
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"-L".into()));
    assert!(args.contains(&"/usr/lib/wasm".into()));
}

#[test]
fn test_wasm_linker_link_library() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.link_library("c", LibraryKind::Static);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"-lc".into()));
}

#[test]
fn test_wasm_linker_custom_entry() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.set_entry("main");
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--entry=main".into()));
}

#[test]
fn test_wasm_linker_no_entry() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.no_entry();
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--no-entry".into()));
}

#[test]
fn test_wasm_linker_allow_undefined() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.allow_undefined(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--allow-undefined".into()));
}

#[test]
fn test_wasm_linker_apply_config() {
    use crate::aot::wasm::{WasmFeatures, WasmMemoryConfig, WasmStackConfig};

    let target = test_target();
    let mut linker = WasmLinker::new(&target);

    let config = WasmConfig {
        memory: WasmMemoryConfig::default().with_initial_pages(32),
        stack: WasmStackConfig::default().with_size_kb(256),
        features: WasmFeatures {
            bulk_memory: true,
            simd: true,
            ..WasmFeatures::default()
        },
        ..WasmConfig::default()
    };

    linker.apply_config(&config);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

    // Check memory config was applied
    assert!(args.iter().any(|a| a.contains("--initial-memory=")));
    assert!(args.iter().any(|a| a.contains("--stack-size=")));
    assert!(args.contains(&"--enable-bulk-memory".into()));
    assert!(args.contains(&"--enable-simd".into()));
}

#[test]
fn test_wasm_linker_verbose() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.verbose(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--verbose".into()));
}

#[test]
fn test_wasm_linker_shared_memory() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.shared_memory(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--shared-memory".into()));
}

#[test]
fn test_wasm_linker_exception_handling() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.enable_exception_handling(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--enable-exception-handling".into()));
}

#[test]
fn test_wasm_linker_reference_types() {
    let target = test_target();
    let mut linker = WasmLinker::new(&target);
    linker.enable_reference_types(true);
    let cmd = linker.finalize();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    assert!(args.contains(&"--enable-reference-types".into()));
}
