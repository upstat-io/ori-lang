//! The `find` pattern - find first element matching a predicate.
//!
//! ```sigil
//! find(.over: items, .where: x -> x.id == target_id)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Find the first element matching a predicate.
pub struct FindPattern;

static FIND_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with("over", "collection to search", TypeConstraint::Iterable),
    ParamSpec::required_with(
        "where",
        "predicate function (elem) -> bool",
        TypeConstraint::FunctionArity(1),
    ),
];

impl PatternDefinition for FindPattern {
    fn keyword(&self) -> &'static str {
        "find"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FIND_PARAMS
    }

    fn description(&self) -> &'static str {
        "Find the first element matching a predicate"
    }

    fn help(&self) -> &'static str {
        r#"The `find` pattern searches a collection for the first element
that satisfies a predicate, returning an Option.

Type signature: find(.over: [T], .where: T -> bool) -> Option<T>

Returns Some(element) if found, None otherwise."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "find(.over: users, .where: u -> u.id == 42)",
            "find(.over: [1, 2, 3], .where: x -> x > 5)",
        ]
    }
}
