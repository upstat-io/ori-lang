//! Function call and method call inference.

mod call_inference;
mod closure_unify;
mod constraints;
mod impl_lookup;
mod impl_signature;
mod infinite_iterator;
mod method_call;
mod method_diagnostics;
mod method_receiver;
mod module_alias_call;
mod monomorphization;
mod traits;

pub(crate) use call_inference::{infer_call, infer_call_named};
pub(crate) use method_call::{infer_method_call, infer_method_call_named};
pub(crate) use monomorphization::register_concrete_applied_resolutions;
pub(crate) use monomorphization::{compose_builtin_burdens_for_resolved_types, compose_for_idx};

// Consumed by the operator-presence gate (`operators::dispatch`) for
// generic-impl instantiation-bound validation.
pub(crate) use impl_lookup::{match_self_type, pool_base_name};
pub(crate) use traits::type_satisfies_trait as type_satisfies_named_trait;

// Re-export for tests (accessed via `super::calls::type_satisfies_trait` etc.)
#[cfg(test)]
pub(crate) use infinite_iterator::find_infinite_source;
#[cfg(test)]
pub(crate) use method_call::suggest_iterator_fix;
#[cfg(test)]
pub(crate) use traits::type_satisfies_trait;

#[cfg(test)]
mod tests;
