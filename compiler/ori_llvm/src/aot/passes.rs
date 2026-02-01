//! Optimization Pipeline for AOT Compilation
//!
//! This module provides LLVM optimization pass management using the
//! **New Pass Manager** (NPM) introduced in LLVM 13 and made default in LLVM 14.
//!
//! # Architecture
//!
//! We use LLVM's C API for the new pass manager via `llvm-sys`:
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │ OptimizationLevel│───▶│ PassBuilderOpts │───▶│   LLVMRunPasses │
//! │   (O0-O3/Os/Oz) │    │  (vectorize,    │    │ "default<O3>"   │
//! └─────────────────┘    │   unroll, etc)  │    └─────────────────┘
//!                        └─────────────────┘
//! ```
//!
//! The new pass manager uses string-based pipeline specification:
//! - `default<O0>` through `default<O3>` for optimization levels
//! - `default<Os>` and `default<Oz>` for size optimization
//! - `thinlto-pre-link<O2>`, `lto<O2>` for LTO stages
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::passes::{OptimizationConfig, OptimizationLevel, run_optimization_passes};
//!
//! let config = OptimizationConfig::new(OptimizationLevel::O2);
//! run_optimization_passes(&module, &target_machine, &config)?;
//! ```
//!
//! # References
//!
//! - [LLVM New Pass Manager](https://llvm.org/docs/NewPassManager.html)
//! - [LLVM C API](https://llvm.org/docs/doxygen/group__LLVMCCoreNewPM.html)

use inkwell::module::Module;
use inkwell::targets::TargetMachine;
use std::ffi::CString;
use std::fmt;

/// Optimization level for the pass pipeline.
///
/// These map directly to LLVM's `OptimizationLevel` enum and the
/// corresponding `default<OX>` pipeline strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationLevel {
    /// No optimization. Fastest compilation, best for debugging.
    /// Maps to `default<O0>` which runs only essential passes.
    #[default]
    O0,

    /// Basic optimization. Light inlining, CSE, `SimplifyCFG`.
    /// Maps to `default<O1>`.
    O1,

    /// Standard optimization. Production default.
    /// Includes LICM, GVN, moderate inlining, limited loop unrolling.
    /// Maps to `default<O2>`.
    O2,

    /// Aggressive optimization. Maximum performance.
    /// Full vectorization, aggressive inlining, full loop unrolling.
    /// Maps to `default<O3>`.
    O3,

    /// Size optimization. Optimize for smaller code size.
    /// Similar to O2 but prefers smaller code over faster code.
    /// Maps to `default<Os>`.
    Os,

    /// Aggressive size optimization. Smallest possible code.
    /// Disables most size-increasing optimizations.
    /// Maps to `default<Oz>`.
    Oz,
}

impl OptimizationLevel {
    /// Get the pipeline string for the new pass manager.
    ///
    /// This string is passed to `LLVMRunPasses` to construct the
    /// appropriate optimization pipeline.
    #[must_use]
    pub fn pipeline_string(&self) -> &'static str {
        match self {
            Self::O0 => "default<O0>",
            Self::O1 => "default<O1>",
            Self::O2 => "default<O2>",
            Self::O3 => "default<O3>",
            Self::Os => "default<Os>",
            Self::Oz => "default<Oz>",
        }
    }

    /// Check if this level enables loop vectorization by default.
    #[must_use]
    pub fn enables_loop_vectorization(&self) -> bool {
        matches!(self, Self::O2 | Self::O3)
    }

    /// Check if this level enables SLP vectorization by default.
    #[must_use]
    pub fn enables_slp_vectorization(&self) -> bool {
        matches!(self, Self::O2 | Self::O3)
    }

    /// Check if this level enables loop unrolling by default.
    #[must_use]
    pub fn enables_loop_unrolling(&self) -> bool {
        !matches!(self, Self::O0 | Self::Os | Self::Oz)
    }

    /// Check if this level enables function merging by default.
    #[must_use]
    pub fn enables_merge_functions(&self) -> bool {
        matches!(self, Self::Os | Self::Oz)
    }
}

impl fmt::Display for OptimizationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::O0 => write!(f, "O0"),
            Self::O1 => write!(f, "O1"),
            Self::O2 => write!(f, "O2"),
            Self::O3 => write!(f, "O3"),
            Self::Os => write!(f, "Os"),
            Self::Oz => write!(f, "Oz"),
        }
    }
}

/// Link-Time Optimization (LTO) mode.
///
/// LTO performs whole-program optimization at link time, enabling
/// optimizations that aren't possible with separate compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LtoMode {
    /// No LTO. Default for debug builds.
    #[default]
    Off,

    /// Thin LTO. Parallel, scalable LTO with good performance.
    /// Recommended for most use cases when LTO is desired.
    Thin,

    /// Full LTO. Maximum optimization, but slower and uses more memory.
    /// Best for final release builds where compile time doesn't matter.
    Full,
}

impl LtoMode {
    /// Get the pre-link pipeline string for this LTO mode.
    ///
    /// Returns `None` if LTO is off (use regular `default<OX>` instead).
    #[must_use]
    pub fn prelink_pipeline_string(&self, opt_level: OptimizationLevel) -> Option<String> {
        let level = match opt_level {
            OptimizationLevel::O0 => "O0",
            OptimizationLevel::O1 => "O1",
            OptimizationLevel::O2 => "O2",
            OptimizationLevel::O3 => "O3",
            OptimizationLevel::Os => "Os",
            OptimizationLevel::Oz => "Oz",
        };

        match self {
            Self::Off => None,
            Self::Thin => Some(format!("thinlto-pre-link<{level}>")),
            Self::Full => Some(format!("lto-pre-link<{level}>")),
        }
    }

    /// Get the LTO pipeline string for this mode.
    ///
    /// This is used during the LTO phase itself.
    /// Returns `None` if LTO is off.
    #[must_use]
    pub fn lto_pipeline_string(&self, opt_level: OptimizationLevel) -> Option<String> {
        let level = match opt_level {
            OptimizationLevel::O0 => "O0",
            OptimizationLevel::O1 => "O1",
            OptimizationLevel::O2 => "O2",
            OptimizationLevel::O3 => "O3",
            OptimizationLevel::Os => "Os",
            OptimizationLevel::Oz => "Oz",
        };

        match self {
            Self::Off => None,
            Self::Thin => Some(format!("thinlto<{level}>")),
            Self::Full => Some(format!("lto<{level}>")),
        }
    }
}

impl fmt::Display for LtoMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Thin => write!(f, "thin"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// Configuration for the optimization pipeline.
///
/// Provides fine-grained control over which optimizations are enabled.
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Base optimization level.
    pub level: OptimizationLevel,

    /// LTO mode.
    pub lto: LtoMode,

    /// Whether this is the LTO phase (vs pre-link phase).
    pub is_lto_phase: bool,

    /// Enable loop vectorization.
    /// Default: follows optimization level.
    pub loop_vectorization: Option<bool>,

    /// Enable SLP (Superword Level Parallelism) vectorization.
    /// Default: follows optimization level.
    pub slp_vectorization: Option<bool>,

    /// Enable loop unrolling.
    /// Default: follows optimization level.
    pub loop_unrolling: Option<bool>,

    /// Enable loop interleaving.
    /// Default: follows loop unrolling setting.
    pub loop_interleaving: Option<bool>,

    /// Enable function merging (combines identical functions).
    /// Default: follows optimization level.
    pub merge_functions: Option<bool>,

    /// Inliner threshold. Higher values inline more aggressively.
    /// Default: LLVM decides based on optimization level.
    pub inliner_threshold: Option<u32>,

    /// Enable IR verification after optimization (for debugging).
    pub verify_each: bool,

    /// Enable debug logging from pass manager.
    pub debug_logging: bool,

    /// Additional custom passes to run (comma-separated).
    /// Example: "instcombine,simplifycfg,dce"
    pub extra_passes: Option<String>,
}

impl OptimizationConfig {
    /// Create a new configuration with the given optimization level.
    #[must_use]
    pub fn new(level: OptimizationLevel) -> Self {
        Self {
            level,
            lto: LtoMode::Off,
            is_lto_phase: false,
            loop_vectorization: None,
            slp_vectorization: None,
            loop_unrolling: None,
            loop_interleaving: None,
            merge_functions: None,
            inliner_threshold: None,
            verify_each: false,
            debug_logging: false,
            extra_passes: None,
        }
    }

    /// Create a debug configuration (O0, no LTO).
    #[must_use]
    pub fn debug() -> Self {
        Self::new(OptimizationLevel::O0)
    }

    /// Create a release configuration (O2, no LTO).
    #[must_use]
    pub fn release() -> Self {
        Self::new(OptimizationLevel::O2)
    }

    /// Create an aggressive release configuration (O3).
    #[must_use]
    pub fn aggressive() -> Self {
        Self::new(OptimizationLevel::O3)
    }

    /// Create a size-optimized configuration (Os).
    #[must_use]
    pub fn size() -> Self {
        Self::new(OptimizationLevel::Os)
    }

    /// Create a minimal size configuration (Oz).
    #[must_use]
    pub fn min_size() -> Self {
        Self::new(OptimizationLevel::Oz)
    }

    /// Set LTO mode (builder pattern).
    #[must_use]
    pub fn with_lto(mut self, mode: LtoMode) -> Self {
        self.lto = mode;
        self
    }

    /// Mark this as the LTO phase (builder pattern).
    #[must_use]
    pub fn as_lto_phase(mut self) -> Self {
        self.is_lto_phase = true;
        self
    }

    /// Enable or disable loop vectorization (builder pattern).
    #[must_use]
    pub fn with_loop_vectorization(mut self, enable: bool) -> Self {
        self.loop_vectorization = Some(enable);
        self
    }

    /// Enable or disable SLP vectorization (builder pattern).
    #[must_use]
    pub fn with_slp_vectorization(mut self, enable: bool) -> Self {
        self.slp_vectorization = Some(enable);
        self
    }

    /// Enable or disable loop unrolling (builder pattern).
    #[must_use]
    pub fn with_loop_unrolling(mut self, enable: bool) -> Self {
        self.loop_unrolling = Some(enable);
        self
    }

    /// Enable or disable function merging (builder pattern).
    #[must_use]
    pub fn with_merge_functions(mut self, enable: bool) -> Self {
        self.merge_functions = Some(enable);
        self
    }

    /// Set the inliner threshold (builder pattern).
    #[must_use]
    pub fn with_inliner_threshold(mut self, threshold: u32) -> Self {
        self.inliner_threshold = Some(threshold);
        self
    }

    /// Enable verification after each pass (builder pattern).
    #[must_use]
    pub fn with_verify_each(mut self, enable: bool) -> Self {
        self.verify_each = enable;
        self
    }

    /// Enable debug logging (builder pattern).
    #[must_use]
    pub fn with_debug_logging(mut self, enable: bool) -> Self {
        self.debug_logging = enable;
        self
    }

    /// Add extra passes to run after the main pipeline (builder pattern).
    #[must_use]
    pub fn with_extra_passes(mut self, passes: impl Into<String>) -> Self {
        self.extra_passes = Some(passes.into());
        self
    }

    /// Get the pipeline string for this configuration.
    ///
    /// Handles LTO pre-link vs LTO phase vs normal compilation.
    #[must_use]
    pub fn pipeline_string(&self) -> String {
        // LTO phase uses LTO-specific pipeline
        if self.is_lto_phase {
            if let Some(pipeline) = self.lto.lto_pipeline_string(self.level) {
                return pipeline;
            }
        }

        // Pre-link phase with LTO uses pre-link pipeline
        if self.lto != LtoMode::Off {
            if let Some(pipeline) = self.lto.prelink_pipeline_string(self.level) {
                return pipeline;
            }
        }

        // Normal compilation uses default pipeline
        self.level.pipeline_string().to_string()
    }

    /// Get effective loop vectorization setting.
    #[must_use]
    pub fn effective_loop_vectorization(&self) -> bool {
        self.loop_vectorization
            .unwrap_or_else(|| self.level.enables_loop_vectorization())
    }

    /// Get effective SLP vectorization setting.
    #[must_use]
    pub fn effective_slp_vectorization(&self) -> bool {
        self.slp_vectorization
            .unwrap_or_else(|| self.level.enables_slp_vectorization())
    }

    /// Get effective loop unrolling setting.
    #[must_use]
    pub fn effective_loop_unrolling(&self) -> bool {
        self.loop_unrolling
            .unwrap_or_else(|| self.level.enables_loop_unrolling())
    }

    /// Get effective loop interleaving setting.
    #[must_use]
    pub fn effective_loop_interleaving(&self) -> bool {
        self.loop_interleaving
            .unwrap_or_else(|| self.effective_loop_unrolling())
    }

    /// Get effective merge functions setting.
    #[must_use]
    pub fn effective_merge_functions(&self) -> bool {
        self.merge_functions
            .unwrap_or_else(|| self.level.enables_merge_functions())
    }
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self::new(OptimizationLevel::O0)
    }
}

/// Error type for optimization operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationError {
    /// Failed to create pass builder options.
    PassBuilderOptionsCreationFailed,

    /// Failed to run optimization passes.
    PassesFailed { message: String },

    /// Invalid pass pipeline string.
    InvalidPipeline { pipeline: String, message: String },
}

impl fmt::Display for OptimizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassBuilderOptionsCreationFailed => {
                write!(f, "failed to create pass builder options")
            }
            Self::PassesFailed { message } => {
                write!(f, "optimization passes failed: {message}")
            }
            Self::InvalidPipeline { pipeline, message } => {
                write!(f, "invalid pipeline '{pipeline}': {message}")
            }
        }
    }
}

impl std::error::Error for OptimizationError {}

/// Run optimization passes on a module using the LLVM new pass manager.
///
/// This uses the LLVM 17 C API for the new pass manager, specifically
/// `LLVMRunPasses` with pipeline strings like `"default<O3>"`.
///
/// # Arguments
///
/// * `module` - The LLVM module to optimize
/// * `target_machine` - Target machine for target-specific optimizations
/// * `config` - Optimization configuration
///
/// # Errors
///
/// Returns an error if:
/// - Pass builder options cannot be created
/// - The pipeline string is invalid
/// - Pass execution fails
///
/// # Example
///
/// ```ignore
/// let config = OptimizationConfig::release();
/// run_optimization_passes(&module, &target_machine, &config)?;
/// ```
pub fn run_optimization_passes(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    config: &OptimizationConfig,
) -> Result<(), OptimizationError> {
    use llvm_sys::transforms::pass_builder::{
        LLVMCreatePassBuilderOptions, LLVMDisposePassBuilderOptions,
        LLVMPassBuilderOptionsSetDebugLogging, LLVMPassBuilderOptionsSetInlinerThreshold,
        LLVMPassBuilderOptionsSetLoopInterleaving, LLVMPassBuilderOptionsSetLoopUnrolling,
        LLVMPassBuilderOptionsSetLoopVectorization, LLVMPassBuilderOptionsSetMergeFunctions,
        LLVMPassBuilderOptionsSetSLPVectorization, LLVMPassBuilderOptionsSetVerifyEach,
        LLVMRunPasses,
    };

    // Create pass builder options
    let options = unsafe { LLVMCreatePassBuilderOptions() };
    if options.is_null() {
        return Err(OptimizationError::PassBuilderOptionsCreationFailed);
    }

    // Configure options based on config
    unsafe {
        LLVMPassBuilderOptionsSetLoopVectorization(
            options,
            config.effective_loop_vectorization().into(),
        );
        LLVMPassBuilderOptionsSetSLPVectorization(
            options,
            config.effective_slp_vectorization().into(),
        );
        LLVMPassBuilderOptionsSetLoopUnrolling(options, config.effective_loop_unrolling().into());
        LLVMPassBuilderOptionsSetLoopInterleaving(
            options,
            config.effective_loop_interleaving().into(),
        );
        LLVMPassBuilderOptionsSetMergeFunctions(options, config.effective_merge_functions().into());

        if let Some(threshold) = config.inliner_threshold {
            LLVMPassBuilderOptionsSetInlinerThreshold(options, threshold as i32);
        }

        LLVMPassBuilderOptionsSetVerifyEach(options, config.verify_each.into());
        LLVMPassBuilderOptionsSetDebugLogging(options, config.debug_logging.into());
    }

    // Build the pipeline string
    let mut pipeline = config.pipeline_string();

    // Append extra passes if specified
    if let Some(extra) = &config.extra_passes {
        pipeline = format!("{pipeline},{extra}");
    }

    let pipeline_cstr =
        CString::new(pipeline.clone()).expect("pipeline string should not contain null bytes");

    // Get raw pointers for LLVM C API
    let module_ref = module.as_mut_ptr();
    let tm_ref = target_machine.as_mut_ptr();

    // Run the passes
    let error = unsafe { LLVMRunPasses(module_ref, pipeline_cstr.as_ptr(), tm_ref, options) };

    // Clean up options
    unsafe {
        LLVMDisposePassBuilderOptions(options);
    }

    // Check for errors
    if !error.is_null() {
        let message = unsafe {
            let msg_ptr = llvm_sys::error::LLVMGetErrorMessage(error);
            let msg = if msg_ptr.is_null() {
                "unknown error".to_string()
            } else {
                let msg = std::ffi::CStr::from_ptr(msg_ptr)
                    .to_string_lossy()
                    .into_owned();
                llvm_sys::error::LLVMDisposeErrorMessage(msg_ptr);
                msg
            };
            msg
        };
        return Err(OptimizationError::PassesFailed { message });
    }

    Ok(())
}

/// Run a custom pipeline string on a module.
///
/// This is a lower-level function that accepts any valid LLVM pipeline string.
///
/// # Pipeline String Format
///
/// The pipeline string follows LLVM's pass pipeline specification:
/// - `default<O3>` - Default O3 pipeline
/// - `instcombine,simplifycfg,dce` - Specific passes
/// - `function(instcombine)` - Function pass adapter
/// - `cgscc(inline)` - CGSCC pass adapter
///
/// # Example
///
/// ```ignore
/// // Run specific passes
/// run_custom_pipeline(&module, &tm, "instcombine,simplifycfg")?;
///
/// // Run with function adapter
/// run_custom_pipeline(&module, &tm, "function(mem2reg,instcombine)")?;
/// ```
pub fn run_custom_pipeline(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    pipeline: &str,
) -> Result<(), OptimizationError> {
    use llvm_sys::transforms::pass_builder::{
        LLVMCreatePassBuilderOptions, LLVMDisposePassBuilderOptions, LLVMRunPasses,
    };

    let options = unsafe { LLVMCreatePassBuilderOptions() };
    if options.is_null() {
        return Err(OptimizationError::PassBuilderOptionsCreationFailed);
    }

    let pipeline_cstr = CString::new(pipeline).map_err(|_| OptimizationError::InvalidPipeline {
        pipeline: pipeline.to_string(),
        message: "pipeline contains null bytes".to_string(),
    })?;

    let module_ref = module.as_mut_ptr();
    let tm_ref = target_machine.as_mut_ptr();

    let error = unsafe { LLVMRunPasses(module_ref, pipeline_cstr.as_ptr(), tm_ref, options) };

    unsafe {
        LLVMDisposePassBuilderOptions(options);
    }

    if !error.is_null() {
        let message = unsafe {
            let msg_ptr = llvm_sys::error::LLVMGetErrorMessage(error);
            let msg = if msg_ptr.is_null() {
                "unknown error".to_string()
            } else {
                let msg = std::ffi::CStr::from_ptr(msg_ptr)
                    .to_string_lossy()
                    .into_owned();
                llvm_sys::error::LLVMDisposeErrorMessage(msg_ptr);
                msg
            };
            msg
        };
        return Err(OptimizationError::PassesFailed { message });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- OptimizationLevel tests --

    #[test]
    fn test_optimization_level_pipeline_strings() {
        assert_eq!(OptimizationLevel::O0.pipeline_string(), "default<O0>");
        assert_eq!(OptimizationLevel::O1.pipeline_string(), "default<O1>");
        assert_eq!(OptimizationLevel::O2.pipeline_string(), "default<O2>");
        assert_eq!(OptimizationLevel::O3.pipeline_string(), "default<O3>");
        assert_eq!(OptimizationLevel::Os.pipeline_string(), "default<Os>");
        assert_eq!(OptimizationLevel::Oz.pipeline_string(), "default<Oz>");
    }

    #[test]
    fn test_optimization_level_vectorization() {
        assert!(!OptimizationLevel::O0.enables_loop_vectorization());
        assert!(!OptimizationLevel::O1.enables_loop_vectorization());
        assert!(OptimizationLevel::O2.enables_loop_vectorization());
        assert!(OptimizationLevel::O3.enables_loop_vectorization());
        assert!(!OptimizationLevel::Os.enables_loop_vectorization());
        assert!(!OptimizationLevel::Oz.enables_loop_vectorization());
    }

    #[test]
    fn test_optimization_level_unrolling() {
        assert!(!OptimizationLevel::O0.enables_loop_unrolling());
        assert!(OptimizationLevel::O1.enables_loop_unrolling());
        assert!(OptimizationLevel::O2.enables_loop_unrolling());
        assert!(OptimizationLevel::O3.enables_loop_unrolling());
        assert!(!OptimizationLevel::Os.enables_loop_unrolling());
        assert!(!OptimizationLevel::Oz.enables_loop_unrolling());
    }

    #[test]
    fn test_optimization_level_merge_functions() {
        assert!(!OptimizationLevel::O0.enables_merge_functions());
        assert!(!OptimizationLevel::O3.enables_merge_functions());
        assert!(OptimizationLevel::Os.enables_merge_functions());
        assert!(OptimizationLevel::Oz.enables_merge_functions());
    }

    #[test]
    fn test_optimization_level_display() {
        assert_eq!(format!("{}", OptimizationLevel::O0), "O0");
        assert_eq!(format!("{}", OptimizationLevel::O3), "O3");
        assert_eq!(format!("{}", OptimizationLevel::Os), "Os");
    }

    #[test]
    fn test_optimization_level_default() {
        assert_eq!(OptimizationLevel::default(), OptimizationLevel::O0);
    }

    // -- LtoMode tests --

    #[test]
    fn test_lto_mode_prelink_pipeline() {
        assert_eq!(
            LtoMode::Off.prelink_pipeline_string(OptimizationLevel::O2),
            None
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O2),
            Some("thinlto-pre-link<O2>".to_string())
        );
        assert_eq!(
            LtoMode::Full.prelink_pipeline_string(OptimizationLevel::O2),
            Some("lto-pre-link<O2>".to_string())
        );
    }

    #[test]
    fn test_lto_mode_lto_pipeline() {
        assert_eq!(
            LtoMode::Off.lto_pipeline_string(OptimizationLevel::O3),
            None
        );
        assert_eq!(
            LtoMode::Thin.lto_pipeline_string(OptimizationLevel::O3),
            Some("thinlto<O3>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O3),
            Some("lto<O3>".to_string())
        );
    }

    #[test]
    fn test_lto_mode_display() {
        assert_eq!(format!("{}", LtoMode::Off), "off");
        assert_eq!(format!("{}", LtoMode::Thin), "thin");
        assert_eq!(format!("{}", LtoMode::Full), "full");
    }

    // -- OptimizationConfig tests --

    #[test]
    fn test_config_presets() {
        let debug = OptimizationConfig::debug();
        assert_eq!(debug.level, OptimizationLevel::O0);
        assert_eq!(debug.lto, LtoMode::Off);

        let release = OptimizationConfig::release();
        assert_eq!(release.level, OptimizationLevel::O2);

        let aggressive = OptimizationConfig::aggressive();
        assert_eq!(aggressive.level, OptimizationLevel::O3);

        let size = OptimizationConfig::size();
        assert_eq!(size.level, OptimizationLevel::Os);

        let min_size = OptimizationConfig::min_size();
        assert_eq!(min_size.level, OptimizationLevel::Oz);
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = OptimizationConfig::new(OptimizationLevel::O2)
            .with_lto(LtoMode::Thin)
            .with_loop_vectorization(true)
            .with_slp_vectorization(false)
            .with_inliner_threshold(250)
            .with_verify_each(true);

        assert_eq!(config.level, OptimizationLevel::O2);
        assert_eq!(config.lto, LtoMode::Thin);
        assert_eq!(config.loop_vectorization, Some(true));
        assert_eq!(config.slp_vectorization, Some(false));
        assert_eq!(config.inliner_threshold, Some(250));
        assert!(config.verify_each);
    }

    #[test]
    fn test_config_pipeline_string_normal() {
        let config = OptimizationConfig::new(OptimizationLevel::O3);
        assert_eq!(config.pipeline_string(), "default<O3>");
    }

    #[test]
    fn test_config_pipeline_string_lto_prelink() {
        let config = OptimizationConfig::new(OptimizationLevel::O2).with_lto(LtoMode::Thin);
        assert_eq!(config.pipeline_string(), "thinlto-pre-link<O2>");
    }

    #[test]
    fn test_config_pipeline_string_lto_phase() {
        let config = OptimizationConfig::new(OptimizationLevel::O2)
            .with_lto(LtoMode::Full)
            .as_lto_phase();
        assert_eq!(config.pipeline_string(), "lto<O2>");
    }

    #[test]
    fn test_config_effective_settings() {
        // O2 should enable vectorization by default
        let o2 = OptimizationConfig::new(OptimizationLevel::O2);
        assert!(o2.effective_loop_vectorization());
        assert!(o2.effective_slp_vectorization());
        assert!(o2.effective_loop_unrolling());

        // O0 should disable vectorization by default
        let o0 = OptimizationConfig::new(OptimizationLevel::O0);
        assert!(!o0.effective_loop_vectorization());
        assert!(!o0.effective_loop_unrolling());

        // Override should work
        let overridden =
            OptimizationConfig::new(OptimizationLevel::O0).with_loop_vectorization(true);
        assert!(overridden.effective_loop_vectorization());
    }

    #[test]
    fn test_config_extra_passes() {
        let config =
            OptimizationConfig::new(OptimizationLevel::O2).with_extra_passes("instcombine,dce");
        assert_eq!(config.extra_passes, Some("instcombine,dce".to_string()));
    }

    // -- Error tests --

    #[test]
    fn test_optimization_error_display() {
        let err = OptimizationError::PassBuilderOptionsCreationFailed;
        assert!(format!("{err}").contains("pass builder"));

        let err = OptimizationError::PassesFailed {
            message: "test error".to_string(),
        };
        assert!(format!("{err}").contains("test error"));

        let err = OptimizationError::InvalidPipeline {
            pipeline: "bad".to_string(),
            message: "invalid".to_string(),
        };
        assert!(format!("{err}").contains("bad"));
    }

    // -- Integration tests (require LLVM) --

    #[test]
    fn test_run_optimization_passes_o0() {
        use inkwell::context::Context;

        // Initialize native target
        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            // Skip if native target unavailable
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        // Create a simple function
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("identity", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        builder.build_return(Some(&param)).unwrap();

        // Create target machine
        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::None,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        // Configure module
        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O0 optimization (should succeed even with minimal passes)
        let config = OptimizationConfig::debug();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O0 optimization failed: {:?}", result);
    }

    #[test]
    fn test_run_optimization_passes_o2() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        // Create a function with some optimization opportunity
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("add_zero", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        let zero = i64_type.const_int(0, false);
        let result = builder.build_int_add(param, zero, "add").unwrap();
        builder.build_return(Some(&result)).unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O2 optimization
        let config = OptimizationConfig::release();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O2 optimization failed: {:?}", result);
    }

    #[test]
    fn test_run_custom_pipeline() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function("empty", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder.build_return(None).unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::None,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run a custom minimal pipeline
        let result = run_custom_pipeline(&module, &target_machine, "function(verify)");
        assert!(result.is_ok(), "Custom pipeline failed: {:?}", result);
    }

    #[test]
    fn test_config_with_extra_passes_integration() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = module.add_function("const_42", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let val = i64_type.const_int(42, false);
        builder.build_return(Some(&val)).unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::None,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run with extra passes
        let config = OptimizationConfig::debug().with_extra_passes("function(verify)");
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(
            result.is_ok(),
            "Optimization with extra passes failed: {:?}",
            result
        );
    }

    #[test]
    fn test_optimization_level_display_all() {
        assert_eq!(format!("{}", OptimizationLevel::O0), "O0");
        assert_eq!(format!("{}", OptimizationLevel::O1), "O1");
        assert_eq!(format!("{}", OptimizationLevel::O2), "O2");
        assert_eq!(format!("{}", OptimizationLevel::O3), "O3");
        assert_eq!(format!("{}", OptimizationLevel::Os), "Os");
        assert_eq!(format!("{}", OptimizationLevel::Oz), "Oz");
    }

    #[test]
    fn test_lto_mode_pipelines_all_opt_levels() {
        // Test all opt levels with Thin LTO
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O0),
            Some("thinlto-pre-link<O0>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O1),
            Some("thinlto-pre-link<O1>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::Os),
            Some("thinlto-pre-link<Os>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::Oz),
            Some("thinlto-pre-link<Oz>".to_string())
        );

        // Test all opt levels with Full LTO
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O0),
            Some("lto<O0>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O1),
            Some("lto<O1>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::Os),
            Some("lto<Os>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::Oz),
            Some("lto<Oz>".to_string())
        );
    }

    #[test]
    fn test_config_effective_loop_interleaving() {
        // Loop interleaving follows loop unrolling by default
        let config = OptimizationConfig::new(OptimizationLevel::O2);
        assert!(config.effective_loop_interleaving());

        let config = OptimizationConfig::new(OptimizationLevel::O0);
        assert!(!config.effective_loop_interleaving());

        // Can be explicitly set
        let config = OptimizationConfig::new(OptimizationLevel::O0).with_loop_unrolling(false);
        let config = OptimizationConfig {
            loop_interleaving: Some(true),
            ..config
        };
        assert!(config.effective_loop_interleaving());
    }

    #[test]
    fn test_config_effective_merge_functions() {
        // Size levels enable merge_functions by default
        let config = OptimizationConfig::new(OptimizationLevel::Os);
        assert!(config.effective_merge_functions());

        let config = OptimizationConfig::new(OptimizationLevel::Oz);
        assert!(config.effective_merge_functions());

        // O3 doesn't enable merge_functions by default
        let config = OptimizationConfig::new(OptimizationLevel::O3);
        assert!(!config.effective_merge_functions());

        // Can be explicitly enabled
        let config = OptimizationConfig::new(OptimizationLevel::O3).with_merge_functions(true);
        assert!(config.effective_merge_functions());
    }

    #[test]
    fn test_config_with_debug_logging() {
        let config = OptimizationConfig::new(OptimizationLevel::O2).with_debug_logging(true);
        assert!(config.debug_logging);
    }

    #[test]
    fn test_lto_mode_default() {
        assert_eq!(LtoMode::default(), LtoMode::Off);
    }

    #[test]
    fn test_run_optimization_passes_o3() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_o3");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("square", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        let result = builder.build_int_mul(param, param, "sq").unwrap();
        builder.build_return(Some(&result)).unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Aggressive,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O3 optimization
        let config = OptimizationConfig::aggressive();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O3 optimization failed: {:?}", result);
    }

    #[test]
    fn test_run_optimization_passes_size() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_size");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = module.add_function("const_val", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder
            .build_return(Some(&i64_type.const_int(42, false)))
            .unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run Os optimization
        let config = OptimizationConfig::size();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "Os optimization failed: {:?}", result);

        // Run Oz optimization
        let config = OptimizationConfig::min_size();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "Oz optimization failed: {:?}", result);
    }

    #[test]
    fn test_invalid_custom_pipeline() {
        use inkwell::context::Context;

        if inkwell::targets::Target::initialize_native(&Default::default()).is_err() {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_invalid");

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function("empty", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder.build_return(None).unwrap();

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::None,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run with invalid pipeline - should fail
        let result = run_custom_pipeline(&module, &target_machine, "not-a-real-pass");
        assert!(result.is_err());
        if let Err(OptimizationError::PassesFailed { message }) = result {
            assert!(!message.is_empty());
        }
    }
}
