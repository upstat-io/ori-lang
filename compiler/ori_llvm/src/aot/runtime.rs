//! Runtime Library Configuration for AOT Compilation
//!
//! This module provides configuration for linking the Ori runtime library
//! (`libori_rt`) with AOT-compiled programs.
//!
//! # Runtime Library Discovery
//!
//! Discovery follows rustc's sysroot pattern - walk up from the executable:
//!
//! 1. **Dev layout**: Same directory as compiler binary (`target/release/libori_rt.a`)
//! 2. **Installed layout**: `<exe>/../lib/libori_rt.a` (e.g., `/usr/local/bin/ori` → `/usr/local/lib/`)
//! 3. **Workspace dev**: `$ORI_WORKSPACE_DIR/target/{release,debug}/libori_rt.a`
//!
//! No environment variables are used for primary discovery. Use `--runtime-path` CLI flag for overrides.
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::{RuntimeConfig, LinkerDriver, LinkInput};
//!
//! let rt_config = RuntimeConfig::detect()?;
//! let driver = LinkerDriver::new(&target);
//!
//! let mut input = LinkInput::default();
//! rt_config.configure_link(&mut input);
//!
//! driver.link(&input)?;
//! ```

use std::path::{Path, PathBuf};

use super::{LibraryKind, LinkInput, LinkLibrary};

/// Runtime library configuration.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the directory containing `libori_rt.a`.
    pub library_path: PathBuf,
    /// Whether to link statically (default) or dynamically.
    pub static_link: bool,
}

/// Error when runtime library cannot be found.
#[derive(Debug, Clone)]
pub struct RuntimeNotFound {
    /// Paths that were searched.
    pub searched_paths: Vec<PathBuf>,
}

impl std::fmt::Display for RuntimeNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lib_name = RuntimeConfig::lib_name();
        writeln!(f, "Ori runtime library ({lib_name}) not found.")?;
        writeln!(f)?;
        writeln!(f, "Searched paths:")?;
        for path in &self.searched_paths {
            writeln!(f, "  - {}", path.display())?;
        }
        writeln!(f)?;
        writeln!(f, "To fix this, either:")?;
        writeln!(f, "  1. Build the runtime: cargo build -p ori_rt --release")?;
        writeln!(f, "  2. Install Ori properly: make install")?;
        writeln!(
            f,
            "  3. Specify path: ori build --runtime-path=/path/to/lib"
        )?;
        Ok(())
    }
}

impl std::error::Error for RuntimeNotFound {}

impl RuntimeConfig {
    /// Create a new runtime configuration with an explicit path.
    #[must_use]
    pub fn new(library_path: PathBuf) -> Self {
        Self {
            library_path,
            static_link: true,
        }
    }

    /// Get platform-specific library filename.
    #[must_use]
    pub fn lib_name() -> &'static str {
        if cfg!(windows) {
            "ori_rt.lib"
        } else {
            "libori_rt.a"
        }
    }

    /// Check if runtime library exists in directory.
    fn lib_exists(dir: &Path, lib_name: &str) -> bool {
        dir.join(lib_name).exists()
    }

    /// Detect the runtime library location.
    ///
    /// Discovery strategy (like rustc's sysroot):
    /// 1. Same directory as executable (dev builds: `target/release/`)
    /// 2. Installed layout: `<exe>/../lib/libori_rt.a`
    /// 3. Workspace directory (via `ORI_WORKSPACE_DIR` for `cargo run`)
    ///
    /// No environment variables are used for primary discovery.
    /// Use `--runtime-path` CLI flag for explicit overrides.
    ///
    /// # Errors
    ///
    /// Returns `RuntimeNotFound` if the library cannot be found.
    pub fn detect() -> Result<Self, RuntimeNotFound> {
        let mut searched = Vec::new();
        let lib_name = Self::lib_name();

        // 1. Check relative to current executable (like rustc's sysroot discovery)
        if let Ok(exe_path) = std::env::current_exe() {
            // Canonicalize to resolve symlinks (like rustc does)
            let exe_path = exe_path.canonicalize().unwrap_or(exe_path);

            if let Some(exe_dir) = exe_path.parent() {
                // Dev layout: same directory as executable (target/release/)
                // This is the most common case during development
                if Self::lib_exists(exe_dir, lib_name) {
                    return Ok(Self::new(exe_dir.to_path_buf()));
                }
                searched.push(exe_dir.to_path_buf());

                // Installed layout: bin/ori -> ../lib/libori_rt.a
                // Standard FHS: /usr/local/bin/ori -> /usr/local/lib/libori_rt.a
                let lib_path = exe_dir.join("../lib");
                if Self::lib_exists(&lib_path, lib_name) {
                    return Ok(Self::new(lib_path.canonicalize().unwrap_or(lib_path)));
                }
                searched.push(lib_path);
            }
        }

        // 2. Check workspace directory (for `cargo run` during development)
        // ORI_WORKSPACE_DIR is set by the build system when running via cargo
        if let Ok(workspace) = std::env::var("ORI_WORKSPACE_DIR") {
            for profile in ["release", "debug"] {
                let path = PathBuf::from(&workspace).join("target").join(profile);
                if Self::lib_exists(&path, lib_name) {
                    return Ok(Self::new(path));
                }
                searched.push(path);
            }
        }

        Err(RuntimeNotFound {
            searched_paths: searched,
        })
    }

    /// Set static linking (default: true).
    #[must_use]
    pub fn static_linking(mut self, enable: bool) -> Self {
        self.static_link = enable;
        self
    }

    /// Configure link input with runtime library.
    ///
    /// Adds the library path and library to the link input.
    pub fn configure_link(&self, input: &mut LinkInput) {
        // Add library search path
        input.library_paths.push(self.library_path.clone());

        // Add the runtime library
        let lib = if self.static_link {
            LinkLibrary::new("ori_rt").static_lib()
        } else {
            LinkLibrary::new("ori_rt").dynamic_lib()
        };
        input.libraries.push(lib);

        // On Unix, we also need libc and libm
        #[cfg(unix)]
        {
            input.libraries.push(LinkLibrary::new("c"));
            input.libraries.push(LinkLibrary::new("m"));
            input.libraries.push(LinkLibrary::new("pthread"));
        }
    }

    /// Get the library kind based on static/dynamic setting.
    #[must_use]
    pub fn library_kind(&self) -> LibraryKind {
        if self.static_link {
            LibraryKind::Static
        } else {
            LibraryKind::Dynamic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_config_new() {
        let config = RuntimeConfig::new(PathBuf::from("/usr/lib"));
        assert_eq!(config.library_path, PathBuf::from("/usr/lib"));
        assert!(config.static_link);
    }

    #[test]
    fn test_runtime_config_static_linking() {
        let config = RuntimeConfig::new(PathBuf::from("/usr/lib")).static_linking(false);
        assert!(!config.static_link);
        assert_eq!(config.library_kind(), LibraryKind::Dynamic);
    }

    #[test]
    fn test_configure_link() {
        let config = RuntimeConfig::new(PathBuf::from("/opt/ori/lib"));
        let mut input = LinkInput::default();

        config.configure_link(&mut input);

        assert!(input.library_paths.contains(&PathBuf::from("/opt/ori/lib")));
        assert!(input.libraries.iter().any(|l| l.name == "ori_rt"));
    }

    #[test]
    fn test_lib_name_unix() {
        // On Unix systems, should be libori_rt.a
        #[cfg(unix)]
        assert_eq!(RuntimeConfig::lib_name(), "libori_rt.a");
    }

    #[test]
    fn test_runtime_not_found_display() {
        let err = RuntimeNotFound {
            searched_paths: vec![PathBuf::from("/path/1"), PathBuf::from("/path/2")],
        };

        let msg = err.to_string();
        assert!(msg.contains(RuntimeConfig::lib_name()));
        assert!(msg.contains("/path/1"));
        assert!(msg.contains("/path/2"));
        assert!(msg.contains("cargo build -p ori_rt"));
        assert!(msg.contains("--runtime-path"));
        // Should NOT mention env vars anymore
        assert!(!msg.contains("ORI_RT_PATH"));
        assert!(!msg.contains("ORI_LIB_DIR"));
    }
}
