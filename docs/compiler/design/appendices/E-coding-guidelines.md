---
title: "Appendix E: Coding Guidelines"
description: "Ori Compiler Design — Appendix E: Coding Guidelines"
order: 1005
section: "Appendices"
---

# Appendix E: Coding Guidelines

Coding standards and best practices for the Ori compiler codebase.

## Overview

This document establishes coding conventions to maintain consistency, readability, and quality across the Ori compiler. All contributors should follow these guidelines.

---

## 1. Testing

### 1.1 Test Organization

Use a **hybrid approach** following Rust compiler conventions:

| Test Type | Location | When to Use |
|-----------|----------|-------------|
| Inline tests | `#[cfg(test)] mod tests` at file bottom | Small utilities (< 200 lines of tests) |
| Separate test files | `src/<module>/tests/<name>_tests.rs` | Comprehensive suites (> 200 lines) |
| Spec tests | `tests/spec/` | Language specification conformance |
| Run-pass tests | `tests/run-pass/` | End-to-end execution tests |
| Compile-fail tests | `tests/compile-fail/` | Expected compilation failures |

**Example structure:**

```
oric/src/eval/
├── function_val.rs             # Implementation (minimal or no inline tests)
├── operators.rs                # Implementation
├── methods.rs                  # Implementation
└── tests/
    ├── mod.rs                  # Test module declarations
    ├── function_val_tests.rs   # Comprehensive type conversion tests
    ├── operators_tests.rs      # Binary operator tests
    └── methods_tests.rs        # Method dispatch tests
```

### 1.2 Test File Structure

Organize test files with clear sections:

```rust
//! Tests for [module description].
//!
//! [Optional: Source of test cases, e.g., "Adapted from Go's strconv tests"]

use crate::module::under::test;

// =============================================================================
// Category 1
// =============================================================================

mod category_1 {
    use super::*;

    #[test]
    fn descriptive_test_name() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}

// =============================================================================
// Category 2
// =============================================================================

mod category_2 {
    // ...
}
```

### 1.3 Test Naming

- Use descriptive names: `int_from_float_overflow_error` not `test1`
- Group related tests in nested modules: `mod int_ops { ... }`
- Prefix error cases: `*_error`, `*_invalid`, `*_fails`

### 1.4 Test Coverage

**Must test:**
- Happy path (normal operation)
- Edge cases (boundaries, empty inputs, zero values)
- Error conditions (invalid input, overflow, type mismatches)
- Round-trip operations where applicable

**Reference test suites:**
- Go's `strconv` for type conversions
- Rust's `std` tests for collection operations
- IEEE 754 for float behavior

### 1.5 Running Tests

```bash
cargo test --workspace              # All tests
cargo test -p oric                # Single crate
cargo test -- eval::tests           # Specific module
cargo test -- --nocapture           # Show println! output
```

---

## 2. Code Style

### 2.1 Formatting

Use `rustfmt` with default settings. Run before committing:

```bash
cargo fmt --all
```

### 2.2 File Length

| Guideline | Lines |
|-----------|-------|
| Target | ~300 |
| Maximum | 500 |
| Exception | Grammar files may exceed |

Split large files into focused modules.

**Known debt:** At least 11 files in `ori_types` exceed 500 lines (765–2874 lines), concentrated in inference (`infer/expr/`, `infer/mod.rs`), type checking (`check/mod.rs`), unification (`unify/mod.rs`), error reporting (`type_error/check_error/mod.rs`), and the type pool (`pool/mod.rs`). These are known technical debt targeted for splitting as their subsystems stabilize.

### 2.3 Function Length

- Target: < 50 lines
- Maximum: 100 lines
- Extract helper functions for complex logic

### 2.4 Import Organization

Order imports by:

```rust
// 1. Standard library
use std::collections::HashMap;
use std::path::Path;

// 2. External crates
use salsa::Database;

// 3. Workspace crates
use ori_ir::{ExprId, Name};
use ori_patterns::Value;

// 4. Local modules
use crate::eval::Environment;
use super::helpers;
```

### 2.5 Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types | PascalCase | `TypeChecker`, `ParseResult` |
| Functions | snake_case | `eval_expr`, `parse_item` |
| Constants | SCREAMING_SNAKE | `MAX_RECURSION_DEPTH` |
| Modules | snake_case | `type_registry`, `error_codes` |
| Type parameters | Single uppercase | `T`, `E`, `K`, `V` |

---

## 3. Error Handling

### 3.1 Result vs Panic

| Use | When |
|-----|------|
| `Result<T, E>` | Recoverable errors (user input, file I/O) |
| `panic!` | Programming errors, invariant violations |
| `unreachable!()` | Code paths that should never execute |

### 3.2 Error Types

Define domain-specific error types:

```rust
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub code: ErrorCode,
}

impl std::fmt::Display for ParseError { ... }
impl std::error::Error for ParseError {}
```

### 3.3 Error Messages

- Start with lowercase
- Be specific: `"expected ')' after arguments"` not `"syntax error"`
- Include context: `"undefined variable: {name}"`
- Suggest fixes when possible

### 3.4 Error Factory Functions

Mark error factories as cold paths:

```rust
#[cold]
pub fn division_by_zero() -> EvalError {
    EvalError::new("division by zero")
}

#[cold]
pub fn undefined_variable(name: &str) -> EvalError {
    EvalError::new(format!("undefined variable: {name}"))
}
```

---

## 4. Documentation

### 4.1 Module Documentation

Every module needs a doc comment:

```rust
//! Type conversion functions (`function_val`).
//!
//! These are the built-in type conversion functions like `int(x)`, `str(x)`
//! that allow positional arguments per the Ori spec.
```

### 4.2 Public API Documentation

Document all public items:

```rust
/// Evaluate a binary operation.
///
/// Tries each registered operator in order until one handles the operation.
///
/// # Errors
///
/// Returns `Err` if no operator handles the type combination or if the
/// operation itself fails (e.g., division by zero).
pub fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
    // ...
}
```

### 4.3 Internal Comments

- Explain *why*, not *what*
- Comment non-obvious algorithms
- Reference spec sections: `// Per spec §10.3: try unwraps Result/Option`

---

## 5. Architecture

### 5.1 Module Design

Follow single responsibility principle:

```
eval/
├── mod.rs           # Public exports only
├── evaluator.rs     # Core evaluation logic
├── environment.rs   # Variable scoping
├── operators.rs     # Binary operators
├── methods.rs       # Method dispatch
└── tests/           # Comprehensive tests
```

### 5.2 Dependency Direction

Dependencies flow downward:

```
oric (orchestration)
    ↓
ori_typeck, ori_eval, ori_patterns
    ↓
ori_parse
    ↓
ori_lexer
    ↓
ori_ir, ori_diagnostic (no dependencies)
```

Never import upward in the hierarchy.

### 5.3 Trait Design

Design traits for extension:

```rust
/// Trait for handling binary operations on values.
pub trait BinaryOperator: Send + Sync {
    /// Check if this operator handles the given operands.
    fn handles(&self, left: &Value, right: &Value, op: BinaryOp) -> bool;

    /// Evaluate the operation. Only called if `handles` returns true.
    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult;
}
```

### 5.4 Registry Pattern

Use registries for extensibility:

```rust
pub struct OperatorRegistry {
    operators: Vec<Box<dyn BinaryOperator>>,
}

impl OperatorRegistry {
    pub fn new() -> Self {
        OperatorRegistry {
            operators: vec![
                Box::new(IntOperator),
                Box::new(FloatOperator),
                // ...
            ],
        }
    }

    pub fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        for handler in &self.operators {
            if handler.handles(&left, &right, op) {
                return handler.evaluate(left, right, op);
            }
        }
        Err(type_mismatch_error(left, right))
    }
}
```

---

## 6. Type Safety

### 6.1 Newtypes

Use newtypes for type safety:

```rust
// Good: Can't confuse ExprId with Name
pub struct ExprId(u32);
pub struct Name(u32);

// Bad: Easy to mix up
type ExprId = u32;
type Name = u32;
```

### 6.2 Builder Pattern

Use builders for complex construction:

```rust
pub struct EvaluatorBuilder<'a> {
    interner: &'a StringInterner,
    debug: bool,
    max_recursion: usize,
}

impl<'a> EvaluatorBuilder<'a> {
    #[must_use]
    pub fn debug(mut self, enabled: bool) -> Self {
        self.debug = enabled;
        self
    }

    #[must_use]
    pub fn max_recursion(mut self, depth: usize) -> Self {
        self.max_recursion = depth;
        self
    }

    pub fn build(self) -> Evaluator<'a> {
        // ...
    }
}
```

### 6.3 Exhaustive Matching

Always handle all variants:

```rust
// Good: Explicit about all cases
match value {
    Value::Int(n) => ...,
    Value::Float(f) => ...,
    Value::Bool(b) => ...,
    Value::Str(s) => ...,
    // ... all variants
}

// Bad: Hides new variants
match value {
    Value::Int(n) => ...,
    _ => default_handling(),
}
```

### 6.4 Conversion Safety

Use checked conversions:

```rust
// Good: Explicit about precision loss
if let Ok(i32_val) = i32::try_from(n) {
    Some(f64::from(i32_val))  // Lossless
} else {
    // Handle large values differently
}

// Bad: Silent precision loss
Some(n as f64)
```

---

## 7. Performance

### 7.1 Allocation

Minimize allocations in hot paths:

```rust
// Good: Reuse buffer
let mut buffer = String::new();
for item in items {
    buffer.clear();
    write!(&mut buffer, "{}", item)?;
    process(&buffer);
}

// Bad: Allocate each iteration
for item in items {
    let s = format!("{}", item);
    process(&s);
}
```

### 7.2 Cloning

Avoid unnecessary clones:

```rust
// Good: Borrow when possible
fn process(value: &Value) -> Result { ... }

// Bad: Clone without need
fn process(value: Value) -> Result { ... }  // Forces caller to clone
```

### 7.3 Iteration

Prefer iterators over indexing:

```rust
// Good: Iterator
for item in items.iter() {
    process(item);
}

// Bad: Indexing (bounds check each time)
for i in 0..items.len() {
    process(&items[i]);
}
```

### 7.4 Stack Safety

Use stack guards for recursion:

```rust
pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
    ensure_sufficient_stack(|| self.eval_inner(expr_id))
}
```

---

## 8. Clippy Compliance

### 8.1 Required

The codebase must pass:

```bash
cargo clippy --workspace -- -D warnings
```

### 8.2 Pedantic Lints

We enable pedantic lints. Fix warnings properly:

| Warning | Fix |
|---------|-----|
| `cast_precision_loss` | Use checked conversions or lossless paths |
| `float_cmp` | Use `partial_cmp()` for IEEE 754 semantics |
| `too_many_arguments` | Create parameter structs |
| `implicit_hasher` | Generalize HashMap parameters |
| `match_same_arms` | Merge with or-patterns |
| `unused_self` | Convert to associated function |

### 8.3 Suppression Policy

Fix the underlying issue rather than silencing warnings. When suppression is genuinely necessary (e.g., a clippy lint that does not apply to a specific case), use `#[expect(...)]` (Rust 1.81+) with a `reason`:

```rust
#[expect(clippy::cast_sign_loss, reason = "value is validated non-negative above")]
let index = offset as usize;
```

`#[expect(...)]` is preferred over `#[allow(...)]` because the compiler verifies the suppressed lint actually fires. If the underlying code changes so the lint no longer triggers, `#[expect(...)]` produces a warning — preventing stale suppression attributes from accumulating.

Do not use bare `#[allow(clippy::...)]` without a `reason` string. Do not use Cargo.toml lint configuration to broadly disable lints.

---

## 9. Git Practices

### 9.1 Commit Messages

Format:

```
<type>: <subject>

<body>

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

### 9.2 Branch Naming

- `feat/description` - New features
- `fix/description` - Bug fixes
- `refactor/description` - Code improvements

### 9.3 PR Requirements

Before merging:
- [ ] All tests pass
- [ ] Clippy clean
- [ ] Documentation updated
- [ ] Spec updated (if behavior changed)

---

## 10. Checklist

### Before Committing

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Documentation updated
- [ ] No bare `#[allow(...)]` added (use `#[expect(..., reason = "...")]` when suppression is needed)

### Before PR

- [ ] All CI checks pass
- [ ] Tests added for new functionality
- [ ] Comprehensive edge case coverage
- [ ] Spec/docs updated if needed

---

## Quick Reference

### Do

- Write comprehensive tests in separate files
- Use descriptive names
- Handle all error cases
- Document public APIs
- Use newtypes for safety
- Fix clippy warnings properly
- Keep functions focused and short

### Don't

- Use bare `#[allow(...)]` to silence warnings (use `#[expect(...)]` with a reason instead)
- Leave TODO comments without tracking
- Commit failing tests
- Use `unwrap()` on user input
- Clone unnecessarily
- Write tests only for happy paths
- Exceed 500 lines per file
