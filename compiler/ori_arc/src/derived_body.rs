//! ARC bodies for compiler-derived trait methods.
//!
//! Derived bodies enter the same ARC/AIMS pipeline as source functions. This
//! module constructs only their semantic control and data flow; ownership
//! events remain the responsibility of AIMS realization.

mod clone_body;
mod default_body;
mod eq_body;
mod validation;

pub use clone_body::{build_derived_clone_identity, DerivedCloneBodyError};
pub use default_body::{build_derived_default, DerivedDefaultBodyError};
pub use eq_body::{build_derived_eq, DerivedEqBodyError};
pub(crate) use validation::{
    impl_concrete_type_error_conversion, validate_concrete_type, ConcreteTypeError, RETURN_TYPE,
    SELF_PARAMETER,
};

#[cfg(test)]
mod tests;
