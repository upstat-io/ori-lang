//! Error codes for all compiler diagnostics.
//!
//! Each error code is a unique identifier (e.g., `E1001`) with the first digit
//! indicating the compiler phase. Used for `--explain` lookups and documentation.
//!
//! All error codes are declared in a single [`define_error_codes!`] invocation.
//! The macro generates: the `ErrorCode` enum, `ALL`, `COUNT`, `as_str()`,
//! `description()`, `Display`, and `FromStr`.

mod lifecycle;
mod registry;
mod text;

pub use registry::ErrorCode;

/// Lifecycle state for a registered diagnostic code.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ErrorCodeLifecycle {
    /// A production compiler path constructs this code.
    Emitted,
    /// No production compiler path constructs this stable code.
    Reserved { rationale: &'static str },
    /// The code has a named bug or design owner but no emitting path.
    Tracked {
        issue: &'static str,
        rationale: &'static str,
    },
    /// The code remains parseable for compatibility but should not be emitted.
    Retired { rationale: &'static str },
}

#[cfg(test)]
mod tests;
