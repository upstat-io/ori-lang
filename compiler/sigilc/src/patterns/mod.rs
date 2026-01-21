// Pattern plugin system for the Sigil compiler
//
// This module provides an extensible pattern system where each pattern
// (fold, map, filter, recurse, etc.) is implemented as a self-contained
// handler that can be registered with the global registry.
//
// To add a new pattern:
// 1. Create a new file in patterns/builtins/
// 2. Implement PatternDefinition for your pattern struct
// 3. Add the pattern to patterns/builtins/mod.rs
// 4. Register it in patterns/registry.rs register_builtins()

pub mod builtins;
pub mod core;
mod registry;

pub use builtins::*;
pub use core::{ParamSpec, PatternArgs, PatternDefinition, TypeConstraint};
pub use registry::{global_registry, PatternRegistry};
