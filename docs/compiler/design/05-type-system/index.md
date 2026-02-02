---
title: "Type System Overview"
description: "Ori Compiler Design — Type System Overview"
order: 500
section: "Type System"
---

# Type System Overview

The Ori type system provides strict static typing with Hindley-Milner type inference. The type checker validates programs and infers types for expressions.

## Location

Type definitions are split across two crates:

```
compiler/ori_types/src/
├── lib.rs                    # Module exports, size assertions, tests
├── core.rs                   # Type enum, TypeScheme (external API)
├── data.rs                   # TypeData enum, TypeVar (internal representation)
├── type_interner.rs          # TypeInterner, SharedTypeInterner (O(1) equality)
├── env.rs                    # TypeEnv for name resolution/scoping
├── traverse.rs               # TypeFolder, TypeVisitor, TypeIdFolder, TypeIdVisitor
├── context.rs                # InferenceContext, TypeContext
└── error.rs                  # TypeError enum with diagnostic conversion

compiler/ori_typeck/src/
├── lib.rs                    # Module exports
├── checker/                  # Main type checker
│   ├── mod.rs                    # TypeChecker struct, constructors
│   ├── api.rs                    # Public API: type_check, type_check_with_source, type_check_with_config
│   ├── orchestration.rs          # check_module 4-pass logic
│   ├── utilities.rs              # validate_capabilities, resolve_through_aliases, report_type_error
│   ├── builder.rs                # TypeCheckerBuilder pattern
│   ├── components.rs             # CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext
│   ├── scope_guards.rs           # RAII scope guards (private fields, pub(super) access)
│   ├── function_checking.rs      # check_callable, check_function, check_test, check_impl_methods
│   ├── type_resolution.rs        # type_id_to_type, parsed_type_to_type, resolve_well_known_generic
│   ├── signatures.rs             # infer_function_signature
│   ├── pattern_binding.rs        # bind_pattern logic
│   ├── cycle_detection.rs        # collect_free_vars, closure self-capture
│   ├── trait_registration.rs     # register_traits, register_impls
│   ├── bound_checking.rs         # type_satisfies_bound
│   ├── type_registration.rs      # register_type_declarations
│   ├── imports.rs                # ImportedFunction, ImportedModuleAlias, register_module_alias
│   ├── types.rs                  # Helper types (FunctionType, TypeCheckError)
│   └── tests/                    # Test modules
│       └── mod.rs                    # TypeChecker unit tests
├── operators.rs              # Operator type rules
├── derives/                  # Derive macro support
│   └── mod.rs                    # Derive registration and checking
├── registry/                 # User-defined types
│   ├── mod.rs                    # TypeRegistry struct, re-exports
│   ├── trait_registry.rs         # TraitRegistry core (method_cache)
│   ├── trait_types.rs            # TraitMethodDef, TraitAssocTypeDef, TraitEntry
│   ├── impl_types.rs             # ImplMethodDef, ImplAssocTypeDef, ImplEntry, CoherenceError
│   ├── method_lookup.rs          # MethodLookup result type
│   └── tests/                    # Test modules
│       ├── mod.rs                    # Test module declarations
│       ├── trait_registry_tests.rs   # TraitRegistry tests
│       └── type_registry_tests.rs    # TypeRegistry tests
└── infer/
    ├── mod.rs                # Inference dispatcher, re-exports
    ├── free_vars.rs          # collect_free_vars_inner, add_pattern_bindings
    ├── type_annotations.rs   # infer_let_init, check_type_annotation
    ├── call.rs               # Call type checking
    ├── control.rs            # Control flow inference
    ├── match_binding.rs      # extract_match_pattern_bindings, collect_match_pattern_names
    ├── pattern_types.rs      # get_variant_field_types (Vec<Type>), get_struct_field_types
    ├── pattern_unification.rs # unify_pattern_with_scrutinee
    ├── pattern.rs            # Pattern type checking
    ├── expressions/          # Expression type inference (split from expr.rs)
    │   ├── mod.rs                # Re-exports, substitute_type_params
    │   ├── identifiers.rs        # infer_ident, infer_function_ref, builtin_function_type
    │   ├── operators.rs          # infer_binary, infer_unary, check_binary_op, check_unary_op
    │   ├── lambdas.rs            # infer_lambda
    │   ├── collections.rs        # infer_list, infer_tuple, infer_map, infer_range
    │   ├── structs.rs            # infer_struct, FieldLookupResult, field lookup helpers
    │   ├── access.rs             # infer_field, infer_index
    │   └── variants.rs           # infer_ok, infer_err, infer_some, infer_none
    └── builtin_methods/      # Built-in type method handlers
        ├── mod.rs                # BuiltinMethodRegistry, BuiltinMethodHandler trait
        ├── string.rs             # StringMethodHandler
        ├── list.rs               # ListMethodHandler
        ├── map.rs                # MapMethodHandler
        ├── option.rs             # OptionMethodHandler
        ├── result.rs             # ResultMethodHandler
        └── numeric.rs            # NumericMethodHandler (int, float, bool)
```

The `ori_types` crate contains:
- `core.rs`: The external `Type` enum and `TypeScheme` definitions
- `data.rs`: The internal `TypeData` enum and `TypeVar` for the interner
- `type_interner.rs`: `TypeInterner` and `SharedTypeInterner` for O(1) type equality
- `env.rs`: `TypeEnv` for variable-to-type bindings with scope support
- `traverse.rs`: Traversal traits for both representations:
  - `TypeFolder`/`TypeVisitor`: Work with boxed `Type` (external API)
  - `TypeIdFolder`/`TypeIdVisitor`: Work with interned `TypeId` (internal, preferred)
- `context.rs`: `InferenceContext` (TypeId-based unification) and `TypeContext` (deduplication)
- `error.rs`: `TypeError` enum with diagnostic conversion

The type checker lives in the `ori_typeck` crate, with orchestration in `oric` via Salsa queries.

Note: `oric/src/types.rs` re-exports from `ori_types` (DRY consolidation).

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
    inference: InferenceState,   // ctx, env, expr_types
    registries: Registries,      // pattern, types, traits
    diagnostics: DiagnosticState,
}
    │
    │ infer types for all expressions (immediate unification)
    ▼
TypedModule {
    expr_types: Vec<TypeId>,  // Type per ExprId
    errors: Vec<TypeError>,
}
```

## Core Components

### Type Enum

```rust
pub enum Type {
    // Primitives
    Int, Float, Bool, Str, Char, Byte, Unit, Never,
    Duration, Size, Ordering,

    // Compound
    List(Box<Type>),
    Map { key: Box<Type>, value: Box<Type> },
    Set(Box<Type>),
    Option(Box<Type>),
    Result { ok: Box<Type>, err: Box<Type> },
    Range(Box<Type>),
    Channel(Box<Type>),
    Tuple(Vec<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },

    // Module namespace (for module alias imports)
    ModuleNamespace { items: Vec<(Name, Type)> },

    // Inference
    Var(TypeVar),

    // User-defined
    Named(Name),
    Applied { name: Name, args: Vec<Type> },
    Projection { base: Box<Type>, trait_name: Name, assoc_name: Name },

    // Error recovery
    Error,
}
```

### ModuleNamespace Type

The `ModuleNamespace` variant represents module aliases created by `use std.http as http` imports. It stores a mapping of exported item names to their types, enabling qualified access type checking:

```rust
// Ori source
use std.http as http
http.get(url: "/api")  // Qualified access

// Type representation
Type::ModuleNamespace {
    items: vec![
        (intern("get"), Type::Function { params: vec![Type::String], ret: ... }),
        (intern("post"), Type::Function { params: vec![Type::String, ...], ret: ... }),
        // ... other exports
    ]
}
```

When field access occurs on a `ModuleNamespace` type, the type checker looks up the field name in the `items` vector and returns the corresponding function type.

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

The scope guard structs (`SavedCapabilityContext`, `SavedImplContext`) use private fields with `pub(super)` access, ensuring they can only be constructed through the guard methods.

#### Extracted Checker Modules

The type checker's `check_function` and `check_test` methods share ~95% of their logic. The shared `check_callable()` method in `function_checking.rs` eliminates this duplication:

```rust
// function_checking.rs
fn check_callable(&mut self, params: &[Param], param_types: &[Type],
                   return_type: &Type, body: ExprId, capabilities: HashSet<Name>)
```

Both `check_function()` and `check_test()` prepare their specific params/capabilities then delegate to `check_callable()`.

Type resolution logic (`type_id_to_type`, `parsed_type_to_type`, `resolve_well_known_generic`, `make_projection_type`) is extracted to `type_resolution.rs`.

#### TraitRegistry Method Cache

The `TraitRegistry` uses a `RefCell<HashMap<(Type, Name), Option<MethodLookup>>>` cache for method lookups, converting the `lookup_method()` scan from O(n) to O(1) for repeated lookups:

```rust
pub fn lookup_method(&self, self_ty: &Type, method_name: Name) -> Option<MethodLookup> {
    // Check cache first
    if let Some(cached) = self.method_cache.borrow().get(&cache_key) {
        return cached.clone();
    }
    // Uncached path: scan all impls, then cache result
    let result = self.lookup_method_uncached(self_ty, method_name);
    self.method_cache.borrow_mut().insert(cache_key, result.clone());
    result
}
```

The cache is cleared whenever `register_trait()` or `register_impl()` is called.

### TypeEnv

The type environment uses `Rc`-based parent chain for efficient scoping:

```rust
/// Internal storage wrapped in Rc for O(1) child creation.
struct TypeEnvInner {
    bindings: FxHashMap<Name, TypeSchemeId>,
    parent: Option<TypeEnv>,
    interner: SharedTypeInterner,
}

pub struct TypeEnv(Rc<TypeEnvInner>);
```

See [Type Environment](type-environment.md) for details on the parent chain design.

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

**Convenience methods** (built on shared `deduplicate_type()` helper):

All convenience methods delegate to `deduplicate_type(origin_id, targs, make_type)`, which handles the hash lookup, deduplication check, and caching. Named origin constants (`LIST_ORIGIN`, `OPTION_ORIGIN`, etc.) replace magic numbers:

```rust
impl TypeContext {
    // Shared deduplication helper
    fn deduplicate_type(&mut self, origin_id: u32, targs: Vec<Type>,
                        make_type: impl FnOnce() -> Type) -> Type;

    // Each method is a thin wrapper around deduplicate_type()
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

InferenceContext uses TypeId internally for O(1) equality, with Type conversion at API boundaries:

```rust
pub struct InferenceContext {
    next_var: u32,
    substitutions: HashMap<TypeVar, TypeId>,  // Internal: TypeId-based
    type_context: TypeContext,
    interner: SharedTypeInterner,             // Shared type interner
}

impl InferenceContext {
    // Construction
    pub fn new() -> Self;                                    // New interner
    pub fn with_interner(interner: SharedTypeInterner) -> Self;  // Shared

    // Type variable management
    pub fn fresh_var(&mut self) -> Type;         // External API
    pub fn fresh_var_id(&mut self) -> TypeId;    // Internal API

    // Unification (external API accepts Type)
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError>;

    // Unification (internal API uses TypeId for O(1) fast-path)
    pub fn unify_ids(&mut self, id1: TypeId, id2: TypeId) -> Result<(), TypeError>;

    // Resolution
    pub fn resolve(&self, ty: &Type) -> Type;      // External
    pub fn resolve_id(&self, id: TypeId) -> TypeId;  // Internal

    // Type construction (uses TypeContext for deduplication)
    pub fn make_list(&mut self, elem: Type) -> Type;
    pub fn make_option(&mut self, inner: Type) -> Type;
    pub fn make_result(&mut self, ok: Type, err: Type) -> Type;
    // ... other make_* methods
}
```

**TypeId-based unification provides O(1) fast-path:**

```rust
pub fn unify_ids(&mut self, id1: TypeId, id2: TypeId) -> Result<(), TypeError> {
    // O(1) fast path: identical TypeIds always unify
    if id1 == id2 {
        return Ok(());
    }
    // ... full structural unification if needed
}
```

Usage in type inference:

```rust
// Instead of:
Type::List(Box::new(elem_type))

// Use:
checker.ctx.make_list(elem_type)
```

### TypeIdFolder and TypeIdVisitor

The `TypeIdFolder` and `TypeIdVisitor` traits provide traversal for interned types:

```rust
/// Transform interned types via structural recursion.
pub trait TypeIdFolder {
    fn interner(&self) -> &TypeInterner;
    fn fold(&mut self, id: TypeId) -> TypeId;
    fn fold_var(&mut self, var: TypeVar) -> TypeId;
    fn fold_function(&mut self, params: &[TypeId], ret: TypeId) -> TypeId;
    // ... other fold_* methods
}

/// Visit interned types without modification.
pub trait TypeIdVisitor {
    fn interner(&self) -> &TypeInterner;
    fn visit(&mut self, id: TypeId);
    fn visit_var(&mut self, var: TypeVar);
    // ... other visit_* methods
}
```

Example: Resolving type variables with TypeIdFolder:

```rust
struct TypeIdResolver<'a> {
    interner: &'a TypeInterner,
    substitutions: &'a HashMap<TypeVar, TypeId>,
}

impl TypeIdFolder for TypeIdResolver<'_> {
    fn interner(&self) -> &TypeInterner { self.interner }

    fn fold_var(&mut self, var: TypeVar) -> TypeId {
        if let Some(&resolved) = self.substitutions.get(&var) {
            self.fold(resolved)  // Recursively resolve
        } else {
            self.interner.intern(TypeData::Var(var))
        }
    }
}
```

These traits should be preferred over `TypeFolder`/`TypeVisitor` for new code as they enable O(1) equality comparisons and better cache locality.

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
42      : int
3.14    : float
"hello" : str
true    : bool
'a'     : char
0xFF    : byte (when used with byte context)
[]      : [T]  (fresh T)
()      : ()   (unit)
```

### Binary Operations

Arithmetic and bitwise operators are type-checked through operator traits. The type checker first attempts primitive operation checking, then falls back to trait-based dispatch for user-defined types.

```
int + int       -> int       (primitive fast path)
float + float   -> float     (primitive fast path)
str + str       -> str       (primitive fast path)
int < int       -> bool      (comparison via Comparable)
T == T          -> bool      (where T: Eq)
T + U           -> T::Output (where T: Add<U>)
```

**Operator Trait Dispatch** (in `infer/expressions/operators.rs`):

1. Try primitive operation checking via `check_binary_operation()`
2. If the left operand is a primitive type and the check fails, report error
3. For user-defined types, look up the trait method (e.g., `Add.add`)
4. Unify the right operand with the method's `rhs` parameter type
5. Return the method's return type (typically an associated `Output` type)

```rust
fn check_binary_op(checker, op, left, right, span) -> Type {
    // Try primitive fast path
    match check_binary_operation(op, left, right) {
        TypeOpResult::Ok(ty) => return ty,
        TypeOpResult::Err(e) if is_primitive_type(left) => {
            checker.push_error(e);
            return Type::Error;
        }
        _ => {} // Continue to trait lookup
    }

    // Trait-based dispatch for user-defined types
    if let Some((trait_name, method_name)) = binary_op_to_trait(op) {
        if let Some(result_ty) = check_operator_trait(checker, left, right, trait_name, method_name, span) {
            return result_ty;
        }
    }

    // No trait impl found
    checker.push_error("type does not implement the required operator trait");
    Type::Error
}
```

### Conditionals

```
if cond then t else e
  cond : bool
  t : T
  e : T
  result : T
```

### Functions

```
@add (a: int, b: int) -> int
  a, b : int
  body : int
  function : (int, int) -> int
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

A `TYPECK_BUILTIN_METHODS` constant in `builtin_methods/mod.rs` exports a sorted list of all `(type_name, method_name)` pairs. A cross-crate consistency test in `oric` verifies that the evaluator's `EVAL_BUILTIN_METHODS` is a subset of this list, catching drift between the two crates.

## Related Documents

- [Type Inference](type-inference.md) - Hindley-Milner inference
- [Unification](unification.md) - Constraint solving
- [Type Environment](type-environment.md) - Scope tracking
- [Type Registry](type-registry.md) - User-defined types
