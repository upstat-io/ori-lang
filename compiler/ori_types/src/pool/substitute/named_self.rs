//! Named-binder and `Self` substitution entry points.

mod named;
mod self_type;

pub use named::substitute_named_in_pool;
pub use self_type::substitute_self_in_pool;
