//! Parser tests.
//!
//! Tests are organized into modules by category:
//! - `parser`: Core parser tests for literals, expressions, operators, and capabilities
//! - `compositional`: Compositional tests verifying all combinations of types,
//!   patterns, and expressions work correctly in all valid positions.
//! - `snapshot`: Tests for parser snapshot/speculative parsing functionality

mod compositional;
mod parser;
mod snapshot;
