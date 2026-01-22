//! The `try` pattern - error handling with fallback.
//!
//! ```sigil
//! try(
//!     let result = fallible_operation()?,
//!     Ok(result)
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Error handling pattern with early return on error.
pub struct TryPattern;

static TRY_PARAMS: &[ParamSpec] = &[
    // try takes positional arguments with ? propagation
];

impl PatternDefinition for TryPattern {
    fn keyword(&self) -> &'static str {
        "try"
    }

    fn params(&self) -> &'static [ParamSpec] {
        TRY_PARAMS
    }

    fn description(&self) -> &'static str {
        "Execute with early return on error using ? operator"
    }

    fn help(&self) -> &'static str {
        r#"The `try` pattern enables error handling with the ? operator.
Expressions followed by ? will return early if they are Err.
The final expression is the success value."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "try(let x = parse(input)?, let y = validate(x)?, Ok(y))",
        ]
    }
}
