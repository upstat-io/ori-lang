//! The `recurse` pattern - recursive computation with memoization.
//!
//! ```sigil
//! recurse(
//!     .cond: n <= 1,
//!     .base: 1,
//!     .step: n * self(n - 1),
//!     .memo: true
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Recursive computation with optional memoization.
pub struct RecursePattern;

static RECURSE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with("cond", "base case condition", TypeConstraint::Boolean),
    ParamSpec::required("base", "base case value"),
    ParamSpec::required("step", "recursive step using self()"),
    ParamSpec::flag("memo", "enable memoization"),
];

impl PatternDefinition for RecursePattern {
    fn keyword(&self) -> &'static str {
        "recurse"
    }

    fn params(&self) -> &'static [ParamSpec] {
        RECURSE_PARAMS
    }

    fn description(&self) -> &'static str {
        "Recursive computation with optional memoization"
    }

    fn help(&self) -> &'static str {
        r#"The `recurse` pattern expresses recursive computations declaratively.
The special identifier `self` refers to the recursive function itself.

Type signature: recurse(.cond: bool, .base: T, .step: T) -> T

With `.memo: true`, results are cached for repeated calls with the same arguments."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "recurse(.cond: n <= 1, .base: 1, .step: n * self(n - 1))",
            "recurse(.cond: n <= 1, .base: n, .step: self(n - 1) + self(n - 2), .memo: true)",
        ]
    }
}
