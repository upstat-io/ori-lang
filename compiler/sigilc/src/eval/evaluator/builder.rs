//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use crate::ir::{StringInterner, ExprArena, SharedArena};
use crate::patterns::PatternRegistry;
use crate::context::{CompilerContext, SharedRegistry};
use super::{Evaluator, Environment};
use super::super::operators::OperatorRegistry;
use super::super::methods::MethodRegistry;
use super::super::user_methods::UserMethodRegistry;
use super::super::unary_operators::UnaryOperatorRegistry;

/// Builder for creating Evaluator instances with various configurations.
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    env: Option<Environment>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    context: Option<&'a CompilerContext>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedRegistry<UserMethodRegistry>>,
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
    pub fn user_method_registry(mut self, r: SharedRegistry<UserMethodRegistry>) -> Self { self.user_method_registry = Some(r); self }

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
        Evaluator {
            interner: self.interner, arena: self.arena,
            env: self.env.unwrap_or_default(),
            registry: pat_reg, operator_registry: op_reg,
            method_registry: meth_reg,
            user_method_registry: self.user_method_registry.unwrap_or_else(|| SharedRegistry::new(UserMethodRegistry::new())),
            unary_operator_registry: unary_reg,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
        }
    }
}
