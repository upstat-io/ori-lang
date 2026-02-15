//! Focused traits for interface segregation.
//!
//! Per design principles: "Don't force dependencies on unused methods."
//! Each trait provides one focused capability.
//!
//! The three core traits per design spec:
//! - `Spanned` - just span access
//! - `Named` - just name access
//! - `Typed` - just type access

use super::Span;

/// Trait for types that have a source location span.
///
/// Per design: "Spanned trait - just span access"
pub trait Spanned {
    /// Get the source location span.
    fn span(&self) -> Span;
}

/// Trait for types that have a name.
///
/// Per design: "Named trait - just name access"
pub trait Named {
    /// Get the name.
    fn name(&self) -> super::Name;
}

/// Trait for types that have an associated type.
///
/// Per design: "Typed trait - just type access"
///
/// This trait uses a generic to avoid circular dependencies with the
/// types module. The type checker provides the concrete type.
pub trait Typed<T> {
    /// Get the type of this item.
    fn ty(&self) -> &T;
}

impl Spanned for super::Token {
    fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests;
