//! The `validate` pattern - validation with error accumulation.
//!
//! ```sigil
//! validate(
//!     .value: user,
//!     .rules: [
//!         (u -> u.age >= 18, "Must be 18 or older"),
//!         (u -> len(u.name) > 0, "Name cannot be empty")
//!     ]
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Validation with error accumulation.
pub struct ValidatePattern;

static VALIDATE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required("value", "value to validate"),
    ParamSpec::required_with(
        "rules",
        "list of (predicate, error_message) pairs",
        TypeConstraint::List,
    ),
];

impl PatternDefinition for ValidatePattern {
    fn keyword(&self) -> &'static str {
        "validate"
    }

    fn params(&self) -> &'static [ParamSpec] {
        VALIDATE_PARAMS
    }

    fn description(&self) -> &'static str {
        "Validate a value against multiple rules"
    }

    fn help(&self) -> &'static str {
        r#"The `validate` pattern checks a value against multiple validation rules
and accumulates all errors rather than failing on the first one.

Type signature: validate(.value: T, .rules: [(T -> bool, str)]) -> Result<T, Error>

Each rule is a pair of (predicate, error_message).
If all rules pass, Ok(value) is returned.
If any rules fail, Err with all error messages is returned."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            r#"validate(.value: form, .rules: [
    (f -> len(f.email) > 0, "Email required"),
    (f -> f.age >= 0, "Age must be positive")
])"#,
        ]
    }
}
