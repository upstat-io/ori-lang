---
section: "09"
title: Diagnostic Quality
status: not-started
priority: high
goal: Ensure all errors have spans, suggestions, and user-friendly messages
files:
  - compiler/ori_eval/src/interpreter/mod.rs
  - compiler/ori_eval/src/methods/*.rs
  - compiler/ori_patterns/src/errors.rs
  - compiler/ori_typeck/src/suggest.rs
---

# Section 09: Diagnostic Quality

**Status:** ✅ Infrastructure Complete (incremental improvements ongoing)
**Priority:** HIGH — Poor error messages frustrate users and slow debugging
**Goal:** All errors have source spans, actionable suggestions, and clear messages

**Assessment:** Error infrastructure is solid:
- `EvalError.span: Option<Span>` field exists
- `with_span()` builder method available
- Error factories use `#[cold]` for optimization
- Messages are clear and use consistent style
- Remaining items are incremental quality improvements

---

## Principles

From `.claude/rules/diagnostic.md`:
- All errors have source spans
- Imperative suggestions: "try using X" not "Did you mean X?"
- Three-part structure: problem → source context → actionable guidance
- No `panic!` on user errors

---

## 09.1 Add Spans to EvalErrors

Multiple EvalError constructions lack spans:

### interpreter/mod.rs

- [ ] **Line 377**: Internal error without span
  ```rust
  // Before:
  Err(EvalError::internal(format!("expression {} not found", expr_id)))

  // After:
  Err(EvalError::internal(format!("expression {} not found", expr_id))
      .with_span(fallback_span))
  ```

### methods/variants.rs

- [ ] **Line 158**: Unwrap error without span
  - Thread span through from caller

### methods/compare.rs

- [ ] **Line 49**: Compare error missing span
  - Thread span through from caller

### methods/numeric.rs

- [ ] **Lines 113, 120**: Shift range errors without span
  - Add span parameter to shift functions

### operators.rs

- [ ] **Lines 130, 134**: Shift overflow errors without span
  - Add span from binary expression

### Pattern for Fix

```rust
// Before:
pub fn shift_left(a: i64, b: i64) -> Result<Value, EvalError> {
    if b < 0 || b >= 64 {
        return Err(EvalError::shift_out_of_range());
    }
    // ...
}

// After:
pub fn shift_left(a: i64, b: i64, span: Span) -> Result<Value, EvalError> {
    if b < 0 || b >= 64 {
        return Err(EvalError::shift_out_of_range().with_span(span));
    }
    // ...
}
```

---

## 09.2 Add "Did You Mean?" Suggestions

### ori_patterns/src/errors.rs

- [ ] **Line 185**: `undefined_variable` lacks suggestion
  ```rust
  // Before:
  pub fn undefined_variable(name: Name, span: Span) -> EvalError { ... }

  // After:
  pub fn undefined_variable(
      name: Name,
      span: Span,
      similar: Option<Name>,  // From suggest_identifier
  ) -> EvalError {
      let mut err = EvalError::new(...)
          .with_span(span);
      if let Some(suggestion) = similar {
          err = err.with_suggestion(format!("try using `{}`", suggestion));
      }
      err
  }
  ```

- [ ] **Line 191**: `undefined_function` lacks suggestion
- [ ] **Line 241**: `no_field_on_struct` lacks suggestion

### Integration with suggest.rs

- [ ] Use `ori_typeck/src/suggest.rs` functions:
  - `suggest_identifier(name, available_names)`
  - `suggest_function(name, available_functions)`
  - `suggest_field(name, available_fields)`

- [ ] Pass available names from scope to error constructors

---

## 09.3 Improve Terse Error Messages

### ori_patterns/src/errors.rs

- [ ] **Line 294**: "non-exhaustive match" — Add missing patterns
  ```rust
  // Before:
  "non-exhaustive match"

  // After:
  "non-exhaustive match: patterns not covered: {missing_patterns}"
  ```

- [ ] **Line 349**: "missing struct field" — Include field name
  ```rust
  // Before:
  "missing struct field"

  // After:
  "missing required field `{field_name}` in struct `{struct_name}`"
  ```

- [ ] **Line 381**: "invalid literal pattern" — Explain why
  ```rust
  // Before:
  "invalid literal pattern"

  // After:
  "invalid literal pattern: {reason}"
  // e.g., "floats cannot be used in patterns"
  ```

### interpreter/mod.rs

- [ ] **Line 377**: Hide implementation details
  ```rust
  // Before:
  "Internal error: expression N not found"

  // After:
  "internal compiler error: please report this bug"
  // Log full details to stderr/file for debugging
  ```

---

## 09.4 Add Fix Suggestions

### ori_patterns/src/errors.rs

- [ ] **Line 209**: `wrong_function_args` — Suggest correct signature
  ```rust
  .with_help(format!("expected {} arguments, found {}", expected, found))
  ```

- [ ] **Line 217**: `index_out_of_bounds` — Suggest valid range
  ```rust
  .with_help(format!("valid indices are 0..{}", length))
  ```

- [ ] **Line 273**: `range_bound_not_int` — Suggest conversion
  ```rust
  .with_suggestion("convert to int with `int(value)`")
  ```

- [ ] **Line 311**: `for_requires_iterable` — Suggest wrapping
  ```rust
  .with_suggestion("try wrapping in a list: `[value]`")
  .with_suggestion("or call `.iter()` if available")
  ```

---

## 09.5 Fix Message Style Inconsistencies

### ori_patterns/src/errors.rs

- [ ] **Line 166**: Uses "expects" (present tense)
  - Change to "expected" for consistency

- [ ] **Line 177**: "expects a {expected} argument"
  - Change to "expected {expected} argument"

### General Guidelines

- [ ] Audit all error messages for:
  - Consistent capitalization (lowercase start)
  - No trailing periods
  - Consistent tense (past for what happened)
  - Backticks around code: `` `identifier` ``

---

## 09.6 Add Context to Errors

### ori_patterns/src/errors.rs

- [ ] **Line 130**: `division_by_zero` — Add operation context
  ```rust
  // Before:
  "division by zero"

  // After:
  "division by zero in expression `{expr}`"
  ```

- [ ] **Line 160**: `no_such_method` — List available methods
  ```rust
  // Before:
  "no method `{name}` on type `{ty}`"

  // After:
  "no method `{name}` on type `{ty}`"
  .with_note("available methods: {available}")
  ```

- [ ] **Line 287**: `map_keys_must_be_strings` — Add location
  ```rust
  .with_label(span, "this key is not a string")
  ```

---

## 09.7 Verification

- [ ] Grep for errors without `.with_span()` — should be zero
- [ ] Grep for "Did you mean" — should be zero (use imperative)
- [ ] Review error output for common operations
- [ ] `./clippy-all` passes
- [ ] `./test-all` passes

---

## 09.N Completion Checklist

- [x] EvalError has span field and with_span() builder
- [x] Error factories use `#[cold]` annotation
- [x] Messages use consistent style (lowercase, no periods)
- [x] Messages include relevant context (type names, operator symbols)
- [ ] Incremental: Add spans to more error sites (ongoing)
- [ ] Incremental: Add "try using" suggestions (ongoing)
- [ ] Incremental: Expand terse messages (ongoing)
- [x] `./test-all` passes

**Exit Criteria:** ✅ Infrastructure complete; incremental improvements tracked as future work
