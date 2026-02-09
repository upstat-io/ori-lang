//! `InterpreterBuilder` for creating Interpreter instances with various configurations.

use super::resolvers::{
    BuiltinMethodResolver, CollectionMethodResolver, MethodDispatcher, MethodResolverKind,
    UserRegistryResolver,
};
use super::{Interpreter, TypeNames};
use crate::diagnostics::CallStack;
use crate::eval_mode::{EvalMode, ModeState};
use crate::{
    stdout_handler, Environment, SharedMutableRegistry, SharedPrintHandler, SharedRegistry,
    UserMethodRegistry,
};
use ori_ir::{ExprArena, SharedArena, StringInterner};
use ori_patterns::PatternRegistry;
use ori_types::{Idx, PatternKey, PatternResolution};

/// Builder for creating Interpreter instances with various configurations.
///
/// Every interpreter requires an explicit `EvalMode`. The default is `Interpret`,
/// but callers should specify the mode appropriate for their context:
/// - `EvalMode::Interpret` for `ori run`
/// - `EvalMode::TestRun { .. }` for `ori test`
/// - `EvalMode::ConstEval { .. }` for compile-time evaluation
pub struct InterpreterBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    env: Option<Environment>,
    mode: EvalMode,
    registry: Option<SharedRegistry<PatternRegistry>>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
    print_handler: Option<SharedPrintHandler>,
    owns_scoped_env: bool,
    call_stack: Option<CallStack>,
    /// Expression type table from type checking.
    expr_types: Option<&'a [Idx]>,
    /// Pattern resolutions from type checking.
    pattern_resolutions: &'a [(PatternKey, PatternResolution)],
}

impl<'a> InterpreterBuilder<'a> {
    /// Create a new builder with default `Interpret` mode.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner,
            arena,
            env: None,
            mode: EvalMode::default(),
            registry: None,
            imported_arena: None,
            user_method_registry: None,
            print_handler: None,
            owns_scoped_env: false,
            call_stack: None,
            expr_types: None,
            pattern_resolutions: &[],
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

    /// Set the pattern registry.
    #[must_use]
    pub fn registry(mut self, r: PatternRegistry) -> Self {
        self.registry = Some(SharedRegistry::new(r));
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

    /// Set the print handler for the Print capability.
    ///
    /// Default is stdout for `Interpret` mode. Overrides mode-based default.
    #[must_use]
    pub fn print_handler(mut self, handler: SharedPrintHandler) -> Self {
        self.print_handler = Some(handler);
        self
    }

    /// Mark this interpreter as owning a scoped environment.
    ///
    /// When called, the interpreter will pop its environment scope when dropped.
    /// This is used for function/method call interpreters to ensure RAII panic safety.
    #[must_use]
    pub fn with_scoped_env_ownership(mut self) -> Self {
        self.owns_scoped_env = true;
        self
    }

    /// Set the call stack for recursion tracking.
    ///
    /// Used when creating child interpreters for function calls to propagate
    /// the parent's call stack (clone-per-child model).
    #[must_use]
    pub fn call_stack(mut self, stack: CallStack) -> Self {
        self.call_stack = Some(stack);
        self
    }

    /// Set the expression type table from type checking.
    ///
    /// Enables type-aware evaluation for operators like `??` that need
    /// to distinguish between chaining (`Option<T> ?? Option<T>`) and
    /// unwrapping (`Option<T> ?? T`).
    #[must_use]
    pub fn expr_types(mut self, types: &'a [Idx]) -> Self {
        self.expr_types = Some(types);
        self
    }

    /// Set the pattern resolutions from type checking.
    ///
    /// Enables correct disambiguation of `Binding("Pending")` (unit variant)
    /// vs `Binding("x")` (variable) in match patterns.
    #[must_use]
    pub fn pattern_resolutions(
        mut self,
        resolutions: &'a [(PatternKey, PatternResolution)],
    ) -> Self {
        self.pattern_resolutions = resolutions;
        self
    }

    /// Build the interpreter.
    pub fn build(self) -> Interpreter<'a> {
        let pat_reg = self
            .registry
            .unwrap_or_else(|| SharedRegistry::new(PatternRegistry::new()));

        let user_meth_reg = self
            .user_method_registry
            .unwrap_or_else(|| SharedMutableRegistry::new(UserMethodRegistry::new()));

        // Build method dispatcher once. Because user_method_registry uses interior
        // mutability (RwLock), the dispatcher will see methods registered later.
        let method_dispatcher = MethodDispatcher::new(vec![
            MethodResolverKind::UserRegistry(UserRegistryResolver::new(user_meth_reg.clone())),
            MethodResolverKind::Collection(CollectionMethodResolver::new(self.interner)),
            MethodResolverKind::Builtin(BuiltinMethodResolver::new()),
        ]);

        // Pre-compute the Name for "self" to avoid repeated interning
        let self_name = self.interner.intern("self");

        // Pre-intern all primitive type names for hot-path method dispatch
        let type_names = TypeNames::new(self.interner);

        // Default print handler depends on mode if not explicitly set
        let print_handler = self.print_handler.unwrap_or_else(stdout_handler);

        let mode_state = ModeState::new(&self.mode);

        // Default call stack uses the mode's recursion limit
        let call_stack = self
            .call_stack
            .unwrap_or_else(|| CallStack::new(self.mode.max_recursion_depth()));

        Interpreter {
            interner: self.interner,
            arena: self.arena,
            env: self.env.unwrap_or_default(),
            self_name,
            type_names,
            mode: self.mode,
            mode_state,
            call_stack,
            registry: pat_reg,
            user_method_registry: user_meth_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
            print_handler,
            owns_scoped_env: self.owns_scoped_env,
            expr_types: self.expr_types,
            pattern_resolutions: self.pattern_resolutions,
        }
    }
}
