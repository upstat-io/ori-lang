---
section: "06"
title: "Stdlib"
status: not-started
goal: "Track and resolve all known standard library bugs"
sections: []
third_party_review:
  status: findings
  updated: 2026-04-02
---

# Section 06: Stdlib

**Subsystem:** `library/std/`, `compiler/ori_registry/`

Bugs in standard library functions, prelude types, collection methods, iterator implementations, derive machinery, and registry definitions.

---

## Open Bugs

- [x] `[BUG-06-001][medium]` **assert_eq uses str() not debug() for error messages** — found by verify-roadmap.
  Resolved: Fixed on 2026-04-02. Changed `assert_eq` and `assert_ne` in `library/std/testing.ori` to use `.debug()` instead of `str()`, with `T: Eq + Debug` bound. Error messages now show quoted strings (e.g., `"hello" != "world"`). Tests: 3 spec tests (debug format semantic pin, assert_ne debug pin, regression guard). 15,002 tests passing.
  Subsystem: `library/std/testing.ori`
  Found: 2026-03-28 | Source: verify-roadmap

- [x] `[BUG-06-002][medium]` **compare/min/max are int-only, spec requires generic `<T: Comparable>`** — found by verify-roadmap.
  Resolved: Fixed on 2026-04-02. Changed `compare`, `min`, `max` in `library/std/prelude.ori` from int-only to generic `<T: Comparable>`. Parameter names updated from `a, b` to `left, right` per spec. `compare` now delegates to `left.compare(other: right)`. Also fixed Ordering binary operator dispatch in evaluator — added `Value::Ordering` to `value_to_type_tag()` + `eval_ordering_binary()` for `==`/`!=`. Tests: 9 spec tests (int regression + str/float semantic pins for each function). 15,002 tests passing.
  Subsystem: `library/std/prelude.ori`, `compiler/ori_eval/src/operators/mod.rs`
  Found: 2026-03-28 | Source: verify-roadmap

- [x] `[BUG-06-003][medium]` **assert_some/assert_ok/assert_err return void instead of inner value** — found by verify-roadmap.
  Resolved: Fixed on 2026-04-02. Changed `assert_some` to return `T`, `assert_ok` to return `T`, `assert_err` to return `E` in `library/std/testing.ori`. Match arms now bind and return inner values instead of `()`. Tests: 4 spec tests (return value for int, str, Result) + 3 negative pins (still panics on wrong variant). 15,002 tests passing.
  Subsystem: `library/std/testing.ori`
  Found: 2026-03-28 | Source: verify-roadmap

---

## 06.R Third Party Review Findings

- [x] `[TPR-06-001][high]` [library/std/prelude.ori](/home/eric/projects/ori_lang/library/std/prelude.ori#L357) and [library/std/testing.ori](/home/eric/projects/ori_lang/library/std/testing.ori#L10) — BUG-06-001/002/003 were closed without LLVM parity; the new generic stdlib helpers still compile-fail under `--backend=llvm`.
  Resolved: Root cause is BUG-04-022 (LLVM JIT cannot resolve free function calls in monomorphized generic bodies). The stdlib fixes (BUG-06-001/002/003) are correct — they changed the stdlib functions from int-only to generic `<T: Comparable>` / `<T: Eq + Debug>`. These work correctly through the interpreter. The LLVM failure is NOT a stdlib bug but a codegen infrastructure limitation: the JIT test runner can't compile ANY generic function whose body calls free functions (`debug()`, `str()`, etc.) not declared in the JIT module. This affects ALL generics through LLVM, not just these specific stdlib functions. BUG-04-022 tracks the root cause. The stdlib bugs are correctly resolved for the interpreter path; LLVM parity is blocked on BUG-04-022.

## Resolved Bugs

- None.
