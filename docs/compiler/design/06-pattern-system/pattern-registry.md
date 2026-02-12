---
title: "Pattern Registry"
description: "Ori Compiler Design — Pattern Registry"
order: 603
section: "Pattern System"
---

# Pattern Registry

The PatternRegistry provides pattern lookup via the `Pattern` enum, using static dispatch instead of trait objects or HashMaps.

## Location

```
compiler/ori_patterns/src/registry.rs
```

## Architecture

The registry uses a **`Pattern` enum** as the central dispatch point. Each variant wraps a concrete pattern type (a ZST), and the enum itself implements `PatternDefinition` by delegating to the inner type:

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

impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        match self {
            Pattern::Recurse(p) => p.name(),
            Pattern::Parallel(p) => p.name(),
            // ... delegates to each inner type
        }
    }
    // ... same delegation for all trait methods
}

pub struct PatternRegistry {
    _private: (),  // Marker to prevent external construction
}
```

## Lookup

Pattern lookup returns a concrete `Pattern` enum value -- no trait objects, no HashMap:

```rust
impl PatternRegistry {
    /// Get the pattern for a given kind.
    /// Returns a Pattern enum value (static dispatch, no vtable).
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

    /// Get all registered pattern kinds.
    pub fn kinds(&self) -> impl Iterator<Item = FunctionExpKind> {
        [
            FunctionExpKind::Recurse,
            FunctionExpKind::Parallel,
            FunctionExpKind::Spawn,
            // ...
        ].into_iter()
    }
}
```

Pattern variants are ZSTs created inline in match arms -- no static instances needed.

## Benefits of Pattern Enum Dispatch

1. **Static dispatch** - No vtable indirection; the compiler can inline through enum matches
2. **Zero heap allocation** - All pattern types are ZSTs, Pattern enum is stack-allocated
3. **No HashMap overhead** - Direct enum match on `FunctionExpKind`
4. **Exhaustive matching** - Compiler ensures all patterns are handled
5. **No borrow issues** - Owned values, not references to statics

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

The type checker uses pattern metadata (required props, scoped bindings) from the registry but performs type checking itself in `ori_types`:

```rust
impl TypeChecker {
    fn infer_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: &[NamedProp],
    ) -> Type {
        // Get pattern from registry
        let pattern = self.pattern_registry.get(kind);

        // Use pattern metadata for type checking
        // (type checking logic lives in ori_types, not in patterns)
        // ...
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
4. **Add variant** to `Pattern` enum and implement delegation in each trait method
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
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult { ... }
}

// 4-5. In registry.rs
pub enum Pattern {
    // ...
    MyPattern(MyPatternType),
}

impl PatternRegistry {
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            // ...
            FunctionExpKind::MyPattern => Pattern::MyPattern(MyPatternType),
        }
    }
}
```

## Design Decision: Why Not HashMap?

The previous design considered a `HashMap<Name, Arc<dyn PatternDefinition>>` but this was rejected because:

1. **Fixed set** - Patterns are known at compile time
2. **Performance** - Enum dispatch is faster than HashMap lookup and trait object vtables
3. **No plugins** - Users don't add patterns at runtime
4. **Exhaustiveness** - Compiler catches missing pattern handlers
5. **Memory** - ZSTs have zero runtime cost

The enum dispatch approach aligns with Ori's "prefer enum for fixed sets" design principle.

## Thread Safety

The registry is inherently thread-safe because:
- All pattern types are ZSTs (no mutable state)
- `Pattern` enum values are created on demand, not shared
- Registry itself is immutable after construction
