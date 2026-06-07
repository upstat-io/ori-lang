//! Function call and method call inference.

mod call_inference;
mod closure_unify;
mod constraints;
mod impl_lookup;
mod infinite_iterator;
mod method_call;
mod monomorphization;
mod traits;

pub(crate) use call_inference::{infer_call, infer_call_named};
pub(crate) use method_call::{infer_method_call, infer_method_call_named};
pub(crate) use monomorphization::compose_builtin_burdens_for_resolved_types;

// Re-export for tests (accessed via `super::calls::type_satisfies_trait` etc.)
#[cfg(test)]
pub(crate) use infinite_iterator::find_infinite_source;
#[cfg(test)]
pub(crate) use method_call::suggest_iterator_fix;
#[cfg(test)]
pub(crate) use traits::type_satisfies_trait;

#[cfg(test)]
mod tests;
