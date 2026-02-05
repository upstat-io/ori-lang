---
section: "03"
title: Unification Engine
status: complete
goal: Link-based unification with path compression for O(α(n)) complexity
sections:
  - id: "03.1"
    title: VarState Enum
    status: complete
  - id: "03.2"
    title: UnifyEngine Structure
    status: complete
  - id: "03.3"
    title: Resolution with Path Compression
    status: complete
  - id: "03.4"
    title: Core Unification Algorithm
    status: complete
  - id: "03.5"
    title: Flag-Gated Occurs Check
    status: complete
  - id: "03.6"
    title: Structural Unification
    status: complete
---

# Section 03: Unification Engine

**Status:** ✅ Complete (2026-02-04)
**Goal:** Link-based unification with O(α(n)) complexity via path compression
**Source:** Gleam (`type_/environment.rs`), Elm (`Type/Unify.hs`)

---

## Background

### Current Problems

1. **Substitution maps** — Unification creates maps that must be applied (copying)
2. **No path compression** — Long substitution chains
3. **Inefficient occurs check** — Traverses entire type every time
4. **No rank system** — Generalization is ad-hoc

### Solution from Gleam/Elm

1. **Link-based union-find** — Unification just sets a pointer
2. **Path compression** — O(α(n)) amortized resolution
3. **Flag-gated occurs check** — Skip when HAS_VAR is false
4. **Rank tracking** — Correct let-polymorphism (Section 04)

---

## 03.1 VarState Enum

**Goal:** Define states for type variables

### Design

```rust
/// State of a type variable.
#[derive(Clone, Debug)]
pub enum VarState {
    /// Unbound variable - waiting to be unified.
    Unbound {
        id: u32,
        rank: Rank,
        name: Option<Name>,
    },

    /// Linked to another type - follow the link.
    Link {
        target: Idx,
    },

    /// Rigid variable from annotation - cannot unify with concrete types.
    Rigid {
        name: Name,
    },

    /// Generalized variable - must be instantiated before use.
    Generalized {
        id: u32,
        name: Option<Name>,
    },
}
```

### Tasks

- [ ] Create `ori_types/src/unify/var_state.rs`
- [ ] Define `VarState` enum with all variants
- [ ] Add `VarState::is_unbound()`, `is_link()`, etc. helpers
- [ ] Add tests for state transitions

---

## 03.2 UnifyEngine Structure

**Goal:** Define the unification engine

### Design

```rust
/// The unification engine handles type variable resolution and unification.
pub struct UnifyEngine<'pool> {
    pool: &'pool mut Pool,
    current_rank: Rank,
    errors: Vec<UnifyError>,
}

impl<'pool> UnifyEngine<'pool> {
    pub fn new(pool: &'pool mut Pool) -> Self {
        Self {
            pool,
            current_rank: Rank::FIRST,
            errors: Vec::new(),
        }
    }

    /// Create a fresh unbound type variable at current rank.
    pub fn fresh_var(&mut self) -> Idx {
        self.pool.fresh_var(self.current_rank)
    }

    /// Create a fresh variable with a name (for better error messages).
    pub fn fresh_named_var(&mut self, name: Name) -> Idx {
        self.pool.fresh_named_var(self.current_rank, name)
    }

    /// Take accumulated errors.
    pub fn take_errors(&mut self) -> Vec<UnifyError> {
        std::mem::take(&mut self.errors)
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/mod.rs`
- [ ] Define `UnifyEngine` struct
- [ ] Implement constructor and basic methods
- [ ] Add error accumulation

---

## 03.3 Resolution with Path Compression

**Goal:** Follow links with O(α(n)) amortized complexity

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Resolve a type by following links.
    /// Implements path compression for O(α(n)) amortized complexity.
    pub fn resolve(&mut self, idx: Idx) -> Idx {
        // Fast path: not a variable
        if self.pool.tag(idx) != Tag::Var {
            return idx;
        }

        let var_id = self.pool.data(idx);
        let state = &self.pool.var_states[var_id as usize];

        match state {
            VarState::Link { target } => {
                let target = *target;
                // Recursively resolve the target
                let resolved = self.resolve(target);

                // Path compression: point directly to final target
                if resolved != target {
                    self.pool.var_states[var_id as usize] =
                        VarState::Link { target: resolved };
                }

                resolved
            }
            _ => idx,
        }
    }

    /// Resolve without mutation (for read-only queries).
    pub fn resolve_readonly(&self, idx: Idx) -> Idx {
        // Similar but without path compression
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/resolve.rs`
- [ ] Implement `resolve()` with path compression
- [ ] Implement `resolve_readonly()` for queries
- [ ] Add benchmarks comparing with/without path compression

---

## 03.4 Core Unification Algorithm

**Goal:** Implement the main unification logic

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Unify two types, making them equivalent.
    pub fn unify(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        // Fast path: identical indices
        if a == b {
            return Ok(());
        }

        // Resolve both sides
        let a = self.resolve(a);
        let b = self.resolve(b);

        // After resolution, check again
        if a == b {
            return Ok(());
        }

        // Check flags for early exits
        let a_flags = self.pool.flags(a);
        let b_flags = self.pool.flags(b);

        // Error propagates
        if a_flags.contains(TypeFlags::HAS_ERROR) ||
           b_flags.contains(TypeFlags::HAS_ERROR) {
            return Ok(());
        }

        // Never propagates (bottom type)
        if self.pool.tag(a) == Tag::Never ||
           self.pool.tag(b) == Tag::Never {
            return Ok(());
        }

        // Dispatch based on types
        match (self.pool.tag(a), self.pool.tag(b)) {
            (Tag::Var, _) => self.unify_var_with(a, b),
            (_, Tag::Var) => self.unify_var_with(b, a),

            (Tag::RigidVar, Tag::RigidVar) => self.unify_rigid_rigid(a, b),
            (Tag::RigidVar, _) => Err(UnifyError::RigidMismatch { ... }),
            (_, Tag::RigidVar) => Err(UnifyError::RigidMismatch { ... }),

            _ => self.unify_structural(a, b),
        }
    }

    fn unify_var_with(&mut self, var_idx: Idx, other: Idx) -> Result<(), UnifyError> {
        let var_id = self.pool.data(var_idx);

        // Occurs check
        if self.occurs(var_id, other) {
            return Err(UnifyError::InfiniteType { var: var_id, ty: other });
        }

        // Get variable state
        match &self.pool.var_states[var_id as usize] {
            VarState::Unbound { rank, .. } => {
                let rank = *rank;
                // Update ranks of other type's variables
                self.update_ranks(other, rank);
                // Set link
                self.pool.var_states[var_id as usize] =
                    VarState::Link { target: other };
                Ok(())
            }
            VarState::Link { target } => {
                self.unify(*target, other)
            }
            VarState::Rigid { name } => {
                Err(UnifyError::RigidMismatch {
                    rigid_name: *name,
                    concrete: other,
                })
            }
            VarState::Generalized { .. } => {
                panic!("Unify generalized var - should be instantiated first")
            }
        }
    }
}
```

### Tasks

- [ ] Implement `unify()` main entry point
- [ ] Implement `unify_var_with()` for variable unification
- [ ] Implement `unify_rigid_rigid()` for rigid variable comparison
- [ ] Add comprehensive tests for all unification cases

---

## 03.5 Flag-Gated Occurs Check

**Goal:** Skip occurs check when type has no variables

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Check if variable `var` occurs in type `ty`.
    fn occurs(&self, var_id: u32, ty: Idx) -> bool {
        // Fast path: check flags
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return false;
        }

        self.occurs_inner(var_id, ty)
    }

    fn occurs_inner(&self, var_id: u32, ty: Idx) -> bool {
        let tag = self.pool.tag(ty);

        match tag {
            Tag::Var => {
                let other_id = self.pool.data(ty);
                if other_id == var_id {
                    return true;
                }
                // Follow link if present
                if let VarState::Link { target } =
                    &self.pool.var_states[other_id as usize]
                {
                    return self.occurs_inner(var_id, *target);
                }
                false
            }

            Tag::List | Tag::Option | Tag::Set => {
                let child = Idx(self.pool.data(ty));
                self.occurs_inner(var_id, child)
            }

            Tag::Map | Tag::Result => {
                let (child1, child2) = self.pool.get_two_children(ty);
                self.occurs_inner(var_id, child1) ||
                self.occurs_inner(var_id, child2)
            }

            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);
                params.iter().any(|&p| self.occurs_inner(var_id, p)) ||
                self.occurs_inner(var_id, ret)
            }

            // ... other compound types

            _ => false,
        }
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/occurs.rs`
- [ ] Implement flag-gated `occurs()` check
- [ ] Implement `occurs_inner()` for all type kinds
- [ ] Add tests for infinite type detection
- [ ] Benchmark flag gating effectiveness

---

## 03.6 Structural Unification

**Goal:** Unify concrete types structurally

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    fn unify_structural(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        let tag_a = self.pool.tag(a);
        let tag_b = self.pool.tag(b);

        // Tags must match
        if tag_a != tag_b {
            return Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context: UnifyContext::TopLevel,
            });
        }

        match tag_a {
            // Simple containers
            Tag::List | Tag::Option | Tag::Set => {
                let child_a = Idx(self.pool.data(a));
                let child_b = Idx(self.pool.data(b));
                self.unify(child_a, child_b)
            }

            // Two-child containers
            Tag::Map => {
                let (key_a, val_a) = self.pool.get_map_types(a);
                let (key_b, val_b) = self.pool.get_map_types(b);
                self.unify(key_a, key_b)?;
                self.unify(val_a, val_b)
            }

            Tag::Result => {
                let (ok_a, err_a) = self.pool.get_result_types(a);
                let (ok_b, err_b) = self.pool.get_result_types(b);
                self.unify(ok_a, ok_b)?;
                self.unify(err_a, err_b)
            }

            // Functions
            Tag::Function => {
                let params_a = self.pool.function_params(a);
                let params_b = self.pool.function_params(b);
                let ret_a = self.pool.function_return(a);
                let ret_b = self.pool.function_return(b);

                if params_a.len() != params_b.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: params_a.len(),
                        found: params_b.len(),
                        kind: ArityKind::Function,
                    });
                }

                for (pa, pb) in params_a.iter().zip(params_b.iter()) {
                    self.unify(*pa, *pb)?;
                }
                self.unify(ret_a, ret_b)
            }

            // Tuples
            Tag::Tuple => {
                let elems_a = self.pool.tuple_elems(a);
                let elems_b = self.pool.tuple_elems(b);

                if elems_a.len() != elems_b.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: elems_a.len(),
                        found: elems_b.len(),
                        kind: ArityKind::Tuple,
                    });
                }

                for (ea, eb) in elems_a.iter().zip(elems_b.iter()) {
                    self.unify(*ea, *eb)?;
                }
                Ok(())
            }

            // Primitives
            Tag::Int | Tag::Float | Tag::Bool | Tag::Str |
            Tag::Char | Tag::Byte | Tag::Unit | Tag::Never => Ok(()),

            _ => Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context: UnifyContext::TopLevel,
            }),
        }
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/structural.rs`
- [ ] Implement structural unification for all type kinds
- [ ] Handle arity mismatches with specific errors
- [ ] Add tests for each structural case

---

## 03.7 UnifyError Type

**Goal:** Define comprehensive unification errors

### Design

```rust
#[derive(Clone, Debug)]
pub enum UnifyError {
    Mismatch {
        expected: Idx,
        found: Idx,
        context: UnifyContext,
    },

    InfiniteType {
        var: u32,
        ty: Idx,
    },

    RigidMismatch {
        rigid_name: Name,
        concrete: Idx,
    },

    RigidRigidMismatch {
        rigid1: Name,
        rigid2: Name,
    },

    ArityMismatch {
        expected: usize,
        found: usize,
        kind: ArityKind,
    },
}

#[derive(Copy, Clone, Debug)]
pub enum ArityKind {
    Function,
    Tuple,
    TypeArgs,
}

#[derive(Copy, Clone, Debug)]
pub enum UnifyContext {
    TopLevel,
    FunctionParam { index: usize },
    FunctionReturn,
    ListElement,
    MapKey,
    MapValue,
    TupleElement { index: usize },
    // ... more contexts
}
```

### Tasks

- [x] Create `ori_types/src/unify/error.rs` ✅
- [x] Define `UnifyError` enum with all variants ✅
- [x] Define `ArityKind` and `UnifyContext` enums ✅
- [x] Implement `Display` for user-friendly messages ✅

---

## 03.8 Completion Checklist

- [x] `VarState` enum complete with all variants ✅ (in pool/mod.rs, reused)
- [x] `UnifyEngine` struct with all core methods ✅
- [x] `resolve()` with path compression working ✅
- [x] `unify()` handling all cases correctly ✅
- [x] `occurs()` flag-gated and correct ✅
- [x] Structural unification for all type kinds ✅
- [x] `UnifyError` with comprehensive error types ✅
- [x] All tests passing ✅ (79 ori_types tests, 7187 total)
- [ ] Benchmarks showing O(α(n)) behavior — deferred to optimization phase

**Section 03 Status:** ✅ Complete (2026-02-04)

**Exit Criteria:** Unification is link-based with path compression. No substitution maps are created. Resolution and unification are O(α(n)) amortized.
