//! The `fold` pattern - reduce a collection to a single value.
//!
//! ```sigil
//! fold(.over: items, .init: 0, .op: (acc, x) -> acc + x)
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Reduce a collection to a single value.
pub struct FoldPattern;

static FOLD_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with("over", "collection to fold", TypeConstraint::Iterable),
    ParamSpec::required("init", "initial accumulator value"),
    ParamSpec::required_with(
        "op",
        "combining function (acc, elem) -> acc",
        TypeConstraint::FoldFunction("init", "over"),
    ),
];

impl PatternDefinition for FoldPattern {
    fn keyword(&self) -> &'static str {
        "fold"
    }

    fn params(&self) -> &'static [ParamSpec] {
        FOLD_PARAMS
    }

    fn description(&self) -> &'static str {
        "Reduce a collection to a single value"
    }

    fn help(&self) -> &'static str {
        r#"The `fold` pattern reduces a collection to a single value by
repeatedly applying a combining function to an accumulator and each element.

Type signature: fold(.over: [T], .init: U, .op: (U, T) -> U) -> U

This is also known as reduce, accumulate, or aggregate in other languages."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "fold(.over: [1, 2, 3], .init: 0, .op: (acc, x) -> acc + x)",
            "fold(.over: items, .init: \"\", .op: (s, x) -> s + x.name)",
        ]
    }
}
