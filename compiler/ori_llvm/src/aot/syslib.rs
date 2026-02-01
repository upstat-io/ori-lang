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

use std::path::PathBuf;

use super::target::TargetTripleComponents;

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
    #[must_use]
    pub fn is_native(&self) -> bool {
        // Compare with current platform
        #[cfg(target_os = "linux")]
        let current_os = "linux";
        #[cfg(target_os = "macos")]
        let current_os = "darwin";
        #[cfg(target_os = "windows")]
        let current_os = "windows";
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let current_os = "unknown";

        #[cfg(target_arch = "x86_64")]
        let current_arch = "x86_64";
        #[cfg(target_arch = "aarch64")]
        let current_arch = "aarch64";
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        let current_arch = "unknown";

        self.target.os == current_os && self.target.arch == current_arch
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
    fn detect_sysroot(target: &TargetTripleComponents) -> Option<PathBuf> {
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

        // 3. Check well-known locations based on target
        Self::sysroot_candidates(target)
            .into_iter()
            .find(|candidate| candidate.exists())
    }

    /// Get sysroot candidate paths for a target.
    fn sysroot_candidates(target: &TargetTripleComponents) -> Vec<PathBuf> {
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

        // WASM sysroots
        if target.is_wasm() {
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

            // lib64 for 64-bit targets
            if target.arch == "x86_64" || target.arch == "aarch64" {
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
mod tests {
    use super::*;

    fn linux_target() -> TargetTripleComponents {
        TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap()
    }

    fn macos_target() -> TargetTripleComponents {
        TargetTripleComponents::parse("x86_64-apple-darwin").unwrap()
    }

    fn windows_target() -> TargetTripleComponents {
        TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap()
    }

    fn wasm_target() -> TargetTripleComponents {
        TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap()
    }

    #[test]
    fn test_syslib_config_for_target() {
        let target = linux_target();
        let config = SysLibConfig::for_target(&target).unwrap();

        assert_eq!(config.target().arch, "x86_64");
        assert_eq!(config.target().os, "linux");
    }

    #[test]
    fn test_syslib_config_with_sysroot() {
        let target = linux_target();
        let sysroot = PathBuf::from("/opt/custom-sysroot");
        let config = SysLibConfig::with_sysroot(&target, sysroot.clone());

        assert_eq!(config.sysroot(), Some(&sysroot));
    }

    #[test]
    fn test_required_libraries_linux() {
        let target = linux_target();
        let config = SysLibConfig::for_target(&target).unwrap();
        let libs = config.required_libraries();

        assert!(libs.contains(&"c"));
        assert!(libs.contains(&"m"));
        assert!(libs.contains(&"pthread"));
        assert!(libs.contains(&"dl"));
    }

    #[test]
    fn test_required_libraries_macos() {
        let target = macos_target();
        let config = SysLibConfig::for_target(&target).unwrap();
        let libs = config.required_libraries();

        assert!(libs.contains(&"c"));
        assert!(libs.contains(&"m"));
        assert!(libs.contains(&"System"));
        assert!(!libs.contains(&"pthread")); // macOS uses libSystem
    }

    #[test]
    fn test_required_libraries_wasm() {
        let target = wasm_target();
        let config = SysLibConfig::for_target(&target).unwrap();
        let libs = config.required_libraries();

        assert!(libs.is_empty());
    }

    #[test]
    fn test_required_libraries_windows() {
        let target = windows_target();
        let config = SysLibConfig::for_target(&target).unwrap();
        let libs = config.required_libraries();

        // Windows libraries are linked automatically
        assert!(libs.is_empty());
    }

    #[test]
    fn test_sysroot_env_var() {
        // This test verifies the environment variable format
        let target = linux_target();
        let env_key = format!(
            "ORI_SYSROOT_{}",
            target.to_string().to_uppercase().replace('-', "_")
        );
        assert_eq!(env_key, "ORI_SYSROOT_X86_64_UNKNOWN_LINUX_GNU");
    }

    #[test]
    fn test_library_search_order_default() {
        assert_eq!(LibrarySearchOrder::default(), LibrarySearchOrder::UserFirst);
    }

    #[test]
    fn test_syslib_error_display() {
        let err = SysLibError {
            target: "x86_64-linux-musl".to_string(),
            message: "sysroot not found".to_string(),
        };

        let msg = err.to_string();
        assert!(msg.contains("x86_64-linux-musl"));
        assert!(msg.contains("sysroot not found"));
    }

    #[test]
    fn test_is_native() {
        let target = linux_target();
        let config = SysLibConfig::for_target(&target).unwrap();

        // This will be true or false depending on the host platform
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert!(config.is_native());

        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        assert!(!config.is_native());
    }

    #[test]
    fn test_sysroot_candidates_linux() {
        let target = linux_target();
        let candidates = SysLibConfig::sysroot_candidates(&target);

        // Should include multiarch path
        assert!(candidates
            .iter()
            .any(|p| p.to_string_lossy().contains("x86_64")));
    }

    #[test]
    fn test_sysroot_candidates_wasm() {
        let target = wasm_target();
        let candidates = SysLibConfig::sysroot_candidates(&target);

        // Should include WASI SDK paths
        assert!(candidates
            .iter()
            .any(|p| p.to_string_lossy().contains("wasi")));
    }

    #[test]
    fn test_find_library_not_found() {
        let target = linux_target();
        let paths = vec![PathBuf::from("/nonexistent/path")];

        assert!(find_library("nonexistent", &paths, &target).is_none());
    }

    #[test]
    fn test_library_exists() {
        let target = linux_target();
        let paths = vec![PathBuf::from("/nonexistent/path")];

        assert!(!library_exists("nonexistent", &paths, &target));
    }
}
