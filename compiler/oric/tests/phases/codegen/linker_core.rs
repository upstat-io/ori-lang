//! Core linker infrastructure tests.
//!
//! Tests for `LinkerFlavor`, `LinkOutput`, `LinkLibrary`, `LinkerError`,
//! `LinkerDriver`, `LinkerDetection`, and `LinkInput`.

#[cfg(feature = "llvm")]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use ori_llvm::aot::linker::{
        LibraryKind, LinkInput, LinkLibrary, LinkOutput, LinkerDetection, LinkerDriver,
        LinkerError, LinkerFlavor,
    };
    use ori_llvm::aot::target::{TargetConfig, TargetTripleComponents};

    // -- Test helpers --

    fn test_target() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        TargetConfig::from_components(components)
    }

    #[allow(
        dead_code,
        reason = "test helper for Windows GNU target, not used on all platforms"
    )]
    fn test_target_windows_gnu() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
        TargetConfig::from_components(components)
    }

    fn test_target_wasm() -> TargetConfig {
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        TargetConfig::from_components(components)
    }

    // -- LinkerFlavor tests --

    #[test]
    fn test_linker_flavor_for_target() {
        let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(LinkerFlavor::for_target(&linux), LinkerFlavor::Gcc);

        let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
        assert_eq!(LinkerFlavor::for_target(&macos), LinkerFlavor::Gcc);

        let windows = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(LinkerFlavor::for_target(&windows), LinkerFlavor::Msvc);

        let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        assert_eq!(LinkerFlavor::for_target(&wasm), LinkerFlavor::WasmLd);
    }

    #[test]
    fn test_linker_flavor_for_windows_gnu() {
        let windows_gnu = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
        // Windows GNU uses GCC, not MSVC
        assert_eq!(LinkerFlavor::for_target(&windows_gnu), LinkerFlavor::Gcc);
    }

    #[test]
    fn test_linker_flavor_hash_eq() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LinkerFlavor::Gcc);
        set.insert(LinkerFlavor::Lld);
        set.insert(LinkerFlavor::Msvc);
        set.insert(LinkerFlavor::WasmLd);

        assert_eq!(set.len(), 4);
        assert!(set.contains(&LinkerFlavor::Gcc));
    }

    // -- LinkOutput tests --

    #[test]
    fn test_link_output_extension() {
        let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(LinkOutput::Executable.extension(&linux), "");
        assert_eq!(LinkOutput::SharedLibrary.extension(&linux), "so");
        assert_eq!(LinkOutput::StaticLibrary.extension(&linux), "a");

        let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
        assert_eq!(LinkOutput::SharedLibrary.extension(&macos), "dylib");

        let windows = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(LinkOutput::Executable.extension(&windows), "exe");
        assert_eq!(LinkOutput::SharedLibrary.extension(&windows), "dll");
        assert_eq!(LinkOutput::StaticLibrary.extension(&windows), "lib");
    }

    #[test]
    fn test_link_output_pie_extension() {
        let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(
            LinkOutput::PositionIndependentExecutable.extension(&linux),
            ""
        );

        let windows = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(
            LinkOutput::PositionIndependentExecutable.extension(&windows),
            "exe"
        );
    }

    // -- LinkLibrary tests --

    #[test]
    fn test_link_library_builder() {
        let lib = LinkLibrary::new("foo")
            .static_lib()
            .with_search_path("/usr/lib");

        assert_eq!(lib.name, "foo");
        assert_eq!(lib.kind, LibraryKind::Static);
        assert_eq!(lib.search_path, Some(PathBuf::from("/usr/lib")));
    }

    #[test]
    fn test_link_library_dynamic() {
        let lib = LinkLibrary::new("ssl").dynamic_lib();
        assert_eq!(lib.name, "ssl");
        assert_eq!(lib.kind, LibraryKind::Dynamic);
        assert!(lib.search_path.is_none());
    }

    #[test]
    fn test_link_library_default_kind() {
        let lib = LinkLibrary::new("crypto");
        assert_eq!(lib.kind, LibraryKind::Unspecified);
    }

    // -- LinkerError tests --

    #[test]
    fn test_linker_error_display() {
        let err = LinkerError::LinkerNotFound {
            linker: "cc".to_string(),
            message: "not found".to_string(),
        };
        assert!(err.to_string().contains("cc"));
        assert!(err.to_string().contains("not found"));

        let err = LinkerError::LinkFailed {
            linker: "cc".to_string(),
            exit_code: Some(1),
            stderr: "undefined reference".to_string(),
            command: "cc -o output".to_string(),
        };
        assert!(err.to_string().contains("exit code 1"));
        assert!(err.to_string().contains("undefined reference"));

        let err = LinkerError::InvalidConfig {
            message: "no objects".to_string(),
        };
        assert!(err.to_string().contains("no objects"));
    }

    #[test]
    fn test_linker_error_display_all_variants() {
        // ResponseFileError
        let err = LinkerError::ResponseFileError {
            path: "/tmp/link.rsp".to_string(),
            message: "permission denied".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("response file"));
        assert!(display.contains("/tmp/link.rsp"));
        assert!(display.contains("permission denied"));

        // IoError
        let err = LinkerError::IoError {
            message: "broken pipe".to_string(),
        };
        assert!(err.to_string().contains("I/O error"));
        assert!(err.to_string().contains("broken pipe"));

        // UnsupportedTarget
        let err = LinkerError::UnsupportedTarget {
            triple: "riscv64-unknown-linux-gnu".to_string(),
        };
        assert!(err.to_string().contains("unsupported target"));
        assert!(err.to_string().contains("riscv64"));

        // LinkFailed without exit code
        let err = LinkerError::LinkFailed {
            linker: "ld".to_string(),
            exit_code: None,
            stderr: "error".to_string(),
            command: "ld -o out".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("ld"));
        assert!(!display.contains("exit code")); // No exit code shown
    }

    #[test]
    fn test_linker_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(LinkerError::IoError {
            message: "test".to_string(),
        });
        assert!(err.to_string().contains("test"));
    }

    // -- LinkInput tests --

    #[test]
    fn test_link_input_default() {
        let input = LinkInput::default();
        assert!(input.objects.is_empty());
        assert_eq!(input.output_kind, LinkOutput::Executable);
        assert!(!input.lto);
        assert!(!input.strip);
    }

    #[test]
    fn test_link_input_with_all_fields() {
        let input = LinkInput {
            objects: vec![PathBuf::from("a.o")],
            output: PathBuf::from("out"),
            output_kind: LinkOutput::SharedLibrary,
            libraries: vec![LinkLibrary::new("c")],
            library_paths: vec![PathBuf::from("/lib")],
            exported_symbols: vec!["sym".to_string()],
            lto: true,
            strip: true,
            gc_sections: true,
            extra_args: vec!["-v".to_string()],
            linker: Some(LinkerFlavor::Lld),
        };

        assert_eq!(input.objects.len(), 1);
        assert_eq!(input.output_kind, LinkOutput::SharedLibrary);
        assert!(input.lto);
        assert!(input.strip);
        assert!(input.gc_sections);
        assert_eq!(input.linker, Some(LinkerFlavor::Lld));
    }

    // -- LinkerDriver tests --

    #[test]
    fn test_linker_driver_new() {
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        // Just verify it doesn't panic
        let _ = driver;
    }

    #[test]
    fn test_linker_driver_invalid_input() {
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        let result = driver.link(&LinkInput::default());
        assert!(matches!(result, Err(LinkerError::InvalidConfig { .. })));
    }

    #[test]
    fn test_linker_driver_should_retry() {
        // Test retryable patterns
        assert!(LinkerDriver::should_retry("unrecognized option '-no-pie'"));
        assert!(LinkerDriver::should_retry("unknown option: -static-pie"));
        assert!(LinkerDriver::should_retry("-fuse-ld=lld not found"));

        // Non-retryable errors
        assert!(!LinkerDriver::should_retry("undefined reference to 'foo'"));
        assert!(!LinkerDriver::should_retry("cannot find -lssl"));
    }

    #[test]
    fn test_linker_driver_lld_flavor_linux() {
        // When using LLD on Linux, should use clang with -fuse-ld=lld
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        let input = LinkInput {
            objects: vec![PathBuf::from("main.o")],
            output: PathBuf::from("output"),
            linker: Some(LinkerFlavor::Lld),
            ..Default::default()
        };

        // We can't actually run the linker successfully (no valid object file),
        // but we verify the driver doesn't panic with valid input.
        // Result depends on environment: LinkerNotFound if lld missing,
        // LinkFailed if lld exists but object file invalid.
        let result = driver.link(&input);

        // Accept any error - we're testing the driver setup, not the link result
        assert!(
            result.is_err(),
            "Expected link to fail with fake object file, got Ok"
        );
    }

    #[test]
    fn test_linker_driver_wasm_flavor() {
        let target = test_target_wasm();
        let driver = LinkerDriver::new(&target);

        let input = LinkInput {
            objects: vec![PathBuf::from("main.o")],
            output: PathBuf::from("output.wasm"),
            ..Default::default()
        };

        // Will fail because wasm-ld not found, but verifies setup
        let result = driver.link(&input);
        assert!(result.is_err());
    }

    // -- LinkerDetection tests --

    #[test]
    fn test_linker_detection_default() {
        let detection = LinkerDetection::default();
        assert!(detection.available.is_empty());
        assert!(detection.not_found.is_empty());
        assert!(detection.preferred().is_none());
    }

    #[test]
    fn test_linker_detection_preferred() {
        let mut detection = LinkerDetection::default();
        detection.available.push(LinkerFlavor::Lld);
        detection.available.push(LinkerFlavor::Gcc);

        assert_eq!(detection.preferred(), Some(LinkerFlavor::Lld));
    }

    // -- Response file tests --

    #[test]
    fn test_create_response_file() {
        let mut cmd = Command::new("cc");
        cmd.arg("-o").arg("output");
        cmd.arg("file1.o").arg("file2.o");
        cmd.arg("-lm");

        let result = LinkerDriver::create_response_file(&cmd);
        assert!(result.is_ok());

        let new_cmd = result.unwrap();
        let args: Vec<_> = new_cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Should have a single @response_file argument
        assert_eq!(args.len(), 1);
        assert!(args[0].starts_with('@'));
    }
}
