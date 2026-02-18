//! `InterpreterBuilder` for creating Interpreter instances with various configurations.

// Arc<String> is used here for source file path and source text that are shared
// across child interpreters (clone-per-child). Heap<T> is not suitable because
// its constructor is pub(super) to the value module.
#![allow(
    clippy::disallowed_types,
    reason = "Arc<String> shared across child interpreters"
)]

use std::sync::Arc;

use super::resolvers::{
    BuiltinMethodResolver, CollectionMethodResolver, MethodDispatcher, MethodResolverKind,
    UserRegistryResolver,
};
use super::{Interpreter, OpNames, PrintNames, PropNames, ScopeOwnership, TypeNames};
use crate::diagnostics::CallStack;
use crate::eval_mode::{EvalMode, ModeState};
use crate::{
    stdout_handler, Environment, SharedMutableRegistry, SharedPrintHandler, UserMethodRegistry,
};
use ori_ir::canon::SharedCanonResult;
use ori_ir::{ExprArena, SharedArena, StringInterner};

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
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
    default_field_types: Option<SharedMutableRegistry<crate::DefaultFieldTypeRegistry>>,
    print_handler: Option<SharedPrintHandler>,
    scope_ownership: ScopeOwnership,
    call_stack: Option<CallStack>,
    /// Source file path for Traceable trace entries.
    source_file_path: Option<Arc<String>>,
    /// Source text for computing line/column from spans.
    source_text: Option<Arc<String>>,
    /// Canonical IR for canonical evaluation path.
    canon: Option<SharedCanonResult>,
}

impl<'a> InterpreterBuilder<'a> {
    /// Create a new builder with default `Interpret` mode.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner,
            arena,
            env: None,
            mode: EvalMode::default(),
            imported_arena: None,
            user_method_registry: None,
            default_field_types: None,
            print_handler: None,
            scope_ownership: ScopeOwnership::Borrowed,
            call_stack: None,
            source_file_path: None,
            source_text: None,
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

    /// Set the default field type registry for `#[derive(Default)]`.
    #[must_use]
    pub fn default_field_types(
        mut self,
        r: SharedMutableRegistry<crate::DefaultFieldTypeRegistry>,
    ) -> Self {
        self.default_field_types = Some(r);
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
        self.scope_ownership = ScopeOwnership::Owned;
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

    /// Set the source file path for Traceable trace entries.
    ///
    /// Used by `?` operator to record propagation location in error traces.
    #[must_use]
    pub fn source_file_path(mut self, path: Arc<String>) -> Self {
        self.source_file_path = Some(path);
        self
    }

    /// Set the source text for computing line/column from byte offsets.
    ///
    /// Used by `?` operator to convert span byte offsets to line:column
    /// in error trace entries.
    #[must_use]
    pub fn source_text(mut self, text: Arc<String>) -> Self {
        self.source_text = Some(text);
        self
    }

    /// Set the canonical IR for canonical evaluation dispatch.
    ///
    /// When set, function calls on `FunctionValue`s with canonical bodies
    /// will dispatch via `eval_can()` instead of legacy `eval()`.
    #[must_use]
    pub fn canon(mut self, canon: SharedCanonResult) -> Self {
        self.canon = Some(canon);
        self
    }

    /// Build the interpreter.
    pub fn build(self) -> Interpreter<'a> {
        let user_meth_reg = self
            .user_method_registry
            .unwrap_or_else(|| SharedMutableRegistry::new(UserMethodRegistry::new()));

        let default_field_types = self
            .default_field_types
            .unwrap_or_else(|| SharedMutableRegistry::new(crate::DefaultFieldTypeRegistry::new()));

        // Build method dispatcher once. Because user_method_registry uses interior
        // mutability (RwLock), the dispatcher will see methods registered later.
        let method_dispatcher = MethodDispatcher::new(vec![
            MethodResolverKind::UserRegistry(UserRegistryResolver::new(user_meth_reg.clone())),
            MethodResolverKind::Collection(CollectionMethodResolver::new(self.interner)),
            MethodResolverKind::Builtin(BuiltinMethodResolver::new(self.interner)),
        ]);

        // Pre-compute the Name for "self" to avoid repeated interning
        let self_name = self.interner.intern("self");

        // Pre-intern all primitive type names for hot-path method dispatch
        let type_names = TypeNames::new(self.interner);

        // Pre-intern print method names for PatternExecutor print dispatch
        let print_names = PrintNames::new(self.interner);

        // Pre-intern FunctionExp property names for prop dispatch
        let prop_names = PropNames::new(self.interner);

        // Pre-intern operator trait method names for user-defined operator dispatch
        let op_names = OpNames::new(self.interner);

        // Pre-intern builtin method names for hot-path dispatch (u32 == u32)
        let builtin_method_names = crate::methods::BuiltinMethodNames::new(self.interner);

        // Default print handler depends on mode if not explicitly set
        let print_handler = self.print_handler.unwrap_or_else(stdout_handler);

        let mode_state = ModeState::new(&self.mode);

        // Default call stack uses the mode's recursion limit
        let call_stack = self
            .call_stack
            .unwrap_or_else(|| CallStack::new(self.mode.max_recursion_depth()));

        // Ensure imported_arena is always set so lambda capture never
        // needs to deep-clone the arena. If not explicitly provided,
        // wrap the top-level arena reference in a SharedArena.
        let imported_arena = self
            .imported_arena
            .unwrap_or_else(|| ori_ir::SharedArena::new(self.arena.clone()));

        Interpreter {
            interner: self.interner,
            arena: self.arena,
            env: self.env.unwrap_or_default(),
            self_name,
            type_names,
            print_names,
            prop_names,
            op_names,
            mode: self.mode,
            mode_state,
            call_stack,
            user_method_registry: user_meth_reg,
            default_field_types,
            method_dispatcher,
            imported_arena,
            print_handler,
            scope_ownership: self.scope_ownership,
            builtin_method_names,
            source_file_path: self.source_file_path,
            source_text: self.source_text,
            canon: self.canon,
        }
    }
}
