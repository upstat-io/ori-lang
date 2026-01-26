# Type System Overview

The Sigil type system provides strict static typing with Hindley-Milner type inference. The type checker validates programs and infers types for expressions.

## Location

Type definitions are split across two crates:

```
compiler/sigil_types/src/
└── lib.rs              # Type enum, TypeError, TypeContext, InferenceContext

compiler/sigilc/src/typeck/
├── mod.rs              # Module exports
├── checker/            # Main type checker
│   ├── mod.rs              # TypeChecker struct, check_module entry
│   ├── builder.rs          # TypeCheckerBuilder pattern
│   ├── components.rs       # CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext
│   ├── scope_guards.rs     # RAII scope guards for capability/impl contexts
│   ├── signatures.rs       # infer_function_signature
│   ├── pattern_binding.rs  # bind_pattern logic
│   ├── cycle_detection.rs  # collect_free_vars, closure self-capture
│   ├── trait_registration.rs # register_traits, register_impls
│   ├── bound_checking.rs   # type_satisfies_bound
│   ├── type_registration.rs # register_type_declarations
│   ├── types.rs            # Helper types (FunctionType, TypeCheckError)
│   └── tests.rs            # Unit tests
├── operators.rs        # Operator type rules
├── type_registry/      # User-defined types
│   ├── mod.rs              # TypeRegistry struct
│   └── trait_registry.rs   # TraitRegistry, TraitEntry, ImplEntry
└── infer/
    ├── mod.rs          # Inference dispatcher
    ├── expr.rs         # Expression inference
    ├── call.rs         # Call type checking
    ├── control.rs      # Control flow inference
    ├── match_binding.rs # Match arm binding inference
    └── pattern.rs      # Pattern type checking
```

The `sigil_types` crate contains the `Type` enum, `TypeError`, `TypeContext` (for type instantiation deduplication), and `InferenceContext`. The type checker itself remains in `sigilc` due to complex dependencies on the evaluator and pattern system.

Note: `sigilc/src/types.rs` re-exports from `sigil_types` (DRY consolidation).

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

The TypeChecker is organized into logical components for better testability and maintainability:

```rust
pub struct TypeChecker<'a> {
    /// External references (arena, interner)
    context: CheckContext<'a>,

    /// Inference state (ctx, env, base_env, expr_types)
    inference: InferenceState,

    /// Registries (pattern, type_op, types, traits)
    registries: Registries,

    /// Diagnostic collection (errors, queue, source)
    diagnostics: DiagnosticState,

    /// Function/scope context (function_sigs, current_impl_self, config_types, capabilities)
    scope: ScopeContext,
}
```

#### Component Structs

| Component | Purpose | Fields |
|-----------|---------|--------|
| `CheckContext<'a>` | Immutable external references | `arena`, `interner` |
| `InferenceState` | Mutable inference context | `ctx`, `env`, `base_env`, `expr_types` |
| `Registries` | Pattern, type, and trait registries | `pattern`, `type_op`, `types`, `traits` |
| `DiagnosticState` | Error collection and limiting | `errors`, `queue`, `source` |
| `ScopeContext` | Current scope state | `function_sigs`, `current_impl_self`, `config_types`, `current_function_caps`, `provided_caps` |

#### TypeCheckerBuilder

Use the builder pattern for flexible construction:

```rust
let checker = TypeCheckerBuilder::new(&arena, &interner)
    .with_source(source_code)           // Enable diagnostic queue features
    .with_context(&compiler_context)    // Use custom registries
    .with_diagnostic_config(config)     // Custom error limits
    .build();
```

#### RAII Scope Guards

The type checker uses RAII-style scope guards for safe context management:

```rust
// Capability scope (for function checking)
checker.with_capability_scope(new_caps, |c| {
    // Capabilities are active here
    // Automatically restored on return (even early returns)
});

// Impl scope (for impl block checking)
checker.with_impl_scope(self_type, |c| {
    // Self type is available here
    // Automatically restored on return
});
```

This prevents bugs from forgotten context restores and ensures cleanup on early returns.

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

### TypeContext

TypeContext provides deduplication for generic type instantiations within a single type-checking pass:

```rust
pub struct TypeContext {
    /// hash(origin + targs) -> [(origin, targs, instance)]
    type_map: HashMap<u64, Vec<TypeContextEntry>>,
    /// Origin type -> stable ID for hashing
    origin_ids: HashMap<TypeScheme, u32>,
    next_origin_id: u32,
}
```

This ensures identical generic instantiations (e.g., `Option<int>`) share the same `Type` instance, reducing allocations and enabling fast equality checks.

**Convenience methods:**

```rust
impl TypeContext {
    pub fn list_type(&mut self, elem: Type) -> Type;
    pub fn option_type(&mut self, inner: Type) -> Type;
    pub fn result_type(&mut self, ok: Type, err: Type) -> Type;
    pub fn map_type(&mut self, key: Type, value: Type) -> Type;
    pub fn set_type(&mut self, elem: Type) -> Type;
    pub fn range_type(&mut self, elem: Type) -> Type;
    pub fn channel_type(&mut self, elem: Type) -> Type;
    pub fn tuple_type(&mut self, types: Vec<Type>) -> Type;
    pub fn function_type(&mut self, params: Vec<Type>, ret: Type) -> Type;
}
```

### InferenceContext

InferenceContext integrates type variable management with TypeContext:

```rust
pub struct InferenceContext {
    next_var: u32,
    substitutions: HashMap<TypeVar, Type>,
    type_context: TypeContext,  // Integrated deduplication
}

impl InferenceContext {
    // Type variable management
    pub fn fresh_var(&mut self) -> Type;
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError>;
    pub fn apply(&self, ty: &Type) -> Type;

    // Type construction (uses TypeContext for deduplication)
    pub fn make_list(&mut self, elem: Type) -> Type;
    pub fn make_option(&mut self, inner: Type) -> Type;
    pub fn make_result(&mut self, ok: Type, err: Type) -> Type;
    pub fn make_map(&mut self, key: Type, value: Type) -> Type;
    pub fn make_set(&mut self, elem: Type) -> Type;
    pub fn make_range(&mut self, elem: Type) -> Type;
    pub fn make_channel(&mut self, elem: Type) -> Type;
    pub fn make_tuple(&mut self, types: Vec<Type>) -> Type;
    pub fn make_function(&mut self, params: Vec<Type>, ret: Type) -> Type;
}
```

Usage in type inference:

```rust
// Instead of:
Type::List(Box::new(elem_type))

// Use:
checker.ctx.make_list(elem_type)
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
