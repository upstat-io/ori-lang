//! Function, test, impl, and default-impl body checking.
//!
//! Each body runs in a child of the frozen signature environment with parameter
//! bindings and function context installed. Shared finalization exports inferred
//! types, diagnostics, monomorphization requests, and burden metadata.

mod accumulate;
mod contracts;
mod def_impls;
mod functions;
mod impls;
mod method_sig;
mod validation;

pub(super) use def_impls::check_def_impl_bodies;
pub(super) use functions::{check_function_bodies, check_test_bodies};
pub(super) use impls::{check_extension_bodies, check_impl_bodies};
pub(crate) use method_sig::{allocate_rigid_var_map, allocate_rigid_var_map_for_names};
#[cfg(test)]
use validation::validator_expr_id;
pub(super) use validation::{finalize_body_and_export, BodyOutputs};

#[cfg(test)]
mod tests;
