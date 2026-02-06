---
title: "Type Environment"
description: "Ori Compiler Design — Type Environment"
order: 501
section: "Type System"
---

# Type Environment

The type environment tracks variable-to-type bindings during type checking. It uses an `Rc`-based parent chain for efficient scope management, with all types stored as `Idx` handles into the pool.

## Location

```
compiler/ori_types/src/infer/env.rs
```

## Structure

```rust
struct TypeEnvInner {
    bindings: FxHashMap<Name, Idx>,
    parent: Option<TypeEnv>,
}

pub struct TypeEnv(Rc<TypeEnvInner>);
```

**Design decisions:**
- `Rc<TypeEnvInner>` enables O(1) child scope creation (cheap `Rc` clone, no recursive copying)
- `FxHashMap<Name, Idx>` provides fast hashing for interned `Name` keys
- Bindings map directly to `Idx` (pool handles), not boxed `Type` values
- Parent chain enables lexical scope lookup through linked environments

## Scope Management

Child scopes are created via `child()`, not push/pop. The parent environment remains immutable:

```rust
impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
        }))
    }

    /// Create a child scope — O(1) due to Rc parent sharing.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: Some(self.clone()),  // Cheap Rc clone
        }))
    }
}
```

The caller is responsible for using the child scope where appropriate and discarding it when the scope exits.

## Binding and Lookup

```rust
impl TypeEnv {
    /// Bind a name to a type (as Idx).
    pub fn bind(&mut self, name: Name, ty: Idx) {
        Rc::make_mut(&mut self.0).bindings.insert(name, ty);
    }

    /// Look up a name, searching parent scopes.
    pub fn lookup(&self, name: Name) -> Option<Idx> {
        self.0.bindings.get(&name).copied().or_else(|| {
            self.0.parent.as_ref().and_then(|p| p.lookup(name))
        })
    }

    /// Check if bound in current scope only (not parents).
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.0.bindings.contains_key(&name)
    }
}
```

`Rc::make_mut` provides copy-on-write semantics — the inner data is only cloned if there are multiple references, which prevents unnecessary allocation when the environment has a single owner.

## Shadowing

Variables in inner scopes shadow outer ones. The parent chain lookup stops at the first match:

```ori
let x = 1
let result = run(
    let x = "hello",   // x : str in inner scope (shadows outer)
    len(collection: x), // uses inner x : str
)
// x : int (outer still visible)
```

```rust
// During type checking of the run block:
let child_env = engine.env.child();
child_env.bind(x_name, Idx::STR);  // Shadows outer x : int
// lookup(x_name) in child_env returns Idx::STR
// lookup(x_name) in original env still returns Idx::INT
```

## Polymorphic Types

When a `let`-bound value is generalized into a type scheme, the scheme `Idx` (of tag `Tag::Scheme`) is stored directly in the environment. On lookup, the `InferEngine` checks whether the retrieved `Idx` is a scheme and instantiates it with fresh variables:

```ori
let id = x -> x           // generalized to forall T. T -> T
let a = id(42)            // instantiate: T0 -> T0, unify T0 = int
let b = id("hello")       // instantiate: T1 -> T1, unify T1 = str
```

The environment itself does not distinguish between monomorphic and polymorphic bindings — both are stored as `Idx`. The distinction is made by the tag: `Tag::Scheme` indicates a polymorphic type that needs instantiation.

## Function Scopes

Functions create a child scope with parameter bindings:

```rust
// In ModuleChecker, checking a function body:
let mut func_env = base_env.child();
for (param_name, param_type) in &signature.params {
    func_env.bind(*param_name, *param_type);
}
// Create InferEngine with func_env, infer body
```

## For Loop Scopes

For loops bind the iteration variable in a child scope:

```ori
for x in [1, 2, 3] do print(x)
```

```rust
// Infer iterable type: [int]
// Extract element type: int
let mut loop_env = engine.env.child();
loop_env.bind(x_name, Idx::INT);
// Infer body with loop_env
```

## Error Recovery

The environment supports "did you mean?" suggestions by iterating over all bound names:

```rust
impl TypeEnv {
    /// Iterate over all bound names (current + parent scopes).
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        // Walks the parent chain, yielding names from each scope
    }
}
```

This enables the error infrastructure to suggest similar names when an undefined variable is referenced.
