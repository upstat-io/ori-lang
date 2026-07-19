//! Expression lowering — the core dispatch for canonical IR to ARC IR.

mod dispatch;
mod lowerer;
mod short_circuit;
mod values;

pub(crate) use lowerer::{ArcLowerer, ForLoop, ForYieldContext, ForYieldShape, LoopContext};

#[cfg(test)]
mod tests;
