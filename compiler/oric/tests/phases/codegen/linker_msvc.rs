//! MSVC linker tests.
//!
//! Tests for `MsvcLinker` - the Windows linker driver for MSVC toolchain.

#[cfg(feature = "llvm")]
mod tests {
    use std::path::Path;

    use ori_llvm::aot::linker::{LibraryKind, LinkOutput, MsvcLinker};
    use ori_llvm::aot::target::{TargetConfig, TargetTripleComponents};

    // -- Test helpers --

    fn test_target_windows() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        TargetConfig::from_components(components)
    }

    // -- Basic tests --

    #[test]
    fn test_msvc_linker_basic() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.set_output(Path::new("output.exe"));
        linker.add_object(Path::new("main.obj"));
        linker.link_library("kernel32", LibraryKind::Dynamic);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.iter().any(|a| a.starts_with("/OUT:")));
        assert!(args.contains(&"main.obj".into()));
        assert!(args.contains(&"kernel32.lib".into()));
    }

    #[test]
    fn test_msvc_linker_gc_sections() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.gc_sections(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/OPT:REF".into()));
        assert!(args.contains(&"/OPT:ICF".into()));
    }

    #[test]
    fn test_msvc_linker_with_lld() {
        let target = test_target_windows();
        let linker = MsvcLinker::with_lld(&target);

        let cmd = linker.finalize();
        assert_eq!(cmd.get_program().to_string_lossy(), "lld-link");

        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"/nologo".into()));
    }

    #[test]
    fn test_msvc_linker_strip_symbols() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.strip_symbols(true);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/DEBUG:NONE".into()));
    }

    #[test]
    fn test_msvc_linker_dll() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.set_output_kind(LinkOutput::SharedLibrary);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/DLL".into()));
    }

    #[test]
    fn test_msvc_linker_library_path() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.add_library_path(Path::new("C:\\libs"));

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.iter().any(|a| a.starts_with("/LIBPATH:")));
    }

    #[test]
    fn test_msvc_linker_link_arg() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.link_arg("/VERBOSE");

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/VERBOSE".into()));
    }

    #[test]
    fn test_msvc_linker_target_accessor() {
        let target = test_target_windows();
        let linker = MsvcLinker::new(&target);

        assert!(linker.target().is_windows());
    }

    #[test]
    fn test_msvc_linker_pie_same_as_exe() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        // PIE on Windows is just a regular executable
        linker.set_output_kind(LinkOutput::PositionIndependentExecutable);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/SUBSYSTEM:CONSOLE".into()));
    }

    #[test]
    fn test_export_symbols_msvc() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.export_symbols(&["foo".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/EXPORT:foo".into()));
    }
}
