// Symbol table for the Sigil compiler
//
// Provides efficient storage and lookup of symbols.

use std::collections::HashMap;

use super::id::SymbolId;
use super::symbol::Symbol;

/// Symbol table with efficient lookup by name and ID.
#[derive(Default)]
pub struct SymbolTable {
    /// All symbols indexed by their ID
    symbols: Vec<Symbol>,
    /// Index from name to symbol IDs (handles shadowing/overloading)
    by_name: HashMap<String, Vec<SymbolId>>,
    /// Index from fully qualified name to symbol ID
    by_qualified: HashMap<String, SymbolId>,
}

impl SymbolTable {
    /// Create a new empty symbol table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a new symbol and return its ID.
    pub fn insert(&mut self, symbol: Symbol) -> SymbolId {
        let id = SymbolId::new(self.symbols.len() as u32);

        // Add to qualified name index
        let qualified = symbol.fully_qualified();
        self.by_qualified.insert(qualified, id);

        // Add to name index (supports shadowing)
        self.by_name
            .entry(symbol.name.clone())
            .or_default()
            .push(id);

        self.symbols.push(symbol);
        id
    }

    /// Lookup by simple name (returns most recent if shadowed).
    pub fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).and_then(|ids| ids.last().copied())
    }

    /// Lookup all symbols with a given name (for shadowing/overloading).
    pub fn lookup_all(&self, name: &str) -> Vec<SymbolId> {
        self.by_name.get(name).cloned().unwrap_or_default()
    }

    /// Lookup by fully qualified name.
    pub fn lookup_qualified(&self, path: &[String], name: &str) -> Option<SymbolId> {
        let qualified = if path.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", path.join("::"), name)
        };
        self.by_qualified.get(&qualified).copied()
    }

    /// Lookup by fully qualified string.
    pub fn lookup_qualified_str(&self, qualified: &str) -> Option<SymbolId> {
        self.by_qualified.get(qualified).copied()
    }

    /// Get symbol by ID.
    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.as_u32() as usize)
    }

    /// Get mutable symbol by ID.
    pub fn get_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(id.as_u32() as usize)
    }

    /// Check if a name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Check if a qualified name exists.
    pub fn contains_qualified(&self, path: &[String], name: &str) -> bool {
        self.lookup_qualified(path, name).is_some()
    }

    /// Get all symbols of a specific kind.
    pub fn symbols_of_kind(&self, kind_name: &str) -> Vec<SymbolId> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind.kind_name() == kind_name)
            .map(|(i, _)| SymbolId::new(i as u32))
            .collect()
    }

    /// Get all function symbols.
    pub fn functions(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind.is_function())
            .map(|(i, s)| (SymbolId::new(i as u32), s))
    }

    /// Get all type symbols.
    pub fn types(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind.is_type())
            .map(|(i, s)| (SymbolId::new(i as u32), s))
    }

    /// Get all symbols in a specific module.
    pub fn symbols_in_module(&self, module_path: &[String]) -> Vec<SymbolId> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.module_path == module_path)
            .map(|(i, _)| SymbolId::new(i as u32))
            .collect()
    }

    /// Get the number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Iterate over all symbols.
    pub fn iter(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (SymbolId::new(i as u32), s))
    }

    /// Get all symbol IDs.
    pub fn ids(&self) -> impl Iterator<Item = SymbolId> + '_ {
        (0..self.symbols.len()).map(|i| SymbolId::new(i as u32))
    }

    /// Clear all symbols.
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.by_name.clear();
        self.by_qualified.clear();
    }

    /// Reserve capacity for additional symbols.
    pub fn reserve(&mut self, additional: usize) {
        self.symbols.reserve(additional);
    }
}

impl std::fmt::Debug for SymbolTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymbolTable")
            .field("len", &self.symbols.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TypeExpr;
    use crate::symbols::id::ScopeId;
    use crate::symbols::symbol::{FunctionSymbol, LocalSymbol, SymbolKind};

    fn int_type() -> TypeExpr {
        TypeExpr::Named("int".to_string())
    }

    fn void_type() -> TypeExpr {
        TypeExpr::Named("void".to_string())
    }

    fn str_type() -> TypeExpr {
        TypeExpr::Named("str".to_string())
    }

    #[test]
    fn test_insert_and_lookup() {
        let mut table = SymbolTable::new();

        let id1 = table.insert(Symbol::new(
            "foo".to_string(),
            SymbolKind::Function(FunctionSymbol::new(vec![], vec![], vec![], void_type())),
        ));

        let id2 = table.insert(Symbol::new(
            "bar".to_string(),
            SymbolKind::Local(LocalSymbol {
                ty: int_type(),
                mutable: false,
                scope: ScopeId::ROOT,
            }),
        ));

        assert_eq!(table.lookup("foo"), Some(id1));
        assert_eq!(table.lookup("bar"), Some(id2));
        assert_eq!(table.lookup("baz"), None);
    }

    #[test]
    fn test_shadowing() {
        let mut table = SymbolTable::new();

        let id1 = table.insert(Symbol::new(
            "x".to_string(),
            SymbolKind::Local(LocalSymbol {
                ty: int_type(),
                mutable: false,
                scope: ScopeId::ROOT,
            }),
        ));

        let id2 = table.insert(Symbol::new(
            "x".to_string(),
            SymbolKind::Local(LocalSymbol {
                ty: str_type(),
                mutable: false,
                scope: ScopeId::new(1),
            }),
        ));

        // lookup returns the most recent
        assert_eq!(table.lookup("x"), Some(id2));
        // lookup_all returns all
        assert_eq!(table.lookup_all("x"), vec![id1, id2]);
    }

    #[test]
    fn test_qualified_lookup() {
        let mut table = SymbolTable::new();

        table.insert(Symbol::with_path(
            "HashMap".to_string(),
            vec!["std".to_string(), "collections".to_string()],
            SymbolKind::Module,
        ));

        table.insert(Symbol::new("HashMap".to_string(), SymbolKind::Module));

        // Qualified lookup
        let std_id =
            table.lookup_qualified(&["std".to_string(), "collections".to_string()], "HashMap");
        assert!(std_id.is_some());

        // Simple lookup returns local HashMap
        let local_id = table.lookup("HashMap");
        assert!(local_id.is_some());

        // They should be different
        assert_ne!(std_id, local_id);
    }

    #[test]
    fn test_functions_iterator() {
        let mut table = SymbolTable::new();

        table.insert(Symbol::new(
            "foo".to_string(),
            SymbolKind::Function(FunctionSymbol::new(vec![], vec![], vec![], void_type())),
        ));

        table.insert(Symbol::new(
            "x".to_string(),
            SymbolKind::Local(LocalSymbol {
                ty: int_type(),
                mutable: false,
                scope: ScopeId::ROOT,
            }),
        ));

        table.insert(Symbol::new(
            "bar".to_string(),
            SymbolKind::Function(FunctionSymbol::new(vec![], vec![], vec![], int_type())),
        ));

        let functions: Vec<_> = table.functions().collect();
        assert_eq!(functions.len(), 2);
    }

    #[test]
    fn test_len_and_empty() {
        let mut table = SymbolTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);

        table.insert(Symbol::new("x".to_string(), SymbolKind::Module));
        assert!(!table.is_empty());
        assert_eq!(table.len(), 1);
    }
}
