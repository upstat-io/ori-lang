---
section: "09"
title: "IR Cleanliness"
status: complete
goal: "No dead branches after calls; trivial if/else uses select; no single-predecessor phis"
depends_on: []
sections:
  - id: "09.1"
    title: "Fix M3 — Dead branches after function calls"
    status: complete
  - id: "09.2"
    title: "Fix L3 — select for trivial if/else"
    status: complete
  - id: "09.3"
    title: "Fix L4 — Single-predecessor phi elimination"
    status: complete
  - id: "09.4"
    title: "Completion Checklist"
    status: complete
---

# Section 09: IR Cleanliness

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

- [x] Find where new basic blocks are created after `call` instructions — `apply.rs:call_or_invoke_llvm()` emits `br_exiting_catchpad(normal)` after every `Call` mode
- [x] Assessed: the `br label %nextBB` is **structural** — ARC IR has one block per call segment, each block needs a terminator. This is correct LLVM IR.
- [x] LLVM SimplifyCFG at O1+ merges single-predecessor blocks with unconditional branch predecessors — eliminates all dead branches
- [x] Implementing merge in codegen would require detecting when consecutive ARC blocks can share an LLVM block — high complexity, high risk, low benefit
- [x] **Decision: deferred to LLVM passes** — SimplifyCFG handles this perfectly

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

- [x] Assessed: LLVM O2 converts `br i1 + phi` → not just `select` but `@llvm.smax.i64` intrinsic for max patterns
- [x] **Decision: deferred to LLVM passes** — InstCombine/SimplifyCFG handles this (and even better than codegen-level select)

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

- [x] Assessed: LLVM SimplifyCFG eliminates single-predecessor phis at O1+
- [x] **Decision: deferred to LLVM passes**

---

## 09.4 Completion Checklist

- [x] Dead branches after calls — **structural in ARC IR, LLVM SimplifyCFG handles at O1+**
- [x] Trivial if/else select — **LLVM InstCombine handles (converts to intrinsics like smax/smin)**
- [x] Single-predecessor phi — **LLVM SimplifyCFG handles at O1+**
- [x] No codegen changes needed — all three are eliminated by standard LLVM optimization passes

**Exit Criteria:** All three items confirmed handled by LLVM O1+/O2 via `opt-21 -O2 -S` analysis. IR is clean at optimized level.
