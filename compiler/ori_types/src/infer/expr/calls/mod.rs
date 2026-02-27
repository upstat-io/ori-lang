//! Function call and method call inference.

mod call_inference;
mod constraints;
mod impl_lookup;
mod method_call;
mod monomorphization;
mod traits;

pub(crate) use call_inference::{infer_call, infer_call_named};
pub(crate) use method_call::{infer_method_call, infer_method_call_named};

// Re-export for tests (accessed via `super::calls::type_satisfies_trait` etc.)
#[cfg(test)]
pub(crate) use method_call::find_infinite_source;
#[cfg(test)]
pub(crate) use traits::type_satisfies_trait;
