//! Object File Emission for AOT Compilation
//!
//! Provides functionality to emit LLVM modules as object files for various platforms:
//! - ELF (Linux)
//! - Mach-O (macOS)
//! - COFF (Windows)
//! - WASM (WebAssembly)
//!
//! # Architecture
//!
//! Object file emission is the bridge between LLVM IR and the native linker:
//!
//! ```text
//! ┌─────────────┐    ┌──────────────┐    ┌─────────────┐
//! │  LLVM IR    │───▶│ TargetMachine│───▶│ Object File │
//! │  (Module)   │    │  + FileType  │    │  (.o/.obj)  │
//! └─────────────┘    └──────────────┘    └─────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, ObjectEmitter, EmitOptions};
//!
//! let target = TargetConfig::native()?;
//! let emitter = ObjectEmitter::new(&target)?;
//!
//! // Emit object file
//! emitter.emit_object(&module, Path::new("output.o"))?;
//!
//! // Emit assembly (for debugging)
//! emitter.emit_assembly(&module, Path::new("output.s"))?;
//! ```

use std::fmt;
use std::path::Path;

use inkwell::module::Module;
use inkwell::targets::{FileType, TargetMachine};

use super::target::{TargetConfig, TargetError};

/// Error type for object file emission operations.
#[derive(Debug, Clone)]
pub enum EmitError {
    /// Failed to create target machine.
    TargetMachine(TargetError),
    /// Failed to configure module with target settings.
    ModuleConfiguration(TargetError),
    /// Failed to emit object file.
    ObjectEmission { path: String, message: String },
    /// Failed to emit assembly file.
    AssemblyEmission { path: String, message: String },
    /// Failed to emit LLVM bitcode.
    BitcodeEmission { path: String, message: String },
    /// Failed to emit LLVM IR text.
    LlvmIrEmission { path: String, message: String },
    /// Output path is not valid.
    InvalidPath { path: String, reason: String },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMachine(err) => {
                write!(f, "failed to create target machine: {err}")
            }
            Self::ModuleConfiguration(err) => {
                write!(f, "failed to configure module: {err}")
            }
            Self::ObjectEmission { path, message } => {
                write!(f, "failed to emit object file '{path}': {message}")
            }
            Self::AssemblyEmission { path, message } => {
                write!(f, "failed to emit assembly file '{path}': {message}")
            }
            Self::BitcodeEmission { path, message } => {
                write!(f, "failed to emit bitcode file '{path}': {message}")
            }
            Self::LlvmIrEmission { path, message } => {
                write!(f, "failed to emit LLVM IR file '{path}': {message}")
            }
            Self::InvalidPath { path, reason } => {
                write!(f, "invalid output path '{path}': {reason}")
            }
        }
    }
}

impl std::error::Error for EmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetMachine(err) | Self::ModuleConfiguration(err) => Some(err),
            _ => None,
        }
    }
}

impl From<TargetError> for EmitError {
    fn from(err: TargetError) -> Self {
        Self::TargetMachine(err)
    }
}

/// Error type for the full verify → optimize → emit pipeline.
///
/// Wraps the individual error types from each pipeline stage.
#[derive(Debug, Clone)]
pub enum ModulePipelineError {
    /// LLVM IR verification failed (compiler bug).
    Verification(String),
    /// Optimization pass pipeline failed.
    Optimization(super::passes::OptimizationError),
    /// Object/bitcode/IR emission failed.
    Emission(EmitError),
}

impl fmt::Display for ModulePipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(msg) => write!(f, "LLVM IR verification failed: {msg}"),
            Self::Optimization(err) => write!(f, "optimization failed: {err}"),
            Self::Emission(err) => write!(f, "emission failed: {err}"),
        }
    }
}

impl std::error::Error for ModulePipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Verification(_) => None,
            Self::Optimization(err) => Some(err),
            Self::Emission(err) => Some(err),
        }
    }
}

/// Validate that the parent directory exists for an output path.
fn validate_parent_exists(path: &Path) -> Result<(), EmitError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(EmitError::InvalidPath {
                path: path.to_string_lossy().into_owned(),
                reason: "parent directory does not exist".to_string(),
            });
        }
    }
    Ok(())
}

/// Output format for code emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Native object file (.o on Unix, .obj on Windows).
    Object,
    /// Assembly text (.s).
    Assembly,
    /// LLVM bitcode (.bc).
    Bitcode,
    /// LLVM IR text (.ll).
    LlvmIr,
}

impl OutputFormat {
    /// Get the typical file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Object => "o",
            Self::Assembly => "s",
            Self::Bitcode => "bc",
            Self::LlvmIr => "ll",
        }
    }

    /// Get a human-readable description of this format.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Object => "native object file",
            Self::Assembly => "assembly text",
            Self::Bitcode => "LLVM bitcode",
            Self::LlvmIr => "LLVM IR text",
        }
    }
}

/// Object file emitter for a specific target.
///
/// Wraps an LLVM `TargetMachine` and provides methods to emit various output formats.
pub struct ObjectEmitter {
    /// The underlying LLVM target machine.
    machine: TargetMachine,
    /// The target configuration used to create this emitter.
    config: TargetConfig,
}

impl ObjectEmitter {
    /// Create a new object emitter for the given target configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the target machine cannot be created.
    pub fn new(config: &TargetConfig) -> Result<Self, EmitError> {
        let machine = config.create_target_machine()?;
        Ok(Self {
            machine,
            config: config.clone(),
        })
    }

    /// Create a new object emitter for the native (host) target.
    ///
    /// # Errors
    ///
    /// Returns an error if native target detection or machine creation fails.
    pub fn native() -> Result<Self, EmitError> {
        let config = TargetConfig::native()?;
        Self::new(&config)
    }

    /// Get the target configuration for this emitter.
    #[must_use]
    pub fn config(&self) -> &TargetConfig {
        &self.config
    }

    /// Get a reference to the underlying LLVM target machine.
    #[must_use]
    pub fn machine(&self) -> &TargetMachine {
        &self.machine
    }

    /// Configure a module with target triple and data layout.
    ///
    /// This must be called before emitting the module. It sets:
    /// - Target triple (e.g., "x86_64-unknown-linux-gnu")
    /// - Data layout (pointer sizes, alignments, endianness)
    ///
    /// # Errors
    ///
    /// Returns an error if module configuration fails.
    pub fn configure_module(&self, module: &Module<'_>) -> Result<(), EmitError> {
        self.config
            .configure_module(module)
            .map_err(EmitError::ModuleConfiguration)
    }

    /// Emit a module as a native object file.
    ///
    /// The output format depends on the target:
    /// - Linux: ELF object file
    /// - macOS: Mach-O object file
    /// - Windows: COFF object file
    /// - WebAssembly: WASM object file
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module to emit
    /// * `path` - Output file path (typically ends with `.o` or `.obj`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The module is not configured for this target
    /// - LLVM fails to generate the object file
    /// - The output path is invalid
    pub fn emit_object(&self, module: &Module<'_>, path: &Path) -> Result<(), EmitError> {
        self.emit_to_file(module, path, FileType::Object, OutputFormat::Object)
    }

    /// Emit a module as assembly text.
    ///
    /// Useful for debugging code generation issues.
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module to emit
    /// * `path` - Output file path (typically ends with `.s`)
    ///
    /// # Errors
    ///
    /// Returns an error if LLVM fails to generate the assembly.
    pub fn emit_assembly(&self, module: &Module<'_>, path: &Path) -> Result<(), EmitError> {
        self.emit_to_file(module, path, FileType::Assembly, OutputFormat::Assembly)
    }

    /// Emit a module as LLVM bitcode.
    ///
    /// Bitcode is useful for:
    /// - Link-Time Optimization (LTO)
    /// - Caching compiled modules
    /// - Cross-platform distribution
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module to emit
    /// * `path` - Output file path (typically ends with `.bc`)
    ///
    /// # Errors
    ///
    /// Returns an error if the bitcode cannot be written.
    pub fn emit_bitcode(&self, module: &Module<'_>, path: &Path) -> Result<(), EmitError> {
        let path_str = path.to_string_lossy();

        validate_parent_exists(path)?;

        // Write bitcode using inkwell's built-in method
        if module.write_bitcode_to_path(path) {
            Ok(())
        } else {
            Err(EmitError::BitcodeEmission {
                path: path_str.to_string(),
                message: "LLVM failed to write bitcode".to_string(),
            })
        }
    }

    /// Emit a module as LLVM IR text.
    ///
    /// Human-readable LLVM IR is useful for debugging and understanding
    /// the generated code.
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module to emit
    /// * `path` - Output file path (typically ends with `.ll`)
    ///
    /// # Errors
    ///
    /// Returns an error if the IR cannot be written.
    pub fn emit_llvm_ir(&self, module: &Module<'_>, path: &Path) -> Result<(), EmitError> {
        let path_str = path.to_string_lossy();

        validate_parent_exists(path)?;

        // Print IR to file
        module
            .print_to_file(path)
            .map_err(|e| EmitError::LlvmIrEmission {
                path: path_str.to_string(),
                message: e.to_string(),
            })
    }

    /// Emit a module in the specified format.
    ///
    /// This is a convenience method that dispatches to the appropriate
    /// emit method based on the format.
    ///
    /// # Errors
    ///
    /// Returns an error if emission fails.
    pub fn emit(
        &self,
        module: &Module<'_>,
        path: &Path,
        format: OutputFormat,
    ) -> Result<(), EmitError> {
        match format {
            OutputFormat::Object => self.emit_object(module, path),
            OutputFormat::Assembly => self.emit_assembly(module, path),
            OutputFormat::Bitcode => self.emit_bitcode(module, path),
            OutputFormat::LlvmIr => self.emit_llvm_ir(module, path),
        }
    }

    /// Run the full verify → optimize → emit pipeline for a module.
    ///
    /// This is the recommended entry point for emitting optimized code.
    /// Callers do not need to invoke verification or optimization separately.
    ///
    /// Pipeline:
    /// 1. Verify module IR (unconditional — catches codegen bugs early)
    /// 2. Run optimization passes (per `OptimizationConfig`)
    /// 3. Emit to the requested output format
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module (must be configured with `configure_module` first)
    /// * `opt_config` - Optimization configuration (level, LTO, vectorization, etc.)
    /// * `path` - Output file path
    /// * `format` - Output format (object, assembly, bitcode, LLVM IR)
    ///
    /// # Errors
    ///
    /// Returns an error if verification, optimization, or emission fails.
    pub fn verify_optimize_emit(
        &self,
        module: &Module<'_>,
        opt_config: &super::passes::OptimizationConfig,
        path: &Path,
        format: OutputFormat,
    ) -> Result<(), ModulePipelineError> {
        // Step 1: Verify
        if let Err(msg) = module.verify() {
            return Err(ModulePipelineError::Verification(msg.to_string()));
        }

        // Step 2: Optimize
        super::passes::run_optimization_passes(module, &self.machine, opt_config)
            .map_err(ModulePipelineError::Optimization)?;

        // Step 3: Emit
        self.emit(module, path, format)
            .map_err(ModulePipelineError::Emission)?;

        Ok(())
    }

    /// Emit a module to a memory buffer as an object file.
    ///
    /// Returns the raw bytes of the object file, which can be written
    /// to disk or processed further.
    ///
    /// # Errors
    ///
    /// Returns an error if LLVM fails to generate the object code.
    pub fn emit_object_to_memory(&self, module: &Module<'_>) -> Result<Vec<u8>, EmitError> {
        self.emit_to_memory(module, FileType::Object)
    }

    /// Emit a module to a memory buffer as assembly text.
    ///
    /// Returns the assembly text as bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if LLVM fails to generate the assembly.
    pub fn emit_assembly_to_memory(&self, module: &Module<'_>) -> Result<Vec<u8>, EmitError> {
        self.emit_to_memory(module, FileType::Assembly)
    }

    // -- Internal helpers --

    /// Emit to a file using LLVM's target machine.
    fn emit_to_file(
        &self,
        module: &Module<'_>,
        path: &Path,
        file_type: FileType,
        output_format: OutputFormat,
    ) -> Result<(), EmitError> {
        let path_str = path.to_string_lossy();

        validate_parent_exists(path)?;

        // Emit using LLVM
        self.machine
            .write_to_file(module, file_type, path)
            .map_err(|e| match output_format {
                OutputFormat::Object | OutputFormat::Bitcode | OutputFormat::LlvmIr => {
                    EmitError::ObjectEmission {
                        path: path_str.to_string(),
                        message: e.to_string(),
                    }
                }
                OutputFormat::Assembly => EmitError::AssemblyEmission {
                    path: path_str.to_string(),
                    message: e.to_string(),
                },
            })
    }

    /// Emit to a memory buffer using LLVM's target machine.
    fn emit_to_memory(
        &self,
        module: &Module<'_>,
        file_type: FileType,
    ) -> Result<Vec<u8>, EmitError> {
        let buffer = self
            .machine
            .write_to_memory_buffer(module, file_type)
            .map_err(|e| EmitError::ObjectEmission {
                path: "<memory>".to_string(),
                message: e.to_string(),
            })?;

        Ok(buffer.as_slice().to_vec())
    }
}

impl fmt::Debug for ObjectEmitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Note: TargetMachine doesn't implement Debug, so we show config details instead
        f.debug_struct("ObjectEmitter")
            .field("target", &self.config.triple())
            .field("cpu", &self.config.cpu())
            .field("features", &self.config.features())
            .finish_non_exhaustive()
    }
}
