//! Multi-pass module type checking.
//!
//! Registration precedes signature collection and body checking. Signatures
//! freeze before function, test, and impl bodies so recursion resolves against
//! one stable environment. [`ModuleChecker`] coordinates inference and output.

mod accessors;
mod api;
mod bodies;
mod checker;
mod derived_call_plans;
mod exports;
mod finish;
mod finish_mono;
mod imports;
mod object_safety;
pub(crate) mod registration;
mod scope;
mod signatures;
pub(crate) mod validators;
mod well_known;

pub use api::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
};
pub use checker::ModuleChecker;

pub(crate) use object_safety::{check_parsed_type_object_safety, ObjectSafetyChecker};
pub(crate) use well_known::{is_concrete_named_type, resolve_well_known_generic, WellKnownNames};

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;
