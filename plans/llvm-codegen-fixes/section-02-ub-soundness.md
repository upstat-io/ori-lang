---
section: "02"
title: "UB & Soundness"
status: complete
goal: "Zero LLVM undefined behavior in generated IR; nounwind analysis provably sound"
inspired_by:
  - "Rust rustc_codegen_llvm — nsw/nuw flag propagation on arithmetic"
  - "Zig src/Sema.zig — overflow detection and safety checks"
depends_on: []
sections:
  - id: "02.1"
    title: "Fix M14 — None variant uninitialized payload"
    status: complete
  - id: "02.2"
    title: "Fix H2 — Audit nounwind for runtime calls"
    status: complete
  - id: "02.3"
    title: "Fix M9 — Range overflow for ..=INT_MAX"
    status: complete
  - id: "02.4"
    title: "Add H3 — noalias on proven non-aliasing parameters"
    status: complete
  - id: "02.5"
    title: "Fix M2 — Checked arithmetic (overflow panics)"
    status: complete
  - id: "02.6"
    title: "Completion Checklist"
    status: complete
---

# Section 02: UB & Soundness

**Context:** These issues don't crash today but represent latent bugs. M14 loads `poison` values from uninitialized memory. H2 marks functions `nounwind` that call potentially-panicking runtime functions. M9 silently overflows when creating `1..=INT_MAX` ranges. M2 means integer overflow in AOT silently wraps instead of panicking. Any of these could cause mysterious failures under different optimization levels, LLVM versions, or target architectures.

**Depends on:** Nothing, but coordinate with Section 03 (exception handling) since H2 and H1/M10/M11 share nounwind analysis code.

---

## 02.1 Fix M14 — None Variant Uninitialized Payload

**Journey:** J12 | **Severity:** MEDIUM (but LLVM UB)
**File(s):** `compiler/ori_llvm/src/codegen/` (variant construction)

When constructing None (or any unit variant of a sum type with payload variants), the codegen stores only the tag but then loads ALL fields from the alloca — including the payload which was never written.

```llvm
; None construction:
%variant = alloca { i64, i64 }, align 8
store i64 1, ptr %variant.tag, align 4     ; tag = 1 (None) — stored
; payload at offset 1 is NEVER stored
%variant.f1 = load i64, ptr %variant.f1.ptr, align 4  ; loads uninitialized → poison!
```

**Fix options:**
- **(a) Zero-initialize the alloca** (recommended): `%variant = alloca { i64, i64 }, align 8` followed by `store { i64, i64 } zeroinitializer, ptr %variant`. Simple, correct, tiny cost.
- **(b) Skip loading uninitialized fields**: Only load fields that were stored. More complex codegen logic but avoids the write.
- **(c) Use `insertvalue` instead of alloca**: Build the SSA value directly without memory. Also fixes M7. Best long-term but larger change.

**Recommended:** Option (a) as immediate fix, option (c) as part of Section 05 (M7). Both must use `zeroinitializer` for unset payload fields — this is the consistent invariant across the plan (no `undef` for unused variant payloads).

- [x] Write test: construct unit variant (Empty), verified `zeroinitializer` in IR dump
- [x] Zero-initialize allocas for unit variants of payload sum types (`construction.rs`)
- [x] Verify with Valgrind on Journey 12 code — 0 errors, 0 leaks

---

## 02.2 Fix H2 — Audit nounwind for Runtime Calls

**Journey:** J10 | **Severity:** HIGH
**File(s):** `compiler/ori_llvm/src/codegen/` (nounwind analysis)

`_ori_check_iteration()` is marked `nounwind` but calls `ori_iter_from_list` and `ori_iter_next` which have NO function attributes in their declarations. If either can panic (OOM, bounds violation), the nounwind guarantee is violated.

```llvm
; Function marked nounwind...
define fastcc i64 @_ori_check_iteration() #0 {   ; #0 = nounwind
  ; ...but calls functions without nounwind:
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data5, i64 %list.len, i64 8)
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 8)
```

**Required:**
1. Audit ALL runtime function declarations — which can panic?
2. Mark non-panicking runtime functions with `nounwind` attribute
3. Ensure nounwind analysis considers transitive calls to runtime functions
4. Functions calling potentially-panicking runtime functions must NOT be `nounwind`

- [x] Catalog all `declare` runtime functions in generated IR across all 12 journeys
- [x] For each: determine if it can panic (check ori_rt source)
- [x] Mark nounwind-safe runtime functions with the attribute
- [x] Audit the nounwind propagation algorithm — does it check runtime function attributes?
- [x] Fix: functions calling non-nounwind runtime functions must not be marked nounwind
- [x] Verify: `_ori_check_iteration` should NOT be nounwind (calls allocating runtime functions)

---

## 02.3 Fix M9 — Range Overflow for ..=INT_MAX

**Journey:** J7 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (range iteration codegen)

Inclusive range `1..=n` computes `end + step` to convert to exclusive bound. For `n = INT_MAX`, this overflows.

```llvm
%add = add i64 %proj.1, %proj.3   ; end + step = INT_MAX + 1 → overflow!
```

**Fix options:**
- **(a) Saturating add**: `%add = call i64 @llvm.sadd.sat.i64(i64 %end, i64 %step)` — saturates at INT_MAX instead of wrapping.
- **(b) Sign-aware loop condition** (recommended): Use a comparison that matches the step direction, avoiding the +1 conversion entirely. More natural for inclusive ranges.
- **(c) Overflow check**: Emit `llvm.sadd.with.overflow` and panic on overflow.

**Recommended:** Option (b) — use a sign-aware loop condition instead of converting to exclusive bounds.

**Critical: step sign is NOT always compile-time known.** Ori allows variable step expressions (e.g., `for x in 0..12 by step yield x` where `step` is a runtime variable — see `tests/spec/expressions/ranges.ori:304`). The codegen must handle all three cases at runtime:

- **step > 0** (ascending): continue while `current <= end` (`icmp sle`)
- **step < 0** (descending): continue while `current >= end` (`icmp sge`)
- **step == 0**: panic (infinite loop prevention — `ranges.ori:153` tests syntax only, NOT runtime panic; `assert_panics` test needed)

**Implementation: runtime branch on step sign.**
```llvm
; Runtime step-sign dispatch for inclusive ranges:
%step_positive = icmp sgt i64 %step, 0
%step_negative = icmp slt i64 %step, 0
%step_zero = icmp eq i64 %step, 0
br i1 %step_zero, label %panic_zero_step, label %check_direction

check_direction:
  %cmp_le = icmp sle i64 %current, %end    ; ascending check
  %cmp_ge = icmp sge i64 %current, %end    ; descending check
  %continue = select i1 %step_positive, i1 %cmp_le, i1 %cmp_ge
  br i1 %continue, label %body, label %exit
```

When the step IS a compile-time constant (the common case), LLVM's constant folding will eliminate the dead branch, producing the same code as a hardcoded `sle`/`sge`. No performance cost for the common case.

- [x] Write test: `for x in 0..=0 do ...` (edge case — single element range)
- [x] Write test: ascending inclusive range near INT_MAX
- [x] Write test: `for x in 10..=0 by -1 do ...` (descending inclusive range)
- [x] Write test: `for x in 5..=1 by -1 do ...` (descending inclusive with step -1)
- [x] Write test: variable step with inclusive range (runtime-dynamic sign)
- [x] Write test: zero step panics — `assert_panics(f: () -> for x in 0..=10 by 0 yield x)` (existing `ranges.ori:153` only tests syntax, not runtime behavior)
- [x] Implement fix: emit runtime step-sign branch (step > 0 → sle, step < 0 → sge, step == 0 → panic)
- [x] Verify: LLVM constant-folds the branch away for literal steps
- [x] Verify: Journey 7 still returns 30
- [x] Verify: descending inclusive ranges produce correct results matching eval
- [x] Verify: variable-step inclusive ranges match eval behavior

---

## 02.4 Add H3 — noalias on Proven Non-Aliasing Parameters

**Severity:** HIGH (optimization enabler, not correctness)
**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs`

**WARNING: Blanket `noalias` on all pointer params is UNSOUND.** While Ori has value semantics at the language level, pointer parameters in LLVM IR can alias the same underlying RC-managed buffer. Consider:

```ori
@f (a: [int], b: [int]) -> int = a[0] + b[0]

// Called as:
let xs = [1, 2, 3];
f(a: xs, b: xs)    // a and b share the same RC buffer!
```

At the Ori level, `a` and `b` are independent values. But at the LLVM IR level, before COW triggers a copy, both params point to the same heap allocation. LLVM `noalias` means the pointers do NOT alias — if LLVM reorders a store through `a` before a load through `b` (which `noalias` permits), the result is wrong.

**The existing codebase already handles this correctly.** `emitter_utils.rs:33-50` applies `noalias` only in `CowMode::StaticUnique` contexts — when uniqueness analysis proves refcount == 1. This is the right pattern.

**What to do — extend the existing pattern, not replace it:**
1. **sret and `ori_rc_alloc` return**: Already `noalias` — correct (fresh allocations can't alias).
2. **COW `StaticUnique`**: Already `noalias` — correct (proven unique, refcount == 1).
3. **Function parameters from fresh allocations**: If a parameter is provably freshly constructed at the call site (not passed through from another parameter), it cannot alias other params. This requires interprocedural analysis or annotation propagation.
4. **All other pointer params**: NOT safe for `noalias` — could share RC buffer.

**Conservative approach (recommended):** Extend `noalias` only to parameters that the ARC pipeline's ownership analysis marks as `Owned` AND that are provably not aliased with other params. This requires integrating with borrow inference results, not blanket annotation.

**Acceptance rule (define before implementing):** Document the exact set of ownership/provenance states that permit `noalias`. Candidate rule: a pointer param gets `noalias` if and only if **(a)** it is an sret or fresh `ori_rc_alloc` return (already done), OR **(b)** it is `CowMode::StaticUnique` at a COW call site (already done), OR **(c)** callee-side analysis proves no other live param in the same function shares the same allocation origin. Any state not explicitly listed here must NOT get `noalias`. This rule must be written into a code comment at the annotation site to prevent interpretation drift during future changes.

**Deferred analysis:** A full escape analysis could prove more params non-aliasing, but this is a significant undertaking. Start with the conservative cases and measure the optimization impact.

- [x] Audit all current `noalias` usage — verify soundness of existing annotations
- [x] Identify parameters provably non-aliasing (fresh allocations at call site, `Owned` without shared source)
- [x] Add `noalias` only to proven non-aliasing pointer params (NOT blanket on all pointer params)
- [x] Write test: `f(a: xs, b: xs)` — both params alias same buffer, verify correct behavior with and without noalias
- [x] Verify with `opt -passes=print-alias-summary` on proven-noalias cases
- [x] Verify: no test regressions

---

## 02.5 Fix M2 — Checked Arithmetic (Overflow Panics)

**Journey:** J1 (confirmed J1-J12) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (arithmetic instruction emission)

All integer arithmetic in AOT uses wrapping semantics (`add`, `sub`, `mul` without `nsw`). Ori's spec says integer overflow should panic. The plan's goal is zero LLVM UB — therefore **`nsw` flags are not an option** (`nsw` makes overflow UB, directly contradicting the zero-UB goal).

**Decision: Checked arithmetic (non-negotiable).**

Emit `llvm.sadd.with.overflow` / `ssub.with.overflow` / `smul.with.overflow` intrinsics and branch to a panic function on overflow. This matches Rust's debug mode behavior and Ori's spec. It is the only approach compatible with this plan's "zero UB" mission.

**Implementation sketch:**
```llvm
; Before (wrapping — silent UB on overflow):
%r = add i64 %a, %b

; After (checked — panics on overflow):
%result = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
%r = extractvalue { i64, i1 } %result, 0
%overflow = extractvalue { i64, i1 } %result, 1
br i1 %overflow, label %panic, label %continue
```

**Note:** A future `--release` flag could switch to `nsw` for performance-critical builds, but the default must match the spec (panic on overflow).

- [x] Emit `llvm.sadd.with.overflow` / `ssub.with.overflow` / `smul.with.overflow` for all signed integer arithmetic
- [x] Generate overflow panic via `ori_panic_cstr` with per-operation messages (inline in each overflow branch)
- [x] Write test: arithmetic that would overflow, verify AOT panics (matching eval behavior)
- [x] Verify: non-overflowing arithmetic generates identical results to eval

---

## 02.6 Completion Checklist

- [x] No `load` of uninitialized memory in any generated IR (M14 fixed)
- [x] All runtime function declarations have correct `nounwind` attributes
- [x] No function marked `nounwind` transitively calls a panicking runtime function
- [x] Proven non-aliasing pointer parameters have `noalias` attribute; no blanket annotation (H3)
- [x] `1..=0` range works correctly (empty inclusive range edge case)
- [x] All signed integer arithmetic uses checked overflow intrinsics (panic on overflow)
- [x] `./scripts/valgrind-aot.sh` — 0 errors
- [x] `./test-all.sh` green

**Exit Criteria:** `opt -passes=verify` on generated IR for all 12 journeys reports 0 errors. Valgrind reports 0 invalid reads/writes. nounwind analysis is conservative-correct. Proven non-aliasing parameters have `noalias`; no blanket annotation on shared-buffer-capable params.
