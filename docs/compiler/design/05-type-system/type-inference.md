---
title: "Type Inference"
description: "Ori Compiler Design — Type Inference"
order: 502
section: "Type System"
---

# Type Inference

Ori uses Hindley-Milner (HM) type inference, extended with rank-based let-polymorphism, capability tracking, and pattern resolution. The `InferEngine` orchestrates inference for individual expressions, while `ModuleChecker` coordinates module-level type checking.

## Location

```
compiler/ori_types/src/infer/
├── mod.rs    # InferEngine struct, configuration, error handling
├── expr.rs   # infer_expr() — per-expression type inference dispatch
└── env.rs    # TypeEnv — scope chain for name resolution
```

## InferEngine

```rust
pub struct InferEngine<'pool> {
    unify: UnifyEngine<'pool>,              // Unification engine (borrows Pool)
    env: TypeEnv,                           // Name → scheme bindings
    expr_types: FxHashMap<ExprIndex, Idx>,  // Expression → inferred type
    context_stack: Vec<ContextKind>,        // For error reporting context
    errors: Vec<TypeCheckError>,            // Accumulated errors

    interner: Option<&'pool StringInterner>,
    trait_registry: Option<&'pool TraitRegistry>,
    signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,
    type_registry: Option<&'pool TypeRegistry>,

    self_type: Option<Idx>,                 // For recursive call patterns
    impl_self_type: Option<Idx>,            // For `Self` in impl blocks
    loop_break_types: Vec<Idx>,             // Stack of break value types
    current_capabilities: FxHashSet<Name>,  // `uses` clause capabilities
    provided_capabilities: FxHashSet<Name>, // `with...in` capabilities
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
}
```

The engine is created fresh for each function body check, receiving the pool, environment, and registries from `ModuleChecker`.

## Expression Inference

The core inference function dispatches on `ExprKind`:

```rust
#[tracing::instrument(level = "trace", skip(engine, arena))]
pub fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx
```

### Dispatch Table

| Expression Kind | Inference Rule |
|----------------|---------------|
| `Literal(Int)` | `Idx::INT` |
| `Literal(Float)` | `Idx::FLOAT` |
| `Literal(Str)` | `Idx::STR` |
| `Literal(Bool)` | `Idx::BOOL` |
| `Ident(name)` | Lookup in `TypeEnv`, instantiate if polymorphic |
| `Binary { op, left, right }` | Infer operands, check operator type rules |
| `Call { func, args }` | Infer function type, unify args with params |
| `FieldAccess { expr, field }` | Infer receiver, look up field/method |
| `Index { expr, index }` | Infer collection type, return element type |
| `If { cond, then, else }` | Cond must be `bool`, unify branch types |
| `Match { scrutinee, arms }` | Infer scrutinee, check patterns, unify arm types |
| `For { var, iter, body }` | Infer iterable element type, bind loop var |
| `Loop { body }` | Fresh break type, infer body |
| `Let { pattern, value, body }` | Infer value, bind pattern, infer body |
| `Lambda { params, body }` | Create function type with fresh param vars |
| `List { elems }` | Unify all elements, return `[T]` |
| `Map { entries }` | Unify all keys and values, return `{K: V}` |
| `Tuple { elems }` | Infer each element, return tuple type |
| `Block { stmts, expr }` | Infer statements in sequence, return last expr |
| `Run { stmts }` | Sequential pattern — infer each, return last |
| `Try { stmts }` | Like run, but wraps in `result` and enables `?` |

### Identifier Resolution

When an identifier is looked up, the result may be a polymorphic type scheme. The engine instantiates it with fresh variables:

```rust
// Lookup returns an Idx which may be a Scheme
let ty = engine.env.lookup(name)?;
// If it's a scheme, instantiate with fresh variables
if engine.pool.tag(ty) == Tag::Scheme {
    engine.unify.instantiate(ty)  // Creates fresh vars for each quantified var
} else {
    ty
}
```

This ensures each use of a polymorphic function gets independent type variables.

## Inference Examples

### Let Binding

```ori
let x = 42
let y = x + 1
```

```
1. x : T0 (fresh var at current rank)
2. 42 : int (literal)
3. unify(T0, int) → Link T0 → int
4. y : T1 (fresh var)
5. x + 1 : lookup(+, int, int) = int
6. unify(T1, int) → Link T1 → int
```

### Generic Function

```ori
@identity<T> (x: T) -> T = x
identity(42)
identity("hello")
```

```
1. identity : forall T. (T) -> T  (scheme with one generalized var)
2. identity(42):
   - Instantiate: (T0) -> T0   (fresh vars)
   - Unify arg: T0 = int       (Link T0 → int)
   - Return: int
3. identity("hello"):
   - Instantiate: (T1) -> T1   (new fresh vars)
   - Unify arg: T1 = str       (Link T1 → str)
   - Return: str
```

### Let Polymorphism

```ori
let id = x -> x
let a = id(42)
let b = id("hello")
```

```
1. Infer lambda at rank 3:
   - x : T0 at rank 3
   - body returns T0
   - type: (T0) -> T0
2. Exit rank 3 — generalize:
   - T0 is unbound at rank 3 → generalize
   - id : forall T. T -> T (scheme)
3. id(42) — instantiate scheme:
   - (T1) -> T1, unify T1 = int → result: int
4. id("hello") — instantiate scheme:
   - (T2) -> T2, unify T2 = str → result: str
```

The rank system ensures that `T0` is generalized correctly — see [Unification](unification.md) for rank details.

### Collection Inference

```ori
let xs = [1, 2, 3]
let ys = xs.map(x -> x * 2)
```

```
1. [1, 2, 3]:
   - Fresh elem var T0
   - Unify T0 = int (from first element)
   - Check remaining elements: all int
   - Result: [int]
2. xs.map(x -> x * 2):
   - Receiver: [int]
   - Method: map<A, B>(self, f: (A) -> B) -> [B]
   - Instantiate: A = int, B = T1
   - Lambda: (int) -> int, so T1 = int
   - Result: [int]
```

## Capability Tracking

Functions declare required capabilities with `uses`:

```ori
@fetch_data (url: str) -> str uses Http = ...
```

The `InferEngine` tracks capabilities in two sets:

- `current_capabilities` — Capabilities declared by the current function's `uses` clause
- `provided_capabilities` — Capabilities injected by `with...in` expressions

When a called function requires a capability, the engine verifies it is available:

```ori
@process () -> str uses Http =
    let data = fetch_data(url: "/api")  // Ok: Http is in current_capabilities
    data

@main () -> void =
    with Http = MockHttp in
        process()  // Ok: Http is provided
```

## Pattern Resolutions

During inference of `match` expressions, the engine records how patterns resolve. This information is needed by the LLVM backend for code generation:

```rust
pub enum PatternResolution {
    UnitVariant {
        type_name: Name,
        variant_index: u8,  // Tag value for LLVM discriminant
    },
}
```

Pattern resolutions are accumulated in `InferEngine::pattern_resolutions` and emitted as part of `TypedModule`.

## Error Accumulation

The engine accumulates errors rather than bailing on the first failure. Each error includes rich context:

```rust
pub struct TypeCheckError {
    pub kind: TypeErrorKind,
    pub span: Span,
    pub context: ErrorContext,
    pub severity: Severity,
    pub suggestions: Vec<Suggestion>,
}
```

The `ErrorContext` tracks the origin of type expectations (e.g., "2nd argument to `foo`", "return type of function") for clear error messages.

## Tracing

The inference engine is instrumented with `tracing` for debugging:

```bash
ORI_LOG=ori_types=trace ori check file.ori          # Per-expression inference
ORI_LOG=ori_types=debug ori check file.ori          # Phase boundaries
ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check f.ori  # Hierarchical call tree
```

Key instrumented functions:
- `infer_expr()` — trace level (per-expression, very verbose)
- `check_module()` — debug level (phase boundaries)
- `collect_signatures()`, `check_function_bodies()` — debug level
