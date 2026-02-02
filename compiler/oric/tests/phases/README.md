# Phase Tests

Compiler tests organized by compilation phase.

## Structure

| Directory | Tests | Crates Tested |
|-----------|-------|---------------|
| `parse/` | Lexer and parser | `ori_lexer`, `ori_parse` |
| `typeck/` | Type system | `ori_typeck`, `ori_types` |
| `eval/` | Interpreter | `ori_eval`, `ori_patterns` |
| `codegen/` | LLVM backend | `ori_llvm`, `ori_rt` |
| `common/` | Shared utilities | Multiple |

## When to Use Phase Tests vs Spec Tests

**Phase tests** (`compiler/oric/tests/phases/`):
- Testing compiler internals and implementation details
- Inline test module would exceed 200 lines
- Test needs access to multiple compiler APIs
- Edge cases in specific compiler components

**Spec tests** (`tests/spec/`):
- Testing user-facing language behavior
- Tests should run on both interpreter and LLVM backends
- Validating language specification compliance

## Running Tests

```bash
# Run all phase tests
cargo test -p oric --test phases

# Run specific phase
cargo test -p oric --test phases parse
cargo test -p oric --test phases typeck
cargo test -p oric --test phases eval
cargo test -p oric --test phases codegen

# Run with LLVM codegen tests
cargo test -p oric --test phases --features oric/llvm

# Filter to specific test file
cargo test -p oric --test phases debug_config
cargo test -p oric --test phases linker_gcc
```

## Adding Tests

1. **Identify the phase**: Which compiler component does this test?
2. **Create or find the test file**: `phases/{phase}/{category}.rs`
3. **Add the test**: Follow existing patterns in the file
4. **Update mod.rs**: Include new modules if needed

### Example Test Structure

```rust
// phases/typeck/generics.rs

use ori_types::{Type, TypeContext};

#[test]
fn test_instantiate_generic_function() {
    // Arrange
    let ctx = TypeContext::new();

    // Act
    let result = ctx.instantiate(/* ... */);

    // Assert
    assert_eq!(result, expected);
}
```

## 200-Line Rule

Inline test modules (`#[cfg(test)] mod tests`) in source files should not exceed 200 lines. When tests grow beyond this limit, extract them to phase test files.
