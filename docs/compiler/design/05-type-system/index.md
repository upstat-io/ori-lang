# Type System Overview

The Sigil type system provides strict static typing with Hindley-Milner type inference. The type checker validates programs and infers types for expressions.

## Location

Type definitions are split across two crates:

```
compiler/sigil_types/src/
├── lib.rs              # Module exports, size assertions, tests
├── core.rs             # Type enum, TypeVar, TypeScheme
├── env.rs              # TypeEnv for name resolution/scoping
├── traverse.rs         # TypeFolder, TypeVisitor traits
├── context.rs          # InferenceContext, TypeContext
└── error.rs            # TypeError enum with diagnostic conversion

compiler/sigil_typeck/src/
├── lib.rs              # Module exports
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
│   └── types.rs            # Helper types (FunctionType, TypeCheckError)
├── operators.rs        # Operator type rules
├── derives/            # Derive macro support
│   └── mod.rs              # Derive registration and checking
├── registry/           # User-defined types
│   ├── mod.rs              # TypeRegistry struct
│   └── trait_registry.rs   # TraitRegistry, TraitEntry, ImplEntry
└── infer/
    ├── mod.rs          # Inference dispatcher
    ├── expr.rs         # Expression inference
    ├── call.rs         # Call type checking
    ├── control.rs      # Control flow inference
    ├── match_binding.rs # Match arm binding inference
    ├── pattern.rs      # Pattern type checking
    └── builtin_methods/ # Built-in type method handlers
        ├── mod.rs          # BuiltinMethodRegistry, BuiltinMethodHandler trait
        ├── string.rs       # StringMethodHandler
        ├── list.rs         # ListMethodHandler
        ├── map.rs          # MapMethodHandler
        ├── option.rs       # OptionMethodHandler
        ├── result.rs       # ResultMethodHandler
        └── numeric.rs      # NumericMethodHandler (int, float, bool)
```

The `sigil_types` crate contains:
- `core.rs`: The `Type` enum, `TypeVar`, and `TypeScheme` definitions
- `env.rs`: `TypeEnv` for variable-to-type bindings with scope support
- `traverse.rs`: `TypeFolder` and `TypeVisitor` traits for type transformations
- `context.rs`: `InferenceContext` (unification, generalization) and `TypeContext` (deduplication)
- `error.rs`: `TypeError` enum with diagnostic conversion

The type checker lives in the `sigil_typeck` crate, with orchestration in `sigilc` via Salsa queries.

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

## Built-in Method Type Checking

The type checker uses a registry-based pattern for type checking method calls on built-in types. This follows the Open/Closed Principle—new type handlers can be added without modifying existing code.

### BuiltinMethodHandler Trait

Each built-in type has a dedicated handler implementing the `BuiltinMethodHandler` trait:

```rust
pub trait BuiltinMethodHandler: Send + Sync {
    /// Check if this handler handles the given receiver type.
    fn handles(&self, receiver_ty: &Type) -> bool;

    /// Type check the method call.
    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        args: &[Type],
        span: Span,
    ) -> MethodTypeResult;
}
```

### BuiltinMethodRegistry

The registry iterates through handlers to find one that handles the receiver type:

```rust
pub struct BuiltinMethodRegistry {
    handlers: Vec<Box<dyn BuiltinMethodHandler>>,
}

impl BuiltinMethodRegistry {
    pub fn new() -> Self {
        BuiltinMethodRegistry {
            handlers: vec![
                Box::new(StringMethodHandler),
                Box::new(ListMethodHandler),
                Box::new(MapMethodHandler),
                Box::new(OptionMethodHandler),
                Box::new(ResultMethodHandler),
                Box::new(NumericMethodHandler),
            ],
        }
    }

    pub fn check(&self, ...) -> Option<MethodTypeResult> {
        for handler in &self.handlers {
            if handler.handles(receiver_ty) {
                return Some(handler.check(...));
            }
        }
        None
    }
}
```

### Handler Organization

| Handler | Types | Methods |
|---------|-------|---------|
| `StringMethodHandler` | `str` | `len`, `split`, `trim`, `contains`, etc. |
| `ListMethodHandler` | `[T]` | `len`, `push`, `pop`, `get`, `map`, `filter`, etc. |
| `MapMethodHandler` | `{K: V}` | `len`, `get`, `insert`, `remove`, `keys`, etc. |
| `OptionMethodHandler` | `Option<T>` | `map`, `unwrap_or`, `ok_or`, `and_then`, etc. |
| `ResultMethodHandler` | `Result<T, E>` | `map`, `map_err`, `unwrap_or`, `ok`, `err`, etc. |
| `NumericMethodHandler` | `int`, `float`, `bool` | `abs`, `to_string`, numeric methods |

This design replaces nested match statements with focused, single-responsibility handlers.

## Related Documents

- [Type Inference](type-inference.md) - Hindley-Milner inference
- [Unification](unification.md) - Constraint solving
- [Type Environment](type-environment.md) - Scope tracking
- [Type Registry](type-registry.md) - User-defined types
