//! Runtime Library Configuration for AOT Compilation
//!
//! This module provides configuration for linking the Ori runtime library
//! (`libori_rt`) with AOT-compiled programs.
//!
//! # Runtime Library Discovery
//!
//! Discovery follows rustc's sysroot pattern - walk up from the executable:
//!
//! 1. **Dev layout**: Same directory as compiler binary (`target/{debug,release}/libori_rt.a`)
//! 2. **Sibling profile**: `target/release/` when exe is in `target/debug/` (and vice versa)
//! 3. **Standalone `ori_rt` build**: `compiler/ori_rt/target/{release,debug}/libori_rt.a`
//! 4. **Installed layout**: `<exe>/../lib/libori_rt.a` (e.g., `/usr/local/bin/ori` → `/usr/local/lib/`)
//! 5. **Workspace dev**: `$ORI_WORKSPACE_DIR/target/{release,debug}/libori_rt.a`
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

                // Sibling profile: target/debug/ori -> check target/release/ (and vice versa).
                // Handles the common case where `cargo bl` (debug) builds the compiler but
                // `libori_rt.a` was built in release (or vice versa).
                if let Some(target_dir) = exe_dir.parent() {
                    for profile in &["release", "debug"] {
                        let sibling = target_dir.join(profile);
                        if sibling != exe_dir && Self::lib_exists(&sibling, lib_name) {
                            return Ok(Self::new(sibling));
                        }
                        if sibling != *exe_dir {
                            searched.push(sibling);
                        }
                    }
                }

                // Installed layout: bin/ori -> ../lib/libori_rt.a
                // Standard FHS: /usr/local/bin/ori -> /usr/local/lib/libori_rt.a
                let lib_path = exe_dir.join("../lib");
                if Self::lib_exists(&lib_path, lib_name) {
                    return Ok(Self::new(lib_path.canonicalize().unwrap_or(lib_path)));
                }
                searched.push(lib_path);
            }
        }

        // 2. Check ori_rt's standalone build directory.
        // ori_rt is excluded from the workspace, so `cargo build --manifest-path
        // compiler/ori_rt/Cargo.toml` puts the staticlib in compiler/ori_rt/target/.
        // Detect the workspace root by walking up from the executable.
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_path = exe_path.canonicalize().unwrap_or(exe_path);
            // exe is at <workspace>/target/<profile>/ori → workspace = exe/../../../
            if let Some(workspace_root) = exe_path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
            {
                let ori_rt_target = workspace_root.join("compiler/ori_rt/target");
                for profile in ["release", "debug"] {
                    let path = ori_rt_target.join(profile);
                    if Self::lib_exists(&path, lib_name) {
                        return Ok(Self::new(path));
                    }
                    searched.push(path);
                }
            }
        }

        // 3. Check workspace directory (for `cargo run` during development)
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
