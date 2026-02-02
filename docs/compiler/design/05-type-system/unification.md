---
title: "Unification"
description: "Ori Compiler Design — Unification"
order: 504
section: "Type System"
---

# Unification

Unification is the process of finding a substitution that makes two types equal. It's the core algorithm for type inference.

## Location

```
compiler/ori_types/src/context.rs
```

## TypeId-Based Unification

The implementation uses `TypeId` internally for O(1) equality fast-paths:

```rust
impl InferenceContext {
    /// Public API: accepts Type references.
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
        let id1 = t1.to_type_id(&self.interner);
        let id2 = t2.to_type_id(&self.interner);
        self.unify_ids(id1, id2)
    }

    /// Internal: uses interned TypeIds for efficiency.
    pub fn unify_ids(&mut self, id1: TypeId, id2: TypeId) -> Result<(), TypeError> {
        // O(1) fast path: identical TypeIds always unify
        if id1 == id2 {
            return Ok(());
        }

        let id1 = self.resolve_id(id1);
        let id2 = self.resolve_id(id2);

        // Check again after resolution
        if id1 == id2 {
            return Ok(());
        }

        let data1 = self.interner.lookup(id1);
        let data2 = self.interner.lookup(id2);

        match (&data1, &data2) {
            // Type variable unifies with anything (with occurs check)
            (TypeData::Var(v), _) => {
                if self.occurs_id(*v, id2) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id2);
                Ok(())
            }
            (_, TypeData::Var(v)) => {
                if self.occurs_id(*v, id1) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id1);
                Ok(())
            }

            // Error/Never unify with anything (see Special Type Handling)
            (TypeData::Error | TypeData::Never, _) |
            (_, TypeData::Error | TypeData::Never) => Ok(()),

            // Compound types unify component-wise
            (TypeData::List(a), TypeData::List(b)) => self.unify_ids(*a, *b),

            (TypeData::Option(a), TypeData::Option(b)) => self.unify_ids(*a, *b),

            (TypeData::Result { ok: ok1, err: err1 },
             TypeData::Result { ok: ok2, err: err2 }) => {
                self.unify_ids(*ok1, *ok2)?;
                self.unify_ids(*err1, *err2)
            }

            (TypeData::Function { params: p1, ret: r1 },
             TypeData::Function { params: p2, ret: r2 }) => {
                if p1.len() != p2.len() {
                    return Err(TypeError::ArgCountMismatch {
                        expected: p1.len(),
                        found: p2.len(),
                    });
                }
                for (a, b) in p1.iter().zip(p2.iter()) {
                    self.unify_ids(*a, *b)?;
                }
                self.unify_ids(*r1, *r2)
            }

            // Different types don't unify
            _ => Err(TypeError::Mismatch {
                expected: self.interner.to_type(id1),
                found: self.interner.to_type(id2),
            }),
        }
    }
}
```

## Resolution

The `resolve` methods apply substitutions to resolve type variables:

```rust
impl InferenceContext {
    /// Resolve a Type by applying all substitutions.
    pub fn resolve(&self, ty: &Type) -> Type {
        let id = ty.to_type_id(&self.interner);
        let resolved = self.resolve_id(id);
        self.interner.to_type(resolved)
    }

    /// Resolve a TypeId by applying all substitutions.
    pub fn resolve_id(&self, id: TypeId) -> TypeId {
        let mut resolver = TypeIdResolver {
            interner: &self.interner,
            substitutions: &self.substitutions,
        };
        resolver.fold(id)
    }
}

/// TypeIdFolder that resolves type variables through substitutions.
struct TypeIdResolver<'a> {
    interner: &'a TypeInterner,
    substitutions: &'a HashMap<TypeVar, TypeId>,
}

impl TypeIdFolder for TypeIdResolver<'_> {
    fn interner(&self) -> &TypeInterner { self.interner }

    fn fold_var(&mut self, var: TypeVar) -> TypeId {
        if let Some(&resolved) = self.substitutions.get(&var) {
            self.fold(resolved)  // Recursively resolve chains
        } else {
            self.interner.intern(TypeData::Var(var))
        }
    }
}
```

## Occurs Check

The occurs check prevents creating infinite types:

```rust
// Would create: T0 = [T0] = [[T0]] = ...
let xs = [xs]  // Error: infinite type
```

```rust
impl InferenceContext {
    /// Check if a type variable occurs in a type (prevents infinite types).
    fn occurs_id(&self, var: TypeVar, id: TypeId) -> bool {
        let mut checker = OccursChecker {
            interner: &self.interner,
            substitutions: &self.substitutions,
            target: var,
            found: false,
        };
        checker.visit(id);
        checker.found
    }
}

/// TypeIdVisitor that checks for occurrence of a type variable.
struct OccursChecker<'a> {
    interner: &'a TypeInterner,
    substitutions: &'a HashMap<TypeVar, TypeId>,
    target: TypeVar,
    found: bool,
}

impl TypeIdVisitor for OccursChecker<'_> {
    fn interner(&self) -> &TypeInterner { self.interner }

    fn visit_var(&mut self, var: TypeVar) {
        if var == self.target {
            self.found = true;
        } else if let Some(&resolved) = self.substitutions.get(&var) {
            self.visit(resolved);  // Check through substitution chain
        }
    }
}
```

## Substitution Storage

Substitutions are stored as a simple `HashMap<TypeVar, TypeId>`:

```rust
pub struct InferenceContext {
    /// Type variable substitutions.
    substitutions: FxHashMap<TypeVar, TypeId>,
    // ... other fields
}
```

The `TypeIdResolver` (shown above) handles recursive resolution through substitution chains. No union-find is used — the simple HashMap approach is sufficient for Ori's type inference needs.

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

## Immediate Unification

Ori uses **immediate unification** during type inference — constraints are unified as they're generated, not collected and solved later. This simplifies the implementation while still supporting Hindley-Milner inference.

Benefits of immediate unification:
- Simpler implementation (no constraint storage)
- Errors reported at point of occurrence
- Substitutions available immediately for subsequent inference

The trade-off is that some advanced type system features (like ranked types or bidirectional type checking) would require refactoring to constraint-based approach.
