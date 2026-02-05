//! Unification error types.
//!
//! Provides comprehensive error information for type mismatches,
//! infinite types, and rigid variable violations.

use crate::Idx;

/// Error from type unification.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum UnifyError {
    /// Types could not be unified.
    Mismatch {
        /// The expected type (from context).
        expected: Idx,
        /// The actual type found.
        found: Idx,
        /// Where the mismatch occurred.
        context: UnifyContext,
    },

    /// Infinite/recursive type detected (occurs check failed).
    ///
    /// Example: `a = List<a>` creates an infinite type.
    InfiniteType {
        /// The variable that would recurse.
        var_id: u32,
        /// The type that contains the variable.
        containing_type: Idx,
    },

    /// Rigid type variable cannot unify with concrete type.
    ///
    /// Rigid variables come from type annotations and must remain abstract.
    RigidMismatch {
        /// Name of the rigid variable (e.g., "T" from `fn foo<T>(...)`).
        rigid_name: ori_ir::Name,
        /// The concrete type it was asked to unify with.
        concrete: Idx,
    },

    /// Two different rigid variables cannot unify.
    RigidRigidMismatch {
        /// First rigid variable name.
        rigid1: ori_ir::Name,
        /// Second rigid variable name.
        rigid2: ori_ir::Name,
    },

    /// Arity mismatch (different number of parameters/elements).
    ArityMismatch {
        /// Expected count.
        expected: usize,
        /// Found count.
        found: usize,
        /// What kind of thing has wrong arity.
        kind: ArityKind,
    },
}

/// What kind of construct has an arity mismatch.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArityKind {
    /// Function parameter count.
    Function,
    /// Tuple element count.
    Tuple,
    /// Type argument count (for generics).
    TypeArgs,
}

/// Context where unification occurred.
///
/// Used for generating helpful error messages that point to
/// the specific part of the type that failed to unify.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum UnifyContext {
    /// Top-level unification (no specific context).
    #[default]
    TopLevel,

    /// In a function parameter.
    FunctionParam {
        /// Zero-based parameter index.
        index: usize,
    },

    /// In a function return type.
    FunctionReturn,

    /// In a list element type.
    ListElement,

    /// In an option inner type.
    OptionInner,

    /// In a set element type.
    SetElement,

    /// In a map key type.
    MapKey,

    /// In a map value type.
    MapValue,

    /// In a result Ok type.
    ResultOk,

    /// In a result Err type.
    ResultErr,

    /// In a tuple element.
    TupleElement {
        /// Zero-based element index.
        index: usize,
    },

    /// In a type argument (for Applied types).
    TypeArg {
        /// Zero-based argument index.
        index: usize,
    },

    /// In a range element type.
    RangeElement,

    /// In a channel element type.
    ChannelElement,
}

impl UnifyContext {
    /// Create a context for a function parameter.
    pub fn param(index: usize) -> Self {
        Self::FunctionParam { index }
    }

    /// Create a context for a tuple element.
    pub fn tuple_elem(index: usize) -> Self {
        Self::TupleElement { index }
    }

    /// Create a context for a type argument.
    pub fn type_arg(index: usize) -> Self {
        Self::TypeArg { index }
    }

    /// Get a human-readable description of this context.
    pub fn description(&self) -> &'static str {
        match self {
            Self::TopLevel => "types",
            Self::FunctionParam { .. } => "function parameter",
            Self::FunctionReturn => "function return type",
            Self::ListElement => "list element type",
            Self::OptionInner => "option inner type",
            Self::SetElement => "set element type",
            Self::MapKey => "map key type",
            Self::MapValue => "map value type",
            Self::ResultOk => "result ok type",
            Self::ResultErr => "result error type",
            Self::TupleElement { .. } => "tuple element",
            Self::TypeArg { .. } => "type argument",
            Self::RangeElement => "range element type",
            Self::ChannelElement => "channel element type",
        }
    }
}

impl std::fmt::Display for UnifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch { context, .. } => {
                write!(f, "type mismatch in {}", context.description())
            }
            Self::InfiniteType { var_id, .. } => {
                write!(
                    f,
                    "infinite type: variable ${var_id} occurs in its own definition"
                )
            }
            Self::RigidMismatch { .. } => {
                write!(f, "type parameter cannot be unified with concrete type")
            }
            Self::RigidRigidMismatch { .. } => {
                write!(f, "different type parameters cannot be unified")
            }
            Self::ArityMismatch {
                expected,
                found,
                kind,
            } => {
                let kind_str = match kind {
                    ArityKind::Function => "function parameters",
                    ArityKind::Tuple => "tuple elements",
                    ArityKind::TypeArgs => "type arguments",
                };
                write!(
                    f,
                    "arity mismatch: expected {expected} {kind_str}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for UnifyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_descriptions() {
        assert_eq!(UnifyContext::TopLevel.description(), "types");
        assert_eq!(UnifyContext::param(0).description(), "function parameter");
        assert_eq!(
            UnifyContext::FunctionReturn.description(),
            "function return type"
        );
        assert_eq!(UnifyContext::tuple_elem(2).description(), "tuple element");
    }

    #[test]
    fn error_display() {
        let err = UnifyError::ArityMismatch {
            expected: 2,
            found: 3,
            kind: ArityKind::Function,
        };
        assert_eq!(
            err.to_string(),
            "arity mismatch: expected 2 function parameters, found 3"
        );
    }
}
