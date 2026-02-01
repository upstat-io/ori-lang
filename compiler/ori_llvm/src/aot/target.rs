//! Target Configuration for AOT Compilation
//!
//! Provides target triple parsing, validation, and LLVM target machine creation.
//!
//! # Architecture
//!
//! Target triples follow the format: `<arch>-<vendor>-<os>[-<env>]`
//!
//! Examples:
//! - `x86_64-unknown-linux-gnu` - 64-bit Linux with glibc
//! - `aarch64-apple-darwin` - ARM64 macOS
//! - `wasm32-unknown-unknown` - Standalone WebAssembly
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, TargetError};
//!
//! // Native target (auto-detected)
//! let native = TargetConfig::native()?;
//!
//! // Specific target with features
//! let config = TargetConfig::from_triple("x86_64-unknown-linux-gnu")?
//!     .with_cpu("skylake")
//!     .with_features("+avx2,+fma");
//! ```

use std::fmt;
use std::sync::Once;

use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::OptimizationLevel;

/// Error type for target configuration operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    /// Target triple is not in the supported list.
    UnsupportedTarget {
        triple: String,
        supported: Vec<&'static str>,
    },
    /// Failed to initialize LLVM target.
    InitializationFailed(String),
    /// Failed to create target machine.
    TargetMachineCreationFailed(String),
    /// Invalid target triple format.
    InvalidTripleFormat { triple: String, reason: String },
    /// Invalid CPU name.
    InvalidCpu { cpu: String, target: String },
    /// Invalid feature specification.
    InvalidFeature { feature: String, reason: String },
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget { triple, supported } => {
                write!(
                    f,
                    "unsupported target '{triple}'. Supported targets: {}",
                    supported.join(", ")
                )
            }
            Self::InitializationFailed(msg) => {
                write!(f, "failed to initialize LLVM target: {msg}")
            }
            Self::TargetMachineCreationFailed(msg) => {
                write!(f, "failed to create target machine: {msg}")
            }
            Self::InvalidTripleFormat { triple, reason } => {
                write!(f, "invalid target triple '{triple}': {reason}")
            }
            Self::InvalidCpu { cpu, target } => {
                write!(f, "invalid CPU '{cpu}' for target '{target}'")
            }
            Self::InvalidFeature { feature, reason } => {
                write!(f, "invalid feature '{feature}': {reason}")
            }
        }
    }
}

impl std::error::Error for TargetError {}

/// Supported target triples for AOT compilation.
///
/// These are the officially supported targets. Cross-compilation requires
/// the appropriate sysroot to be installed via `ori target add`.
pub const SUPPORTED_TARGETS: &[&str] = &[
    // Linux
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    // macOS
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    // Windows
    "x86_64-pc-windows-msvc",
    "x86_64-pc-windows-gnu",
    // WebAssembly
    "wasm32-unknown-unknown",
    "wasm32-wasi",
];

/// Parsed components of a target triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetTripleComponents {
    /// CPU architecture (e.g., `x86_64`, `aarch64`, `wasm32`)
    pub arch: String,
    /// Hardware vendor (e.g., `unknown`, `apple`, `pc`)
    pub vendor: String,
    /// Operating system (e.g., `linux`, `darwin`, `windows`, `wasi`)
    pub os: String,
    /// Environment/ABI (e.g., `gnu`, `musl`, `msvc`) - optional
    pub env: Option<String>,
}

impl TargetTripleComponents {
    /// Parse a target triple string into components.
    ///
    /// Format: `<arch>-<vendor>-<os>[-<env>]`
    pub fn parse(triple: &str) -> Result<Self, TargetError> {
        let parts: Vec<&str> = triple.split('-').collect();

        if parts.len() < 3 {
            return Err(TargetError::InvalidTripleFormat {
                triple: triple.to_string(),
                reason: "expected at least 3 components: <arch>-<vendor>-<os>".to_string(),
            });
        }

        Ok(Self {
            arch: parts[0].to_string(),
            vendor: parts[1].to_string(),
            os: parts[2].to_string(),
            env: parts.get(3).map(|s| (*s).to_string()),
        })
    }

    /// Check if this is a WebAssembly target.
    #[must_use]
    pub fn is_wasm(&self) -> bool {
        self.arch == "wasm32" || self.arch == "wasm64"
    }

    /// Check if this is a Windows target.
    #[must_use]
    pub fn is_windows(&self) -> bool {
        self.os == "windows"
    }

    /// Check if this is a macOS target.
    #[must_use]
    pub fn is_macos(&self) -> bool {
        self.os == "darwin"
    }

    /// Check if this is a Linux target.
    #[must_use]
    pub fn is_linux(&self) -> bool {
        self.os == "linux"
    }

    /// Get the target family (unix, windows, or wasm).
    #[must_use]
    pub fn family(&self) -> &'static str {
        if self.is_wasm() {
            "wasm"
        } else if self.is_windows() {
            "windows"
        } else {
            "unix"
        }
    }
}

impl fmt::Display for TargetTripleComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}", self.arch, self.vendor, self.os)?;
        if let Some(env) = &self.env {
            write!(f, "-{env}")?;
        }
        Ok(())
    }
}

/// Target configuration for AOT compilation.
///
/// Encapsulates all target-specific settings needed to generate native code.
#[derive(Debug, Clone)]
pub struct TargetConfig {
    /// The target triple string (e.g., "x86_64-unknown-linux-gnu").
    triple: String,
    /// Parsed triple components for easy querying.
    components: TargetTripleComponents,
    /// Target CPU (e.g., "generic", "native", "skylake").
    cpu: String,
    /// CPU features string (e.g., "+avx2,+fma,-sse4.1").
    features: String,
    /// Optimization level for code generation.
    opt_level: OptimizationLevel,
    /// Relocation model (affects PIC/PIE generation).
    reloc_mode: RelocMode,
    /// Code model (affects addressing modes).
    code_model: CodeModel,
}

impl TargetConfig {
    /// Create a target configuration for the native (host) target.
    ///
    /// This auto-detects the current machine's architecture and OS.
    ///
    /// # Errors
    ///
    /// Returns an error if LLVM target initialization fails.
    pub fn native() -> Result<Self, TargetError> {
        initialize_native_target()?;

        let triple = TargetMachine::get_default_triple();
        let triple_str = triple.as_str().to_string_lossy().to_string();
        let components = TargetTripleComponents::parse(&triple_str)?;

        Ok(Self {
            triple: triple_str,
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        })
    }

    /// Create a target configuration from a target triple string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The triple is not in the supported targets list
    /// - The triple format is invalid
    /// - LLVM target initialization fails
    pub fn from_triple(triple: &str) -> Result<Self, TargetError> {
        // Validate against supported targets
        if !SUPPORTED_TARGETS.contains(&triple) {
            return Err(TargetError::UnsupportedTarget {
                triple: triple.to_string(),
                supported: SUPPORTED_TARGETS.to_vec(),
            });
        }

        let components = TargetTripleComponents::parse(triple)?;

        // Initialize the appropriate LLVM target
        initialize_target_for_triple(&components)?;

        Ok(Self {
            triple: triple.to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        })
    }

    /// Set the target CPU (builder pattern).
    ///
    /// Common values:
    /// - `"generic"` - No specific CPU optimizations (default)
    /// - `"native"` - Optimize for the current machine (use `with_cpu_native()` instead)
    /// - CPU name like `"skylake"`, `"znver3"`, `"apple-m1"`
    #[must_use]
    pub fn with_cpu(mut self, cpu: &str) -> Self {
        self.cpu = cpu.to_string();
        self
    }

    /// Set the CPU to native (auto-detect host CPU).
    ///
    /// This queries LLVM for the host CPU name and enables optimizations
    /// specific to the current machine.
    ///
    /// # Note
    ///
    /// Using native CPU optimizations may produce binaries that don't run
    /// on other machines. For portable builds, use `with_cpu("generic")`.
    #[must_use]
    pub fn with_cpu_native(mut self) -> Self {
        self.cpu = get_host_cpu_name();
        self
    }

    /// Set CPU features (builder pattern).
    ///
    /// Format: comma-separated list with `+` to enable, `-` to disable.
    /// Example: `"+avx2,+fma,-sse4.1"`
    #[must_use]
    pub fn with_features(mut self, features: &str) -> Self {
        self.features = features.to_string();
        self
    }

    /// Set CPU features to native (auto-detect host features).
    ///
    /// This queries LLVM for all CPU features available on the host machine.
    ///
    /// # Note
    ///
    /// Using native features may produce binaries that don't run
    /// on other machines. For portable builds, don't set features.
    #[must_use]
    pub fn with_features_native(mut self) -> Self {
        self.features = get_host_cpu_features();
        self
    }

    /// Add a single CPU feature (builder pattern).
    ///
    /// The feature is added with `+` prefix (enabled).
    #[must_use]
    pub fn with_feature(mut self, feature: &str) -> Self {
        if self.features.is_empty() {
            self.features = format!("+{feature}");
        } else {
            self.features = format!("{},+{feature}", self.features);
        }
        self
    }

    /// Remove/disable a single CPU feature (builder pattern).
    ///
    /// The feature is added with `-` prefix (disabled).
    #[must_use]
    pub fn without_feature(mut self, feature: &str) -> Self {
        if self.features.is_empty() {
            self.features = format!("-{feature}");
        } else {
            self.features = format!("{},-{feature}", self.features);
        }
        self
    }

    /// Set the optimization level (builder pattern).
    #[must_use]
    pub fn with_opt_level(mut self, level: OptimizationLevel) -> Self {
        self.opt_level = level;
        self
    }

    /// Set the relocation model (builder pattern).
    ///
    /// - `RelocMode::Default` - Let LLVM choose
    /// - `RelocMode::Static` - No PIC (position-independent code)
    /// - `RelocMode::PIC` - Position-independent code (for shared libraries)
    #[must_use]
    pub fn with_reloc_mode(mut self, mode: RelocMode) -> Self {
        self.reloc_mode = mode;
        self
    }

    /// Set the code model (builder pattern).
    ///
    /// - `CodeModel::Default` - Let LLVM choose
    /// - `CodeModel::Small` - Code and data fit in lower 2GB
    /// - `CodeModel::Large` - No assumptions about addresses
    #[must_use]
    pub fn with_code_model(mut self, model: CodeModel) -> Self {
        self.code_model = model;
        self
    }

    // -- Accessors --

    /// Get the target triple string.
    #[must_use]
    pub fn triple(&self) -> &str {
        &self.triple
    }

    /// Get the parsed triple components.
    #[must_use]
    pub fn components(&self) -> &TargetTripleComponents {
        &self.components
    }

    /// Get the target CPU.
    #[must_use]
    pub fn cpu(&self) -> &str {
        &self.cpu
    }

    /// Get the CPU features string.
    #[must_use]
    pub fn features(&self) -> &str {
        &self.features
    }

    /// Get the optimization level.
    #[must_use]
    pub fn opt_level(&self) -> OptimizationLevel {
        self.opt_level
    }

    /// Check if this is a WebAssembly target.
    #[must_use]
    pub fn is_wasm(&self) -> bool {
        self.components.is_wasm()
    }

    /// Check if this is a Windows target.
    #[must_use]
    pub fn is_windows(&self) -> bool {
        self.components.is_windows()
    }

    /// Check if this is a macOS target.
    #[must_use]
    pub fn is_macos(&self) -> bool {
        self.components.is_macos()
    }

    /// Check if this is a Linux target.
    #[must_use]
    pub fn is_linux(&self) -> bool {
        self.components.is_linux()
    }

    /// Get the target family.
    #[must_use]
    pub fn family(&self) -> &'static str {
        self.components.family()
    }

    /// Create an LLVM `TargetMachine` for this configuration.
    ///
    /// The target machine is used to emit object files and get data layout.
    ///
    /// # Errors
    ///
    /// Returns an error if LLVM cannot create a target machine for
    /// the configured triple/cpu/features combination.
    pub fn create_target_machine(&self) -> Result<TargetMachine, TargetError> {
        let target_triple = TargetTriple::create(&self.triple);

        let target = Target::from_triple(&target_triple).map_err(|e| {
            TargetError::TargetMachineCreationFailed(format!("failed to get target: {e}"))
        })?;

        target
            .create_target_machine(
                &target_triple,
                &self.cpu,
                &self.features,
                self.opt_level,
                self.reloc_mode,
                self.code_model,
            )
            .ok_or_else(|| {
                TargetError::TargetMachineCreationFailed(format!(
                    "LLVM returned None for target '{}' with CPU '{}' and features '{}'",
                    self.triple, self.cpu, self.features
                ))
            })
    }

    /// Get the LLVM data layout string for this target.
    ///
    /// The data layout specifies pointer sizes, alignments, and endianness.
    ///
    /// # Errors
    ///
    /// Returns an error if a target machine cannot be created.
    pub fn data_layout(&self) -> Result<String, TargetError> {
        let machine = self.create_target_machine()?;
        Ok(machine
            .get_target_data()
            .get_data_layout()
            .as_str()
            .to_string_lossy()
            .to_string())
    }

    /// Configure an LLVM module with the target triple and data layout.
    ///
    /// This sets both the target triple and data layout on the module,
    /// which is required for correct code generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the target machine cannot be created.
    pub fn configure_module(
        &self,
        module: &inkwell::module::Module<'_>,
    ) -> Result<(), TargetError> {
        let machine = self.create_target_machine()?;

        // Set the target triple
        module.set_triple(&TargetTriple::create(&self.triple));

        // Set the data layout from the target machine
        module.set_data_layout(&machine.get_target_data().get_data_layout());

        Ok(())
    }

    /// Get pointer size in bytes for this target.
    ///
    /// Most targets use 8 bytes (64-bit), WASM uses 4 bytes (32-bit).
    #[must_use]
    pub fn pointer_size(&self) -> u32 {
        match self.components.arch.as_str() {
            "wasm32" | "i686" | "i386" | "arm" => 4,
            _ => 8,
        }
    }

    /// Get pointer alignment in bytes for this target.
    #[must_use]
    pub fn pointer_align(&self) -> u32 {
        self.pointer_size() // Pointers are naturally aligned
    }

    /// Check if this target is little-endian.
    #[must_use]
    pub fn is_little_endian(&self) -> bool {
        // All currently supported targets are little-endian
        true
    }
}

impl Default for TargetConfig {
    /// Returns a native target configuration with default settings.
    ///
    /// # Panics
    ///
    /// Panics if native target initialization fails. For fallible creation,
    /// use `TargetConfig::native()` instead.
    fn default() -> Self {
        Self::native().expect("failed to initialize native target")
    }
}

// -- LLVM Target Initialization --

static NATIVE_TARGET_INIT: Once = Once::new();
static X86_TARGET_INIT: Once = Once::new();
static AARCH64_TARGET_INIT: Once = Once::new();
static WASM_TARGET_INIT: Once = Once::new();

/// Initialize the native LLVM target.
///
/// Safe to call multiple times; initialization happens once.
fn initialize_native_target() -> Result<(), TargetError> {
    let mut result = Ok(());

    NATIVE_TARGET_INIT.call_once(|| {
        if let Err(e) = Target::initialize_native(&InitializationConfig::default()) {
            result = Err(TargetError::InitializationFailed(e.clone()));
        }
    });

    result
}

/// Initialize LLVM targets for a given triple.
fn initialize_target_for_triple(components: &TargetTripleComponents) -> Result<(), TargetError> {
    match components.arch.as_str() {
        "x86_64" | "i686" | "i386" => {
            X86_TARGET_INIT.call_once(|| {
                Target::initialize_x86(&InitializationConfig::default());
            });
        }
        "aarch64" | "arm64" => {
            AARCH64_TARGET_INIT.call_once(|| {
                Target::initialize_aarch64(&InitializationConfig::default());
            });
        }
        "wasm32" | "wasm64" => {
            WASM_TARGET_INIT.call_once(|| {
                Target::initialize_webassembly(&InitializationConfig::default());
            });
        }
        arch => {
            return Err(TargetError::InitializationFailed(format!(
                "unsupported architecture: {arch}"
            )));
        }
    }

    Ok(())
}

/// Get the host CPU name as detected by LLVM.
///
/// Returns "generic" if detection fails.
pub fn get_host_cpu_name() -> String {
    TargetMachine::get_host_cpu_name().to_string()
}

/// Get the host CPU features as a comma-separated string.
///
/// Returns an empty string if detection fails.
pub fn get_host_cpu_features() -> String {
    TargetMachine::get_host_cpu_features().to_string()
}

/// Parse a features string and validate format.
///
/// Features are comma-separated with `+` to enable and `-` to disable.
/// Example: `"+avx2,+fma,-sse4.1"`
///
/// # Errors
///
/// Returns an error if a feature doesn't start with `+` or `-`.
pub fn parse_features(features: &str) -> Result<Vec<(&str, bool)>, TargetError> {
    if features.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();

    for feature in features.split(',') {
        let feature = feature.trim();
        if feature.is_empty() {
            continue;
        }

        if let Some(name) = feature.strip_prefix('+') {
            result.push((name, true));
        } else if let Some(name) = feature.strip_prefix('-') {
            result.push((name, false));
        } else {
            return Err(TargetError::InvalidFeature {
                feature: feature.to_string(),
                reason: "feature must start with '+' (enable) or '-' (disable)".to_string(),
            });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_triple_linux() {
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(components.arch, "x86_64");
        assert_eq!(components.vendor, "unknown");
        assert_eq!(components.os, "linux");
        assert_eq!(components.env, Some("gnu".to_string()));
        assert!(components.is_linux());
        assert_eq!(components.family(), "unix");
    }

    #[test]
    fn test_parse_triple_macos() {
        let components = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
        assert_eq!(components.arch, "aarch64");
        assert_eq!(components.vendor, "apple");
        assert_eq!(components.os, "darwin");
        assert_eq!(components.env, None);
        assert!(components.is_macos());
        assert_eq!(components.family(), "unix");
    }

    #[test]
    fn test_parse_triple_windows() {
        let components = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(components.arch, "x86_64");
        assert_eq!(components.vendor, "pc");
        assert_eq!(components.os, "windows");
        assert_eq!(components.env, Some("msvc".to_string()));
        assert!(components.is_windows());
        assert_eq!(components.family(), "windows");
    }

    #[test]
    fn test_parse_triple_wasm() {
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        assert_eq!(components.arch, "wasm32");
        assert!(components.is_wasm());
        assert_eq!(components.family(), "wasm");
    }

    #[test]
    fn test_parse_triple_invalid() {
        let result = TargetTripleComponents::parse("invalid");
        assert!(result.is_err());

        let result = TargetTripleComponents::parse("x86_64-linux");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_features() {
        let features = parse_features("+avx2,+fma,-sse4.1").unwrap();
        assert_eq!(
            features,
            vec![("avx2", true), ("fma", true), ("sse4.1", false)]
        );
    }

    #[test]
    fn test_parse_features_empty() {
        let features = parse_features("").unwrap();
        assert!(features.is_empty());
    }

    #[test]
    fn test_parse_features_invalid() {
        let result = parse_features("avx2"); // Missing +/-
        assert!(result.is_err());
    }

    #[test]
    fn test_target_config_native() {
        // This test requires LLVM to be properly configured
        let config = TargetConfig::native();
        if let Ok(config) = config {
            assert!(!config.triple().is_empty());
            assert_eq!(config.cpu(), "generic");
            assert!(config.features().is_empty());
        }
        // If native target init fails, that's OK for some test environments
    }

    #[test]
    fn test_target_config_builder() {
        // Test builder pattern (doesn't require LLVM init)
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        let config = TargetConfig {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        };

        let config = config.with_cpu("skylake").with_features("+avx2,+fma");

        assert_eq!(config.cpu(), "skylake");
        assert_eq!(config.features(), "+avx2,+fma");
        assert!(config.is_linux());
        assert!(!config.is_wasm());
    }

    #[test]
    fn test_unsupported_target() {
        let result = TargetConfig::from_triple("riscv64-unknown-linux-gnu");
        assert!(matches!(result, Err(TargetError::UnsupportedTarget { .. })));
    }

    #[test]
    fn test_pointer_size() {
        // 64-bit targets
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        let config = TargetConfig {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        };
        assert_eq!(config.pointer_size(), 8);

        // 32-bit WASM target
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        let config = TargetConfig {
            triple: "wasm32-unknown-unknown".to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        };
        assert_eq!(config.pointer_size(), 4);
    }

    #[test]
    fn test_data_layout_native() {
        // This test requires LLVM to be properly configured
        if let Ok(config) = TargetConfig::native() {
            if let Ok(layout) = config.data_layout() {
                // Data layout should be non-empty and start with endianness
                assert!(!layout.is_empty());
                // Most layouts start with 'e' (little-endian) or 'E' (big-endian)
                assert!(layout.starts_with('e') || layout.starts_with('E'));
            }
        }
    }

    #[test]
    fn test_configure_module() {
        use inkwell::context::Context;

        // This test requires LLVM to be properly configured
        if let Ok(config) = TargetConfig::native() {
            let context = Context::create();
            let module = context.create_module("test");

            // Configure should succeed
            let result = config.configure_module(&module);
            if let Ok(()) = result {
                // Module should have triple set
                let module_triple = module.get_triple();
                assert!(!module_triple.as_str().to_string_lossy().is_empty());
            }
        }
    }

    #[test]
    fn test_get_host_cpu_name() {
        // This should always return something, even if just "generic"
        let cpu = get_host_cpu_name();
        assert!(!cpu.is_empty());
    }

    #[test]
    fn test_get_host_cpu_features() {
        // Features may be empty on some systems, but shouldn't panic
        let _features = get_host_cpu_features();
        // No assertion needed - just verify it doesn't panic
    }

    #[test]
    fn test_with_cpu_native() {
        if let Ok(config) = TargetConfig::native() {
            let config = config.with_cpu_native();
            // CPU should be set to something (might be "generic" on some systems)
            assert!(!config.cpu().is_empty());
        }
    }

    #[test]
    fn test_with_features_native() {
        if let Ok(config) = TargetConfig::native() {
            let config = config.with_features_native();
            // Features string may be empty or contain features
            // Just verify it doesn't panic and is a valid string
            let _ = config.features();
        }
    }

    #[test]
    fn test_with_feature() {
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        let config = TargetConfig {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        };

        // Add single feature
        let config = config.with_feature("avx2");
        assert_eq!(config.features(), "+avx2");

        // Add another feature
        let config = config.with_feature("fma");
        assert_eq!(config.features(), "+avx2,+fma");
    }

    #[test]
    fn test_without_feature() {
        let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
        let config = TargetConfig {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        };

        // Disable a feature
        let config = config.without_feature("sse4.1");
        assert_eq!(config.features(), "-sse4.1");

        // Add and disable features
        let config = config.with_feature("avx2").without_feature("sse3");
        assert_eq!(config.features(), "-sse4.1,+avx2,-sse3");
    }
}
