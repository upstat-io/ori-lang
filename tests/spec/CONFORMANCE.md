# Spec Conformance Report

**Generated from spec tests - these tests are the source of truth.**

The tests in this directory validate the compiler against the language specification at `docs/sigil_lang/0.1-alpha/spec/`. When tests fail, the **implementation** is wrong, not the tests.

## Current Status

### Passing (0 issues)

None of the spec tests currently pass due to implementation bugs.

### Failing - Implementation Bugs

#### 1. Empty List Type Inference

**Spec says** (06-types.md): Empty list literals `[]` should be valid and infer their type from context.

**Expected syntax**:
```sigil
assert([] == [])
assert([] + [1] == [1])
```

**Root cause**: Type inference fails on empty list literals.

**Affected tests**: `arithmetic.si`, `comparison.si`

---

#### 2. Conditional Syntax: `then`/`else` vs `:then`/`:else`

**Spec says** (09-expressions.md):
```
if_expr = "if" expression "then" expression
          { "else" "if" expression "then" expression }
          "else" expression .
```

**Expected syntax**:
```sigil
if x > 0 then "positive" else "non-positive"
```

**Implementation error**:
```
Error parsing test file: Expected ColonThen, found Ident("then")
```

**Root cause**: The parser expects `:then` and `:else` tokens instead of `then` and `else` keywords.

**Affected tests**: `conditionals.si`, `data.si` (fold pattern uses if-then-else)

---

#### 3. Wildcard Pattern `_` Not Lexed

**Spec says** (10-patterns.md):
```
wildcard_pattern = "_" .
```

**Expected syntax**:
```sigil
match(n,
    0 -> "zero",
    _ -> "default",
)
```

**Implementation error**:
```
Error parsing test file: Unexpected character at position 436: '_ '
```

**Root cause**: The underscore `_` followed by whitespace is not being recognized as the wildcard pattern.

**Affected tests**: `match.si`

---

#### 4. Multiple Test Targets Syntax

**Spec says** (08-declarations.md):
```
test = "@" identifier "tests" "@" identifier { "tests" "@" identifier } params "->" "void" "=" expression .
```

**Expected syntax**:
```sigil
@test_run_with_calls tests @run_with_calls tests @double () -> void = run(...)
```

**Implementation error**:
```
Error parsing test file: Expected LParen, found Tests
```

**Root cause**: The parser doesn't support multiple `tests @target` clauses.

**Affected tests**: `run.si`

---

#### 5. Duration Literals Not Fully Supported

**Spec says** (03-lexical-elements.md):
```
duration_literal = int_literal duration_unit .
duration_unit = "ms" | "s" | "m" | "h" .
```

**Expected syntax**:
```sigil
let ms = 100ms
let s = 30s
let m = 5m
let h = 2h
```

**Status**: Lexer tokenizes these, but parser/type system may not handle them.

**Affected tests**: `literals.si`

---

#### 6. Size Literals Not Fully Supported

**Spec says** (03-lexical-elements.md):
```
size_literal = int_literal size_unit .
size_unit = "b" | "kb" | "mb" | "gb" .
```

**Expected syntax**:
```sigil
let bytes = 1024b
let kb = 4kb
```

**Status**: Lexer tokenizes these, but parser/type system may not handle them.

**Affected tests**: `literals.si`

---

### Failing - Test Harness Issue

The following tests fail with:
```
Error: Test file must import the module being tested
Add: use module_name { function1, function2 }
```

This is a test harness strictness issue, not a spec conformance bug:
- `arithmetic.si`
- `comparison.si`
- `literals.si`
- `primitives.si`

The spec tests are self-contained (functions and tests in same file) which should be valid.

---

## How to Use This Report

1. **Never modify spec tests to match broken code**
2. Fix the implementation bugs listed above
3. Re-run spec tests: `sigil test tests/spec/`
4. Update this report when issues are resolved

## Running Spec Tests

```bash
# Run all spec tests
for f in tests/spec/**/*.si; do sigil test "$f"; done

# Run specific category
sigil test tests/spec/expressions/conditionals.si
```
