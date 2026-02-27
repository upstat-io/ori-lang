---
section: "02"
title: "UB & Soundness"
status: not-started
goal: "Zero LLVM undefined behavior in generated IR; nounwind analysis provably sound"
inspired_by:
  - "Rust rustc_codegen_llvm — nsw/nuw flag propagation on arithmetic"
  - "Zig src/Sema.zig — overflow detection and safety checks"
depends_on: []
sections:
  - id: "02.1"
    title: "Fix M14 — None variant uninitialized payload"
    status: not-started
  - id: "02.2"
    title: "Fix H2 — Audit nounwind for runtime calls"
    status: not-started
  - id: "02.3"
    title: "Fix M9 — Range overflow for ..=INT_MAX"
    status: not-started
  - id: "02.4"
    title: "Design M2 — nsw flags / checked arithmetic"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: UB & Soundness

**Status:** Not Started
**Goal:** All LLVM undefined behavior eliminated from generated IR. The `nounwind` analysis is provably sound — no function marked `nounwind` can transitively call a function that may unwind.

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

**Recommended:** Option (a) as immediate fix, option (c) as part of Section 05 (M7).

- [ ] Write test: construct None, verify no poison in IR (check with `opt -passes=verify`)
- [ ] Zero-initialize allocas for unit variants of payload sum types
- [ ] Verify with `./scripts/valgrind-aot.sh` on Journey 12 code

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

- [ ] Catalog all `declare` runtime functions in generated IR across all 12 journeys
- [ ] For each: determine if it can panic (check ori_rt source)
- [ ] Mark nounwind-safe runtime functions with the attribute
- [ ] Audit the nounwind propagation algorithm — does it check runtime function attributes?
- [ ] Fix: functions calling non-nounwind runtime functions must not be marked nounwind
- [ ] Verify: `_ori_check_iteration` should NOT be nounwind (calls allocating runtime functions)

---

## 02.3 Fix M9 — Range Overflow for ..=INT_MAX

**Journey:** J7 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (range iteration codegen)

Inclusive range `1..=n` computes `end + step` to convert to exclusive bound. For `n = INT_MAX`, this overflows.

```llvm
%add = add i64 %proj.1, %proj.3   ; end + step = INT_MAX + 1 → overflow!
```

**Fix options:**
- **(a) Saturating add** (recommended): `%add = call i64 @llvm.sadd.sat.i64(i64 %end, i64 %step)` — saturates at INT_MAX instead of wrapping.
- **(b) Different loop condition**: Use `<=` comparison instead of `<` to avoid the +1 conversion entirely. More natural for inclusive ranges.
- **(c) Overflow check**: Emit `llvm.sadd.with.overflow` and panic on overflow.

**Recommended:** Option (b) — change the loop condition to `icmp sle` to avoid the conversion altogether.

- [ ] Write test: `for x in 0..=0 do ...` (edge case — single element range)
- [ ] Write test: range with large values near INT_MAX (may need to test logic, not actual INT_MAX)
- [ ] Implement fix: use `icmp sle` for inclusive ranges instead of converting to exclusive
- [ ] Verify: Journey 7 still returns 30

---

## 02.4 Design M2 — nsw Flags / Checked Arithmetic

**Journey:** J1 (confirmed J1-J12) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (arithmetic instruction emission)

All integer arithmetic in AOT uses wrapping semantics (`add`, `sub`, `mul` without `nsw`). Ori's spec says integer overflow should panic. Two approaches:

**Option (a) — `nsw` flags** (simpler):
Add `nsw` to all signed integer arithmetic. LLVM will optimize based on the no-overflow assumption. If overflow occurs at runtime, it's UB — but Ori programs shouldn't overflow (eval would panic). This matches Rust's release mode behavior.

**Option (b) — Checked arithmetic** (correct):
Emit `llvm.sadd.with.overflow` / `ssub.with.overflow` / `smul.with.overflow` intrinsics and branch to a panic function on overflow. This matches Rust's debug mode behavior and Ori's spec.

**Recommended:** Option (b) is correct but invasive. Option (a) is a pragmatic interim. Design decision needed — defer to implementation time.

- [ ] Decide: `nsw` flags (pragmatic, matches Rust release) vs checked arithmetic (correct, matches spec)
- [ ] If `nsw`: add `nsw` flag to all `add`, `sub`, `mul` instructions for signed integers
- [ ] If checked: emit overflow intrinsics + panic branch for all arithmetic
- [ ] Write test: arithmetic that would overflow, verify behavior matches eval

---

## 02.5 Completion Checklist

- [ ] No `load` of uninitialized memory in any generated IR (M14 fixed)
- [ ] All runtime function declarations have correct `nounwind` attributes
- [ ] No function marked `nounwind` transitively calls a panicking runtime function
- [ ] `1..=0` range works correctly (empty inclusive range edge case)
- [ ] Integer arithmetic semantics decision documented and implemented
- [ ] `./scripts/valgrind-aot.sh` — 0 errors
- [ ] `./test-all.sh` green

**Exit Criteria:** `opt -passes=verify` on generated IR for all 12 journeys reports 0 errors. Valgrind reports 0 invalid reads/writes. nounwind analysis is conservative-correct.
