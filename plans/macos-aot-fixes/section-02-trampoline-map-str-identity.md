---
section: "02"
title: "Trampoline map str identity SIGSEGV"
status: in-progress
reviewed: false
goal: "Keep indirect sret closure calls ABI-correct across wrappers, trampolines, and SEH funclets for large return values such as `str`."
depends_on: []
third_party_review:
  status: findings
  updated: 2026-03-23
sections:
  - id: "02.1"
    title: "Indirect sret closure ABI"
    status: in-progress
  - id: "02.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "02.N"
    title: "Completion Checklist"
    status: pending
---

# Section 02: Trampoline map str identity SIGSEGV

**Status:** In Progress
**Goal:** Closure trampolines, wrappers, and `ApplyIndirect` must all agree on indirect-sret ABI details for fat returns, and that agreement must hold on ARM64 and Windows/MSVC funclet paths.

**Context:** The original SIGSEGV came from an ARM64 sret ABI mismatch in trampoline-mediated closure calls returning `str`, and the current code fixes the main failure path. Review of the follow-up changes found two ABI contract gaps that remain outside the existing regression suite: wrapper attributes on hidden sret params and funclet handling for indirect-sret closure calls on Windows/MSVC.

**Depends on:** None.

---

## 02.1 Indirect sret closure ABI

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs`, `compiler/ori_llvm/src/codegen/ir_builder/calls.rs`, `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs`

The ARM64 trampoline crash is fixed, but the follow-up ABI work is not yet closed. This subsection remains open until wrapper declarations preserve the repo's `sret` attribute contract and indirect-sret closure calls inside SEH funclets use the required operand bundle.

- [ ] Skip `noundef` on hidden wrapper sret parameters while keeping the intended attributes on the remaining wrapper arguments.
- [ ] Route `ret_is_indirect && current_funclet_pad.is_some()` through a funclet-aware indirect-sret call helper.
- [ ] Add targeted regression coverage for a capturing closure wrapper returning `str` and for Windows/MSVC SEH closure calls returning a fat value.
- [ ] Re-run the trampoline and IR-quality tests that cover the `str` indirect-sret path.

---

## 02.R Third Party Review Findings

- [ ] `[TPR-02-001][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:407` — Closure wrappers incorrectly mark the hidden sret pointer `noundef`.
  Evidence: The wrapper declaration loop adds `noundef` to every parameter, including the explicit sret pointer introduced for fat returns. The existing IR-quality rule in `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs` says the sret pointer must not get `noundef`, and a fresh `ORI_DUMP_AFTER_LLVM=1` build of a capturing closure returning `str` emitted `_ori_partial_1(ptr noalias noundef sret(...), ...)`.
  Impact: The wrapper ABI now disagrees with the repository's own attribute contract for sret parameters, which risks misoptimization and leaves wrapper declarations outside current regression coverage.
  Required plan update: Skip `noundef` on wrapper sret params, then add an IR-quality regression that inspects a capturing closure wrapper returning `str` or another >16-byte aggregate.

- [ ] `[TPR-02-002][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs:314` — Indirect sret closure calls inside SEH funclets bypass the required `"funclet"` operand bundle.
  Evidence: `emit_apply_indirect()` takes the `ret_is_indirect` path before checking `current_funclet_pad`, so the code always emits a plain `call_indirect_with_sret()` for fat returns. The emitter contract says all calls inside SEH pads must carry a funclet bundle, and the dedicated `call_indirect_with_funclet()` helper is documented as required for `ApplyIndirect` in that context.
  Impact: Windows MSVC `catch(expr:)` or cleanup paths that invoke closures returning fat values can emit invalid LLVM IR or incorrect unwind metadata, even though the non-SEH macOS tests pass.
  Required plan update: Add a funclet-aware indirect-sret call helper, route the `ret_is_indirect && current_funclet_pad.is_some()` path through it, and add Windows-targeted coverage for closure calls returning `str` inside `catch(expr:)`.

---

## 02.N Completion Checklist

- [ ] `TPR-02-001` is resolved in code and pinned by an IR-quality regression.
- [ ] `TPR-02-002` is resolved in code and pinned by Windows-targeted coverage or equivalent IR verification.
- [ ] `cargo test -p ori_llvm test_trampoline_map_str_identity -- --nocapture` passes.
- [ ] `cargo test -p ori_llvm test_trampoline_filter_str -- --nocapture` passes.
- [ ] `cargo test -p ori_llvm test_trampoline_fold_str -- --nocapture` passes.
- [ ] `cargo test -p ori_llvm test_trampoline_for_each_str -- --nocapture` passes.

**Exit Criteria:** All indirect-sret closure entry points agree on the ABI for large returns, wrapper IR no longer marks hidden sret pointers `noundef`, Windows/MSVC funclet calls carry the required bundle, and the relevant trampoline and IR-quality tests pass without platform-specific drift.
