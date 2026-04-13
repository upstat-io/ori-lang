//! Linker Detection
//!
//! Detects available system linkers and provides cross-compilation support.
//! Separates the concern of *finding* linkers from the linker types and
//! driver orchestration in `mod.rs`.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::Command;

use crate::aot::target::TargetConfig;
use crate::aot::target_features::{HostPlatform, TargetTripleComponents};

use super::{msvc, LinkerError, LinkerFlavor};

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
