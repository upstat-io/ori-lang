//! Target Configuration for AOT Compilation
//!
//! Provides the [`TargetConfig`] struct for configuring LLVM target machines
//! with target triple, CPU, features, optimization level, and code model.
//!
//! Target triple parsing, validation, LLVM initialization, and CPU feature
//! detection are in the sibling [`target_features`](super::target_features) module.
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

use inkwell::targets::{CodeModel, RelocMode, Target, TargetMachine, TargetTriple};
use inkwell::OptimizationLevel;

use super::target_features::{
    get_host_cpu_features, get_host_cpu_name, initialize_native_target,
    initialize_target_for_triple, is_supported_target, TargetError, TargetTripleComponents,
    SUPPORTED_TARGETS,
};

/// Target configuration for AOT compilation.
///
/// Encapsulates all target-specific settings needed to generate native code.
#[derive(Debug, Clone)]
pub struct TargetConfig {
    /// The target triple string (e.g., "x86_64-unknown-linux-gnu").
    pub triple: String,
    /// Parsed triple components for easy querying.
    pub components: TargetTripleComponents,
    /// Target CPU (e.g., "generic", "native", "skylake").
    pub cpu: String,
    /// CPU features string (e.g., "+avx2,+fma,-sse4.1").
    pub features: String,
    /// Optimization level for code generation.
    pub opt_level: OptimizationLevel,
    /// Relocation model (affects PIC/PIE generation).
    pub reloc_mode: RelocMode,
    /// Code model (affects addressing modes).
    pub code_model: CodeModel,
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
        // Use into_owned() directly on Cow to avoid redundant allocation
        let triple_str = triple.as_str().to_string_lossy().into_owned();
        let components = TargetTripleComponents::parse(&triple_str)?;

        // Use PIC for Linux targets to support PIE linking (modern default)
        let reloc_mode = if components.is_linux() {
            RelocMode::PIC
        } else {
            RelocMode::Default
        };

        Ok(Self {
            triple: triple_str,
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode,
            code_model: CodeModel::Default,
        })
    }

    /// Create a target configuration from a target triple string.
    ///
    /// The `triple` may use any known arch alias (`arm64` for `aarch64`,
    /// `amd64` for `x86_64`, `i486/i586/i686` for `i386`). Parse happens
    /// BEFORE the supported-targets check, so the canonicalized form is
    /// what gets validated — this fixes the asymmetry where
    /// `arm64-apple-darwin` was rejected even though Apple Silicon
    /// emits exactly that spelling from LLVM. The stored triple is the
    /// canonical spelling, ensuring downstream consumers see one name.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The triple format is invalid or uses an unknown architecture
    /// - The canonicalized triple is not in the supported targets list
    /// - LLVM target initialization fails
    pub fn from_triple(triple: &str) -> Result<Self, TargetError> {
        // Parse first — this canonicalizes arch aliases (arm64 → aarch64).
        let components = TargetTripleComponents::parse(triple)?;

        // Validate the canonical form against the supported list.
        let canonical = components.to_string();
        if !is_supported_target(&canonical) {
            return Err(TargetError::UnsupportedTarget {
                // Report the user's input spelling in the error for clarity.
                triple: triple.to_string(),
                supported: SUPPORTED_TARGETS.to_vec(),
            });
        }

        // Initialize the appropriate LLVM target
        initialize_target_for_triple(&components)?;

        // Use PIC for Linux targets to support PIE linking (modern default)
        let reloc_mode = if components.is_linux() {
            RelocMode::PIC
        } else {
            RelocMode::Default
        };

        Ok(Self {
            triple: canonical,
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode,
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
        if !self.features.is_empty() {
            self.features.push(',');
        }
        self.features.push('+');
        self.features.push_str(feature);
        self
    }

    /// Remove/disable a single CPU feature (builder pattern).
    ///
    /// The feature is added with `-` prefix (disabled).
    #[must_use]
    pub fn without_feature(mut self, feature: &str) -> Self {
        if !self.features.is_empty() {
            self.features.push(',');
        }
        self.features.push('-');
        self.features.push_str(feature);
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
    /// Delegates to the typed [`Arch::pointer_size_bytes`] — exhaustive over
    /// every supported architecture, no string fallthrough.
    #[must_use]
    pub fn pointer_size(&self) -> u32 {
        self.components.arch.pointer_size_bytes()
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

impl TargetConfig {
    /// Create a target configuration from pre-parsed components.
    ///
    /// This is useful for testing or when you already have parsed triple components.
    /// Note: This does not initialize LLVM targets, so methods like `create_target_machine`
    /// may fail unless you've called the appropriate initialization functions.
    #[must_use]
    pub fn from_components(components: TargetTripleComponents) -> Self {
        // Use PIC for Linux targets to support PIE linking (modern default)
        let reloc_mode = if components.is_linux() {
            RelocMode::PIC
        } else {
            RelocMode::Default
        };

        Self {
            triple: components.to_string(),
            components,
            cpu: "generic".to_string(),
            features: String::new(),
            opt_level: OptimizationLevel::None,
            reloc_mode,
            code_model: CodeModel::Default,
        }
    }
}
