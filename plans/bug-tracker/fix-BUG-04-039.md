---
bug: "BUG-04-039"
title: "LLVM codegen: join on non-string iterators crashes (missing to_str_fn trampoline)"
severity: high
status: complete
goal: "Iterator.join(separator:) correctly converts non-string elements to strings in LLVM backend, producing identical output to the interpreter"
success_criteria:
  - "join on [int], [float], [bool] iterators produces correct output in both JIT and AOT"
  - "Existing string-element join tests continue to pass"
  - "All 8 tests in tests/spec/traits/iterator/join.ori pass under --backend=llvm"
  - "No regressions in test-all.sh"
subsystem: "compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs"
found: "2026-04-06"
source: "continue-roadmap"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-039 — LLVM codegen: join on non-string iterators crashes

**Status:** In Progress
**Severity:** High
**Goal:** `Iterator.join(separator:)` correctly converts non-string elements to strings via a compiler-generated `to_str` trampoline in the LLVM backend, matching interpreter behavior.

**Success Criteria:**
- [ ] join on `[int]`, `[float]`, `[bool]` iterators produces correct output
- [ ] Existing string-element join tests continue to pass
- [ ] All 8 tests in `tests/spec/traits/iterator/join.ori` pass under `--backend=llvm`
- [ ] AOT tests for non-string join pass in both debug and release
- [ ] No regressions in `test-all.sh`

**Context:** `emit_iter_join` in the LLVM codegen bails with a codegen error for non-string element types (line 514-524 of `iterator_consumers.rs`). It always passes `null` for the `to_str_fn` trampoline, which causes the runtime to interpret raw int/bool/float bytes as 24-byte `OriStr` structs — SIGSEGV. The runtime `ori_iter_join` already supports a `to_str_fn` callback with signature `(env, elem_ptr, out_ptr) -> void`. The fix is to generate a type-specific trampoline in the LLVM codegen that reads the element and calls the appropriate `ori_str_from_*` runtime function. The interpreter's `eval_iter_join` handles this correctly via `eval_method_call(val, to_str, [])`.

---

## 1. Root Cause Analysis

- **Symptom**: SIGSEGV when calling `.join(separator:)` on non-string iterators (e.g., `[1,2,3].iter().join(separator: ", ")`) via LLVM backend. Currently produces `LCFail` because a guard was added to emit a codegen error instead of crashing.
- **Proximate cause**: `emit_iter_join` passes `null` for `to_str_fn` and `null` for `to_str_env` to `ori_iter_join`. The runtime interprets raw element bytes (8-byte int) as 24-byte `OriStr` structs.
- **Root cause**: No trampoline generation exists for the `to_str` conversion path in the LLVM codegen. Other consumer trampolines (map, filter, fold) wrap user-provided closures, but join needs a compiler-generated conversion function specific to the element type.
- **Blast radius**: All non-string `join` calls in LLVM backend. The codegen error guard (added to prevent SIGSEGV) causes `LCFail` for all 3 non-string tests in `join.ori`, which poisons the entire JIT module and fails all 8 tests in the file.
- **Affected files**:
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs` — generate `to_str` trampoline, remove codegen-error guard, pass correct elem_size
  - `compiler/ori_llvm/tests/aot/iterators.rs` — add AOT tests for non-string join
  - `compiler/ori_llvm/tests/aot/fixtures/iterators/` — add fixture `.ori` files

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] `[1, 2, 3].iter().join(separator: ", ")` → `"1, 2, 3"` (int join)

### Edge cases
- [ ] Empty list join: `[].iter().join(separator: ", ")` → `""` (empty int list)
- [ ] Single element: `[42].iter().join(separator: ", ")` → `"42"`

### Cross-type coverage
- [ ] `[int]` join: `[1, 2, 3].iter().join(separator: ", ")` → `"1, 2, 3"`
- [ ] `[float]` join: `[1.5, 2.5].iter().join(separator: "-")` → `"1.5-2.5"`
- [ ] `[bool]` join: `[true, false, true].iter().join(separator: " ")` → `"true false true"`

### Cross-feature interactions
- [ ] Join after map (int → int): `[1,2,3].iter().map(x -> x*10).join(separator: "-")` → `"10-20-30"`
- [ ] Join after filter: `[1,2,3,4,5].iter().filter(x -> x % 2 == 0).join(separator: "+")` → `"2+4"`

### Semantic pin
- [ ] AOT test: int join produces correct string (would fail if `to_str_fn` is null)

### Negative pin
- [ ] The old codegen-error guard no longer fires for supported primitive types

### Verify tests fail before fix
- [ ] All new AOT tests fail (or LCFail in JIT) against current code

---

## 3. Implementation

Generate a `to_str` trampoline function for non-string element types:

- [ ] Add `generate_to_str_trampoline(elem_ty: Idx) -> Option<FunctionId>` method
  - Signature: `(env: ptr, elem_ptr: ptr, out_ptr: ptr) -> void`
  - Reads element from `elem_ptr` (handling narrowed int types, sext/fpext)
  - Calls `ori_str_from_int/float/bool/char` with `out_ptr` as sret argument
  - Returns `None` for unsupported types (structs, user types — future work)

- [ ] Update `emit_iter_join` to use the trampoline:
  - Remove the codegen-error guard for primitive types
  - Generate trampoline via `generate_to_str_trampoline(elem_ty)`
  - Use correct `elem_size` for the source type (not str's 24 bytes)
  - Pass trampoline fn ptr and null env to the runtime
  - Keep codegen-error guard for unsupported types (structs, closures, etc.)

- [ ] Add AOT test fixtures and Rust test entries

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified — int, float, bool types all tested
- [ ] Debug AND release builds pass
- [ ] Interpreter and LLVM produce identical results for all new tests
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on join test programs
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push`
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]`
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` open bug count updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `[1,2,3].iter().join(separator: ", ")` produces `"1, 2, 3"` in both JIT and AOT modes, all 8 tests in `tests/spec/traits/iterator/join.ori` pass under `--backend=llvm`, AOT tests for int/float/bool join pass in debug and release, and `test-all.sh` is green with 0 regressions.
