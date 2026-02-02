---
title: "Pattern Registry"
description: "Ori Compiler Design — Pattern Registry"
order: 603
section: "Pattern System"
---

# Pattern Registry

The PatternRegistry provides pattern lookup via direct enum dispatch with static pattern instances.

## Location

```
compiler/ori_patterns/src/registry.rs
```

## Architecture

The registry uses **compile-time enum dispatch** rather than a dynamic HashMap:

```rust
// Static ZST pattern instances for 'static lifetime references
static RECURSE: RecursePattern = RecursePattern;
static PARALLEL: ParallelPattern = ParallelPattern;
static SPAWN: SpawnPattern = SpawnPattern;
static TIMEOUT: TimeoutPattern = TimeoutPattern;
static CACHE: CachePattern = CachePattern;
static WITH: WithPattern = WithPattern;
static PRINT: PrintPattern = PrintPattern;
static PANIC: PanicPattern = PanicPattern;
static CATCH: CatchPattern = CatchPattern;
static TODO: TodoPattern = TodoPattern;
static UNREACHABLE: UnreachablePattern = UnreachablePattern;

pub struct PatternRegistry {
    _private: (),  // Marker to prevent external construction
}
```

## Lookup

Pattern lookup is a direct enum match, not a HashMap lookup:

```rust
impl PatternRegistry {
    /// Get the pattern definition for a given kind.
    /// Returns a static reference to avoid borrow issues.
    pub fn get(&self, kind: FunctionExpKind) -> &'static dyn PatternDefinition {
        match kind {
            FunctionExpKind::Recurse => &RECURSE,
            FunctionExpKind::Parallel => &PARALLEL,
            FunctionExpKind::Spawn => &SPAWN,
            FunctionExpKind::Timeout => &TIMEOUT,
            FunctionExpKind::Cache => &CACHE,
            FunctionExpKind::With => &WITH,
            FunctionExpKind::Print => &PRINT,
            FunctionExpKind::Panic => &PANIC,
            FunctionExpKind::Catch => &CATCH,
            FunctionExpKind::Todo => &TODO,
            FunctionExpKind::Unreachable => &UNREACHABLE,
        }
    }

    /// Get all pattern kinds.
    pub fn all_kinds(&self) -> &'static [FunctionExpKind] {
        &[
            FunctionExpKind::Recurse,
            FunctionExpKind::Parallel,
            FunctionExpKind::Spawn,
            // ...
        ]
    }
}
```

## Benefits of Static Dispatch

1. **Zero heap allocation** - All patterns are ZSTs
2. **No HashMap overhead** - Direct enum match
3. **No borrow issues** - Static lifetime references
4. **Exhaustive matching** - Compiler ensures all patterns handled
5. **Optimal performance** - Patterns can be inlined

## Registered Patterns

| Pattern | Purpose | Required Props |
|---------|---------|----------------|
| `recurse` | Self-referential recursion | `condition`, `base`, `step` |
| `parallel` | Concurrent execution | `tasks` |
| `spawn` | Fire-and-forget tasks | `tasks` |
| `timeout` | Time-limited execution | `op`, `after` |
| `cache` | Capability-aware caching | `key`, `op` |
| `with` | RAII resource management | `acquire`, `use`, `release` |
| `print` | Debug output | `msg` |
| `panic` | Trigger panic (returns Never) | `msg` |
| `catch` | Catch panics | `expr` |
| `todo` | Unimplemented marker (returns Never) | - |
| `unreachable` | Unreachable marker (returns Never) | - |

## Usage in Type Checker

```rust
impl TypeChecker {
    fn infer_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: &[NamedProp],
    ) -> Type {
        // Get pattern from registry
        let pattern = self.pattern_registry.get(kind);

        // Build type check context
        let ctx = TypeCheckContext::new(
            self.interner,
            &prop_types,
            // ...
        );

        // Delegate to pattern's type checking
        pattern.type_check(&mut ctx)
    }
}
```

## Usage in Evaluator

```rust
impl Evaluator {
    fn eval_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: &[NamedProp],
    ) -> EvalResult {
        // Get pattern from registry
        let pattern = self.pattern_registry.get(kind);

        // Build evaluation context
        let ctx = EvalContext::new(&props, span);

        // Delegate to pattern's evaluation
        pattern.evaluate(&ctx, self)
    }
}
```

## Adding New Patterns

To add a new pattern:

1. **Add enum variant** to `FunctionExpKind` in `ori_ir/src/ast/patterns/exp.rs`
2. **Create pattern struct** in `ori_patterns/src/` (usually a ZST)
3. **Implement `PatternDefinition`** trait
4. **Add static instance** to `registry.rs`
5. **Add match arm** to `get()` method
6. **Update parser** to recognize the pattern name

```rust
// 1. In ori_ir (FunctionExpKind enum)
pub enum FunctionExpKind {
    // ... existing variants
    MyPattern,
}

// 2-3. In ori_patterns (new file my_pattern.rs)
pub struct MyPattern;

impl PatternDefinition for MyPattern {
    fn name(&self) -> &'static str { "my_pattern" }
    fn required_props(&self) -> &'static [&'static str] { &["arg1"] }
    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type { ... }
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult { ... }
}

// 4-5. In registry.rs
static MY_PATTERN: MyPattern = MyPattern;

pub fn get(&self, kind: FunctionExpKind) -> &'static dyn PatternDefinition {
    match kind {
        // ...
        FunctionExpKind::MyPattern => &MY_PATTERN,
    }
}
```

## Design Decision: Why Not HashMap?

The previous design considered a `HashMap<Name, Arc<dyn PatternDefinition>>` but this was rejected because:

1. **Fixed set** - Patterns are known at compile time
2. **Performance** - Enum dispatch is faster than HashMap lookup
3. **No plugins** - Users don't add patterns at runtime
4. **Exhaustiveness** - Compiler catches missing pattern handlers
5. **Memory** - ZSTs have zero runtime cost

The enum dispatch approach aligns with Ori's "prefer enum for fixed sets" design principle.

## Thread Safety

The registry is inherently thread-safe because:
- All patterns are static references
- Pattern instances are ZSTs (no mutable state)
- Registry itself is immutable after construction

```rust
// Safe to use from multiple threads
let pattern = registry.get(FunctionExpKind::Parallel);
// pattern is &'static dyn PatternDefinition
```
