//! Compile-time Salsa compatibility assertions.
//!
//! Salsa query results must implement Clone + Eq + `PartialEq` + Hash + Debug.
//! These static assertions catch missing derives at compile time rather than
//! runtime. If a type stops deriving a required trait, the build fails here
//! with a clear error.

use crate::{
    ArityKind, ArityMismatchKind, ConstParamInfo, ContextKind, ErrorContext, Expected,
    ExpectedOrigin, FnWhereClause, FunctionSig, Idx, LifetimeId, Rank, SequenceKind, Tag,
    TypeCheckError, TypeCheckResult, TypeCheckWarning, TypeEntry, TypeErrorKind, TypeFlags,
    TypeProblem, TypedModule, UnifyContext, UnifyError, ValueCategory,
};

/// Compile-time assertion that `T` implements all Salsa-required traits.
///
/// Evaluates to 0 if the bounds are satisfied. Produces a compile error otherwise.
/// The type parameter is intentionally unused in the body — only the bounds matter.
#[allow(
    clippy::extra_unused_type_parameters,
    reason = "type param exists only for trait bound checking"
)]
const fn assert_salsa_compatible<T: Clone + Eq + std::hash::Hash + std::fmt::Debug>() -> usize {
    0
}

// Core type handles
const _: usize = assert_salsa_compatible::<Idx>();
const _: usize = assert_salsa_compatible::<Tag>();
const _: usize = assert_salsa_compatible::<TypeFlags>();
const _: usize = assert_salsa_compatible::<Rank>();
const _: usize = assert_salsa_compatible::<LifetimeId>();
const _: usize = assert_salsa_compatible::<ValueCategory>();

// Output types (Salsa query results)
const _: usize = assert_salsa_compatible::<TypeCheckResult>();
const _: usize = assert_salsa_compatible::<TypedModule>();
const _: usize = assert_salsa_compatible::<FunctionSig>();
const _: usize = assert_salsa_compatible::<FnWhereClause>();
const _: usize = assert_salsa_compatible::<ConstParamInfo>();
const _: usize = assert_salsa_compatible::<TypeEntry>();

// Error types (embedded in query results)
const _: usize = assert_salsa_compatible::<TypeCheckError>();
const _: usize = assert_salsa_compatible::<TypeCheckWarning>();
const _: usize = assert_salsa_compatible::<TypeErrorKind>();
const _: usize = assert_salsa_compatible::<ErrorContext>();
const _: usize = assert_salsa_compatible::<ArityMismatchKind>();
const _: usize = assert_salsa_compatible::<TypeProblem>();
const _: usize = assert_salsa_compatible::<ContextKind>();
const _: usize = assert_salsa_compatible::<Expected>();
const _: usize = assert_salsa_compatible::<ExpectedOrigin>();
const _: usize = assert_salsa_compatible::<SequenceKind>();

// Unification types (used in error reporting)
const _: usize = assert_salsa_compatible::<UnifyError>();
const _: usize = assert_salsa_compatible::<UnifyContext>();
const _: usize = assert_salsa_compatible::<ArityKind>();
