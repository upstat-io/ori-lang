//! Channel pattern implementation (stub).

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor};

/// The `channel` pattern constructs a channel pair for message passing.
///
/// Syntax: `channel<T>(buffer: n)`, `channel_in<T>(buffer: n)`,
///         `channel_out<T>(buffer: n)`, `channel_all<T>(buffer: n)`
///
/// This is a stub — channel semantics are a later roadmap item.
#[derive(Clone, Copy)]
pub struct ChannelPattern;

impl PatternDefinition for ChannelPattern {
    fn name(&self) -> &'static str {
        "channel"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["buffer"]
    }

    fn evaluate(&self, _ctx: &EvalContext, _exec: &mut dyn PatternExecutor) -> EvalResult {
        Err(crate::EvalError::new("channel patterns are not yet implemented".to_string()).into())
    }
}
