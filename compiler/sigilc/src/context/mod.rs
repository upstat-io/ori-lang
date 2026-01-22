// Context module for Sigil compiler
//
// Provides trait abstractions for phase-specific contexts:
// - Type checking: TypeContext implements CheckingContext
// - Evaluation: Environment implements runtime context traits
// - Lowering: LowerContext for AST→TIR transformation
//
// This design allows patterns and other components to be generic over
// the specific phase context while maintaining type safety.

mod check_impl;
mod eval_impl;
mod lower;
mod traits;

pub use lower::LowerContext;
pub use traits::*;

// Re-export concrete contexts from their original locations
// This provides a unified import point while keeping implementations
// in their domain-specific modules.
pub use crate::eval::Environment;
pub use crate::types::TypeContext;
