---
title: "Type Environment"
description: "Ori Compiler Design — Type Environment"
order: 501
section: "Type System"
---

# Type Environment

The type environment tracks variable bindings during type checking. It uses a stack of scopes for lexical scoping.

## Structure

```rust
pub struct TypeEnv {
    /// Stack of scopes (innermost last)
    scopes: Vec<Scope>,
}

pub struct Scope {
    /// Variable name -> Type
    bindings: HashMap<Name, Type>,

    /// Optional scope kind for error messages
    kind: ScopeKind,
}

pub enum ScopeKind {
    Global,
    Function(Name),
    Block,
    Lambda,
    ForLoop,
}
```

## Operations

### Creating Environment

```rust
impl TypeEnv {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::global()],
        }
    }

    pub fn with_prelude() -> Self {
        let mut env = Self::new();

        // Add built-in functions
        env.bind_builtin("print", Type::Function {
            params: vec![Type::String],
            ret: Box::new(Type::Void),
            capabilities: vec![],
        });

        env.bind_builtin("len", Type::Function {
            params: vec![Type::TypeVar(TypeVarId(0))],  // Generic
            ret: Box::new(Type::Int),
            capabilities: vec![],
        });

        // ... more builtins

        env
    }
}
```

### Scope Management

```rust
impl TypeEnv {
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::block());
    }

    pub fn push_function_scope(&mut self, name: Name) {
        self.scopes.push(Scope::function(name));
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}
```

### Variable Binding

```rust
impl TypeEnv {
    pub fn bind(&mut self, name: Name, ty: Type) {
        let scope = self.scopes.last_mut().expect("no scope");
        scope.bindings.insert(name, ty);
    }

    pub fn lookup(&self, name: Name) -> Option<&Type> {
        // Search from innermost to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.bindings.get(&name) {
                return Some(ty);
            }
        }
        None
    }

    pub fn is_defined(&self, name: Name) -> bool {
        self.lookup(name).is_some()
    }
}
```

### Shadowing

Variables in inner scopes shadow outer ones:

```ori
let x = 1       // x : Int in outer scope
let result = run(
    let x = "hello",  // x : String in inner scope
    len(collection: x),  // uses inner x
)
// x : Int (outer still visible)
```

```rust
// In infer_run:
self.env.push_scope();
self.env.bind(x_name, Type::String);  // Shadows outer x
let len_result = self.infer_expr(len_call);
self.env.pop_scope();
// Outer x is now visible again
```

## Type Schemes

For polymorphic bindings, use type schemes:

```rust
pub enum TypeScheme {
    /// Monomorphic type
    Mono(Type),

    /// Polymorphic type: forall vars. type
    Poly {
        vars: Vec<TypeVarId>,
        ty: Type,
    },
}
```

### Instantiation

When using a polymorphic binding, instantiate with fresh variables:

```rust
impl TypeEnv {
    pub fn lookup_instantiate(&mut self, name: Name, fresh_var: &mut impl FnMut() -> TypeVarId) -> Option<Type> {
        match self.lookup_scheme(name)? {
            TypeScheme::Mono(ty) => Some(ty.clone()),
            TypeScheme::Poly { vars, ty } => {
                // Create fresh variables for each bound variable
                let subst: HashMap<TypeVarId, Type> = vars
                    .iter()
                    .map(|&v| (v, Type::TypeVar(fresh_var())))
                    .collect();

                Some(ty.apply_subst(&subst))
            }
        }
    }
}
```

Example:
```ori
let id = x -> x  // forall T. T -> T
id(42)           // Instantiate: T0 -> T0, then unify T0=Int
id("hi")         // Instantiate: T1 -> T1, then unify T1=String
```

## Function Scopes

Functions create their own scope with parameters:

```rust
fn infer_function(&mut self, func: &Function) -> Type {
    self.env.push_function_scope(func.name);

    // Bind parameters
    for param in &func.params {
        self.env.bind(param.name, param.ty.clone());
    }

    // Infer body type
    let body_ty = self.infer_expr(func.body);

    // Check return type annotation
    if let Some(ret_ty) = &func.ret_type {
        self.unify(&body_ty, ret_ty)?;
    }

    self.env.pop_scope();

    Type::Function {
        params: func.params.iter().map(|p| p.ty.clone()).collect(),
        ret: Box::new(body_ty),
        capabilities: func.capabilities.clone(),
    }
}
```

## For Loop Scopes

For loops bind the iteration variable:

```rust
fn infer_for(&mut self, for_expr: &ForExpr) -> Type {
    let iter_ty = self.infer_expr(for_expr.iterable);

    // Extract element type
    let elem_ty = match &iter_ty {
        Type::List(elem) => (**elem).clone(),
        Type::Range(elem) => (**elem).clone(),
        _ => {
            self.error(TypeError::NotIterable(iter_ty));
            return Type::Error;
        }
    };

    self.env.push_scope();
    self.env.bind(for_expr.var, elem_ty);

    let body_ty = self.infer_expr(for_expr.body);

    self.env.pop_scope();

    // for..yield produces a list
    if for_expr.is_yield {
        Type::List(Box::new(body_ty))
    } else {
        Type::Void
    }
}
```

## Error Messages

Track scope kind for better errors:

```rust
fn error_undefined(&self, name: Name) -> Type {
    let similar = self.find_similar_names(name);

    let in_scope = match self.current_scope_kind() {
        ScopeKind::Function(fn_name) => format!("in function @{}", fn_name),
        ScopeKind::Lambda => "in lambda".to_string(),
        _ => "".to_string(),
    };

    self.errors.push(TypeError::UndefinedVariable {
        name,
        similar,
        context: in_scope,
    });

    Type::Error
}
```

## Free Type Variables

Find type variables not bound in environment:

```rust
impl TypeEnv {
    pub fn free_type_vars(&self) -> HashSet<TypeVarId> {
        self.scopes
            .iter()
            .flat_map(|s| s.bindings.values())
            .flat_map(|ty| ty.free_type_vars())
            .collect()
    }
}
```

Used for let-generalization to determine which variables can be generalized.
