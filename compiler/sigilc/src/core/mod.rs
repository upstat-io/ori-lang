// Core shared infrastructure for the Sigil compiler
//
// This module contains types and utilities shared across multiple compiler phases
// (type checking, evaluation, lowering, etc.) to reduce duplication and ensure consistency.

mod binding;
mod scope;

pub use binding::Binding;
pub use scope::{Scope, ScopeGuard};
