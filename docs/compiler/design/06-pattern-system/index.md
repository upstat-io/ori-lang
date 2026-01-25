# Pattern System Overview

Sigil's pattern system provides first-class constructs for common programming patterns like iteration, transformation, and concurrency. Patterns replace traditional loops with declarative, composable operations.

## Location

```
compiler/sigil_patterns/src/
├── lib.rs          # Core interfaces and re-exports
├── registry.rs     # Pattern registration
├── signature.rs    # Pattern signatures
├── errors.rs       # Pattern errors
├── builtins/       # Built-in patterns
│   ├── mod.rs          # Re-exports
│   ├── print.rs        # PrintPattern implementation
│   └── panic.rs        # PanicPattern implementation
├── fusion.rs       # Pattern fusion optimization
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

**Note:** Following the "Lean Core, Rich Libraries" principle, most built-in
patterns like `assert`, `len`, `compare`, `min`, `max` have been moved to the
standard library. Only `print` and `panic` remain as compiler builtins.

## Design Goals

1. **Declarative** - Express intent, not mechanism
2. **Composable** - Patterns can be combined
3. **Optimizable** - Fusion eliminates intermediate data
4. **Extensible** - New patterns can be added via registry

## Pattern Categories

### Data Transformation (function_exp)

```sigil
map(over: items, transform: fn)      // Transform each element
filter(over: items, predicate: fn)   // Keep matching elements
fold(over: items, init: val, op: fn) // Reduce to single value
find(over: items, where: fn)         // Find first match
collect(range: 0..10, transform: fn) // Generate from range
```

### Control Flow (function_seq)

```sigil
run(expr1, expr2, result)              // Sequential execution
try(expr?, Ok(value))                  // Error propagation
match(value, pat -> expr, ...)         // Pattern matching
```

### Recursion

```sigil
recurse(
    cond: base_case,
    base: value,
    step: self(n - 1) * n,
)
```

### Concurrency

```sigil
parallel(tasks: [...], max_concurrent: 4)  // Concurrent execution
spawn(tasks: [...])                          // Fire and forget
timeout(op: expr, after: 5s)               // Time limit
retry(op: expr, attempts: 3)               // Retry on failure
```

### Resource Management

```sigil
with(acquire: resource, action: r -> use(r), release: r -> cleanup(r))
```

The `with` pattern provides RAII-style resource management. The `release` function is always called, even if `action` throws.

### Caching and Validation

```sigil
cache(key: k, op: expensive(), ttl: 5m)
validate(rules: [...], then: value)
```

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

```sigil
// Transform and filter a list
let result = filter(
    over: map(
        over: items,
        transform: x -> x * 2,
    ),
    predicate: x -> x > 10,
)
```

With fusion optimization:
```
map -> filter
  becomes
map_filter (single pass)
```

## Related Documents

- [Pattern Trait](pattern-trait.md) - PatternDefinition interface
- [Pattern Registry](pattern-registry.md) - Registration system
- [Pattern Fusion](pattern-fusion.md) - Optimization passes
- [Adding Patterns](adding-patterns.md) - How to add new patterns
