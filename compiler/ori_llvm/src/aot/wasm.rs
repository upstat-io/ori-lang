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
    wasm_opt_path: std::path::PathBuf,
    /// Optimization level.
    level: WasmOptLevel,
    /// Enable debug names (for debugging).
    debug_names: bool,
    /// Enable source maps.
    source_map: bool,
    /// Additional features to enable.
    features: Vec<String>,
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
    module_name: String,
    /// Exported functions.
    exports: Vec<WasmExport>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_config_default() {
        let config = WasmMemoryConfig::default();
        assert_eq!(config.initial_pages, 16);
        assert_eq!(config.max_pages, Some(256));
        assert!(!config.import_memory);
        assert!(config.export_memory);
    }

    #[test]
    fn test_memory_config_bytes() {
        let config = WasmMemoryConfig::default();
        assert_eq!(config.initial_bytes(), 16 * 65536); // 1MB
        assert_eq!(config.max_bytes(), Some(256 * 65536)); // 16MB
    }

    #[test]
    fn test_memory_config_linker_args() {
        let config = WasmMemoryConfig::default();
        let args = config.linker_args();
        assert!(args.contains(&"--initial-memory=1048576".to_string()));
        assert!(args.contains(&"--max-memory=16777216".to_string()));
        assert!(args.contains(&"--export-memory".to_string()));
    }

    #[test]
    fn test_memory_config_import() {
        let config = WasmMemoryConfig::default().with_import("env", "memory");
        assert!(config.import_memory);
        assert!(!config.export_memory);
        let args = config.linker_args();
        assert!(args.contains(&"--import-memory".to_string()));
    }

    #[test]
    fn test_stack_config_default() {
        let config = WasmStackConfig::default();
        assert_eq!(config.size, 1024 * 1024); // 1MB
    }

    #[test]
    fn test_stack_config_linker_args() {
        let config = WasmStackConfig::default().with_size_kb(512);
        let args = config.linker_args();
        assert!(args.contains(&format!("--stack-size={}", 512 * 1024)));
    }

    #[test]
    fn test_wasm_config_standalone() {
        let config = WasmConfig::standalone();
        assert!(!config.wasi);
        assert!(!config.generate_js_bindings());
        assert!(config.bulk_memory());
    }

    #[test]
    fn test_wasm_config_wasi() {
        let config = WasmConfig::wasi();
        assert!(config.wasi);
    }

    #[test]
    fn test_wasm_config_browser() {
        let config = WasmConfig::browser();
        assert!(config.generate_js_bindings());
        assert!(config.generate_dts());
    }

    #[test]
    fn test_wasm_config_linker_args() {
        let config = WasmConfig::default().with_bulk_memory(true).with_simd(true);
        let args = config.linker_args();
        assert!(args.contains(&"--enable-bulk-memory".to_string()));
        assert!(args.contains(&"--enable-simd".to_string()));
    }

    #[test]
    fn test_wasm_opt_level_flags() {
        assert_eq!(WasmOptLevel::O0.flag(), "-O0");
        assert_eq!(WasmOptLevel::O2.flag(), "-O2");
        assert_eq!(WasmOptLevel::Os.flag(), "-Os");
        assert_eq!(WasmOptLevel::Oz.flag(), "-Oz");
    }

    // wasm-opt Runner Tests

    #[test]
    fn test_wasm_opt_runner_default() {
        let runner = WasmOptRunner::default();
        assert_eq!(runner.level, WasmOptLevel::O2);
        assert!(!runner.debug_names);
        assert!(!runner.source_map);
    }

    #[test]
    fn test_wasm_opt_runner_with_level() {
        let runner = WasmOptRunner::with_level(WasmOptLevel::Oz);
        assert_eq!(runner.level, WasmOptLevel::Oz);
    }

    #[test]
    fn test_wasm_opt_runner_builder() {
        let runner = WasmOptRunner::new()
            .with_opt_level(WasmOptLevel::O3)
            .with_debug_names(true)
            .with_source_map(true)
            .with_feature("bulk-memory")
            .with_feature("simd");

        assert_eq!(runner.level, WasmOptLevel::O3);
        assert!(runner.debug_names);
        assert!(runner.source_map);
        assert_eq!(runner.features.len(), 2);
    }

    #[test]
    fn test_wasm_opt_runner_with_path() {
        let runner = WasmOptRunner::new().with_path("/usr/local/bin/wasm-opt");
        assert_eq!(
            runner.wasm_opt_path,
            std::path::PathBuf::from("/usr/local/bin/wasm-opt")
        );
    }

    #[test]
    fn test_wasm_opt_runner_build_command() {
        let runner = WasmOptRunner::new()
            .with_opt_level(WasmOptLevel::Oz)
            .with_debug_names(true)
            .with_feature("bulk-memory");

        let cmd = runner.build_command(Path::new("input.wasm"), Path::new("output.wasm"));
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"-Oz".into()));
        assert!(args.contains(&"input.wasm".into()));
        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"output.wasm".into()));
        assert!(args.contains(&"--debuginfo".into()));
        assert!(args.contains(&"--enable-bulk-memory".into()));
    }

    #[test]
    fn test_wasm_opt_runner_build_command_with_source_map() {
        let runner = WasmOptRunner::new().with_source_map(true);

        let cmd = runner.build_command(Path::new("in.wasm"), Path::new("out.wasm"));
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        assert!(args.contains(&"--source-map".into()));
    }

    #[test]
    fn test_wasm_type_typescript() {
        assert_eq!(WasmType::I32.typescript_type(), "number");
        assert_eq!(WasmType::String.typescript_type(), "string");
        assert_eq!(WasmType::Void.typescript_type(), "void");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(pascal_case("my_module"), "MyModule");
        assert_eq!(pascal_case("hello-world"), "HelloWorld");
        assert_eq!(pascal_case("test"), "Test");
        assert_eq!(pascal_case("a_b_c"), "ABC");
    }

    #[test]
    fn test_js_binding_generator_new() {
        let exports = vec![WasmExport {
            ori_name: "add".to_string(),
            wasm_name: "_ori_add_ii".to_string(),
            params: vec![WasmType::I32, WasmType::I32],
            return_type: WasmType::I32,
            is_async: false,
        }];
        let gen = JsBindingGenerator::new("my_module", exports);
        assert_eq!(gen.module_name, "my_module");
        assert_eq!(gen.exports.len(), 1);
    }

    #[test]
    fn test_wasm_error_display() {
        let err = WasmError::JsBindingGeneration {
            message: "test error".to_string(),
        };
        assert!(err.to_string().contains("JavaScript bindings"));

        let err = WasmError::DtsGeneration {
            message: "test error".to_string(),
        };
        assert!(err.to_string().contains("TypeScript declarations"));

        let err = WasmError::WriteError {
            path: "/tmp/test.js".to_string(),
            message: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("/tmp/test.js"));
    }

    // WASI Configuration Tests

    #[test]
    fn test_wasi_version_default() {
        let version = WasiVersion::default();
        assert_eq!(version, WasiVersion::Preview1);
        assert_eq!(version.target_suffix(), "wasi");
    }

    #[test]
    fn test_wasi_version_preview2() {
        let version = WasiVersion::Preview2;
        assert_eq!(version.target_suffix(), "wasip2");
    }

    #[test]
    fn test_wasi_config_default() {
        let config = WasiConfig::default();
        assert!(config.filesystem);
        assert!(config.clock);
        assert!(config.random);
        assert!(config.env);
        assert!(config.args);
        assert!(config.preopens.is_empty());
    }

    #[test]
    fn test_wasi_config_minimal() {
        let config = WasiConfig::minimal();
        assert!(!config.filesystem);
        assert!(config.clock);
        assert!(config.random);
        assert!(!config.env);
        assert!(!config.args);
    }

    #[test]
    fn test_wasi_config_cli() {
        let config = WasiConfig::cli();
        assert!(config.filesystem);
        assert!(config.clock);
        assert!(config.random);
        assert!(config.env);
        assert!(config.args);
    }

    #[test]
    fn test_wasi_config_with_preopen() {
        let config = WasiConfig::default()
            .with_preopen("/app", "/home/user/app")
            .with_preopen("/data", "/var/data");
        assert_eq!(config.preopens.len(), 2);
        assert_eq!(config.preopens[0].guest_path, "/app");
        assert_eq!(config.preopens[0].host_path, "/home/user/app");
    }

    #[test]
    fn test_wasi_config_with_env() {
        let config = WasiConfig::default()
            .with_env("HOME", "/app")
            .with_env("DEBUG", "1");
        assert_eq!(config.env_vars.len(), 2);
        assert_eq!(config.env_vars[0], ("HOME".to_string(), "/app".to_string()));
    }

    #[test]
    fn test_wasi_config_with_args() {
        let config = WasiConfig::default().with_args(vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(config.argv.len(), 2);
    }

    #[test]
    fn test_wasi_config_undefined_symbols() {
        let config = WasiConfig::default();
        let symbols = config.undefined_symbols();
        // Should contain core WASI imports
        assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
        assert!(symbols.contains(&"wasi_snapshot_preview1.fd_write"));
        // Should contain filesystem imports (enabled by default)
        assert!(symbols.contains(&"wasi_snapshot_preview1.path_open"));
        // Should contain clock imports
        assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));
        // Should contain random imports
        assert!(symbols.contains(&"wasi_snapshot_preview1.random_get"));
    }

    #[test]
    fn test_wasi_config_minimal_symbols() {
        let config = WasiConfig::minimal();
        let symbols = config.undefined_symbols();
        // Should contain core imports
        assert!(symbols.contains(&"wasi_snapshot_preview1.proc_exit"));
        // Should NOT contain filesystem imports
        assert!(!symbols.contains(&"wasi_snapshot_preview1.path_open"));
        // Should contain clock imports
        assert!(symbols.contains(&"wasi_snapshot_preview1.clock_time_get"));
    }

    #[test]
    fn test_wasm_config_wasi_cli() {
        let config = WasmConfig::wasi_cli();
        assert!(config.wasi);
        assert!(config.wasi_config.is_some());
        let wasi = config.wasi_config.unwrap();
        assert!(wasi.filesystem);
        assert!(wasi.args);
    }

    #[test]
    fn test_wasm_config_wasi_minimal() {
        let config = WasmConfig::wasi_minimal();
        assert!(config.wasi);
        assert!(config.wasi_config.is_some());
        let wasi = config.wasi_config.unwrap();
        assert!(!wasi.filesystem);
    }

    #[test]
    fn test_wasm_config_with_wasi_config() {
        let wasi = WasiConfig::default().with_preopen("/", "/tmp");
        let config = WasmConfig::default().with_wasi_config(wasi);
        assert!(config.wasi);
        assert!(config.wasi_config.is_some());
        assert_eq!(config.wasi_config.unwrap().preopens.len(), 1);
    }

    #[test]
    fn test_wasm_config_with_wasi_enables_wasi() {
        let config = WasmConfig::default().with_wasi(true);
        assert!(config.wasi);
        // Should auto-create default WASI config
        assert!(config.wasi_config.is_some());
    }

    // Additional coverage tests

    #[test]
    fn test_wasm_error_invalid_config() {
        let err = WasmError::InvalidConfig {
            message: "bad config".to_string(),
        };
        assert!(err.to_string().contains("invalid WASM configuration"));
        assert!(err.to_string().contains("bad config"));
    }

    #[test]
    fn test_wasm_type_all_variants() {
        assert_eq!(WasmType::I32.typescript_type(), "number");
        assert_eq!(WasmType::I64.typescript_type(), "number");
        assert_eq!(WasmType::F32.typescript_type(), "number");
        assert_eq!(WasmType::F64.typescript_type(), "number");
        assert_eq!(WasmType::String.typescript_type(), "string");
        assert_eq!(WasmType::Void.typescript_type(), "void");
        assert_eq!(WasmType::Pointer.typescript_type(), "number");
        assert_eq!(
            WasmType::List(Box::new(WasmType::I32)).typescript_type(),
            "Array<any>"
        );

        // JSDoc types
        assert_eq!(WasmType::I32.jsdoc_type(), "number");
        assert_eq!(WasmType::String.jsdoc_type(), "string");
        assert_eq!(WasmType::Void.jsdoc_type(), "void");
        assert_eq!(WasmType::Pointer.jsdoc_type(), "number");
        assert_eq!(
            WasmType::List(Box::new(WasmType::I32)).jsdoc_type(),
            "Array"
        );
    }

    #[test]
    fn test_memory_config_with_export() {
        let config = WasmMemoryConfig::default().with_export("mem");
        assert!(config.export_memory);
        assert_eq!(config.export_name, Some("mem".to_string()));
    }

    #[test]
    fn test_memory_config_without_export() {
        let config = WasmMemoryConfig::default().without_export();
        assert!(!config.export_memory);
        assert!(config.export_name.is_none());
    }

    #[test]
    fn test_memory_config_with_shared() {
        let config = WasmMemoryConfig::default().with_shared(true);
        assert!(config.shared);
        let args = config.linker_args();
        assert!(args.contains(&"--shared-memory".to_string()));
    }

    #[test]
    fn test_memory_config_no_max() {
        let config = WasmMemoryConfig::default().with_max_pages(None);
        assert!(config.max_pages.is_none());
        assert!(config.max_bytes().is_none());
        let args = config.linker_args();
        assert!(!args.iter().any(|a| a.contains("--max-memory")));
    }

    #[test]
    fn test_wasm_config_all_features() {
        let config = WasmConfig::default()
            .with_bulk_memory(true)
            .with_simd(true)
            .with_reference_types(true)
            .with_exception_handling(true);

        let args = config.linker_args();
        assert!(args.contains(&"--enable-bulk-memory".to_string()));
        assert!(args.contains(&"--enable-simd".to_string()));
        assert!(args.contains(&"--enable-reference-types".to_string()));
        assert!(args.contains(&"--enable-exception-handling".to_string()));
    }

    #[test]
    fn test_wasm_config_with_reference_types() {
        let config = WasmConfig::default().with_reference_types(true);
        assert!(config.reference_types());
    }

    #[test]
    fn test_wasm_config_with_exception_handling() {
        let config = WasmConfig::default().with_exception_handling(true);
        assert!(config.exception_handling());
    }

    #[test]
    fn test_js_binding_generator_generate_js() {
        use std::fs;

        let exports = vec![
            WasmExport {
                ori_name: "add".to_string(),
                wasm_name: "_ori_add_ii".to_string(),
                params: vec![WasmType::I32, WasmType::I32],
                return_type: WasmType::I32,
                is_async: false,
            },
            WasmExport {
                ori_name: "greet".to_string(),
                wasm_name: "_ori_greet_s".to_string(),
                params: vec![WasmType::String],
                return_type: WasmType::Void,
                is_async: false,
            },
        ];
        let gen = JsBindingGenerator::new("test_module", exports);

        let temp_dir = std::env::temp_dir();
        let js_path = temp_dir.join("test_ori_wasm.js");

        let result = gen.generate_js(&js_path);
        assert!(result.is_ok(), "generate_js failed: {result:?}");

        // Verify file was created and has content
        let content = fs::read_to_string(&js_path).unwrap();
        assert!(content.contains("test_module.wasm"));
        assert!(content.contains("TextEncoder"));
        assert!(content.contains("TextDecoder"));
        assert!(content.contains("function add"));
        assert!(content.contains("function greet"));
        assert!(content.contains("export { init"));

        // Clean up
        let _ = fs::remove_file(&js_path);
    }

    #[test]
    fn test_js_binding_generator_generate_dts() {
        use std::fs;

        let exports = vec![WasmExport {
            ori_name: "calculate".to_string(),
            wasm_name: "_ori_calc".to_string(),
            params: vec![WasmType::F64],
            return_type: WasmType::F64,
            is_async: true,
        }];
        let gen = JsBindingGenerator::new("calc_module", exports);

        let temp_dir = std::env::temp_dir();
        let dts_path = temp_dir.join("test_ori_wasm.d.ts");

        let result = gen.generate_dts(&dts_path);
        assert!(result.is_ok(), "generate_dts failed: {result:?}");

        // Verify file was created and has content
        let content = fs::read_to_string(&dts_path).unwrap();
        assert!(content.contains("CalcModule"));
        assert!(content.contains("WebAssembly.Memory"));
        assert!(content.contains("export function init"));
        assert!(content.contains("export function calculate"));
        assert!(content.contains("Promise<number>")); // async function

        // Clean up
        let _ = fs::remove_file(&dts_path);
    }

    #[test]
    fn test_js_binding_generator_string_return() {
        use std::fs;

        let exports = vec![WasmExport {
            ori_name: "get_name".to_string(),
            wasm_name: "_ori_get_name".to_string(),
            params: vec![],
            return_type: WasmType::String,
            is_async: false,
        }];
        let gen = JsBindingGenerator::new("name_module", exports);

        let temp_dir = std::env::temp_dir();
        let js_path = temp_dir.join("test_ori_wasm_string.js");

        let result = gen.generate_js(&js_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&js_path).unwrap();
        assert!(content.contains("get_name"));

        let _ = fs::remove_file(&js_path);
    }

    #[test]
    fn test_wasi_config_write_undefined_symbols() {
        use std::fs;

        let config = WasiConfig::default();
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_wasi_symbols.txt");

        let result = config.write_undefined_symbols(&path);
        assert!(result.is_ok(), "write_undefined_symbols failed: {result:?}");

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("wasi_snapshot_preview1.proc_exit"));
        assert!(content.contains("wasi_snapshot_preview1.fd_write"));
        assert!(content.contains("wasi_snapshot_preview1.path_open"));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wasm_opt_runner_is_available() {
        // wasm-opt may or may not be available in the test environment
        let runner = WasmOptRunner::new();
        let _available = runner.is_available(); // Just verify it doesn't panic
    }

    #[test]
    fn test_wasm_opt_all_levels() {
        assert_eq!(WasmOptLevel::O1.flag(), "-O1");
        assert_eq!(WasmOptLevel::O3.flag(), "-O3");
        assert_eq!(WasmOptLevel::O4.flag(), "-O4");
    }

    #[test]
    fn test_pascal_case_empty() {
        assert_eq!(pascal_case(""), "");
    }

    #[test]
    fn test_pascal_case_single_char() {
        assert_eq!(pascal_case("a"), "A");
    }

    #[test]
    fn test_pascal_case_multiple_separators() {
        assert_eq!(pascal_case("a_b-c_d"), "ABCD");
    }

    #[test]
    fn test_wasm_export_clone() {
        let export = WasmExport {
            ori_name: "test".to_string(),
            wasm_name: "_test".to_string(),
            params: vec![WasmType::I32],
            return_type: WasmType::Void,
            is_async: false,
        };
        let cloned = export.clone();
        assert_eq!(cloned.ori_name, "test");
    }

    #[test]
    fn test_wasi_preopen_clone() {
        let preopen = WasiPreopen {
            guest_path: "/app".to_string(),
            host_path: "/home/user/app".to_string(),
        };
        let cloned = preopen.clone();
        assert_eq!(cloned.guest_path, "/app");
        assert_eq!(cloned.host_path, "/home/user/app");
    }

    #[test]
    fn test_wasm_config_clone() {
        let config = WasmConfig::browser();
        let cloned = config.clone();
        assert!(cloned.generate_js_bindings());
        assert!(cloned.generate_dts());
    }

    #[test]
    fn test_wasi_config_clone() {
        let config = WasiConfig::cli()
            .with_preopen("/app", "/tmp")
            .with_env("KEY", "value");
        let cloned = config.clone();
        assert_eq!(cloned.preopens.len(), 1);
        assert_eq!(cloned.env_vars.len(), 1);
    }

    #[test]
    fn test_js_binding_empty_exports() {
        use std::fs;

        let gen = JsBindingGenerator::new("empty_module", vec![]);
        let temp_dir = std::env::temp_dir();
        let js_path = temp_dir.join("test_ori_empty.js");

        let result = gen.generate_js(&js_path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&js_path).unwrap();
        assert!(content.contains("EmptyModule"));

        let _ = fs::remove_file(&js_path);
    }
}
