//! `InterpreterBuilder` for creating Interpreter instances with various configurations.

use super::resolvers::{
    BuiltinMethodResolver, CollectionMethodResolver, MethodDispatcher, MethodResolverKind,
    UserRegistryResolver,
};
use super::Interpreter;
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
}

impl<'a> InterpreterBuilder<'a> {
    /// Create a new builder.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena) -> Self {
        Self {
            interner,
            arena,
            env: None,
            registry: None,
            imported_arena: None,
            user_method_registry: None,
            print_handler: None,
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

        Interpreter {
            interner: self.interner,
            arena: self.arena,
            env: self.env.unwrap_or_default(),
            registry: pat_reg,
            user_method_registry: user_meth_reg,
            method_dispatcher,
            imported_arena: self.imported_arena,
            prelude_loaded: false,
            print_handler: self.print_handler.unwrap_or_else(stdout_handler),
        }
    }
}
