---
section: "06"
title: "Stdlib"
status: not-started
goal: "Track and resolve all known standard library bugs"
sections: []
---

# Section 06: Stdlib

**Subsystem:** `library/std/`, `compiler/ori_registry/`

Bugs in standard library functions, prelude types, collection methods, iterator implementations, derive machinery, and registry definitions.

---

## Open Bugs

- [ ] `[BUG-06-001][medium]` **assert_eq uses str() not debug() for error messages** — found by verify-roadmap.
  Repro: `assert_eq(actual: "hello", expected: "world")` — error message uses `to_str()` but spec says it should use `debug()` (which escapes strings).
  Subsystem: `library/std/testing.ori` (assert_eq implementation)
  Found: 2026-03-28 | Source: verify-roadmap
  Note: Active work in roadmap section 7A (core builtins) touches this area.

- [ ] `[BUG-06-002][medium]` **compare/min/max are int-only, spec requires generic `<T: Comparable>`** — found by verify-roadmap.
  Repro: `compare(left: "a", right: "b")` — fails because compare only accepts int, not generic Comparable types.
  Subsystem: `compiler/ori_eval/src/function_val.rs`, `compiler/ori_types/src/infer/expr/identifiers.rs`
  Found: 2026-03-28 | Source: verify-roadmap
  Note: Active work in roadmap section 7A (core builtins) touches this area.

- [ ] `[BUG-06-003][medium]` **assert_some/assert_ok/assert_err return void instead of inner value** — found by verify-roadmap.
  Repro: `let x = assert_some(option: Some(42))` — returns void, but spec says it should return the inner value (42).
  Subsystem: `library/std/testing.ori` (assert_some/assert_ok/assert_err implementations)
  Found: 2026-03-28 | Source: verify-roadmap
  Note: Active work in roadmap section 7A (core builtins) touches this area.

---

## Resolved Bugs

- None.
