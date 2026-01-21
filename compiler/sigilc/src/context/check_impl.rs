// Trait implementations for TypeContext
//
// Makes TypeContext implement the context traits defined in traits.rs

use crate::ast::{TypeDef, TypeExpr};
use crate::context::traits::*;
use crate::types::FunctionSig;
use crate::types::TypeContext;

impl TypeLookup for TypeContext {
    fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        TypeContext::lookup_type(self, name)
    }
}

impl FunctionLookup<FunctionSig> for TypeContext {
    fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        TypeContext::lookup_function(self, name)
    }
}

impl ConfigLookup<TypeExpr> for TypeContext {
    fn lookup_config(&self, name: &str) -> Option<&TypeExpr> {
        TypeContext::lookup_config(self, name)
    }
}

impl VariableScope for TypeContext {
    type Binding = TypeExpr;

    fn define_variable(&mut self, name: String, binding: TypeExpr, mutable: bool) {
        self.define_local(name, binding, mutable);
    }

    fn lookup_variable(&self, name: &str) -> Option<&TypeExpr> {
        self.lookup_local(name)
    }

    fn is_variable_mutable(&self, name: &str) -> Option<bool> {
        self.is_mutable(name)
    }
}

impl ReturnTypeContext for TypeContext {
    fn current_return_type(&self) -> Option<TypeExpr> {
        TypeContext::current_return_type(self)
    }

    fn set_return_type(&mut self, ty: TypeExpr) {
        self.set_current_return_type(ty);
    }

    fn clear_return_type(&mut self) {
        TypeContext::clear_current_return_type(self);
    }
}

impl ReadOnlyContext for TypeContext {}

impl CheckingContext for TypeContext {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_context_implements_traits() {
        let mut ctx = TypeContext::new();

        // Test TypeLookup - builtin types may or may not be registered
        // Just verify the method works
        let _ = TypeLookup::lookup_type(&ctx, "int");

        // Test FunctionLookup - some builtins are registered
        assert!(FunctionLookup::<FunctionSig>::lookup_function(&ctx, "len").is_some());

        // Test VariableScope
        ctx.define_variable("x".to_string(), TypeExpr::Named("int".to_string()), false);
        assert!(ctx.lookup_variable("x").is_some());
        assert_eq!(ctx.is_variable_mutable("x"), Some(false));

        // Test ReturnTypeContext
        ctx.set_return_type(TypeExpr::Named("int".to_string()));
        assert_eq!(
            ctx.current_return_type(),
            Some(TypeExpr::Named("int".to_string()))
        );
    }

    #[test]
    fn test_mutable_variable() {
        let mut ctx = TypeContext::new();

        ctx.define_variable("y".to_string(), TypeExpr::Named("int".to_string()), true);
        assert_eq!(ctx.is_variable_mutable("y"), Some(true));
    }
}
