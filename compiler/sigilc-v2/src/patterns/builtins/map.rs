//! The `map` pattern - transform each element of a collection.
//!
//! ```sigil
//! map(.over: items, .transform: x -> x * 2)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Transform each element of a collection.
pub struct MapPattern;

static MAP_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with("over", "collection to map over", TypeConstraint::Iterable),
    ParamSpec::required_with(
        "transform",
        "transformation function (elem) -> result",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for MapPattern {
    fn keyword(&self) -> &'static str {
        "map"
    }

    fn params(&self) -> &'static [ParamSpec] {
        MAP_PARAMS
    }

    fn description(&self) -> &'static str {
        "Transform each element of a collection"
    }

    fn help(&self) -> &'static str {
        r#"The `map` pattern applies a transformation function to each element
of a collection, producing a new collection of the same length.

Type signature: map(.over: [T], .transform: T -> U) -> [U]"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "map(.over: [1, 2, 3], .transform: x -> x * 2)",
            "map(.over: users, .transform: u -> u.name)",
        ]
    }

    fn can_fuse_with(&self, other: &'static str) -> bool {
        matches!(other, "filter" | "map")
    }
}
