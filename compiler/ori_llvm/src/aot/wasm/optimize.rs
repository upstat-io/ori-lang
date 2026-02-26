//! wasm-opt integration for post-processing WASM binaries.
//!
//! This module provides `WasmOptLevel` for selecting optimization intensity
//! and `WasmOptRunner` for invoking the Binaryen `wasm-opt` tool.

use std::fs;
use std::path::Path;

use super::WasmError;

/// Optimization level for wasm-opt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WasmOptLevel {
    /// No optimization.
    O0,
    /// Basic optimization.
    O1,
    /// Standard optimization.
    #[default]
    O2,
    /// Aggressive optimization.
    O3,
    /// Super-aggressive optimization.
    O4,
    /// Optimize for size.
    Os,
    /// Aggressively optimize for size.
    Oz,
}

impl WasmOptLevel {
    /// Get the command-line flag for this optimization level.
    #[must_use]
    pub fn flag(&self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O1 => "-O1",
            Self::O2 => "-O2",
            Self::O3 => "-O3",
            Self::O4 => "-O4",
            Self::Os => "-Os",
            Self::Oz => "-Oz",
        }
    }
}

/// Runs `wasm-opt` on a WASM file for additional optimization.
///
/// wasm-opt is the Binaryen optimizer that can perform WASM-specific
/// optimizations beyond what LLVM provides.
pub struct WasmOptRunner {
    /// Path to wasm-opt binary.
    pub wasm_opt_path: std::path::PathBuf,
    /// Optimization level.
    pub level: WasmOptLevel,
    /// Enable debug names (for debugging).
    pub debug_names: bool,
    /// Enable source maps.
    pub source_map: bool,
    /// Additional features to enable.
    pub features: Vec<String>,
}

impl Default for WasmOptRunner {
    fn default() -> Self {
        Self {
            wasm_opt_path: std::path::PathBuf::from("wasm-opt"),
            level: WasmOptLevel::O2,
            debug_names: false,
            source_map: false,
            features: Vec::new(),
        }
    }
}

impl WasmOptRunner {
    /// Create a new wasm-opt runner with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a runner with a specific optimization level.
    #[must_use]
    pub fn with_level(level: WasmOptLevel) -> Self {
        Self {
            level,
            ..Self::default()
        }
    }

    /// Set the path to the wasm-opt binary.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.wasm_opt_path = path.into();
        self
    }

    /// Set the optimization level.
    #[must_use]
    pub fn with_opt_level(mut self, level: WasmOptLevel) -> Self {
        self.level = level;
        self
    }

    /// Enable debug names in output.
    #[must_use]
    pub fn with_debug_names(mut self, enable: bool) -> Self {
        self.debug_names = enable;
        self
    }

    /// Enable source map output.
    #[must_use]
    pub fn with_source_map(mut self, enable: bool) -> Self {
        self.source_map = enable;
        self
    }

    /// Enable a WASM feature.
    #[must_use]
    pub fn with_feature(mut self, feature: &str) -> Self {
        self.features.push(format!("--enable-{feature}"));
        self
    }

    /// Build the command to run wasm-opt.
    #[must_use]
    pub fn build_command(&self, input: &Path, output: &Path) -> std::process::Command {
        let mut cmd = std::process::Command::new(&self.wasm_opt_path);

        // Optimization level
        cmd.arg(self.level.flag());

        // Input and output
        cmd.arg(input);
        cmd.arg("-o").arg(output);

        // Debug options
        if self.debug_names {
            cmd.arg("--debuginfo");
        }

        if self.source_map {
            let map_path = output.with_extension("wasm.map");
            cmd.arg("--source-map").arg(&map_path);
        }

        // Feature flags
        for feature in &self.features {
            cmd.arg(feature);
        }

        cmd
    }

    /// Run wasm-opt on the input file, writing to output.
    ///
    /// # Errors
    ///
    /// Returns an error if wasm-opt is not found or fails.
    pub fn run(&self, input: &Path, output: &Path) -> Result<(), WasmError> {
        let mut cmd = self.build_command(input, output);

        let status = cmd.status().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                WasmError::InvalidConfig {
                    message: format!(
                        "wasm-opt not found at '{}'. Install Binaryen or specify path with --wasm-opt-path",
                        self.wasm_opt_path.display()
                    ),
                }
            } else {
                WasmError::WriteError {
                    path: output.to_string_lossy().into_owned(),
                    message: e.to_string(),
                }
            }
        })?;

        if !status.success() {
            return Err(WasmError::InvalidConfig {
                message: format!(
                    "wasm-opt failed with exit code {}",
                    status.code().unwrap_or(-1)
                ),
            });
        }

        Ok(())
    }

    /// Run wasm-opt in-place (modifies the input file).
    ///
    /// This creates a temporary file, runs optimization, then replaces the original.
    pub fn run_in_place(&self, file: &Path) -> Result<(), WasmError> {
        let temp = file.with_extension("wasm.tmp");
        self.run(file, &temp)?;

        // Replace original with optimized
        fs::rename(&temp, file).map_err(|e| WasmError::WriteError {
            path: file.to_string_lossy().into_owned(),
            message: format!("failed to replace with optimized file: {e}"),
        })
    }

    /// Check if wasm-opt is available.
    #[must_use]
    pub fn is_available(&self) -> bool {
        std::process::Command::new(&self.wasm_opt_path)
            .arg("--version")
            .output()
            .is_ok()
    }
}
