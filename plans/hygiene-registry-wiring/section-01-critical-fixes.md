---
section: "01"
title: "Critical Fixes (ori_llvm)"
status: done
goal: "Fix correctness bug in Result trait codegen and memory leaks in WASM JS wrapper"
depends_on: []
sections:
  - id: "01.1"
    title: "Result trait methods ignore err_ty"
    status: done
  - id: "01.2"
    title: "WASM JS wrapper string param leaks"
    status: done
  - id: "01.3"
    title: "Completion Checklist"
    status: done
---

# Section 01: Critical Fixes (ori_llvm)

**Status:** Not Started
**Goal:** Fix a correctness bug where Result trait methods use Ok type logic for Err variant comparisons, and fix two memory leaks in WASM JS wrapper string parameter handling.

**Context:** The `emit_result_equals`, `emit_result_compare`, and `emit_result_hash` functions accept an `err_ty: Idx` parameter but prefix it with `_` and never use it. When both values have the Err tag, the payload comparison still uses `ok_ty`, producing wrong results when `Ok` and `Err` have different types. The WASM JS wrapper leaks encoded string memory for void-return and String-return functions.

---

## 01.1 Result trait methods ignore err_ty

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls.rs:113-177`

The three Result trait methods (`emit_result_equals`, `emit_result_compare`, `emit_result_hash`) accept `err_ty: Idx` but prefix it with `_` and never use it. When comparing/hashing Err payloads, they use `ok_ty`'s logic instead of `err_ty`'s logic.

**Complexity note:** The current implementations use `select` (phi-like) for branching, which works when both branches produce the same type of value. The fix requires tag-based branching where each branch calls a different `emit_element_*` function for the payload. This requires LLVM basic blocks (if-then-else pattern), not just `select`. Use the same pattern as Option's `emit_option_compare` if one exists, or follow the `emit_result_compare` tag comparison pattern already in the function (the `tags_eq` / `select` pattern) but extended with basic blocks for the payload comparison.

- [ ] Write AOT test: `Result<int, str>` — compare `Err("a")` vs `Err("b")` using `equals`, `compare`, and `hash`. Current codegen will use int comparison logic on str payloads.
- [ ] In `emit_result_equals`: remove `_` prefix from `err_ty`, add tag-based branching — when both tags are Ok, use `ok_ty` for payload; when both are Err, use `err_ty` for payload. This requires LLVM basic block branching (if-then-else on tag value).
- [ ] In `emit_result_compare`: same fix — branch on tag to select `ok_ty` vs `err_ty` for payload comparison.
- [ ] In `emit_result_hash`: same fix — branch on tag to select `ok_ty` vs `err_ty` for `emit_element_hash`.
- [ ] Verify: AOT test passes with correct comparison behavior for both Ok and Err variants with different types.
- [ ] Verify: eval path (`ori_eval/src/methods/compare.rs`) already handles Err correctly (it dispatches on variant at runtime). Add a spec test in `tests/spec/` if one does not exist for `Result<int, str>` equality/comparison.
- [ ] Check callers in `compound_traits.rs` (lines 83, 94, 105, 115, 188, 225, 258) — they already pass both `ok` and `err` params, so no caller changes needed.

---

## 01.2 WASM JS wrapper string param leaks

**File(s):** `compiler/ori_llvm/src/aot/wasm/mod.rs` — `generate_js_wrapper()` function (~line 380)

The `generate_js_wrapper` function encodes string parameters into WASM memory (`encodeString(arg)`) and collects cleanup pointers, but the cleanup loop that frees these pointers is only reached by the fallback (non-void, non-string) return branch. Both the void-return branch (line 380-381) and the String-return branch (line 382-387) skip cleanup entirely.

- [ ] Move the string parameter cleanup loop (freeing `_str*.ptr`) to execute **before** the return-type branching, so all return paths free encoded string memory. For void returns, cleanup goes before the closing `}`. For value returns, cleanup goes between the call and the return statement. The simplest approach: store result in `_result` for all branches, cleanup, then return.
- [ ] Alternative: duplicate cleanup into each branch (less clean but preserves structure).
- [ ] Verify: generated JS wrapper for a void-return function with string params includes cleanup calls.
- [ ] Verify: generated JS wrapper for a non-void, non-string return function still works correctly (cleanup must happen after call but before return).

---

## 01.3 Completion Checklist

- [ ] `emit_result_equals` correctly uses `err_ty` for Err payload comparison
- [ ] `emit_result_compare` correctly uses `err_ty` for Err payload comparison
- [ ] `emit_result_hash` correctly uses `err_ty` for Err payload hashing
- [ ] WASM JS wrapper cleanup runs for all return types (void, String, other)
- [ ] AOT tests pass: `cargo test -p ori_llvm`
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `Result<int, str>` equals/compare/hash AOT tests pass correctly differentiating Ok vs Err payload types. WASM-generated JS wrapper includes cleanup for string params regardless of return type.
