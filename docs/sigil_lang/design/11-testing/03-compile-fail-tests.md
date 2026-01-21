# Compile-Fail Tests

This document covers Sigil's compile-fail tests: testing that code correctly fails to compile with expected errors, using caret-based annotations to mark error locations.

---

## Overview

Compile-fail tests verify that the compiler correctly rejects invalid code. They are essential for:

1. **Testing error detection** - Verify type errors, syntax errors, etc.
2. **Regression testing** - Ensure fixed bugs don't reappear
3. **Error message quality** - Confirm helpful diagnostics

```sigil
// In _test/compile-fail/type_errors.si

@bad_add (a: int, b: str) -> int = a + b
//                                     ^ E0308: cannot add int and str
```

The `// ^` annotation marks where the error should occur and what error code to expect.

---

## Basic Syntax

### Caret-Based Annotations

Expected errors use caret (`^`) annotations in comments:

```sigil
@bad_func () -> int = "hello"
//                    ^ E0308
```

**Structure:**
- `//` - Start of comment
- `^` - Caret pointing at error location
- `E0308` - Error code (required)
- `: message` - Optional documentation

### Positioning

The caret points to the error location on the line above:

```sigil
@example () -> int = unknown_var
//                   ^
// The caret points to 'u' in 'unknown_var'
```

The caret must be within the error's span (see [Column Flexibility](#column-flexibility)).

### Error Code

Every annotation requires an error code:

```sigil
// Good - includes error code
@bad () -> int = "hello"
//               ^ E0308

// Bad - no error code
@bad () -> int = "hello"
//               ^
```

Error codes match compiler output (`E0308`, `E0100`, etc.).

### Optional Message

The message after the colon is documentation, not an assertion:

```sigil
@bad (a: int, b: str) -> int = a + b
//                                 ^ E0308: cannot add int and str
```

The message helps readers understand the expected error. It is **not** verified against the compiler's actual message (which may change).

---

## Multi-Character Spans

### Span Syntax

Use multiple carets to show the error spans multiple characters:

```sigil
@bad () -> int = unknown_identifier
//               ^^^^^^^^^^^^^^^^^ E0100: unknown identifier
```

The span shows the exact extent of the error.

### Rules

| Syntax | Meaning |
|--------|---------|
| `^` | Single character error |
| `^^^` | Three-character span |
| `^^^^^^^^^` | Nine-character span |

The caret count should match the error span length.

### Examples

```sigil
// Single character
@bad () -> int = x
//               ^ E0100

// Identifier span
@bad () -> int = undefined_var
//               ^^^^^^^^^^^^^ E0100

// Expression span
@bad () -> int = "hello" + 42
//               ^^^^^^^^^^^^ E0308
```

---

## Multiple Expected Errors

### One File, Multiple Errors

A compile-fail test file can expect multiple errors:

```sigil
@multi_error () -> int = run(
    x = "hello" + 5,
//      ^^^^^^^^^^^ E0308: cannot add str and int
    y = unknown_var,
//      ^^^^^^^^^^^ E0100: unknown identifier
    z = bad_func(),
//      ^^^^^^^^ E0101: unknown function
    x + y + z
)
```

Each error gets its own annotation line.

### Multiple Errors on Same Line

For multiple errors on one line, use separate annotation lines:

```sigil
@complex () -> int = bad_a + bad_b
//                   ^^^^^ E0100
//                          ^^^^^ E0100
```

### Same Error, Multiple Locations

If the same error occurs multiple times:

```sigil
@repeated () -> int = run(
    a = unknown_1,
//      ^^^^^^^^^ E0100
    b = unknown_2,
//      ^^^^^^^^^ E0100
    a + b
)
```

---

## Test File Convention

### Directory Structure

Compile-fail tests live in a dedicated directory:

```
src/
  _test/
    compile-fail/
      type_errors.si        # Type mismatch tests
      syntax_errors.si      # Parser error tests
      coverage_errors.si    # Missing test coverage
      import_errors.si      # Import failures
    math.test.si            # Normal passing tests
```

### Naming

Files in `compile-fail/` should describe what they test:

| File | Tests |
|------|-------|
| `type_errors.si` | Type mismatches, wrong types |
| `syntax_errors.si` | Parse failures |
| `coverage_errors.si` | Missing tests (E0500) |
| `generic_errors.si` | Generic type issues |
| `import_errors.si` | Module import failures |

### Runner Behavior

The test runner treats `compile-fail/` specially:

1. Files **must** fail compilation
2. All expected errors must occur
3. No unexpected errors allowed
4. Test passes if expected = actual errors

---

## Error Code Matching

### Strict Matching

Error codes must match exactly:

```sigil
@bad () -> int = "hello"
//               ^ E0308
```

If the compiler produces `E0309` instead of `E0308`, the test fails.

### Message is Documentation Only

The message after the colon is **not** checked:

```sigil
// These are equivalent for testing:
@bad () -> int = "hello"
//               ^ E0308

@bad () -> int = "hello"
//               ^ E0308: expected int, found str

@bad () -> int = "hello"
//               ^ E0308: this is wrong but test still passes
```

The message documents intent without creating brittle tests that break when error messages are reworded.

### Rationale

Error codes are stable identifiers. Messages may change for clarity without changing the underlying error. Testing codes, not messages, prevents unnecessary test breakage.

---

## Expected Warnings

### Warning Syntax

Use `W` prefix for expected warnings:

```sigil
@unused_param (x: int) -> int = 5
//             ^ W0100: unused parameter
```

### Warning Codes

| Prefix | Meaning |
|--------|---------|
| `E` | Error (compilation fails) |
| `W` | Warning (compilation succeeds) |

### Examples

```sigil
// Unused parameter warning
@unused_param (x: int) -> int = 5
//             ^ W0100

// Unused config warning
$unused_config = 42
// ^ W0200: unused config

// Shadowing warning
@shadow () -> int = run(
    x = 1,
    x = 2,
//  ^ W0300: variable shadows previous binding
    x
)
```

### Warning Test Files

Warnings can be tested in regular or compile-fail directories:

```
_test/
  compile-fail/
    type_errors.si     # Must have errors
  warnings/
    unused.si          # Expected warnings, compiles successfully
```

---

## No Error Expected

### The `ok` Annotation

Use `// ^ ok` to assert no error at a location:

```sigil
// Regression test - this used to fail
@was_broken (a: int, b: int) -> int = a + b
//                                        ^ ok
```

### Use Case

The `// ^ ok` annotation is for **regression tests**:

1. A bug caused a false positive error at this location
2. The bug was fixed
3. Add `// ^ ok` to prevent regression

### Difference from Missing Annotation

| Annotation | Meaning |
|------------|---------|
| (none) | No expectation - any error or no error |
| `// ^ ok` | Assert: NO error at this location |
| `// ^ E0308` | Assert: error E0308 at this location |

### Example

```sigil
// Fixed bug: compiler incorrectly flagged valid generic usage
@identity<T> (x: T) -> T = x
//                         ^ ok

// This ensures the fix doesn't regress
@test_identity tests @identity () -> void = run(
    assert_eq(identity(5), 5)
)
```

---

## Negation

### Negation Syntax

Use `!^` to assert an error should NOT occur:

```sigil
@valid_add (a: int, b: int) -> int = a + b
// !^ E0308
```

### Difference from `ok`

| Annotation | Meaning |
|------------|---------|
| `// ^ ok` | No error at all |
| `// !^ E0308` | Not error E0308 (other errors allowed) |

### Use Case

Test that a specific error doesn't occur, even if others might:

```sigil
// Ensure we don't get type error, even if other issues exist
@complex () -> SomeType = complicated_expression()
// !^ E0308
```

This is more targeted than `// ^ ok`.

### Examples

```sigil
// Assert: no type mismatch error
@valid_types (a: int) -> int = a * 2
// !^ E0308

// Assert: no "unknown identifier" error (function exists)
@uses_helper () -> int = helper(5)
// !^ E0100

// Combined: assert specific error, deny another
@partial_valid () -> int = run(
    x = unknown,
//      ^^^^^^^ E0100
    y = valid_func(x),
//      ^^^^^^^^^^^ !^ E0308
    y
)
```

---

## Column Flexibility

### Approximate Position

The caret position is **approximate**, not exact:

```sigil
@bad (a: int, b: str) -> int = a + b
//                                 ^ E0308
```

The caret must be within the error's span, but not necessarily at the exact start.

### Rules

1. Caret must be within the error's column range
2. Doesn't need to point at exact start
3. Error code must match exactly

### Why Flexibility?

Exact column matching is fragile:
- Different compiler versions may report slightly different positions
- Refactoring the error reporting shouldn't break tests
- The error code is the stable identifier

### Example

Both of these work:

```sigil
@bad () -> int = "hello world"
//               ^ E0308

@bad () -> int = "hello world"
//                    ^ E0308

@bad () -> int = "hello world"
//                           ^ E0308
```

As long as the caret falls within the string literal span.

---

## Test Runner Integration

### Running Compile-Fail Tests

```bash
sigil test                          # All tests including compile-fail
sigil test src/_test/compile-fail/  # Only compile-fail tests
sigil test src/_test/compile-fail/type_errors.si  # Specific file
```

### Output Format

#### Success

```
Running compile-fail tests...
  ✓ type_errors.si (3 expected errors)
  ✓ syntax_errors.si (2 expected errors)
  ✓ coverage_errors.si (1 expected error)

3/3 compile-fail tests passed
```

#### Failure - Missing Expected Error

```
Running compile-fail tests...
  ✗ type_errors.si
    - Expected E0308 at line 5, but no error occurred

0/1 compile-fail tests passed
```

#### Failure - Wrong Error

```
Running compile-fail tests...
  ✗ type_errors.si
    - Expected E0308 at line 5, but got E0309

0/1 compile-fail tests passed
```

#### Failure - Unexpected Error

```
Running compile-fail tests...
  ✗ type_errors.si
    - Unexpected error E0100 at line 8 (no annotation)

0/1 compile-fail tests passed
```

### JSON Output

```bash
sigil test src/_test/compile-fail/ --format json
```

```json
{
  "compile_fail_tests": [
    {
      "file": "type_errors.si",
      "status": "passed",
      "expected_errors": 3,
      "matched": 3
    },
    {
      "file": "syntax_errors.si",
      "status": "failed",
      "expected_errors": 2,
      "matched": 1,
      "mismatches": [
        {
          "line": 5,
          "expected": "E0308",
          "actual": null,
          "type": "missing"
        }
      ],
      "unexpected": [
        {
          "line": 8,
          "code": "E0100",
          "message": "unknown identifier"
        }
      ]
    }
  ],
  "summary": {
    "total": 2,
    "passed": 1,
    "failed": 1
  }
}
```

---

## Complete Examples

### Type Error Tests

```sigil
// _test/compile-fail/type_errors.si

// Return type mismatch
@returns_wrong () -> int = "hello"
//                         ^^^^^^^ E0308: expected int, found str

// Argument type mismatch
@takes_int (x: int) -> int = x

@bad_call () -> int = takes_int("hello")
//                              ^^^^^^^ E0308: expected int, found str

// Binary operator type mismatch
@bad_add () -> int = 5 + "hello"
//                       ^^^^^^^ E0308: cannot add int and str

// Generic type mismatch
@pair<T> (a: T, b: T) -> (T, T) = (a, b)

@bad_pair () -> (int, int) = pair(1, "two")
//                                   ^^^^^ E0308: expected int, found str
```

### Missing Test Coverage

```sigil
// _test/compile-fail/coverage_errors.si

// Function without test - should fail
@untested_func (x: int) -> int = x * 2
// ^ E0500: function @untested_func has no tests

// This is how the test requirement is verified
```

### Syntax Errors

```sigil
// _test/compile-fail/syntax_errors.si

// Missing return type
@no_return (x: int) = x
//                  ^ E0200: expected '->'

// Unclosed parenthesis
@unclosed (x: int -> int = x
//                 ^ E0201: expected ')'

// Invalid token
@bad_token () -> int = 5 @@ 3
//                       ^^ E0202: unexpected token
```

### Regression Tests

```sigil
// _test/compile-fail/regressions.si

// Issue #123: false positive on valid generic instantiation
@fixed_generic<T> (x: T) -> T = x
//                              ^ ok

// Issue #456: ensure we still catch actual type errors
@still_catches () -> int = "nope"
//                         ^^^^^^ E0308

// Issue #789: shouldn't report E0308 for this pattern
@special_case () -> Result<int, str> = Ok(5)
// !^ E0308
```

### Mixed Errors and Warnings

```sigil
// _test/compile-fail/mixed.si

@problematic (unused_param: int, bad_param: str) -> int = run(
//            ^^^^^^^^^^^^ W0100: unused parameter
    result = bad_param + 5,
//           ^^^^^^^^^^^^^ E0308: cannot add str and int
    result
)
```

---

## Best Practices

### One Concept Per File

```
compile-fail/
  type_mismatch_return.si    # Return type errors
  type_mismatch_arg.si       # Argument type errors
  type_mismatch_binary.si    # Binary operator errors
  unknown_identifier.si      # Undefined variables
  unknown_function.si        # Undefined functions
```

### Document the Intent

```sigil
// Test: Compiler rejects adding incompatible types
// Ensures proper type checking for binary operators

@add_int_str () -> int = 1 + "two"
//                           ^^^^^ E0308: cannot add int and str

@add_str_int () -> str = "one" + 2
//                               ^ E0308: cannot add str and int
```

### Test Edge Cases

```sigil
// Test: Type errors in nested expressions

@nested () -> int = run(
    a = if true then "wrong" else 5,
//                   ^^^^^^^ E0308
    b = match(x,
        .Some: v -> v + "oops",
//                      ^^^^^^ E0308
        .None: 0
    ),
    a + b
)
```

### Group Related Annotations

Keep annotations close to their error:

```sigil
// Good - annotation immediately follows
@bad () -> int = unknown
//               ^^^^^^^ E0100

// Less clear - annotation far from error
@bad () -> int = unknown



//               ^^^^^^^ E0100
```

---

## Summary

| Annotation | Meaning |
|------------|---------|
| `// ^ E0308` | Expect error E0308 at this location |
| `// ^^^^^ E0308` | Expect E0308 spanning 5 characters |
| `// ^ E0308: msg` | Expect E0308 (message is documentation) |
| `// ^ W0100` | Expect warning W0100 |
| `// ^ ok` | Assert NO error at this location |
| `// !^ E0308` | Assert E0308 does NOT occur |

| Directory | Purpose |
|-----------|---------|
| `_test/compile-fail/` | Tests that must fail compilation |
| Regular test files | Tests that must pass |

| Command | Action |
|---------|--------|
| `sigil test` | Run all tests |
| `sigil test _test/compile-fail/` | Run only compile-fail tests |
| `sigil test --format json` | JSON output for tooling |

---

## See Also

- [Mandatory Tests](01-mandatory-tests.md) - Why tests are required
- [Test Syntax](02-test-syntax.md) - Normal test syntax
- [Structured Errors](../12-tooling/02-structured-errors.md) - Error code reference
- [AI-First Design](../01-philosophy/01-ai-first-design.md) - Design philosophy
