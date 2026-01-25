//! Grammar modules for the parser.
//!
//! Each module extends the Parser with methods for parsing specific
//! syntactic constructs.

mod attr;
mod expr;
mod item;
mod ty;

pub use attr::ParsedAttrs;
