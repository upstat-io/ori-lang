//! Type environment for name resolution and scoping.

use sigil_ir::Name;
use std::collections::HashMap;

use crate::{Type, TypeScheme, TypeVar, InferenceContext};

/// Type environment for name resolution and scoping.
///
/// Supports both monomorphic types and polymorphic type schemes.
#[derive(Clone, Debug, Default)]
pub struct TypeEnv {
    /// Variable bindings: name -> type scheme
    bindings: HashMap<Name, TypeScheme>,
    /// Parent scope (for nested scopes)
    parent: Option<Box<TypeEnv>>,
}

impl TypeEnv {
    /// Create a new empty environment.
    pub fn new() -> Self {
        TypeEnv::default()
    }

    /// Create a child scope.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Bind a name to a monomorphic type in the current scope.
    pub fn bind(&mut self, name: Name, ty: Type) {
        self.bindings.insert(name, TypeScheme::mono(ty));
    }

    /// Bind a name to a polymorphic type scheme in the current scope.
    pub fn bind_scheme(&mut self, name: Name, scheme: TypeScheme) {
        self.bindings.insert(name, scheme);
    }

    /// Look up a name, searching parent scopes.
    /// Returns the type scheme (use instantiate to get a concrete type).
    pub fn lookup_scheme(&self, name: Name) -> Option<&TypeScheme> {
        self.bindings.get(&name).or_else(|| {
            self.parent.as_ref().and_then(|p| p.lookup_scheme(name))
        })
    }

    /// Look up a name and return just the type (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    pub fn lookup(&self, name: Name) -> Option<&Type> {
        self.lookup_scheme(name).map(|s| &s.ty)
    }

    /// Check if a name is bound in the current scope only.
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.bindings.contains_key(&name)
    }

    /// Collect all free type variables in the environment.
    ///
    /// This is used during generalization to avoid quantifying over
    /// variables that are free in the environment.
    pub fn free_vars(&self, ctx: &InferenceContext) -> Vec<TypeVar> {
        let mut vars = Vec::new();
        self.collect_env_free_vars(ctx, &mut vars);
        vars
    }

    fn collect_env_free_vars(&self, ctx: &InferenceContext, vars: &mut Vec<TypeVar>) {
        for scheme in self.bindings.values() {
            // Only collect free vars that are NOT quantified in the scheme
            let scheme_free = ctx.free_vars(&scheme.ty);
            for v in scheme_free {
                if !scheme.vars.contains(&v) && !vars.contains(&v) {
                    vars.push(v);
                }
            }
        }
        if let Some(parent) = &self.parent {
            parent.collect_env_free_vars(ctx, vars);
        }
    }
}
