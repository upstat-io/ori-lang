# Type System Overview

The Sigil type system provides strict static typing with Hindley-Milner type inference. The type checker validates programs and infers types for expressions.

## Location

```
compiler/sigilc/src/
├── types.rs            # Type definitions (~1,015 lines)
└── typeck/
    ├── mod.rs          # Module exports
    ├── checker.rs      # Main type checker (~576 lines)
    ├── operators.rs    # Operator type rules (~515 lines)
    ├── type_registry.rs # User-defined types (~432 lines)
    └── infer/
        ├── mod.rs      # Inference dispatcher
        ├── expr.rs     # Expression inference
        ├── call.rs     # Call type checking
        ├── control.rs  # Control flow inference
        └── pattern.rs  # Pattern type checking
```

## Design Goals

1. **Sound type system** - No runtime type errors
2. **Full inference** - Minimal type annotations required
3. **Good error messages** - Clear, actionable diagnostics
4. **Capability tracking** - Track side effects in types

## Type Checking Flow

```
ParseResult { Module, ExprArena }
    │
    │ create TypeChecker
    ▼
TypeChecker {
    env: TypeEnv,       // Variable -> Type
    registry: TypeRegistry,  // Named types
    constraints: Vec<Constraint>,
}
    │
    │ infer types for all expressions
    ▼
TypedModule {
    expr_types: Vec<Type>,  // Type per ExprId
    errors: Vec<TypeError>,
}
```

## Core Components

### Type Enum

```rust
pub enum Type {
    // Primitives
    Int, Float, Bool, String, Char, Void, Never,

    // Compound
    List(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },

    // Inference
    TypeVar(TypeVarId),

    // User-defined
    Named(Name),
}
```

### TypeChecker

```rust
pub struct TypeChecker {
    /// Type environment (variable -> type)
    env: TypeEnv,

    /// User-defined types
    registry: TypeRegistry,

    /// Fresh type variable counter
    next_var: TypeVarId,

    /// Substitution from unification
    substitution: HashMap<TypeVarId, Type>,

    /// Accumulated errors
    errors: Vec<TypeError>,
}
```

### TypeEnv

```rust
pub struct TypeEnv {
    /// Stack of scopes
    scopes: Vec<Scope>,
}

pub struct Scope {
    /// Variable bindings
    bindings: HashMap<Name, Type>,
}
```

## Inference Algorithm

1. **Constraint Generation**
   - Walk AST, generate type constraints
   - Fresh type variables for unknowns

2. **Unification**
   - Solve constraints by unifying types
   - Build substitution map

3. **Substitution**
   - Apply substitution to resolve type variables

```rust
// Example: let x = 42
// 1. x has fresh type T0
// 2. 42 has type Int
// 3. Constraint: T0 = Int
// 4. Unify: substitution[T0] = Int
// 5. Result: x has type Int
```

## Type Rules

### Literals

```
42      : Int
3.14    : Float
"hello" : String
true    : Bool
[]      : [T]  (fresh T)
```

### Binary Operations

```
Int + Int       -> Int
Float + Float   -> Float
String + String -> String
Int < Int       -> Bool
T == T          -> Bool (where T: Eq)
```

### Conditionals

```
if cond then t else e
  cond : Bool
  t : T
  e : T
  result : T
```

### Functions

```
@add (a: int, b: int) -> int
  a, b : Int
  body : Int
  function : (Int, Int) -> Int
```

## Related Documents

- [Type Inference](type-inference.md) - Hindley-Milner inference
- [Unification](unification.md) - Constraint solving
- [Type Environment](type-environment.md) - Scope tracking
- [Type Registry](type-registry.md) - User-defined types
