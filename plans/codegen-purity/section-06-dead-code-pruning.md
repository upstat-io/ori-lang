---
section: "06"
title: "Dead Code Pruning"
status: in-progress
goal: "No dead loads (unused struct/list fields) and no code generation after noreturn calls"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/operand.rs — loads only accessed fields via OperandValue::Ref"
  - "Zig src/codegen.zig — skips codegen after noreturn call/trap"
depends_on: ["02"]
sections:
  - id: "06.1"
    title: "Surgical Struct Field Loading"
    status: complete
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

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` (parameter binding at lines 213–238), `compiler/ori_llvm/src/codegen/ir_builder/memory.rs` (new `load_struct_selective` method)

> **TDD requirement:** Write IR-quality tests asserting current (broken) behavior FIRST. Verify they capture the over-loading. Then implement the fix and verify tests change to the expected pattern. Do NOT implement first.

Instead of loading all fields of a struct into an aggregate, load only the fields that are referenced by the function.

**Decision (2026-03-05): Approach (b) — pre-scan usage analysis.** Lazy loading (a) breaks the pipeline invariant that `self.var(id)` returns a value, not a pointer. Every instruction handler relies on this contract. Pre-scan preserves it: the emitter still loads an aggregate at function entry — it just loads fewer fields. Downstream code is unaware anything changed.

**How it works:**
1. Before parameter binding, scan all `ArcInstr::Project { value, field }` in the function to build `HashMap<ArcVarId, HashSet<u32>>` of accessed fields per variable.
2. Also scan `Apply`/`ApplyIndirect`/`Construct` args — if a struct param is passed whole (not via `Project`), all fields must be loaded.
3. During `Indirect`/`Reference` param loading (`emit_function.rs:223–230`), call a new `IrBuilder::load_struct_selective(ty, ptr, &used_fields)` that only emits GEP+load+insert_value for fields in the used set. Unaccessed fields get `undef` in the aggregate.
4. The aggregate shape is unchanged — downstream code sees the same type.

- [x] Implement `scan_used_fields(func: &ArcFunction) -> HashMap<ArcVarId, HashSet<u32>>` in `emit_function.rs`
- [x] Include `Apply`/`ApplyIndirect`/`Construct` arg scanning (whole-struct passthrough = all fields used)
- [x] Add `load_struct_selective(ty, ptr, used_fields, name)` to `IrBuilder` in `memory.rs`
- [x] Wire selective loading into `Indirect`/`Reference` parameter binding in `emit_function.rs`
- [x] Verify: J4 `_ori_area` only loads `width` and `height`, not `origin.x`/`origin.y`
- [x] Verify: J10 `_ori_count_items` only loads `length`, not `capacity` or `data_ptr`

### 06.1 Completion Checklist

- [x] Struct parameters: only referenced fields are loaded from memory
- [x] J4 `_ori_area` loads exactly 2 fields (not 4)
- [x] J10 `_ori_count_items` loads exactly 1 field (length, not 3)
- [x] IR test: function accessing 1 of 4 struct fields emits 1 load (not 4)
- [x] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for surgical field loading
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## 06.2 Skip Codegen After Noreturn Calls

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` (block emission loop), `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs` (call emission — can detect noreturn callees), `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` (source of truth for noreturn status)

After emitting a call to a known-noreturn function (e.g., `ori_panic`, `ori_panic_cstr`), immediately terminate the **normal** path (`unreachable`) and stop generating normal-path code in that block. Do not emit cleanup, drop, or continuation code on the impossible normal-return edge.

**Dependency:** Requires §02.1 to land first (provides `Attr::Noreturn` and `is_rt_fn_noreturn()`).

**Three categories of noreturn call sites to handle:**
1. **`emit_checked_binop()` overflow panic** — ALREADY handled correctly. `arithmetic.rs` emits `call ori_panic_cstr` + `unreachable` + positions at continue block. No fix needed.
2. **Runtime panic calls outside overflow** — e.g., zero-step loop guard, OOB index. These call `ori_panic`/`ori_panic_cstr` through `Apply` instructions in ARC IR. The ARC emitter's `emit_apply` does not check for noreturn.
3. **User `panic()` calls** — `panic(msg: "reason")` in Ori source. These lower to `Apply` calling `ori_panic`. Same path as (2).

**Implementation approach:** In the ARC emitter's call emission path (`apply.rs` or `emit_function.rs`), after emitting a `call` to a function proven `noreturn` via `is_rt_fn_noreturn()`, emit `unreachable` and skip remaining instructions in that block.

- [ ] Use `is_rt_fn_noreturn()` from §02.1 to query noreturn status of runtime functions at call sites
- [ ] In ARC emitter call emission: after calling a noreturn function, emit `unreachable` and stop emitting the current block
- [ ] Handle the ARC IR block structure: remaining instructions AND terminator after the noreturn call must be skipped
- [ ] Do not emit drop/cleanup code after the unreachable on the normal path
- [ ] Keep existing cleanup behavior for unwind paths where applicable (do not conflate `nounwind` and `noreturn`) — panic functions are `noreturn` but may still unwind for RC cleanup
- [ ] Verify `emit_checked_binop()` already handles this correctly (no change needed there)
- [ ] Verify: J7 panic path (bb6) has no code after `ori_panic()` call
- [ ] Verify: user `panic()` calls also get `unreachable` after the call

### 06.2 Completion Checklist

- [ ] No instructions emitted after noreturn calls on the normal path
- [ ] J7 panic path (bb6) has `call @ori_panic_cstr(...)` + `unreachable` only
- [ ] Unwind paths for RC cleanup are preserved (not affected by noreturn pruning)
- [ ] IR test: function with explicit `panic()` has `unreachable` immediately after the call
- [ ] IR test: function with `if cond then panic(msg: "x") else value` — the panic arm has `unreachable`, the else arm continues normally
- [ ] Regression test: `emit_checked_binop` overflow path still has `unreachable` (guard against breaking the existing correct behavior)
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for no code after noreturn
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Dependency Note

§06.2 (Skip Codegen After Noreturn) has a hard dependency on §02.1 (noreturn on Panic Functions). §02.1 MUST land before §06.2 begins — no partial implementation with hardcoded function names. The `is_rt_fn_noreturn()` query is the proper abstraction.

§06.1 (Surgical Struct Field Loading) has NO dependency on §02 and can proceed independently.

## Section 06 Exit Criteria

IR dumps show no `load` instructions for struct fields that are never used in the function body. No instructions follow `ori_panic`/`ori_panic_cstr` calls except `unreachable`.
