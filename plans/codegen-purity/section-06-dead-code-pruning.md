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

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (field extraction via `emit_project`), `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` (struct parameter loading), `compiler/ori_llvm/src/codegen/ir_builder/aggregates.rs`

> **WARNING — HIGH COMPLEXITY / HIGH RISK:** This subsection changes how struct parameters are loaded from memory. This is a fundamental ABI-level change that affects every function receiving struct arguments. `define_phase.rs` is already 461 lines — adding lazy-load tracking could push it over the 500-line limit (BLOAT). Consider extracting struct parameter loading into a dedicated `param_loading.rs` submodule before implementing. Approach (a) introduces a new "by-pointer vs by-value" distinction that must be threaded through the entire emission pipeline. Both approaches require extensive AOT test coverage across all struct-using programs, not just the targeted journeys.

> **TDD requirement:** Write IR-quality tests asserting current (broken) behavior FIRST. Verify they capture the over-loading. Then implement the fix and verify tests change to the expected pattern. Do NOT implement first.

Instead of loading all fields of a struct into an aggregate, load only the fields that are referenced by the function.

Two approaches:
- **(a) Lazy field loading** (preferred): Don't load struct fields eagerly. When a field access (`extractvalue` or GEP) is emitted, load that field on-demand from the pointer. This requires tracking whether a value is "by-pointer" or "by-value" at the codegen level.
- **(b) Usage analysis**: Before emitting loads, scan the function body for field references and only load referenced fields.

- [ ] Choose approach: (a) lazy field loading (track by-pointer vs by-value in codegen state) or (b) pre-scan function body for field references before emitting loads. Document choice rationale.
- [ ] Implement the chosen approach for struct parameter field loading
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
