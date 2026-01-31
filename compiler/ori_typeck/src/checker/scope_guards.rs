//! RAII-style scope guards for `TypeChecker` context management.
//!
//! These helpers ensure context (capabilities, impl Self type) is properly
//! restored even on early returns, preventing bugs from forgotten restores.

use ori_ir::Name;
use ori_types::Type;
use rustc_hash::FxHashSet;

use super::TypeChecker;

/// Saved capability context for restoration.
struct SavedCapabilityContext {
    /// The old capabilities to restore.
    old_caps: FxHashSet<Name>,
    /// The old provided capabilities to restore.
    old_provided: FxHashSet<Name>,
}

/// Saved impl context for restoration.
struct SavedImplContext {
    /// The previous Self type to restore.
    prev_self: Option<Type>,
}

impl TypeChecker<'_> {
    /// Execute a closure with a specific capability scope.
    ///
    /// Sets the current function's capabilities to the provided set,
    /// executes the closure, and then restores the previous capabilities.
    /// This is used when type-checking function bodies that declare capabilities.
    pub fn with_capability_scope<T, F>(&mut self, caps: FxHashSet<Name>, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        // Save current state
        let saved = SavedCapabilityContext {
            old_caps: std::mem::replace(&mut self.scope.current_function_caps, caps),
            old_provided: std::mem::take(&mut self.scope.provided_caps),
        };

        // Execute closure
        let result = f(self);

        // Restore state
        self.scope.current_function_caps = saved.old_caps;
        self.scope.provided_caps = saved.old_provided;

        result
    }

    /// Execute a closure with an empty capability scope.
    ///
    /// This is used for tests and other contexts that don't declare capabilities
    /// but still need capability tracking for `with...in` expressions.
    pub fn with_empty_capability_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.with_capability_scope(FxHashSet::default(), f)
    }

    /// Execute a closure with a specific impl Self type.
    ///
    /// Sets the current impl Self type to the provided type,
    /// executes the closure, and then restores the previous Self type.
    /// This is used when type-checking impl block methods.
    pub fn with_impl_scope<T, F>(&mut self, self_ty: Type, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        // Save current state
        let saved = SavedImplContext {
            prev_self: self.scope.current_impl_self.replace(self_ty),
        };

        // Execute closure
        let result = f(self);

        // Restore state
        self.scope.current_impl_self = saved.prev_self;

        result
    }

    /// Execute a closure with a child inference environment scope.
    ///
    /// Creates a new child environment, executes the closure, and then
    /// restores the previous environment. This is the RAII pattern for
    /// managing type environment scopes during inference.
    ///
    /// Use this when you need to introduce new bindings that should be
    /// visible only within a specific scope (e.g., match arms, for loops,
    /// lambda bodies).
    pub fn with_infer_env_scope<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let child_env = self.inference.env.child();
        let old_env = std::mem::replace(&mut self.inference.env, child_env);

        let result = f(self);

        self.inference.env = old_env;
        result
    }

    /// Execute a closure with pre-bound variables in a child scope.
    ///
    /// Creates a new child environment, binds the provided variables,
    /// executes the closure, and then restores the previous environment.
    ///
    /// This is a convenience method for the common pattern of creating
    /// a child scope and immediately binding variables (e.g., match arm
    /// bindings, for loop variables).
    pub fn with_infer_bindings<T, F>(&mut self, bindings: Vec<(Name, Type)>, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.with_infer_env_scope(|checker| {
            for (name, ty) in bindings {
                checker.inference.env.bind(name, ty);
            }
            f(checker)
        })
    }

    /// Execute a closure with a pre-created inference environment.
    ///
    /// Replaces the current environment with the provided one, executes the
    /// closure, and then restores the previous environment. Use this when you
    /// need to set up a custom environment before switching to it (e.g., when
    /// binding parameters requires access to the current checker state).
    pub fn with_custom_env_scope<T, F>(&mut self, new_env: ori_types::TypeEnv, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old_env = std::mem::replace(&mut self.inference.env, new_env);
        let result = f(self);
        self.inference.env = old_env;
        result
    }

    /// Execute a closure with a specific function type scope.
    ///
    /// Sets the current function type (for `recurse`/`self()` support),
    /// executes the closure, and then restores the previous function type.
    pub fn with_function_type_scope<T, F>(&mut self, fn_type: Type, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old_fn_type = self.scope.current_function_type.take();
        self.scope.current_function_type = Some(fn_type);

        let result = f(self);

        self.scope.current_function_type = old_fn_type;
        result
    }
}
