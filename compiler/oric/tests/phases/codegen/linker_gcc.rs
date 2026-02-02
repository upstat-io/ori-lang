//! GCC/Clang linker tests.
//!
//! Tests for `GccLinker` - the Unix linker driver used on Linux and macOS.

#[cfg(feature = "llvm")]
mod tests {
    use std::path::Path;

    use ori_llvm::aot::linker::{GccLinker, LibraryKind, LinkOutput};
    use ori_llvm::aot::target::{TargetConfig, TargetTripleComponents};

    // -- Test helpers --

    fn test_target() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        TargetConfig::from_components(components)
    }

    fn test_target_macos() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
        TargetConfig::from_components(components)
    }

    // -- Basic tests --

    #[test]
    fn test_gcc_linker_basic() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.set_output(Path::new("output"));
        linker.add_object(Path::new("main.o"));
        linker.add_library_path(Path::new("/usr/lib"));
        linker.link_library("c", LibraryKind::Dynamic);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"output".into()));
        assert!(args.contains(&"main.o".into()));
        assert!(args.contains(&"-L".into()));
        assert!(args.contains(&"-lc".into()));
    }

    #[test]
    fn test_gcc_linker_shared() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.set_output_kind(LinkOutput::SharedLibrary);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-shared".into()));
        assert!(args.contains(&"-fPIC".into()));
    }

    #[test]
    fn test_gcc_linker_gc_sections() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.gc_sections(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,--gc-sections".into()));
    }

    #[test]
    fn test_gcc_linker_macos_gc_sections() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        linker.gc_sections(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,-dead_strip".into()));
    }

    #[test]
    fn test_gcc_linker_with_custom_path() {
        let target = test_target();
        let linker = GccLinker::with_path(&target, "/usr/bin/gcc-12");

        let cmd = linker.finalize();
        assert_eq!(cmd.get_program().to_string_lossy(), "/usr/bin/gcc-12");
    }

    #[test]
    fn test_gcc_linker_strip_symbols() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.strip_symbols(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,--strip-all".into()));
    }

    #[test]
    fn test_gcc_linker_macos_strip_symbols() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        linker.strip_symbols(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,-S".into()));
    }

    #[test]
    fn test_gcc_linker_pie() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.set_output_kind(LinkOutput::PositionIndependentExecutable);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-pie".into()));
        assert!(args.contains(&"-fPIE".into()));
    }

    #[test]
    fn test_gcc_linker_macos_shared() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        linker.set_output_kind(LinkOutput::SharedLibrary);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-shared".into()));
        assert!(args.contains(&"-dynamiclib".into()));
    }

    #[test]
    fn test_gcc_linker_link_arg() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.link_arg("--as-needed");

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,--as-needed".into()));
    }

    #[test]
    fn test_gcc_linker_add_arg() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.add_arg("-v");

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-v".into()));
    }

    #[test]
    fn test_gcc_linker_unspecified_library() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.link_library("m", LibraryKind::Unspecified);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-lm".into()));
        // Should not have -Bstatic or -Bdynamic
        assert!(!args.iter().any(|a| a.contains("-Bstatic")));
    }

    #[test]
    fn test_gcc_linker_macos_static_library() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        // macOS doesn't use -Bstatic, just -l
        linker.link_library("ssl", LibraryKind::Static);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-lssl".into()));
        // macOS should not have -Bstatic
        assert!(!args.iter().any(|a| a.contains("-Bstatic")));
    }

    #[test]
    fn test_gcc_linker_static_library_output() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        // Static library output is a no-op (handled by ar)
        linker.set_output_kind(LinkOutput::StaticLibrary);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Should not have -shared or other flags
        assert!(!args.contains(&"-shared".into()));
    }

    #[test]
    fn test_gcc_linker_gc_sections_disabled() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.gc_sections(false);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(!args.iter().any(|a| a.contains("gc-sections")));
    }

    #[test]
    fn test_gcc_linker_strip_disabled() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.strip_symbols(false);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(!args.iter().any(|a| a.contains("strip")));
    }

    #[test]
    fn test_gcc_linker_empty_export_symbols() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.export_symbols(&[]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Empty export should not add --export-dynamic
        assert!(!args.iter().any(|a| a.contains("export")));
    }

    #[test]
    fn test_gcc_linker_target_accessor() {
        let target = test_target();
        let linker = GccLinker::new(&target);

        assert!(linker.target().is_linux());
    }

    #[test]
    fn test_gcc_linker_cmd_accessor() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        // Access cmd directly and add arg
        linker.cmd().arg("--help");

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--help".into()));
    }

    // -- Static/dynamic hint switching tests --

    #[test]
    fn test_static_hint_switching() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        // Start dynamic (default)
        linker.link_library("a", LibraryKind::Dynamic);
        // Switch to static
        linker.link_library("b", LibraryKind::Static);
        // Back to dynamic (automatic reset)
        linker.link_library("c", LibraryKind::Dynamic);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Should have -Bstatic before b and -Bdynamic after
        let bstatic_pos = args.iter().position(|a| a.contains("-Bstatic"));
        let bdynamic_pos = args.iter().position(|a| a.contains("-Bdynamic"));

        assert!(bstatic_pos.is_some());
        assert!(bdynamic_pos.is_some());
        assert!(bstatic_pos < bdynamic_pos);
    }

    #[test]
    fn test_gcc_linker_multiple_static_libraries() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        // Each static lib resets to dynamic after, so consecutive statics
        // each get their own -Bstatic/-Bdynamic bracket
        linker.link_library("a", LibraryKind::Static);
        linker.link_library("b", LibraryKind::Static);
        linker.link_library("c", LibraryKind::Dynamic);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Each static library triggers hint_static + hint_dynamic
        let bstatic_count = args.iter().filter(|a| a.contains("-Bstatic")).count();
        let bdynamic_count = args.iter().filter(|a| a.contains("-Bdynamic")).count();

        // Two static libs = two -Bstatic, two -Bdynamic
        assert_eq!(bstatic_count, 2);
        assert_eq!(bdynamic_count, 2);

        // Libraries should be in order
        assert!(args.contains(&"-la".into()));
        assert!(args.contains(&"-lb".into()));
        assert!(args.contains(&"-lc".into()));
    }

    // -- Export symbols tests --

    #[test]
    fn test_export_symbols_linux() {
        let target = test_target();
        let mut linker = GccLinker::new(&target);

        linker.export_symbols(&["foo".to_string(), "bar".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Wl,--export-dynamic".into()));
    }

    #[test]
    fn test_export_symbols_macos() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        linker.export_symbols(&["foo".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args
            .iter()
            .any(|a| a.contains("-exported_symbol") && a.contains("_foo")));
    }

    #[test]
    fn test_export_symbols_macos_multiple() {
        let target = test_target_macos();
        let mut linker = GccLinker::new(&target);

        linker.export_symbols(&["foo".to_string(), "bar".to_string(), "baz".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Should have individual -exported_symbol for each
        let export_count = args
            .iter()
            .filter(|a| a.contains("exported_symbol"))
            .count();
        assert_eq!(export_count, 3);
    }
}
