---
title: "Adding New Patterns"
description: "Ori Compiler Design — Adding New Patterns"
order: 601
section: "Pattern System"
---

# Adding New Patterns

This guide explains how to add new patterns to the Ori compiler.

## Overview

To add a new pattern:

1. Create pattern struct implementing `PatternDefinition`
2. Register pattern in `PatternRegistry`
3. Add tests
4. Update documentation

## Step 1: Create Pattern Struct

Create a new file in `compiler/oric/src/patterns/`:

```rust
// patterns/take.rs

use crate::patterns::{PatternDefinition, PatternArg, ArgType, TypedArg, EvalArg};
use crate::types::Type;
use crate::eval::Value;

/// take(over: items, count: n) - Take first n items
pub struct TakePattern;

impl PatternDefinition for TakePattern {
    fn name(&self) -> &str {
        "take"
    }

    fn arguments(&self) -> &[PatternArg] {
        &[
            PatternArg {
                name: "over",
                ty: ArgType::Iterable,
                required: true,
                default: None,
            },
            PatternArg {
                name: "count",
                ty: ArgType::Exact(Type::Int),
                required: true,
                default: None,
            },
        ]
    }

    fn type_check(
        &self,
        args: &[TypedArg],
        checker: &mut TypeChecker,
    ) -> Result<Type, TypeError> {
        let over_arg = args.iter().find(|a| a.name.as_str() == "over")
            .ok_or(TypeError::MissingArg("over"))?;
        let count_arg = args.iter().find(|a| a.name.as_str() == "count")
            .ok_or(TypeError::MissingArg("count"))?;

        // Verify count is int
        checker.unify(&count_arg.ty, &Type::Int)?;

        // Result has same type as input
        Ok(over_arg.ty.clone())
    }

    fn evaluate(
        &self,
        args: &[EvalArg],
        _evaluator: &mut Evaluator,
    ) -> Result<Value, EvalError> {
        let over = args.iter().find(|a| a.name.as_str() == "over")
            .ok_or(EvalError::MissingArg("over"))?
            .value.as_list()?;

        let count = args.iter().find(|a| a.name.as_str() == "count")
            .ok_or(EvalError::MissingArg("count"))?
            .value.as_int()? as usize;

        let result: Vec<Value> = over.iter()
            .take(count)
            .cloned()
            .collect();

        Ok(Value::List(Arc::new(result)))
    }
}
```

## Step 2: Add to Module

Update `patterns/mod.rs`:

```rust
mod take;

pub use take::TakePattern;
```

## Step 3: Register Pattern

Update `patterns/registry.rs`:

```rust
impl PatternRegistry {
    pub fn with_builtins(interner: Arc<Interner>) -> Self {
        let mut registry = Self::new(interner);

        // ... existing patterns ...

        // Add new pattern
        registry.register(TakePattern);

        registry
    }
}
```

## Step 4: Add Tests

Create `patterns/take_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_basic() {
        let result = eval("take(over: [1, 2, 3, 4, 5], count: 3)");
        assert_eq!(result, Value::List(vec![1, 2, 3].into()));
    }

    #[test]
    fn test_take_empty() {
        let result = eval("take(over: [], count: 3)");
        assert_eq!(result, Value::List(vec![].into()));
    }

    #[test]
    fn test_take_more_than_available() {
        let result = eval("take(over: [1, 2], count: 5)");
        assert_eq!(result, Value::List(vec![1, 2].into()));
    }

    #[test]
    fn test_take_type_error() {
        let err = eval_err("take(over: [1, 2], count: \"not a number\")");
        assert!(matches!(err, TypeError::Mismatch { .. }));
    }
}
```

## Step 5: Add Documentation

Update `docs/ori_lang/0.1-alpha/spec/10-patterns.md`:

```markdown
### take

Takes the first n elements from a collection.

**Signature:**
```
take(over: [T], count: int) -> [T]
```

**Arguments:**
- `.over` - Collection to take from
- `.count` - Number of elements to take

**Example:**
```ori
take(over: [1, 2, 3, 4, 5], count: 3)  // [1, 2, 3]
```
```

## Optional: Add Fusion Support

If your pattern can fuse with others:

```rust
impl PatternDefinition for TakePattern {
    // ... other methods ...

    fn can_fuse(&self, next: &dyn PatternDefinition) -> bool {
        // take can fuse with map
        next.name() == "map"
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
    ) -> Option<Box<dyn PatternDefinition>> {
        if next.name() == "map" {
            Some(Box::new(TakeMapPattern::new(self, next)))
        } else {
            None
        }
    }
}
```

## Pattern Categories

### Data Patterns (function_exp)

Return transformed data:

```rust
// map, filter, fold, take, skip, etc.
fn evaluate(&self, args: &[EvalArg], eval: &mut Evaluator) -> Result<Value, EvalError> {
    // Transform input and return result
    Ok(transformed_value)
}
```

### Control Patterns (function_seq)

Control execution flow:

```rust
// run, try, match
fn evaluate(&self, args: &[EvalArg], eval: &mut Evaluator) -> Result<Value, EvalError> {
    // Execute in sequence, handle control flow
    for expr in sequence {
        eval.eval_expr(expr)?;
    }
    Ok(final_value)
}
```

### Effect Patterns

Require capabilities:

```rust
impl PatternDefinition for HttpGetPattern {
    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::Http]
    }

    fn evaluate(&self, args: &[EvalArg], eval: &mut Evaluator) -> Result<Value, EvalError> {
        // Verify capability is available
        eval.require_capability(Capability::Http)?;

        // Perform HTTP request
        let url = args.get("url")?.as_string()?;
        let response = eval.http_client()?.get(&url)?;
        Ok(Value::String(response))
    }
}
```

## Checklist

Before submitting:

- [ ] Pattern struct implements `PatternDefinition`
- [ ] Registered in `PatternRegistry::with_builtins`
- [ ] Type checking validates all arguments
- [ ] Evaluation handles edge cases
- [ ] Unit tests cover basic usage
- [ ] Unit tests cover error cases
- [ ] Documentation added to spec
- [ ] Example added to guide (optional)
- [ ] Fusion rules defined (if applicable)
- [ ] Capabilities declared (if applicable)

## Common Mistakes

1. **Forgetting required arguments** - Always check required args in type_check and evaluate
2. **Wrong return type** - Ensure type_check returns same type as evaluate produces
3. **Not handling empty input** - Test with empty lists/collections
4. **Missing capability check** - Declare and verify capabilities for effects
5. **Thread safety** - Ensure pattern is `Send + Sync`
