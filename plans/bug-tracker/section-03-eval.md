---
section: "03"
title: "Evaluator"
status: in-progress
goal: "Track and resolve all known evaluator/interpreter bugs"
sections: []
---

# Section 03: Evaluator

**Subsystem:** `compiler/ori_eval/`, `compiler/ori_patterns/`

Bugs in expression evaluation, method dispatch, iterator machinery, closure handling, pattern matching, and control flow.

---

## Open Bugs

- [ ] `[BUG-03-001][medium]` **Byte binary operations not implemented in evaluator** — found by tpr-review.
  Repro: `10 as byte == 10 as byte` fails at runtime with "cannot apply operator to `byte` and `byte`". All byte binary ops (arithmetic, comparison, bitwise) fail.
  Subsystem: `compiler/ori_eval/src/operators/mod.rs` — `evaluate_binary()` has no `(Value::Byte, Value::Byte)` match arm.
  Found: 2026-03-31 | Source: tpr-review (hygiene-full §01 eval sync tests)

---

## Resolved Bugs

- None.
