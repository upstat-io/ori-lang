//! Shape Tracking (Layer 3)
//!
//! Rustfmt-style shape tracking for width-based breaking decisions.
//!
//! # Architecture
//!
//! This module implements the third layer of the 5-layer formatter architecture:
//!
//! 1. **`Shape`**: Tracks available width, current indentation, and position
//! 2. **`FormatConfig`**: Configuration for formatting (from `context` module)
//! 3. **`Shape` operations**: consume, indent, dedent, fits, `next_line`
//!
//! # Key Concept: Independent Breaking
//!
//! Nested constructs break independently based on their own width.
//! A function call that fits on one line stays inline even if it's
//! inside a larger construct that needs to break.
//!
//! Spec: Annex D §General Rules (max width 100, indent 4) and §Independent Breaking.

mod core;

pub use core::Shape;

#[cfg(test)]
mod tests;
