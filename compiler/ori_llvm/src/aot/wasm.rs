//! WebAssembly-specific configuration and code generation.
//!
//! This module provides WASM-specific functionality beyond basic target support:
//! - Memory configuration (import/export, initial/max size)
//! - JavaScript binding generation
//! - TypeScript declaration generation
//! - WASI support
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │   LLVM IR   │───▶│  WASM Emit  │───▶│  .wasm file │
//! │  (Module)   │    │  (wasm-ld)  │    │             │
//! └─────────────┘    └──────┬──────┘    └──────┬──────┘
//!                           │                  │
//!                    ┌──────▼──────┐    ┌──────▼──────┐
//!                    │  .js glue   │    │  .d.ts decl │
//!                    │  (optional) │    │  (optional) │
//!                    └─────────────┘    └─────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::wasm::{WasmConfig, WasmMemoryConfig, JsBindingGenerator};
//!
//! let config = WasmConfig::default()
//!     .with_memory(WasmMemoryConfig::default().with_initial_pages(16))
//!     .with_js_bindings(true);
//!
//! // Generate WASM with JS bindings
//! let js_gen = JsBindingGenerator::new("my_module", &exports);
//! js_gen.generate_js(Path::new("my_module.js"))?;
//! js_gen.generate_dts(Path::new("my_module.d.ts"))?;
//! ```

use std::fmt::{self, Write as _};
use std::fs;
use std::path::Path;

/// Error type for WASM-specific operations.
#[derive(Debug, Clone)]
pub enum WasmError {
    /// Failed to generate JavaScript bindings.
    JsBindingGeneration { message: String },
    /// Failed to generate TypeScript declarations.
    DtsGeneration { message: String },
    /// Failed to write output file.
    WriteError { path: String, message: String },
    /// Invalid WASM configuration.
    InvalidConfig { message: String },
}

impl fmt::Display for WasmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsBindingGeneration { message } => {
                write!(f, "failed to generate JavaScript bindings: {message}")
            }
            Self::DtsGeneration { message } => {
                write!(f, "failed to generate TypeScript declarations: {message}")
            }
            Self::WriteError { path, message } => {
                write!(f, "failed to write '{path}': {message}")
            }
            Self::InvalidConfig { message } => {
                write!(f, "invalid WASM configuration: {message}")
            }
        }
    }
}

impl std::error::Error for WasmError {}

/// WebAssembly memory configuration.
///
/// WASM memory is organized in pages of 64KB each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmMemoryConfig {
    /// Initial memory size in pages (64KB each).
    pub initial_pages: u32,
    /// Maximum memory size in pages (None = no limit).
    pub max_pages: Option<u32>,
    /// Whether memory is imported from the host.
    pub import_memory: bool,
    /// Memory import name (module, field) if imported.
    pub import_name: Option<(String, String)>,
    /// Whether memory is exported to the host.
    pub export_memory: bool,
    /// Memory export name if exported.
    pub export_name: Option<String>,
    /// Whether memory is shared (for threading).
    pub shared: bool,
}

impl Default for WasmMemoryConfig {
    fn default() -> Self {
        Self {
            // 1MB initial (16 pages * 64KB)
            initial_pages: 16,
            // 16MB max by default
            max_pages: Some(256),
            import_memory: false,
            import_name: None,
            export_memory: true,
            export_name: Some("memory".to_string()),
            shared: false,
        }
    }
}

impl WasmMemoryConfig {
    /// Set initial memory pages.
    #[must_use]
    pub fn with_initial_pages(mut self, pages: u32) -> Self {
        self.initial_pages = pages;
        self
    }

    /// Set maximum memory pages.
    #[must_use]
    pub fn with_max_pages(mut self, pages: Option<u32>) -> Self {
        self.max_pages = pages;
        self
    }

    /// Enable memory import from host environment.
    #[must_use]
    pub fn with_import(mut self, module: &str, field: &str) -> Self {
        self.import_memory = true;
        self.import_name = Some((module.to_string(), field.to_string()));
        self.export_memory = false;
        self
    }

    /// Configure memory export.
    #[must_use]
    pub fn with_export(mut self, name: &str) -> Self {
        self.export_memory = true;
        self.export_name = Some(name.to_string());
        self
    }

    /// Disable memory export.
    #[must_use]
    pub fn without_export(mut self) -> Self {
        self.export_memory = false;
        self.export_name = None;
        self
    }

    /// Enable shared memory (for threading).
    #[must_use]
    pub fn with_shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }

    /// Calculate initial memory in bytes.
    #[must_use]
    pub fn initial_bytes(&self) -> u64 {
        u64::from(self.initial_pages) * 65536
    }

    /// Calculate maximum memory in bytes.
    #[must_use]
    pub fn max_bytes(&self) -> Option<u64> {
        self.max_pages.map(|p| u64::from(p) * 65536)
    }

    /// Generate wasm-ld arguments for this memory configuration.
    #[must_use]
    pub fn linker_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        args.push(format!("--initial-memory={}", self.initial_bytes()));

        if let Some(max) = self.max_bytes() {
            args.push(format!("--max-memory={max}"));
        }

        if self.import_memory {
            args.push("--import-memory".to_string());
        }

        if self.export_memory {
            args.push("--export-memory".to_string());
        }

        if self.shared {
            args.push("--shared-memory".to_string());
        }

        args
    }
}

/// WebAssembly stack configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmStackConfig {
    /// Stack size in bytes.
    pub size: u32,
}

impl Default for WasmStackConfig {
    fn default() -> Self {
        Self {
            // 1MB stack (reasonable default)
            size: 1024 * 1024,
        }
    }
}

impl WasmStackConfig {
    /// Set stack size in bytes.
    #[must_use]
    pub fn with_size(mut self, bytes: u32) -> Self {
        self.size = bytes;
        self
    }

    /// Set stack size in kilobytes.
    #[must_use]
    pub fn with_size_kb(self, kb: u32) -> Self {
        self.with_size(kb * 1024)
    }

    /// Generate wasm-ld arguments for this stack configuration.
    #[must_use]
    pub fn linker_args(&self) -> Vec<String> {
        vec![format!("--stack-size={}", self.size)]
    }
}

/// WebAssembly feature flags.
///
/// This struct intentionally uses boolean fields for feature flags,
/// as each flag is independent and this pattern is standard for
/// compiler/linker feature configuration.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct WasmFeatures {
    /// Enable bulk memory operations (faster memcpy/memset).
    pub bulk_memory: bool,
    /// Enable reference types.
    pub reference_types: bool,
    /// Enable multi-value returns.
    pub multi_value: bool,
    /// Enable SIMD instructions.
    pub simd: bool,
    /// Enable exception handling.
    pub exception_handling: bool,
}

impl WasmFeatures {
    /// Create default features (`bulk_memory` and `multi_value` enabled).
    #[must_use]
    pub fn default_enabled() -> Self {
        Self {
            bulk_memory: true,
            multi_value: true,
            ..Self::default()
        }
    }

    /// Generate wasm-ld arguments for these features.
    #[must_use]
    pub fn linker_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if self.bulk_memory {
            args.push("--enable-bulk-memory".to_string());
        }

        if self.reference_types {
            args.push("--enable-reference-types".to_string());
        }

        if self.multi_value {
            args.push("--enable-multivalue".to_string());
        }

        if self.simd {
            args.push("--enable-simd".to_string());
        }

        if self.exception_handling {
            args.push("--enable-exception-handling".to_string());
        }

        args
    }
}

/// Output generation options.
#[derive(Debug, Clone, Default)]
pub struct WasmOutputOptions {
    /// Generate JavaScript binding glue code.
    pub generate_js_bindings: bool,
    /// Generate TypeScript declarations.
    pub generate_dts: bool,
    /// Run wasm-opt post-processor.
    pub run_wasm_opt: bool,
    /// wasm-opt optimization level (0-4, or "s"/"z" for size).
    pub wasm_opt_level: WasmOptLevel,
}

/// Comprehensive WebAssembly build configuration.
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Memory configuration.
    pub memory: WasmMemoryConfig,
    /// Stack configuration.
    pub stack: WasmStackConfig,
    /// Output generation options.
    pub output: WasmOutputOptions,
    /// Enable WASI support.
    pub wasi: bool,
    /// WASI-specific configuration (when wasi is true).
    pub wasi_config: Option<WasiConfig>,
    /// WebAssembly feature flags.
    pub features: WasmFeatures,
}

impl WasmConfig {
    /// Check if JS bindings should be generated.
    #[must_use]
    pub fn generate_js_bindings(&self) -> bool {
        self.output.generate_js_bindings
    }

    /// Check if TypeScript declarations should be generated.
    #[must_use]
    pub fn generate_dts(&self) -> bool {
        self.output.generate_dts
    }

    /// Check if wasm-opt should run.
    #[must_use]
    pub fn run_wasm_opt(&self) -> bool {
        self.output.run_wasm_opt
    }

    /// Get wasm-opt optimization level.
    #[must_use]
    pub fn wasm_opt_level(&self) -> WasmOptLevel {
        self.output.wasm_opt_level
    }

    /// Check if bulk memory is enabled.
    #[must_use]
    pub fn bulk_memory(&self) -> bool {
        self.features.bulk_memory
    }

    /// Check if reference types are enabled.
    #[must_use]
    pub fn reference_types(&self) -> bool {
        self.features.reference_types
    }

    /// Check if multi-value is enabled.
    #[must_use]
    pub fn multi_value(&self) -> bool {
        self.features.multi_value
    }

    /// Check if SIMD is enabled.
    #[must_use]
    pub fn simd(&self) -> bool {
        self.features.simd
    }

    /// Check if exception handling is enabled.
    #[must_use]
    pub fn exception_handling(&self) -> bool {
        self.features.exception_handling
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            memory: WasmMemoryConfig::default(),
            stack: WasmStackConfig::default(),
            output: WasmOutputOptions::default(),
            wasi: false,
            wasi_config: None,
            features: WasmFeatures::default_enabled(),
        }
    }
}

impl WasmConfig {
    /// Create configuration for standalone WASM (wasm32-unknown-unknown).
    #[must_use]
    pub fn standalone() -> Self {
        Self::default()
    }

    /// Create configuration for WASI (wasm32-wasi).
    #[must_use]
    pub fn wasi() -> Self {
        Self {
            wasi: true,
            wasi_config: Some(WasiConfig::default()),
            ..Self::default()
        }
    }

    /// Create configuration for WASI CLI applications.
    #[must_use]
    pub fn wasi_cli() -> Self {
        Self {
            wasi: true,
            wasi_config: Some(WasiConfig::cli()),
            ..Self::default()
        }
    }

    /// Create minimal WASI configuration (no filesystem).
    #[must_use]
    pub fn wasi_minimal() -> Self {
        Self {
            wasi: true,
            wasi_config: Some(WasiConfig::minimal()),
            ..Self::default()
        }
    }

    /// Create configuration for browser embedding with JS bindings.
    #[must_use]
    pub fn browser() -> Self {
        Self {
            output: WasmOutputOptions {
                generate_js_bindings: true,
                generate_dts: true,
                ..WasmOutputOptions::default()
            },
            features: WasmFeatures {
                bulk_memory: true,
                ..WasmFeatures::default()
            },
            ..Self::default()
        }
    }

    /// Set memory configuration.
    #[must_use]
    pub fn with_memory(mut self, memory: WasmMemoryConfig) -> Self {
        self.memory = memory;
        self
    }

    /// Set stack configuration.
    #[must_use]
    pub fn with_stack(mut self, stack: WasmStackConfig) -> Self {
        self.stack = stack;
        self
    }

    /// Enable JavaScript binding generation.
    #[must_use]
    pub fn with_js_bindings(mut self, enable: bool) -> Self {
        self.output.generate_js_bindings = enable;
        self
    }

    /// Enable TypeScript declaration generation.
    #[must_use]
    pub fn with_dts(mut self, enable: bool) -> Self {
        self.output.generate_dts = enable;
        self
    }

    /// Enable WASI support.
    #[must_use]
    pub fn with_wasi(mut self, enable: bool) -> Self {
        self.wasi = enable;
        if enable && self.wasi_config.is_none() {
            self.wasi_config = Some(WasiConfig::default());
        }
        self
    }

    /// Configure WASI settings.
    #[must_use]
    pub fn with_wasi_config(mut self, config: WasiConfig) -> Self {
        self.wasi = true;
        self.wasi_config = Some(config);
        self
    }

    /// Enable wasm-opt post-processing.
    #[must_use]
    pub fn with_wasm_opt(mut self, level: WasmOptLevel) -> Self {
        self.output.run_wasm_opt = true;
        self.output.wasm_opt_level = level;
        self
    }

    /// Enable bulk memory operations.
    #[must_use]
    pub fn with_bulk_memory(mut self, enable: bool) -> Self {
        self.features.bulk_memory = enable;
        self
    }

    /// Enable SIMD instructions.
    #[must_use]
    pub fn with_simd(mut self, enable: bool) -> Self {
        self.features.simd = enable;
        self
    }

    /// Enable reference types.
    #[must_use]
    pub fn with_reference_types(mut self, enable: bool) -> Self {
        self.features.reference_types = enable;
        self
    }

    /// Enable exception handling.
    #[must_use]
    pub fn with_exception_handling(mut self, enable: bool) -> Self {
        self.features.exception_handling = enable;
        self
    }

    /// Get all linker arguments for this configuration.
    #[must_use]
    pub fn linker_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Memory configuration
        args.extend(self.memory.linker_args());

        // Stack configuration
        args.extend(self.stack.linker_args());

        // Feature flags
        args.extend(self.features.linker_args());

        args
    }
}

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
    /// Get the target triple suffix for this WASI version.
    #[must_use]
    pub fn target_suffix(&self) -> &'static str {
        match self {
            Self::Preview1 => "wasi",
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
#[allow(clippy::struct_excessive_bools)]
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

/// Information about an exported WASM function for binding generation.
#[derive(Debug, Clone)]
pub struct WasmExport {
    /// Ori function name (e.g., "add").
    pub ori_name: String,
    /// WASM export name (e.g., `_ori_add_ii`).
    pub wasm_name: String,
    /// Parameter types for documentation/TypeScript.
    pub params: Vec<WasmType>,
    /// Return type for documentation/TypeScript.
    pub return_type: WasmType,
    /// Whether this function is async (returns Promise in JS).
    pub is_async: bool,
}

/// WASM-level type representation for binding generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmType {
    /// 32-bit integer (Ori: int on WASM32).
    I32,
    /// 64-bit integer (Ori: int on native, not typically used on WASM).
    I64,
    /// 32-bit float.
    F32,
    /// 64-bit float (Ori: float).
    F64,
    /// Pointer to string data (i32 offset + i32 length).
    String,
    /// Pointer to list data (i32 offset + i32 length).
    List(Box<WasmType>),
    /// Void (no return value).
    Void,
    /// Opaque pointer (for complex types).
    Pointer,
}

impl WasmType {
    /// Get the TypeScript type representation.
    #[must_use]
    pub fn typescript_type(&self) -> &'static str {
        match self {
            Self::I32 | Self::I64 | Self::F32 | Self::F64 | Self::Pointer => "number",
            Self::String => "string",
            Self::List(_) => "Array<any>", // Could be more specific
            Self::Void => "void",
        }
    }

    /// Get the JavaScript `JSDoc` type annotation.
    #[must_use]
    pub fn jsdoc_type(&self) -> &'static str {
        match self {
            Self::I32 | Self::I64 | Self::F32 | Self::F64 | Self::Pointer => "number",
            Self::String => "string",
            Self::List(_) => "Array",
            Self::Void => "void",
        }
    }
}

/// Generator for JavaScript bindings and TypeScript declarations.
pub struct JsBindingGenerator {
    /// Module name (used for naming).
    pub module_name: String,
    /// Exported functions.
    pub exports: Vec<WasmExport>,
}

impl JsBindingGenerator {
    /// Create a new binding generator.
    #[must_use]
    pub fn new(module_name: &str, exports: Vec<WasmExport>) -> Self {
        Self {
            module_name: module_name.to_string(),
            exports,
        }
    }

    /// Generate JavaScript glue code.
    ///
    /// The generated code handles:
    /// - Loading and instantiating the WASM module
    /// - String marshalling (TextEncoder/TextDecoder)
    /// - Memory management helpers
    /// - Clean wrapper functions for each export
    pub fn generate_js(&self, output: &Path) -> Result<(), WasmError> {
        let mut content = String::new();

        // Header
        let _ = write!(
            content,
            r"// Auto-generated JavaScript bindings for {module_name}.wasm
// Generated by Ori compiler

/**
 * @typedef {{{{
 *   memory: WebAssembly.Memory,
 *   instance: WebAssembly.Instance,
{export_types}
 * }}}} {module_name_pascal}Module
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

let instance = null;
let memory = null;

/**
 * Allocate memory in the WASM heap.
 * @param {{number}} size - Size in bytes
 * @returns {{number}} Pointer to allocated memory
 */
function alloc(size) {{
    return instance.exports.ori_alloc(size);
}}

/**
 * Free memory in the WASM heap.
 * @param {{number}} ptr - Pointer to free
 */
function free(ptr) {{
    instance.exports.ori_free(ptr);
}}

/**
 * Encode a string to WASM memory.
 * @param {{string}} str - String to encode
 * @returns {{{{ptr: number, len: number}}}} Pointer and length
 */
function encodeString(str) {{
    const bytes = encoder.encode(str);
    const ptr = alloc(bytes.length);
    const view = new Uint8Array(memory.buffer, ptr, bytes.length);
    view.set(bytes);
    return {{ ptr, len: bytes.length }};
}}

/**
 * Decode a string from WASM memory.
 * @param {{number}} ptr - Pointer to string data
 * @param {{number}} len - Length in bytes
 * @returns {{string}} Decoded string
 */
function decodeString(ptr, len) {{
    const bytes = new Uint8Array(memory.buffer, ptr, len);
    return decoder.decode(bytes);
}}

",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
            export_types = self.generate_jsdoc_export_types(),
        );

        // Generate wrapper functions
        for export in &self.exports {
            content.push_str(&Self::generate_js_wrapper(export));
            content.push('\n');
        }

        // Init function
        content.push_str(&self.generate_js_init());

        // Export
        let _ = write!(
            content,
            r"
export {{ init, {exports} }};
export default {{ init, {exports} }};
",
            exports = self
                .exports
                .iter()
                .map(|e| e.ori_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Write file
        fs::write(output, content).map_err(|e| WasmError::WriteError {
            path: output.to_string_lossy().into_owned(),
            message: e.to_string(),
        })
    }

    /// Generate TypeScript declaration file.
    pub fn generate_dts(&self, output: &Path) -> Result<(), WasmError> {
        let mut content = String::new();

        // Header
        let _ = write!(
            content,
            r"// Auto-generated TypeScript declarations for {module_name}.wasm
// Generated by Ori compiler

export interface {module_name_pascal}Module {{
    memory: WebAssembly.Memory;
    instance: WebAssembly.Instance;
}}

/**
 * Initialize the WASM module.
 * @param url - URL or path to the .wasm file
 * @param imports - Optional additional imports
 */
export function init(
    url?: string | URL | Response | BufferSource,
    imports?: WebAssembly.Imports
): Promise<{module_name_pascal}Module>;

",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
        );

        // Generate function declarations
        for export in &self.exports {
            content.push_str(&Self::generate_dts_function(export));
            content.push('\n');
        }

        // Write file
        fs::write(output, content).map_err(|e| WasmError::WriteError {
            path: output.to_string_lossy().into_owned(),
            message: e.to_string(),
        })
    }

    fn generate_jsdoc_export_types(&self) -> String {
        self.exports
            .iter()
            .map(|e| {
                let params = e
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("arg{}: {}", i, t.jsdoc_type()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    " *   {}: ({}) => {}",
                    e.ori_name,
                    params,
                    e.return_type.jsdoc_type()
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    fn generate_js_wrapper(export: &WasmExport) -> String {
        let mut code = String::new();

        // JSDoc
        code.push_str("/**\n");
        for (i, param) in export.params.iter().enumerate() {
            let _ = writeln!(code, " * @param {{{}}} arg{}", param.jsdoc_type(), i);
        }
        let _ = writeln!(code, " * @returns {{{}}}", export.return_type.jsdoc_type());
        code.push_str(" */\n");

        // Function signature
        let params = (0..export.params.len())
            .map(|i| format!("arg{i}"))
            .collect::<Vec<_>>()
            .join(", ");

        let _ = writeln!(code, "export function {}({}) {{", export.ori_name, params);

        // Check initialization
        code.push_str(
            "    if (!instance) throw new Error('Module not initialized. Call init() first.');\n",
        );

        // Handle string parameters
        let mut cleanup = Vec::new();
        let mut call_args = Vec::new();

        for (i, param) in export.params.iter().enumerate() {
            if *param == WasmType::String {
                let _ = writeln!(code, "    const _str{i} = encodeString(arg{i});");
                call_args.push(format!("_str{i}.ptr, _str{i}.len"));
                cleanup.push(format!("_str{i}.ptr"));
            } else {
                call_args.push(format!("arg{i}"));
            }
        }

        // Make the call
        let call = format!(
            "instance.exports.{}({})",
            export.wasm_name,
            call_args.join(", ")
        );

        if export.return_type == WasmType::Void {
            let _ = writeln!(code, "    {call};");
        } else if export.return_type == WasmType::String {
            let _ = writeln!(code, "    const _result = {call};");
            code.push_str("    // TODO: Decode string result from WASM memory\n");
            code.push_str("    return _result;\n");
        } else {
            let _ = writeln!(code, "    const _result = {call};");
            // Cleanup allocated strings
            for ptr in &cleanup {
                let _ = writeln!(code, "    free({ptr});");
            }
            code.push_str("    return _result;\n");
        }

        code.push_str("}\n");
        code
    }

    fn generate_js_init(&self) -> String {
        format!(
            r"
/**
 * Initialize the WASM module.
 * @param {{string | URL | Response | BufferSource}} [url='{module_name}.wasm'] - URL or source
 * @param {{WebAssembly.Imports}} [imports={{}}] - Additional imports
 * @returns {{Promise<{module_name_pascal}Module>}}
 */
export async function init(url = '{module_name}.wasm', imports = {{}}) {{
    let source;

    if (url instanceof Response) {{
        source = url;
    }} else if (url instanceof ArrayBuffer || ArrayBuffer.isView(url)) {{
        source = url;
    }} else {{
        source = fetch(url);
    }}

    const wasmImports = {{
        env: {{
            ...imports.env,
        }},
        ...imports,
    }};

    let result;
    if (source instanceof Response || source instanceof Promise) {{
        result = await WebAssembly.instantiateStreaming(source, wasmImports);
    }} else {{
        result = await WebAssembly.instantiate(source, wasmImports);
    }}

    instance = result.instance;
    memory = instance.exports.memory;

    return {{
        memory,
        instance,
{export_props}
    }};
}}
",
            module_name = self.module_name,
            module_name_pascal = pascal_case(&self.module_name),
            export_props = self
                .exports
                .iter()
                .map(|e| format!("        {}: {},", e.ori_name, e.ori_name))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    fn generate_dts_function(export: &WasmExport) -> String {
        let params = export
            .params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{}: {}", i, t.typescript_type()))
            .collect::<Vec<_>>()
            .join(", ");

        let ret_type = if export.is_async {
            format!("Promise<{}>", export.return_type.typescript_type())
        } else {
            export.return_type.typescript_type().to_string()
        };

        format!(
            "/**\n * {}\n */\nexport function {}({}): {};\n",
            export.ori_name, export.ori_name, params, ret_type
        )
    }
}

/// Convert a string to `PascalCase`.
fn pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

// Tests extracted to: compiler/oric/tests/phases/codegen/wasm.rs
