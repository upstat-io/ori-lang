//! Configuration types for the optimization pipeline.
//!
//! Contains [`OptimizationLevel`], [`LtoMode`], and [`OptimizationConfig`]
//! which define how LLVM optimization passes are configured.

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

/// Get the pipeline level suffix string for an optimization level.
fn level_suffix(level: OptimizationLevel) -> &'static str {
    match level {
        OptimizationLevel::O0 => "O0",
        OptimizationLevel::O1 => "O1",
        OptimizationLevel::O2 => "O2",
        OptimizationLevel::O3 => "O3",
        OptimizationLevel::Os => "Os",
        OptimizationLevel::Oz => "Oz",
    }
}

impl LtoMode {
    /// Get the pre-link pipeline string for this LTO mode.
    ///
    /// Returns `None` if LTO is off (use regular `default<OX>` instead).
    #[must_use]
    pub fn prelink_pipeline_string(&self, opt_level: OptimizationLevel) -> Option<String> {
        let level = level_suffix(opt_level);

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
        let level = level_suffix(opt_level);

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

    /// Create an aggressive release configuration (O3, maximum performance).
    #[must_use]
    pub fn release_fast() -> Self {
        Self::new(OptimizationLevel::O3)
    }

    /// Create a size-optimized configuration (Os).
    #[must_use]
    pub fn release_small() -> Self {
        Self::new(OptimizationLevel::Os)
    }

    /// Create an aggressive size-optimized configuration (Oz, smallest code).
    #[must_use]
    pub fn release_min_size() -> Self {
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
