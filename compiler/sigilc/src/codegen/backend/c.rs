// C Backend for Sigil compiler
//
// Implements the Backend trait for C code generation.
// Wraps the existing TIR-based C code generator.

use super::traits::{
    Backend, CodegenMetadata, CodegenOptions, CodegenStats, ExecutableBackend, GeneratedCode,
    GeneratedContent, OutputFormat,
};
use crate::ir::TModule;
use std::path::Path;
use std::process::Command;

/// C code generation backend.
pub struct CBackend {
    /// C compiler to use
    compiler: String,
}

impl Default for CBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CBackend {
    /// Create a new C backend with default settings.
    pub fn new() -> Self {
        // Try to find a C compiler
        let compiler = Self::detect_compiler();
        CBackend { compiler }
    }

    /// Create a C backend with a specific compiler.
    pub fn with_compiler(compiler: impl Into<String>) -> Self {
        CBackend {
            compiler: compiler.into(),
        }
    }

    /// Detect available C compiler.
    fn detect_compiler() -> String {
        // Check for common compilers in order of preference
        for compiler in &["cc", "gcc", "clang"] {
            if Command::new(compiler).arg("--version").output().is_ok() {
                return compiler.to_string();
            }
        }
        "cc".to_string() // Default fallback
    }

    /// Get the compiler being used.
    pub fn compiler(&self) -> &str {
        &self.compiler
    }
}

impl Backend for CBackend {
    fn name(&self) -> &'static str {
        "c"
    }

    fn description(&self) -> &'static str {
        "Generate C source code"
    }

    fn file_extension(&self) -> &'static str {
        "c"
    }

    fn supports_format(&self, format: &OutputFormat) -> bool {
        matches!(
            format,
            OutputFormat::Source | OutputFormat::Executable | OutputFormat::Object
        )
    }

    fn generate(
        &self,
        module: &TModule,
        _options: &CodegenOptions,
    ) -> Result<GeneratedCode, String> {
        // Use the existing TIR code generator
        let source = crate::codegen::tir::generate(module)?;

        // Count functions and types for stats
        let function_count = module.functions.len();
        let type_count = module.types.len();

        Ok(GeneratedCode {
            content: GeneratedContent::Source(source.clone()),
            metadata: CodegenMetadata {
                backend: self.name().to_string(),
                warnings: Vec::new(),
                stats: CodegenStats {
                    functions: function_count,
                    types: type_count,
                    output_size: source.len(),
                },
            },
        })
    }

    fn supported_targets(&self) -> &[&'static str] {
        // C is portable, but we list common targets
        &[
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "aarch64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        ]
    }
}

impl ExecutableBackend for CBackend {
    fn compile(&self, code: &GeneratedCode, output: &Path) -> Result<(), String> {
        let source = code
            .content
            .as_source()
            .ok_or("C backend requires source code")?;

        // Write source to temp file
        let temp_dir = std::env::temp_dir();
        let temp_source = temp_dir.join("sigil_temp.c");

        std::fs::write(&temp_source, source)
            .map_err(|e| format!("Failed to write temp source: {}", e))?;

        // Compile with the C compiler
        let status = Command::new(&self.compiler)
            .arg("-o")
            .arg(output)
            .arg(&temp_source)
            .arg("-lm") // Link math library
            .status()
            .map_err(|e| format!("Failed to run compiler: {}", e))?;

        // Clean up temp file
        let _ = std::fs::remove_file(temp_source);

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Compilation failed with exit code: {:?}",
                status.code()
            ))
        }
    }

    fn compiler_command(&self) -> &str {
        &self.compiler
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_backend_name() {
        let backend = CBackend::new();
        assert_eq!(backend.name(), "c");
    }

    #[test]
    fn test_c_backend_extension() {
        let backend = CBackend::new();
        assert_eq!(backend.file_extension(), "c");
    }

    #[test]
    fn test_c_backend_supports_source() {
        let backend = CBackend::new();
        assert!(backend.supports_format(&OutputFormat::Source));
        assert!(backend.supports_format(&OutputFormat::Executable));
        assert!(!backend.supports_format(&OutputFormat::Assembly));
    }

    #[test]
    fn test_c_backend_with_compiler() {
        let backend = CBackend::with_compiler("clang");
        assert_eq!(backend.compiler(), "clang");
    }

    #[test]
    fn test_c_backend_generate_empty_module() {
        let backend = CBackend::new();
        let module = TModule::new("test".to_string());
        let options = CodegenOptions::default();

        let result = backend.generate(&module, &options);
        assert!(result.is_ok());

        let code = result.unwrap();
        assert!(code.content.as_source().is_some());
        assert_eq!(code.metadata.backend, "c");
    }
}
