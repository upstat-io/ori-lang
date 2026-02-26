---
section: "02"
title: "IR Emission Cleanup"
status: complete
goal: "Eliminate unnecessary IR artifacts: eager declarations, dead blocks, redundant branches"
inspired_by:
  - "LLVM lazy symbol resolution (JIT engines resolve symbols on first call)"
  - "Rust rustc_codegen_llvm dead block elimination (compiler/rustc_codegen_llvm/src/mir/block.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Lazy Runtime Declarations"
    status: complete
  - id: "02.2"
    title: "Dead Unreachable Block Elimination"
    status: complete
  - id: "02.3"
    title: "Redundant Match Arm Branches"
    status: complete
  - id: "02.4"
    title: "Completion Checklist"
    status: complete
---

# Section 02: IR Emission Cleanup

**Status:** Complete
**Goal:** Generated LLVM IR contains only declarations, blocks, and branches that are actually needed. A trivial program like `@main () -> int = 33` should produce near-minimal IR — no 98 unused `declare` statements, no dead `unreachable` blocks, no redundant `br` instructions.

**Context:** Journeys 1–6 consistently showed IR bloat: 98 runtime declarations even when zero are called (Journey 1), dead unreachable blocks from nounwind invoke→call downgrade (Journeys 2, 4, 5, 6), and redundant unconditional branches in match arms (Journey 6). While LLVM's optimizer removes all of these at `-O1`+, they clutter `-O0` output, slow debug builds, and make IR inspection harder during development.

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/callee.rs`: Runtime functions are declared lazily via `get_fn()` which caches and only emits `declare` on first use.
- **Zig** `src/Compilation.zig`: Extern declarations are demand-driven — only emitted when a function body references them.

**Depends on:** Section 01 (nounwind soundness must be correct before dead block elimination can trust the nounwind set).

---

## 02.1 Lazy Runtime Declarations

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`

**Finding #4** (MEDIUM): `declare_runtime()` eagerly declares all 98 runtime functions at module creation time, even when a program uses zero of them. Journey 1's `@main` returns constant `33` and still gets 98 `declare` statements.

**Current behavior:** `declare_runtime(&mut IrBuilder)` is called once at module init, producing 98 `declare` lines.

**Target behavior:** Each runtime function is declared on first use via a lazy accessor.

- [x] Create data-driven `RT_FUNCTIONS` table + `IrBuilder::runtime_fn()` cache
  - Static `RT_FUNCTIONS: &[RtFn]` table (98 entries) with `Ty`/`Attr` enums — single source of truth
  - `IrBuilder::runtime_fn(name)` with `FxHashMap<&'static str, FunctionId>` cache
  - Simpler than OnceCell struct: no new lifetime params, no threading through call chain

- [x] Replace `declare_runtime()` with lazy `declare_single()` / `runtime_fn()`
  - `declare_single(builder, name)` resolves from table, declares with correct signature + attrs
  - `declare_runtime()` retained for tests only (delegates to `declare_single`)
  - Removed eager `declare_runtime()` call from `evaluator.rs`

- [x] Update all call sites (~60) to use `runtime_fn()` instead of name-based lookups
  - Converted `get_function("ori_*") → intern_function()` pattern across 12 files
  - `rc_ops.rs`, `arc_emitter/mod.rs`, `drop_gen.rs`, `builtins/*.rs`, `derive_codegen/*.rs`, `entry_point.rs`
  - Added `try_runtime_fn(&str) -> Option<FunctionId>` for dynamic-name fallback paths in `arc_emitter`
  - Removed eager `declare_runtime()` from AOT path in `compile_common.rs` (2026-02-26)

- [x] Test: empty module has 0 declarations; single request declares only 1 function
- [x] Test: `runtime_fn()` caches FunctionId; lazy declaration preserves attributes
- [x] Verify no regressions: `./llvm-test.sh` — 383 unit + 1012 AOT tests pass

---

## 02.2 Dead Unreachable Block Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Finding #5** (MEDIUM): When `invoke` is downgraded to `call` for nounwind callees, the former unwind-target blocks become unreachable. Currently they're emitted with a single `unreachable` instruction (lines 484-496). One dead block per nounwind call site.

**Current behavior:** The emitter pre-scans for dead unwind blocks (`dead_unwind` set) and emits them as `unreachable`. This is correct but wasteful.

**Target behavior:** Skip emitting dead blocks entirely. If a block has no predecessors (after nounwind downgrading), don't create the LLVM basic block at all.

- [x] Instead of emitting `unreachable` for dead blocks, skip them entirely (2026-02-26)
  - Moved dead_unwind computation before block pre-creation
  - Dead check now before `position_at_end` — dead blocks never entered
  - Removed `self.builder.unreachable()` emission for dead blocks

- [x] Ensure `self.block(block_id)` is never called for dead blocks (2026-02-26)
  - `block_map` changed from `Vec<BlockId>` to `Vec<Option<BlockId>>`
  - Dead blocks map to `None`; `block()` panics with clear message if called for dead block
  - `emit_invoke` defers `self.block(unwind)` to only when callee is NOT nounwind

- [x] Handle edge case: a dead unwind block might be referenced by phi nodes in other blocks (2026-02-26)
  - Dead blocks are only reachable as unwind targets of nounwind invokes (invariant assertion)
  - Phi creation already skips dead blocks (pre-existing `dead_unwind.contains()` guard)
  - Phi incoming values never reference dead blocks (dead blocks never emitted)

- [x] Test: compile Journey 2 program — verify zero `unreachable` blocks in IR (2026-02-26)
  - `test_nounwind_program_has_no_unreachable_blocks` — pure arithmetic, zero unreachable
  - `test_nounwind_generic_call_no_unreachable` — nounwind generic, zero unreachable
  - `test_constant_main_minimal_ir` — constant return, zero unreachable/invoke/landingpad
- [x] Test: compile program with mix of nounwind and may-unwind calls — only may-unwind unwind blocks emitted (2026-02-26)
  - `test_mixed_calls_no_dead_unreachable` — nounwind `add` + may-unwind `may_panic`
- [x] Verify no regressions: `./llvm-test.sh` — 384 unit + 1,016 AOT tests pass (2026-02-26)

---

## 02.3 Redundant Match Arm Branches

**File(s):** `compiler/ori_arc/src/decision_tree/emit.rs`, `compiler/ori_arc/src/lower/mod.rs`, `compiler/ori_arc/src/ir/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Finding #7** (MEDIUM): Trivial match arms (each returning a constant/variable) each get their own basic block. For small matches on int/bool/char/tag, this creates unnecessary branch misprediction. Instead of folding at the LLVM emitter level, we implemented branchless `select` chains at the ARC IR level.

**Approach:** Added `ArcInstr::Select { dst, ty, cond, true_val, false_val }` to ARC IR, mapped to LLVM's `select` instruction. The decision tree emitter detects eligible switches (≤8 edges, all simple leaves, no bindings, no mutable vars) and emits a chain of `icmp eq` + `select` in a single block — completely branchless.

**Generated IR pattern (before):**
```llvm
switch i64 %x, label %default [i64 0, label %case.0  i64 1, label %case.1]
case.0: br label %merge
case.1: br label %merge
merge: %result = phi i64 [%val0, %case.0], [%val1, %case.1], [%default_val, %default]
```

**Generated IR pattern (after):**
```llvm
%eq = icmp eq i64 %x, 0
%sel = select i1 %eq, i64 10, i64 40       ; fallback = default
%eq1 = icmp eq i64 %x, 1
%sel2 = select i1 %eq1, i64 20, i64 %sel   ; chain
br label %merge
```

- [x] Add `ArcInstr::Select` to ARC IR + exhaustive match updates across production code (2026-02-26)
- [x] Add `ArcIrBuilder::emit_select()` convenience method (2026-02-26)
- [x] Wire LLVM emitter to emit `builder.select()` for `ArcInstr::Select` (2026-02-26)
- [x] Add `is_select_eligible()` — validates switch edges for select chain optimization (2026-02-26)
  - ≤8 edges, all Leaf nodes, empty bindings, simple bodies, no mutable vars
  - Only Int/Bool/Char/Tag test values (ListLen/Str/Float/IntRange excluded)
- [x] Add `emit_select_chain()` — emits branchless accumulator pattern (2026-02-26)
- [x] Add `emit_eq_test()` — emits type-correct equality comparison per TestValue kind (2026-02-26)
  - Uses `Idx::CHAR` + `LitValue::Char` for char (i32), `Idx::INT` for int/tag (i64)
- [x] Hook into `emit_int_switch()` and `emit_tag_switch()` (2026-02-26)
- [x] Borrow inference handles `ArcInstr::Select` (no ownership propagation needed) (2026-02-26)
- [x] Test: int match — `match x { 0->10, 1->20, 2->30, _->40 }` emits 3 select, 0 switch (2026-02-26)
- [x] Test: char match — verified `i32` type consistency in icmp (2026-02-26)
- [x] Non-trivial arms (bindings, complex bodies) still get block-per-arm structure (2026-02-26)
- [x] Verify no regressions: `./test-all.sh` — 10,112 tests pass, 0 failures (2026-02-26)

---

## 02.4 Completion Checklist

- [x] `@main () -> int = 33` produces ≤5 `declare` statements (only what's actually called) — **0 declarations** (2026-02-26)
- [x] Zero `unreachable` blocks in IR for programs with only nounwind calls (2026-02-26)
- [x] Simple pattern match: no single-instruction `br label %merge` blocks (2026-02-26)
- [x] `./test-all.sh` green — 10,184 passed, 0 failed (2026-02-26)
- [x] `./llvm-test.sh` green — 1,460 passed, 0 failed (2026-02-26)
- [x] `./llvm-clippy.sh` green (2026-02-26)

**Exit Criteria:** Journey 1 program (`let x = 10; let y = 3; x * y + 3`) produces IR with 0 unused declarations, 0 dead blocks, and 0 redundant branches. All test suites pass with zero regressions.
