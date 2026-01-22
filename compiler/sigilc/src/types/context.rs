// Type context for the Sigil type checker
// Facade that delegates to focused registries for single-responsibility

use super::builtins::register_builtins;
use super::registries::{ConfigRegistry, ExtensionRegistry, FunctionRegistry, TypeRegistry};
use super::scope::{LocalBinding, ScopeGuard, ScopeManager};
use crate::ast::{TypeDef, TypeExpr, WhereBound};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// Re-export FunctionSig from registries for backwards compatibility
pub use super::registries::FunctionSig;

/// Type checking context (facade over focused registries)
///
/// Uses Arc for registries to enable cheap cloning when creating child contexts.
/// The registries are shared during type checking and lowering phases, with
/// copy-on-write semantics for the rare case of mutation after sharing.
pub struct TypeContext {
    /// Type definitions registry (shared via Arc)
    pub(crate) types: Arc<TypeRegistry>,

    /// Function signatures registry (shared via Arc)
    pub(crate) functions: Arc<FunctionRegistry>,

    /// Config variables registry (shared via Arc)
    pub(crate) configs: Arc<ConfigRegistry>,

    /// Extension methods registry (shared via Arc)
    pub(crate) extensions: Arc<ExtensionRegistry>,

    /// Scope manager for local variables (not shared - per-context)
    pub(crate) scopes: ScopeManager,

    /// Source filename for diagnostic spans
    pub(crate) filename: String,

    /// Available capabilities in the current scope
    /// Capabilities are added by `uses` clauses in function definitions
    /// and by `with` expressions for capability injection
    pub(crate) capabilities: HashSet<String>,

    /// Imported extension methods (trait_name, method_name) pairs
    /// Only extension methods that are explicitly imported are available
    pub(crate) imported_extensions: HashSet<(String, String)>,
}

impl Default for TypeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeContext {
    pub fn new() -> Self {
        let mut functions = FunctionRegistry::new();
        // Register builtin functions using declarative module
        register_builtins(&mut functions);

        TypeContext {
            types: Arc::new(TypeRegistry::new()),
            functions: Arc::new(functions),
            configs: Arc::new(ConfigRegistry::new()),
            extensions: Arc::new(ExtensionRegistry::new()),
            scopes: ScopeManager::new(),
            filename: String::new(),
            capabilities: HashSet::new(),
            imported_extensions: HashSet::new(),
        }
    }

    /// Create a new context with a filename for diagnostics
    pub fn with_filename(filename: impl Into<String>) -> Self {
        let mut ctx = Self::new();
        ctx.filename = filename.into();
        ctx
    }

    /// Get the current filename
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Set the filename for diagnostics
    pub fn set_filename(&mut self, filename: impl Into<String>) {
        self.filename = filename.into();
    }

    /// Create an error::Span from an ast::Span range
    pub fn make_span(&self, range: std::ops::Range<usize>) -> crate::errors::Span {
        crate::errors::Span::new(&self.filename, range)
    }

    // === Type Registry Facade ===

    pub fn define_type(&mut self, name: String, def: TypeDef) {
        Arc::make_mut(&mut self.types).define(name, def);
    }

    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.lookup(name)
    }

    // === Function Registry Facade ===

    pub fn define_function(&mut self, name: String, sig: FunctionSig) {
        Arc::make_mut(&mut self.functions).define(name, sig);
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.lookup(name)
    }

    // === Config Registry Facade ===

    pub fn define_config(&mut self, name: String, ty: TypeExpr) {
        Arc::make_mut(&mut self.configs).define(name, ty);
    }

    pub fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        self.configs.lookup(name)
    }

    // === Extension Methods ===

    /// Define an extension method for a trait
    pub fn define_extension_method(
        &mut self,
        trait_name: String,
        method_name: String,
        sig: FunctionSig,
        where_clause: Vec<WhereBound>,
    ) {
        Arc::make_mut(&mut self.extensions).define(trait_name, method_name, sig, where_clause);
    }

    /// Import an extension method (makes it available for use)
    pub fn import_extension(&mut self, trait_name: String, method_name: String) {
        self.imported_extensions.insert((trait_name, method_name));
    }

    /// Check if an extension method is imported
    pub fn is_extension_imported(&self, trait_name: &str, method_name: &str) -> bool {
        self.imported_extensions
            .contains(&(trait_name.to_string(), method_name.to_string()))
    }

    /// Lookup an extension method (only if imported)
    pub fn lookup_extension_method(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<&super::registries::ExtensionMethod> {
        if self.is_extension_imported(trait_name, method_name) {
            self.extensions.lookup(trait_name, method_name)
        } else {
            None
        }
    }

    /// Lookup an extension method regardless of import status (for error messages)
    pub fn lookup_extension_method_unimported(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<&super::registries::ExtensionMethod> {
        self.extensions.lookup(trait_name, method_name)
    }

    // === Capability Management ===

    /// Add a capability to the current scope
    pub fn add_capability(&mut self, capability: String) {
        self.capabilities.insert(capability);
    }

    /// Add multiple capabilities to the current scope
    pub fn add_capabilities(&mut self, capabilities: impl IntoIterator<Item = String>) {
        self.capabilities.extend(capabilities);
    }

    /// Remove a capability from the current scope
    pub fn remove_capability(&mut self, capability: &str) {
        self.capabilities.remove(capability);
    }

    /// Check if a capability is available in the current scope
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    /// Get all available capabilities
    pub fn available_capabilities(&self) -> &HashSet<String> {
        &self.capabilities
    }

    /// Check if a function's capability requirements are satisfied
    /// Returns a list of missing capabilities
    pub fn check_capabilities(&self, required: &[String]) -> Vec<String> {
        required
            .iter()
            .filter(|cap| !self.capabilities.contains(*cap))
            .cloned()
            .collect()
    }

    // === Scope Manager Facade ===

    /// Enter a new scope and return a RAII guard
    pub fn enter_scope(&mut self) -> ScopeGuard<'_> {
        self.scopes.enter_scope()
    }

    /// Enter a function scope with a specific return type
    pub fn enter_function_scope(&mut self, return_type: TypeExpr) -> ScopeGuard<'_> {
        self.scopes.enter_function_scope(return_type)
    }

    /// Set the current return type for self() calls
    pub fn set_current_return_type(&mut self, ty: TypeExpr) {
        self.scopes.set_return_type(ty);
    }

    #[allow(dead_code)]
    pub fn clear_current_return_type(&mut self) {
        self.scopes.clear_return_type();
    }

    /// Define a local variable with type and mutability
    pub fn define_local(&mut self, name: String, ty: TypeExpr, mutable: bool) {
        self.scopes.define_local(name, ty, mutable);
    }

    /// Define a local variable (immutable by default) - for backwards compatibility
    pub fn define_local_immutable(&mut self, name: String, ty: TypeExpr) {
        self.scopes.define_local_immutable(name, ty);
    }

    /// Lookup the type of a local variable
    pub fn lookup_local(&self, name: &str) -> Option<&TypeExpr> {
        self.scopes.lookup_local(name)
    }

    /// Lookup the full binding info for a local variable
    pub fn lookup_local_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.scopes.lookup_local_binding(name)
    }

    /// Check if a local variable is mutable
    pub fn is_mutable(&self, name: &str) -> Option<bool> {
        self.scopes.is_mutable(name)
    }

    /// Get the current function's return type (for `self` calls in recurse)
    pub fn current_return_type(&self) -> Option<TypeExpr> {
        self.scopes.current_return_type()
    }

    /// Get a snapshot of current locals (for saving/restoring state)
    pub fn save_locals(&self) -> HashMap<String, LocalBinding> {
        self.scopes.save_locals()
    }

    /// Restore locals from a saved snapshot
    pub fn restore_locals(&mut self, locals: HashMap<String, LocalBinding>) {
        self.scopes.restore_locals(locals);
    }

    /// Create a child context that inherits all state (for block scopes)
    /// This is cheap because registries are shared via Arc.
    pub fn child(&self) -> Self {
        TypeContext {
            types: Arc::clone(&self.types),
            functions: Arc::clone(&self.functions),
            configs: Arc::clone(&self.configs),
            extensions: Arc::clone(&self.extensions),
            scopes: self.scopes.clone(),
            filename: self.filename.clone(),
            capabilities: self.capabilities.clone(),
            imported_extensions: self.imported_extensions.clone(),
        }
    }

    /// Create a child context with additional locals added via a closure
    pub fn child_with_locals<F>(&self, f: F) -> Self
    where
        F: FnOnce(&mut HashMap<String, LocalBinding>),
    {
        let mut child = self.child();
        f(&mut child.scopes.locals);
        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_management() {
        let mut ctx = TypeContext::new();

        // Initially no capabilities
        assert!(!ctx.has_capability("Http"));
        assert!(!ctx.has_capability("FileSystem"));

        // Add a capability
        ctx.add_capability("Http".to_string());
        assert!(ctx.has_capability("Http"));
        assert!(!ctx.has_capability("FileSystem"));

        // Add multiple capabilities
        ctx.add_capabilities(vec!["FileSystem".to_string(), "Clock".to_string()]);
        assert!(ctx.has_capability("Http"));
        assert!(ctx.has_capability("FileSystem"));
        assert!(ctx.has_capability("Clock"));

        // Remove a capability
        ctx.remove_capability("Http");
        assert!(!ctx.has_capability("Http"));
        assert!(ctx.has_capability("FileSystem"));
    }

    #[test]
    fn test_check_capabilities() {
        let mut ctx = TypeContext::new();
        ctx.add_capabilities(vec!["Http".to_string(), "Clock".to_string()]);

        // All satisfied
        let missing = ctx.check_capabilities(&["Http".to_string()]);
        assert!(missing.is_empty());

        // Some missing
        let missing = ctx.check_capabilities(&["Http".to_string(), "FileSystem".to_string()]);
        assert_eq!(missing, vec!["FileSystem".to_string()]);

        // All missing
        let missing = ctx.check_capabilities(&["FileSystem".to_string(), "Database".to_string()]);
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn test_capabilities_inherited_by_child() {
        let mut ctx = TypeContext::new();
        ctx.add_capability("Http".to_string());

        let child = ctx.child();
        assert!(child.has_capability("Http"));
    }
}
