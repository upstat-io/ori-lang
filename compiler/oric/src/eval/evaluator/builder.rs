//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use super::Evaluator;
use crate::context::CompilerContext;
use crate::db::Db;
use crate::ir::{ExprArena, SharedArena, StringInterner, TypeId};
use ori_eval::{
    Environment, InterpreterBuilder, PatternRegistry, SharedMutableRegistry, SharedRegistry,
    UserMethodRegistry,
};
use ori_types::SharedTypeInterner;

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
    /// Expression type table from type checking, indexed by `ExprId`.
    expr_types: Option<&'a [TypeId]>,
    /// Type interner for resolving `TypeId` to `TypeData`.
    type_interner: Option<SharedTypeInterner>,
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
            expr_types: None,
            type_interner: None,
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

    /// Set the expression type table from type checking.
    ///
    /// Enables type-aware evaluation for operators like `??` that need
    /// to distinguish between chaining (`Option<T> ?? Option<T>`) and
    /// unwrapping (`Option<T> ?? T`).
    ///
    /// Should be paired with `type_interner()` to enable type resolution.
    #[must_use]
    pub fn expr_types(mut self, types: &'a [TypeId]) -> Self {
        self.expr_types = Some(types);
        self
    }

    /// Set the type interner for resolving `TypeId` to `TypeData`.
    ///
    /// Required when `expr_types()` is set to look up actual type information.
    #[must_use]
    pub fn type_interner(mut self, interner: SharedTypeInterner) -> Self {
        self.type_interner = Some(interner);
        self
    }

    /// Build the evaluator.
    pub fn build(self) -> Evaluator<'a> {
        // Build the underlying interpreter
        let mut interpreter_builder = InterpreterBuilder::new(self.interner, self.arena);

        if let Some(env) = self.env {
            interpreter_builder = interpreter_builder.env(env);
        }

        // PatternRegistry is a stateless ZST that dispatches to static pattern definitions.
        // All instances are functionally equivalent, so we always create a fresh one.
        // The context/registry options exist for future extension with custom patterns.
        if self.context.is_some() || self.registry.is_some() {
            interpreter_builder = interpreter_builder.registry(PatternRegistry::new());
        }

        if let Some(arena) = self.imported_arena {
            interpreter_builder = interpreter_builder.imported_arena(arena);
        }

        if let Some(registry) = self.user_method_registry {
            interpreter_builder = interpreter_builder.user_method_registry(registry);
        }

        // Pass type information for type-aware evaluation (e.g., ?? operator)
        if let Some(types) = self.expr_types {
            interpreter_builder = interpreter_builder.expr_types(types);
        }
        if let Some(interner) = self.type_interner {
            interpreter_builder = interpreter_builder.type_interner(interner);
        }

        Evaluator {
            interpreter: interpreter_builder.build(),
            db: self.db,
            prelude_loaded: false,
        }
    }
}
