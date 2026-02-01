//! Runtime Library Configuration for AOT Compilation
//!
//! This module provides configuration for linking the Ori runtime library
//! (`libori_rt`) with AOT-compiled programs.
//!
//! # Runtime Library Location
//!
//! The runtime library can be found in several locations:
//!
//! 1. **Environment variable**: `ORI_RT_PATH` - explicit path to `libori_rt.a`
//! 2. **Relative to compiler**: `../ori_rt/target/release/libori_rt.a`
//! 3. **System install**: `/usr/local/lib/ori/libori_rt.a` or similar
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

use std::path::PathBuf;

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
        writeln!(f, "Ori runtime library (libori_rt.a) not found.")?;
        writeln!(f, "Searched paths:")?;
        for path in &self.searched_paths {
            writeln!(f, "  - {}", path.display())?;
        }
        writeln!(f)?;
        writeln!(f, "To fix this, either:")?;
        writeln!(
            f,
            "  1. Set ORI_RT_PATH environment variable to the path containing libori_rt.a"
        )?;
        writeln!(f, "  2. Build the runtime: cargo build -p ori_rt --release")?;
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

    /// Detect the runtime library location.
    ///
    /// Searches in order:
    /// 1. `ORI_RT_PATH` environment variable
    /// 2. Relative to the current executable
    /// 3. Common system locations
    ///
    /// # Errors
    ///
    /// Returns `RuntimeNotFound` if the library cannot be found.
    pub fn detect() -> Result<Self, RuntimeNotFound> {
        let mut searched = Vec::new();

        // 1. Check ORI_RT_PATH environment variable
        if let Ok(path) = std::env::var("ORI_RT_PATH") {
            let path = PathBuf::from(path);
            if path.join("libori_rt.a").exists() || path.join("ori_rt.lib").exists() {
                return Ok(Self::new(path));
            }
            searched.push(path);
        }

        // 2. Check relative to compiler build directory
        // When running from cargo, the target directory is at the workspace root
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let release_path = PathBuf::from(&manifest_dir).join("../ori_llvm/target/release");
            if release_path.join("libori_rt.a").exists() {
                return Ok(Self::new(release_path));
            }
            searched.push(release_path);

            let debug_path = PathBuf::from(&manifest_dir).join("../ori_llvm/target/debug");
            if debug_path.join("libori_rt.a").exists() {
                return Ok(Self::new(debug_path));
            }
            searched.push(debug_path);
        }

        // 3. Check relative to current executable
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Check same directory as executable
                if exe_dir.join("libori_rt.a").exists() {
                    return Ok(Self::new(exe_dir.to_path_buf()));
                }
                searched.push(exe_dir.to_path_buf());

                // Check ../lib relative to executable
                let lib_path = exe_dir.join("../lib");
                if lib_path.join("libori_rt.a").exists() {
                    return Ok(Self::new(lib_path));
                }
                searched.push(lib_path);
            }
        }

        // 4. Check common system locations
        let system_paths = [
            PathBuf::from("/usr/local/lib/ori"),
            PathBuf::from("/usr/lib/ori"),
            PathBuf::from("/opt/ori/lib"),
        ];

        for path in system_paths {
            if path.join("libori_rt.a").exists() {
                return Ok(Self::new(path));
            }
            searched.push(path);
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
    fn test_runtime_not_found_display() {
        let err = RuntimeNotFound {
            searched_paths: vec![PathBuf::from("/path/1"), PathBuf::from("/path/2")],
        };

        let msg = err.to_string();
        assert!(msg.contains("libori_rt.a"));
        assert!(msg.contains("/path/1"));
        assert!(msg.contains("/path/2"));
        assert!(msg.contains("ORI_RT_PATH"));
    }
}
