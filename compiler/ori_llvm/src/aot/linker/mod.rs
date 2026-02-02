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
