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

---

## Resolved Bugs

- None.
