//! WebAssembly build configuration types.
//!
//! This module defines the core configuration types for WASM compilation:
//! memory layout, stack size, feature flags, output options, and the
//! top-level `WasmConfig` that bundles them together.

use super::optimize::WasmOptLevel;
use super::wasi::WasiConfig;

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
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent WASM feature flags; each is a genuine on/off toggle"
)]
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

    /// Create configuration for WASI Preview 1 (`wasm32-unknown-wasip1`).
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
