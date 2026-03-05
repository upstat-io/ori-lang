---
section: "06"
title: "Dead Code Pruning"
status: not-started
goal: "No dead loads (unused struct/list fields) and no code generation after noreturn calls"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/operand.rs — loads only accessed fields via OperandValue::Ref"
  - "Zig src/codegen.zig — skips codegen after noreturn call/trap"
depends_on: ["02"]
sections:
  - id: "06.1"
    title: "Surgical Struct Field Loading"
    status: not-started
  - id: "06.2"
    title: "Skip Codegen After Noreturn Calls"
    status: not-started
---

# Section 06: Dead Code Pruning

**Status:** Not Started
**Goal:** The codegen only loads struct/list fields that are actually used by the function, and emits no instructions after known-noreturn function calls (e.g., `ori_panic`).

**Context:** Two categories of dead code in the emitted IR:

1. **Dead field loads (L-5):** When a function receives a struct by pointer, the codegen loads ALL fields into an aggregate before extracting the needed ones. J4's `_ori_area` loads all 4 fields of `Rect` (including unused `origin.x` and `origin.y`) but only uses `width` and `height`. J10's `_ori_count_items` loads all 3 list fields but only uses length.

2. **Dead code after noreturn (L-7):** In J7's `_ori_sum_for`, the zero-step panic path (bb6) generates SSO/RC cleanup code after the `ori_panic()` call. Since `ori_panic` never returns, this code is unreachable. (Synergy with §02: once `ori_panic_cstr` has `noreturn`, LLVM can eliminate this automatically, but the codegen shouldn't emit it in the first place.)

**Note:** The checked arithmetic overflow path already handles this correctly (`emit_checked_binop()` emits panic call + `unreachable` with no trailing code). The issue is in other panic call sites outside overflow arithmetic (e.g., zero-step loop guard, explicit `panic()` calls from user code).

**Journeys affected:** J4, J7, J10.

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/mir/operand.rs`: Uses `OperandValue::Ref` to defer field loading until field access.
- **Zig** `src/codegen.zig`: After emitting a noreturn call, immediately terminates the block with `unreachable`.

---

## 06.1 Surgical Struct Field Loading

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`, `compiler/ori_llvm/src/codegen/ir_builder/aggregates.rs`

Instead of loading all fields of a struct into an aggregate, load only the fields that are referenced by the function.

Two approaches:
- **(a) Lazy field loading** (preferred): Don't load struct fields eagerly. When a field access (`extractvalue` or GEP) is emitted, load that field on-demand from the pointer. This requires tracking whether a value is "by-pointer" or "by-value" at the codegen level.
- **(b) Usage analysis**: Before emitting loads, scan the function body for field references and only load referenced fields.

- [ ] Determine which approach fits the current codegen architecture
- [ ] Implement on-demand field loading for struct parameters
- [ ] Verify: J4 `_ori_area` only loads `width` and `height`, not `origin.x`/`origin.y`
- [ ] Verify: J10 `_ori_count_items` only loads `length`, not `capacity` or `data_ptr`

### 06.1 Completion Checklist

- [ ] Struct parameters: only referenced fields are loaded from memory
- [ ] J4 `_ori_area` loads exactly 2 fields (not 4)
- [ ] J10 `_ori_count_items` loads exactly 1 field (length, not 3)
- [ ] IR test: function accessing 1 of 4 struct fields emits 1 load (not 4)
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for surgical field loading
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 06.2 Skip Codegen After Noreturn Calls

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

After emitting a call to a known-noreturn function (e.g., `ori_panic`, `ori_panic_cstr`), immediately terminate the **normal** path (`unreachable`) and stop generating normal-path code in that block. Do not emit cleanup, drop, or continuation code on the impossible normal-return edge.

- [ ] Track which runtime functions are noreturn (coordinate with §02.1 — requires `Attr::Noreturn` infrastructure)
- [ ] After emitting a call to a proven-noreturn function, emit `unreachable` and terminate the block
- [ ] Do not emit drop/cleanup code after the unreachable
- [ ] Keep existing cleanup behavior for unwind paths where applicable (do not conflate `nounwind` and `noreturn`) — panic functions are `noreturn` but may still unwind for RC cleanup
- [ ] Verify: J7 panic path (bb6) has no code after `ori_panic()` call

### 06.2 Completion Checklist

- [ ] No instructions emitted after noreturn calls on the normal path
- [ ] J7 panic path (bb6) has `call @ori_panic_cstr(...)` + `unreachable` only
- [ ] Unwind paths for RC cleanup are preserved (not affected by noreturn pruning)
- [ ] IR test: function with explicit `panic()` has `unreachable` immediately after the call
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for no code after noreturn
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 06 Exit Criteria

IR dumps show no `load` instructions for struct fields that are never used in the function body. No instructions follow `ori_panic`/`ori_panic_cstr` calls except `unreachable`.
