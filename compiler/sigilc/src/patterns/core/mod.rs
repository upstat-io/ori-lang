// Core pattern infrastructure for the Sigil compiler
//
// Provides the enhanced pattern definition system with type-safe
// argument extraction and unified behavior across compiler phases.

pub mod args;
pub mod definition;
pub mod param;

pub use args::PatternArgs;
pub use definition::PatternDefinition;
pub use param::{ParamSpec, TypeConstraint};
