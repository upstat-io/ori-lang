//! The `run` pattern - sequential execution block.
//!
//! ```sigil
//! run(
//!     let x = compute(),
//!     let y = transform(x),
//!     result(y)
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::ParamSpec;

/// Sequential execution pattern.
///
/// Executes a series of statements in order, returning the value of the last one.
pub struct RunPattern;

static RUN_PARAMS: &[ParamSpec] = &[
    // run takes positional arguments (statements), not named params
];

impl PatternDefinition for RunPattern {
    fn keyword(&self) -> &'static str {
        "run"
    }

    fn params(&self) -> &'static [ParamSpec] {
        RUN_PARAMS
    }

    fn description(&self) -> &'static str {
        "Execute statements sequentially, returning the last value"
    }

    fn help(&self) -> &'static str {
        r#"The `run` pattern executes a sequence of statements in order.
Each statement can be a let binding or an expression.
The value of the last expression is returned.

This is Sigil's equivalent of a begin/do block in other languages."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "run(let x = 1, let y = 2, x + y)",
            "run(print(\"hello\"), compute(), result)",
        ]
    }
}
