---
section: "02"
title: "Type Checker"
status: not-started
goal: "Track and resolve all known type checker bugs"
sections: []
---

# Section 02: Type Checker

**Subsystem:** `compiler/ori_types/`

Bugs in type inference, unification, trait resolution, method dispatch, generics, bounds checking, and error reporting.

---

## Open Bugs

- [ ] `[BUG-02-001][high]` **infer_if() allows non-void then-branch without else** — found by verify-roadmap.
  Repro: `@main () -> void = { if true then 42 }` — compiles without error, but spec requires `if` without `else` to have type `void` or `Never` in the then-branch.
  Subsystem: `compiler/ori_types/src/infer/expr/control_flow.rs` (lines 53-63)
  Found: 2026-03-28 | Source: verify-roadmap
  Note: Active work in roadmap section 10 (control flow) touches this area.

- [x] `[BUG-02-002][high]` **Coalesce (`??`) same-type wrapper forms typed as `T` instead of wrapper** — found by tpr-review.
  Repro: `let a: Option<int> = Some(1); let b: Option<int> = Some(2); let c: Option<int> = a ?? b;` — fails with `expected int?, found int`. Same for `Result<T,E> ?? Result<T,E>`.
  Spec: `operator-rules.md` §Coalesce defines `Option<T> ?? Option<T> -> Option<T>` and `Result<T,E> ?? Result<T,E> -> Result<T,E>`.
  Resolved: Fixed on 2026-03-30. Type checker now uses Idx identity (pool interning) to detect same-type wrapper chaining. ARC lowering passes through LHS when result type is Option/Result (chaining) instead of extracting payload. 12 new spec tests + 3 Rust unit tests.

---

## Resolved Bugs

- None.
