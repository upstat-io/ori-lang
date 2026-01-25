//! Type Checker Output Types
//!
//! Contains `TypedModule`, `GenericBound`, `FunctionType`, and `TypeCheckError`.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::ir::{Name, Span};
use crate::types::Type;

/// Type-checked module.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypedModule {
    /// Type of each expression (indexed by `ExprId`).
    pub expr_types: Vec<Type>,
    /// Type of each function.
    pub function_types: Vec<FunctionType>,
    /// Type checking errors.
    pub errors: Vec<TypeCheckError>,
}

impl TypedModule {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// A generic parameter with its trait bounds and associated type variable.
#[derive(Clone, Debug)]
pub struct GenericBound {
    /// The generic parameter name (e.g., `T` in `<T: Eq>`)
    pub param: Name,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`)
    pub bounds: Vec<Vec<Name>>,
    /// The type variable used for this generic in the function signature.
    /// Used to resolve the actual type at call sites for constraint checking.
    pub type_var: Type,
}

// Manual Eq/PartialEq/Hash that ignores type_var (which contains fresh vars)
impl PartialEq for GenericBound {
    fn eq(&self, other: &Self) -> bool {
        self.param == other.param && self.bounds == other.bounds
    }
}

impl Eq for GenericBound {}

impl std::hash::Hash for GenericBound {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.param.hash(state);
        self.bounds.hash(state);
    }
}

/// Function type information.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionType {
    pub name: Name,
    /// Generic parameters with their trait bounds
    pub generics: Vec<GenericBound>,
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// Type checking error with location.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckError {
    pub message: String,
    pub span: Span,
    pub code: ErrorCode,
}

impl TypeCheckError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, "type error here")
    }

    /// Check if this is a soft error that can be suppressed after hard errors.
    ///
    /// Soft errors are typically inference failures that result from
    /// earlier errors propagating through the type system.
    pub fn is_soft(&self) -> bool {
        // Cannot infer errors are often caused by earlier errors
        if self.code == ErrorCode::E2005 {
            return true;
        }
        // Errors involving the error type are soft
        if self.message.contains("<error>") {
            return true;
        }
        false
    }

    /// Check if this is a follow-on error resulting from previous errors.
    ///
    /// Follow-on errors contain types like `<error>` or phrases indicating
    /// they're a consequence of earlier type errors.
    pub fn is_follow_on(&self) -> bool {
        let msg = self.message.to_lowercase();
        msg.contains("<error>")
            || msg.contains("invalid operand")
            || msg.contains("invalid type")
    }
}
