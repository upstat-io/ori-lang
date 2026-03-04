---
section: "01"
title: "Critical Correctness"
status: complete
goal: "All 12 code journeys produce correct results in both eval and AOT paths"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/operand.rs — OperandValue variant handling"
  - "Zig src/Sema.zig — monomorphization dispatch consistency"
depends_on: []
sections:
  - id: "01.1"
    title: "Fix C4 — Option match tag inversion"
    status: complete
  - id: "01.2"
    title: "Fix C3 — Payload sum type $eq codegen"
    status: complete
  - id: "01.3"
    title: "Fix C1 — Mixed closure argument mismatch"
    status: complete
  - id: "01.4"
    title: "Fix C2 — List indexing __index registration"
    status: complete
  - id: "01.5"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Critical Correctness

**Context:** Code journeys 5, 10, 11, and 12 discovered that AOT produces wrong results or crashes for closures, list indexing, derived Eq on payload sum types, and Option match. Two of these (C3, C4) are *silent* miscompilations — the program runs but returns wrong values. This is the most dangerous class of defect because users cannot detect the error.

**Depends on:** Nothing — this is the highest priority.

---

## 01.1 Fix C4 — Option Match Tag Inversion

**Journey:** J12 | **Severity:** CRITICAL
**File(s):** `compiler/ori_llvm/src/codegen/` (match codegen path)

The built-in `Option<T>` type uses tag 0 = Some (first variant), tag 1 = None (second variant). Construction codegen is correct. But the `match` switch codegen inverts the labels:

```llvm
; CONSTRUCTION (correct):
store i64 0, ptr %variant.tag   ; Some = tag 0
store i64 1, ptr %variant.tag   ; None = tag 1

; MATCH (inverted):
switch i64 %proj.0, label %bb4 [
  i64 1, label %bb2    ; tag 1 → Some arm (WRONG! tag 1 is None)
  i64 0, label %bb3    ; tag 0 → None arm (WRONG! tag 0 is Some)
]

; ? OPERATOR (correct — different code path):
%eq = icmp eq i64 %proj.0, 0    ; tag 0 = Some → continue
```

**Key diagnostic clue:** User-defined sum types are NOT affected. `type MyOption = MySome(v: int) | MyNone` matches correctly. The bug is specific to monomorphized built-in generic types.

**Root cause hypothesis:** The match codegen for monomorphized built-in generic types uses a different variant ordering than construction. The `?` operator avoids this because it uses a direct `icmp eq` rather than a `switch`.

- [x] Reproduce with Journey 12's code — AOT returns 144 instead of 33
- [x] Write targeted test: `Option<int>` construction + match, compare eval vs AOT
- [x] Write test: user-defined `type MyOpt<T> = MySome(v: T) | MyNone` — should pass (confirms isolation)
- [x] Trace the match codegen path for `Option<int>` — find where variant tag → switch label mapping diverges
- [x] Find the divergence between user-defined sum type match (correct) and built-in generic match (inverted)
- [x] Fix the root cause — ensure match switch labels match construction tags for all sum types
- [x] Verify `?` operator still works (it uses a different code path)
- [x] Journey 12 AOT returns 33

---

## 01.2 Fix C3 — Payload Sum Type $eq Codegen

**Journey:** J11 | **Severity:** CRITICAL
**File(s):** `compiler/ori_llvm/src/codegen/derive_codegen.rs`

`#[derive(Eq)]` generates `$eq` functions for structs (field-by-field comparison) and unit-only sum types (tag comparison), but NOT for payload sum types. When `Shape$eq` is needed, the codegen falls through to raw `icmp` on an aggregate type, which can't compare structs — the error handler silently substitutes `false`.

```llvm
; Point$eq — GENERATED (correct):
define fastcc i1 @"_ori_Point$eq"(%ori.Point %0, %ori.Point %1) { ... }

; Color$eq — GENERATED (correct):
define fastcc i1 @"_ori_Color$eq"(%ori.Color %0, %ori.Color %1) { ... }

; Shape$eq — NOT GENERATED! Codegen falls back to:
br i1 false, label %bb1, label %bb2    ; hardcoded false!
```

**Required codegen for Shape$eq:**
1. Compare tags first (`extractvalue` tag, `icmp eq`)
2. If tags differ → return false
3. If tags match → switch on tag value:
   - Circle: compare `radius` field
   - Rect: compare `w` and `h` fields
4. Return result

- [x] Reproduce with Journey 11's code — AOT returns 18 instead of 33
- [x] Write test: `#[derive(Eq)]` on payload sum type, eval vs AOT
- [x] Write test: mixed variant comparison (Circle == Rect → false, Circle(10) == Circle(10) → true)
- [x] Write test: single-payload sum type (e.g., `type Wrapper = Val(x: int)`)
- [x] Implement `$eq` generation for payload sum types in `derive_codegen/bodies.rs`:
  - Tag comparison first (early exit if different)
  - Switch on tag for payload comparison per variant
  - Field-by-field comparison within each variant arm
  - Also implemented Comparable and Hashable payload enum support
- [x] Journey 11 AOT returns 33

---

## 01.3 Fix C1 — Mixed Closure Argument Mismatch

**Journey:** J5 | **Severity:** CRITICAL
**File(s):** `compiler/ori_llvm/src/codegen/` (closure/lambda compilation)

When a module contains BOTH non-capturing lambdas (`x -> x * 2`) and capturing closures (`x -> x + n`), AOT crashes with LLVM verification error: "Incorrect number of arguments passed to called function."

```
ERROR: parameter index out of bounds — returning zero
  func=_ori___lambda_0 param_index=2 param_count=2

Cloning a Module seems to segfault when module is not valid.
Error: "Incorrect number of arguments passed to called function!
  %result = call i64 @_ori___lambda_1(i64 %cap.0)"
```

**Isolation test results:**
- Non-capturing lambda alone → works
- Capturing closure alone → works
- Both in same module → **crashes**

**Root cause:** Lambda names are per-function (`__lambda_0` in each `ArcLowerer`), but stored in a global `codegen_ctx.functions` HashMap. When two functions each contain a lambda, both produce `__lambda_0`, and the second insert overwrites the first. This corrupts the `non_capturing_lambdas` set, causing phantom_env_offset to apply to the wrong lambda — shifting parameter indices out of bounds.

**Fix:** `declare_and_process_lambda` now renames lambda names to globally unique values using the existing `lambda_counter`. Both `emit_arc_function` and `prepare_arc_function` remap `PartialApply { func }` references in the parent function to match.

- [x] Reproduce with Journey 5's code — AOT crashes
- [x] Write test: non-capturing + capturing in different functions (cross-function collision)
- [x] Write test: two non-capturing lambdas in different functions
- [x] Write test: multiple functions with lambdas (3 functions, mixed capture types)
- [x] Trace the closure compilation path — found name collision in `codegen_ctx.functions` HashMap
- [x] Fix: globally unique lambda names + PartialApply remapping in parent functions
- [x] Journey 5 AOT returns 27

---

## 01.4 Fix C2 — List Indexing __index Registration

**Journey:** J10 | **Severity:** CRITICAL
**File(s):** `compiler/ori_llvm/src/codegen/` (monomorphization registration)

`xs[0]` on `[int]` triggers "unresolved function `__index` in apply — missing mono instance?" followed by crash. The `__index` builtin function for lists is not registered as a mono instance in the LLVM codegen path.

```
WARNING: unresolved function `__index` in apply — missing mono instance?
ERROR: ArcIrEmitter: variable not yet defined
PANIC: ValueId 4294967295 out of bounds
```

**Note:** List `.length()` and `for..in` iteration work correctly — only element access via indexing is broken.

- [x] Reproduce with `@main () -> int = { let xs = [10, 20, 30]; xs[0] }` — no longer crashes (fixed in prior work)
- [x] Write test: list indexing simple case — existing test in `spec.rs:1884` and `collections_ext.rs:452`
- [x] Write test: list indexing with variable index — existing test in `collections_ext.rs:1145`
- [x] Verify that `xs[0]` returns correct value (10, not crash) — confirmed
- [x] Journey 10 AOT returns 33 — confirmed

---

## 01.5 Completion Checklist

- [x] Journey 5 (closures): AOT returns 27, matching eval
- [x] Journey 10: list indexing test case returns correct element value
- [x] Journey 11 (derived Eq): AOT returns 33, matching eval
- [x] Journey 12 (Option match): AOT returns 33, matching eval
- [x] All 12 journeys: eval vs AOT — 0 mismatches (verified manually)
- [x] No new regressions: `./test-all.sh` green
- [x] `./clippy-all.sh` green

**Exit Criteria:** All 4 critical bugs fixed. `./scripts/dual-exec-verify.sh` runs all 12 journey programs through both eval and AOT, producing 0 mismatches. No test regressions.
