// Scope tree for lexical scoping in the Sigil compiler
//
// Provides parent-linked scopes for efficient name resolution.

use std::collections::HashMap;

use super::id::{ScopeId, SymbolId};

/// The kind of scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    /// Global/module scope
    Global,
    /// Function body scope
    Function,
    /// Block scope (if, match arm, etc.)
    Block,
    /// Loop scope
    Loop,
    /// Lambda/closure scope
    Lambda,
}

/// Information about a single scope.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The scope's ID
    pub id: ScopeId,
    /// Parent scope (None for global scope)
    pub parent: Option<ScopeId>,
    /// The kind of scope
    pub kind: ScopeKind,
    /// Symbols defined in this scope (name -> SymbolId)
    pub bindings: HashMap<String, SymbolId>,
    /// Depth in the scope tree (0 = global)
    pub depth: u32,
    /// Optional label for named scopes (e.g., loop labels)
    pub label: Option<String>,
}

impl Scope {
    /// Create a new scope.
    fn new(id: ScopeId, parent: Option<ScopeId>, kind: ScopeKind, depth: u32) -> Self {
        Scope {
            id,
            parent,
            kind,
            bindings: HashMap::new(),
            depth,
            label: None,
        }
    }

    /// Create a new scope with a label.
    fn with_label(
        id: ScopeId,
        parent: Option<ScopeId>,
        kind: ScopeKind,
        depth: u32,
        label: String,
    ) -> Self {
        Scope {
            id,
            parent,
            kind,
            bindings: HashMap::new(),
            depth,
            label: Some(label),
        }
    }
}

/// A tree of scopes for lexical name resolution.
#[derive(Debug)]
pub struct ScopeTree {
    /// All scopes indexed by ID
    scopes: Vec<Scope>,
    /// The currently active scope
    current: ScopeId,
}

impl Default for ScopeTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeTree {
    /// Create a new scope tree with a global scope.
    pub fn new() -> Self {
        let root = Scope::new(ScopeId::ROOT, None, ScopeKind::Global, 0);
        ScopeTree {
            scopes: vec![root],
            current: ScopeId::ROOT,
        }
    }

    /// Get the current scope ID.
    pub fn current(&self) -> ScopeId {
        self.current
    }

    /// Get the current scope.
    pub fn current_scope(&self) -> &Scope {
        self.get(self.current).expect("current scope must exist")
    }

    /// Get a scope by ID.
    pub fn get(&self, id: ScopeId) -> Option<&Scope> {
        self.scopes.get(id.as_u32() as usize)
    }

    /// Get a mutable scope by ID.
    pub fn get_mut(&mut self, id: ScopeId) -> Option<&mut Scope> {
        self.scopes.get_mut(id.as_u32() as usize)
    }

    /// Enter a new scope.
    pub fn enter(&mut self, kind: ScopeKind) -> ScopeId {
        let parent_depth = self.current_scope().depth;
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = Scope::new(id, Some(self.current), kind, parent_depth + 1);
        self.scopes.push(scope);
        self.current = id;
        id
    }

    /// Enter a new labeled scope.
    pub fn enter_labeled(&mut self, kind: ScopeKind, label: String) -> ScopeId {
        let parent_depth = self.current_scope().depth;
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = Scope::with_label(id, Some(self.current), kind, parent_depth + 1, label);
        self.scopes.push(scope);
        self.current = id;
        id
    }

    /// Exit the current scope, returning to the parent.
    /// Returns the exited scope's ID, or None if already at root.
    pub fn exit(&mut self) -> Option<ScopeId> {
        let current_scope = self.current_scope();
        if let Some(parent) = current_scope.parent {
            let exited = self.current;
            self.current = parent;
            Some(exited)
        } else {
            None // Can't exit global scope
        }
    }

    /// Define a symbol in the current scope.
    pub fn define(&mut self, name: String, symbol_id: SymbolId) -> Option<SymbolId> {
        let scope = self.get_mut(self.current)?;
        scope.bindings.insert(name, symbol_id)
    }

    /// Lookup a symbol in the current scope only.
    pub fn lookup_local(&self, name: &str) -> Option<SymbolId> {
        self.current_scope().bindings.get(name).copied()
    }

    /// Lookup a symbol, searching from current scope up through ancestors.
    pub fn lookup(&self, name: &str) -> Option<(SymbolId, ScopeId)> {
        let mut scope_id = self.current;
        loop {
            let scope = self.get(scope_id)?;
            if let Some(&sym_id) = scope.bindings.get(name) {
                return Some((sym_id, scope_id));
            }
            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return None,
            }
        }
    }

    /// Get all visible symbols from current scope.
    pub fn visible_symbols(&self) -> Vec<(String, SymbolId, ScopeId)> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut scope_id = self.current;

        loop {
            let scope = match self.get(scope_id) {
                Some(s) => s,
                None => break,
            };

            for (name, &sym_id) in &scope.bindings {
                if !seen.contains(name) {
                    seen.insert(name.clone());
                    result.push((name.clone(), sym_id, scope_id));
                }
            }

            match scope.parent {
                Some(parent) => scope_id = parent,
                None => break,
            }
        }

        result
    }

    /// Find the nearest enclosing scope of a given kind.
    pub fn find_enclosing(&self, kind: ScopeKind) -> Option<ScopeId> {
        let mut scope_id = self.current;
        loop {
            let scope = self.get(scope_id)?;
            if scope.kind == kind {
                return Some(scope_id);
            }
            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return None,
            }
        }
    }

    /// Find a labeled scope by name.
    pub fn find_labeled(&self, label: &str) -> Option<ScopeId> {
        let mut scope_id = self.current;
        loop {
            let scope = self.get(scope_id)?;
            if scope.label.as_deref() == Some(label) {
                return Some(scope_id);
            }
            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return None,
            }
        }
    }

    /// Get the depth of the current scope.
    pub fn depth(&self) -> u32 {
        self.current_scope().depth
    }

    /// Check if we're in the global scope.
    pub fn is_global(&self) -> bool {
        self.current.is_root()
    }

    /// Check if we're inside a function.
    pub fn is_in_function(&self) -> bool {
        self.find_enclosing(ScopeKind::Function).is_some()
    }

    /// Check if we're inside a loop.
    pub fn is_in_loop(&self) -> bool {
        self.find_enclosing(ScopeKind::Loop).is_some()
    }

    /// Get the number of scopes.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Check if there are any scopes (always true, as global scope always exists).
    pub fn is_empty(&self) -> bool {
        false // Global scope always exists
    }

    /// Get the parent chain from current scope to root.
    pub fn parent_chain(&self) -> Vec<ScopeId> {
        let mut chain = Vec::new();
        let mut scope_id = self.current;
        loop {
            chain.push(scope_id);
            match self.get(scope_id).and_then(|s| s.parent) {
                Some(parent) => scope_id = parent,
                None => break,
            }
        }
        chain
    }
}

/// RAII guard for scope entry/exit.
pub struct ScopeGuard<'a> {
    tree: &'a mut ScopeTree,
    scope_id: ScopeId,
}

impl<'a> ScopeGuard<'a> {
    /// Create a new scope guard.
    pub fn new(tree: &'a mut ScopeTree, kind: ScopeKind) -> Self {
        let scope_id = tree.enter(kind);
        ScopeGuard { tree, scope_id }
    }

    /// Create a new labeled scope guard.
    pub fn new_labeled(tree: &'a mut ScopeTree, kind: ScopeKind, label: String) -> Self {
        let scope_id = tree.enter_labeled(kind, label);
        ScopeGuard { tree, scope_id }
    }

    /// Get the scope ID.
    pub fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Define a symbol in this scope.
    pub fn define(&mut self, name: String, symbol_id: SymbolId) -> Option<SymbolId> {
        self.tree.define(name, symbol_id)
    }
}

impl<'a> Drop for ScopeGuard<'a> {
    fn drop(&mut self) {
        self.tree.exit();
    }
}

impl<'a> std::ops::Deref for ScopeGuard<'a> {
    type Target = ScopeTree;

    fn deref(&self) -> &Self::Target {
        self.tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_tree_creation() {
        let tree = ScopeTree::new();
        assert_eq!(tree.current(), ScopeId::ROOT);
        assert!(tree.is_global());
        assert_eq!(tree.depth(), 0);
    }

    #[test]
    fn test_enter_exit_scope() {
        let mut tree = ScopeTree::new();

        // Enter function scope
        let func_id = tree.enter(ScopeKind::Function);
        assert!(!tree.is_global());
        assert_eq!(tree.depth(), 1);
        assert_eq!(tree.current(), func_id);

        // Enter block scope
        let block_id = tree.enter(ScopeKind::Block);
        assert_eq!(tree.depth(), 2);
        assert_eq!(tree.current(), block_id);

        // Exit block scope
        tree.exit();
        assert_eq!(tree.current(), func_id);

        // Exit function scope
        tree.exit();
        assert!(tree.is_global());
    }

    #[test]
    fn test_define_and_lookup() {
        let mut tree = ScopeTree::new();

        let sym1 = SymbolId::new(1);
        tree.define("x".to_string(), sym1);

        tree.enter(ScopeKind::Block);
        let sym2 = SymbolId::new(2);
        tree.define("y".to_string(), sym2);

        // Can see both x and y
        assert_eq!(tree.lookup("x"), Some((sym1, ScopeId::ROOT)));
        assert_eq!(tree.lookup("y").map(|(id, _)| id), Some(sym2));

        // Only y is local
        assert_eq!(tree.lookup_local("y"), Some(sym2));
        assert_eq!(tree.lookup_local("x"), None);

        tree.exit();

        // After exit, y is no longer visible
        assert_eq!(tree.lookup("x"), Some((sym1, ScopeId::ROOT)));
        assert_eq!(tree.lookup("y"), None);
    }

    #[test]
    fn test_shadowing() {
        let mut tree = ScopeTree::new();

        let sym1 = SymbolId::new(1);
        tree.define("x".to_string(), sym1);

        tree.enter(ScopeKind::Block);
        let sym2 = SymbolId::new(2);
        tree.define("x".to_string(), sym2);

        // Inner x shadows outer x
        assert_eq!(tree.lookup("x").map(|(id, _)| id), Some(sym2));

        tree.exit();

        // After exit, outer x is visible again
        assert_eq!(tree.lookup("x").map(|(id, _)| id), Some(sym1));
    }

    #[test]
    fn test_find_enclosing() {
        let mut tree = ScopeTree::new();

        tree.enter(ScopeKind::Function);
        tree.enter(ScopeKind::Loop);
        tree.enter(ScopeKind::Block);

        assert!(tree.find_enclosing(ScopeKind::Loop).is_some());
        assert!(tree.find_enclosing(ScopeKind::Function).is_some());
        assert!(!tree.is_global());
    }

    #[test]
    fn test_labeled_scope() {
        let mut tree = ScopeTree::new();

        tree.enter_labeled(ScopeKind::Loop, "outer".to_string());
        tree.enter_labeled(ScopeKind::Loop, "inner".to_string());

        assert_eq!(tree.find_labeled("inner"), Some(tree.current()));
        assert!(tree.find_labeled("outer").is_some());
        assert!(tree.find_labeled("nonexistent").is_none());
    }

    #[test]
    fn test_scope_guard() {
        let mut tree = ScopeTree::new();

        {
            let mut guard = ScopeGuard::new(&mut tree, ScopeKind::Function);
            let sym = SymbolId::new(1);
            guard.define("local".to_string(), sym);
            assert!(guard.lookup("local").is_some());
        }

        // After guard drops, we're back to global
        assert!(tree.is_global());
        assert!(tree.lookup("local").is_none());
    }

    #[test]
    fn test_visible_symbols() {
        let mut tree = ScopeTree::new();

        tree.define("global".to_string(), SymbolId::new(1));

        tree.enter(ScopeKind::Function);
        tree.define("local".to_string(), SymbolId::new(2));
        tree.define("global".to_string(), SymbolId::new(3)); // Shadow

        let visible = tree.visible_symbols();
        assert_eq!(visible.len(), 2); // "global" and "local"

        // Shadowed global should return the inner one
        let global_entry = visible.iter().find(|(name, _, _)| name == "global");
        assert_eq!(global_entry.map(|(_, id, _)| id), Some(&SymbolId::new(3)));
    }

    #[test]
    fn test_parent_chain() {
        let mut tree = ScopeTree::new();

        let s1 = tree.enter(ScopeKind::Function);
        let s2 = tree.enter(ScopeKind::Block);
        let s3 = tree.enter(ScopeKind::Block);

        let chain = tree.parent_chain();
        assert_eq!(chain, vec![s3, s2, s1, ScopeId::ROOT]);
    }
}
