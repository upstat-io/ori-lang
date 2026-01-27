//! C Code Generation Backend for Ori
//!
//! This crate provides a C code generation backend with performance optimizations:
//!
//! 1. **Unboxed enums** - `Option<int>` and `Result<int, E>` use tagged unions, no heap
//! 2. **Small String Optimization (SSO)** - strings ≤23 bytes stored inline
//! 3. **ARC elision** - skip retain/release when ownership is provable
//!
//! # Architecture
//!
//! ```text
//! TypedModule + ExprArena
//!        ↓
//!   OwnershipAnalysis  (determine which expressions need ARC)
//!        ↓
//!     CCodegen         (generate C code with optimizations)
//!        ↓
//!    CodegenResult     (C source + any errors)
//! ```

pub mod analysis;
pub mod c;
mod context;

pub use c::CCodegen;
pub use context::CodegenContext;

/// Result of code generation.
///
/// # Salsa Compatibility
///
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CodegenResult {
    /// Generated C code (empty if errors occurred).
    pub code: String,
    /// Errors encountered during codegen.
    pub errors: Vec<CodegenError>,
    /// Whether codegen succeeded.
    pub success: bool,
}

impl CodegenResult {
    /// Create a successful result with generated code.
    pub fn success(code: String) -> Self {
        Self {
            code,
            errors: Vec::new(),
            success: true,
        }
    }

    /// Create an error result.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            code: String::new(),
            errors: vec![CodegenError {
                message: message.into(),
            }],
            success: false,
        }
    }

    /// Check if codegen failed.
    pub fn has_errors(&self) -> bool {
        !self.success || !self.errors.is_empty()
    }
}

/// A code generation error.
///
/// # Salsa Compatibility
///
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CodegenError {
    pub message: String,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CodegenError {}
