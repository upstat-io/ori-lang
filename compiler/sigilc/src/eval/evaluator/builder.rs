//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use crate::ir::{StringInterner, ExprArena, SharedArena};
use sigil_patterns::PatternRegistry;
use sigil_eval::{MethodRegistry, OperatorRegistry, UnaryOperatorRegistry, UserMethodRegistry};
use crate::context::{CompilerContext, SharedRegistry, SharedMutableRegistry};
use super::{Evaluator, Environment};
use super::resolvers::{
    MethodDispatcher, UserRegistryResolver, CollectionMethodResolver, BuiltinMethodResolver,
};

/// Builder for creating Evaluator instances with various configurations.
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    env: Option<Environment>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    context: Option<&'a CompilerContext>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
}

impl<'a> EvaluatorBuilder<'a> {
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner, arena, env: None, registry: None, context: None,
            imported_arena: None, user_method_registry: None,
        }
    }

    #[must_use]
    pub fn env(mut self, env: Environment) -> Self { self.env = Some(env); self }
    #[must_use]
    pub fn registry(mut self, r: PatternRegistry) -> Self { self.registry = Some(SharedRegistry::new(r)); self }
    #[must_use]
    pub fn context(mut self, c: &'a CompilerContext) -> Self { self.context = Some(c); self }
    #[must_use]
    pub fn imported_arena(mut self, a: SharedArena) -> Self { self.imported_arena = Some(a); self }
    #[must_use]
    pub fn user_method_registry(mut self, r: SharedMutableRegistry<UserMethodRegistry>) -> Self { self.user_method_registry = Some(r); self }

    pub fn build(self) -> Evaluator<'a> {
        let (pat_reg, op_reg, meth_reg, unary_reg) = if let Some(ctx) = self.context {
            (ctx.pattern_registry.clone(), ctx.operator_registry.clone(),
             ctx.method_registry.clone(), ctx.unary_operator_registry.clone())
        } else {
            (self.registry.unwrap_or_else(|| SharedRegistry::new(PatternRegistry::new())),
             SharedRegistry::new(OperatorRegistry::new()),
             SharedRegistry::new(MethodRegistry::new()),
             SharedRegistry::new(UnaryOperatorRegistry::new()))
        };

        let user_meth_reg = self.user_method_registry
            .unwrap_or_else(|| SharedMutableRegistry::new(UserMethodRegistry::new()));

        // Build method dispatcher once. Because user_method_registry uses interior
        // mutability (RwLock), the dispatcher will see methods registered later.
        // Uses unified UserRegistryResolver for both user-defined and derived methods.
        let method_dispatcher = MethodDispatcher::new(vec![
            Box::new(UserRegistryResolver::new(user_meth_reg.clone())),
            Box::new(CollectionMethodResolver::new(self.interner)),
            Box::new(BuiltinMethodResolver::new()),
        ]);

        Evaluator {
            interner: self.interner, arena: self.arena,
            env: self.env.unwrap_or_default(),
            registry: pat_reg, operator_registry: op_reg,
            method_registry: meth_reg,
            user_method_registry: user_meth_reg,
            unary_operator_registry: unary_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
        }
    }
}
