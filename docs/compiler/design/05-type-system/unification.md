---
title: "Unification"
description: "Ori Compiler Design — Unification"
order: 504
section: "Type System"
---

# Unification

Unification finds whether two types can be made equal by binding type variables. The `UnifyEngine` implements link-based union-find with path compression, providing near-constant-time unification.

## Location

```
compiler/ori_types/src/unify/
├── mod.rs    # UnifyEngine — core unification
├── rank.rs   # Rank system for let-polymorphism
└── error.rs  # UnifyError, UnifyContext
```

## UnifyEngine

```rust
pub struct UnifyEngine<'pool> {
    pool: &'pool mut Pool,
    current_rank: Rank,
    errors: Vec<UnifyError>,
}
```

The engine borrows the pool mutably to update `VarState` links during unification. Errors are accumulated rather than returned immediately, enabling continued type checking after failures.

## Link-Based Union-Find

Unlike the substitution-map approach used in textbook HM implementations, the unification engine uses **direct linking** through the pool's `VarState`:

```rust
pub enum VarState {
    Unbound { id: u32, rank: Rank, name: Option<Name> },
    Link { target: Idx },     // Points to unified type
    Rigid { name: Name },     // From annotation, cannot unify
    Generalized { id: u32, name: Option<Name> },
}
```

When variable `T0` unifies with `int`, the engine sets `var_states[T0] = Link { target: Idx::INT }`. No separate substitution map is needed.

### Path Compression

During `resolve()`, intermediate links are updated to point directly to the final target, achieving O(α(n)) amortized complexity (where α is the inverse Ackermann function):

```rust
pub fn resolve(&mut self, idx: Idx) -> Idx {
    // If idx is a type variable, follow links
    if pool.tag(idx) == Tag::Var {
        let var_id = pool.data(idx);
        match pool.var_state(var_id) {
            VarState::Link { target } => {
                let resolved = self.resolve(target);
                // Path compression: update link to point directly to final
                pool.set_var_state(var_id, VarState::Link { target: resolved });
                resolved
            }
            _ => idx,  // Unbound, rigid, or generalized
        }
    } else {
        idx  // Not a variable, return as-is
    }
}
```

### Core Unification Algorithm

```rust
pub fn unify(&mut self, left: Idx, right: Idx) {
    // O(1) fast path: identical indices
    if left == right { return; }

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

        // Error/Never unify with anything (error recovery)
        (Tag::Error, _) | (_, Tag::Error) => {},
        (Tag::Never, _) | (_, Tag::Never) => {},

        // Structural unification for matching tags
        (Tag::List, Tag::List) => {
            self.unify(self.pool.list_elem(left), self.pool.list_elem(right));
        }

        (Tag::Function, Tag::Function) => {
            // Check parameter count, then unify each param + return
        }

        // ... other compound types

        // Mismatch
        _ => self.push_error(UnifyError::Mismatch { expected: left, got: right }),
    }
}
```

## Flag-Gated Occurs Check

The occurs check prevents infinite types (e.g., `T = [T]`). The pool's `TypeFlags` enable an important optimization:

```rust
fn occurs_check(&self, var_id: u32, in_type: Idx) -> bool {
    // O(1) fast path: if the type has no variables, no need to traverse
    if !self.pool.flags(in_type).contains(TypeFlags::HAS_VAR) {
        return false;
    }
    // Only traverse if HAS_VAR flag is set
    self.occurs_check_inner(var_id, in_type)
}
```

Since `TypeFlags::HAS_VAR` propagates during construction, the occurs check skips the entire traversal for monomorphic types — which is the common case.

## Rank System

The rank system controls let-polymorphism by tracking the scope depth of type variables.

### Rank Type

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

### Rank Semantics

Each type variable is created at a specific rank corresponding to its scope depth. When the type checker enters a `let` binding, the rank increases; when it exits, variables at the current rank can be generalized:

```
Rank 2 (module level):
  let id = x -> x         ← infer at rank 3
                           ← generalize at rank 3: forall T. T -> T
  let a = id(42)           ← instantiate with fresh vars at rank 2
  let b = id("hello")      ← instantiate with fresh vars at rank 2
```

A variable at rank N can be generalized when exiting rank N:

```rust
impl Rank {
    pub fn can_generalize_at(&self, gen_rank: Rank) -> bool {
        self.0 >= gen_rank.0
    }
    pub fn is_generalized(&self) -> bool { self.0 == Self::TOP.0 }
}
```

### Generalization

When a `let`-bound value's type is complete, the engine generalizes unbound variables at the current rank into a type scheme:

```rust
pub fn generalize(&mut self, ty: Idx, rank: Rank) -> Idx {
    // Walk the type, converting Unbound vars at rank >= current to Generalized
    // Returns a Scheme if any variables were generalized
}
```

### Instantiation

When a polymorphic value is used, its scheme is instantiated with fresh variables:

```rust
pub fn instantiate(&mut self, scheme: Idx) -> Idx {
    // For each generalized variable in the scheme, create a fresh var
    // Substitute into the body
}
```

## Special Type Handling

### Never Type (Bottom)

`Never` is the bottom type — an uninhabited type representing diverging computations. It unifies with any type:

```rust
(Tag::Never, _) | (_, Tag::Never) => {},  // Always succeeds
```

This enables diverging expressions in any context:

```ori
let x: int = if false then panic(msg: "fail") else 42
let y: str = if true then "hello" else todo()
```

Expressions producing `Never`: `panic(msg:)`, `todo()`, `unreachable()`, `break`, `continue`, infinite `loop(...)`.

### Error Type

`Error` is a sentinel for error recovery. It unifies with anything to prevent cascading errors:

```rust
(Tag::Error, _) | (_, Tag::Error) => {},  // Suppress secondary errors
```

Unlike `Never` (a legitimate language type), `Error` indicates a type checking failure occurred earlier. Without this, a single type error would cascade into many "mismatched types" errors downstream.

## Unification Examples

### Simple

```
unify(int, int) = Ok           (identical Idx)
unify(int, str) = Err(Mismatch)
```

### Variables

```
unify(T0, int)
  → set var_states[T0] = Link { target: Idx::INT }
  → Ok

unify(T0, T1)
  → set var_states[T0] = Link { target: T1_idx }
  → Ok
```

### Compound Types

```
unify([T0], [int])
  → unify(T0, int) → Link T0 to int
  → Ok

unify((int, T0), (int, str))
  → unify(int, int) → Ok
  → unify(T0, str) → Link T0 to str
  → Ok
```

### Functions

```
unify((T0) -> T0, (int) -> int)
  → unify(T0, int) → Link T0 to int
  → unify(T0, int) → resolve T0 = int, Ok
  → Ok
```

### Failure Cases

```
unify((int, int), (int,))    → Err(TupleLengthMismatch)
unify([int], {str: int})     → Err(Mismatch)
unify(T0, [T0])              → Err(InfiniteType) via occurs check
```

## Immediate Unification

Ori uses **immediate unification** — constraints are solved as they are generated during AST traversal, not collected and solved later. This simplifies the implementation while fully supporting Hindley-Milner inference:

- Simpler implementation (no constraint storage or solver)
- Errors reported at the point of occurrence
- Substitutions available immediately for subsequent inference
- Rank-based let-polymorphism handles generalization correctly
