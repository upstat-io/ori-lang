//! System Library Detection for Cross-Compilation
//!
//! This module provides utilities for detecting system libraries and sysroots
//! when compiling for different target platforms.
//!
//! # Overview
//!
//! When cross-compiling, the linker needs to find:
//! 1. Target-specific C runtime libraries (libc, libm, etc.)
//! 2. The sysroot containing the target's system files
//! 3. Platform-specific library paths
//!
//! # Sysroot Detection
//!
//! Sysroots are detected in this order:
//! 1. `ORI_SYSROOT_<TARGET>` environment variable
//! 2. `--sysroot` flag in linker configuration
//! 3. Well-known locations for common targets
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::{SysLibConfig, TargetConfig};
//!
//! let target = TargetConfig::from_triple("aarch64-linux-gnu")?;
//! let syslib = SysLibConfig::for_target(&target)?;
//!
//! // Get library search paths
//! for path in syslib.library_paths() {
//!     println!("Search path: {}", path.display());
//! }
//! ```

use std::path::{Path, PathBuf};

use super::target_features::{HostPlatform, TargetTripleComponents};

// =============================================================================
// Install-paths SSOT
// =============================================================================
//
// These functions are the canonical home for "where Ori puts and looks for
// user-managed sysroots". Both the install side (`oric::commands::target::
// add_target`) and the discovery side (`SysLibConfig::detect_sysroot`)
// query the same functions, so an install always lands somewhere a build
// can find it. Before this SSOT was extracted, the install side hard-coded
// `~/.ori/sysroots/<target>` and `~/.wasi-sdk` while the discovery side
// hard-coded only `/opt/wasi-sdk/...` and `/usr/share/wasi-sysroot` —
// installs reported "success" whose results were invisible to subsequent
// builds. See BUG-04-045 / TPR-BUG-04-045-06.
//
// All paths are parameterized by `home: &Path` so tests can supply a tempdir
// without touching process-global `HOME`. The convenience wrappers
// (`ori_sysroots_dir`, `home_wasi_sdk_sysroot`) read `HOME`/`USERPROFILE`
// from the environment.

/// User home directory, derived from `HOME` (Unix) or `USERPROFILE`
/// (Windows). Falls back to `.` if neither is set — callers that need
/// a deterministic path should use the `_for_home` parameterized variants
/// instead.
#[must_use]
pub fn user_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_or_else(|_| PathBuf::from("."), PathBuf::from)
}

/// Per-user directory under which Ori stores managed sysroots:
/// `<home>/.ori/sysroots`. The canonical install root.
#[must_use]
pub fn ori_sysroots_dir_for_home(home: &Path) -> PathBuf {
    home.join(".ori").join("sysroots")
}

/// `~/.ori/sysroots` — convenience wrapper around
/// [`ori_sysroots_dir_for_home`] that reads the home dir from the
/// environment.
#[must_use]
pub fn ori_sysroots_dir() -> PathBuf {
    ori_sysroots_dir_for_home(&user_home_dir())
}

/// Path of an Ori-managed sysroot for a specific target identified by its
/// canonical key. The canonical key is [`TargetTripleComponents::support_key`],
/// which strips Darwin OS version suffixes and normalizes arch aliases —
/// the same SSOT key the supported-targets check uses. This guarantees one
/// sysroot per logical target, no per-OS-subversion duplicates.
#[must_use]
pub fn ori_sysroot_path_for_home(home: &Path, canonical_key: &str) -> PathBuf {
    ori_sysroots_dir_for_home(home).join(canonical_key)
}

/// Convenience wrapper for [`ori_sysroot_path_for_home`].
#[must_use]
pub fn ori_sysroot_path(canonical_key: &str) -> PathBuf {
    ori_sysroot_path_for_home(&user_home_dir(), canonical_key)
}

/// Per-user WASI SDK sysroot, used by `ori target add` for WASI targets:
/// `<home>/.wasi-sdk/share/wasi-sysroot`. This location is recognized by
/// the install side and consulted by the discovery side, so a user-local
/// WASI SDK install is visible to `ori build` without needing to set
/// `ORI_SYSROOT_*`.
#[must_use]
pub fn home_wasi_sdk_sysroot_for_home(home: &Path) -> PathBuf {
    home.join(".wasi-sdk").join("share").join("wasi-sysroot")
}

/// Convenience wrapper for [`home_wasi_sdk_sysroot_for_home`].
#[must_use]
pub fn home_wasi_sdk_sysroot() -> PathBuf {
    home_wasi_sdk_sysroot_for_home(&user_home_dir())
}

/// System library configuration for a target.
#[derive(Debug, Clone)]
pub struct SysLibConfig {
    /// Target triple components.
    target: TargetTripleComponents,
    /// Sysroot path (if found).
    sysroot: Option<PathBuf>,
    /// Additional library search paths.
    library_paths: Vec<PathBuf>,
}

/// Error when system libraries cannot be found.
#[derive(Debug, Clone)]
pub struct SysLibError {
    /// Target that was being configured.
    pub target: String,
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for SysLibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to configure system libraries for {}: {}",
            self.target, self.message
        )
    }
}

impl std::error::Error for SysLibError {}

impl SysLibConfig {
    /// Create a system library configuration for the given target.
    ///
    /// This detects sysroot and library paths for the target.
    pub fn for_target(target: &TargetTripleComponents) -> Result<Self, SysLibError> {
        let sysroot = Self::detect_sysroot(target);
        let library_paths = Self::detect_library_paths(target, sysroot.as_ref());

        Ok(Self {
            target: target.clone(),
            sysroot,
            library_paths,
        })
    }

    /// Create a system library configuration with an explicit sysroot.
    #[must_use]
    pub fn with_sysroot(target: &TargetTripleComponents, sysroot: PathBuf) -> Self {
        let library_paths = Self::detect_library_paths(target, Some(&sysroot));

        Self {
            target: target.clone(),
            sysroot: Some(sysroot),
            library_paths,
        }
    }

    /// Get the sysroot path, if any.
    #[must_use]
    pub fn sysroot(&self) -> Option<&PathBuf> {
        self.sysroot.as_ref()
    }

    /// Get the library search paths for this target.
    ///
    /// Returns paths in search order (most specific first).
    #[must_use]
    pub fn library_paths(&self) -> &[PathBuf] {
        &self.library_paths
    }

    /// Get the target triple components.
    #[must_use]
    pub fn target(&self) -> &TargetTripleComponents {
        &self.target
    }

    /// Check if this is a native compilation (no cross-compilation).
    ///
    /// Delegates to the typed [`TargetTripleComponents::is_native_for`]
    /// against [`HostPlatform::current`], operating on canonical [`Arch`]
    /// values rather than raw `cfg` string compares. See BUG-04-045.
    ///
    /// [`Arch`]: crate::aot::target_features::Arch
    #[must_use]
    pub fn is_native(&self) -> bool {
        self.target.is_native_for(HostPlatform::current())
    }

    /// Get the required system libraries for linking.
    ///
    /// Returns library names that should be linked (without -l prefix).
    #[must_use]
    pub fn required_libraries(&self) -> Vec<&'static str> {
        let mut libs = Vec::new();

        if self.target.is_wasm() {
            // WASM has minimal library requirements
            return libs;
        }

        // All Unix-like systems need libc
        if self.target.is_linux() || self.target.is_macos() {
            libs.push("c");
            libs.push("m");
        }

        // Linux may need additional libraries
        if self.target.is_linux() {
            libs.push("pthread");
            libs.push("dl");
        }

        // macOS-specific libraries
        if self.target.is_macos() {
            libs.push("System");
        }

        // Windows libraries are handled differently (kernel32, etc.)
        // They're typically linked automatically by the MSVC linker

        libs
    }

    /// Detect the sysroot for a target.
    ///
    /// Convenience wrapper around [`Self::detect_sysroot_for_home`] that
    /// reads `HOME` / `USERPROFILE` from the process environment. Tests
    /// that need a deterministic home directory should call the
    /// `_for_home` variant directly to avoid process-global state.
    ///
    /// Search order:
    /// 1. `ORI_SYSROOT_<TARGET>` env var (per-target override)
    /// 2. `ORI_SYSROOT` env var (generic override)
    /// 3. Per-user Ori-managed install at `~/.ori/sysroots/<canonical-key>`
    ///    where the canonical key is [`TargetTripleComponents::support_key`].
    ///    This is the same key `oric::commands::target::add_target` uses
    ///    when writing the install, so the round-trip is invariant under
    ///    Darwin version suffixes and arch aliases.
    /// 4. Hard-coded system locations from [`Self::sysroot_candidates`]
    ///    (Xcode SDK, /opt/wasi-sdk, /usr/share/wasi-sysroot, etc.).
    fn detect_sysroot(target: &TargetTripleComponents) -> Option<PathBuf> {
        Self::detect_sysroot_for_home(target, &user_home_dir())
    }

    /// Sysroot detection parameterized by an explicit home directory.
    /// Lets tests verify the install/discovery round-trip without
    /// touching process-global `HOME`/`USERPROFILE` state.
    pub(crate) fn detect_sysroot_for_home(
        target: &TargetTripleComponents,
        home: &Path,
    ) -> Option<PathBuf> {
        // 1. Check environment variable: ORI_SYSROOT_<TARGET>
        let env_key = format!(
            "ORI_SYSROOT_{}",
            target.to_string().to_uppercase().replace('-', "_")
        );
        if let Ok(path) = std::env::var(&env_key) {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // 2. Check generic ORI_SYSROOT
        if let Ok(path) = std::env::var("ORI_SYSROOT") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // 3. Per-user Ori-managed install location. Use the canonical
        // support key as the directory name, matching what the install
        // side writes — so a `target add arm64-apple-darwin25.2.0` is
        // discoverable by `ori build` for any darwin25.x.x input.
        let install_path = ori_sysroot_path_for_home(home, &target.support_key());
        if install_path.exists() {
            return Some(install_path);
        }

        // 4. Check well-known system locations
        Self::sysroot_candidates_for_home(target, home)
            .into_iter()
            .find(|candidate| candidate.exists())
    }

    /// Get sysroot candidate paths for a target.
    ///
    /// Convenience wrapper that reads the home directory from the
    /// environment. Tests should use [`Self::sysroot_candidates_for_home`]
    /// to keep results deterministic.
    #[cfg(test)]
    pub(crate) fn sysroot_candidates(target: &TargetTripleComponents) -> Vec<PathBuf> {
        Self::sysroot_candidates_for_home(target, &user_home_dir())
    }

    /// Get sysroot candidate paths for a target with an explicit home
    /// directory. Used by [`Self::detect_sysroot_for_home`] and tests.
    pub(crate) fn sysroot_candidates_for_home(
        target: &TargetTripleComponents,
        home: &Path,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        let triple = target.to_string();

        // Linux cross-compilation sysroots
        if target.is_linux() {
            // Debian/Ubuntu multiarch
            candidates.push(PathBuf::from(format!("/usr/{triple}")));

            // Common cross-compilation prefixes
            candidates.push(PathBuf::from(format!("/opt/cross/{triple}")));
            candidates.push(PathBuf::from(format!("/opt/{triple}")));

            // musl sysroots
            if target.env.as_deref() == Some("musl") {
                candidates.push(PathBuf::from("/usr/lib/musl"));
                candidates.push(PathBuf::from(format!("/opt/musl/{}", target.arch)));
            }
        }

        // macOS SDK locations
        if target.is_macos() {
            // Xcode SDK paths
            if let Ok(output) = std::process::Command::new("xcrun")
                .args(["--sdk", "macosx", "--show-sdk-path"])
                .output()
            {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        candidates.push(PathBuf::from(path.trim()));
                    }
                }
            }

            // Common SDK locations
            candidates.push(PathBuf::from(
                "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk"
            ));
            candidates.push(PathBuf::from(
                "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
            ));
        }

        // Windows SDK locations (for cross-compilation from Unix)
        if target.is_windows() {
            candidates.push(PathBuf::from("/opt/windows-sdk"));
            candidates.push(PathBuf::from(format!("/opt/{triple}")));
        }

        // WASM sysroots — both system-wide and per-user installs.
        // The per-user `~/.wasi-sdk/share/wasi-sysroot` location is what
        // `ori target add wasm32-unknown-wasip1` recognizes as a valid
        // install source (see `oric::commands::target::check_wasi_sdk`),
        // so the discovery side must consult it too — otherwise the
        // install side reports success while builds silently miss the
        // sysroot. Regression: BUG-04-045 / TPR-BUG-04-045-06.
        if target.is_wasm() {
            candidates.push(home_wasi_sdk_sysroot_for_home(home));
            candidates.push(PathBuf::from("/opt/wasi-sdk/share/wasi-sysroot"));
            candidates.push(PathBuf::from("/usr/share/wasi-sysroot"));
        }

        candidates
    }

    /// Detect library paths for a target.
    fn detect_library_paths(
        target: &TargetTripleComponents,
        sysroot: Option<&PathBuf>,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // If we have a sysroot, add its lib directories first
        if let Some(sysroot) = sysroot {
            paths.push(sysroot.join("lib"));
            paths.push(sysroot.join("usr/lib"));

            // Architecture-specific subdirectories
            let triple = target.to_string();
            paths.push(sysroot.join(format!("lib/{triple}")));
            paths.push(sysroot.join(format!("usr/lib/{triple}")));

            // lib64 for 64-bit non-wasm targets
            if target.arch.is_64_bit_non_wasm() {
                paths.push(sysroot.join("lib64"));
                paths.push(sysroot.join("usr/lib64"));
            }
        }

        // Add native paths if this is a native compilation
        if sysroot.is_none() {
            if target.is_linux() {
                paths.push(PathBuf::from("/lib"));
                paths.push(PathBuf::from("/usr/lib"));
                paths.push(PathBuf::from("/lib64"));
                paths.push(PathBuf::from("/usr/lib64"));
                paths.push(PathBuf::from("/usr/local/lib"));

                // Multiarch paths
                let triple = target.to_string();
                paths.push(PathBuf::from(format!("/lib/{triple}")));
                paths.push(PathBuf::from(format!("/usr/lib/{triple}")));
            } else if target.is_macos() {
                paths.push(PathBuf::from("/usr/lib"));
                paths.push(PathBuf::from("/usr/local/lib"));

                // Homebrew paths
                #[cfg(target_arch = "aarch64")]
                paths.push(PathBuf::from("/opt/homebrew/lib"));
                #[cfg(target_arch = "x86_64")]
                paths.push(PathBuf::from("/usr/local/lib"));
            } else if target.is_windows() {
                // Windows library paths are typically handled by the linker
                // based on environment variables like LIB
            }
        }

        // Filter to existing paths
        paths.retain(|p| p.exists());

        paths
    }
}

/// Library search order for linking.
///
/// This determines the priority when searching for libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySearchOrder {
    /// User paths first, then system paths.
    #[default]
    UserFirst,
    /// System paths first, then user paths.
    SystemFirst,
    /// Only search user-specified paths.
    UserOnly,
    /// Only search system paths.
    SystemOnly,
}

/// Find a library by name in the given search paths.
///
/// Returns the first path where the library is found.
#[must_use]
pub fn find_library(
    name: &str,
    paths: &[PathBuf],
    target: &TargetTripleComponents,
) -> Option<PathBuf> {
    let extensions = if target.is_windows() {
        vec!["lib"]
    } else if target.is_macos() {
        vec!["dylib", "a"]
    } else {
        vec!["so", "a"]
    };

    let prefixes = if target.is_windows() {
        vec![""]
    } else {
        vec!["lib"]
    };

    for path in paths {
        for prefix in &prefixes {
            for ext in &extensions {
                let filename = format!("{prefix}{name}.{ext}");
                let full_path = path.join(&filename);
                if full_path.exists() {
                    return Some(full_path);
                }
            }
        }
    }

    None
}

/// Check if a library exists in the given search paths.
#[must_use]
pub fn library_exists(name: &str, paths: &[PathBuf], target: &TargetTripleComponents) -> bool {
    find_library(name, paths, target).is_some()
}

#[cfg(test)]
mod tests;
