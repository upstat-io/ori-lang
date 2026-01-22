//! The `retry` pattern - retry with backoff strategy.
//!
//! ```sigil
//! retry(.op: flaky_operation(), .attempts: 3, .backoff: Exponential)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Retry an operation with configurable backoff.
pub struct RetryPattern;

static RETRY_PARAMS: &[ParamSpec] = &[
    ParamSpec::required("op", "operation to retry"),
    ParamSpec::optional_default("attempts", "maximum number of attempts", "3"),
    ParamSpec::optional("backoff", "backoff strategy (None, Linear, Exponential)"),
    ParamSpec::optional_with("delay", "initial delay between retries", TypeConstraint::Duration),
];

impl PatternDefinition for RetryPattern {
    fn keyword(&self) -> &'static str {
        "retry"
    }

    fn params(&self) -> &'static [ParamSpec] {
        RETRY_PARAMS
    }

    fn description(&self) -> &'static str {
        "Retry an operation with configurable backoff"
    }

    fn help(&self) -> &'static str {
        r#"The `retry` pattern retries a failing operation with configurable
number of attempts and backoff strategy.

Type signature: retry(.op: T, .attempts: int, .backoff: Strategy) -> T

Backoff strategies:
- None: immediate retry
- Linear: delay increases linearly
- Exponential: delay doubles each attempt"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "retry(.op: network_call(), .attempts: 3)",
            "retry(.op: flaky(), .attempts: 5, .backoff: Exponential, .delay: 100ms)",
        ]
    }
}
