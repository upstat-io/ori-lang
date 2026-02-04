---
section: "04"
title: Rank-Based Generalization
status: not-started
goal: Correct let-polymorphism with rank tracking
sections:
  - id: "04.1"
    title: Rank Type
    status: not-started
  - id: "04.2"
    title: Scope Management
    status: not-started
  - id: "04.3"
    title: Rank Updates During Unification
    status: not-started
  - id: "04.4"
    title: Generalization
    status: not-started
  - id: "04.5"
    title: Instantiation
    status: not-started
  - id: "04.6"
    title: Type Scheme Storage
    status: not-started
---

# Section 04: Rank-Based Generalization

**Status:** Not Started
**Goal:** Correct let-polymorphism using Elm/Roc-style rank tracking
**Source:** Elm (`Type/Solve.hs`), Roc (`solve/src/solve.rs`)

---

## Background

### The Let-Polymorphism Problem

```ori
let id = |x| x in
let a = id(1)       // id : int -> int ?
let b = id("hello") // id : str -> str ?
```

Without proper generalization, `id` gets a monomorphic type after first use. Rank-based generalization ensures polymorphism is preserved correctly.

### Rank System

- **Rank 0**: Universally quantified (generalizable)
- **Rank N>0**: Created in nested scope at depth N
- **Rule**: Variables at rank N can only be generalized when exiting rank N

---

## 04.1 Rank Type

**Goal:** Define the rank tracking type

### Design

```rust
/// Rank tracks the depth of let-bindings for correct generalization.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Rank(u16);

impl Rank {
    /// Top-level rank (can always be generalized).
    pub const TOP: Self = Self(0);

    /// Import rank (always generalizable, from other modules).
    pub const IMPORT: Self = Self(1);

    /// First user rank (top-level definitions).
    pub const FIRST: Self = Self(2);

    /// Maximum rank (prevents overflow).
    pub const MAX: Self = Self(u16::MAX - 1);

    #[inline]
    pub fn next(self) -> Self {
        debug_assert!(self.0 < Self::MAX.0);
        Self(self.0 + 1)
    }

    #[inline]
    pub fn prev(self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    #[inline]
    pub fn can_generalize_at(self, generalization_rank: Rank) -> bool {
        self >= generalization_rank
    }

    #[inline]
    pub fn is_generalizable(self) -> bool {
        self == Self::TOP
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/rank.rs`
- [ ] Define `Rank` with constants and methods
- [ ] Add overflow protection
- [ ] Add tests for rank arithmetic

---

## 04.2 Scope Management

**Goal:** Track scope entry/exit for rank management

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Enter a new let scope (increment rank).
    pub fn enter_scope(&mut self) {
        self.current_rank = self.current_rank.next();
    }

    /// Exit a let scope (decrement rank).
    /// Returns variables eligible for generalization.
    pub fn exit_scope(&mut self) -> Vec<u32> {
        let generalizable = self.collect_generalizable_vars();
        self.current_rank = self.current_rank.prev();
        generalizable
    }

    /// Get current rank.
    pub fn current_rank(&self) -> Rank {
        self.current_rank
    }

    fn collect_generalizable_vars(&self) -> Vec<u32> {
        self.pool.var_states
            .iter()
            .enumerate()
            .filter_map(|(id, state)| {
                if let VarState::Unbound { rank, .. } = state {
                    if rank.can_generalize_at(self.current_rank) {
                        return Some(id as u32);
                    }
                }
                None
            })
            .collect()
    }
}
```

### Tasks

- [ ] Implement `enter_scope()`
- [ ] Implement `exit_scope()` with var collection
- [ ] Add scope depth tracking for debugging
- [ ] Add tests for nested scopes

---

## 04.3 Rank Updates During Unification

**Goal:** Maintain rank invariants during unification

### Design

When unifying a variable at rank N with a type, all variables in that type must have rank ≤ N. This prevents inner variables from escaping.

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Update ranks of all variables in `ty` to be at most `max_rank`.
    fn update_ranks(&mut self, ty: Idx, max_rank: Rank) {
        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return;
        }

        self.update_ranks_inner(ty, max_rank);
    }

    fn update_ranks_inner(&mut self, ty: Idx, max_rank: Rank) {
        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty) as usize;
                if let VarState::Unbound { rank, id, name } =
                    &self.pool.var_states[var_id]
                {
                    if *rank > max_rank {
                        self.pool.var_states[var_id] = VarState::Unbound {
                            id: *id,
                            rank: max_rank,
                            name: *name,
                        };
                    }
                } else if let VarState::Link { target } =
                    &self.pool.var_states[var_id]
                {
                    self.update_ranks_inner(*target, max_rank);
                }
            }

            Tag::List | Tag::Option | Tag::Set => {
                let child = Idx(self.pool.data(ty));
                self.update_ranks_inner(child, max_rank);
            }

            Tag::Function => {
                for &p in self.pool.function_params(ty) {
                    self.update_ranks_inner(p, max_rank);
                }
                self.update_ranks_inner(self.pool.function_return(ty), max_rank);
            }

            // ... other compound types

            _ => {}
        }
    }
}
```

### Tasks

- [ ] Implement `update_ranks()` with flag gating
- [ ] Implement `update_ranks_inner()` for all type kinds
- [ ] Integrate into `unify_var_with()`
- [ ] Add tests for rank propagation

---

## 04.4 Generalization

**Goal:** Convert unbound variables to generalized variables

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Generalize a type at the current rank.
    /// Returns a type scheme if any variables were generalized.
    pub fn generalize(&mut self, ty: Idx) -> Idx {
        let vars = self.collect_free_vars_at_rank(ty, self.current_rank);

        if vars.is_empty() {
            return ty; // Monomorphic
        }

        // Mark variables as generalized
        for &var_id in &vars {
            if let VarState::Unbound { id, name, .. } =
                &self.pool.var_states[var_id as usize]
            {
                self.pool.var_states[var_id as usize] = VarState::Generalized {
                    id: *id,
                    name: *name,
                };
            }
        }

        // Create type scheme
        self.pool.scheme(&vars, ty)
    }

    fn collect_free_vars_at_rank(&self, ty: Idx, min_rank: Rank) -> Vec<u32> {
        let mut vars = Vec::new();
        self.collect_free_vars_inner(ty, min_rank, &mut vars);
        vars.sort_unstable();
        vars.dedup();
        vars
    }

    fn collect_free_vars_inner(
        &self,
        ty: Idx,
        min_rank: Rank,
        vars: &mut Vec<u32>,
    ) {
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);
                match &self.pool.var_states[var_id as usize] {
                    VarState::Unbound { rank, .. } if *rank >= min_rank => {
                        vars.push(var_id);
                    }
                    VarState::Link { target } => {
                        self.collect_free_vars_inner(*target, min_rank, vars);
                    }
                    _ => {}
                }
            }
            // Recurse into compound types...
            _ => {}
        }
    }
}
```

### Tasks

- [ ] Create `ori_types/src/unify/generalize.rs`
- [ ] Implement `generalize()` with scheme creation
- [ ] Implement `collect_free_vars_at_rank()`
- [ ] Add tests for polymorphic generalization

---

## 04.5 Instantiation

**Goal:** Create fresh variables from generalized schemes

### Design

```rust
impl<'pool> UnifyEngine<'pool> {
    /// Instantiate a type scheme with fresh variables.
    pub fn instantiate(&mut self, scheme_idx: Idx) -> Idx {
        if self.pool.tag(scheme_idx) != Tag::Scheme {
            return scheme_idx; // Not a scheme
        }

        let vars = self.pool.scheme_vars(scheme_idx);
        let body = self.pool.scheme_body(scheme_idx);

        if vars.is_empty() {
            return body; // Monomorphic
        }

        // Create fresh variables for each quantified variable
        let mut subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for &var_id in vars {
            let fresh = self.fresh_var();
            subst.insert(var_id, fresh);
        }

        // Substitute in the body
        self.substitute(body, &subst)
    }

    /// Substitute variables according to the given mapping.
    fn substitute(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return ty;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);
                if let Some(&replacement) = subst.get(&var_id) {
                    replacement
                } else if let VarState::Link { target } =
                    &self.pool.var_states[var_id as usize]
                {
                    self.substitute(*target, subst)
                } else {
                    ty
                }
            }

            Tag::List => {
                let child = Idx(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child { ty } else { self.pool.list(new_child) }
            }

            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);

                let new_params: Vec<_> = params.iter()
                    .map(|&p| self.substitute(p, subst))
                    .collect();
                let new_ret = self.substitute(ret, subst);

                if new_params.as_slice() == params && new_ret == ret {
                    ty
                } else {
                    self.pool.function(&new_params, new_ret)
                }
            }

            // ... other compound types

            _ => ty,
        }
    }
}
```

### Tasks

- [ ] Implement `instantiate()` for scheme instantiation
- [ ] Implement `substitute()` for all type kinds
- [ ] Optimize to avoid allocation when no changes
- [ ] Add tests for polymorphic instantiation

---

## 04.6 Type Scheme Storage

**Goal:** Store type schemes in the pool

### Design

```rust
impl Pool {
    /// Create a type scheme.
    pub fn scheme(&mut self, vars: &[u32], body: Idx) -> Idx {
        if vars.is_empty() {
            return body; // Monomorphic
        }

        let mut extra = Vec::with_capacity(vars.len() + 2);
        extra.push(vars.len() as u32);
        for &v in vars {
            extra.push(v);
        }
        extra.push(body.0);

        self.intern_complex(Tag::Scheme, &extra)
    }

    /// Get scheme quantified variables.
    pub fn scheme_vars(&self, idx: Idx) -> &[u32] {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        &self.extra[extra_idx + 1..extra_idx + 1 + count]
    }

    /// Get scheme body type.
    pub fn scheme_body(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx(self.extra[extra_idx + 1 + count])
    }
}
```

### Tasks

- [ ] Implement `Pool::scheme()` construction
- [ ] Implement `Pool::scheme_vars()` accessor
- [ ] Implement `Pool::scheme_body()` accessor
- [ ] Add tests for scheme storage and retrieval

---

## 04.7 Completion Checklist

- [ ] `Rank` type complete with all methods
- [ ] Scope entry/exit working correctly
- [ ] Rank updates during unification correct
- [ ] `generalize()` producing correct schemes
- [ ] `instantiate()` creating fresh variables
- [ ] Type schemes stored and retrieved correctly
- [ ] Let-polymorphism working (id can be both int->int and str->str)
- [ ] All tests passing

**Exit Criteria:** The identity function `|x| x` can be used with different types in the same scope. Generalization correctly quantifies variables at scope exit. Instantiation creates fresh variables for each use.
