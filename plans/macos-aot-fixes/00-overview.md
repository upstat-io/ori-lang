---
status: in-progress
reviewed: false
---

# macOS AOT Fixes

## Context

CI run `23420458239` (PR #88) passed Linux fully but failed on macOS with 2 AOT test failures + Windows had a timeout. The string concat exponential capacity bug is already fixed. These are the remaining issues.

## Checklist

- [ ] **Fix 1**: `arc::test_rc_project_merge_edge_scoped_cleanup` (exit 1) — implementation landed in commit `9d323f30`, reopened by `TPR-01-001`
- [ ] **Fix 2**: `elem_dec_scope::test_trampoline_map_str_identity` (SIGSEGV -139) — implementation landed across `b53f147b..HEAD`, reopened by `TPR-02-001` and `TPR-02-002`
- [x] **Fix 3**: CI cross-platform timeout (10→30 min) — already done in `.github/workflows/ci.yml`
- [ ] All 3 fixes committed, pushed, CI green

---

## Fix 1: Merge-Edge Scoped Cleanup (REOPENED)

Commit `9d323f30` fixed the original root cause: `DeferredDec` emitted RC decrements on all successor edges instead of scoping them to the correct merge-edge successor. That work added `target_block` to `DeferredDec`.

Third-party review on `2026-03-23` found a remaining gap: multi-field escaping projections from the same parent aggregate still collapse to one compensation path. Section 01 stays open until `TPR-01-001` is resolved and covered by a regression.

---

## Fix 2: Trampoline Map Str Identity (REOPENED)

**Root cause:** ARM64 sret calling convention mismatch in iterator trampolines.

On ARM64 AAPCS64, the `sret` (struct return) pointer goes in register X8, NOT in a regular parameter register (X0). The closure's lambda function uses explicit `sret` attribute (for types >16 bytes like `str` = 24 bytes), but the trampoline was calling it without sret — either passing `out_ptr` as a regular first parameter (X0) or using implicit struct return (which LLVM returns in X0/X1/X2 registers). Both cause register misalignment:

- Lambda expects: X8=sret_out, X0=env, X1=input
- Trampoline was providing: X0=out_ptr, X1=env, X2=input (no sret)

On x86_64 this worked by coincidence because sret uses RDI (same register as first parameter).

**Fix** (4 files):

1. **`ir_builder/calls.rs`**: Added `call_indirect_with_sret()` — indirect call with explicit `sret` + `noalias` attributes on the first parameter, ensuring LLVM places it in X8 on ARM64.

2. **`arc_emitter/builtins/trampolines.rs`**: Map and Fold trampolines now use `call_indirect_with_sret` when `result_is_indirect` (return type >16 bytes), instead of `call_indirect_void` or `call_indirect` with struct return.

3. **`arc_emitter/closures.rs`**: Closure wrappers now use explicit sret when the callee uses sret — declared as `void(ptr sret, ptr env, ...)` instead of `RetTy(ptr env, ...)`. This ensures wrappers and lambdas have the same ABI from the trampoline's perspective.

4. **`arc_emitter/apply.rs`**: `emit_apply_indirect` now uses sret for closures returning large types (>16 bytes), matching the updated wrapper ABI.

Third-party review on `2026-03-23` found two remaining gaps in that follow-up:

- wrapper declarations now mark the hidden sret pointer `noundef`, which conflicts with the existing IR-quality rule
- indirect sret closure calls inside SEH funclets bypass the required `"funclet"` operand bundle on the Windows/MSVC path

Section 02 stays open until `TPR-02-001` and `TPR-02-002` are resolved with targeted coverage.

---

## Fix 3: CI Timeout (DONE)

Already applied: `.github/workflows/ci.yml` line 198 changed from `timeout-minutes: 10` to `timeout-minutes: 30`.

---

## Files Involved

| File | Role |
|------|------|
| `compiler/ori_llvm/src/codegen/ir_builder/calls.rs` | New `call_indirect_with_sret` method |
| `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs` | Trampoline sret calling |
| `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs` | Wrapper sret passthrough |
| `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs` | ApplyIndirect sret for large returns |
| `compiler/ori_llvm/tests/aot/elem_dec_scope.rs:148` | Test (semantic pin) |
