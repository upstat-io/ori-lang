// LowerContext for AST to TIR transformation
//
// Provides context for the lowering phase. Unlike TypeContext (for checking)
// and Environment (for evaluation), the LowerContext tracks information
// needed to build the typed intermediate representation.

use crate::ast::TypeExpr;
use crate::context::traits::*;
use crate::ir::LocalId;
use crate::types::TypeContext;
use std::collections::HashMap;

/// Context for AST to TIR lowering.
///
/// Combines type checking context with local variable tracking for IR building.
pub struct LowerContext {
    /// Type checking context (for type lookups and function signatures).
    pub type_ctx: TypeContext,

    /// Local variable name to ID mapping for IR generation.
    pub local_scope: HashMap<String, LocalId>,

    /// Parameter name to index mapping.
    pub param_indices: HashMap<String, usize>,
}

impl LowerContext {
    /// Create a new lowering context from a type checking context.
    pub fn new(type_ctx: TypeContext) -> Self {
        LowerContext {
            type_ctx,
            local_scope: HashMap::new(),
            param_indices: HashMap::new(),
        }
    }

    /// Create from a reference to TypeContext (clones it).
    pub fn from_type_ctx(ctx: &TypeContext) -> Self {
        Self::new(ctx.child())
    }

    /// Register a local variable and return its ID.
    pub fn register_local(&mut self, name: String, id: LocalId) {
        self.local_scope.insert(name, id);
    }

    /// Look up a local variable's ID.
    pub fn lookup_local_id(&self, name: &str) -> Option<LocalId> {
        self.local_scope.get(name).copied()
    }

    /// Register a parameter index.
    pub fn register_param(&mut self, name: String, index: usize) {
        self.param_indices.insert(name, index);
    }

    /// Look up a parameter's index.
    pub fn lookup_param_index(&self, name: &str) -> Option<usize> {
        self.param_indices.get(name).copied()
    }

    /// Check if a name refers to a parameter.
    pub fn is_param(&self, name: &str) -> bool {
        self.param_indices.contains_key(name)
    }
}

// Delegate trait implementations to the inner TypeContext

impl TypeLookup for LowerContext {
    fn lookup_type(&self, name: &str) -> Option<&crate::ast::TypeDef> {
        TypeLookup::lookup_type(&self.type_ctx, name)
    }
}

impl FunctionLookup<crate::types::FunctionSig> for LowerContext {
    fn lookup_function(&self, name: &str) -> Option<&crate::types::FunctionSig> {
        FunctionLookup::lookup_function(&self.type_ctx, name)
    }
}

impl ConfigLookup<TypeExpr> for LowerContext {
    fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        ConfigLookup::lookup_config(&self.type_ctx, name)
    }
}

impl VariableScope for LowerContext {
    type Binding = TypeExpr;

    fn define_variable(&mut self, name: String, binding: TypeExpr, mutable: bool) {
        self.type_ctx.define_variable(name, binding, mutable);
    }

    fn lookup_variable(&self, name: &str) -> Option<&TypeExpr> {
        self.type_ctx.lookup_variable(name)
    }

    fn is_variable_mutable(&self, name: &str) -> Option<bool> {
        self.type_ctx.is_variable_mutable(name)
    }
}

impl ReturnTypeContext for LowerContext {
    fn current_return_type(&self) -> Option<TypeExpr> {
        self.type_ctx.current_return_type()
    }

    fn set_return_type(&mut self, ty: TypeExpr) {
        self.type_ctx.set_return_type(ty);
    }

    fn clear_return_type(&mut self) {
        self.type_ctx.clear_return_type();
    }
}

impl ReadOnlyContext for LowerContext {}
impl CheckingContext for LowerContext {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::LocalId;

    #[test]
    fn test_lower_context_creation() {
        let type_ctx = TypeContext::new();
        let lower_ctx = LowerContext::new(type_ctx);

        // Should start with empty local scope
        assert!(lower_ctx.lookup_local_id("x").is_none());
    }

    #[test]
    fn test_local_registration() {
        let type_ctx = TypeContext::new();
        let mut lower_ctx = LowerContext::new(type_ctx);

        let id = LocalId(0);
        lower_ctx.register_local("x".to_string(), id);

        assert_eq!(lower_ctx.lookup_local_id("x"), Some(id));
        assert!(lower_ctx.lookup_local_id("y").is_none());
    }

    #[test]
    fn test_param_registration() {
        let type_ctx = TypeContext::new();
        let mut lower_ctx = LowerContext::new(type_ctx);

        lower_ctx.register_param("n".to_string(), 0);
        lower_ctx.register_param("m".to_string(), 1);

        assert_eq!(lower_ctx.lookup_param_index("n"), Some(0));
        assert_eq!(lower_ctx.lookup_param_index("m"), Some(1));
        assert!(lower_ctx.is_param("n"));
        assert!(!lower_ctx.is_param("x"));
    }

    #[test]
    fn test_type_lookup_delegation() {
        let type_ctx = TypeContext::new();
        let lower_ctx = LowerContext::new(type_ctx);

        // Builtin function 'len' should be available through delegation
        assert!(
            FunctionLookup::<crate::types::FunctionSig>::lookup_function(&lower_ctx, "len")
                .is_some()
        );
    }
}
