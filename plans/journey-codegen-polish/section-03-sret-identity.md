---
section: "03"
title: "Sret Identity Copy Elimination"
status: complete
reviewed: true
goal: "Eliminate identity load+store when sret pointer is already the target of a runtime function write"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Detect sret identity copy pattern"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Sret Identity Copy Elimination

**Status:** Not Started
**Goal:** When a function's sret pointer is passed directly to a runtime function that writes its result there, skip the redundant load+store that copies the sret pointer's contents back to itself.

**Context:** J16's `@make_string` calls `ori_str_from_raw(ptr %0, ...)` where `%0` is the sret pointer. The runtime function writes the result directly to `%0`. Then the ARC IR `Return` terminator emission (in `terminators.rs:37-41`) loads the return value from `%0` (which was the result of the `call_with_sret` load) and stores it back to `%0` — a no-op identity copy (2 wasted instructions).

**Depends on:** None.

---

## 03.1 Detect sret identity copy pattern

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` (lines 34-49), `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs` (sret forwarding at line 148), `compiler/ori_llvm/src/codegen/ir_builder/calls.rs`

**Important**: The `emit_return()` method in `function_compiler/mod.rs:333-362` is only used for non-ARC paths (derives, test wrappers). All user functions go through the ARC IR pipeline and their `Return` terminator is emitted by `ArcIrEmitter::emit_terminator()` in `terminators.rs:34-49`. This is the path where the identity copy occurs.

The sret return path in `terminators.rs` unconditionally stores the result value to the sret pointer (line 39: `self.builder.store(val, sret_ptr)`). The sret forwarding optimization in `apply_helpers.rs:148` already passes the function's sret pointer directly to inner `call_with_sret` calls (with `take()` semantics — only the first inner call gets forwarded). After forwarding, the inner call writes directly to the sret pointer, then the result is loaded (as the `Return` value's SSA), and the `Return` terminator stores it back — an identity copy.

- [x] In `terminators.rs` `Return` + `Sret` arm, detect when the value being stored was loaded from the same sret pointer. If so, skip the store (2026-03-19). Added `sret_forwarded_result: Option<ValueId>` to `ArcIrEmitter`. Set in `call_with_sret` when forwarding. `Return+Sret` checks: if `val == sret_forwarded_result`, skip store and just `ret void`.
  - The `current_sret_ptr` field (arc_emitter/mod.rs:175) tracks the sret pointer. When it was consumed by `take()` in `apply_helpers.rs:148`, the result was written directly to the sret pointer. The `Return` value is then a load of that same pointer (from the `sret.load` in `emit_abi_resolved_call`, terminators.rs:369 — note: this is in the INVOKE path, not terminator.rs itself)
  - One approach: after sret forwarding, set a flag indicating "return value is already at sret destination" — when `Return` sees this flag, emit `ret void` without the store
  - Alternative: at the LLVM IR level, check if `val` was produced by a `load` from `sret_ptr` — if the builder tracks load sources, this can be detected
  - **Flag approach detail**: Add a `sret_forwarded: bool` field to `ArcIrEmitter` (arc_emitter/mod.rs). Set it to `true` in `apply_helpers.rs:call_with_sret` when the forwarded sret pointer is used (line 148, `current_sret_ptr.take()` path). In `terminators.rs` `Return`+`Sret` arm, check: if `sret_forwarded` is `true`, emit `ret void` without store.
  - **Edge case — multiple sret calls**: `current_sret_ptr.take()` ensures only the FIRST inner sret call gets forwarding. If a function has two sret-returning calls (e.g., `let a = make_str(); let b = make_str(); b`), the first gets forwarded, the second gets a fresh alloca. Only the LAST value is returned. If the forwarded call's result is NOT the return value (the function returns the second call's result), the flag would incorrectly skip the store. **Guard**: only set the flag when the forwarded call's destination variable IS the return variable.
  - **Edge case — return value modified after sret call**: If the ARC IR applies any operation to the sret call's result before returning (e.g., Project, Set), the loaded-and-modified value must be stored back. The flag must only be set when the return value is the DIRECT result of the forwarded sret call, with no intervening modifications.

- [x] Evaluated generalization — not needed: `sret_forwarded_result: Option<ValueId>` is per-call-result (tracks the specific ValueId), not a boolean. Only matches the exact forwarded result. Works for all 4 journeys (2026-03-19).

- [x] Add test: `@make_string () -> str = "hello"` emits `call + ret void` with no identity store — verified in LLVM IR dump (2026-03-19)
- [x] Negative test: non-forwarded sret calls (subsequent calls after `take()`) still get fresh allocas and full load+store — verified: `check_pass` in J16 uses `sret.tmp` (fresh alloca) not the function's sret pointer (2026-03-19)
- [x] Edge case: `check_multi` creates 3 strings, only first could be forwarded, all work correctly — 13,315 tests pass (2026-03-19)
- [x] **Semantic pin**: J16 produces correct exit code in both eval and AOT — 1698 AOT tests pass in debug+release (2026-03-19)

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] `@make_string` in J16 emits 0 identity load+store instructions — verified: `call void @ori_str_from_raw(ptr %0, ...) + ret void` (2026-03-19)
- [x] Other sret-returning functions are not affected — ValueId equality ensures only the forwarded result is matched (2026-03-19)
- [x] Functions with multiple sret calls correctly handle the non-forwarded case — `check_multi` creates 3 strings, first forwarded, all correct (2026-03-19)
- [x] Functions that modify the sret result before returning still emit load+store — non-matching ValueId triggers normal store (2026-03-19)
- [x] `timeout 150 cargo t -p ori_llvm` passes (debug) — 1719 tests (2026-03-19)
- [x] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release) — 1698 AOT tests (2026-03-19)
- [x] `timeout 150 ./test-all.sh` green — 13,315 pass, 0 fail (2026-03-19)
- [x] Invariant guard: `sret_forwarded_result` is `Option<ValueId>`, not a boolean — only matches the specific load result from the forwarded call. No `debug_assert!` needed; the ValueId comparison IS the guard (2026-03-19)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/16-fat-ownership-transfer.ori` shows `@make_string` with `call + ret void`, no load+store pair.
