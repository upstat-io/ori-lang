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
compiler/ori_types/src/infer/env/mod.rs
```

## Structure

```rust
/// A single binding entry in the type environment.
#[derive(Copy, Clone, Debug)]
struct Binding {
    ty: Idx,                       // The type (or type scheme) for this name
    mutable: Option<Mutability>,   // None for prelude/param bindings
}

struct TypeEnvInner {
    bindings: FxHashMap<Name, Binding>,
    parent: Option<TypeEnv>,
}

pub struct TypeEnv(Rc<TypeEnvInner>);
```

**Design decisions:**
- `Rc<TypeEnvInner>` enables O(1) child scope creation (cheap `Rc` clone, no recursive copying)
- `FxHashMap<Name, Binding>` provides fast hashing for interned `Name` keys
- `Binding` combines type and mutability in one struct, eliminating the need for parallel maps
- Bindings store `Idx` (pool handles), not boxed `Type` values
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
    /// Get the parent scope, if any.
    pub fn parent(&self) -> Option<Self>;

    /// Bind a name to a type (as Idx). Mutability is not tracked.
    pub fn bind(&mut self, name: Name, ty: Idx);

    /// Bind a name to a type and record its mutability.
    /// Mutability::Mutable = `let x` (can be reassigned).
    /// Mutability::Immutable = `let $x` (immutable binding).
    pub fn bind_with_mutability(&mut self, name: Name, ty: Idx, mutable: Mutability);

    /// Look up a name, searching parent scopes.
    pub fn lookup(&self, name: Name) -> Option<Idx>;

    /// Check if a binding is mutable, searching parent scopes.
    /// Returns Some(true) for mutable, Some(false) for immutable,
    /// None if the name has no recorded mutability (e.g., function params,
    /// prelude bindings).
    pub fn is_mutable(&self, name: Name) -> Option<bool>;

    /// Bind a name to a type scheme (alias for bind, for code clarity).
    pub fn bind_scheme(&mut self, name: Name, scheme: Idx);

    /// Look up a name and return its type scheme (alias for lookup).
    pub fn lookup_scheme(&self, name: Name) -> Option<Idx>;

    /// Check if bound in current scope only (not parents).
    pub fn is_bound_locally(&self, name: Name) -> bool;

    /// Count bindings in the current scope only (not parents).
    pub fn local_count(&self) -> usize;
}
```

`Rc::make_mut` provides copy-on-write semantics — the inner data is only cloned if there are multiple references, which prevents unnecessary allocation when the environment has a single owner.

## Shadowing

Variables in inner scopes shadow outer ones. The parent chain lookup stops at the first match:

```ori
let x = 1;
let result = {
    let x = "hello";   // x : str in inner scope (shadows outer)
    len(collection: x)  // uses inner x : str
};
// x : int (outer still visible)
```

```rust
// During type checking of the block expression:
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

The environment supports "did you mean?" suggestions by iterating over all bound names and finding similar ones via edit distance:

```rust
impl TypeEnv {
    /// Iterate over all bound names (current + parent scopes).
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        // Walks the parent chain, yielding names from each scope
    }

    /// Find names similar to the given name (for typo suggestions).
    /// Uses Levenshtein edit distance with a dynamic threshold based
    /// on name length (1-2 chars: distance <= 1, 3-5: <= 2, 6+: <= 3).
    /// Returns up to `max_results` names, sorted by edit distance.
    /// The `resolve` closure maps Name handles to their string representations.
    pub fn find_similar<'r>(
        &self,
        target: Name,
        max_results: usize,
        resolve: impl Fn(Name) -> Option<&'r str>,
    ) -> Vec<Name>;
}
```

`find_similar()` deduplicates names across scopes and applies a quick length-difference reject before computing the full edit distance, keeping the cost low for large environments.
