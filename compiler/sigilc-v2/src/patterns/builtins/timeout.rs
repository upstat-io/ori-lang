//! The `timeout` pattern - operation with time limit.
//!
//! ```sigil
//! timeout(.op: slow_operation(), .after: 5s)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Execute an operation with a time limit.
pub struct TimeoutPattern;

static TIMEOUT_PARAMS: &[ParamSpec] = &[
    ParamSpec::required("op", "operation to execute"),
    ParamSpec::required_with("after", "timeout duration", TypeConstraint::Duration),
];

impl PatternDefinition for TimeoutPattern {
    fn keyword(&self) -> &'static str {
        "timeout"
    }

    fn params(&self) -> &'static [ParamSpec] {
        TIMEOUT_PARAMS
    }

    fn description(&self) -> &'static str {
        "Execute an operation with a time limit"
    }

    fn help(&self) -> &'static str {
        r#"The `timeout` pattern executes an operation with a time limit.
If the operation doesn't complete in time, an error is returned.

Type signature: timeout(.op: T, .after: Duration) -> Result<T, Error>"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "timeout(.op: fetch_data(), .after: 5s)",
            "timeout(.op: compute(), .after: 100ms)",
        ]
    }

    fn required_capability(&self) -> Option<&'static str> {
        Some("Async")
    }
}
