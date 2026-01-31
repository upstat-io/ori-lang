---
title: "Unification"
description: "Ori Compiler Design — Unification"
order: 504
section: "Type System"
---

# Unification

Unification is the process of finding a substitution that makes two types equal. It's the core algorithm for type inference.

## Basic Algorithm

```rust
fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
    // Apply current substitution first
    let t1 = self.apply_subst(t1);
    let t2 = self.apply_subst(t2);

    match (&t1, &t2) {
        // Identical types unify trivially
        _ if t1 == t2 => Ok(()),

        // Type variable unifies with anything (with occurs check)
        (Type::TypeVar(id), ty) | (ty, Type::TypeVar(id)) => {
            self.bind_var(*id, ty)
        }

        // Compound types unify component-wise
        (Type::List(a), Type::List(b)) => self.unify(a, b),

        (Type::Option(a), Type::Option(b)) => self.unify(a, b),

        (Type::Result(ok1, err1), Type::Result(ok2, err2)) => {
            self.unify(ok1, ok2)?;
            self.unify(err1, err2)
        }

        (Type::Tuple(ts1), Type::Tuple(ts2)) => {
            if ts1.len() != ts2.len() {
                return Err(TypeError::TupleLengthMismatch);
            }
            for (a, b) in ts1.iter().zip(ts2.iter()) {
                self.unify(a, b)?;
            }
            Ok(())
        }

        (Type::Function { params: p1, ret: r1, .. },
         Type::Function { params: p2, ret: r2, .. }) => {
            if p1.len() != p2.len() {
                return Err(TypeError::ParamCountMismatch);
            }
            for (a, b) in p1.iter().zip(p2.iter()) {
                self.unify(a, b)?;
            }
            self.unify(r1, r2)
        }

        // Different types don't unify
        _ => Err(TypeError::Mismatch {
            expected: t1.clone(),
            found: t2.clone(),
        }),
    }
}
```

## Variable Binding

When binding a type variable, check for cycles:

```rust
fn bind_var(&mut self, var: TypeVarId, ty: &Type) -> Result<(), TypeError> {
    // Check if variable is already bound
    if let Some(existing) = self.substitution.get(&var) {
        return self.unify(existing, ty);
    }

    // Skip if binding to self
    if let Type::TypeVar(id) = ty {
        if *id == var {
            return Ok(());
        }
    }

    // Occurs check - prevent infinite types
    if self.occurs_in(var, ty) {
        return Err(TypeError::InfiniteType {
            var,
            ty: ty.clone(),
        });
    }

    // Add binding to substitution
    self.substitution.insert(var, ty.clone());
    Ok(())
}
```

## Occurs Check

The occurs check prevents creating infinite types:

```rust
// Would create: T0 = [T0] = [[T0]] = ...
let xs = [xs]  // Error: infinite type
```

```rust
fn occurs_in(&self, var: TypeVarId, ty: &Type) -> bool {
    match ty {
        Type::TypeVar(id) => {
            if *id == var {
                return true;
            }
            // Check through substitution
            if let Some(resolved) = self.substitution.get(id) {
                return self.occurs_in(var, resolved);
            }
            false
        }

        Type::List(elem) => self.occurs_in(var, elem),

        Type::Option(inner) => self.occurs_in(var, inner),

        Type::Result(ok, err) => {
            self.occurs_in(var, ok) || self.occurs_in(var, err)
        }

        Type::Tuple(elems) => elems.iter().any(|e| self.occurs_in(var, e)),

        Type::Function { params, ret, .. } => {
            params.iter().any(|p| self.occurs_in(var, p))
                || self.occurs_in(var, ret)
        }

        // Primitives don't contain type variables
        _ => false,
    }
}
```

## Substitution

A substitution maps type variables to types:

```rust
struct Substitution {
    map: HashMap<TypeVarId, Type>,
}

impl Substitution {
    fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::TypeVar(id) => {
                self.map.get(id)
                    .map(|t| self.apply(t))  // Apply recursively
                    .unwrap_or_else(|| ty.clone())
            }

            Type::List(elem) => Type::List(Box::new(self.apply(elem))),

            Type::Function { params, ret, caps } => Type::Function {
                params: params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(ret)),
                caps: caps.clone(),
            },

            // Other compound types...

            _ => ty.clone(),
        }
    }
}
```

## Special Type Handling

### Never Type (Bottom Type)

The `Never` type is the bottom type — an uninhabited type with no values. It represents computations that never complete normally (diverge). In unification, `Never` coerces to any type `T`:

```rust
// In unify_ids():
(TypeData::Never, _) | (_, TypeData::Never) => Ok(()),
```

This enables diverging expressions to appear in any context:

```ori
let x: int = if false then panic(msg: "fail") else 42  // Never coerces to int
let y: str = if true then "hello" else todo()          // Never coerces to str
```

**Rationale:** Since `Never` has no values, the coercion never actually executes — the expression diverges before producing a value. This is safe because unreachable code has no runtime behavior.

**Expressions producing Never:**
- `panic(msg:)` — halt with error message
- `todo()` / `todo(reason:)` — mark unfinished code
- `unreachable()` / `unreachable(reason:)` — mark impossible code paths
- `break` / `continue` (inside loops)
- Infinite `loop(...)` with no break

### Error Type

The `Error` type is a sentinel for error recovery during type checking:

```rust
// In unify_ids():
(TypeData::Error, _) | (_, TypeData::Error) => Ok(()),
```

Unlike `Never` (a legitimate language type), `Error` indicates a type checking failure. It unifies with anything to prevent cascading errors — one type error shouldn't cause dozens of "mismatched types" errors downstream.

## Unification Examples

### Simple Unification

```
unify(Int, Int) = Ok(())
unify(Int, String) = Err(Mismatch)
```

### Variable Unification

```
unify(T0, Int) = Ok(substitution[T0] = Int)
unify(T0, T1) = Ok(substitution[T0] = T1)
```

### Never Unification

```
unify(Never, Int) = Ok()      // Never coerces to Int
unify(String, Never) = Ok()   // Never coerces to String
unify(Never, [T0]) = Ok()     // Never coerces to any compound type
unify(Never, Never) = Ok()    // Never unifies with itself
```

### Compound Unification

```
unify([T0], [Int])
  = unify(T0, Int)
  = Ok(substitution[T0] = Int)

unify((Int, T0), (Int, String))
  = unify(Int, Int) = Ok
  = unify(T0, String) = Ok(substitution[T0] = String)
```

### Function Unification

```
unify((T0) -> T0, (Int) -> Int)
  = unify(T0, Int) = Ok
  = unify(T0, Int) = Ok (T0 already Int)
  = Ok(substitution[T0] = Int)
```

### Failure Cases

```
// Length mismatch
unify((Int, Int), (Int,)) = Err(TupleLengthMismatch)

// Type mismatch
unify([Int], {String: Int}) = Err(Mismatch)

// Occurs check failure
unify(T0, [T0]) = Err(InfiniteType)
```

## Constraint-Based Approach

Instead of unifying immediately, collect constraints:

```rust
struct Constraint {
    left: Type,
    right: Type,
    span: Span,
    context: String,
}

impl TypeChecker {
    fn add_constraint(&mut self, left: Type, right: Type, span: Span) {
        self.constraints.push(Constraint {
            left,
            right,
            span,
            context: self.current_context(),
        });
    }

    fn solve_constraints(&mut self) -> Result<(), Vec<TypeError>> {
        let mut errors = Vec::new();

        for constraint in &self.constraints {
            if let Err(e) = self.unify(&constraint.left, &constraint.right) {
                errors.push(e.with_span(constraint.span));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
```

Benefits of constraint collection:
- Better error messages (know context)
- Can report multiple errors
- Enables advanced inference features

## Union-Find Optimization

For efficiency, use union-find for type variables:

```rust
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);  // Path compression
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let px = self.find(x);
        let py = self.find(y);
        if px != py {
            // Union by rank
            if self.rank[px] < self.rank[py] {
                self.parent[px] = py;
            } else if self.rank[px] > self.rank[py] {
                self.parent[py] = px;
            } else {
                self.parent[py] = px;
                self.rank[px] += 1;
            }
        }
    }
}
```

This makes variable lookup nearly O(1) amortized.
