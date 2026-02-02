//! WebAssembly linker tests.
//!
//! Tests for `WasmLinker` - the WebAssembly linker driver using wasm-ld.

#[cfg(feature = "llvm")]
mod tests {
    use std::path::Path;

    use ori_llvm::aot::linker::{LibraryKind, LinkOutput, WasmLinker};
    use ori_llvm::aot::target::{TargetConfig, TargetTripleComponents};

    // -- Test helpers --

    fn test_target_wasm() -> TargetConfig {
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        TargetConfig::from_components(components)
    }

    // -- Basic tests --

    #[test]
    fn test_wasm_linker_basic() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.set_output(Path::new("output.wasm"));
        linker.add_object(Path::new("main.o"));
        linker.set_output_kind(LinkOutput::Executable);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"output.wasm".into()));
        assert!(args.contains(&"--entry=_start".into()));
    }

    #[test]
    fn test_wasm_linker_no_entry() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.set_output_kind(LinkOutput::SharedLibrary);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--no-entry".into()));
        assert!(args.contains(&"--export-dynamic".into()));
    }

    #[test]
    fn test_wasm_linker_gc_sections() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.gc_sections(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--gc-sections".into()));
    }

    #[test]
    fn test_wasm_linker_strip_symbols() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.strip_symbols(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--strip-all".into()));
    }

    #[test]
    fn test_wasm_linker_library_path() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.add_library_path(Path::new("/wasm/lib"));

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-L".into()));
        assert!(args.contains(&"/wasm/lib".into()));
    }

    #[test]
    fn test_wasm_linker_link_library() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        // WASM ignores library kind - always static
        linker.link_library("c", LibraryKind::Dynamic);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-lc".into()));
    }

    #[test]
    fn test_wasm_linker_add_arg() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.add_arg("--verbose");
        linker.link_arg("--allow-undefined");

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--verbose".into()));
        assert!(args.contains(&"--allow-undefined".into()));
    }

    #[test]
    fn test_wasm_linker_target_accessor() {
        let target = test_target_wasm();
        let linker = WasmLinker::new(&target);

        assert!(linker.target().is_wasm());
    }

    #[test]
    fn test_wasm_linker_static_library_kind() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        // Even when specifying static, wasm just uses -l
        linker.link_library("wasi", LibraryKind::Static);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-lwasi".into()));
    }

    #[test]
    fn test_wasm_export_symbols() {
        let target = test_target_wasm();
        let mut linker = WasmLinker::new(&target);

        linker.export_symbols(&["main".to_string(), "malloc".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--export=main".into()));
        assert!(args.contains(&"--export=malloc".into()));
    }
}
