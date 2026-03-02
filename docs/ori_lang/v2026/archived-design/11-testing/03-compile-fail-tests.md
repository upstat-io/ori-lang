# Compile-Fail Tests

This document covers Ori's compile-fail tests: testing that code correctly fails to compile with expected errors.

---

## Overview

Compile-fail tests verify that the compiler correctly rejects invalid code. They are essential for:

1. **Testing error detection** - Verify type errors, syntax errors, etc.
2. **Regression testing** - Ensure fixed bugs don't reappear
3. **Error message quality** - Confirm helpful diagnostics

---

## The compile_fail Attribute

The primary mechanism for compile-fail tests is the `#compile_fail` attribute on a test function.

### Basic Usage

```ori
#compile_fail("type mismatch")
@test_bad_return tests @main () -> void = run(
    let x: int = "hello",
    ()
)
```

The test passes if:
1. Compilation fails (parse error, type error, etc.)
2. At least one error message contains the specified substring

### Why This Design?

The attribute approach has several advantages:

1. **Real code** - The test contains actual Ori code, not strings, so you get IDE support (syntax highlighting, completion)
2. **Consistent with skip** - Uses the same attribute pattern as `#skip("reason")`
3. **Clear semantics** - The test body is the code that should fail; the attribute specifies the expected error
4. **Integrated** - Compile-fail tests live alongside regular tests, not in separate directories

### Examples

```ori
// Test: closure self-capture is detected
#compile_fail("closure cannot capture itself")
@test_self_capture tests @main () -> void = run(
    let f = () -> f,
    ()
)

// Test: type mismatch in binary operation
#compile_fail("cannot add int and str")
@test_add_mismatch tests @main () -> void = run(
    let result = 1 + "two",
    ()
)

// Test: unknown identifier
#compile_fail("unknown identifier")
@test_undefined tests @main () -> void = run(
    let result = undefined_var,
    ()
)
```

### Combining with skip

A test can have both attributes if needed:

```ori
#skip("parser doesn't support this yet")
#compile_fail("expected type annotation")
@test_future_error tests @main () -> void = run(
    let result = ambiguous_expression,
    ()
)
```

---

## Advanced: Caret-Based Annotations

For more precise error location assertions, Ori supports caret-based annotations within compile-fail tests. These are optional and complement the `compile_fail` attribute.

### Basic Syntax

Expected errors use caret (`^`) annotations in comments:

```ori
#compile_fail("E0308")
@test_type_error tests @main () -> void = run(
    let x: int = "hello",
//               ^^^^^^^ E0308
    ()
)
```

**Structure:**
- `//` - Start of comment
- `^` - Caret pointing at error location
- `E0308` - Error code (required)
- `: message` - Optional documentation

### Positioning

The caret points to the error location on the line above:

```ori
@example () -> int = unknown_var
//                   ^
// The caret points to 'u' in 'unknown_var'
```

The caret must be within the error's span (see [Column Flexibility](#column-flexibility)).

### Error Code

Every annotation requires an error code:

```ori
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

```ori
@bad (left: int, right: str) -> int = left + right
//                                           ^ E0308: cannot add int and str
```

The message helps readers understand the expected error. It is **not** verified against the compiler's actual message (which may change).

---

## Multi-Character Spans

### Span Syntax

Use multiple carets to show the error spans multiple characters:

```ori
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

```ori
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

```ori
@multi_error () -> int = run(
    let first = "hello" + 5,
//              ^^^^^^^^^^^ E0308: cannot add str and int
    let second = unknown_var,
//               ^^^^^^^^^^^ E0100: unknown identifier
    let third = bad_func(),
//              ^^^^^^^^ E0101: unknown function
    first + second + third,
)
```

Each error gets its own annotation line.

### Multiple Errors on Same Line

For multiple errors on one line, use separate annotation lines:

```ori
@complex () -> int = bad_a + bad_b
//                   ^^^^^ E0100
//                          ^^^^^ E0100
```

### Same Error, Multiple Locations

If the same error occurs multiple times:

```ori
@repeated () -> int = run(
    let a = unknown_1,
//          ^^^^^^^^^ E0100
    let b = unknown_2,
//          ^^^^^^^^^ E0100
    a + b,
)
```

---

## Test File Organization

### Inline with Regular Tests (Recommended)

With the `#compile_fail` attribute, compile-fail tests can live alongside regular tests:

```ori
// math.ori

@add (left: int, right: int) -> int = left + right

// Regular test
@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(
            .left: 1,
            .right: 2,
        ),
        .expected: 3,
    ),
)

// Compile-fail test in same file
#compile_fail("cannot add int and str")
@test_add_type_error tests @add () -> void = run(
    let result = add(
        .left: 1,
        .right: "two",
    ),
    (),
)
```

### Dedicated Directory (Optional)

For large test suites, a dedicated directory is still an option:

```
src/
  _test/
    compile-fail/
      type_errors.ori        # Type mismatch tests
      syntax_errors.ori      # Parser error tests
    math.test.ori            # Normal passing tests
```

### Naming Conventions

| Pattern | Purpose |
|---------|---------|
| `@test_*_error` | Compile-fail test |
| `@test_*` | Regular test |
| `coverage_errors.ori` | Missing tests (E0500) |
| `generic_errors.ori` | Generic type issues |
| `import_errors.ori` | Module import failures |

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

```ori
@bad () -> int = "hello"
//               ^ E0308
```

If the compiler produces `E0309` instead of `E0308`, the test fails.

### Message is Documentation Only

The message after the colon is **not** checked:

```ori
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

```ori
@unused_param (value: int) -> int = 5
//             ^^^^^ W0100: unused parameter
```

### Warning Codes

| Prefix | Meaning |
|--------|---------|
| `E` | Error (compilation fails) |
| `W` | Warning (compilation succeeds) |

### Examples

```ori
// Unused parameter warning
@unused_param (value: int) -> int = 5
//             ^^^^^ W0100

// Unused config warning
$unused_config = 42
// ^ W0200: unused config

// Shadowing warning
@shadow () -> int = run(
    let count = 1,
    let count = 2,
//      ^^^^^ W0300: variable shadows previous binding
    count,
)
```

### Warning Test Files

Warnings can be tested in regular or compile-fail directories:

```
_test/
  compile-fail/
    type_errors.ori     # Must have errors
  warnings/
    unused.ori          # Expected warnings, compiles successfully
```

---

## No Error Expected

### The `ok` Annotation

Use `// ^ ok` to assert no error at a location:

```ori
// Regression test - this used to fail
@was_broken (left: int, right: int) -> int = left + right
//                                                  ^ ok
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

```ori
// Fixed bug: compiler incorrectly flagged valid generic usage
@identity<T> (value: T) -> T = value
//                             ^ ok

// This ensures the fix doesn't regress
@test_identity tests @identity () -> void = run(
    assert_eq(
        .actual: identity(
            .value: 5,
        ),
        .expected: 5,
    ),
)
```

---

## Negation

### Negation Syntax

Use `!^` to assert an error should NOT occur:

```ori
@valid_add (left: int, right: int) -> int = left + right
// !^ E0308
```

### Difference from `ok`

| Annotation | Meaning |
|------------|---------|
| `// ^ ok` | No error at all |
| `// !^ E0308` | Not error E0308 (other errors allowed) |

### Use Case

Test that a specific error doesn't occur, even if others might:

```ori
// Ensure we don't get type error, even if other issues exist
@complex () -> SomeType = complicated_expression()
// !^ E0308
```

This is more targeted than `// ^ ok`.

### Examples

```ori
// Assert: no type mismatch error
@valid_types (value: int) -> int = value * 2
// !^ E0308

// Assert: no "unknown identifier" error (function exists)
@uses_helper () -> int = helper(
    .value: 5,
)
// !^ E0100

// Combined: assert specific error, deny another
@partial_valid () -> int = run(
    let value = unknown,
//              ^^^^^^^ E0100
    let result = valid_func(
        .value: value,
    ),
//              ^^^^^^^^^^^ !^ E0308
    result,
)
```

---

## Column Flexibility

### Approximate Position

The caret position is **approximate**, not exact:

```ori
@bad (left: int, right: str) -> int = left + right
//                                           ^ E0308
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

```ori
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
ori test                          # All tests including compile-fail
ori test src/_test/compile-fail/  # Only compile-fail tests
ori test src/_test/compile-fail/type_errors.ori  # Specific file
```

### Output Format

#### Success

```
Running compile-fail tests...
  ✓ type_errors.ori (3 expected errors)
  ✓ syntax_errors.ori (2 expected errors)
  ✓ coverage_errors.ori (1 expected error)

3/3 compile-fail tests passed
```

#### Failure - Missing Expected Error

```
Running compile-fail tests...
  ✗ type_errors.ori
    - Expected E0308 at line 5, but no error occurred

0/1 compile-fail tests passed
```

#### Failure - Wrong Error

```
Running compile-fail tests...
  ✗ type_errors.ori
    - Expected E0308 at line 5, but got E0309

0/1 compile-fail tests passed
```

#### Failure - Unexpected Error

```
Running compile-fail tests...
  ✗ type_errors.ori
    - Unexpected error E0100 at line 8 (no annotation)

0/1 compile-fail tests passed
```

### JSON Output

```bash
ori test src/_test/compile-fail/ --format json
```

```json
{
  "compile_fail_tests": [
    {
      "file": "type_errors.ori",
      "status": "passed",
      "expected_errors": 3,
      "matched": 3
    },
    {
      "file": "syntax_errors.ori",
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

```ori
// _test/compile-fail/type_errors.ori

// Return type mismatch
@returns_wrong () -> int = "hello"
//                         ^^^^^^^ E0308: expected int, found str

// Argument type mismatch
@takes_int (value: int) -> int = value

@bad_call () -> int = takes_int(
    .value: "hello",
)
//         ^^^^^^^ E0308: expected int, found str

// Binary operator type mismatch
@bad_add () -> int = 5 + "hello"
//                       ^^^^^^^ E0308: cannot add int and str

// Generic type mismatch
@pair<T> (first: T, second: T) -> (T, T) = (first, second)

@bad_pair () -> (int, int) = pair(
    .first: 1,
    .second: "two",
)
//          ^^^^^ E0308: expected int, found str
```

### Missing Test Coverage

```ori
// _test/compile-fail/coverage_errors.ori

// Function without test - should fail
@untested_func (value: int) -> int = value * 2
// ^ E0500: function @untested_func has no tests

// This is how the test requirement is verified
```

### Syntax Errors

```ori
// _test/compile-fail/syntax_errors.ori

// Missing return type
@no_return (value: int) = value
//                      ^ E0200: expected '->'

// Unclosed parenthesis
@unclosed (value: int -> int = value
//                     ^ E0201: expected ')'

// Invalid token
@bad_token () -> int = 5 @@ 3
//                       ^^ E0202: unexpected token
```

### Regression Tests

```ori
// _test/compile-fail/regressions.ori

// Issue #123: false positive on valid generic instantiation
@fixed_generic<T> (value: T) -> T = value
//                                  ^ ok

// Issue #456: ensure we still catch actual type errors
@still_catches () -> int = "nope"
//                         ^^^^^^ E0308

// Issue #789: shouldn't report E0308 for this pattern
@special_case () -> Result<int, str> = Ok(
    .value: 5,
)
// !^ E0308
```

### Mixed Errors and Warnings

```ori
// _test/compile-fail/mixed.ori

@problematic (unused_param: int, bad_param: str) -> int = run(
//            ^^^^^^^^^^^^ W0100: unused parameter
    let result = bad_param + 5,
//               ^^^^^^^^^^^^^ E0308: cannot add str and int
    result,
)
```

---

## Best Practices

### One Concept Per File

```
compile-fail/
  type_mismatch_return.ori    # Return type errors
  type_mismatch_arg.ori       # Argument type errors
  type_mismatch_binary.ori    # Binary operator errors
  unknown_identifier.ori      # Undefined variables
  unknown_function.ori        # Undefined functions
```

### Document the Intent

```ori
// Test: Compiler rejects adding incompatible types
// Ensures proper type checking for binary operators

@add_int_str () -> int = 1 + "two"
//                           ^^^^^ E0308: cannot add int and str

@add_str_int () -> str = "one" + 2
//                               ^ E0308: cannot add str and int
```

### Test Edge Cases

```ori
// Test: Type errors in nested expressions

@nested () -> int = run(
    let a = if true then "wrong" else 5,
//                       ^^^^^^^ E0308
    let b = match(
        .value: x,
        Some(value) -> value + "oops",
//                             ^^^^^^ E0308
        None -> 0,
    ),
    a + b,
)
```

### Group Related Annotations

Keep annotations close to their error:

```ori
// Good - annotation immediately follows
@bad () -> int = unknown
//               ^^^^^^^ E0100

// Less clear - annotation far from error
@bad () -> int = unknown



//               ^^^^^^^ E0100
```

---

## Summary

### Primary Mechanism: compile_fail Attribute

| Syntax | Meaning |
|--------|---------|
| `#compile_fail("msg")` | Test must fail to compile with error containing "msg" |
| `#skip("reason")` | Skip the test (can combine with compile_fail) |

### Advanced: Caret Annotations (Optional)

| Annotation | Meaning |
|------------|---------|
| `// ^ E0308` | Expect error E0308 at this location |
| `// ^^^^^ E0308` | Expect E0308 spanning 5 characters |
| `// ^ E0308: msg` | Expect E0308 (message is documentation) |
| `// ^ W0100` | Expect warning W0100 |
| `// ^ ok` | Assert NO error at this location |
| `// !^ E0308` | Assert E0308 does NOT occur |

### Commands

| Command | Action |
|---------|--------|
| `ori test` | Run all tests (including compile-fail) |
| `ori test --format json` | JSON output for tooling |

---

## See Also

- [Mandatory Tests](01-mandatory-tests.md) - Why tests are required
- [Test Syntax](02-test-syntax.md) - Normal test syntax
- [Structured Errors](../12-tooling/02-structured-errors.md) - Error code reference
- [AI-First Design](../01-philosophy/01-ai-first-design.md) - Design philosophy
