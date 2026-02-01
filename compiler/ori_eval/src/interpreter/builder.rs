//! `InterpreterBuilder` for creating Interpreter instances with various configurations.

use super::resolvers::{
    BuiltinMethodResolver, CollectionMethodResolver, MethodDispatcher, MethodResolverKind,
    UserRegistryResolver,
};
use super::Interpreter;
#[cfg(target_arch = "wasm32")]
use super::DEFAULT_MAX_CALL_DEPTH;
use crate::{
    stdout_handler, Environment, SharedMutableRegistry, SharedPrintHandler, SharedRegistry,
    UserMethodRegistry,
};
use ori_ir::{ExprArena, SharedArena, StringInterner};
use ori_patterns::PatternRegistry;

/// Builder for creating Interpreter instances with various configurations.
pub struct InterpreterBuilder<'a> {
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    env: Option<Environment>,
    registry: Option<SharedRegistry<PatternRegistry>>,
    imported_arena: Option<SharedArena>,
    user_method_registry: Option<SharedMutableRegistry<UserMethodRegistry>>,
    print_handler: Option<SharedPrintHandler>,
    owns_scoped_env: bool,
    call_depth: usize,
    #[cfg(target_arch = "wasm32")]
    max_call_depth: usize,
}

impl<'a> InterpreterBuilder<'a> {
    /// Create a new builder.
    #[cfg(target_arch = "wasm32")]
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner,
            arena,
            env: None,
            registry: None,
            imported_arena: None,
            user_method_registry: None,
            print_handler: None,
            owns_scoped_env: false,
            call_depth: 0,
            max_call_depth: DEFAULT_MAX_CALL_DEPTH,
        }
    }

    /// Create a new builder.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner,
            arena,
            env: None,
            registry: None,
            imported_arena: None,
            user_method_registry: None,
            print_handler: None,
            owns_scoped_env: false,
            call_depth: 0,
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
    /// Default is stdout for native builds.
    /// Use `buffer_handler()` for WASM or testing.
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

    /// Set the initial call depth for recursion tracking.
    ///
    /// Used when creating child interpreters for function calls to propagate
    /// the current call depth.
    #[must_use]
    pub fn call_depth(mut self, depth: usize) -> Self {
        self.call_depth = depth;
        self
    }

    /// Set the maximum call depth for recursion limiting (WASM only).
    ///
    /// Default is 200, which is conservative for browser environments.
    /// WASM runtimes outside browsers (Node.js, Wasmtime, etc.) may support
    /// higher limits depending on their stack configuration.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn max_call_depth(mut self, limit: usize) -> Self {
        self.max_call_depth = limit;
        self
    }

    /// Build the interpreter (WASM version with max_call_depth).
    #[cfg(target_arch = "wasm32")]
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

        Interpreter {
            interner: self.interner,
            arena: self.arena,
            env: self.env.unwrap_or_default(),
            self_name,
            call_depth: self.call_depth,
            max_call_depth: self.max_call_depth,
            registry: pat_reg,
            user_method_registry: user_meth_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
            print_handler: self.print_handler.unwrap_or_else(stdout_handler),
            owns_scoped_env: self.owns_scoped_env,
        }
    }

    /// Build the interpreter (native version without `max_call_depth`).
    #[cfg(not(target_arch = "wasm32"))]
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

        Interpreter {
            interner: self.interner,
            arena: self.arena,
            env: self.env.unwrap_or_default(),
            self_name,
            call_depth: self.call_depth,
            registry: pat_reg,
            user_method_registry: user_meth_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
            print_handler: self.print_handler.unwrap_or_else(stdout_handler),
            owns_scoped_env: self.owns_scoped_env,
        }
    }
}
