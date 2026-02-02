//! Linker Driver for AOT Compilation
//!
//! Provides a platform-agnostic interface to system linkers for producing
//! native executables and shared libraries.
//!
//! # Architecture
//!
//! The linker driver uses enum-based dispatch:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    LinkerDriver                         │
//! │  - Orchestrates linking process                         │
//! │  - Selects platform linker                              │
//! │  - Handles response files                               │
//! │  - Retry logic for fallbacks                            │
//! └────────────────────────┬────────────────────────────────┘
//!                          │
//!          ┌───────────────┼───────────────┐
//!          ▼               ▼               ▼
//! ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
//! │  GccLinker  │  │ MsvcLinker  │  │  WasmLinker │
//! │  (Unix)     │  │ (Windows)   │  │  (WASM)     │
//! └─────────────┘  └─────────────┘  └─────────────┘
//! ```
//!
//! # Key Features
//!
//! - **Enum-based dispatch**: Static dispatch with exhaustiveness checking
//! - **Response file support**: Automatic handling of long command lines
//! - **Static/dynamic hints**: Clean API for switching between static and dynamic linking
//! - **Three-tier argument system**: Separates linker args from cc wrapper args
//! - **Error handling with retry**: Graceful fallbacks for missing linker features
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, LinkerDriver, LinkOutput};
//!
//! let target = TargetConfig::native()?;
//! let driver = LinkerDriver::new(&target)?;
//!
//! driver.link(LinkInput {
//!     objects: vec!["main.o".into()],
//!     output: "myapp".into(),
//!     output_kind: LinkOutput::Executable,
//!     libraries: vec!["ori_rt".into()],
//!     ..Default::default()
//! })?;
//! ```

mod gcc;
mod msvc;
mod wasm;

pub use gcc::GccLinker;
pub use msvc::MsvcLinker;
pub use wasm::WasmLinker;

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::aot::target::{TargetConfig, TargetTripleComponents};

// --- Error Types ---

/// Error type for linker operations.
#[derive(Debug, Clone)]
pub enum LinkerError {
    /// Linker executable not found.
    LinkerNotFound { linker: String, message: String },
    /// Linker invocation failed.
    LinkFailed {
        linker: String,
        exit_code: Option<i32>,
        stderr: String,
        command: String,
    },
    /// Failed to create response file.
    ResponseFileError { path: String, message: String },
    /// Invalid linker configuration.
    InvalidConfig { message: String },
    /// I/O error during linking.
    IoError { message: String },
    /// Unsupported target for linking.
    UnsupportedTarget { triple: String },
}

impl fmt::Display for LinkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LinkerNotFound { linker, message } => {
                write!(f, "linker '{linker}' not found: {message}")
            }
            Self::LinkFailed {
                linker,
                exit_code,
                stderr,
                command,
            } => {
                write!(f, "linking with '{linker}' failed")?;
                if let Some(code) = exit_code {
                    write!(f, " (exit code {code})")?;
                }
                if !stderr.is_empty() {
                    write!(f, "\n\nLinker stderr:\n{stderr}")?;
                }
                write!(f, "\n\nCommand: {command}")
            }
            Self::ResponseFileError { path, message } => {
                write!(f, "failed to create response file '{path}': {message}")
            }
            Self::InvalidConfig { message } => {
                write!(f, "invalid linker configuration: {message}")
            }
            Self::IoError { message } => {
                write!(f, "I/O error during linking: {message}")
            }
            Self::UnsupportedTarget { triple } => {
                write!(f, "unsupported target for linking: {triple}")
            }
        }
    }
}

impl std::error::Error for LinkerError {}

// --- Output Types ---

/// Type of output to produce from linking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkOutput {
    /// Standard executable.
    #[default]
    Executable,
    /// Position-independent executable (PIE).
    PositionIndependentExecutable,
    /// Shared library (.so, .dylib, .dll).
    SharedLibrary,
    /// Static library (.a, .lib).
    StaticLibrary,
}

impl LinkOutput {
    /// Get the appropriate file extension for this output type.
    #[must_use]
    pub fn extension(&self, target: &TargetTripleComponents) -> &'static str {
        match (self, target.os.as_str()) {
            (Self::Executable | Self::PositionIndependentExecutable, "windows") => "exe",
            (Self::Executable | Self::PositionIndependentExecutable, _) => "",
            (Self::SharedLibrary, "windows") => "dll",
            (Self::SharedLibrary, "darwin") => "dylib",
            (Self::SharedLibrary, _) => "so",
            (Self::StaticLibrary, "windows") => "lib",
            (Self::StaticLibrary, _) => "a",
        }
    }
}

// --- Library Types ---

/// Kind of library to link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryKind {
    /// Let the linker decide (usually prefers dynamic).
    #[default]
    Unspecified,
    /// Force static linking.
    Static,
    /// Force dynamic linking.
    Dynamic,
}

/// A library to link.
#[derive(Debug, Clone)]
pub struct LinkLibrary {
    /// Library name (without lib prefix or extension).
    pub name: String,
    /// Library kind (static/dynamic/unspecified).
    pub kind: LibraryKind,
    /// Optional search path for this specific library.
    pub search_path: Option<PathBuf>,
}

impl LinkLibrary {
    /// Create a new library reference.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: LibraryKind::Unspecified,
            search_path: None,
        }
    }

    /// Set this library to static linking.
    #[must_use]
    pub fn static_lib(mut self) -> Self {
        self.kind = LibraryKind::Static;
        self
    }

    /// Set this library to dynamic linking.
    #[must_use]
    pub fn dynamic_lib(mut self) -> Self {
        self.kind = LibraryKind::Dynamic;
        self
    }

    /// Set a search path for this library.
    #[must_use]
    pub fn with_search_path(mut self, path: &str) -> Self {
        self.search_path = Some(PathBuf::from(path));
        self
    }
}

// --- Link Input ---

/// Input configuration for the linker.
#[derive(Debug, Clone, Default)]
pub struct LinkInput {
    /// Object files to link.
    pub objects: Vec<PathBuf>,
    /// Output file path.
    pub output: PathBuf,
    /// Type of output to produce.
    pub output_kind: LinkOutput,
    /// Libraries to link.
    pub libraries: Vec<LinkLibrary>,
    /// Library search paths.
    pub library_paths: Vec<PathBuf>,
    /// Symbols to export (for shared libraries).
    pub exported_symbols: Vec<String>,
    /// Enable link-time optimization.
    pub lto: bool,
    /// Strip debug symbols.
    pub strip: bool,
    /// Enable garbage collection of unused sections.
    pub gc_sections: bool,
    /// Additional linker arguments.
    pub extra_args: Vec<String>,
    /// Override the linker flavor.
    pub linker: Option<LinkerFlavor>,
}

// --- Linker Flavor ---

/// Linker flavor/family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkerFlavor {
    /// GNU-compatible (gcc, clang).
    Gcc,
    /// LLVM LLD.
    Lld,
    /// Microsoft Visual C++.
    Msvc,
    /// WebAssembly (wasm-ld).
    WasmLd,
}

impl LinkerFlavor {
    /// Determine the default linker flavor for a target.
    #[must_use]
    pub fn for_target(target: &TargetTripleComponents) -> Self {
        if target.is_wasm() {
            Self::WasmLd
        } else if target.is_windows() && target.env.as_deref() == Some("msvc") {
            Self::Msvc
        } else {
            Self::Gcc
        }
    }
}

// --- Linker Implementation Enum ---

/// Enum-based linker dispatch.
///
/// Uses enum dispatch instead of trait objects for:
/// - Exhaustiveness checking at compile time
/// - Static dispatch (no vtable overhead)
/// - No heap allocation for the linker itself
pub enum LinkerImpl {
    /// GCC/Clang-style linker (Unix).
    Gcc(GccLinker),
    /// MSVC-style linker (Windows).
    Msvc(MsvcLinker),
    /// WebAssembly linker.
    Wasm(WasmLinker),
}

/// Generates forwarding methods for `LinkerImpl` that dispatch to all variants.
///
/// This macro eliminates boilerplate for the enum-based dispatch pattern,
/// where each method simply forwards to the underlying linker implementation.
macro_rules! impl_linker_forward {
    // Methods that take &mut self and return nothing
    (mut $method:ident($($arg:ident: $ty:ty),* $(,)?)) => {
        pub fn $method(&mut self, $($arg: $ty),*) {
            match self {
                Self::Gcc(l) => l.$method($($arg),*),
                Self::Msvc(l) => l.$method($($arg),*),
                Self::Wasm(l) => l.$method($($arg),*),
            }
        }
    };
    // Methods that consume self and return a value
    (self $method:ident() -> $ret:ty) => {
        pub fn $method(self) -> $ret {
            match self {
                Self::Gcc(l) => l.$method(),
                Self::Msvc(l) => l.$method(),
                Self::Wasm(l) => l.$method(),
            }
        }
    };
}

impl LinkerImpl {
    impl_linker_forward!(mut set_output(path: &Path));
    impl_linker_forward!(mut set_output_kind(kind: LinkOutput));
    impl_linker_forward!(mut add_object(path: &Path));
    impl_linker_forward!(mut add_library_path(path: &Path));
    impl_linker_forward!(mut link_library(name: &str, kind: LibraryKind));
    impl_linker_forward!(mut gc_sections(enable: bool));
    impl_linker_forward!(mut strip_symbols(strip: bool));
    impl_linker_forward!(mut export_symbols(symbols: &[String]));
    impl_linker_forward!(mut add_arg(arg: &str));
    impl_linker_forward!(self finalize() -> Command);
}

// --- Linker Driver ---

/// High-level linker driver that orchestrates the linking process.
///
/// The driver:
/// - Selects the appropriate linker for the target
/// - Constructs the linker command line
/// - Handles response files for long command lines
/// - Provides retry logic for missing linker features
pub struct LinkerDriver {
    target: TargetConfig,
}

impl LinkerDriver {
    /// Create a new linker driver for the given target.
    pub fn new(target: &TargetConfig) -> Self {
        Self {
            target: target.clone(),
        }
    }

    /// Link object files into an executable or library.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The linker is not found
    /// - Linking fails
    /// - Response file creation fails
    pub fn link(&self, input: &LinkInput) -> Result<(), LinkerError> {
        // Validate input
        if input.objects.is_empty() {
            return Err(LinkerError::InvalidConfig {
                message: "no object files to link".to_string(),
            });
        }

        // Select linker flavor
        let flavor = input
            .linker
            .unwrap_or_else(|| LinkerFlavor::for_target(self.target.components()));

        // Create the appropriate linker using enum dispatch (not trait objects)
        // This provides exhaustiveness checking, static dispatch, and no heap allocation
        let mut linker = match flavor {
            LinkerFlavor::Gcc => LinkerImpl::Gcc(GccLinker::new(&self.target)),
            LinkerFlavor::Lld => {
                if self.target.is_windows() {
                    LinkerImpl::Msvc(MsvcLinker::with_lld(&self.target))
                } else if self.target.is_wasm() {
                    LinkerImpl::Wasm(WasmLinker::new(&self.target))
                } else {
                    // Use clang with -fuse-ld=lld
                    let mut gcc = GccLinker::with_path(&self.target, "clang");
                    gcc.cmd().arg("-fuse-ld=lld");
                    LinkerImpl::Gcc(gcc)
                }
            }
            LinkerFlavor::Msvc => LinkerImpl::Msvc(MsvcLinker::new(&self.target)),
            LinkerFlavor::WasmLd => LinkerImpl::Wasm(WasmLinker::new(&self.target)),
        };

        // Configure linker
        Self::configure_linker(&mut linker, input)?;

        // Get the final command
        let cmd = linker.finalize();

        // Execute with retry logic
        self.execute_with_retry(cmd, input)
    }

    /// Configure the linker with all input settings.
    pub fn configure_linker(linker: &mut LinkerImpl, input: &LinkInput) -> Result<(), LinkerError> {
        // Set output kind first (affects other options)
        linker.set_output_kind(input.output_kind);

        // Add object files
        for obj in &input.objects {
            linker.add_object(obj);
        }

        // Add library search paths
        for path in &input.library_paths {
            linker.add_library_path(path);
        }

        // Add libraries
        for lib in &input.libraries {
            if let Some(ref path) = lib.search_path {
                linker.add_library_path(path);
            }
            linker.link_library(&lib.name, lib.kind);
        }

        // Configure optimizations
        if input.gc_sections {
            linker.gc_sections(true);
        }

        if input.strip {
            linker.strip_symbols(true);
        }

        // Export symbols
        if !input.exported_symbols.is_empty() {
            linker.export_symbols(&input.exported_symbols);
        }

        // Add extra arguments
        for arg in &input.extra_args {
            linker.add_arg(arg);
        }

        // Set output last (some linkers are order-sensitive)
        linker.set_output(&input.output);

        Ok(())
    }

    /// Execute linker with retry logic for common failures.
    fn execute_with_retry(&self, cmd: Command, input: &LinkInput) -> Result<(), LinkerError> {
        // Check if we need to use a response file
        let cmd = self.maybe_use_response_file(cmd)?;

        // First attempt
        let output = Self::run_linker(&cmd)?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Check for retryable errors
        if Self::should_retry(&stderr) {
            // Retry with adjusted options
            return self.retry_link(input, &stderr);
        }

        // Linking failed
        Err(LinkerError::LinkFailed {
            linker: cmd.get_program().to_string_lossy().into(),
            exit_code: output.status.code(),
            stderr,
            command: format!("{cmd:?}"),
        })
    }

    /// Run the linker and capture output.
    fn run_linker(cmd: &Command) -> Result<Output, LinkerError> {
        // Clone the command for execution
        // Note: Command doesn't implement Clone, so we need to reconstruct
        let program = cmd.get_program().to_owned();
        let args: Vec<OsString> = cmd.get_args().map(ToOwned::to_owned).collect();

        let mut exec_cmd = Command::new(program);
        exec_cmd.args(args);

        exec_cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LinkerError::LinkerNotFound {
                    linker: cmd.get_program().to_string_lossy().into(),
                    message: e.to_string(),
                }
            } else {
                LinkerError::IoError {
                    message: e.to_string(),
                }
            }
        })
    }

    /// Check if the error is retryable.
    pub fn should_retry(stderr: &str) -> bool {
        // Common retryable errors (from rustc's experience)
        let retryable_patterns = [
            "unrecognized option",
            "unknown option",
            "-no-pie",
            "-static-pie",
            "-fuse-ld=lld",
        ];

        retryable_patterns
            .iter()
            .any(|pattern| stderr.contains(pattern))
    }

    /// Retry linking with adjusted options.
    fn retry_link(&self, input: &LinkInput, _stderr: &str) -> Result<(), LinkerError> {
        // Create new input with adjusted settings
        let mut adjusted = input.clone();

        // Remove potentially problematic extra args
        adjusted.extra_args.retain(|arg| {
            !arg.contains("-no-pie")
                && !arg.contains("-static-pie")
                && !arg.contains("-fuse-ld=lld")
        });

        // If we were using LLD, fall back to default
        if adjusted.linker == Some(LinkerFlavor::Lld) {
            adjusted.linker = Some(LinkerFlavor::Gcc);
        }

        // Retry with adjusted settings
        self.link(&adjusted)
    }

    /// Use a response file if the command line is too long.
    fn maybe_use_response_file(&self, cmd: Command) -> Result<Command, LinkerError> {
        // Estimate command line length
        let args: Vec<_> = cmd.get_args().collect();
        let total_len: usize = args.iter().map(|a| a.len() + 1).sum();

        // Platform-specific limits:
        // - Unix: ~2MB (ARG_MAX)
        // - Windows cmd.exe: ~8KB
        // - Windows CreateProcess: ~32KB
        let limit = if self.target.is_windows() {
            8 * 1024 // Conservative for cmd.exe
        } else {
            128 * 1024 // Conservative for Unix
        };

        if total_len < limit {
            return Ok(cmd);
        }

        // Create response file
        Self::create_response_file(&cmd)
    }

    /// Create a response file and return a command that uses it.
    pub fn create_response_file(cmd: &Command) -> Result<Command, LinkerError> {
        // Create temp file
        let temp_dir = std::env::temp_dir();
        let rsp_path = temp_dir.join(format!("ori_link_{}.rsp", std::process::id()));

        let mut file =
            std::fs::File::create(&rsp_path).map_err(|e| LinkerError::ResponseFileError {
                path: rsp_path.display().to_string(),
                message: e.to_string(),
            })?;

        // Write arguments to response file
        for arg in cmd.get_args() {
            let arg_str = arg.to_string_lossy();

            // Quote arguments with spaces
            if arg_str.contains(' ') || arg_str.contains('"') {
                writeln!(file, "\"{}\"", arg_str.replace('"', "\\\""))
            } else {
                writeln!(file, "{arg_str}")
            }
            .map_err(|e| LinkerError::ResponseFileError {
                path: rsp_path.display().to_string(),
                message: e.to_string(),
            })?;
        }

        // Create new command with response file
        let mut new_cmd = Command::new(cmd.get_program());
        new_cmd.arg(format!("@{}", rsp_path.display()));

        Ok(new_cmd)
    }
}

// --- Linker Detection ---

/// Detect available linkers on the system.
#[derive(Debug, Clone, Default)]
pub struct LinkerDetection {
    /// Available linkers, in preference order.
    pub available: Vec<LinkerFlavor>,
    /// Linkers that were checked but not found.
    pub not_found: Vec<LinkerFlavor>,
}

impl LinkerDetection {
    /// Detect available linkers for the given target.
    pub fn detect(target: &TargetConfig) -> Self {
        let mut detection = Self::default();
        let mut checked = HashSet::new();

        // Determine which linkers to check based on target
        let to_check = if target.is_wasm() {
            vec![LinkerFlavor::WasmLd]
        } else if target.is_windows() {
            vec![LinkerFlavor::Msvc, LinkerFlavor::Lld, LinkerFlavor::Gcc]
        } else {
            vec![LinkerFlavor::Gcc, LinkerFlavor::Lld]
        };

        for flavor in to_check {
            if checked.insert(flavor) {
                if Self::is_available(flavor) {
                    detection.available.push(flavor);
                } else {
                    detection.not_found.push(flavor);
                }
            }
        }

        detection
    }

    /// Check if a specific linker flavor is available.
    fn is_available(flavor: LinkerFlavor) -> bool {
        let program = match flavor {
            LinkerFlavor::Gcc => "cc",
            LinkerFlavor::Lld => "lld",
            LinkerFlavor::Msvc => "link.exe",
            LinkerFlavor::WasmLd => "wasm-ld",
        };

        // Try to run with --version or /? to check availability
        let result = match flavor {
            LinkerFlavor::Msvc => Command::new(program).arg("/?").output(),
            _ => Command::new(program).arg("--version").output(),
        };

        result.is_ok()
    }

    /// Get the preferred linker, if any are available.
    #[must_use]
    pub fn preferred(&self) -> Option<LinkerFlavor> {
        self.available.first().copied()
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn test_target() -> TargetConfig {
        // Create a test config without initializing LLVM
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        TargetConfig::from_components(components)
    }

    fn test_target_macos() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
        TargetConfig::from_components(components)
    }

    fn test_target_windows() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        TargetConfig::from_components(components)
    }

    fn test_target_wasm() -> TargetConfig {
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        TargetConfig::from_components(components)
    }

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
    fn test_link_library_builder() {
        let lib = LinkLibrary::new("foo")
            .static_lib()
            .with_search_path("/usr/lib");

        assert_eq!(lib.name, "foo");
        assert_eq!(lib.kind, LibraryKind::Static);
        assert_eq!(lib.search_path, Some(PathBuf::from("/usr/lib")));
    }

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
    fn test_link_input_default() {
        let input = LinkInput::default();
        assert!(input.objects.is_empty());
        assert_eq!(input.output_kind, LinkOutput::Executable);
        assert!(!input.lto);
        assert!(!input.strip);
    }

    #[test]
    fn test_linker_driver_invalid_input() {
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        let result = driver.link(&LinkInput::default());
        assert!(matches!(result, Err(LinkerError::InvalidConfig { .. })));
    }

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
    fn test_export_symbols_msvc() {
        let target = test_target_windows();
        let mut linker = MsvcLinker::new(&target);

        linker.export_symbols(&["foo".to_string()]);

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"/EXPORT:foo".into()));
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

    // Additional tests for improved coverage

    #[allow(dead_code)]
    fn test_target_windows_gnu() -> TargetConfig {
        let components = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
        TargetConfig::from_components(components)
    }

    // -- LinkerError coverage --

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

    // -- LinkOutput coverage --

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

    // -- LinkLibrary coverage --

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

    // -- GccLinker coverage --

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

    // -- MsvcLinker coverage --

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

    // -- WasmLinker coverage --

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

    // -- LinkerFlavor coverage --

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

    // -- LinkerDriver coverage --

    #[test]
    fn test_linker_driver_new() {
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        // Just verify it doesn't panic
        let _ = driver;
    }

    #[test]
    fn test_linker_driver_configure_linker() {
        // Test that configure_linker sets all fields correctly
        let target = test_target();
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

        LinkerDriver::configure_linker(&mut linker, &input).unwrap();

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Check objects
        assert!(args.contains(&"main.o".into()));
        assert!(args.contains(&"lib.o".into()));

        // Check libraries
        assert!(args.contains(&"-lm".into()));
        assert!(args.contains(&"-lpthread".into()));

        // Check library paths
        assert!(args.iter().any(|a| a.contains("/usr/local/lib")));

        // Check gc_sections
        assert!(args.iter().any(|a| a.contains("gc-sections")));

        // Check strip
        assert!(args.iter().any(|a| a.contains("strip")));

        // Check extra args
        assert!(args.contains(&"-v".into()));

        // Check output
        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"output".into()));
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
    fn test_linker_driver_with_library_search_path() {
        let target = test_target();
        let mut linker = LinkerImpl::Gcc(GccLinker::new(&target));

        let input = LinkInput {
            objects: vec![PathBuf::from("main.o")],
            output: PathBuf::from("output"),
            libraries: vec![LinkLibrary::new("custom").with_search_path("/opt/custom/lib")],
            ..Default::default()
        };

        LinkerDriver::configure_linker(&mut linker, &input).unwrap();

        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Library's search path should be added
        assert!(args.iter().any(|a| a.contains("/opt/custom/lib")));
    }

    // -- LinkerDetection coverage --

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

    // -- Response file coverage --

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

    #[test]
    fn test_maybe_use_response_file_short_command() {
        let target = test_target();
        let driver = LinkerDriver::new(&target);

        let mut cmd = Command::new("cc");
        cmd.arg("-o").arg("output").arg("main.o");

        // Short command should not use response file
        let result = driver.maybe_use_response_file(cmd);
        assert!(result.is_ok());

        let cmd = result.unwrap();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Should still have original args, not @file
        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"main.o".into()));
    }

    // -- LinkInput coverage --

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

    // -- Edge cases --

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

        // We can't actually run the linker, but we can verify the setup
        // by checking the link method doesn't panic with valid input
        let result = driver.link(&input);

        // Will fail because linker not found, but that's expected
        assert!(matches!(
            result,
            Err(LinkerError::LinkerNotFound { .. } | LinkerError::LinkFailed { .. })
        ));
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
}
