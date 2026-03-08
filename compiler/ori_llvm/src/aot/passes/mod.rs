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

mod config;

pub use config::{LtoMode, OptimizationConfig, OptimizationLevel};

use inkwell::module::Module;
use inkwell::targets::TargetMachine;
use std::ffi::CString;
use std::fmt;
use std::path::Path;

/// Error type for the module codegen pipeline (verify → optimize → emit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationError {
    /// LLVM module verification failed.
    ///
    /// This indicates a compiler bug: the generated IR is malformed.
    /// Caught unconditionally before optimization to prevent LLVM segfaults.
    VerificationFailed { message: String },

    /// Failed to create pass builder options.
    PassBuilderOptionsCreationFailed,

    /// Failed to run optimization passes.
    PassesFailed { message: String },

    /// Invalid pass pipeline string.
    InvalidPipeline { pipeline: String, message: String },

    /// Failed to write bitcode file during LTO pre-link phase.
    BitcodeWriteFailed { path: String },
}

impl fmt::Display for OptimizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerificationFailed { message } => {
                write!(f, "LLVM IR verification failed: {message}")
            }
            Self::PassBuilderOptionsCreationFailed => {
                write!(f, "failed to create pass builder options")
            }
            Self::PassesFailed { message } => {
                write!(f, "optimization passes failed: {message}")
            }
            Self::InvalidPipeline { pipeline, message } => {
                write!(f, "invalid pipeline '{pipeline}': {message}")
            }
            Self::BitcodeWriteFailed { path } => {
                write!(f, "failed to write bitcode file '{path}'")
            }
        }
    }
}

impl std::error::Error for OptimizationError {}

/// Extract error message from LLVM error, disposing the error.
///
/// # Safety
/// The error pointer must be valid and non-null.
unsafe fn extract_llvm_error_message(error: llvm_sys::error::LLVMErrorRef) -> String {
    let msg_ptr = llvm_sys::error::LLVMGetErrorMessage(error);
    if msg_ptr.is_null() {
        "unknown error".to_string()
    } else {
        let msg = std::ffi::CStr::from_ptr(msg_ptr)
            .to_string_lossy()
            .into_owned();
        llvm_sys::error::LLVMDisposeErrorMessage(msg_ptr);
        msg
    }
}

/// RAII guard for `LLVMPassBuilderOptionsRef`.
///
/// Ensures proper cleanup of pass builder options even on early returns or panics.
struct PassBuilderOptionsGuard {
    options: llvm_sys::transforms::pass_builder::LLVMPassBuilderOptionsRef,
}

impl PassBuilderOptionsGuard {
    /// Create a new pass builder options guard.
    ///
    /// Returns `None` if LLVM fails to create the options.
    fn new() -> Option<Self> {
        // SAFETY: LLVMCreatePassBuilderOptions has no preconditions; returns null on failure.
        let options = unsafe { llvm_sys::transforms::pass_builder::LLVMCreatePassBuilderOptions() };
        if options.is_null() {
            None
        } else {
            Some(Self { options })
        }
    }

    /// Get the underlying options pointer for LLVM API calls.
    fn as_ptr(&self) -> llvm_sys::transforms::pass_builder::LLVMPassBuilderOptionsRef {
        self.options
    }
}

impl Drop for PassBuilderOptionsGuard {
    fn drop(&mut self) {
        // SAFETY: self.options is a valid, non-null LLVMPassBuilderOptionsRef created
        // in new() and not yet disposed. Each guard is dropped exactly once.
        unsafe {
            llvm_sys::transforms::pass_builder::LLVMDisposePassBuilderOptions(self.options);
        }
    }
}

/// Run the optimization pipeline with mandatory pre-verification.
///
/// Pipeline: `verify_module` → `run_optimization_passes`
///
/// Verification is unconditional — catching invalid IR early prevents
/// LLVM segfaults during optimization or object emission. This is cheap
/// relative to optimization and should always be used instead of calling
/// `run_optimization_passes` directly.
///
/// # Arguments
///
/// * `module` - The LLVM module to verify and optimize
/// * `target_machine` - Target machine for target-specific optimizations
/// * `config` - Optimization configuration
///
/// # Errors
///
/// Returns `VerificationFailed` if the module contains invalid IR (compiler bug).
/// Returns other `OptimizationError` variants if the pass pipeline fails.
pub fn optimize_module(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    config: &OptimizationConfig,
) -> Result<(), OptimizationError> {
    // Step 1: Verify module (unconditional)
    if let Err(msg) = module.verify() {
        return Err(OptimizationError::VerificationFailed {
            message: msg.to_string(),
        });
    }

    // Step 2: Run optimization passes
    run_optimization_passes(module, target_machine, config)
}

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
        LLVMPassBuilderOptionsSetDebugLogging, LLVMPassBuilderOptionsSetInlinerThreshold,
        LLVMPassBuilderOptionsSetLoopInterleaving, LLVMPassBuilderOptionsSetLoopUnrolling,
        LLVMPassBuilderOptionsSetLoopVectorization, LLVMPassBuilderOptionsSetMergeFunctions,
        LLVMPassBuilderOptionsSetSLPVectorization, LLVMPassBuilderOptionsSetVerifyEach,
        LLVMRunPasses,
    };

    // Create pass builder options with RAII cleanup
    let guard = PassBuilderOptionsGuard::new()
        .ok_or(OptimizationError::PassBuilderOptionsCreationFailed)?;

    // SAFETY: guard.as_ptr() is a valid, non-null LLVMPassBuilderOptionsRef.
    // All setter functions take a valid options ref and a boolean/integer value.
    unsafe {
        LLVMPassBuilderOptionsSetLoopVectorization(
            guard.as_ptr(),
            config.effective_loop_vectorization().into(),
        );
        LLVMPassBuilderOptionsSetSLPVectorization(
            guard.as_ptr(),
            config.effective_slp_vectorization().into(),
        );
        LLVMPassBuilderOptionsSetLoopUnrolling(
            guard.as_ptr(),
            config.effective_loop_unrolling().into(),
        );
        LLVMPassBuilderOptionsSetLoopInterleaving(
            guard.as_ptr(),
            config.effective_loop_interleaving().into(),
        );
        LLVMPassBuilderOptionsSetMergeFunctions(
            guard.as_ptr(),
            config.effective_merge_functions().into(),
        );

        if let Some(threshold) = config.inliner_threshold {
            LLVMPassBuilderOptionsSetInlinerThreshold(guard.as_ptr(), threshold as i32);
        }

        LLVMPassBuilderOptionsSetVerifyEach(guard.as_ptr(), config.verify_each.into());
        LLVMPassBuilderOptionsSetDebugLogging(guard.as_ptr(), config.debug_logging.into());
    }

    // Build the pipeline string
    let mut pipeline = config.pipeline_string();

    // Append extra passes if specified (using push_str to avoid allocation)
    if let Some(extra) = &config.extra_passes {
        pipeline.push(',');
        pipeline.push_str(extra);
    }

    let pipeline_cstr = CString::new(pipeline).map_err(|e| OptimizationError::InvalidPipeline {
        pipeline: String::from_utf8_lossy(&e.into_vec()).into_owned(),
        message: "pipeline contains null bytes".to_string(),
    })?;

    // Get raw pointers for LLVM C API
    let module_ref = module.as_mut_ptr();
    let tm_ref = target_machine.as_mut_ptr();

    // SAFETY: module_ref and tm_ref are valid LLVM refs from inkwell RAII wrappers.
    // pipeline_cstr is a valid null-terminated C string. guard.as_ptr() is valid options.
    let error =
        unsafe { LLVMRunPasses(module_ref, pipeline_cstr.as_ptr(), tm_ref, guard.as_ptr()) };

    // Check for errors
    if !error.is_null() {
        // SAFETY: error is a non-null LLVMErrorRef returned by LLVMRunPasses.
        // extract_llvm_error_message consumes and disposes it.
        let message = unsafe { extract_llvm_error_message(error) };
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
    use llvm_sys::transforms::pass_builder::LLVMRunPasses;

    // Create pass builder options with RAII cleanup
    let guard = PassBuilderOptionsGuard::new()
        .ok_or(OptimizationError::PassBuilderOptionsCreationFailed)?;

    let pipeline_cstr = CString::new(pipeline).map_err(|_| OptimizationError::InvalidPipeline {
        pipeline: pipeline.to_string(),
        message: "pipeline contains null bytes".to_string(),
    })?;

    let module_ref = module.as_mut_ptr();
    let tm_ref = target_machine.as_mut_ptr();

    // SAFETY: module_ref and tm_ref are valid LLVM refs from inkwell RAII wrappers.
    // pipeline_cstr is a valid null-terminated C string. guard.as_ptr() is valid options.
    let error =
        unsafe { LLVMRunPasses(module_ref, pipeline_cstr.as_ptr(), tm_ref, guard.as_ptr()) };

    if !error.is_null() {
        // SAFETY: error is a non-null LLVMErrorRef returned by LLVMRunPasses.
        let message = unsafe { extract_llvm_error_message(error) };
        return Err(OptimizationError::PassesFailed { message });
    }

    Ok(())
}

// LTO Pipeline

/// Run the pre-link optimization phase for LTO.
///
/// When LTO is enabled, per-module compilation uses the pre-link pipeline
/// (e.g., `thinlto-pre-link<O2>`) instead of the full pipeline. After
/// pre-link optimization, the module should be emitted as bitcode (not
/// object code) for the subsequent LTO merge phase.
///
/// This function verifies the module, runs the pre-link pipeline, and
/// writes bitcode to the specified path.
///
/// # Errors
///
/// Returns `VerificationFailed` if the module IR is invalid.
/// Returns `PassesFailed` if the pre-link pipeline fails.
/// Returns `BitcodeWriteFailed` if the bitcode file cannot be written.
pub fn prelink_and_emit_bitcode(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    config: &OptimizationConfig,
    bitcode_path: &Path,
) -> Result<(), OptimizationError> {
    // Verify first
    if let Err(msg) = module.verify() {
        return Err(OptimizationError::VerificationFailed {
            message: msg.to_string(),
        });
    }

    // Run pre-link pipeline (config.pipeline_string() already returns
    // the pre-link variant when LTO is configured)
    run_optimization_passes(module, target_machine, config)?;

    // Emit bitcode
    if !module.write_bitcode_to_path(bitcode_path) {
        return Err(OptimizationError::BitcodeWriteFailed {
            path: bitcode_path.to_string_lossy().into_owned(),
        });
    }

    Ok(())
}

/// Run the LTO phase on a merged module.
///
/// After all per-module bitcode files have been linked into a single module,
/// this function runs the LTO pipeline (`thinlto<O2>` or `lto<O2>`) and
/// produces the final optimized module.
///
/// # Arguments
///
/// * `module` - The merged LLVM module (all per-module bitcode linked together)
/// * `target_machine` - Target machine for optimization
/// * `config` - Base optimization config (must have LTO mode set)
///
/// # Errors
///
/// Returns `VerificationFailed` if the merged module is invalid.
/// Returns `PassesFailed` if the LTO pipeline fails.
pub fn run_lto_pipeline(
    module: &Module<'_>,
    target_machine: &TargetMachine,
    config: &OptimizationConfig,
) -> Result<(), OptimizationError> {
    // Verify the merged module
    if let Err(msg) = module.verify() {
        return Err(OptimizationError::VerificationFailed {
            message: msg.to_string(),
        });
    }

    // Create LTO phase config
    let lto_config = OptimizationConfig {
        is_lto_phase: true,
        ..config.clone()
    };

    // Run LTO pipeline (pipeline_string() returns the LTO variant when
    // is_lto_phase is true)
    run_optimization_passes(module, target_machine, &lto_config)
}
