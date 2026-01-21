// Core pattern infrastructure for the Sigil compiler
//
// Provides the enhanced pattern definition system with type-safe
// argument extraction and unified behavior across compiler phases.

pub mod definition;
pub mod param;
pub mod args;

pub use definition::PatternDefinition;
pub use param::{ParamSpec, TypeConstraint};
pub use args::PatternArgs;
