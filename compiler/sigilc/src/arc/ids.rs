// Newtype IDs for ARC memory management
//
// These provide type-safe identifiers for various ARC concepts.

use std::fmt;

/// Unique identifier for a type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeClassId(pub u32);

impl TypeClassId {
    pub fn new(id: u32) -> Self {
        TypeClassId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for TypeClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeClass({})", self.0)
    }
}

/// Unique identifier for a scope in the scope tracker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

impl ScopeId {
    pub const ROOT: ScopeId = ScopeId(0);

    pub fn new(id: u32) -> Self {
        ScopeId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn is_root(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scope({})", self.0)
    }
}

/// Unique identifier for a local variable allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

impl LocalId {
    pub fn new(id: u32) -> Self {
        LocalId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Local({})", self.0)
    }
}

/// Unique identifier for a type in the type graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

impl TypeId {
    pub fn new(id: u32) -> Self {
        TypeId(id)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Type({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_class_id() {
        let id = TypeClassId::new(42);
        assert_eq!(id.index(), 42);
        assert_eq!(format!("{}", id), "TypeClass(42)");
    }

    #[test]
    fn test_scope_id() {
        let id = ScopeId::new(5);
        assert_eq!(id.index(), 5);
        assert!(!id.is_root());
        assert!(ScopeId::ROOT.is_root());
    }

    #[test]
    fn test_local_id() {
        let id = LocalId::new(10);
        assert_eq!(id.index(), 10);
        assert_eq!(format!("{}", id), "Local(10)");
    }

    #[test]
    fn test_type_id() {
        let id = TypeId::new(7);
        assert_eq!(id.index(), 7);
        assert_eq!(format!("{}", id), "Type(7)");
    }
}
