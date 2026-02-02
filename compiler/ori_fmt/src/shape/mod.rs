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
//! # Spec Reference
//!
//! - Lines 14, 19: Max width (100 chars)
//! - Lines 18: Indent size (4 spaces)
//! - Lines 93-95: Independent breaking

mod core;

pub use core::Shape;

#[cfg(test)]
mod tests;
