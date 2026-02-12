//! `EvaluatorBuilder` for creating Evaluator instances with various configurations.

use super::Evaluator;
use crate::db::Db;
use crate::ir::{ExprArena, SharedArena, StringInterner};
use ori_eval::{
    Environment, EvalMode, InterpreterBuilder, SharedMutableRegistry, UserMethodRegistry,
};
use ori_ir::canon::SharedCanonResult;

/// Builder for creating Evaluator instances with various configurations.
///
/// The database is required for proper Salsa-tracked file loading.
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    db: &'a dyn Db,
    mode: EvalMode,
    env: Option<Environment>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
    /// Canonical IR for canonical evaluation dispatch.
    canon: Option<SharedCanonResult>,
}

impl<'a> EvaluatorBuilder<'a> {
    /// Create a new builder with required database.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, db: &'a dyn Db) -> Self {
        Self {
            interner,
            arena,
            db,
            mode: EvalMode::default(),
            env: None,
            imported_arena: None,
            user_method_registry: None,
            canon: None,
        }
    }

    /// Set the evaluation mode.
    ///
    /// Controls I/O access, recursion limits, test collection, and const-eval budget.
    #[must_use]
    pub fn mode(mut self, mode: EvalMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the initial environment.
    #[must_use]
    pub fn env(mut self, env: Environment) -> Self {
        self.env = Some(env);
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

    /// Set the canonical IR for canonical evaluation dispatch.
    ///
    /// When set, the evaluator can dispatch via `eval_can()` for root expressions
    /// that have been lowered to canonical IR.
    #[must_use]
    pub fn canon(mut self, canon: SharedCanonResult) -> Self {
        self.canon = Some(canon);
        self
    }

    /// Build the evaluator.
    pub fn build(self) -> Evaluator<'a> {
        // Build the underlying interpreter with the configured mode
        let mut interpreter_builder =
            InterpreterBuilder::new(self.interner, self.arena).mode(self.mode);

        if let Some(env) = self.env {
            interpreter_builder = interpreter_builder.env(env);
        }

        if let Some(arena) = self.imported_arena {
            interpreter_builder = interpreter_builder.imported_arena(arena);
        }

        if let Some(registry) = self.user_method_registry {
            interpreter_builder = interpreter_builder.user_method_registry(registry);
        }

        // Pass canonical IR for eval_can dispatch
        if let Some(canon) = self.canon {
            interpreter_builder = interpreter_builder.canon(canon);
        }

        Evaluator {
            interpreter: interpreter_builder.build(),
            db: self.db,
            prelude_loaded: false,
        }
    }
}
