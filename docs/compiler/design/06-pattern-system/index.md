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
├── lib.rs          # Core interfaces and re-exports
├── registry.rs     # Pattern registration
├── signature.rs    # Pattern signatures
├── errors.rs       # Pattern errors
├── builtins/       # Built-in patterns
│   ├── mod.rs          # Re-exports
│   ├── print.rs        # PrintPattern implementation
│   └── panic.rs        # PanicPattern implementation
├── recurse.rs      # recurse pattern
├── parallel.rs     # parallel pattern
├── spawn.rs        # spawn pattern
├── timeout.rs      # timeout pattern
├── cache.rs        # cache pattern
├── with_pattern.rs # with pattern (RAII resource management)
└── value/          # Runtime value system
    ├── mod.rs          # Value enum and factory methods
    ├── heap.rs         # Heap<T> wrapper for Arc enforcement
    └── composite.rs    # FunctionValue, StructValue, RangeValue
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

## Pattern Interface

All patterns implement the `PatternDefinition` trait:

```rust
pub trait PatternDefinition: Send + Sync {
    /// Pattern name (e.g., "map", "filter")
    fn name(&self) -> &str;

    /// Expected arguments
    fn arguments(&self) -> &[PatternArg];

    /// Type check the pattern
    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError>;

    /// Evaluate the pattern
    fn evaluate(
        &self,
        args: &[EvalArg],
        evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError>;

    /// Optional: fuse with following pattern
    fn fuse_with(&self, _next: &dyn PatternDefinition) -> Option<Box<dyn PatternDefinition>> {
        None
    }
}
```

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

## Related Documents

- [Pattern Trait](pattern-trait.md) - PatternDefinition interface
- [Pattern Registry](pattern-registry.md) - Registration system
- [Pattern Fusion](pattern-fusion.md) - Optimization passes
- [Adding Patterns](adding-patterns.md) - How to add new patterns
