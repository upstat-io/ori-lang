//! Top-level type-check result.
//!
//! Wraps a [`TypedModule`] with an `ErrorGuaranteed` token proving error
//! reporting was not forgotten when errors were emitted.

use ori_diagnostic::ErrorGuaranteed;

use crate::TypeCheckError;

use super::typed_module::TypedModule;

/// Type check result with typed module and error guarantee.
///
/// This is the top-level result returned by the type checker query.
/// It wraps `TypedModule` and provides an `ErrorGuaranteed` token
/// for cases where errors were emitted.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckResult {
    /// The typed module.
    pub typed: TypedModule,

    /// Error guarantee token.
    ///
    /// `Some` if at least one error was emitted during type checking.
    /// This provides a compile-time proof that error reporting was not forgotten.
    pub error_guarantee: Option<ErrorGuaranteed>,
}

impl TypeCheckResult {
    /// Create a successful result (no errors).
    pub fn ok(typed: TypedModule) -> Self {
        debug_assert!(typed.errors.is_empty(), "ok() called with errors present");
        Self {
            typed,
            error_guarantee: None,
        }
    }

    /// Create an error result.
    pub fn err(typed: TypedModule, guarantee: ErrorGuaranteed) -> Self {
        debug_assert!(
            !typed.errors.is_empty(),
            "err() called with no errors present"
        );
        Self {
            typed,
            error_guarantee: Some(guarantee),
        }
    }

    /// Create a result, automatically determining if errors are present.
    pub fn from_typed(typed: TypedModule) -> Self {
        if typed.has_errors() {
            // Create ErrorGuaranteed from the error count
            Self {
                error_guarantee: ErrorGuaranteed::from_error_count(typed.errors.len()),
                typed,
            }
        } else {
            Self {
                typed,
                error_guarantee: None,
            }
        }
    }

    /// Check if this result has errors.
    pub fn has_errors(&self) -> bool {
        self.error_guarantee.is_some()
    }

    /// Get the errors.
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.typed.errors
    }
}
