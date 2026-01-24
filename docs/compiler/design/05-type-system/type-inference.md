# Type Inference

Sigil uses Hindley-Milner (HM) type inference, extended with features for patterns and capabilities.

## How HM Inference Works

### 1. Fresh Type Variables

When a type is unknown, create a fresh type variable:

```rust
fn fresh_type_var(&mut self) -> Type {
    let id = self.next_var;
    self.next_var = TypeVarId(self.next_var.0 + 1);
    Type::TypeVar(id)
}
```

### 2. Constraint Generation

Walk the AST and generate equality constraints:

```rust
fn infer_expr(&mut self, expr: ExprId) -> Type {
    let expr_data = self.arena.get(expr);

    match &expr_data.kind {
        ExprKind::Literal(Literal::Int(_)) => Type::Int,

        ExprKind::Ident(name) => {
            self.env.lookup(*name).cloned()
                .unwrap_or_else(|| self.error_undefined(*name))
        }

        ExprKind::Binary { left, op, right } => {
            let left_ty = self.infer_expr(*left);
            let right_ty = self.infer_expr(*right);
            self.infer_binary_op(*op, left_ty, right_ty)
        }

        ExprKind::Let { name, value, body } => {
            let value_ty = self.infer_expr(*value);
            self.env.push_scope();
            self.env.bind(*name, value_ty);
            let body_ty = self.infer_expr(*body);
            self.env.pop_scope();
            body_ty
        }

        // ...
    }
}
```

### 3. Unification

Solve constraints by unifying types:

```rust
fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
    let t1 = self.apply_subst(t1);
    let t2 = self.apply_subst(t2);

    match (&t1, &t2) {
        // Same type - ok
        (Type::Int, Type::Int) => Ok(()),

        // Type variable - bind it
        (Type::TypeVar(id), ty) | (ty, Type::TypeVar(id)) => {
            if self.occurs_in(*id, ty) {
                Err(TypeError::InfiniteType)
            } else {
                self.substitution.insert(*id, ty.clone());
                Ok(())
            }
        }

        // Compound types - recurse
        (Type::List(a), Type::List(b)) => self.unify(a, b),

        (Type::Function { params: p1, ret: r1 },
         Type::Function { params: p2, ret: r2 }) => {
            if p1.len() != p2.len() {
                return Err(TypeError::ParamCountMismatch);
            }
            for (a, b) in p1.iter().zip(p2.iter()) {
                self.unify(a, b)?;
            }
            self.unify(r1, r2)
        }

        // Mismatch
        _ => Err(TypeError::Mismatch { expected: t1, found: t2 }),
    }
}
```

### 4. Substitution Application

Apply the substitution to resolve type variables:

```rust
fn apply_subst(&self, ty: &Type) -> Type {
    match ty {
        Type::TypeVar(id) => {
            if let Some(resolved) = self.substitution.get(id) {
                self.apply_subst(resolved)
            } else {
                ty.clone()
            }
        }
        Type::List(elem) => Type::List(Box::new(self.apply_subst(elem))),
        Type::Function { params, ret } => Type::Function {
            params: params.iter().map(|p| self.apply_subst(p)).collect(),
            ret: Box::new(self.apply_subst(ret)),
        },
        _ => ty.clone(),
    }
}
```

## Inference Examples

### Let Binding

```sigil
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

```sigil
@double (x: int) -> int = x * 2
double(21)
```

```
1. double : (Int) -> Int
2. 21 : Int
3. double(21) : apply (Int) -> Int to (Int) = Int
```

### Generic Function

```sigil
@identity<T> (x: T) -> T = x
identity(42)
identity("hello")
```

```
1. identity : forall T. (T) -> T
2. identity(42):
   - Instantiate: (T0) -> T0
   - Unify(T0, Int)
   - Result: Int
3. identity("hello"):
   - Instantiate: (T1) -> T1
   - Unify(T1, String)
   - Result: String
```

### List Inference

```sigil
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

```sigil
let id = x -> x           // forall T. T -> T
let a = id(42)            // Int
let b = id("hello")       // String
```

This is called "let-generalization":

```rust
fn infer_let(&mut self, name: Name, value: ExprId, body: ExprId) -> Type {
    let value_ty = self.infer_expr(value);

    // Generalize: find unbound type variables
    let generalized = self.generalize(value_ty);

    self.env.bind(name, generalized);
    self.infer_expr(body)
}

fn generalize(&self, ty: Type) -> Type {
    // Find type variables not bound in environment
    let free_vars = self.free_type_vars(&ty);
    let env_vars = self.env.free_type_vars();
    let generalizable = free_vars.difference(&env_vars);

    if generalizable.is_empty() {
        ty
    } else {
        Type::Forall { vars: generalizable.collect(), ty: Box::new(ty) }
    }
}
```

## Occurs Check

Prevent infinite types like `T = [T]`:

```rust
fn occurs_in(&self, var: TypeVarId, ty: &Type) -> bool {
    match ty {
        Type::TypeVar(id) => {
            if *id == var {
                true
            } else if let Some(resolved) = self.substitution.get(id) {
                self.occurs_in(var, resolved)
            } else {
                false
            }
        }
        Type::List(elem) => self.occurs_in(var, elem),
        Type::Function { params, ret } => {
            params.iter().any(|p| self.occurs_in(var, p))
                || self.occurs_in(var, ret)
        }
        _ => false,
    }
}
```

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
