---
title: "Type Environment"
description: "Ori Compiler Design — Type Environment"
order: 501
section: "Type System"
---

# Type Environment

## What Is a Type Environment?

A type environment (also called a **typing context** or **symbol table**) maps names to their types. When the type checker encounters the expression `x + 1`, it looks up `x` in the environment to find its type. If `x` is bound as `int`, the addition type-checks; if `x` is bound as `str`, it produces a type error.

Type environments are fundamental to every type system. The notation Γ ⊢ e : τ ("expression e has type τ in environment Γ") appears in virtually every typing judgment in the literature. The environment Γ is where all the accumulated knowledge about the program's bindings lives.

### The Scope Problem

The interesting challenge is **scoping**. Variables can be shadowed, and new scopes can introduce names that are only visible within that scope:

```ori
let x = 1;
let result = {
    let x = "hello";     // shadows outer x
    len(collection: x)   // uses inner x : str
};
// x : int (outer binding restored)
```

The environment must handle three operations efficiently:

1. **Bind** — add a name-to-type mapping in the current scope
2. **Lookup** — find a name's type, searching outward through enclosing scopes
3. **Enter/exit scope** — create a new scope (for blocks, functions, loops) and discard it when done

The design of these three operations determines the environment's performance characteristics and memory behavior.

### Classical Approaches

Compilers have used several environment representations:

**Flat hash map with push/pop.** Maintain a single `HashMap<Name, Vec<Type>>` where each name maps to a stack of bindings. Entering a scope pushes new bindings; exiting pops them. Fast lookup (O(1) hash + O(1) stack peek) and compact, but scope exit requires tracking which names were bound in the current scope to know what to pop.

**Immutable linked list (functional).** Each scope is a list node pointing to its parent. Entering a scope allocates a new node; exiting scope simply drops the reference. This is the approach used in many functional language implementations (Haskell, ML). Lookup walks the list (O(depth)), but scope creation is O(1). The functional purity means scopes can be freely shared — useful when checking multiple branches of a match expression against the same parent scope.

**Persistent hash maps.** Libraries like Haskell's `Data.HashMap.Strict` or Clojure's persistent maps provide immutable maps with structural sharing. Binding is O(log n), lookup is O(log n), and old versions are preserved. Used by some research compilers but rarely in production due to constant-factor overhead.

**Rc-linked scope chain.** A hybrid approach: each scope is a hash map pointing to a parent scope via reference counting. Lookup searches the chain (O(depth) worst case, but usually O(1) since most lookups hit the current scope). Scope creation is O(1) via `Rc` cloning. This is what Ori uses.

## Ori's Approach: Rc-Linked Scope Chain

Ori's `TypeEnv` uses an `Rc`-based parent chain with `FxHashMap` at each scope level:

```rust
struct Binding {
    ty: Idx,                       // Type (or type scheme) from pool
    mutable: Option<Mutability>,   // let x (mutable) vs let $x (immutable)
}

struct TypeEnvInner {
    bindings: FxHashMap<Name, Binding>,
    parent: Option<TypeEnv>,
}

pub struct TypeEnv(Rc<TypeEnvInner>);
```

### Why This Design?

**`Rc<TypeEnvInner>`** enables O(1) child scope creation. Creating a child scope is just an `Rc::clone` of the parent (a reference count increment) plus a new empty hash map for the child's own bindings. No copying of the parent's bindings.

**`FxHashMap<Name, Binding>`** provides fast hashing for interned `Name` keys. Since `Name` is a `u32` index into the string interner, hashing is a single integer operation — `FxHashMap` (from Firefox's hash implementation) is optimized for integer keys with minimal entropy mixing.

**`Binding`** combines type and mutability in one struct, avoiding parallel maps. The `mutable` field is `None` for bindings that don't track mutability (prelude bindings, function parameters), `Some(Mutable)` for `let x`, and `Some(Immutable)` for `let $x`. This distinction lets the type checker reject assignment to immutable bindings.

**`Idx` storage (not `Box<Type>`)** keeps bindings as 4-byte pool handles. There's no recursive type structure in the environment — just indices into the pool where the full type information lives.

## Scope Management

Child scopes are created via `child()`, not push/pop. The parent environment remains immutable — no state to restore on scope exit:

```rust
impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
        }))
    }

    /// Create a child scope — O(1) via Rc parent sharing.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: Some(self.clone()),  // Cheap Rc clone
        }))
    }
}
```

The `#[must_use]` annotation prevents accidentally discarding the child scope — the caller must use it or explicitly drop it. This prevents a class of bugs where a scope is created but the original environment is used by mistake.

The caller is responsible for using the child scope where appropriate (in blocks, function bodies, loop bodies) and letting it go out of scope naturally when the scope exits. No explicit "close scope" call is needed — Rust's ownership system handles cleanup.

### Copy-on-Write Semantics

When a `TypeEnv` has multiple references (e.g., a parent that has been shared with multiple children), binding a new name uses `Rc::make_mut` to clone the inner data only if needed:

```rust
pub fn bind(&mut self, name: Name, ty: Idx) {
    Rc::make_mut(&mut self.0).bindings.insert(name, Binding { ty, mutable: None });
}
```

`Rc::make_mut` checks the reference count. If there's only one reference (the common case — the environment is owned, not shared), it mutates in place. If there are multiple references, it clones the inner data first, ensuring other references see the original bindings. This gives copy-on-write semantics without explicit clone management.

## Binding and Lookup

### Binding

```rust
impl TypeEnv {
    /// Bind a name to a type without mutability tracking.
    pub fn bind(&mut self, name: Name, ty: Idx);

    /// Bind with explicit mutability.
    pub fn bind_with_mutability(&mut self, name: Name, ty: Idx, mutable: Mutability);

    /// Bind a type scheme (alias for bind, for code clarity).
    pub fn bind_scheme(&mut self, name: Name, scheme: Idx);
}
```

Bindings only affect the current scope — they never propagate to parent scopes. A child scope's binding shadows the parent's binding for the same name, which is exactly the semantics of lexical scoping.

### Lookup

```rust
impl TypeEnv {
    /// Look up a name, searching parent scopes.
    pub fn lookup(&self, name: Name) -> Option<Idx>;

    /// Check if a binding is mutable, searching parent scopes.
    pub fn is_mutable(&self, name: Name) -> Option<bool>;

    /// Check if bound in current scope only (not parents).
    pub fn is_bound_locally(&self, name: Name) -> bool;

    /// Count bindings in current scope only.
    pub fn local_count(&self) -> usize;

    /// Get the parent scope, if any.
    pub fn parent(&self) -> Option<Self>;
}
```

`lookup` searches the parent chain from innermost to outermost scope, returning the first match. This implements standard lexical scoping — inner bindings shadow outer ones:

```ori
let x = 1;              // x : int in outer scope
let result = {
    let x = "hello";    // x : str in inner scope (shadows)
    len(collection: x)  // lookup(x) finds str (inner), not int (outer)
};
// lookup(x) here finds int (outer) — inner scope is gone
```

The `is_bound_locally` method checks only the current scope, which is used to detect accidental shadowing in the same scope (e.g., two `let x` bindings in the same block).

## Shadowing

Shadowing is a deliberate feature in Ori, not an accident. Variables in inner scopes shadow outer ones, and the parent chain lookup stops at the first match:

```ori
let x = 1
let result = {
    let x = "hello"     // x : str shadows outer x : int
    len(collection: x)  // uses inner x : str
}
// x : int (outer still visible)
```

During type checking:

```rust
// Outer scope
let outer_env = engine.env.clone();
outer_env.bind(x_name, Idx::INT);

// Block creates child scope
let mut child_env = outer_env.child();
child_env.bind(x_name, Idx::STR);  // Shadows outer x

// lookup(x) in child_env → Idx::STR (inner scope)
// lookup(x) in outer_env → Idx::INT (outer scope, unchanged)
```

## Polymorphic Bindings

When a `let`-bound value is generalized into a type scheme (via the rank system — see [Unification](unification.md)), the scheme `Idx` is stored directly in the environment. The environment does not distinguish between monomorphic and polymorphic bindings — both are stored as `Idx`. The distinction is made by the tag: `Tag::Scheme` indicates a polymorphic type that needs instantiation.

```ori
let id = x -> x           // generalized to forall T. T -> T
let a = id(42)            // instantiate: T0 -> T0, unify T0 = int
let b = id("hello")       // instantiate: T1 -> T1, unify T1 = str
```

On lookup, the `InferEngine` checks whether the retrieved `Idx` is a scheme and instantiates it with fresh variables:

```rust
let ty = engine.env.lookup(name)?;
if engine.pool.tag(ty) == Tag::Scheme {
    engine.unify.instantiate(ty)  // Fresh vars for each quantified variable
} else {
    ty
}
```

This transparent handling means the environment doesn't need special "polymorphic binding" infrastructure — schemes are just another kind of type `Idx`.

## Scope Usage Patterns

### Function Scopes

Functions create a child scope with parameter bindings:

```rust
let mut func_env = base_env.child();
for (param_name, param_type) in &signature.params {
    func_env.bind(*param_name, *param_type);
}
// Create InferEngine with func_env, infer body
```

### For Loop Scopes

For loops bind the iteration variable in a child scope:

```ori
for x in [1, 2, 3] do print(msg: str(value: x))
```

```rust
let mut loop_env = engine.env.child();
loop_env.bind(x_name, elem_type);  // elem_type inferred from iterable
// Infer loop body with loop_env
```

### Match Arm Scopes

Each match arm creates its own scope for pattern bindings:

```ori
match value {
    Some(n) -> n + 1,    // n bound in arm scope
    None -> 0,           // different scope, no n
}
```

### Block Scopes

Block expressions create scopes for local bindings:

```ori
let result = {
    let $x = compute();
    let $y = transform(value: x);
    x + y                // result expression
}
```

## Error Recovery: "Did You Mean?"

The environment supports typo suggestions by iterating over all bound names and computing edit distances:

```rust
impl TypeEnv {
    /// Iterate over all bound names (current + parent scopes).
    pub fn names(&self) -> impl Iterator<Item = Name> + '_;

    /// Find names similar to the given name.
    pub fn find_similar<'r>(
        &self,
        target: Name,
        max_results: usize,
        resolve: impl Fn(Name) -> Option<&'r str>,
    ) -> Vec<Name>;
}
```

`find_similar` uses Levenshtein edit distance with dynamic thresholds based on name length:

| Name Length | Max Edit Distance | Example |
|-------------|-------------------|---------|
| 1–2 chars | 1 | `x` → `y` |
| 3–5 chars | 2 | `name` → `nane` |
| 6+ chars | 3 | `transform` → `transfom` |

A quick length-difference reject filters out obviously dissimilar names before computing the full edit distance, keeping the cost low for large environments. Names are deduplicated across scopes (a name shadowed in an inner scope appears only once in the suggestion list).

## Prior Art

### ML and OCaml — Functional Environments

[OCaml](https://github.com/ocaml/ocaml) uses functional (immutable) environments represented as association lists or balanced trees. Each scope produces a new environment without modifying the parent. This purity makes it easy to check multiple branches of a pattern match against the same base environment. Ori's `Rc`-based approach achieves similar sharing semantics with hash map lookup performance.

### GHC — TcRnEnv

[GHC](https://gitlab.haskell.org/ghc/ghc)'s type checking environment (`TcRnEnv`) is substantially more complex — it tracks type class instances, type family equations, given constraints, and wanted constraints alongside simple name bindings. This complexity is driven by Haskell's richer type system. Ori's environment is simpler because it only needs to track name-to-type bindings and mutability.

### Rust (rustc) — Scoped Resolution

[Rust's compiler](https://github.com/rust-lang/rust) uses a resolver that builds a tree of scopes during name resolution (before type checking). Type checking then operates on resolved names, not raw strings. Ori combines name resolution and type checking in a single pass, using the environment for both.

### Elm — Scope-Based Error Recovery

[Elm](https://github.com/elm/compiler)'s type checker uses the environment for "did you mean?" suggestions, similar to Ori's `find_similar`. Elm's approach of computing edit distances over all names in scope produces high-quality error messages for typos. Ori's implementation follows the same pattern with the additional optimization of length-based early rejection.

## Design Tradeoffs

**Rc-linked chain vs. flat stack.** The `Rc` chain enables O(1) scope creation and natural sharing between branches (e.g., two match arms sharing the same parent scope). A flat stack (like a `Vec<HashMap>`) would have O(1) lookup (top-of-stack first) but O(n) scope creation if bindings need copying. The `Rc` approach trades slightly more complex lifetime management for better sharing characteristics.

**Per-scope hash maps vs. single global map.** Each scope has its own `FxHashMap`, which means lookup walks the parent chain checking each map. A single global map with a scope stack per name would give O(1) lookup always, but scope exit would require scanning the map for entries to pop. For the typical scope depth (3–5 levels), the chain walk is fast enough, and the simpler data structure is easier to maintain.

**`Option<Mutability>` vs. always tracking.** Not all bindings have meaningful mutability (prelude bindings, function parameters). Using `Option<Mutability>` avoids inventing a mutability for bindings where the concept doesn't apply, at the cost of a slightly more complex API (callers must handle `None`).

**No De Bruijn indices.** Some type checkers use De Bruijn indices (integers that count the number of enclosing lambdas) instead of names, eliminating the need for alpha-renaming. Ori uses names directly because De Bruijn indices make error messages harder to produce (you need to reconstruct the original name from the index) and interact poorly with named arguments and the capability system. The lookup cost of using names (hash table access) is negligible compared to the clarity benefit.
