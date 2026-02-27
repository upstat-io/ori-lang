---
section: "09"
title: "IR Cleanliness"
status: not-started
goal: "No dead branches after calls; trivial if/else uses select; no single-predecessor phis"
depends_on: []
sections:
  - id: "09.1"
    title: "Fix M3 — Dead branches after function calls"
    status: not-started
  - id: "09.2"
    title: "Fix L3 — select for trivial if/else"
    status: not-started
  - id: "09.3"
    title: "Fix L4 — Single-predecessor phi elimination"
    status: not-started
  - id: "09.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: IR Cleanliness

**Status:** Not Started
**Goal:** Generated IR is clean and minimal at -O0 level. No dead branches, no unnecessary phis, no branch+phi where `select` suffices.

**Context:** M3 (dead `br label` after every function call) is the most universally confirmed finding — present in ALL 12 journeys. L3 (branch+phi instead of select) and L4 (single-predecessor phi) add IR noise that LLVM optimizes away but makes debugging and IR inspection harder.

**Note:** All of these are optimization-pass-fixable. The value is in producing cleaner -O0 IR for debugging.

---

## 09.1 Fix M3 — Dead Branches After Function Calls

**Journey:** J1 (confirmed ALL 12 journeys) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (block-structured codegen, call emission)

Every function call emits a `br label %nextBB` to the immediately following block. This is because the block-structured codegen starts a new basic block after every function call, even when not needed.

```llvm
; Current:
%call = call fastcc i64 @_ori_add(i64 3, i64 4)
br label %bb1       ; ← dead branch to next block
bb1:
%mul = mul i64 %call, 5
```

**Fix:** Don't start a new basic block after a `call` instruction (only after `invoke`, which has two successors). A `call` always falls through to the next instruction.

- [ ] Find where new basic blocks are created after `call` instructions
- [ ] Change: only create new BB after `invoke` (has normal + unwind successors)
- [ ] Verify: Journey 1 `_ori_main` has no `br label` after `call` to `_ori_add`
- [ ] Verify: `invoke` still correctly creates new BB (for ARC functions)
- [ ] Count: total dead branches eliminated across all 12 journeys

---

## 09.2 Fix L3 — select for Trivial if/else

**Journey:** J2 (confirmed J9) | **Severity:** LOW
**File(s):** `compiler/ori_llvm/src/codegen/` (if/else codegen)

Trivial `if b then 1 else 0` compiles to branch+phi instead of `select`:

```llvm
; Current — 6 instructions:
br i1 %0, label %bb1, label %bb2
bb1: br label %bb3
bb2: br label %bb3
bb3: %v4 = phi i64 [ 1, %bb1 ], [ 0, %bb2 ]

; Target — 1 instruction:
%v4 = select i1 %0, i64 1, i64 0
```

**Fix:** When both if/else branches are single-value expressions (no side effects, no function calls), emit `select` instead of branch+phi.

- [ ] Detect when if/else branches are side-effect-free single values
- [ ] Emit `select` for these cases
- [ ] Verify: Journey 2 `my_max` uses `select` for simple comparison
- [ ] Verify: Complex if/else (with function calls) still uses branch+phi

---

## 09.3 Fix L4 — Single-Predecessor Phi Elimination

**Journey:** J6 (confirmed J7) | **Severity:** LOW
**File(s):** `compiler/ori_llvm/src/codegen/` (phi node emission)

Match codegen emits phi nodes with only one incoming edge:

```llvm
bb3:
  %v2 = phi i64 [ %sel2, %bb0 ]    ; only one predecessor — phi is useless
  ret i64 %v2
```

**Fix:** When building a phi, if there's only one predecessor, use the value directly instead of creating a phi.

- [ ] Find where phi nodes are constructed in match codegen
- [ ] Check: if only one incoming edge, skip phi and use the value directly
- [ ] Verify: Journey 6 `to_code` has no single-predecessor phi

---

## 09.4 Completion Checklist

- [ ] No `br label %nextBB` immediately after `call` instructions
- [ ] Trivial if/else expressions use `select`
- [ ] No single-predecessor phi nodes in match codegen
- [ ] `./test-all.sh` green
- [ ] All 12 journeys produce correct results

**Exit Criteria:** Dead `br label` count across all 12 journeys drops from ~40+ to 0. IR is readable at -O0 level.
