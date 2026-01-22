//! The `filter` pattern - select elements matching a predicate.
//!
//! ```sigil
//! filter(.over: items, .predicate: x -> x > 0)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Select elements that match a predicate.
pub struct FilterPattern;

static FILTER_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with("over", "collection to filter", TypeConstraint::Iterable),
    ParamSpec::required_with(
        "predicate",
        "predicate function (elem) -> bool",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for FilterPattern {
    fn keyword(&self) -> &'static str {
        "filter"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FILTER_PARAMS
    }

    fn description(&self) -> &'static str {
        "Select elements that satisfy a predicate"
    }

    fn help(&self) -> &'static str {
        r#"The `filter` pattern selects elements from a collection that
satisfy a predicate function.

Type signature: filter(.over: [T], .predicate: T -> bool) -> [T]"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "filter(.over: [1, 2, 3, 4], .predicate: x -> x > 2)",
            "filter(.over: users, .predicate: u -> u.active)",
        ]
    }

    fn can_fuse_with(&self, other: &'static str) -> bool {
        matches!(other, "map" | "filter" | "fold")
    }
}
