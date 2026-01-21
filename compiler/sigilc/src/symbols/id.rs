// Symbol identifiers for the Sigil compiler
//
// Provides strongly-typed IDs for various entities in the compiler.

/// Unique identifier for a symbol in the symbol table.
///
/// SymbolIds are stable across compilation and can be used
/// as keys in maps for efficient lookup.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SymbolId(pub(crate) u32);

impl SymbolId {
    /// Create a new symbol ID.
    pub(crate) fn new(id: u32) -> Self {
        SymbolId(id)
    }

    /// Get the raw ID value.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Invalid/sentinel symbol ID.
    pub const INVALID: SymbolId = SymbolId(u32::MAX);

    /// Check if this is a valid symbol ID.
    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

impl Default for SymbolId {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "sym#{}", self.0)
        } else {
            write!(f, "sym#INVALID")
        }
    }
}

/// Unique identifier for a scope in the scope tree.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId(pub(crate) u32);

impl ScopeId {
    /// Create a new scope ID.
    pub(crate) fn new(id: u32) -> Self {
        ScopeId(id)
    }

    /// Get the raw ID value.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// The root/global scope ID.
    pub const ROOT: ScopeId = ScopeId(0);

    /// Invalid/sentinel scope ID.
    pub const INVALID: ScopeId = ScopeId(u32::MAX);

    /// Check if this is the root scope.
    pub fn is_root(&self) -> bool {
        *self == Self::ROOT
    }

    /// Check if this is a valid scope ID.
    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

impl Default for ScopeId {
    fn default() -> Self {
        Self::ROOT
    }
}

impl std::fmt::Display for ScopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            write!(f, "scope#ROOT")
        } else if self.is_valid() {
            write!(f, "scope#{}", self.0)
        } else {
            write!(f, "scope#INVALID")
        }
    }
}

/// Unique identifier for an AST node.
///
/// NodeIds allow mapping between AST nodes and their
/// resolved symbols after name resolution.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    /// Create a new node ID.
    pub(crate) fn new(id: u32) -> Self {
        NodeId(id)
    }

    /// Get the raw ID value.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Invalid/sentinel node ID.
    pub const INVALID: NodeId = NodeId(u32::MAX);

    /// Check if this is a valid node ID.
    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "node#{}", self.0)
        } else {
            write!(f, "node#INVALID")
        }
    }
}

/// Unique identifier for a type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeId(pub(crate) u32);

impl TypeId {
    /// Create a new type ID.
    pub(crate) fn new(id: u32) -> Self {
        TypeId(id)
    }

    /// Get the raw ID value.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Invalid/sentinel type ID.
    pub const INVALID: TypeId = TypeId(u32::MAX);

    /// Check if this is a valid type ID.
    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

impl Default for TypeId {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for TypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "type#{}", self.0)
        } else {
            write!(f, "type#INVALID")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_id() {
        let id = SymbolId::new(42);
        assert_eq!(id.as_u32(), 42);
        assert!(id.is_valid());
        assert!(!SymbolId::INVALID.is_valid());
    }

    #[test]
    fn test_scope_id() {
        let root = ScopeId::ROOT;
        assert!(root.is_root());
        assert!(root.is_valid());

        let id = ScopeId::new(1);
        assert!(!id.is_root());
        assert!(id.is_valid());

        assert!(!ScopeId::INVALID.is_valid());
    }

    #[test]
    fn test_node_id() {
        let id = NodeId::new(100);
        assert_eq!(id.as_u32(), 100);
        assert!(id.is_valid());
        assert!(!NodeId::INVALID.is_valid());
    }

    #[test]
    fn test_type_id() {
        let id = TypeId::new(5);
        assert_eq!(id.as_u32(), 5);
        assert!(id.is_valid());
        assert!(!TypeId::INVALID.is_valid());
    }

    #[test]
    fn test_id_display() {
        assert_eq!(format!("{}", SymbolId::new(1)), "sym#1");
        assert_eq!(format!("{}", SymbolId::INVALID), "sym#INVALID");
        assert_eq!(format!("{}", ScopeId::ROOT), "scope#ROOT");
        assert_eq!(format!("{}", ScopeId::new(1)), "scope#1");
    }
}
