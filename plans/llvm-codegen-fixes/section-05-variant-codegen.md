---
section: "05"
title: "Variant Codegen"
status: complete
goal: "Variant construction uses insertvalue (no alloca roundtrip); identical match arms deduplicated"
depends_on: []
sections:
  - id: "05.1"
    title: "Fix M7 — insertvalue for variant construction"
    status: complete
  - id: "05.2"
    title: "Fix M8 — Deduplicate identical match arms"
    status: complete
  - id: "05.3"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Variant Codegen

**Context:** J6 showed that creating `Success(42)` takes 12 instructions (alloca, store tag, GEP, store value, load tag, insertvalue, load payload, insertvalue) when it could be 2 `insertvalue` instructions. J6 also showed that `Success` and `Failure` match arms compile to identical code (same offset extraction) but aren't merged. This also fixes M14 (uninitialized payload) as a side effect — `insertvalue` doesn't read uninitialized memory.

**Cross-section:** This subsumes the M14 fix from Section 02 for unit variants. Unit variant payloads use `zeroinitializer` (not `undef`) — this is the consistent invariant across the plan. While `undef` in unused positions is technically well-defined LLVM IR, `zeroinitializer` is strictly safer and matches Section 02's recommendation.

---

## 05.1 Fix M7 — insertvalue for Variant Construction

**Journey:** J6 (confirmed J6, J11, J12) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (variant construction)

Replace:
```llvm
; Current — 12 instructions:
%variant = alloca { i64, [1 x i64] }, align 8
%tag.ptr = getelementptr inbounds ..., ptr %variant, i32 0, i32 0
store i64 0, ptr %tag.ptr, align 4
%payload.ptr = getelementptr inbounds ..., ptr %variant, i32 0, i32 1, i64 0
store i64 42, ptr %payload.ptr, align 4
%tag = load i64, ptr %tag.ptr, align 4
%s0 = insertvalue { i64, [1 x i64] } zeroinitializer, i64 %tag, 0
%payload = load i64, ptr %payload.ptr, align 4
%s1 = insertvalue { i64, [1 x i64] } %s0, i64 %payload, 1, 0
```

With:
```llvm
; Target — 2 instructions:
%s0 = insertvalue { i64, [1 x i64] } zeroinitializer, i64 0, 0           ; tag = 0 (Some)
%s1 = insertvalue { i64, [1 x i64] } %s0, i64 42, 1, 0                   ; payload = 42
```

- [x] Find the variant construction code in codegen
- [x] Replace alloca+store+load pattern with direct `insertvalue` chain
- [x] Handle unit variants: `insertvalue` with tag only, payload left as `zeroinitializer` (avoids M14)
- [x] Handle multi-field payload variants (e.g., `Rect(w: int, h: int)`)
- [x] Verify: Journey 6 `Success(42)` compiles to 2 insertvalue instructions
- [x] Verify: Journey 12 `None` construction has no uninitialized load

---

## 05.2 Fix M8 — Deduplicate Identical Match Arms

**Journey:** J6 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (match codegen)

In Journey 6, the `extract` function's `Success` and `Failure` arms generate identical code (both extract payload[0] from the same offset). They should share a single block.

```llvm
; Current — 2 identical blocks:
bb2:                                   ; Success arm
  %proj.alloca = alloca %ori.Result2, ...
  store %ori.Result2 %0, ptr %proj.alloca
  %proj.payload = getelementptr ... i32 0, i32 1
  %proj.1 = load i64, ptr %proj.payload
  br label %bb1
bb3:                                   ; Failure arm — IDENTICAL!
  %proj.alloca1 = alloca %ori.Result2, ...
  store %ori.Result2 %0, ptr %proj.alloca1
  %proj.payload2 = getelementptr ... i32 0, i32 1
  %proj.14 = load i64, ptr %proj.payload2
  br label %bb1
```

Target: single block when the codegen for different arms is structurally identical.

- [x] Analyze: how common is this pattern? (Only when all arms extract from the same offset)
- [x] Alternative: LLVM's MergeIdenticalBlocks pass may handle this — check if it does
- [x] Confirmed: LLVM O2 eliminates the switch entirely (both arms extract same value → dead switch)
- [x] If LLVM handles it: consider this LOW priority (optimization passes clean it up) — CONFIRMED: no codegen change needed

---

## 05.3 Completion Checklist

- [x] Variant construction uses `insertvalue` chain (no alloca+store+load)
- [x] Unit variant construction produces no uninitialized loads (M14 fixed as side effect)
- [x] Match arm deduplication evaluated (deferred to LLVM passes — O2 eliminates dead switches)
- [x] `./test-all.sh` green (11,955 pass, 1 flaky unrelated to changes)
- [x] `diagnostics/valgrind-aot.sh` — 0 errors (7/7 pass)

**Exit Criteria:** `grep -c "alloca.*variant" *.ll` shows no variant construction allocas. Journey 6 `Success(42)` compiles to 2-3 instructions.
