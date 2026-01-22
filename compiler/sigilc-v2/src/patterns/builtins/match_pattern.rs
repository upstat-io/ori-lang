//! The `match` pattern - pattern matching on values.
//!
//! ```sigil
//! match(value,
//!     Some(x) -> process(x),
//!     None -> default_value
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::ParamSpec;

/// Pattern matching on values.
pub struct MatchPattern;

static MATCH_PARAMS: &[ParamSpec] = &[
    // match takes a scrutinee followed by arms
];

impl PatternDefinition for MatchPattern {
    fn keyword(&self) -> &'static str {
        "match"
    }

    fn params(&self) -> &'static [ParamSpec] {
        MATCH_PARAMS
    }

    fn description(&self) -> &'static str {
        "Pattern match on a value with multiple arms"
    }

    fn help(&self) -> &'static str {
        r#"The `match` pattern matches a value against multiple patterns.
Each arm consists of a pattern and an expression to evaluate if matched.
The first matching pattern wins."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "match(opt, Some(x) -> x * 2, None -> 0)",
            "match(result, Ok(v) -> v, Err(e) -> panic(e))",
        ]
    }
}
