# Testing

This section covers Ori's mandatory testing: test requirements, test syntax, and compile-fail tests.

---

## Documents

| Document | Description |
|----------|-------------|
| [Mandatory Tests](01-mandatory-tests.md) | Test requirements, coverage enforcement |
| [Test Syntax](02-test-syntax.md) | tests keyword, assertions |
| [Compile-Fail Tests](03-compile-fail-tests.md) | Testing expected errors |

### Related

| Document | Description |
|----------|-------------|
| [Testing Effectful Code](../14-capabilities/03-testing-effectful-code.md) | Mocking side effects with capabilities |

---

## Overview

Ori enforces test coverage at compile time:

```ori
// Function definition
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1)
)

// Test is REQUIRED - compilation fails without it
@test_factorial tests @factorial () -> void = run(
    assert_eq(
        .actual: factorial(.number: 0),
        .expected: 1,
    ),
    assert_eq(
        .actual: factorial(.number: 1),
        .expected: 1,
    ),
    assert_eq(
        .actual: factorial(.number: 5),
        .expected: 120,
    ),
)
```

### Test Requirements

| Rule | Description |
|------|-------------|
| All functions require tests | Compiler error if missing |
| `@main` is exempt | Entry point tested by running |
| Config variables exempt | `$timeout` doesn't need tests |
| Multiple tests allowed | One function can have many tests |

### Compile-Fail Tests

Test that code correctly fails to compile:

```ori
// In _test/compile-fail/type_errors.ori
@bad_add (left: int, right: str) -> int = left + right
//                                               ^ E0308: cannot add int and str
```

### Why Mandatory Testing?

1. **Validates AI output** - AI-generated code is immediately verified
2. **Executable specification** - Tests show how functions should be used
3. **Catches mistakes early** - Errors found at compile time
4. **No excuses** - "I'll add tests later" isn't possible

---

## See Also

- [Main Index](../00-index.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
