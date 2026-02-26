---
section: "02"
title: "IR Emission Cleanup"
status: not-started
goal: "Eliminate unnecessary IR artifacts: eager declarations, dead blocks, redundant branches"
inspired_by:
  - "LLVM lazy symbol resolution (JIT engines resolve symbols on first call)"
  - "Rust rustc_codegen_llvm dead block elimination (compiler/rustc_codegen_llvm/src/mir/block.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Lazy Runtime Declarations"
    status: not-started
  - id: "02.2"
    title: "Dead Unreachable Block Elimination"
    status: not-started
  - id: "02.3"
    title: "Redundant Match Arm Branches"
    status: not-started
  - id: "02.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: IR Emission Cleanup

**Status:** Not Started
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

- [ ] Create `RuntimeDeclarations` struct with cached `Option<FunctionValue>` for each runtime function
  ```rust
  pub struct RuntimeDeclarations<'ctx> {
      module: &'ctx Module,
      // Lazily populated:
      ori_print: OnceCell<FunctionValue<'ctx>>,
      ori_rc_alloc: OnceCell<FunctionValue<'ctx>>,
      // ... one field per runtime function
  }
  ```

- [ ] Replace `declare_runtime()` with lazy `get_or_declare_*()` methods
  ```rust
  impl<'ctx> RuntimeDeclarations<'ctx> {
      pub fn ori_print(&self) -> FunctionValue<'ctx> {
          *self.ori_print.get_or_init(|| {
              // declare_extern_function(module, "ori_print", ...)
          })
      }
  }
  ```

- [ ] Update all call sites in `arc_emitter/mod.rs` to use lazy accessors instead of name-based lookups
  - Search for `module.get_function("ori_*")` pattern
  - Replace with `self.runtime.ori_*()` calls

- [ ] Test: compile `@main () -> int = 33` — verify 0 unused `declare` statements in IR
- [ ] Test: compile a program using `print()` — verify only `ori_print` (and transitive deps) declared
- [ ] Verify no regressions: `./llvm-test.sh`

---

## 02.2 Dead Unreachable Block Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Finding #5** (MEDIUM): When `invoke` is downgraded to `call` for nounwind callees, the former unwind-target blocks become unreachable. Currently they're emitted with a single `unreachable` instruction (lines 484-496). One dead block per nounwind call site.

**Current behavior:** The emitter pre-scans for dead unwind blocks (`dead_unwind` set) and emits them as `unreachable`. This is correct but wasteful.

**Target behavior:** Skip emitting dead blocks entirely. If a block has no predecessors (after nounwind downgrading), don't create the LLVM basic block at all.

- [ ] Instead of emitting `unreachable` for dead blocks, skip them entirely
  ```rust
  // Current (lines 484-496):
  if dead_unwind.contains(&block_id) {
      self.builder.position_at_end(self.block(block_id));
      self.builder.unreachable();
      continue;
  }

  // Target:
  if dead_unwind.contains(&block_id) {
      continue;  // Don't create the LLVM block at all
  }
  ```

- [ ] Ensure `self.block(block_id)` is never called for dead blocks
  - The block map must not pre-create LLVM blocks for dead blocks
  - Or: defer LLVM block creation to first use (lazy, like runtime decls)

- [ ] Handle edge case: a dead unwind block might be referenced by phi nodes in other blocks
  - Pre-scan: verify dead blocks have zero incoming edges (they should, by definition)
  - If a phi references a dead block's value, that phi arm is also dead

- [ ] Test: compile Journey 2 program — verify zero `unreachable` blocks in IR
- [ ] Test: compile program with mix of nounwind and may-unwind calls — only may-unwind unwind blocks emitted
- [ ] Verify no regressions: `./llvm-test.sh`

---

## 02.3 Redundant Match Arm Branches

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Finding #7** (MEDIUM): Match arm bodies each get their own basic block containing only `br label %merge`. The switch instruction targets these single-branch blocks instead of targeting the merge block directly. LLVM's SimplifyCFG folds these at `-O1`+, but at `-O0` they remain.

**Current IR pattern:**
```llvm
switch i64 %scrutinee, label %default [
  i64 0, label %case.0
  i64 1, label %case.1
]
case.0:
  br label %merge    ; redundant — could jump directly to merge
case.1:
  br label %merge
merge:
  %result = phi i64 [%val0, %case.0], [%val1, %case.1]
```

**Target IR pattern:**
```llvm
switch i64 %scrutinee, label %default [
  i64 0, label %merge    ; direct
  i64 1, label %merge
]
merge:
  %result = phi i64 [%val0, %entry], [%val1, %entry]  ; predecessors are entry
```

**Note:** This optimization only applies when the match arm body is empty (just produces a value and branches). If the arm body has side effects or multiple instructions, the separate block is necessary.

- [ ] Detect trivial match arms: arm block contains only a value computation and `br`
- [ ] For trivial arms, emit the value computation inline and add phi predecessor directly
- [ ] For non-trivial arms, keep the current block-per-arm structure
- [ ] Test: compile Journey 6 program — verify no single-instruction `br` blocks for simple cases
- [ ] Verify: programs with side-effectful match arms still get separate blocks
- [ ] Verify no regressions: `./llvm-test.sh`

---

## 02.4 Completion Checklist

- [ ] `@main () -> int = 33` produces ≤5 `declare` statements (only what's actually called)
- [ ] Zero `unreachable` blocks in IR for programs with only nounwind calls
- [ ] Simple pattern match: no single-instruction `br label %merge` blocks
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./llvm-clippy.sh` green

**Exit Criteria:** Journey 1 program (`let x = 10; let y = 3; x * y + 3`) produces IR with 0 unused declarations, 0 dead blocks, and 0 redundant branches. All test suites pass with zero regressions.
