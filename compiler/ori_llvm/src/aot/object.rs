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

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use std::error::Error;

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Object.extension(), "o");
        assert_eq!(OutputFormat::Assembly.extension(), "s");
        assert_eq!(OutputFormat::Bitcode.extension(), "bc");
        assert_eq!(OutputFormat::LlvmIr.extension(), "ll");
    }

    #[test]
    fn test_output_format_description() {
        assert_eq!(OutputFormat::Object.description(), "native object file");
        assert_eq!(OutputFormat::Assembly.description(), "assembly text");
        assert_eq!(OutputFormat::Bitcode.description(), "LLVM bitcode");
        assert_eq!(OutputFormat::LlvmIr.description(), "LLVM IR text");
    }

    #[test]
    fn test_object_emitter_native() {
        // May fail on systems without proper LLVM setup
        if let Ok(emitter) = ObjectEmitter::native() {
            assert!(!emitter.config().triple().is_empty());
        }
    }

    #[test]
    fn test_object_emitter_configure_module() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test");

            let result = emitter.configure_module(&module);
            assert!(result.is_ok());

            // Module should have triple set
            let triple = module.get_triple();
            assert!(!triple.as_str().to_string_lossy().is_empty());
        }
    }

    #[test]
    fn test_emit_llvm_ir() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_ir");

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("test_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(42, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.ll");

            let result = emitter.emit_llvm_ir(&module, &path);
            assert!(result.is_ok(), "emit_llvm_ir failed: {:?}", result);

            // Verify file exists and contains expected content
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("test_func"));
            assert!(content.contains("ret i64 42"));

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_bitcode() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_bc");

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("bc_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(100, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.bc");

            let result = emitter.emit_bitcode(&module, &path);
            assert!(result.is_ok(), "emit_bitcode failed: {:?}", result);

            // Verify file exists and has content
            let metadata = std::fs::metadata(&path).unwrap();
            assert!(metadata.len() > 0);

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_object() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_obj");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("obj_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(200, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.o");

            let result = emitter.emit_object(&module, &path);
            assert!(result.is_ok(), "emit_object failed: {:?}", result);

            // Verify file exists and has content
            let metadata = std::fs::metadata(&path).unwrap();
            assert!(metadata.len() > 0);

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_assembly() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_asm");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("asm_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(300, false)))
                .unwrap();

            // Emit to temp file
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_emit.s");

            let result = emitter.emit_assembly(&module, &path);
            assert!(result.is_ok(), "emit_assembly failed: {:?}", result);

            // Verify file exists and contains assembly
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("asm_func") || content.contains("_asm_func"));

            // Cleanup
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_object_to_memory() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_mem");

            // Configure module for target
            emitter.configure_module(&module).unwrap();

            // Create a simple function
            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("mem_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(400, false)))
                .unwrap();

            let result = emitter.emit_object_to_memory(&module);
            assert!(result.is_ok(), "emit_object_to_memory failed: {:?}", result);

            let bytes = result.unwrap();
            assert!(!bytes.is_empty());
        }
    }

    #[test]
    fn test_emit_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_invalid");

            // Try to emit to a non-existent directory
            let path = Path::new("/nonexistent/directory/file.o");
            let result = emitter.emit_object(&module, path);

            assert!(matches!(result, Err(EmitError::InvalidPath { .. })));
        }
    }

    #[test]
    fn test_emit_error_display() {
        let err = EmitError::ObjectEmission {
            path: "test.o".to_string(),
            message: "LLVM error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit object file 'test.o': LLVM error"
        );

        let err = EmitError::InvalidPath {
            path: "/bad/path".to_string(),
            reason: "does not exist".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid output path '/bad/path': does not exist"
        );
    }

    #[test]
    fn test_object_emitter_debug() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let debug_str = format!("{:?}", emitter);
            assert!(debug_str.contains("ObjectEmitter"));
            assert!(debug_str.contains("target"));
        }
    }

    #[test]
    fn test_object_emitter_machine() {
        if let Ok(emitter) = ObjectEmitter::native() {
            // Verify we can access the machine and it has expected properties
            let _machine = emitter.machine();
            // Just verify it doesn't panic - TargetMachine has limited introspection
        }
    }

    #[test]
    fn test_emit_error_display_all_variants() {
        // TargetMachine error
        let target_err = TargetError::UnsupportedTarget {
            triple: "test".to_string(),
            supported: vec!["x86_64-unknown-linux-gnu"],
        };
        let err = EmitError::TargetMachine(target_err);
        let display = err.to_string();
        assert!(display.contains("failed to create target machine"));
        assert!(display.contains("test"));

        // ModuleConfiguration error
        let config_err = TargetError::InitializationFailed("test init".to_string());
        let err = EmitError::ModuleConfiguration(config_err);
        let display = err.to_string();
        assert!(display.contains("failed to configure module"));

        // AssemblyEmission error
        let err = EmitError::AssemblyEmission {
            path: "test.s".to_string(),
            message: "assembly error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit assembly file 'test.s': assembly error"
        );

        // BitcodeEmission error
        let err = EmitError::BitcodeEmission {
            path: "test.bc".to_string(),
            message: "bitcode error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit bitcode file 'test.bc': bitcode error"
        );

        // LlvmIrEmission error
        let err = EmitError::LlvmIrEmission {
            path: "test.ll".to_string(),
            message: "ir error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to emit LLVM IR file 'test.ll': ir error"
        );
    }

    #[test]
    fn test_emit_error_source() {
        // TargetMachine has a source
        let target_err = TargetError::InitializationFailed("test".to_string());
        let err = EmitError::TargetMachine(target_err);
        assert!(err.source().is_some());

        // ModuleConfiguration has a source
        let config_err = TargetError::InitializationFailed("test".to_string());
        let err = EmitError::ModuleConfiguration(config_err);
        assert!(err.source().is_some());

        // ObjectEmission has no source
        let err = EmitError::ObjectEmission {
            path: "test.o".to_string(),
            message: "error".to_string(),
        };
        assert!(err.source().is_none());

        // InvalidPath has no source
        let err = EmitError::InvalidPath {
            path: "/bad".to_string(),
            reason: "test".to_string(),
        };
        assert!(err.source().is_none());
    }

    #[test]
    fn test_emit_error_from_target_error() {
        let target_err = TargetError::UnsupportedTarget {
            triple: "bad-target".to_string(),
            supported: vec!["x86_64-unknown-linux-gnu"],
        };
        let emit_err: EmitError = target_err.into();
        assert!(matches!(emit_err, EmitError::TargetMachine(_)));
    }

    #[test]
    fn test_emit_bitcode_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_bc");
            emitter.configure_module(&module).unwrap();

            // Try to emit to a path with non-existent parent
            let bad_path = std::path::Path::new("/nonexistent_dir_12345/test.bc");
            let result = emitter.emit_bitcode(&module, bad_path);
            assert!(result.is_err());
            if let Err(EmitError::InvalidPath { path, reason }) = result {
                assert!(path.contains("nonexistent"));
                assert!(reason.contains("parent"));
            }
        }
    }

    #[test]
    fn test_emit_llvm_ir_invalid_path() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_ir");
            emitter.configure_module(&module).unwrap();

            // Try to emit to a path with non-existent parent
            let bad_path = std::path::Path::new("/nonexistent_dir_12345/test.ll");
            let result = emitter.emit_llvm_ir(&module, bad_path);
            assert!(result.is_err());
            if let Err(EmitError::InvalidPath { path, reason }) = result {
                assert!(path.contains("nonexistent"));
                assert!(reason.contains("parent"));
            }
        }
    }

    #[test]
    fn test_emit_dispatch_bitcode() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_dispatch_bc");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("dispatch_bc", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(1, false)))
                .unwrap();

            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_dispatch.bc");

            // Use the dispatch emit method with Bitcode format
            let result = emitter.emit(&module, &path, OutputFormat::Bitcode);
            assert!(result.is_ok());

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_dispatch_llvm_ir() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_dispatch_ir");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("dispatch_ir", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(2, false)))
                .unwrap();

            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join("test_dispatch.ll");

            // Use the dispatch emit method with LlvmIr format
            let result = emitter.emit(&module, &path, OutputFormat::LlvmIr);
            assert!(result.is_ok());

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_emit_assembly_to_memory() {
        if let Ok(emitter) = ObjectEmitter::native() {
            let context = Context::create();
            let module = context.create_module("test_asm_mem");
            emitter.configure_module(&module).unwrap();

            let i64_type = context.i64_type();
            let fn_type = i64_type.fn_type(&[], false);
            let function = module.add_function("asm_mem_func", fn_type, None);
            let entry = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(entry);
            builder
                .build_return(Some(&i64_type.const_int(99, false)))
                .unwrap();

            let result = emitter.emit_assembly_to_memory(&module);
            assert!(result.is_ok());
            let asm_bytes = result.unwrap();
            assert!(!asm_bytes.is_empty());
        }
    }
}
