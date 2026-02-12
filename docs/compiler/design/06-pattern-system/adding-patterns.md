---
title: "Adding New Patterns"
description: "Ori Compiler Design — Adding New Patterns"
order: 601
section: "Pattern System"
---

# Adding New Patterns

This guide explains how to add new patterns to the Ori compiler.

## Overview

Adding a new pattern requires changes across multiple crates:

1. Add enum variant to `FunctionExpKind` in `ori_ir`
2. Create pattern struct implementing `PatternDefinition` in `ori_patterns`
3. Add variant to `Pattern` enum + implement trait delegation + add match arm in registry
4. Update parser to recognize the pattern name
5. Add tests and documentation

## Step 1: Add Enum Variant

In `compiler/ori_ir/src/ast/patterns/exp.rs`:

```rust
/// Kind of function_exp pattern.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum FunctionExpKind {
    Recurse,
    Parallel,
    Spawn,
    Timeout,
    Cache,
    With,
    Print,
    Panic,
    Catch,
    Todo,
    Unreachable,
    Take,  // <-- Add new variant
}
```

## Step 2: Create Pattern Struct

Create a new file in `compiler/ori_patterns/src/`:

```rust
// take.rs

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext};
use ori_types::Type;

/// take(over: items, count: n) - Take first n items
pub struct TakePattern;

impl PatternDefinition for TakePattern {
    fn name(&self) -> &'static str {
        "take"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["over", "count"]
    }

    // Note: type checking is handled by ori_types, not by patterns.
    // The type checker uses required_props() and scoped_bindings() metadata
    // to drive inference for this pattern's properties.

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Evaluate properties
        let over = ctx.eval_prop("over", exec)?;
        let count = ctx.eval_prop("count", exec)?.as_int()? as usize;

        // Get list from over
        let list = over.as_list()?;

        // Take first n elements
        let result: Vec<_> = list.iter()
            .take(count)
            .cloned()
            .collect();

        Ok(Value::list(result))
    }
}
```

## Step 3: Add to Module

Update `compiler/ori_patterns/src/lib.rs`:

```rust
mod take;

pub use take::TakePattern;
```

## Step 4: Register in Registry

Update `compiler/ori_patterns/src/registry.rs`:

```rust
// Add variant to Pattern enum
pub enum Pattern {
    // ... existing variants ...
    Take(TakePattern),
}

// Add delegation in PatternDefinition impl for Pattern
impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        match self {
            // ... existing arms ...
            Pattern::Take(p) => p.name(),
        }
    }
    // ... same for all other trait methods (required_props, evaluate, etc.)
}

// Add match arm in get()
impl PatternRegistry {
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            // ... existing patterns ...
            FunctionExpKind::Take => Pattern::Take(TakePattern),
        }
    }
}
```

## Step 5: Update Parser

In `compiler/ori_parse/src/expr.rs`, add the pattern name to the function_exp parser:

```rust
fn parse_function_exp(&mut self) -> Option<FunctionExpKind> {
    let name = self.expect_identifier()?;
    match name.as_str() {
        "recurse" => Some(FunctionExpKind::Recurse),
        "parallel" => Some(FunctionExpKind::Parallel),
        // ... existing patterns ...
        "take" => Some(FunctionExpKind::Take),
        _ => None,
    }
}
```

## Step 6: Add Tests

Create test file or add to existing tests:

```rust
#[test]
fn test_take_basic() {
    let result = eval("take(over: [1, 2, 3, 4, 5], count: 3)");
    assert_eq!(result, Value::list(vec![1, 2, 3]));
}

#[test]
fn test_take_empty() {
    let result = eval("take(over: [], count: 3)");
    assert_eq!(result, Value::list(vec![]));
}

#[test]
fn test_take_more_than_available() {
    let result = eval("take(over: [1, 2], count: 5)");
    assert_eq!(result, Value::list(vec![1, 2]));
}
```

## Step 7: Add Documentation

Update `docs/ori_lang/0.1-alpha/spec/10-patterns.md`:

```markdown
### take

Takes the first n elements from a collection.

**Signature:**
```
take(over: [T], count: int) -> [T]
```

**Arguments:**
- `.over:` - Collection to take from
- `.count:` - Number of elements to take

**Example:**
```ori
take(over: [1, 2, 3, 4, 5], count: 3)  // [1, 2, 3]
```
```

## Pattern Categories

### function_exp Patterns

Registered patterns using the `PatternDefinition` trait:

```rust
impl PatternDefinition for MyPattern {
    fn name(&self) -> &'static str { "my_pattern" }
    fn required_props(&self) -> &'static [&'static str] { &["arg1", "arg2"] }
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult { ... }
}
```

### function_seq Patterns

Control flow constructs (`run`, `try`, `match`) are NOT in the pattern registry. They are:
- Defined as AST nodes in `ori_ir/src/ast/patterns/seq.rs`
- Type-checked directly in `ori_typeck`
- Evaluated directly in `ori_eval`

Do NOT add control flow patterns to the `PatternRegistry`.

## Optional Features

### Optional Properties

```rust
fn optional_props(&self) -> &'static [&'static str] {
    &["limit", "offset"]
}
```

### Scoped Bindings

For patterns that introduce identifiers (like `recurse` with `self`):

```rust
fn scoped_bindings(&self) -> &'static [ScopedBinding] {
    &[ScopedBinding {
        name: "item",
        for_props: &["transform"],
        type_from: ScopedBindingType::SameAs("over"),
    }]
}
```

### Pattern Fusion

If your pattern can fuse with others for performance:

```rust
fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
    next.name() == "filter"
}

fn fuse_with(
    &self,
    next: &dyn PatternDefinition,
    self_ctx: &EvalContext,
    next_ctx: &EvalContext,
) -> Option<FusedPattern> {
    if next.name() == "filter" {
        Some(FusedPattern::TakeFilter { ... })
    } else {
        None
    }
}
```

## Checklist

Before submitting:

- [ ] Added `FunctionExpKind` variant in `ori_ir`
- [ ] Created pattern struct in `ori_patterns`
- [ ] Implemented `PatternDefinition` trait with correct signatures
- [ ] Added variant to `Pattern` enum with trait delegation + match arm in registry
- [ ] Updated parser to recognize pattern name
- [ ] Type checking in `ori_types` handles the new pattern's properties
- [ ] Evaluation handles edge cases
- [ ] Unit tests cover basic usage
- [ ] Unit tests cover error cases
- [ ] Documentation added to spec
- [ ] All crates compile (`cargo build -p ori_ir -p ori_patterns -p ori_parse`)

## Common Mistakes

1. **Forgetting to update all locations** - Pattern needs changes in `ori_ir`, `ori_patterns`, `ori_parse`
2. **Wrong trait signature** - Use `required_props()` not `arguments()`
3. **Not using context types** - Use `TypeCheckContext`/`EvalContext`, not raw checker/evaluator
4. **Missing parser update** - Pattern won't parse without adding to parser
5. **Thread safety** - All patterns must be `Send + Sync` (ZSTs are automatically)

## Note on Stdlib Methods

Data transformation operations like `map`, `filter`, `fold`, `find`, `take`, `skip` are actually **collection methods** in stdlib, not patterns. They don't require compiler support.

Only add to the pattern registry if your construct genuinely needs:
- Special syntax not expressible as a method call
- Scoped bindings (introducing new identifiers)
- Capability-aware behavior
- Concurrency semantics
- Control flow manipulation
