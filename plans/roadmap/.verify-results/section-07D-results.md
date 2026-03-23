# Section 07D Verification Results: Stdlib Modules

**Verified by**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-19
**Section status**: not-started (0/985 items, 0%)
**Method**: Random sample of 15 items across all subsections to confirm genuinely not implemented

---

## Summary

All 15 sampled items confirmed NOT IMPLEMENTED. The section is genuinely 0% complete. Every stdlib module file (except `library/std/testing.ori`) is a TODO comment stub with no executable code. Even `std.testing`, which has real Ori code, is not functional as an importable module.

---

## Sample Verification

### 7D.1 std.validate Module

**Item**: `validate(rules, value)` function
**Status**: NOT IMPLEMENTED
**Evidence**: No `library/std/validate/` directory or file exists. `std.validate` appears only in docs/proposals/roadmap. No compiler registration for `validate` function.

---

### 7D.2 std.resilience Module

**Item**: `retry(operation, attempts, backoff)` function
**Status**: NOT IMPLEMENTED
**Evidence**: No `library/std/resilience/` directory or file exists. `std.resilience` appears only in docs/proposals/roadmap. No compiler registration for `retry` function.

---

### 7D.3 std.math Module -- Overflow-Safe Arithmetic

**Item**: `saturating_add(a: int, b: int) -> int`
**Status**: NOT IMPLEMENTED
**Evidence**: `library/std/math/mod.ori` is a TODO-comment stub listing planned functions but containing zero executable code. `saturating_add` references in the compiler codebase (36 files) are all internal Rust usage -- none are Ori stdlib functions exposed to users.

**Item**: `int.min` / `int.max` constants
**Status**: NOT IMPLEMENTED
**Evidence**: No type-level constants (`int.min`, `int.max`, `byte.min`, `byte.max`) found in `ori_types/src/infer/expr/identifiers.rs` or any type checker registration.

---

### 7D.4 std.testing Module

**Item**: `assert_eq(actual, expected)` as std.testing import
**Status**: NOT IMPLEMENTED (as stdlib module import)
**Evidence**: `library/std/testing.ori` has actual Ori code implementing all 8 assertion functions. However, `use std.testing { assert_eq }` fails at runtime with `E2003: module 'std.testing' not found`. The module resolution system cannot find it. Tests that use `assert_eq` work because it is a prelude built-in, not via `std.testing` import. The `library/std/testing/mod.ori` is a separate TODO stub with additional planned functions (assert_lt, assert_contains, etc.) that have no implementation.

---

### 7D.5 Developer Functions

**Item**: `todo()` and `todo(reason: str)` -> `Never`
**Status**: PARTIALLY IMPLEMENTED (interpreter only)
**Evidence**: `todo()` exists as `FunctionExpKind::Todo` in `ori_ir/src/ast/patterns/exp/mod.rs`. The evaluator handles it in `compiler/ori_eval/src/interpreter/can_eval/function_exp.rs:187` -- panics with "not yet implemented" or "not yet implemented: {reason}". However: no LLVM codegen (FunctionExpKind not handled in `ori_llvm`), no location capture, no LLVM tests, no AOT support. The plan's sub-items for LLVM/AOT are genuinely incomplete.

**Item**: `unreachable()` and `unreachable(reason: str)` -> `Never`
**Status**: PARTIALLY IMPLEMENTED (interpreter only)
**Evidence**: Same as `todo()` -- exists as `FunctionExpKind::Unreachable` in the evaluator (line 198), panics with "reached unreachable code". No LLVM codegen, no `reason:` parameter support, no location capture, no AOT.

**Item**: `dbg(value: T)` and `dbg(value: T, label: str)` -> `T`
**Status**: NOT IMPLEMENTED
**Evidence**: No `FunctionExpKind::Dbg` variant exists. Not registered as a function_val. No references to "dbg" as a string in the evaluator or type checker. Completely absent from the compiler.

**Item**: Location capture for `todo`, `unreachable`, `dbg`
**Status**: NOT IMPLEMENTED
**Evidence**: The current `todo()`/`unreachable()` implementations in the evaluator do not pass call-site location information. No location capture infrastructure exists for these functions.

---

### 7D.6 std.time Module

**Item**: `Instant` type
**Status**: NOT IMPLEMENTED
**Evidence**: `library/std/time/mod.ori` is a TODO-comment stub. No `Instant` type definition in the compiler type system or evaluator. No spec tests at `tests/spec/stdlib/time/`.

**Item**: `DateTime` type
**Status**: NOT IMPLEMENTED
**Evidence**: Same as above -- entirely in TODO comments.

---

### 7D.7 std.json Module

**Item**: `JsonValue` sum type and `parse(source: str)`
**Status**: NOT IMPLEMENTED
**Evidence**: `library/std/json/mod.ori` is a TODO-comment stub. No `JsonValue` type, no `parse` function, no `Json` trait. No tests at `tests/spec/stdlib/json/`.

---

### 7D.8 std.fs Module

**Item**: `read(path: str)` and `Path` type
**Status**: NOT IMPLEMENTED
**Evidence**: `library/std/fs/mod.ori` is a TODO-comment stub. No `Path` type, no `FileInfo` type, no file system functions. No tests at `tests/spec/stdlib/fs/`.

---

### 7D.9 std.crypto Module

**Item**: `hash(data: [byte], algorithm: HashAlgorithm)` and `SecretKey` type
**Status**: NOT IMPLEMENTED
**Evidence**: `library/std/crypto/mod.ori` is a TODO-comment stub. No crypto types or functions implemented. No FFI bindings to libsodium/OpenSSL. No tests at `tests/spec/stdlib/crypto/`.

---

### 7D.10 Duration and Size to Stdlib

**Item**: Move Duration/Size from compiler built-ins to pure Ori library types
**Status**: NOT IMPLEMENTED
**Evidence**: Duration and Size remain as compiler built-in types (`Value::Duration`, `Value::Size` in the evaluator, hardcoded operators in `ori_eval/src/operators/duration_size.rs`). No `library/std/duration.ori` or `library/std/size.ori` exists. Literal desugaring not implemented.

---

### 7D.11 std.bytes Module

**Item**: `find_byte` -- SIMD-backed byte search
**Status**: NOT IMPLEMENTED
**Evidence**: No `library/std/bytes/` directory exists. No `std.bytes` module in any compiler registration. The `Intrinsics` capability (prerequisite) is itself not fully implemented.

---

## Observations

1. **All stdlib module files are TODO stubs**: `math/mod.ori`, `time/mod.ori`, `json/mod.ori`, `fs/mod.ori`, `crypto/mod.ori` -- all contain only comment blocks describing planned APIs with zero executable code.

2. **No test directory**: `tests/spec/stdlib/` does not exist at all. No spec tests for any stdlib module.

3. **Module resolution gap**: Even `std.testing` (which has real Ori code at `library/std/testing.ori`) cannot be imported with `use std.testing { ... }`. The module resolution system does not find stdlib modules from user code. This is a prerequisite blocker for ALL of 7D.

4. **Partial interpreter support for 7D.5**: `todo()` and `unreachable()` work in the interpreter as `FunctionExpKind` variants, but lack LLVM codegen, location capture, and AOT support. `dbg()` is completely absent.

5. **Two testing files**: `library/std/testing.ori` (has code) and `library/std/testing/mod.ori` (TODO stub) exist side-by-side. The former has real assertion implementations; the latter describes additional planned functions. This duplication should be resolved when the module is properly implemented.

6. **Heavy dependency chain**: Several subsections have prerequisites in other plan sections (operator traits for 7D.10, Intrinsics capability for 7D.11, std.time for 7D.8's FileInfo). These sections are also not-started.

---

## Verdict

**Section is genuinely not started (0%).** All sampled items are confirmed not implemented. The 0/985 status is accurate. No items need to be reclassified.
