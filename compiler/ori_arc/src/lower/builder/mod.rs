//! ARC IR function builder.

mod emission;
mod literal_queries;
mod state;
mod terminators;

pub(crate) use state::{ArcIrBuilder, InvokeTargets};

#[cfg(test)]
mod tests;
