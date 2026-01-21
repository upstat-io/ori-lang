// Builtin functions and methods registry for Sigil
// Provides a single source of truth for all builtins

mod registry;

pub use registry::{BuiltinFunction, BuiltinMethod, BuiltinRegistry};
