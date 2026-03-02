---
title: "Unification"
description: "Ori Compiler Design — Unification"
order: 504
section: "Type System"
---

# Unification

## What Is Unification?

Type inference constantly asks the question: "can these two types be made equal?" When you write `let x = 42; let y = x + 1`, the type checker needs the operands of `+` to be the same type. When you call `identity(42)` where `identity : T -> T`, the checker needs `T` to equal `int`. **Unification** is the algorithm that answers this question.

More precisely, given two type expressions that may contain type variables, unification finds a *substitution* — an assignment of types to variables — that makes the two expressions identical. If no such substitution exists, unification fails and the type checker reports a type error.

The concept was introduced by **J.A. Robinson** in his [1965 paper on mechanical theorem proving](https://dl.acm.org/doi/10.1145/321250.321253). Robinson showed that first-order unification is decidable and gave an algorithm for it. Every Hindley-Milner type checker since then has used unification as its core operation.

### Unification by Example

Consider unifying `(T, int)` with `(str, U)` — two tuple types with type variables `T` and `U`:

```
unify((T, int), (str, U))
  → unify(T, str)         T must equal str
  → unify(int, U)         U must equal int
  → substitution: {T → str, U → int}
```

Both types become `(str, int)` under this substitution. Unification succeeded.

Now consider unifying `int` with `str`:

```
unify(int, str)
  → no substitution makes int equal str
  → FAIL: type mismatch
```

And the tricky case — unifying `T` with `[T]`:

```
unify(T, [T])
  → T must equal [T]
  → but [T] contains T, so T = [T] = [[T]] = [[[T]]] = ...
  → FAIL: infinite type (occurs check)
```

This last case is why every unification algorithm includes an **occurs check** — verifying that a variable doesn't appear in the type it's being unified with, which would create an infinite type.

### Implementation Approaches

There are two fundamental approaches to implementing unification:

**Substitution maps.** The textbook approach maintains an explicit map from variables to types: `HashMap<VarId, Type>`. When `T` unifies with `int`, add `T → int` to the map. When looking up a variable, consult the map and follow chains. This is simple and pure (no mutation) but has overhead: the map grows monotonically, and every lookup requires a hash table access.

**Mutable linking (union-find).** The efficient approach stores the unification result *in the variable itself*. When `T` unifies with `int`, mutate T's state to `Link { target: int }`. Subsequent lookups follow the link directly. With path compression (shortcutting chains during traversal), this achieves O(α(n)) amortized complexity — effectively constant time. This is the approach used by Ori and most production type checkers.

The union-find approach was originally developed by Tarjan (1975) for the disjoint-set problem. Its application to type unification is natural: type variables partition into equivalence classes, and unification merges classes.

## The UnifyEngine

```rust
pub struct UnifyEngine<'pool> {
    pool: &'pool mut Pool,
    current_rank: Rank,
    errors: Vec<UnifyError>,
}
```

The engine is deliberately minimal. It borrows the pool mutably (to update `VarState` links during unification), tracks the current scope depth for let-polymorphism (see Rank System below), and accumulates errors rather than returning them immediately.

Errors are accumulated because type checking should not stop at the first unification failure. When `unify(int, str)` fails, the engine records the error and continues, allowing the type checker to discover additional errors in the same function body. Only after all expressions are checked does the error list get reported.

## Link-Based Union-Find

Ori's unification uses **direct linking** through the pool's `VarState`, not a separate substitution map:

```rust
pub enum VarState {
    Unbound { id: u32, rank: Rank, name: Option<Name> },
    Link { target: Idx },     // Points to unified type
    Rigid { name: Name },     // From annotation, cannot unify
    Generalized { id: u32, name: Option<Name> },
}
```

When variable `T0` unifies with `int`, the engine sets `var_states[T0] = Link { target: Idx::INT }`. There is no separate substitution map — the variable *is* the map entry.

### Path Compression

Following link chains can create multi-hop paths: `T0 → T1 → T2 → int`. Without optimization, each lookup follows the full chain. **Path compression** short-circuits this by updating intermediate links to point directly to the final target during resolution:

```rust
pub fn resolve(&mut self, idx: Idx) -> Idx {
    if pool.tag(idx) == Tag::Var {
        let var_id = pool.data(idx);
        match pool.var_state(var_id) {
            VarState::Link { target } => {
                let resolved = self.resolve(target);
                // Path compression: skip intermediate links
                pool.set_var_state(var_id, VarState::Link { target: resolved });
                resolved
            }
            _ => idx,  // Unbound, rigid, or generalized — stop here
        }
    } else {
        idx  // Not a variable, return as-is
    }
}
```

After resolution:

```
Before: T0 → T1 → T2 → int
After:  T0 → int, T1 → int, T2 → int
```

With path compression, the amortized cost of `resolve()` is O(α(n)), where α is the inverse Ackermann function — a function that grows so slowly it's effectively constant for all practical inputs (α(2^65536) = 5). This makes unification nearly free compared to the substitution-map approach, where every lookup is O(1) but with hash table constant factors.

### Core Unification Algorithm

The `unify` method is the heart of the type checker. It handles identical types, variable binding, error recovery, and structural recursion:

```rust
pub fn unify(&mut self, left: Idx, right: Idx) {
    // Fast path: identical indices (same type)
    if left == right { return; }

    // Resolve both sides (follow links)
    let left = self.resolve(left);
    let right = self.resolve(right);

    // Check again after resolution
    if left == right { return; }

    let left_tag = self.pool.tag(left);
    let right_tag = self.pool.tag(right);

    match (left_tag, right_tag) {
        // Variable unifies with anything (after occurs check)
        (Tag::Var, _) => self.unify_var(left, right),
        (_, Tag::Var) => self.unify_var(right, left),

        // Error and Never unify with anything (error recovery / bottom type)
        (Tag::Error, _) | (_, Tag::Error) => {},
        (Tag::Never, _) | (_, Tag::Never) => {},

        // Structural unification for matching tags
        (Tag::List, Tag::List) => {
            self.unify(self.pool.list_elem(left), self.pool.list_elem(right));
        }
        (Tag::Option, Tag::Option) => {
            self.unify(self.pool.option_inner(left), self.pool.option_inner(right));
        }
        (Tag::Map, Tag::Map) => {
            self.unify(self.pool.map_key(left), self.pool.map_key(right));
            self.unify(self.pool.map_value(left), self.pool.map_value(right));
        }
        (Tag::Function, Tag::Function) => {
            let lc = self.pool.function_param_count(left);
            let rc = self.pool.function_param_count(right);
            if lc != rc {
                self.push_error(UnifyError::ArityMismatch { expected: lc, got: rc });
                return;
            }
            for i in 0..lc {
                self.unify(
                    self.pool.function_param(left, i),
                    self.pool.function_param(right, i),
                );
            }
            self.unify(
                self.pool.function_return(left),
                self.pool.function_return(right),
            );
        }
        (Tag::Tuple, Tag::Tuple) => {
            // Check element count, then unify each element
        }
        // ... other compound types ...

        // Mismatch — no rule applies
        _ => self.push_error(UnifyError::Mismatch { expected: left, got: right }),
    }
}
```

The algorithm proceeds in layers:

1. **Identity check** — if the two `Idx` values are equal, the types are trivially the same (O(1) thanks to interning)
2. **Resolution** — follow variable links to find the actual types
3. **Post-resolution identity** — check again after resolution (a common case when both sides are linked to the same type)
4. **Variable binding** — if either side is a variable, bind it to the other (with occurs check)
5. **Error/Never absorption** — `Error` and `Never` unify with anything to prevent cascading errors and support bottom types
6. **Structural recursion** — for compound types with matching tags, recursively unify their children
7. **Mismatch** — if no rule applies, report a type error

### Variable Binding

When unifying a variable with a type, the engine must check for infinite types and handle rank ordering:

```rust
fn unify_var(&mut self, var_idx: Idx, other: Idx) {
    let var_id = self.pool.data(var_idx);
    let var_state = self.pool.var_state(var_id);

    match var_state {
        VarState::Unbound { rank, .. } => {
            // Occurs check: prevent T = [T]
            if self.occurs_in(var_id, other) {
                self.push_error(UnifyError::InfiniteType { var: var_idx, ty: other });
                return;
            }
            // Link the variable to the other type
            self.pool.set_var_state(var_id, VarState::Link { target: other });
        }
        VarState::Rigid { name } => {
            // Rigid variables cannot unify with concrete types
            self.push_error(UnifyError::RigidMismatch { var: var_idx, ty: other });
        }
        _ => unreachable!("resolved variables should not reach unify_var"),
    }
}
```

Rigid variables (from type annotations like `T` in `@foo<T>(x: T)`) reject unification with concrete types. They represent universally quantified parameters that must remain abstract — `T` should work for *any* type, so it cannot be pinned to `int` or `str`.

## Flag-Gated Occurs Check

The occurs check prevents infinite types. Without it, `T = [T]` would succeed, creating an infinitely nested list type that no finite representation can hold.

The naive occurs check traverses the entire type to look for the variable — O(n) in the size of the type. Ori optimizes this with the pool's `TypeFlags`:

```rust
fn occurs_in(&self, var_id: u32, in_type: Idx) -> bool {
    // O(1) fast path: if the type has no variables, skip traversal entirely
    if !self.pool.flags(in_type).contains(TypeFlags::HAS_VAR) {
        return false;
    }
    // Full traversal only when HAS_VAR is set
    self.occurs_in_inner(var_id, in_type)
}
```

Since `TypeFlags::HAS_VAR` propagates from children to parents during type construction, a monomorphic type like `(int, str) -> bool` has `HAS_VAR = false`, and the occurs check returns immediately. In practice, most types in a program are monomorphic, so this optimization skips the vast majority of occurs checks.

This is one of the most impactful optimizations in the type checker — it turns a potentially expensive O(n) traversal into an O(1) bitfield check for the common case.

## The Rank System

The rank system controls **let-polymorphism** — the mechanism that determines which type variables can be generalized (made polymorphic) and which cannot.

### Why Ranks?

Consider this program:

```ori
let id = x -> x
let a = id(42)        // works: id instantiated with T = int
let b = id("hello")   // works: id instantiated with T = str
```

For this to work, `id` must have a polymorphic type `forall T. T -> T`. But not every lambda should be generalized:

```ori
let f = x -> {
    let g = x           // g should NOT be forall T. T — it's the same x
    (g, g)
}
```

The rank system determines the boundary: variables created *inside* a let-binding's scope can be generalized when exiting that scope, but variables from *outside* cannot.

### Rank Constants

```rust
#[repr(transparent)]
pub struct Rank(u16);

impl Rank {
    pub const TOP: Self = Self(0);      // Universally quantified
    pub const IMPORT: Self = Self(1);   // Imported from modules
    pub const FIRST: Self = Self(2);    // Top-level in current module
    pub const MAX: Self = Self(u16::MAX - 1);
}
```

| Constant | Value | Purpose |
|----------|-------|---------|
| `TOP` | 0 | Already-generalized variables (in type schemes) |
| `IMPORT` | 1 | Type schemes imported from other modules |
| `FIRST` | 2 | Top-level definitions; type checking starts here |
| `MAX` | 65534 | Safety bound preventing overflow in deeply nested code |

### Scope Management

The engine tracks the current rank and adjusts it when entering and exiting scopes:

```rust
impl UnifyEngine {
    pub fn enter_scope(&mut self) {
        self.current_rank = self.current_rank.next();
    }

    pub fn exit_scope(&mut self) {
        self.current_rank = self.current_rank.prev().max(Rank::FIRST);
    }
}
```

The `exit_scope()` method clamps at `FIRST` to prevent underflow. The `InferEngine` wraps these in higher-level methods that also manage the `TypeEnv` child scope chain.

### How Generalization Works

When a `let`-bound value's type is fully inferred, the engine generalizes unbound variables at the current rank:

```
Rank 2 (module level):
  let id = x -> x         ← enter rank 3, infer lambda
                           ← T0 created at rank 3
                           ← T0 unbound after inference
                           ← generalize: T0 at rank 3 → Generalized
                           ← id : forall T. T -> T (scheme)
  let a = id(42)           ← instantiate: fresh T1 at rank 2
                           ← unify T1 = int
  let b = id("hello")      ← instantiate: fresh T2 at rank 2
                           ← unify T2 = str
```

A variable can be generalized if its rank is ≥ the generalization rank:

```rust
impl Rank {
    pub fn can_generalize_at(&self, gen_rank: Rank) -> bool {
        self.0 >= gen_rank.0
    }
}
```

Variables from outer scopes (lower ranks) are *not* generalized — they represent constraints that are still being solved. Only variables local to the current let-binding are free to become polymorphic.

### Generalization and Instantiation

```rust
/// Generalize unbound variables at the given rank into a type scheme.
pub fn generalize(&mut self, ty: Idx, rank: Rank) -> Idx {
    // Walk the type, converting Unbound vars at rank >= current to Generalized
    // Returns a Scheme(vars, body) if any variables were generalized
    // Returns ty directly if no variables were generalized (monomorphic)
}

/// Replace generalized variables with fresh unbound variables.
pub fn instantiate(&mut self, scheme: Idx) -> Idx {
    // For each Generalized variable in the scheme, create a fresh Unbound var
    // at the current rank, and substitute into the body
}
```

The monomorphic optimization in `generalize` is important: most bindings are not polymorphic, so eliding the scheme wrapper saves both allocation and the instantiation overhead at use sites.

## Special Type Handling

### Never Type (Bottom)

`Never` is the bottom type — it has no values and unifies with any type:

```rust
(Tag::Never, _) | (_, Tag::Never) => {},  // Always succeeds
```

This enables diverging expressions in any type context:

```ori
let x: int = if false then panic(msg: "fail") else 42
let y: str = if true then "hello" else todo()
```

`panic(msg:)`, `todo()`, `unreachable()`, `break`, `continue`, and infinite `loop` all produce `Never`. The semantic justification: a function that never returns can be said to return *any* type, because the return type is never observed.

### Error Type (Error Recovery)

`Error` is a sentinel for type checking failures. Like `Never`, it unifies with everything:

```rust
(Tag::Error, _) | (_, Tag::Error) => {},  // Suppress secondary errors
```

But unlike `Never`, `Error` is not a legitimate language type — it indicates that a type checking failure occurred earlier. When the type checker can't determine a type (unresolved name, malformed expression), it assigns `Error` to the expression and continues. Without this, a single type error would cascade into dozens of downstream "mismatched types" errors.

The distinction matters for diagnostics: `Never` is reported as `Never` in error messages (it's a real type), while `Error` is never shown to users (it's an internal marker).

## Unification Examples

### Identical Types

```
unify(int, int) = Ok           (identical Idx, fast path)
unify(int, str) = Err(Mismatch)
```

### Variable Binding

```
unify(T0, int)
  → occurs check: int has no vars → pass
  → set var_states[T0] = Link { target: Idx::INT }
  → Ok

unify(T0, T1)
  → occurs check: T1 is not T0 → pass
  → set var_states[T0] = Link { target: T1_idx }
  → Ok
```

### Compound Types

```
unify([T0], [int])
  → tags match: List = List
  → unify(T0, int) → Link T0 to int
  → Ok

unify((int, T0), (int, str))
  → tags match: Tuple = Tuple, same length
  → unify(int, int) → Ok (identical Idx)
  → unify(T0, str) → Link T0 to str
  → Ok
```

### Functions

```
unify((T0) -> T0, (int) -> int)
  → tags match: Function = Function, same arity
  → unify(T0, int) → Link T0 to int
  → unify(T0, int) → resolve T0 = int, identical → Ok
  → Ok
```

### Failure Cases

```
unify((int, int), (int,))         → Err(TupleLengthMismatch)
unify([int], {str: int})          → Err(Mismatch) — List vs Map
unify(T0, [T0])                   → Err(InfiniteType) via occurs check
unify(Rigid("T"), int)            → Err(RigidMismatch)
```

## Prior Art

### Robinson (1965) — First-Order Unification

J.A. Robinson's ["A Machine-Oriented Logic Based on the Resolution Principle"](https://dl.acm.org/doi/10.1145/321250.321253) introduced unification as the core operation for automated theorem proving. His algorithm operates on first-order terms (trees of function symbols and variables) and finds the most general unifier (MGU) — the most general substitution that makes two terms equal. Every type unification algorithm since then is a specialization of Robinson's algorithm to the domain of types.

### Tarjan (1975) — Union-Find

Robert Tarjan's union-find data structure (also called disjoint-set forest) provides near-constant-time operations for merging equivalence classes and finding representatives. Ori's link-based unification is a direct application of union-find to type variables: each variable is a node, unification merges two nodes, and path compression during find operations keeps chains short. The O(α(n)) amortized bound comes from Tarjan's analysis.

### Martelli and Montanari (1982)

["An Efficient Unification Algorithm"](https://dl.acm.org/doi/10.1145/357162.357169) by Martelli and Montanari improved Robinson's algorithm with nearly-linear time complexity. Their approach decomposes the unification problem into a set of equations and solves them by repeated simplification. While Ori doesn't use their specific algorithm, the decomposition approach (matching tags, then recursively unifying children) follows the same structure.

### Oleg Kiselyov — Rank-Based Generalization

[Oleg Kiselyov's work](http://okmij.org/ftp/ML/generalization.html) on efficient generalization describes the rank-based approach that Ori implements. The key insight is that variable levels (ranks) enable generalization without explicitly collecting free variables — a variable at rank N can be generalized when exiting scope N, determined by a simple comparison rather than a set traversal.

### OCaml and GHC — Production Union-Find

[OCaml](https://github.com/ocaml/ocaml)'s type checker uses mutable cells for type variables with union-find-style linking, essentially the same approach as Ori. [GHC](https://gitlab.haskell.org/ghc/ghc) uses a more complex variant with "meta-variables" that supports its richer type system (higher-rank types, type families, impredicative polymorphism). Both demonstrate that union-find-based unification scales to large, real-world codebases.

## Design Tradeoffs

**Union-find vs. substitution maps.** Union-find is faster (O(α(n)) vs O(1) with hash table constant factors) and uses less memory (no growing map), but requires mutable state in the type variable storage. This means the pool's `var_states` vector is mutable during inference, unlike the append-only type arrays. The mutability is contained within `UnifyEngine` (which borrows the pool mutably), preventing accidental modification from other code.

**Occurs check cost.** The occurs check prevents infinite types but adds a traversal to every variable binding. Ori mitigates this with flag-gated checking (O(1) for monomorphic types), but for polymorphic types with many nested variables, the traversal is still O(n). Some implementations skip the occurs check entirely for performance (infinite types are rare in practice), but this can produce confusing errors when they do occur. Ori keeps the check for correctness.

**Immediate error accumulation vs. best-error selection.** Ori accumulates all unification errors during inference. A constraint-based system could collect all constraints, then choose the "best" error to report (e.g., the one with the shortest explanation or the most specific source location). Ori's approach is simpler but may report errors in traversal order rather than in the order most helpful to the user.

**Rank-based generalization vs. level-based.** Ori uses Kiselyov's rank approach (explicit scope depth counter). An alternative is the level-based approach used by some OCaml implementations, where each variable carries a mutable level that is adjusted during unification. The rank approach is slightly simpler (no level adjustment during unification) but requires entering/exiting scopes explicitly.
