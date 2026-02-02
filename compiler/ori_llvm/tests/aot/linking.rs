//! Linker Integration Tests
//!
//! Test scenarios inspired by:
//! - Rust: `tests/run-make/native-link-modifier-whole-archive/`
//! - Rust: `tests/run-make/c-link-to-rust-*/`
//! - Rust: `tests/run-make/link-args-order/`
//! - Zig: `test/link/static_libs_from_object_files/`
//!
//! These tests verify:
//! - Static vs dynamic library linking
//! - Symbol visibility and export control
//! - Library search path handling
//! - Platform-specific linker behavior
//! - Response file generation for long command lines

use std::path::{Path, PathBuf};
use std::process::Command;

use ori_llvm::aot::linker::{
    GccLinker, LibraryKind, LinkInput, LinkLibrary, LinkOutput, LinkerDetection, LinkerDriver,
    LinkerError, LinkerFlavor, LinkerImpl, MsvcLinker, WasmLinker,
};

use super::util::{
    command_args, linux_target, macos_arm_target, macos_target, wasm32_target, windows_gnu_target,
    windows_msvc_target,
};
use crate::assert_command_args;

// ============================================================================
// Linker Flavor Detection Tests
// ============================================================================

/// Test: Correct linker flavor for each target
///
/// Scenario: Auto-detection of linker based on target triple.
#[test]
fn test_linker_flavor_for_target() {
    use ori_llvm::aot::TargetTripleComponents;

    // Linux uses GCC-style
    let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(LinkerFlavor::for_target(&linux), LinkerFlavor::Gcc);

    // macOS uses GCC-style (clang)
    let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    assert_eq!(LinkerFlavor::for_target(&macos), LinkerFlavor::Gcc);

    // Windows MSVC uses MSVC linker
    let windows_msvc = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(LinkerFlavor::for_target(&windows_msvc), LinkerFlavor::Msvc);

    // Windows GNU uses GCC-style (mingw)
    let windows_gnu = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    assert_eq!(LinkerFlavor::for_target(&windows_gnu), LinkerFlavor::Gcc);

    // WASM uses wasm-ld
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(LinkerFlavor::for_target(&wasm), LinkerFlavor::WasmLd);
}

// ============================================================================
// Output Type Extension Tests
// ============================================================================

/// Test: Correct file extensions per platform
///
/// Scenario from Rust `emit`:
/// Output files have correct platform extensions.
#[test]
fn test_link_output_extensions() {
    use ori_llvm::aot::TargetTripleComponents;

    // Linux
    let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(LinkOutput::Executable.extension(&linux), "");
    assert_eq!(LinkOutput::SharedLibrary.extension(&linux), "so");
    assert_eq!(LinkOutput::StaticLibrary.extension(&linux), "a");
    assert_eq!(
        LinkOutput::PositionIndependentExecutable.extension(&linux),
        ""
    );

    // macOS
    let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    assert_eq!(LinkOutput::Executable.extension(&macos), "");
    assert_eq!(LinkOutput::SharedLibrary.extension(&macos), "dylib");
    assert_eq!(LinkOutput::StaticLibrary.extension(&macos), "a");

    // Windows
    let windows = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(LinkOutput::Executable.extension(&windows), "exe");
    assert_eq!(LinkOutput::SharedLibrary.extension(&windows), "dll");
    assert_eq!(LinkOutput::StaticLibrary.extension(&windows), "lib");
}

// ============================================================================
// GCC Linker Tests (Linux/macOS)
// ============================================================================

/// Test: Basic GCC linker command
///
/// Scenario: Simple executable linking.
#[test]
fn test_gcc_linker_basic() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.set_output(Path::new("output"));
    linker.add_object(Path::new("main.o"));
    linker.add_object(Path::new("lib.o"));
    linker.add_library_path(Path::new("/usr/lib"));
    linker.link_library("m", LibraryKind::Dynamic);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-o", "output", "main.o", "lib.o", "-L", "-lm");
}

/// Test: GCC linker shared library output
///
/// Scenario from Rust `cdylib`:
/// Creating a shared library (.so).
#[test]
fn test_gcc_linker_shared_library() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.set_output_kind(LinkOutput::SharedLibrary);
    linker.set_output(Path::new("libfoo.so"));

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-shared", "-fPIC");
}

/// Test: GCC linker PIE executable
///
/// Scenario: Position-independent executable for ASLR.
#[test]
fn test_gcc_linker_pie() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.set_output_kind(LinkOutput::PositionIndependentExecutable);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-pie", "-fPIE");
}

/// Test: GCC linker static library hint switching
///
/// Scenario from Rust `native-link-modifier-whole-archive`:
/// -Bstatic/-Bdynamic hints for mixed linking.
#[test]
fn test_gcc_linker_static_dynamic_hints() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    // Dynamic library
    linker.link_library("a", LibraryKind::Dynamic);
    // Switch to static
    linker.link_library("b", LibraryKind::Static);
    // Back to dynamic
    linker.link_library("c", LibraryKind::Dynamic);

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    // Should have -Bstatic before -lb and -Bdynamic after
    let bstatic_pos = args.iter().position(|a| a.contains("-Bstatic"));
    let lb_pos = args.iter().position(|a| a == "-lb");
    let bdynamic_pos = args.iter().position(|a| a.contains("-Bdynamic"));

    assert!(bstatic_pos.is_some(), "Missing -Bstatic");
    assert!(lb_pos.is_some(), "Missing -lb");
    assert!(bdynamic_pos.is_some(), "Missing -Bdynamic");
    assert!(bstatic_pos < lb_pos, "-Bstatic should come before -lb");
    assert!(lb_pos < bdynamic_pos, "-lb should come before -Bdynamic");
}

/// Test: GCC linker garbage collection
///
/// Scenario: Remove unused sections for smaller binaries.
#[test]
fn test_gcc_linker_gc_sections_linux() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.gc_sections(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-Wl,--gc-sections");
}

/// Test: macOS GCC linker garbage collection
///
/// Scenario: macOS uses -dead_strip instead of --gc-sections.
#[test]
fn test_gcc_linker_gc_sections_macos() {
    let target = macos_target();
    let mut linker = GccLinker::new(&target);

    linker.gc_sections(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-Wl,-dead_strip");
}

/// Test: GCC linker strip symbols
///
/// Scenario: Strip debug info for release builds.
#[test]
fn test_gcc_linker_strip_symbols_linux() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.strip_symbols(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-Wl,--strip-all");
}

/// Test: macOS GCC linker strip symbols
///
/// Scenario: macOS uses -S for stripping.
#[test]
fn test_gcc_linker_strip_symbols_macos() {
    let target = macos_target();
    let mut linker = GccLinker::new(&target);

    linker.strip_symbols(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-Wl,-S");
}

/// Test: GCC linker export symbols Linux
///
/// Scenario from Rust `link-args-order`:
/// Export dynamic symbols for plugins.
#[test]
fn test_gcc_linker_export_symbols_linux() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.export_symbols(&["foo".to_string(), "bar".to_string()]);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-Wl,--export-dynamic");
}

/// Test: macOS GCC linker export symbols
///
/// Scenario: macOS uses -exported_symbol.
#[test]
fn test_gcc_linker_export_symbols_macos() {
    let target = macos_target();
    let mut linker = GccLinker::new(&target);

    linker.export_symbols(&["foo".to_string(), "bar".to_string()]);

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    // Should have individual -exported_symbol for each (with _ prefix)
    assert!(args
        .iter()
        .any(|a| a.contains("-exported_symbol") && a.contains("_foo")));
    assert!(args
        .iter()
        .any(|a| a.contains("-exported_symbol") && a.contains("_bar")));
}

/// Test: macOS shared library flags
///
/// Scenario: macOS uses -dynamiclib for shared libs.
#[test]
fn test_gcc_linker_macos_shared() {
    let target = macos_target();
    let mut linker = GccLinker::new(&target);

    linker.set_output_kind(LinkOutput::SharedLibrary);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-shared", "-dynamiclib");
}

/// Test: GCC linker with custom path
///
/// Scenario: Using a specific compiler version.
#[test]
fn test_gcc_linker_custom_path() {
    let target = linux_target();
    let linker = GccLinker::with_path(&target, "/usr/bin/gcc-12");

    let cmd = linker.finalize();
    assert_eq!(cmd.get_program().to_string_lossy(), "/usr/bin/gcc-12");
}

/// Test: GCC linker raw arguments
///
/// Scenario: Pass-through linker arguments.
#[test]
fn test_gcc_linker_raw_args() {
    let target = linux_target();
    let mut linker = GccLinker::new(&target);

    linker.add_arg("-v");
    linker.link_arg("--as-needed");

    let cmd = linker.finalize();
    assert_command_args!(cmd, "-v", "-Wl,--as-needed");
}

// ============================================================================
// MSVC Linker Tests (Windows)
// ============================================================================

/// Test: Basic MSVC linker command
///
/// Scenario: Windows executable linking.
#[test]
fn test_msvc_linker_basic() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.set_output(Path::new("output.exe"));
    linker.add_object(Path::new("main.obj"));
    linker.link_library("kernel32", LibraryKind::Dynamic);

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    assert!(args.iter().any(|a| a.starts_with("/OUT:")));
    assert!(args.contains(&"main.obj".to_string()));
    assert!(args.contains(&"kernel32.lib".to_string()));
    // Note: /nologo is only added with with_lld(), not with new()
}

/// Test: MSVC linker DLL output
///
/// Scenario: Creating a Windows DLL.
#[test]
fn test_msvc_linker_dll() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.set_output_kind(LinkOutput::SharedLibrary);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "/DLL");
}

/// Test: MSVC linker GC sections
///
/// Scenario: Remove unreferenced code.
#[test]
fn test_msvc_linker_gc_sections() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.gc_sections(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "/OPT:REF", "/OPT:ICF");
}

/// Test: MSVC linker strip symbols
///
/// Scenario: Disable debug info in output.
#[test]
fn test_msvc_linker_strip_symbols() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.strip_symbols(true);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "/DEBUG:NONE");
}

/// Test: MSVC linker export symbols
///
/// Scenario: DLL exports.
#[test]
fn test_msvc_linker_export_symbols() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.export_symbols(&["foo".to_string(), "bar".to_string()]);

    let cmd = linker.finalize();
    assert_command_args!(cmd, "/EXPORT:foo", "/EXPORT:bar");
}

/// Test: MSVC linker library path
///
/// Scenario: Additional library search directories.
#[test]
fn test_msvc_linker_library_path() {
    let target = windows_msvc_target();
    let mut linker = MsvcLinker::new(&target);

    linker.add_library_path(Path::new("C:\\libs"));

    let cmd = linker.finalize();
    let args = command_args(&cmd);
    assert!(args.iter().any(|a| a.starts_with("/LIBPATH:")));
}

/// Test: MSVC linker with LLD
///
/// Scenario: Using lld-link for faster linking.
#[test]
fn test_msvc_linker_with_lld() {
    let target = windows_msvc_target();
    let linker = MsvcLinker::with_lld(&target);

    let cmd = linker.finalize();
    assert_eq!(cmd.get_program().to_string_lossy(), "lld-link");
}

// ============================================================================
// Linker Driver Tests
// ============================================================================

/// Test: Linker driver invalid input
///
/// Scenario: Empty object list should fail.
#[test]
fn test_linker_driver_invalid_input() {
    let target = linux_target();
    let driver = LinkerDriver::new(&target);

    let input = LinkInput::default();
    let result = driver.link(&input);

    assert!(matches!(result, Err(LinkerError::InvalidConfig { .. })));
}

/// Test: Linker driver configures all options
///
/// Scenario: Full configuration is applied to linker.
#[test]
fn test_linker_driver_full_config() {
    let target = linux_target();
    let mut linker = LinkerImpl::Gcc(GccLinker::new(&target));

    let input = LinkInput {
        objects: vec![PathBuf::from("main.o"), PathBuf::from("lib.o")],
        output: PathBuf::from("output"),
        output_kind: LinkOutput::Executable,
        libraries: vec![
            LinkLibrary::new("m"),
            LinkLibrary::new("pthread").static_lib(),
        ],
        library_paths: vec![PathBuf::from("/usr/local/lib")],
        exported_symbols: vec!["main".to_string()],
        gc_sections: true,
        strip: true,
        extra_args: vec!["-v".to_string()],
        ..Default::default()
    };

    LinkerDriver::configure_linker(&mut linker, &input).expect("Configure failed");

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    // Objects
    assert!(args.contains(&"main.o".to_string()));
    assert!(args.contains(&"lib.o".to_string()));

    // Libraries
    assert!(args.contains(&"-lm".to_string()));
    assert!(args.contains(&"-lpthread".to_string()));

    // Library paths
    assert!(args.iter().any(|a| a.contains("/usr/local/lib")));

    // GC sections
    assert!(args.iter().any(|a| a.contains("gc-sections")));

    // Strip
    assert!(args.iter().any(|a| a.contains("strip")));

    // Extra args
    assert!(args.contains(&"-v".to_string()));

    // Output
    assert!(args.contains(&"-o".to_string()));
    assert!(args.contains(&"output".to_string()));
}

/// Test: Linker driver retry logic detection
///
/// Scenario from Rust linker experience:
/// Detect retryable errors.
#[test]
fn test_linker_driver_retry_detection() {
    // Retryable errors
    assert!(LinkerDriver::should_retry("unrecognized option '-no-pie'"));
    assert!(LinkerDriver::should_retry("unknown option: -static-pie"));
    assert!(LinkerDriver::should_retry("-fuse-ld=lld not found"));

    // Non-retryable errors
    assert!(!LinkerDriver::should_retry("undefined reference to 'foo'"));
    assert!(!LinkerDriver::should_retry("cannot find -lssl"));
    assert!(!LinkerDriver::should_retry("multiple definition of 'bar'"));
}

/// Test: Response file creation
///
/// Scenario from Rust `response-file`:
/// Handle long command lines via response files.
#[test]
fn test_linker_driver_response_file() {
    let mut cmd = Command::new("cc");
    cmd.arg("-o").arg("output");
    cmd.arg("file1.o").arg("file2.o");
    cmd.arg("-lm");

    let result = LinkerDriver::create_response_file(&cmd);
    assert!(result.is_ok());

    let new_cmd = result.unwrap();
    let args = command_args(&new_cmd);

    // Should have a single @response_file argument
    assert_eq!(args.len(), 1);
    assert!(args[0].starts_with('@'));
}

// ============================================================================
// Link Library Builder Tests
// ============================================================================

/// Test: LinkLibrary builder pattern
#[test]
fn test_link_library_builder() {
    // Static library with search path
    let lib = LinkLibrary::new("foo")
        .static_lib()
        .with_search_path("/usr/lib");

    assert_eq!(lib.name, "foo");
    assert_eq!(lib.kind, LibraryKind::Static);
    assert_eq!(lib.search_path, Some(PathBuf::from("/usr/lib")));

    // Dynamic library
    let lib = LinkLibrary::new("bar").dynamic_lib();
    assert_eq!(lib.kind, LibraryKind::Dynamic);

    // Unspecified (default)
    let lib = LinkLibrary::new("baz");
    assert_eq!(lib.kind, LibraryKind::Unspecified);
}

// ============================================================================
// Linker Detection Tests
// ============================================================================

/// Test: Linker detection structure
#[test]
fn test_linker_detection_default() {
    let detection = LinkerDetection::default();
    assert!(detection.available.is_empty());
    assert!(detection.not_found.is_empty());
    assert!(detection.preferred().is_none());
}

/// Test: Linker detection preferred selection
#[test]
fn test_linker_detection_preferred() {
    let mut detection = LinkerDetection::default();
    detection.available.push(LinkerFlavor::Lld);
    detection.available.push(LinkerFlavor::Gcc);

    // First in list is preferred
    assert_eq!(detection.preferred(), Some(LinkerFlavor::Lld));
}

// ============================================================================
// Linker Error Tests
// ============================================================================

/// Test: Linker error display
#[test]
fn test_linker_error_display() {
    // LinkerNotFound
    let err = LinkerError::LinkerNotFound {
        linker: "cc".to_string(),
        message: "not found".to_string(),
    };
    assert!(err.to_string().contains("cc"));
    assert!(err.to_string().contains("not found"));

    // LinkFailed with exit code
    let err = LinkerError::LinkFailed {
        linker: "cc".to_string(),
        exit_code: Some(1),
        stderr: "undefined reference to 'foo'".to_string(),
        command: "cc -o output main.o".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("exit code 1"));
    assert!(msg.contains("undefined reference"));
    assert!(msg.contains("Command:"));

    // LinkFailed without exit code
    let err = LinkerError::LinkFailed {
        linker: "ld".to_string(),
        exit_code: None,
        stderr: "error".to_string(),
        command: "ld -o out".to_string(),
    };
    assert!(!err.to_string().contains("exit code"));

    // ResponseFileError
    let err = LinkerError::ResponseFileError {
        path: "/tmp/link.rsp".to_string(),
        message: "permission denied".to_string(),
    };
    assert!(err.to_string().contains("response file"));
    assert!(err.to_string().contains("/tmp/link.rsp"));

    // InvalidConfig
    let err = LinkerError::InvalidConfig {
        message: "no objects".to_string(),
    };
    assert!(err.to_string().contains("no objects"));

    // IoError
    let err = LinkerError::IoError {
        message: "broken pipe".to_string(),
    };
    assert!(err.to_string().contains("I/O error"));

    // UnsupportedTarget
    let err = LinkerError::UnsupportedTarget {
        triple: "riscv64-unknown-linux-gnu".to_string(),
    };
    assert!(err.to_string().contains("unsupported target"));
    assert!(err.to_string().contains("riscv64"));
}

// ============================================================================
// Cross-Platform Linker Tests
// ============================================================================

/// Test: Windows GNU linker uses GCC style
///
/// Scenario: MinGW toolchain on Windows.
#[test]
fn test_windows_gnu_uses_gcc_linker() {
    let target = windows_gnu_target();
    let linker = GccLinker::new(&target);

    // Should create command successfully
    let cmd = linker.finalize();
    assert!(cmd.get_program().to_string_lossy().len() > 0);
}

/// Test: ARM64 macOS linker
///
/// Scenario: Apple Silicon target.
#[test]
fn test_macos_arm64_linker() {
    let target = macos_arm_target();
    let mut linker = GccLinker::new(&target);

    linker.set_output(Path::new("output"));
    linker.set_output_kind(LinkOutput::Executable);

    let cmd = linker.finalize();
    // Should work the same as x86_64 macOS
    assert_command_args!(cmd, "-o", "output");
}

// ============================================================================
// Library Search Path Tests
// ============================================================================

/// Test: Library with custom search path
///
/// Scenario from Rust `c-static-rlib`:
/// Library-specific search paths.
#[test]
fn test_library_with_search_path() {
    let target = linux_target();
    let mut linker = LinkerImpl::Gcc(GccLinker::new(&target));

    let input = LinkInput {
        objects: vec![PathBuf::from("main.o")],
        output: PathBuf::from("output"),
        libraries: vec![LinkLibrary::new("custom").with_search_path("/opt/custom/lib")],
        ..Default::default()
    };

    LinkerDriver::configure_linker(&mut linker, &input).expect("Configure failed");

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    // Library's search path should be added
    assert!(args.iter().any(|a| a.contains("/opt/custom/lib")));
}

// ============================================================================
// LinkerImpl Enum Dispatch Tests
// ============================================================================

/// Test: LinkerImpl dispatch to correct implementation
#[test]
fn test_linker_impl_dispatch() {
    // GCC
    let target = linux_target();
    let mut linker = LinkerImpl::Gcc(GccLinker::new(&target));
    linker.set_output(Path::new("out_gcc"));
    let cmd = linker.finalize();
    assert!(command_args(&cmd).contains(&"out_gcc".to_string()));

    // MSVC
    let target = windows_msvc_target();
    let mut linker = LinkerImpl::Msvc(MsvcLinker::new(&target));
    linker.set_output(Path::new("out_msvc.exe"));
    let cmd = linker.finalize();
    let args = command_args(&cmd);
    assert!(args.iter().any(|a| a.contains("out_msvc.exe")));

    // WASM
    let target = wasm32_target();
    let mut linker = LinkerImpl::Wasm(WasmLinker::new(&target));
    linker.set_output(Path::new("out_wasm.wasm"));
    let cmd = linker.finalize();
    assert!(command_args(&cmd).contains(&"out_wasm.wasm".to_string()));
}

/// Test: LinkerImpl all methods dispatch correctly
#[test]
fn test_linker_impl_all_methods() {
    let target = linux_target();
    let mut linker = LinkerImpl::Gcc(GccLinker::new(&target));

    // All these should dispatch to the inner GccLinker
    linker.set_output_kind(LinkOutput::Executable);
    linker.add_object(Path::new("test.o"));
    linker.add_library_path(Path::new("/lib"));
    linker.link_library("c", LibraryKind::Dynamic);
    linker.gc_sections(true);
    linker.strip_symbols(true);
    linker.export_symbols(&["main".to_string()]);
    linker.add_arg("-v");

    let cmd = linker.finalize();
    let args = command_args(&cmd);

    assert!(args.contains(&"test.o".to_string()));
    assert!(args.iter().any(|a| a.contains("/lib")));
    assert!(args.contains(&"-lc".to_string()));
    assert!(args.iter().any(|a| a.contains("gc-sections")));
    assert!(args.iter().any(|a| a.contains("strip")));
    assert!(args.contains(&"-v".to_string()));
}
