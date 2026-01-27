//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use crate::db::Db;
use crate::ir::{StringInterner, ExprArena, SharedArena};
use ori_patterns::PatternRegistry;
use ori_eval::UserMethodRegistry;
use crate::context::{CompilerContext, SharedRegistry, SharedMutableRegistry};
use super::{Evaluator, Environment};
use super::resolvers::{
    MethodDispatcher, UserRegistryResolver, CollectionMethodResolver, BuiltinMethodResolver,
};

/// Builder for creating Evaluator instances with various configurations.
///
/// The database is required for proper Salsa-tracked file loading.
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    db: &'a dyn Db,
    env: Option<Environment>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    context: Option<&'a CompilerContext>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
}

impl<'a> EvaluatorBuilder<'a> {
    /// Create a new builder with required database.
    ///
    /// The database is required for proper Salsa-tracked import resolution.
    /// All file access goes through `db.load_file()`.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, db: &'a dyn Db) -> Self {
        Self {
            interner, arena, db, env: None, registry: None, context: None,
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
        let pat_reg = if let Some(ctx) = self.context {
            ctx.pattern_registry.clone()
        } else {
            self.registry.unwrap_or_else(|| SharedRegistry::new(PatternRegistry::new()))
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
            db: self.db,
            env: self.env.unwrap_or_default(),
            registry: pat_reg,
            user_method_registry: user_meth_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
        }
    }
}
