//! WASI (WebAssembly System Interface) configuration.
//!
//! This module provides WASI-specific types for configuring system interface
//! capabilities: filesystem access, clock, random, environment variables,
//! preopened directories, and undefined symbol generation.

use std::fs;
use std::path::Path;

use super::WasmError;

/// WASI version/preview level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WasiVersion {
    /// WASI Preview 1 (stable, widely supported).
    #[default]
    Preview1,
    /// WASI Preview 2 (component model, newer).
    Preview2,
}

impl WasiVersion {
    /// Get the target triple OS suffix for this WASI version.
    ///
    /// Returns the modern Rust 1.78+ canonical naming: `wasip1` for
    /// Preview 1 (historically spelled `wasi`, deprecated upstream in
    /// May 2024) and `wasip2` for Preview 2. The change disambiguates
    /// the two preview tracks now that both ship side-by-side in the
    /// upstream WASI specification.
    #[must_use]
    pub fn target_suffix(&self) -> &'static str {
        match self {
            Self::Preview1 => "wasip1",
            Self::Preview2 => "wasip2",
        }
    }
}

/// WASI-specific configuration.
///
/// WASI (WebAssembly System Interface) provides standardized system call
/// interfaces for WASM modules running in compatible runtimes.
///
/// This struct intentionally uses boolean fields for capability flags,
/// as each capability is independent and this pattern is standard for
/// WASI configuration.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent WASI capability flags; each is a genuine on/off toggle"
)]
pub struct WasiConfig {
    /// WASI version to target.
    pub version: WasiVersion,
    /// Enable filesystem access.
    pub filesystem: bool,
    /// Enable clock/time access.
    pub clock: bool,
    /// Enable random number generation.
    pub random: bool,
    /// Enable environment variable access.
    pub env: bool,
    /// Enable command-line argument access.
    pub args: bool,
    /// Preopened directories (mapped paths).
    pub preopens: Vec<WasiPreopen>,
    /// Environment variables to set.
    pub env_vars: Vec<(String, String)>,
    /// Command-line arguments.
    pub argv: Vec<String>,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            version: WasiVersion::Preview1,
            filesystem: true,
            clock: true,
            random: true,
            env: true,
            args: true,
            preopens: Vec::new(),
            env_vars: Vec::new(),
            argv: Vec::new(),
        }
    }
}

impl WasiConfig {
    /// Create a minimal WASI configuration (no filesystem access).
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            filesystem: false,
            clock: true,
            random: true,
            env: false,
            args: false,
            ..Self::default()
        }
    }

    /// Create configuration for CLI applications.
    #[must_use]
    pub fn cli() -> Self {
        Self {
            filesystem: true,
            clock: true,
            random: true,
            env: true,
            args: true,
            ..Self::default()
        }
    }

    /// Add a preopened directory mapping.
    #[must_use]
    pub fn with_preopen(mut self, guest_path: &str, host_path: &str) -> Self {
        self.preopens.push(WasiPreopen {
            guest_path: guest_path.to_string(),
            host_path: host_path.to_string(),
        });
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.push((key.to_string(), value.to_string()));
        self
    }

    /// Set command-line arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.argv = args;
        self
    }

    /// Generate WASI import list for undefined symbols file.
    ///
    /// This generates a list of WASI function imports that should be
    /// allowed as undefined during linking (they'll be provided by the runtime).
    #[must_use]
    pub fn undefined_symbols(&self) -> Vec<&'static str> {
        let mut symbols = Vec::new();

        // Core WASI imports (always needed)
        symbols.extend_from_slice(&[
            "wasi_snapshot_preview1.proc_exit",
            "wasi_snapshot_preview1.fd_write",
            "wasi_snapshot_preview1.fd_read",
            "wasi_snapshot_preview1.fd_close",
        ]);

        if self.filesystem {
            symbols.extend_from_slice(&[
                "wasi_snapshot_preview1.path_open",
                "wasi_snapshot_preview1.path_create_directory",
                "wasi_snapshot_preview1.path_remove_directory",
                "wasi_snapshot_preview1.path_unlink_file",
                "wasi_snapshot_preview1.path_rename",
                "wasi_snapshot_preview1.path_readlink",
                "wasi_snapshot_preview1.path_symlink",
                "wasi_snapshot_preview1.path_filestat_get",
                "wasi_snapshot_preview1.path_filestat_set_times",
                "wasi_snapshot_preview1.fd_prestat_get",
                "wasi_snapshot_preview1.fd_prestat_dir_name",
                "wasi_snapshot_preview1.fd_seek",
                "wasi_snapshot_preview1.fd_tell",
                "wasi_snapshot_preview1.fd_sync",
                "wasi_snapshot_preview1.fd_datasync",
                "wasi_snapshot_preview1.fd_filestat_get",
                "wasi_snapshot_preview1.fd_filestat_set_size",
                "wasi_snapshot_preview1.fd_filestat_set_times",
                "wasi_snapshot_preview1.fd_readdir",
                "wasi_snapshot_preview1.fd_renumber",
                "wasi_snapshot_preview1.fd_allocate",
                "wasi_snapshot_preview1.fd_advise",
                "wasi_snapshot_preview1.fd_pread",
                "wasi_snapshot_preview1.fd_pwrite",
            ]);
        }

        if self.clock {
            symbols.extend_from_slice(&[
                "wasi_snapshot_preview1.clock_time_get",
                "wasi_snapshot_preview1.clock_res_get",
            ]);
        }

        if self.random {
            symbols.push("wasi_snapshot_preview1.random_get");
        }

        if self.env {
            symbols.extend_from_slice(&[
                "wasi_snapshot_preview1.environ_sizes_get",
                "wasi_snapshot_preview1.environ_get",
            ]);
        }

        if self.args {
            symbols.extend_from_slice(&[
                "wasi_snapshot_preview1.args_sizes_get",
                "wasi_snapshot_preview1.args_get",
            ]);
        }

        symbols
    }

    /// Write undefined symbols file for wasm-ld.
    ///
    /// The linker uses this file to know which symbols are expected
    /// to be provided by the WASI runtime.
    pub fn write_undefined_symbols(&self, path: &Path) -> Result<(), WasmError> {
        let symbols = self.undefined_symbols();
        let content = symbols.join("\n");
        fs::write(path, content).map_err(|e| WasmError::WriteError {
            path: path.to_string_lossy().into_owned(),
            message: e.to_string(),
        })
    }
}

/// A preopened directory mapping for WASI.
#[derive(Debug, Clone)]
pub struct WasiPreopen {
    /// Path as seen by the WASM module.
    pub guest_path: String,
    /// Actual host filesystem path.
    pub host_path: String,
}
