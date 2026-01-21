// Generic binding type for the Sigil compiler
//
// This provides a unified abstraction for bindings with associated metadata,
// used across type checking (Binding<TypeExpr>) and evaluation (Binding<Value>).

use std::fmt::Debug;

/// A generic binding that associates a value with mutability information.
///
/// This is used to represent variable bindings in various compiler phases:
/// - Type checking: `Binding<TypeExpr>` stores type information
/// - Evaluation: `Binding<Value>` stores runtime values
///
/// # Example
/// ```ignore
/// let binding: Binding<i32> = Binding::new(42, false);
/// assert_eq!(binding.get(), &42);
/// assert!(!binding.is_mutable());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding<T> {
    value: T,
    mutable: bool,
}

impl<T> Binding<T> {
    /// Create a new binding with the given value and mutability.
    pub fn new(value: T, mutable: bool) -> Self {
        Binding { value, mutable }
    }

    /// Create a new immutable binding.
    pub fn immutable(value: T) -> Self {
        Binding::new(value, false)
    }

    /// Create a new mutable binding.
    pub fn mutable(value: T) -> Self {
        Binding::new(value, true)
    }

    /// Get a reference to the bound value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Get a mutable reference to the bound value.
    /// Note: This doesn't check mutability - use `set` for that.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Check if this binding is mutable.
    pub fn is_mutable(&self) -> bool {
        self.mutable
    }

    /// Try to update the value. Returns an error if the binding is immutable.
    pub fn set(&mut self, value: T) -> Result<(), BindingError> {
        if self.mutable {
            self.value = value;
            Ok(())
        } else {
            Err(BindingError::ImmutableBinding)
        }
    }

    /// Extract the value, consuming the binding.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Map the value to a new type.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Binding<U> {
        Binding {
            value: f(self.value),
            mutable: self.mutable,
        }
    }

    /// Map the value to a new type, preserving mutability as a reference.
    pub fn as_ref(&self) -> Binding<&T> {
        Binding {
            value: &self.value,
            mutable: self.mutable,
        }
    }
}

impl<T: Clone> Binding<T> {
    /// Clone the value out of the binding.
    pub fn cloned(&self) -> T {
        self.value.clone()
    }
}

impl<T: Default> Default for Binding<T> {
    fn default() -> Self {
        Binding::immutable(T::default())
    }
}

/// Errors that can occur when working with bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// Attempted to mutate an immutable binding.
    ImmutableBinding,
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingError::ImmutableBinding => write!(f, "cannot assign to immutable binding"),
        }
    }
}

impl std::error::Error for BindingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_new() {
        let binding = Binding::new(42, false);
        assert_eq!(binding.get(), &42);
        assert!(!binding.is_mutable());
    }

    #[test]
    fn test_binding_immutable() {
        let binding = Binding::immutable("hello");
        assert_eq!(binding.get(), &"hello");
        assert!(!binding.is_mutable());
    }

    #[test]
    fn test_binding_mutable() {
        let mut binding = Binding::mutable(42);
        assert!(binding.is_mutable());
        assert!(binding.set(100).is_ok());
        assert_eq!(binding.get(), &100);
    }

    #[test]
    fn test_binding_immutable_set_fails() {
        let mut binding = Binding::immutable(42);
        assert!(matches!(binding.set(100), Err(BindingError::ImmutableBinding)));
        assert_eq!(binding.get(), &42); // Value unchanged
    }

    #[test]
    fn test_binding_map() {
        let binding = Binding::new(42, true);
        let mapped = binding.map(|n| n.to_string());
        assert_eq!(mapped.get(), "42");
        assert!(mapped.is_mutable()); // Mutability preserved
    }

    #[test]
    fn test_binding_as_ref() {
        let binding = Binding::new(42, true);
        let ref_binding = binding.as_ref();
        assert_eq!(*ref_binding.get(), &42);
        assert!(ref_binding.is_mutable());
    }

    #[test]
    fn test_binding_into_value() {
        let binding = Binding::new(String::from("hello"), false);
        let value = binding.into_value();
        assert_eq!(value, "hello");
    }

    #[test]
    fn test_binding_cloned() {
        let binding = Binding::new(vec![1, 2, 3], false);
        let cloned = binding.cloned();
        assert_eq!(cloned, vec![1, 2, 3]);
    }
}
