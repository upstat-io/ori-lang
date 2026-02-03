---
section: "05"
title: Panic Elimination
status: in-progress
priority: critical
goal: Replace panic! with Result for all recoverable errors
files:
  - compiler/ori_ir/src/span.rs
  - compiler/ori_ir/src/interner.rs
  - compiler/ori_ir/src/arena.rs
  - compiler/ori_types/src/type_interner.rs
  - compiler/ori_eval/src/methods/*.rs
---

# Section 05: Panic Elimination

**Status:** 🔄 In Progress (05.1-05.4 ✅, 05.5-05.7 remaining)
**Priority:** CRITICAL — Panics on user input cause poor error experience and potential data loss
**Goal:** Replace all `panic!` on recoverable errors with proper error handling

---

## Background

The code review found `panic!` calls in core infrastructure that can be triggered by:
- Invalid span ranges (user input)
- Interner capacity exceeded (large programs)
- Arena capacity exceeded (deeply nested AST)

These should return `Result` and propagate errors to the diagnostic system.

---

## 05.1 Span Validation ✅

Location: `compiler/ori_ir/src/span.rs`

**COMPLETED** — Already implemented with proper error handling:

- [x] `SpanError` enum exists with `StartTooLarge` and `EndTooLarge` variants
- [x] `try_from_range()` returns `Result<Self, SpanError>`
- [x] `from_range()` is the infallible wrapper (documented to panic)
- [x] `SpanError` implements `Display` and `std::error::Error`

---

## 05.2 Interner Capacity ✅

Location: `compiler/ori_ir/src/interner.rs`

**COMPLETED** — Already implemented with dual API (fallible + infallible):

- [x] `InternError::ShardOverflow` enum with `shard_idx` and `count`
- [x] `try_intern(&str)` returns `Result<Name, InternError>`
- [x] `try_intern_owned(String)` returns `Result<Name, InternError>` (zero-alloc path)
- [x] `intern(&str)` is the infallible wrapper (documented to panic)
- [x] `intern_owned(String)` is the infallible wrapper
- [x] `InternError` implements `Display` and `std::error::Error`

---

## 05.3 Type Interner Capacity ✅

Location: `compiler/ori_types/src/type_interner.rs`

**COMPLETED** — Already implemented with dual API:

- [x] `TypeInternError::ShardOverflow` enum with `shard_idx`
- [x] `try_intern(TypeData)` returns `Result<TypeId, TypeInternError>`
- [x] `intern(TypeData)` is the infallible wrapper (documented to panic)
- [x] `TypeInternError` implements `Display` and `std::error::Error`

---

## 05.4 Arena Capacity ✅

Location: `compiler/ori_ir/src/arena.rs`

**DESIGN DECISION** — Uses panic with `#[cold]` helpers for truly exceptional conditions:

- [x] `panic_capacity_exceeded()` — `#[cold]` helper for > 4 billion expressions
- [x] `panic_range_exceeded()` — `#[cold]` helper for > 65K list elements
- [x] Clear error messages with hex values for debugging
- [x] Documented capacity limits: `u32::MAX` expressions, `u16::MAX` list length

**Rationale**: Converting to `Result` would require all arena allocation call sites to handle
errors, adding significant complexity for a condition that can only occur with maliciously
crafted input (> 4 billion AST nodes). The `#[cold]` annotation ensures no performance impact
on the happy path. This follows Rust's own arena design philosophy.

---

## 05.5 Eval Error Spans ⚠️

**STATUS**: Infrastructure exists, consistent usage needed

The `EvalError` type already has a `span: Option<Span>` field and `with_span()` builder method.
Spans ARE being attached in `ori_patterns` for property evaluation errors. The evaluator
(`ori_eval`) should also attach spans when errors bubble up.

**Architectural Approach**: Spans should be attached at the **call site** where both the
error and the expression span are available, not in low-level method dispatch functions.

### Pattern (in interpreter):
```rust
// Attach span when error bubbles up from method dispatch
let result = dispatch_method(receiver, method, args);
result.map_err(|e| if e.span.is_none() { e.with_span(expr.span) } else { e })?
```

### Status:
- [x] `EvalError.span` field exists
- [x] `EvalError.with_span()` builder method exists
- [x] `ori_patterns` uses `with_span()` for property errors
- [ ] Audit `ori_eval` for consistent span attachment at error sites

---

## 05.6 Propagate Errors to Diagnostics

After converting to `Result`, errors need to flow to the diagnostic system:

- [ ] Add error codes for capacity errors (E9xxx internal errors)
  ```rust
  // In error_code.rs
  E9003, // Interner capacity exceeded
  E9004, // Arena capacity exceeded
  E9005, // Type interner capacity exceeded
  ```

- [ ] Create diagnostic conversion for infrastructure errors
  ```rust
  impl From<InternerError> for Diagnostic {
      fn from(err: InternerError) -> Self {
          Diagnostic::error(ErrorCode::E9003)
              .with_message("internal error: interner capacity exceeded")
              .with_note("this may indicate an extremely large source file")
      }
  }
  ```

---

## 05.7 Verification

- [ ] Grep for `panic!` in non-test code — should be minimal
- [ ] Grep for `unwrap()` on user input paths — should be zero
- [ ] `./clippy-all` passes
- [ ] `./test-all` passes

---

## 05.N Completion Checklist

- [x] `span.rs` returns `Result` for invalid ranges (`SpanError`, `try_from_range()`)
- [x] `interner.rs` returns `Result` for capacity exceeded (`InternError`, `try_intern()`)
- [x] `type_interner.rs` returns `Result` for capacity exceeded (`TypeInternError`, `try_intern()`)
- [x] `arena.rs` uses `#[cold]` panics for truly exceptional conditions (> 4B elements)
- [x] `EvalError` has span field and `with_span()` builder
- [ ] Evaluator consistently attaches spans at error sites
- [ ] Error codes added for infrastructure errors (E9xxx)
- [ ] `./test-all` passes

**Exit Criteria:** No `panic!` on recoverable user-triggered errors; all errors flow to diagnostic system with source spans
