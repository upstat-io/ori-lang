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

- [x] `[BUG-03-002][high]` **Option/Result closure methods (map, and_then, flat_map, filter, or_else) fail with "expects a function argument"** — found by continue-roadmap. Fixed 2026-04-01: added Option/Result match arms to CollectionMethodResolver + 9 new eval handlers (OptionMap, OptionAndThen, OptionFlatMap, OptionFilter, OptionOrElse, ResultMap, ResultMapErr, ResultAndThen, ResultOrElse) in collection_ops.rs.
  Repro: `Some(2).map(x -> x * 3)` → E6099 "map expects a function argument". All closure-taking methods on Option/Result fail at runtime.
  Subsystem: `compiler/ori_eval/src/methods/variants.rs:390-393` — `dispatch_option_method_str` catches these methods and returns `wrong_arg_type("function")` instead of deferring to the `CollectionMethodResolver` which has evaluator access for closure evaluation. The string-based dispatch path runs before the collection resolver and short-circuits.
  Found: 2026-04-01 | Source: continue-roadmap (hygiene-full §03.2 spec test writing)
  Note: Active work in roadmap section 07B and hygiene-full §03.2 touches this area.

---

## Resolved Bugs

- None.
