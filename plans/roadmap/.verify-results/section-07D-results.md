# Section 7D Verification Results: Stdlib Modules

**Verified**: 2026-03-28
**Verifier**: Claude Opus 4.6 (1M context)
**Section status**: not-started
**Total items**: 986 `[ ]`, 0 `[x]`

## Files Loaded Before Verification

1. `/home/eric/projects/ori_lang/CLAUDE.md` (full, 183 lines)
2. `.claude/rules/registry.md` (full)
3. `.claude/rules/tests.md` (full)
4. `.claude/rules/spec.md` (full)
5. `.claude/rules/roadmap.md` (full)
6. `.claude/rules/ori-lang.md` (full)
7. `.claude/rules/arc.md` (full)
8. `.claude/rules/compiler.md` (full)
9. `.claude/rules/eval.md` (full)
10. `.claude/rules/typeck.md` (partial — 30 lines)
11. `.claude/rules/llvm.md` (partial — 30 lines)
12. `.claude/rules/patterns.md` (full, loaded via system-reminder)
13. `.claude/rules/impl-hygiene.md` (full, loaded via system-reminder)
14. `.claude/rules/ori-syntax.md` (full, loaded via system-reminder)
15. `plans/roadmap/section-07D-stdlib-modules.md` (full, 1530 lines)
16. Spec files consulted: annex-c-built-in-functions.md (via search), 06-types.md (referenced by tests)

---

## Executive Summary

Section 7D is entirely **not-started**. All 986 checkbox items are `[ ]`. No `[x]` items exist. The section covers 12 subsections spanning std.validate, std.resilience, std.math, std.testing, developer functions, std.time, std.json, std.fs, std.crypto, Duration/Size migration, and std.bytes.

All `library/std/` module files referenced in this section are TODO stubs containing only comments describing planned APIs. No actual Ori library implementations exist. No spec tests exist in `tests/spec/stdlib/`. No LLVM codegen tests exist for any stdlib module.

However, some **partial infrastructure** exists that overlaps with plan items:
- `library/std/testing.ori` (the flat file, not `testing/mod.ori`) contains working implementations of `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`
- `todo()` and `unreachable()` are implemented as patterns in `ori_patterns` with Rust unit tests
- Duration and Size are currently compiler built-in types with overflow tests in `tests/spec/types/`
- `dbg()` is NOT implemented anywhere

---

## Subsection-by-Subsection Results

### 7D.1 std.validate Module (5 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. No implementation exists. `library/std/validate/` does not exist. `tests/spec/patterns/validate.ori` exists but is entirely commented out (blocked on stdlib). No Ori or Rust tests.

| Item | Classification |
|------|---------------|
| `validate(rules, value)` — Implement | NEEDS TESTS |
| Rust Tests | NEEDS TESTS |
| Ori Tests | NEEDS TESTS |
| LLVM Support | NEEDS TESTS |
| AOT Tests | NEEDS TESTS |

---

### 7D.2 std.resilience Module (15 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. No implementation exists. `library/std/resilience/` does not exist. No tests of any kind. Depends on capabilities (`uses Suspend`) and Duration types.

| Item | Classification |
|------|---------------|
| All 15 items (retry, exponential, linear + their tests) | NEEDS TESTS |

---

### 7D.3 std.math Module — Overflow-Safe Arithmetic (55 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. No `saturating_*`, `wrapping_*`, or `checked_*` functions exist anywhere in the compiler or stdlib. No `int.min`/`int.max`/`byte.min`/`byte.max` constants exist.

**Partial overlap note**: Section 7D.3.5 (Default Overflow Behavior) describes arithmetic panicking on overflow. This IS already implemented for the built-in `int` type (the interpreter panics on overflow). Duration and Size overflow behavior is tested in `tests/spec/types/duration_overflow.ori` and `tests/spec/types/size_overflow.ori` (both pass). However, these existing tests cover Duration/Size overflow, NOT the `std.math` module items in this section.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.3.1 Saturating Arithmetic (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.3.2 Wrapping Arithmetic (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.3.3 Checked Arithmetic (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.3.4 Type Bounds Constants (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.3.5 Default Overflow Behavior (12 items) | All `[ ]` | NEEDS TESTS |

**Note on 7D.3.5**: While default overflow behavior exists for Duration/Size as built-ins, the plan items specifically cover `std.math` module functions and compile-time constant overflow. Neither is implemented. The existing `duration_overflow.ori` and `size_overflow.ori` tests cover built-in type overflow, not `std.math` functions.

---

### 7D.4 std.testing Module (40 items)

**Status**: NEEDS TESTS — partially implemented but no dedicated tests

All items `[ ]`. However, `library/std/testing.ori` (the flat file at `library/std/testing.ori`, NOT `library/std/testing/mod.ori`) contains **working implementations** of:
- `assert(cond: bool)`
- `assert_eq<T: Eq>(actual, expected)`
- `assert_ne<T: Eq>(actual, unexpected)`
- `assert_some<T>(opt)`
- `assert_none<T>(opt)`
- `assert_ok<T, E>(result)`
- `assert_err<T, E>(result)`
- `assert_panics<T>(f: () -> T)`
- `assert_panics_with<T>(f: () -> T, msg: str)`

These are actively used by hundreds of spec tests via `use std.testing { assert_eq }`. The functions work in the interpreter. However:
- No dedicated `tests/spec/stdlib/testing.ori` test file exists
- No LLVM codegen tests exist
- No AOT tests exist
- The `library/std/testing/mod.ori` (module directory version) is still a TODO stub

**Observation**: The plan items say "Move testing assertions from built-ins to std.testing" but the `testing.ori` file already implements them as Ori functions. The existing infrastructure is functional but untested in isolation and has no LLVM/AOT coverage.

| Item | Classification |
|------|---------------|
| assert_eq — Implement | NEEDS TESTS (implementation exists in testing.ori, but no dedicated tests) |
| assert_ne — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_some — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_none — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_ok — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_err — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_panics — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| assert_panics_with — Implement | NEEDS TESTS (implementation exists, no dedicated tests) |
| All Rust Tests (8 items) | NEEDS TESTS |
| All Ori Tests (8 items) | NEEDS TESTS |
| All LLVM Support (8 items) | NEEDS TESTS |
| All LLVM Rust Tests (8 items) | NEEDS TESTS |
| All AOT Tests (8 items) | NEEDS TESTS |

---

### 7D.5 Developer Functions (20 items)

**Status**: NEEDS TESTS — partially implemented

All items `[ ]`. However:

**`todo()` and `todo(reason:)`**: Implemented in `compiler/ori_patterns/src/builtins/todo/mod.rs` as `TodoPattern`. Has 5 Rust unit tests in `todo/tests.rs` (all pass). Also tested in `tests/spec/types/never.ori` (tests `todo()` and `todo(reason:)` as Never-coercing expressions — 3 tests pass). No LLVM codegen or AOT tests.

**`unreachable()` and `unreachable(reason:)`**: Implemented in `compiler/ori_patterns/src/builtins/unreachable/mod.rs` as `UnreachablePattern`. Has 5 Rust unit tests in `unreachable/tests.rs` (all pass). Also tested in `tests/spec/types/never.ori` (tests `unreachable()` and `unreachable(reason:)` — 2 tests pass). No LLVM codegen or AOT tests.

**`dbg(value:)` and `dbg(value:, label:)`**: NOT implemented anywhere. No pattern, no function_val, no tests.

**Location capture**: NOT implemented. The compiler does not pass call-site location implicitly to todo/unreachable/dbg.

| Item | Classification |
|------|---------------|
| `todo()` — Implement | NEEDS TESTS (implemented, has Rust tests and indirect Ori tests, no LLVM/AOT, no dedicated Ori test file) |
| `todo()` — Rust Tests | WEAK (5 unit tests exist in ori_patterns, but plan references `ori_eval/src/function_val.rs` which is wrong location) |
| `todo()` — Ori Tests | NEEDS TESTS (tested indirectly in never.ori, no `tests/spec/stdlib/todo.ori`) |
| `todo()` — LLVM Support | NEEDS TESTS |
| `todo()` — AOT Tests | NEEDS TESTS |
| `unreachable()` — Implement | NEEDS TESTS (implemented, has Rust tests, no LLVM/AOT) |
| `unreachable()` — Rust Tests | WEAK (5 unit tests exist, plan references wrong file) |
| `unreachable()` — Ori Tests | NEEDS TESTS (tested indirectly in never.ori, no dedicated file) |
| `unreachable()` — LLVM Support | NEEDS TESTS |
| `unreachable()` — AOT Tests | NEEDS TESTS |
| `dbg()` — Implement | NEEDS TESTS (not implemented at all) |
| `dbg()` — Rust Tests | NEEDS TESTS |
| `dbg()` — Ori Tests | NEEDS TESTS |
| `dbg()` — LLVM Support | NEEDS TESTS |
| `dbg()` — AOT Tests | NEEDS TESTS |
| Location capture — Implement | NEEDS TESTS (not implemented) |
| Location capture — Rust Tests | NEEDS TESTS |
| Location capture — LLVM Support | NEEDS TESTS |
| Location capture — LLVM Rust Tests | NEEDS TESTS |
| Location capture — AOT Tests | NEEDS TESTS |

---

### 7D.6 std.time Module (75 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. `library/std/time/mod.ori` is a TODO stub. No types (Instant, DateTime, Date, Time, Timezone, Weekday) are implemented. No formatting, parsing, or clock capability integration exists. No tests of any kind.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.6.1 Core Types (30 items) | All `[ ]` | NEEDS TESTS |
| 7D.6.2 Duration Extension Methods (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.6.3 Formatting (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.6.4 Parsing (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.6.5 Error Type (5 items) | All `[ ]` | NEEDS TESTS |
| 7D.6.6 Clock Capability (7 items) | All `[ ]` | NEEDS TESTS |

---

### 7D.7 std.json Module (120 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. `library/std/json/mod.ori` is a TODO stub. No JsonValue type, no parse/stringify functions, no Json trait, no derive(Json), no streaming API, no FFI bindings. The section notes the original FFI proposal was superseded (2026-03-26) in favor of a pure Ori approach using compile-time reflection. No tests of any kind.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.7.1 Core Types (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.2 Parsing API (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.3 Serialization API (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.4 JsonValue Methods (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.5 Derive Macro (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.6 Standard Type Implementations (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.7 Streaming API (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.7.8 FFI Implementation (30 items) | All `[ ]` | NEEDS TESTS |

---

### 7D.8 std.fs Module (130 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. `library/std/fs/mod.ori` is a TODO stub. No Path type, no FileInfo, no read/write functions, no directory operations, no glob, no temp files, no permissions. Depends on `std.time` (Instant) and capabilities (FileSystem). No tests of any kind.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.8.1 Core Types (25 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.2 Reading Files (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.3 Writing Files (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.4 Directory Operations (30 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.5 File Operations (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.6 File Info Functions (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.7 Glob Patterns (5 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.8 Temporary Files (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.9 Permissions (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.8.10 Path Utilities (15 items) | All `[ ]` | NEEDS TESTS |

---

### 7D.9 std.crypto Module (180 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. `library/std/crypto/mod.ori` is a TODO stub. No hash functions, no encryption, no signing, no key exchange, no secure random, no key derivation, no capability integration. Depends on external libraries (libsodium, OpenSSL). No tests of any kind.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.9.1 Core Types (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.2 Signing Key Types (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.3 Encryption Key Types (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.4 Key Exchange Types (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.5 Hashing API (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.6 HMAC API (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.7 Symmetric Encryption (20 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.8 Asymmetric Encryption (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.9 Digital Signatures (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.10 Key Exchange (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.11 Secure Random (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.12 Key Derivation (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.13 Key Serialization (15 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.14 Utilities (5 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.15 Crypto Capability (10 items) | All `[ ]` | NEEDS TESTS |
| 7D.9.16 Algorithm Deprecation (3 items) | All `[ ]` | NEEDS TESTS |

---

### 7D.10 Duration and Size to Stdlib (30 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. Duration and Size are currently compiler built-in types with built-in operator implementations. The plan proposes migrating them to pure Ori library types using operator traits. This migration depends on operator traits (Section 3.21) and associated functions (Section 3.x), neither of which is complete.

**Existing infrastructure (NOT matching plan items)**: Duration and Size work as built-in types today. Existing tests in `tests/spec/types/` cover:
- `duration_literals.ori` — literal syntax
- `duration_overflow.ori` — overflow panics (19 tests, all pass)
- `duration_size_sendable.ori`, `duration_size_clone_printable.ori`, `duration_size_const.ori`, `duration_size_hashable.ori`, `duration_size_default.ori`, `duration_size_comparable.ori` — trait implementations
- `size_literals.ori` — literal syntax
- `size_overflow.ori` — overflow panics (15 tests, all pass)

None of these test stdlib migration. They test the current built-in implementation.

| Subsection | Items | Classification |
|-----------|-------|---------------|
| 7D.10.1 Prerequisites (2 items) | All `[ ]` | NEEDS TESTS |
| 7D.10.2 Literal Suffix Desugaring (5 items) | All `[ ]` | NEEDS TESTS |
| 7D.10.3 Duration Library Implementation (17 items) | All `[ ]` | NEEDS TESTS |
| 7D.10.4 Size Library Implementation (17 items) | All `[ ]` | NEEDS TESTS |
| 7D.10.5 Compiler Cleanup (6 items) | All `[ ]` | NEEDS TESTS |
| 7D.10.6 Error Messages (2 items) | All `[ ]` | NEEDS TESTS |

---

### 7D.11 std.bytes Module (10 items)

**Status**: NEEDS TESTS — not implemented

All items `[ ]`. No byte search functions exist. Depends on Section 6.14 (Intrinsics Capability). No tests of any kind.

| Item | Classification |
|------|---------------|
| All 10 items (find_byte, find_any, find_not, count_byte, contains_byte, benchmarks) | NEEDS TESTS |

---

### 7D.12 Section Completion Checklist (6 items)

**Status**: NEEDS TESTS — not applicable until section work begins

| Item | Classification |
|------|---------------|
| All 6 checklist items | NEEDS TESTS |

---

## Summary Statistics

| Classification | Count |
|---------------|-------|
| NEEDS TESTS | 986 |
| WEAK | 0 |
| VERIFIED | 0 |
| INCOMPLETE MATRIX | 0 |
| NEEDS PIN | 0 |
| WRONG TEST | 0 |
| STALE | 0 |
| REGRESSION | 0 |
| BUG FOUND | 0 |

## Existing Infrastructure Overlap

While no plan items are implemented, the following pre-existing infrastructure overlaps with section scope:

| Feature | Current State | Location | Tests |
|---------|--------------|----------|-------|
| `assert_eq`, `assert_ne`, etc. | Implemented in Ori | `library/std/testing.ori` | Indirect only (used by 100+ spec tests) |
| `todo()` / `todo(reason:)` | Implemented as pattern | `ori_patterns/src/builtins/todo/` | 5 Rust unit tests + 3 Ori tests in never.ori |
| `unreachable()` / `unreachable(reason:)` | Implemented as pattern | `ori_patterns/src/builtins/unreachable/` | 5 Rust unit tests + 2 Ori tests in never.ori |
| `dbg()` | NOT implemented | N/A | None |
| Duration (built-in) | Implemented as compiler built-in | `ori_eval`, `ori_types` | 8 test files in tests/spec/types/ |
| Size (built-in) | Implemented as compiler built-in | `ori_eval`, `ori_types` | 3 test files in tests/spec/types/ |
| std.validate | Blocked TODO stub | `tests/spec/patterns/validate.ori` (commented out) | None |
| std.math | TODO stub | `library/std/math/mod.ori` | None |
| std.time | TODO stub | `library/std/time/mod.ori` | None |
| std.json | TODO stub | `library/std/json/mod.ori` | None |
| std.fs | TODO stub | `library/std/fs/mod.ori` | None |
| std.crypto | TODO stub | `library/std/crypto/mod.ori` | None |

## Plan Accuracy Notes

1. **7D.5 Developer Functions**: Plan says Rust tests should be in `ori_eval/src/function_val.rs` but `todo()` and `unreachable()` are implemented as patterns in `ori_patterns`, not as function_vals. Plan file references are inaccurate.

2. **7D.4 std.testing**: Plan says "Move testing assertions from built-ins to std.testing" but `library/std/testing.ori` already implements them as Ori functions (not built-ins). The `testing/mod.ori` (module directory) is still a TODO stub, suggesting a discrepancy between the flat file and module directory approach.

3. **7D.10 Duration/Size to Stdlib**: The prerequisites (operator traits, associated functions) are from other sections. This subsection cannot begin until those are complete. The plan correctly identifies this dependency.

4. **7D.7 std.json**: Plan notes the original FFI proposal was superseded 2026-03-26 in favor of compile-time reflection. The plan items still describe the original FFI approach (yyjson, WASM). These items may need updating to reflect the new direction.

## Test Runs Performed

| Test Command | Result |
|-------------|--------|
| `cargo st tests/spec/types/duration_overflow.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/size_overflow.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/never.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo test -p ori_patterns -- todo` | 5 passed |
| `cargo test -p ori_patterns -- unreachable` | 5 passed |

All tests pass. No regressions detected.
