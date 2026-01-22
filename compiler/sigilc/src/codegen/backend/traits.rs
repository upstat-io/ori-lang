// Backend traits for Sigil code generation
//
// Defines the interface that all code generation backends must implement.
// This allows for multiple backends (C, LLVM, etc.) with a unified interface.

use crate::ir::TModule;
use std::path::Path;

/// Options for code generation.
#[derive(Debug, Clone, Default)]
pub struct CodegenOptions {
    /// Optimization level (0-3)
    pub opt_level: u8,
    /// Include debug information
    pub debug_info: bool,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu")
    pub target: Option<String>,
    /// Output format preferences
    pub format: OutputFormat,
}

/// Output format for generated code.
#[derive(Debug, Clone, Default)]
pub enum OutputFormat {
    /// Human-readable source code (for C backend)
    #[default]
    Source,
    /// Object file
    Object,
    /// Executable binary
    Executable,
    /// Assembly
    Assembly,
}

/// Generated code output.
#[derive(Debug, Clone)]
pub struct GeneratedCode {
    /// The generated code content
    pub content: GeneratedContent,
    /// Metadata about the generation
    pub metadata: CodegenMetadata,
}

/// Content of generated code.
#[derive(Debug, Clone)]
pub enum GeneratedContent {
    /// Source code as a string
    Source(String),
    /// Binary content
    Binary(Vec<u8>),
}

impl GeneratedContent {
    /// Get as source string if available.
    pub fn as_source(&self) -> Option<&str> {
        match self {
            GeneratedContent::Source(s) => Some(s),
            GeneratedContent::Binary(_) => None,
        }
    }

    /// Get as binary if available.
    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            GeneratedContent::Source(_) => None,
            GeneratedContent::Binary(b) => Some(b),
        }
    }
}

/// Metadata about the code generation.
#[derive(Debug, Clone, Default)]
pub struct CodegenMetadata {
    /// Backend name that generated this code
    pub backend: String,
    /// Any warnings generated during codegen
    pub warnings: Vec<String>,
    /// Statistics about the generation
    pub stats: CodegenStats,
}

/// Statistics about code generation.
#[derive(Debug, Clone, Default)]
pub struct CodegenStats {
    /// Number of functions generated
    pub functions: usize,
    /// Number of types generated
    pub types: usize,
    /// Approximate output size in bytes
    pub output_size: usize,
}

/// Trait for code generation backends.
///
/// Backends transform TIR (Typed Intermediate Representation) into
/// target-specific code. This could be C source, LLVM IR, machine code, etc.
pub trait Backend: Send + Sync {
    /// Get the name of this backend.
    fn name(&self) -> &'static str;

    /// Get a description of this backend.
    fn description(&self) -> &'static str;

    /// Get the file extension for generated output.
    fn file_extension(&self) -> &'static str;

    /// Check if this backend supports the given output format.
    fn supports_format(&self, format: &OutputFormat) -> bool;

    /// Generate code from TIR.
    fn generate(&self, module: &TModule, options: &CodegenOptions)
        -> Result<GeneratedCode, String>;

    /// Emit generated code to a file.
    fn emit(&self, code: &GeneratedCode, path: &Path) -> std::io::Result<()> {
        use std::fs;
        use std::io::Write;

        match &code.content {
            GeneratedContent::Source(s) => {
                let mut file = fs::File::create(path)?;
                file.write_all(s.as_bytes())?;
            }
            GeneratedContent::Binary(b) => {
                fs::write(path, b)?;
            }
        }
        Ok(())
    }

    /// Get supported target triples (if applicable).
    fn supported_targets(&self) -> &[&'static str] {
        &[]
    }
}

/// Trait for backends that can compile to executables.
pub trait ExecutableBackend: Backend {
    /// Compile the generated code to an executable.
    fn compile(&self, code: &GeneratedCode, output: &Path) -> Result<(), String>;

    /// Get the compiler command that would be used.
    fn compiler_command(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_options_default() {
        let opts = CodegenOptions::default();
        assert_eq!(opts.opt_level, 0);
        assert!(!opts.debug_info);
        assert!(opts.target.is_none());
    }

    #[test]
    fn test_generated_content_source() {
        let content = GeneratedContent::Source("int main() {}".to_string());
        assert!(content.as_source().is_some());
        assert!(content.as_binary().is_none());
    }

    #[test]
    fn test_generated_content_binary() {
        let content = GeneratedContent::Binary(vec![0x7F, 0x45, 0x4C, 0x46]);
        assert!(content.as_source().is_none());
        assert!(content.as_binary().is_some());
    }
}
