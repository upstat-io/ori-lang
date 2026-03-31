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

- [ ] `[BUG-02-002][high]` **Coalesce (`??`) same-type wrapper forms typed as `T` instead of wrapper** — found by tpr-review.
  Repro: `let a: Option<int> = Some(1); let b: Option<int> = Some(2); let c: Option<int> = a ?? b;` — fails with `expected int?, found int`. Same for `Result<T,E> ?? Result<T,E>`.
  Spec: `operator-rules.md` §Coalesce defines `Option<T> ?? Option<T> -> Option<T>` and `Result<T,E> ?? Result<T,E> -> Result<T,E>`.
  Root cause: `infer_coalesce()` in `compiler/ori_types/src/infer/expr/operators.rs:284` always unifies result with the unwrapped inner type `T`. ARC lowering in `compiler/ori_arc/src/lower/expr/mod.rs:429` always projects payload field 1.
  Subsystem: `compiler/ori_types/src/infer/expr/operators.rs`, `compiler/ori_arc/src/lower/expr/mod.rs`
  Found: 2026-03-30 | Source: tpr-review
  Related: BUG-04-009 (resolved) fixed eager RHS evaluation but did not restore wrapper-preserving semantics.
  Note: Roadmap section 15C (literals/operators) and section 10 (control flow) touch this area.

---

## Resolved Bugs

- None.
