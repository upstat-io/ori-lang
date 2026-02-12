---
title: "Pattern System Overview"
description: "Ori Compiler Design — Pattern System Overview"
order: 600
section: "Pattern System"
---

# Pattern System Overview

Ori's pattern system provides compiler-level constructs for control flow, concurrency, and resource management. These are distinct from regular function calls because they require special syntax or static analysis.

## Lean Core, Rich Libraries

The pattern system follows the "Lean Core, Rich Libraries" principle:

| In Compiler | In Stdlib |
|-------------|-----------|
| `run`, `try`, `match` (bindings, early return) | `map`, `filter`, `fold`, `find` (collection methods) |
| `recurse` (self-referential `self()`) | `retry`, `validate` (library functions) |
| `parallel`, `spawn`, `timeout` (concurrency) | |
| `cache`, `with` (capability-aware resources) | |

Data transformation moved to stdlib because `items.map(transform: fn)` is just a method call—no special compiler support needed. The compiler focuses on constructs that genuinely require special handling.

## Location

```
compiler/ori_patterns/src/
├── lib.rs              # Core interfaces and re-exports
├── registry.rs         # Pattern registration
├── signature.rs        # Pattern signatures
├── errors.rs           # Pattern errors
├── builtins/           # Built-in patterns
│   ├── mod.rs              # Re-exports
│   ├── print.rs            # PrintPattern implementation
│   ├── panic.rs            # PanicPattern implementation (returns Never)
│   ├── catch.rs            # CatchPattern implementation
│   ├── todo.rs             # TodoPattern implementation (returns Never)
│   └── unreachable.rs      # UnreachablePattern implementation (returns Never)
├── recurse.rs          # recurse pattern
├── parallel.rs         # parallel pattern
├── spawn.rs            # spawn pattern
├── timeout.rs          # timeout pattern
├── cache.rs            # cache pattern
├── with_pattern.rs     # with pattern (RAII resource management)
└── value/              # Runtime value system
    ├── mod.rs              # Value enum and factory methods
    ├── heap.rs             # Heap<T> wrapper for Arc enforcement
    └── composite.rs        # FunctionValue, StructValue, RangeValue
```

## Design Goals

1. **Minimal** - Only what requires compiler support
2. **Declarative** - Express intent, not mechanism
3. **Composable** - Patterns can be combined
4. **Extensible** - New patterns can be added via registry

## Compiler Pattern Categories

### Control Flow (function_seq)

```ori
run(expr1, expr2, result)              // Sequential execution
try(expr?, Ok(value))                  // Error propagation
match(value, pat -> expr, ...)         // Pattern matching
```

### Recursion

```ori
recurse(
    cond: base_case,
    base: value,
    step: self(n - 1) * n,
)
```

### Concurrency

```ori
parallel(tasks: [...], max_concurrent: 4)  // Concurrent execution
spawn(tasks: [...])                        // Fire and forget
timeout(op: expr, after: 5s)               // Time limit
```

### Resource Management

```ori
cache(key: k, op: expensive(), ttl: 5m)    // Requires Cache capability
with(acquire: resource, use: r -> use(r), release: r -> cleanup(r))
```

The `with` pattern provides RAII-style resource management. The `release` function is always called, even if `use` panics.

## Pattern Registry

The `PatternRegistry` uses a `Pattern` enum for static dispatch, avoiding both HashMap overhead and trait object indirection:

```rust
pub enum Pattern {
    Recurse(RecursePattern),
    Parallel(ParallelPattern),
    Spawn(SpawnPattern),
    Timeout(TimeoutPattern),
    Cache(CachePattern),
    With(WithPattern),
    Print(PrintPattern),
    Panic(PanicPattern),
    Catch(CatchPattern),
    Todo(TodoPattern),
    Unreachable(UnreachablePattern),
}

pub struct PatternRegistry {
    _private: (),  // Marker to prevent external construction
}

impl PatternRegistry {
    /// Get the pattern for a given kind.
    /// Returns a concrete Pattern enum value (static dispatch).
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            FunctionExpKind::Recurse => Pattern::Recurse(RecursePattern),
            FunctionExpKind::Parallel => Pattern::Parallel(ParallelPattern),
            FunctionExpKind::Spawn => Pattern::Spawn(SpawnPattern),
            FunctionExpKind::Timeout => Pattern::Timeout(TimeoutPattern),
            FunctionExpKind::Cache => Pattern::Cache(CachePattern),
            FunctionExpKind::With => Pattern::With(WithPattern),
            FunctionExpKind::Print => Pattern::Print(PrintPattern),
            FunctionExpKind::Panic => Pattern::Panic(PanicPattern),
            FunctionExpKind::Catch => Pattern::Catch(CatchPattern),
            FunctionExpKind::Todo => Pattern::Todo(TodoPattern),
            FunctionExpKind::Unreachable => Pattern::Unreachable(UnreachablePattern),
        }
    }
}
```

The `Pattern` enum implements `PatternDefinition` by delegating to each inner type's implementation. Pattern variants are ZSTs created inline in match arms -- no static instances or heap allocation needed. This provides:
- Static dispatch (no vtable indirection)
- Zero heap allocation overhead
- Direct dispatch (no HashMap lookup)
- Exhaustive matching enforced by compiler

## Pattern Interface

All patterns implement the `PatternDefinition` trait:

```rust
pub trait PatternDefinition: Send + Sync {
    /// Pattern name (e.g., "recurse", "parallel")
    fn name(&self) -> &'static str;

    /// Required property names (e.g., ["condition", "base", "step"])
    fn required_props(&self) -> &'static [&'static str];

    /// Optional property names
    fn optional_props(&self) -> &'static [&'static str] { &[] }

    /// Optional arguments with default values
    fn optional_args(&self) -> &'static [OptionalArg] { &[] }

    /// Scoped bindings (e.g., `self` in recurse step)
    fn scoped_bindings(&self) -> &'static [ScopedBinding] { &[] }

    /// Whether this pattern allows arbitrary additional properties
    fn allows_arbitrary_props(&self) -> bool { false }

    /// Evaluate using EvalContext and PatternExecutor
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult;

    /// Check if this pattern can fuse with another
    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool { false }

    /// Create fused pattern if possible
    fn fuse_with(&self, next: &dyn PatternDefinition,
                 self_ctx: &EvalContext, next_ctx: &EvalContext) -> Option<FusedPattern> { None }
}
```

Note: Type checking is handled by `ori_types`, not by patterns themselves. The `PatternDefinition` trait focuses on metadata (property declarations, scoped bindings) and evaluation. Uses `EvalContext` for property access and `PatternExecutor` for evaluation abstraction, not raw `Evaluator`.

## Pattern Resolution

The `pattern_resolution.rs` module in `ori_ir` defines types for type-checker to evaluator communication:

```rust
/// Key identifying a match pattern in the AST.
pub enum PatternKey {
    Arm(u32),    // Top-level arm pattern
    Nested(u32), // Nested pattern via MatchPatternId
}

/// Type-checker resolution of an ambiguous Binding pattern.
pub enum PatternResolution {
    UnitVariant { type_name: Name, variant_index: u8 },
}
```

These types bridge the type checker and evaluator: the type checker produces `PatternResolution` entries keyed by `PatternKey`, and the evaluator consumes them to disambiguate `Binding` patterns that could be either variable bindings or unit variants. Lives in `ori_ir` because both `ori_types` and `ori_eval` depend on it.

## Iterable Helpers

The `Iterable` enum (List or Range) provides a shared `iter_values()` method that abstracts over iteration:

```rust
impl Iterable {
    fn iter_values(&self) -> Box<dyn Iterator<Item = Value> + '_> {
        match self {
            Iterable::List(list) => Box::new(list.iter().cloned()),
            Iterable::Range(range) => Box::new(range.iter().map(Value::int)),
        }
    }
}
```

The `map_values`, `filter_values`, `fold_values`, and `find_value` methods all use `iter_values()`, eliminating duplicated match arms across each operation.

## Usage Example

```ori
// Compiler patterns: special syntax for concurrency
let results = parallel(
    tasks: [fetch(url1), fetch(url2), fetch(url3)],
    max_concurrent: 2,
    timeout: 5s,
)

// Stdlib methods: regular method calls for data transformation
let doubled = items.map(transform: x -> x * 2)
let positives = items.filter(predicate: x -> x > 0)
let sum = items.fold(initial: 0, op: (acc, x) -> acc + x)
```

## Match Pattern Representation

The `MatchPattern` enum represents patterns used in `match` expressions. Variant patterns use `Vec<MatchPattern>` for inner patterns, enabling unit, single-field, and multi-field variants with a uniform representation.

### Variant Pattern AST

```rust
pub enum MatchPattern {
    Variant {
        name: Name,
        inner: Vec<MatchPattern>,  // Not Option<Box<MatchPattern>>
    },
    // ... other patterns
}
```

**Key Design Decision:** Using `Vec<MatchPattern>` instead of `Option<Box<MatchPattern>>` enables:
- Unit variants: `None` → `inner: []`
- Single-field: `Some(x)` → `inner: [Binding("x")]`
- Multi-field: `Click(x, y)` → `inner: [Binding("x"), Binding("y")]`
- Nested: `Event(Click(x, _))` → `inner: [Variant { name: "Click", inner: [...] }]`

**Variant vs Binding Disambiguation:** Uppercase pattern names are treated as variant constructors, lowercase as bindings:
- `Some` → variant pattern (matches `Value::Variant { name: "Some", ... }`)
- `x` → binding pattern (binds value to `x`)

### Type Checking Variant Patterns

The type checker uses `get_variant_field_types()` in `pattern_types.rs` to extract expected field types:

```rust
fn get_variant_field_types(
    type_registry: &TypeRegistry,
    sum_type: &Type,
    variant_name: Name,
) -> Vec<Type>
```

This returns a `Vec<Type>` matching the variant's fields, enabling correct unification of each inner pattern with its corresponding field type.

## Pattern Matching Algorithm

### Type Checking Flow

1. **Infer scrutinee type** via `infer_expr(checker, scrutinee)`
2. **For each arm:**
   - Unify pattern with scrutinee type via `unify_pattern_with_scrutinee()`
   - Extract bindings via `extract_match_pattern_bindings()` returning `Vec<(Name, Type)>`
   - Type-check guard expression (must be `bool`)
   - Unify arm body type with result type
3. **Result type:** Common type from all arm bodies

### Binding Extraction

The `extract_match_pattern_bindings()` function recursively extracts variable bindings:

| Pattern | Bindings |
|---------|----------|
| `Wildcard`, `Literal`, `Range` | None |
| `Binding(name)` | `[(name, scrutinee_ty)]` |
| `Variant`, `Struct`, `Tuple`, `List` | Recursive from nested patterns |
| `Or` | From first alternative (all alternatives must have same bindings) |
| `At { name, pattern }` | Both outer name and inner pattern bindings |

### Runtime Matching

`try_match()` returns `Ok(Some(bindings))` on match, `Ok(None)` on no-match:

```rust
pub fn try_match(
    pattern: &MatchPattern,
    value: &Value,
    arena: &ExprArena,
    interner: &StringInterner,
) -> Result<Option<Vec<(Name, Value)>>, EvalError>
```

### Guard Expressions

Guards (`.match(expr)`) are evaluated after pattern match succeeds but before the arm body. If the guard returns false, matching continues to the next arm.

**Exhaustiveness:** Guards are NOT considered for exhaustiveness checking—the compiler cannot statically verify guard conditions.

## Related Documents

- [Pattern Trait](pattern-trait.md) - PatternDefinition interface
- [Pattern Registry](pattern-registry.md) - Registration system
- [Pattern Fusion](pattern-fusion.md) - Fusion optimization (FusedPattern enum)
- [Adding Patterns](adding-patterns.md) - How to add new patterns
