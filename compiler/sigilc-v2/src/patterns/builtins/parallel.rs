//! The `parallel` pattern - concurrent execution of tasks.
//!
//! ```sigil
//! parallel(
//!     .task1: fetch_user(id),
//!     .task2: fetch_posts(id)
//! )
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::ParamSpec;

/// Execute multiple tasks concurrently.
pub struct ParallelPattern;

static PARALLEL_PARAMS: &[ParamSpec] = &[
    // parallel takes named tasks dynamically
];

impl PatternDefinition for ParallelPattern {
    fn keyword(&self) -> &'static str {
        "parallel"
    }

    fn params(&self) -> &'static [ParamSpec] {
        PARALLEL_PARAMS
    }

    fn description(&self) -> &'static str {
        "Execute multiple tasks concurrently"
    }

    fn help(&self) -> &'static str {
        r#"The `parallel` pattern executes multiple tasks concurrently
and waits for all of them to complete.

Type signature: parallel(.task1: T, .task2: U, ...) -> (T, U, ...)

Results are returned as a tuple in the order the tasks were specified."#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "parallel(.user: fetch_user(id), .posts: fetch_posts(id))",
        ]
    }

    fn required_capability(&self) -> Option<&'static str> {
        Some("Async")
    }
}
