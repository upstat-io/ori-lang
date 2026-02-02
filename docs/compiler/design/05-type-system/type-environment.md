---
title: "Type Environment"
description: "Ori Compiler Design — Type Environment"
order: 501
section: "Type System"
---

# Type Environment

The type environment tracks variable bindings during type checking. It uses an `Rc`-based parent chain for efficient scope management.

## Location

```
compiler/ori_types/src/env.rs
```

## Structure

The environment uses `Rc<TypeEnvInner>` for O(1) parent chain cloning:

```rust
/// Internal storage wrapped in Rc for cheap cloning.
struct TypeEnvInner {
    /// Variable bindings: name -> type scheme (TypeSchemeId for efficiency)
    bindings: FxHashMap<Name, TypeSchemeId>,
    /// Parent scope (optional for nested scopes)
    parent: Option<TypeEnv>,
    /// Type interner for TypeId conversion
    interner: SharedTypeInterner,
}

/// Type environment for name resolution and scoping.
pub struct TypeEnv(Rc<TypeEnvInner>);
```

**Key design decisions:**
- `Rc<TypeEnvInner>` enables O(1) child scope creation (no recursive cloning)
- `FxHashMap` provides faster hashing for `Name` keys
- `TypeSchemeId` stores polymorphic types efficiently (intern once, compare by ID)
- Parent chain enables lexical scope lookup

## Operations

### Creating Environment

```rust
impl TypeEnv {
    /// Create with new type interner.
    pub fn new() -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
            interner: SharedTypeInterner::new(),
        }))
    }

    /// Create with shared interner (for multi-phase compilation).
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
            interner,
        }))
    }
}
```

### Scope Management

Child scopes are created via `child()`, not push/pop:

```rust
impl TypeEnv {
    /// Create a child scope - O(1) due to Rc parent sharing.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: Some(self.clone()), // Cheap Rc clone
            interner: self.0.interner.clone(),
        }))
    }
}
```

Note: Unlike the evaluator's stack-based `Environment`, `TypeEnv` creates new scope instances. The caller is responsible for using the child scope where appropriate.

### Variable Binding

```rust
impl TypeEnv {
    /// Bind a name to a monomorphic type.
    pub fn bind(&mut self, name: Name, ty: Type) {
        let inner = Rc::make_mut(&mut self.0);
        let ty_id = ty.to_type_id(&inner.interner);
        inner.bindings.insert(name, TypeSchemeId::mono(ty_id));
    }

    /// Bind a name to a TypeId directly (avoids Type conversion).
    pub fn bind_id(&mut self, name: Name, ty: TypeId) {
        let inner = Rc::make_mut(&mut self.0);
        inner.bindings.insert(name, TypeSchemeId::mono(ty));
    }

    /// Bind a polymorphic type scheme.
    pub fn bind_scheme(&mut self, name: Name, scheme: TypeScheme) {
        let inner = Rc::make_mut(&mut self.0);
        let scheme_id = scheme.to_scheme_id(&inner.interner);
        inner.bindings.insert(name, scheme_id);
    }

    /// Look up a name, searching parent scopes.
    pub fn lookup(&self, name: Name) -> Option<Type> {
        self.lookup_id(name).map(|id| self.0.interner.to_type(id))
    }

    /// Look up returning TypeId (avoids Type allocation).
    pub fn lookup_id(&self, name: Name) -> Option<TypeId> {
        self.lookup_scheme_id(name).map(|s| s.ty)
    }

    /// Look up returning TypeSchemeId.
    pub fn lookup_scheme_id(&self, name: Name) -> Option<&TypeSchemeId> {
        self.0.bindings.get(&name).or_else(|| {
            self.0.parent.as_ref().and_then(|p| p.lookup_scheme_id(name))
        })
    }

    /// Check if bound in current scope only (not parents).
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.0.bindings.contains_key(&name)
    }
}
```

Note: `Rc::make_mut` provides copy-on-write semantics - the inner data is only cloned if there are multiple references.

### Shadowing

Variables in inner scopes shadow outer ones:

```ori
let x = 1       // x : int in outer scope
let result = run(
    let x = "hello",  // x : str in inner scope
    len(collection: x),  // uses inner x
)
// x : int (outer still visible)
```

```rust
// In infer_run:
let child_env = self.env.child();
child_env.bind(x_name, Type::Str);  // Shadows outer x
let len_result = self.infer_expr_with_env(len_call, &child_env);
// Original env still has x : Int
```

## Type Schemes

The environment stores `TypeSchemeId` which represents both monomorphic and polymorphic types:

```rust
/// Interned type scheme reference.
pub struct TypeSchemeId {
    /// The type variables quantified over (empty for monomorphic).
    pub vars: Vec<TypeVar>,
    /// The type body (as TypeId).
    pub ty: TypeId,
}

impl TypeSchemeId {
    /// Create a monomorphic scheme (no quantified variables).
    pub fn mono(ty: TypeId) -> Self {
        TypeSchemeId { vars: Vec::new(), ty }
    }
}
```

### Instantiation

Polymorphic types are instantiated with fresh type variables via `InferenceContext`:

```rust
impl InferenceContext {
    /// Instantiate a type scheme with fresh variables.
    pub fn instantiate(&mut self, scheme: &TypeScheme) -> Type {
        if scheme.vars.is_empty() {
            return scheme.ty.clone();
        }

        // Create fresh variables for each quantified variable
        let subst: HashMap<TypeVar, Type> = scheme.vars
            .iter()
            .map(|&v| (v, self.fresh_var()))
            .collect();

        scheme.ty.apply_subst(&subst)
    }
}
```

Example:
```ori
let id = x -> x  // forall T. T -> T
id(42)           // Instantiate: T0 -> T0, then unify T0=int
id("hi")         // Instantiate: T1 -> T1, then unify T1=str
```

### Names Iterator

The environment provides an iterator over all bound names (for "did you mean?" suggestions):

```rust
impl TypeEnv {
    /// Iterate over all bound names (current + parent scopes).
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        NamesIterator { current: Some(self), current_iter: None }
    }
}
```

## Function Scopes

Functions create a child scope with parameters:

```rust
fn infer_function(&mut self, func: &Function) -> Type {
    let mut func_env = self.env.child();

    // Bind parameters
    for param in &func.params {
        func_env.bind(param.name, param.ty.clone());
    }

    // Infer body type with function scope
    let body_ty = self.infer_expr_with_env(func.body, &func_env);

    // Check return type annotation
    if let Some(ret_ty) = &func.ret_type {
        self.ctx.unify(&body_ty, ret_ty)?;
    }

    Type::Function {
        params: func.params.iter().map(|p| p.ty.clone()).collect(),
        ret: Box::new(body_ty),
    }
}
```

## For Loop Scopes

For loops bind the iteration variable in a child scope:

```rust
fn infer_for(&mut self, for_expr: &ForExpr) -> Type {
    let iter_ty = self.infer_expr(for_expr.iterable);

    // Extract element type
    let elem_ty = match &iter_ty {
        Type::List(elem) => (**elem).clone(),
        Type::Range(elem) => (**elem).clone(),
        _ => {
            self.push_error(TypeError::NotIterable(iter_ty));
            return Type::Error;
        }
    };

    let mut loop_env = self.env.child();
    loop_env.bind(for_expr.var, elem_ty);

    let body_ty = self.infer_expr_with_env(for_expr.body, &loop_env);

    // for..yield produces a list
    if for_expr.is_yield {
        Type::List(Box::new(body_ty))
    } else {
        Type::Unit
    }
}
```

## Error Recovery

The `names()` iterator enables "did you mean?" suggestions:

```rust
fn error_undefined(&self, name: Name, span: Span) {
    let similar: Vec<_> = self.env.names()
        .filter(|&n| self.is_similar(n, name))
        .collect();

    self.push_error(TypeError::UndefinedVariable {
        name,
        similar,
        span,
    });
}
```

## Free Type Variables

Collect free type variables not quantified in the environment:

```rust
impl TypeEnv {
    /// Collect all free type variables in the environment.
    ///
    /// Used during generalization to avoid quantifying over
    /// variables that are free in the environment.
    pub fn free_vars(&self, ctx: &InferenceContext) -> Vec<TypeVar> {
        let mut vars = HashSet::new();
        self.collect_env_free_vars(ctx, &mut vars);
        vars.into_iter().collect()
    }

    fn collect_env_free_vars(&self, ctx: &InferenceContext, vars: &mut HashSet<TypeVar>) {
        for scheme in self.0.bindings.values() {
            // Only collect free vars NOT quantified in the scheme
            let scheme_free = ctx.free_vars_id(scheme.ty);
            for v in scheme_free {
                if !scheme.vars.contains(&v) {
                    vars.insert(v);
                }
            }
        }
        if let Some(parent) = &self.0.parent {
            parent.collect_env_free_vars(ctx, vars);
        }
    }
}
```

Used for let-generalization to determine which variables can be generalized.
