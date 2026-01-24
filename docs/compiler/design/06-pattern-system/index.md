# Pattern System Overview

Sigil's pattern system provides first-class constructs for common programming patterns like iteration, transformation, and concurrency. Patterns replace traditional loops with declarative, composable operations.

## Location

```
compiler/sigilc/src/patterns/
├── mod.rs          # Core interfaces (~505 lines)
├── registry.rs     # Pattern registration (~289 lines)
├── builtins.rs     # Built-in patterns (~531 lines)
├── fusion.rs       # Optimization (~420 lines)
├── map.rs          # map pattern
├── filter.rs       # filter pattern
├── fold.rs         # fold pattern
├── find.rs         # find pattern
├── collect.rs      # collect pattern
├── recurse.rs      # recurse pattern
├── parallel.rs     # parallel pattern
├── spawn.rs        # spawn pattern
├── timeout.rs      # timeout pattern
├── retry.rs        # retry pattern
├── cache.rs        # cache pattern
└── validate.rs     # validate pattern
```

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
