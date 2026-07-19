//! Derived-trait metadata for type checking and execution.

mod registry;
pub(crate) mod strategy;

pub use registry::{DerivedImplId, DerivedMethodInfo, DerivedMethodShape, DerivedTrait};

#[cfg(test)]
mod tests;
