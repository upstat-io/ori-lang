//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use super::Evaluator;
use crate::context::CompilerContext;
use crate::db::Db;
use crate::ir::{ExprArena, SharedArena, StringInterner};
use ori_eval::{
    Environment, InterpreterBuilder, PatternRegistry, SharedMutableRegistry, SharedRegistry,
    UserMethodRegistry,
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
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, db: &'a dyn Db) -> Self {
        Self {
            interner,
            arena,
            db,
            env: None,
            registry: None,
            context: None,
            imported_arena: None,
            user_method_registry: None,
        }
    }

    /// Set the initial environment.
    #[must_use]
    pub fn env(mut self, env: Environment) -> Self {
        self.env = Some(env);
        self
    }

    /// Set the pattern registry.
    #[must_use]
    pub fn registry(mut self, r: PatternRegistry) -> Self {
        self.registry = Some(SharedRegistry::new(r));
        self
    }

    /// Set the compiler context.
    #[must_use]
    pub fn context(mut self, c: &'a CompilerContext) -> Self {
        self.context = Some(c);
        self
    }

    /// Set the imported arena reference.
    #[must_use]
    pub fn imported_arena(mut self, a: SharedArena) -> Self {
        self.imported_arena = Some(a);
        self
    }

    /// Set the user method registry.
    #[must_use]
    pub fn user_method_registry(mut self, r: SharedMutableRegistry<UserMethodRegistry>) -> Self {
        self.user_method_registry = Some(r);
        self
    }

    /// Build the evaluator.
    pub fn build(self) -> Evaluator<'a> {
        // Build the underlying interpreter
        let mut interpreter_builder = InterpreterBuilder::new(self.interner, self.arena);

        if let Some(env) = self.env {
            interpreter_builder = interpreter_builder.env(env);
        }

        // Use pattern registry from context if provided
        if let Some(_ctx) = self.context {
            // TODO: Extract pattern registry from context
            interpreter_builder = interpreter_builder.registry(PatternRegistry::new());
        } else if let Some(_registry) = self.registry {
            // TODO: Extract inner PatternRegistry from SharedRegistry
            interpreter_builder = interpreter_builder.registry(PatternRegistry::new());
        }

        if let Some(arena) = self.imported_arena {
            interpreter_builder = interpreter_builder.imported_arena(arena);
        }

        if let Some(registry) = self.user_method_registry {
            interpreter_builder = interpreter_builder.user_method_registry(registry);
        }

        Evaluator {
            interpreter: interpreter_builder.build(),
            db: self.db,
            prelude_loaded: false,
        }
    }
}
