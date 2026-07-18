//! Registry population before signature and body checking.
//!
//! Builtins, user types, traits, implementations, derives, extern burdens, and
//! constants register in dependency order against one module checker state.

mod builtin_types;
pub(crate) mod burden_compute;
mod consts;
mod derived;
mod extern_types;
mod impls;
mod traits;
mod type_resolution;
mod user_types;

pub use builtin_types::register_builtin_types;
pub use consts::register_consts;
pub use derived::register_derived_impls;
pub use extern_types::register_extern_burdens;
pub(super) use impls::register_imported_impls;
pub use impls::{register_builtin_extensions, register_impls};
pub use traits::{register_object_safety_violations, register_traits};
pub use user_types::register_user_types;

pub(super) use traits::register_imported_traits;

#[cfg(test)]
pub(super) use type_resolution::resolve_parsed_type_simple;
pub(super) use type_resolution::resolve_type_with_method_generics;

#[cfg(test)]
use derived::{build_derived_methods, register_derived_impl};
#[cfg(test)]
use traits::compute_object_safety_violations;
#[cfg(test)]
use type_resolution::{
    parsed_type_contains_self, resolve_type_with_params, resolve_type_with_self,
};

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "registration tests abort when a required fixture symbol is absent"
)]
#[expect(
    clippy::expect_used,
    reason = "registration tests abort when a required fixture symbol is absent"
)]
mod tests;
