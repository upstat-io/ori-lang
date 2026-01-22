//! The `collect` pattern - build a collection from a range.
//!
//! ```sigil
//! collect(.range: 0..10, .transform: i -> i * i)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Build a collection from a range with transformation.
pub struct CollectPattern;

static COLLECT_PARAMS: &[ParamSpec] = &[
    ParamSpec::required("range", "range to iterate over"),
    ParamSpec::optional_with(
        "transform",
        "transformation function (index) -> value",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for CollectPattern {
    fn keyword(&self) -> &'static str {
        "collect"
    }

    fn params(&self) -> &'static [ParamSpec] {
        COLLECT_PARAMS
    }

    fn description(&self) -> &'static str {
        "Build a collection from a range"
    }

    fn help(&self) -> &'static str {
        r#"The `collect` pattern builds a list by iterating over a range
and optionally transforming each value.

Type signature: collect(.range: Range<T>, .transform: T -> U) -> [U]

Without a transform, it simply collects the range into a list."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "collect(.range: 0..5)",
            "collect(.range: 1..=10, .transform: i -> i * i)",
        ]
    }
}
