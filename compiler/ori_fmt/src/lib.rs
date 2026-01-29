//! Ori Formatter
//!
//! Code formatter for the Ori programming language.
//!
//! # Architecture
//!
//! The formatter uses a two-pass, width-based breaking algorithm:
//!
//! 1. **Measure Pass**: Bottom-up traversal calculating inline width of each node
//! 2. **Render Pass**: Top-down rendering deciding inline vs broken based on width
//!
//! Core principle: render inline if it fits (<=100 chars), break otherwise.
//!
//! # Modules
//!
//! - [`width`]: Width calculation for AST nodes

pub mod width;

pub use width::WidthCalculator;
