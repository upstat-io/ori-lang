---
title: "Type Inference"
description: "Ori Compiler Design — Type Inference"
order: 502
section: "Type System"
---

# Type Inference

Ori uses Hindley-Milner (HM) type inference, extended with features for patterns and capabilities.

## How HM Inference Works

### 1. Fresh Type Variables

When a type is unknown, create a fresh type variable:

```rust
impl InferenceContext {
    pub fn fresh_var(&mut self) -> Type {
        let var = TypeVar::new(self.next_var);
        self.next_var += 1;
        Type::Var(var)
    }

    pub fn fresh_var_id(&mut self) -> TypeId {
        let var = TypeVar::new(self.next_var);
        self.next_var += 1;
        self.interner.intern(TypeData::Var(var))
    }
}
```

### 2. Expression Inference

Walk the AST and infer types, unifying immediately:

```rust
fn infer_expr(&mut self, expr: ExprId) -> Type {
    let expr_data = self.arena.get(expr);

    match &expr_data.kind {
        ExprKind::Literal(Literal::Int(_)) => Type::Int,

        ExprKind::Ident(name) => {
            self.env.lookup(*name)
                .unwrap_or_else(|| self.error_undefined(*name))
        }

        ExprKind::Binary { left, op, right } => {
            let left_ty = self.infer_expr(*left);
            let right_ty = self.infer_expr(*right);
            self.infer_binary_op(*op, left_ty, right_ty)
        }

        ExprKind::Let { name, value, body } => {
            let value_ty = self.infer_expr(*value);
            let mut child_env = self.env.child();
            child_env.bind(*name, value_ty);
            self.infer_expr_with_env(*body, &child_env)
        }

        // ...
    }
}
```

### 3. Unification

Unification happens immediately via `InferenceContext`:

```rust
impl InferenceContext {
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
        let id1 = t1.to_type_id(&self.interner);
        let id2 = t2.to_type_id(&self.interner);
        self.unify_ids(id1, id2)  // O(1) fast path if identical
    }

    pub fn unify_ids(&mut self, id1: TypeId, id2: TypeId) -> Result<(), TypeError> {
        if id1 == id2 { return Ok(()); }  // O(1) fast path

        let data1 = self.interner.lookup(self.resolve_id(id1));
        let data2 = self.interner.lookup(self.resolve_id(id2));

        match (&data1, &data2) {
            // Type variable - bind it (after occurs check)
            (TypeData::Var(v), _) => {
                if self.occurs_id(*v, id2) { return Err(TypeError::InfiniteType); }
                self.substitutions.insert(*v, id2);
                Ok(())
            }

            // Compound types - recurse
            (TypeData::List(a), TypeData::List(b)) => self.unify_ids(*a, *b),

            // Error/Never unify with anything
            (TypeData::Error | TypeData::Never, _) |
            (_, TypeData::Error | TypeData::Never) => Ok(()),

            // Mismatch
            _ => Err(TypeError::Mismatch { /* ... */ }),
        }
    }
}
```

See [Unification](unification.md) for the complete algorithm.

### 4. Resolution

Apply substitutions to resolve type variables:

```rust
impl InferenceContext {
    /// Resolve a Type by applying all substitutions.
    pub fn resolve(&self, ty: &Type) -> Type {
        let id = ty.to_type_id(&self.interner);
        let resolved = self.resolve_id(id);
        self.interner.to_type(resolved)
    }

    /// Resolve a TypeId (internal, uses TypeIdFolder).
    pub fn resolve_id(&self, id: TypeId) -> TypeId {
        let mut resolver = TypeIdResolver {
            interner: &self.interner,
            substitutions: &self.substitutions,
        };
        resolver.fold(id)
    }
}
```

## Inference Examples

### Let Binding

```ori
let x = 42
let y = x + 1
```

```
1. x : T0 (fresh)
2. 42 : Int
3. Unify(T0, Int) -> substitution[T0] = Int
4. y : T1 (fresh)
5. x + 1 : lookup(+, Int, Int) = Int
6. Unify(T1, Int) -> substitution[T1] = Int
```

### Function Application

```ori
@double (x: int) -> int = x * 2
double(21)
```

```
1. double : (Int) -> Int
2. 21 : Int
3. double(21) : apply (Int) -> Int to (Int) = Int
```

### Generic Function

```ori
@identity<T> (x: T) -> T = x
identity(42)
identity("hello")
```

```
1. identity : forall T. (T) -> T
2. identity(42):
   - Instantiate: (T0) -> T0
   - Unify(T0, int)
   - Result: int
3. identity("hello"):
   - Instantiate: (T1) -> T1
   - Unify(T1, str)
   - Result: str
```

### List Inference

```ori
let xs = [1, 2, 3]
let ys = map(over: xs, transform: x -> x * 2)
```

```
1. [1, 2, 3] : [T0] where each element unifies with T0
   - 1 : Int, unify(T0, Int)
   - Result: [Int]
2. map:
   - over: [Int]
   - transform: T1 -> T2
   - Unify(T1, Int) from element type
   - x * 2 : Int, so T2 = Int
   - Result: [Int]
```

## Let Polymorphism

Variables bound with `let` can be polymorphic:

```ori
let id = x -> x           // forall T. T -> T
let a = id(42)            // int
let b = id("hello")       // str
```

This is called "let-generalization":

```rust
impl InferenceContext {
    /// Generalize a type by quantifying over free type variables.
    pub fn generalize(&self, ty: &Type, env: &TypeEnv) -> TypeScheme {
        let ty_vars = self.free_vars(ty);
        let env_vars: HashSet<_> = env.free_vars(self).into_iter().collect();

        // Quantify over variables free in ty but not in env
        let generalizable: Vec<_> = ty_vars.iter()
            .filter(|v| !env_vars.contains(v))
            .cloned()
            .collect();

        TypeScheme {
            vars: generalizable,
            ty: ty.clone(),
        }
    }
}
```

The `TypeScheme` is stored in the environment and instantiated with fresh variables on each use.

## Occurs Check

Prevent infinite types like `T = [T]`:

```rust
impl InferenceContext {
    /// Check if type variable occurs in a type (prevents infinite types).
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
```

See [Unification](unification.md) for the `OccursChecker` implementation using `TypeIdVisitor`.

## Error Reporting

When unification fails, report the mismatch:

```rust
Err(TypeError::Mismatch {
    expected: Type::Int,
    found: Type::String,
    span: expr.span,
    context: "in binary addition",
})
```

Output:
```
error[E2001]: type mismatch
 --> src/mainsi:5:10
  |
5 |     42 + "hello"
  |          ^^^^^^^ expected int, found str
  |
  = note: in binary addition
```
