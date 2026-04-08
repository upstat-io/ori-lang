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

mod driver;
mod gcc;
mod msvc;
mod wasm;

pub use gcc::GccLinker;
pub use msvc::MsvcLinker;
pub use wasm::WasmLinker;

use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::aot::target::TargetConfig;
use crate::aot::target_features::{HostPlatform, TargetTripleComponents};

// Error Types

/// Error type for linker operations.
#[derive(Debug, Clone)]
pub enum LinkerError {
    /// Linker executable not found.
    LinkerNotFound { linker: String, message: String },
    /// Linker invocation failed.
    LinkFailed {
        linker: String,
        exit_code: Option<i32>,
        stdout: String,
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
                stdout,
                stderr,
                command,
            } => {
                write!(f, "linking with '{linker}' failed")?;
                if let Some(code) = exit_code {
                    write!(f, " (exit code {code})")?;
                }
                // MSVC link.exe sends diagnostics (LNK2019, etc.) to stdout.
                // Show stdout first since it often contains the primary error.
                if !stdout.is_empty() {
                    write!(f, "\n\nLinker stdout:\n{stdout}")?;
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

// Output Types

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
    ///
    /// Delegates to the typed [`TargetTripleComponents::is_windows`] /
    /// [`TargetTripleComponents::is_macos`] predicates so that Darwin OS
    /// version suffixes (e.g., `darwin25.2.0` from Apple Silicon's
    /// LLVM-default triple) correctly select `.dylib` for shared libraries.
    /// Matching `target.os.as_str()` directly against `"darwin"` would
    /// fall through to the Linux/ELF branch and emit `.so` — see
    #[must_use]
    pub fn extension(&self, target: &TargetTripleComponents) -> &'static str {
        match self {
            Self::Executable | Self::PositionIndependentExecutable => {
                if target.is_windows() {
                    "exe"
                } else {
                    ""
                }
            }
            Self::SharedLibrary => {
                if target.is_windows() {
                    "dll"
                } else if target.is_macos() {
                    "dylib"
                } else {
                    "so"
                }
            }
            Self::StaticLibrary => {
                if target.is_windows() {
                    "lib"
                } else {
                    "a"
                }
            }
        }
    }
}

// Library Types

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

// Link Input

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

// Linker Flavor

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

// Linker Implementation Enum

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

// Linker Driver

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
}

// Linker Detection

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
        match flavor {
            LinkerFlavor::Msvc => Self::is_msvc_available(),
            LinkerFlavor::Gcc => Self::check_program("cc", "--version"),
            LinkerFlavor::Lld => {
                let program = if cfg!(windows) { "lld-link" } else { "lld" };
                Self::check_program(program, "--version")
            }
            LinkerFlavor::WasmLd => Self::check_program("wasm-ld", "--version"),
        }
    }

    /// Check if a linker program is available by running it with a version flag.
    fn check_program(program: &str, flag: &str) -> bool {
        Command::new(program).arg(flag).output().is_ok()
    }

    /// Check if MSVC's `link.exe` is available.
    ///
    /// Uses the same Visual Studio discovery as [`MsvcLinker::new`] so that
    /// detection and construction agree on which `link.exe` to use. Falls
    /// back to PATH-based lookup only when VS discovery finds nothing, and
    /// verifies the PATH result is actually MSVC (not GNU coreutils `link`).
    fn is_msvc_available() -> bool {
        // If VS discovery finds link.exe, it's definitely MSVC's — no need to verify.
        if msvc::find_msvc_toolchain().is_some() {
            return true;
        }

        // Fall back to PATH lookup — must verify it's actually MSVC's link.exe,
        // not GNU coreutils `link` (which creates hard links).
        // MSVC link.exe prints "Microsoft (R) Incremental Linker" on stdout
        // when invoked with /?. GNU link prints a one-line usage message.
        match Command::new("link.exe").arg("/?").output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("Microsoft") || stdout.contains("Incremental Linker")
            }
            Err(_) => false,
        }
    }

    /// Get the preferred linker, if any are available.
    #[must_use]
    pub fn preferred(&self) -> Option<LinkerFlavor> {
        self.available.first().copied()
    }

    /// Check if we're cross-compiling (target OS or arch differs from host).
    ///
    /// Cross-compilation includes both different-OS targets (Linux→Windows)
    /// and same-OS different-arch targets (`x86_64` Linux → `aarch64` Linux).
    /// Both cases require a cross-compiler toolchain.
    ///
    /// Delegates to the typed [`TargetTripleComponents::is_cross_for`] against
    /// [`HostPlatform::current`], operating on canonical [`Arch`] values.
    /// This avoids the `arm64` (LLVM default triple on Apple Silicon) vs
    /// `aarch64` (Rust `cfg(target_arch)`) raw-string mismatch that mis-
    /// detected native Apple Silicon builds as cross-compilation — see
    /// regression pin in `target_features/tests.rs`.
    ///
    /// [`Arch`]: crate::aot::target_features::Arch
    #[must_use]
    pub fn is_cross_compiling(target: &TargetConfig) -> bool {
        target.components().is_cross_for(HostPlatform::current())
    }

    /// Get the expected GCC cross-compiler program name for a target.
    ///
    /// Returns `None` for targets that don't have a standard GCC cross-compiler
    /// naming convention (macOS, WASM, windows-msvc).
    #[must_use]
    pub fn gcc_cross_compiler_name(target: &TargetTripleComponents) -> Option<String> {
        if target.is_wasm() {
            return None;
        }
        if target.is_windows() {
            return match target.env.as_deref() {
                Some("gnu") => Some(format!("{}-w64-mingw32-gcc", target.arch)),
                // No standard GCC cross-compiler for MSVC targets
                _ => None,
            };
        }
        if target.is_macos() {
            // macOS cross-compilation uses osxcross with non-standard names;
            // no single standard name to check
            return None;
        }
        if target.is_linux() {
            let env = target.env.as_deref().unwrap_or("gnu");
            return Some(format!("{}-linux-{}-gcc", target.arch, env));
        }
        None
    }

    /// Check if a specific linker flavor is available for a given target.
    ///
    /// Unlike [`is_available`], this method accounts for cross-compilation:
    /// when the target OS differs from the host, it checks for the appropriate
    /// cross-compiler instead of the host compiler.
    pub fn is_available_for_target(flavor: LinkerFlavor, target: &TargetConfig) -> bool {
        let cross = Self::is_cross_compiling(target);

        match flavor {
            LinkerFlavor::Msvc => {
                if cross && !cfg!(target_os = "windows") {
                    // MSVC tools are only available on Windows hosts
                    false
                } else {
                    Self::is_msvc_available()
                }
            }
            LinkerFlavor::Gcc => {
                if cross {
                    // Need a target-specific cross-compiler, not the host `cc`
                    match Self::gcc_cross_compiler_name(target.components()) {
                        Some(name) => Self::check_program(&name, "--version"),
                        None => false, // No known GCC cross-compiler for this target
                    }
                } else {
                    Self::check_program("cc", "--version")
                }
            }
            LinkerFlavor::Lld => {
                // LLD works for cross-compilation — check the right variant.
                // For non-Windows/non-WASM, create_linker() uses `clang -fuse-ld=lld`,
                // so we check for clang (the actual driver) as well as standalone ld.lld.
                if target.is_windows() {
                    Self::check_program("lld-link", "--version")
                } else if target.is_wasm() {
                    Self::check_program("wasm-ld", "--version")
                } else {
                    Self::check_program("ld.lld", "--version")
                        || Self::check_program("clang", "--version")
                }
            }
            LinkerFlavor::WasmLd => Self::check_program("wasm-ld", "--version"),
        }
    }

    /// Detect available linkers for a target, accounting for cross-compilation.
    pub fn detect_for_target(target: &TargetConfig) -> Self {
        let mut detection = Self::default();
        let mut checked = HashSet::new();

        let to_check = if target.is_wasm() {
            vec![LinkerFlavor::WasmLd]
        } else if target.is_windows() {
            vec![LinkerFlavor::Msvc, LinkerFlavor::Lld, LinkerFlavor::Gcc]
        } else {
            vec![LinkerFlavor::Gcc, LinkerFlavor::Lld]
        };

        for flavor in to_check {
            if checked.insert(flavor) {
                if Self::is_available_for_target(flavor, target) {
                    detection.available.push(flavor);
                } else {
                    detection.not_found.push(flavor);
                }
            }
        }

        detection
    }

    /// Build an actionable error message for failed cross-compilation linker detection.
    pub fn cross_compilation_error(target: &TargetConfig) -> LinkerError {
        let triple = target.triple();
        let components = target.components();

        let mut help = String::new();

        if components.is_windows() {
            if components.env.as_deref() == Some("msvc") {
                help.push_str(
                    "  - Install LLVM/LLD (provides lld-link for MSVC-compatible linking)\n",
                );
                help.push_str(
                    "  - Or build natively on Windows with Visual Studio / MSVC Build Tools\n",
                );
            } else if components.env.as_deref() == Some("gnu") {
                let _ = writeln!(
                    help,
                    "  - Install mingw-w64 (provides {}-w64-mingw32-gcc)",
                    components.arch
                );
                help.push_str("  - Or install LLVM/LLD (provides lld-link)\n");
            }
        } else if components.is_macos() {
            help.push_str("  - Install osxcross (https://github.com/tpoechtrager/osxcross)\n");
            help.push_str("  - Or install LLVM/LLD (provides ld.lld)\n");
            help.push_str("  - Or build natively on macOS\n");
        } else if components.is_linux() {
            if let Some(gcc_name) = Self::gcc_cross_compiler_name(components) {
                let _ = writeln!(
                    help,
                    "  - Install the cross-compiler toolchain (provides {gcc_name})"
                );
            }
            help.push_str("  - Or install LLVM/LLD (provides ld.lld)\n");
        } else {
            help.push_str("  - Install a cross-linker for this target\n");
        }

        LinkerError::LinkerNotFound {
            linker: format!("cross-linker for {triple}"),
            message: format!(
                "no suitable cross-linker found for target '{triple}'\n\n\
                 Cross-compilation requires a linker that can produce {os} binaries.\n\
                 Install one of the following:\n{help}",
                os = components.os
            ),
        }
    }
}

#[cfg(test)]
mod tests;
