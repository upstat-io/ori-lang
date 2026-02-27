---
section: "04"
title: Codegen Audit & Analysis
status: not-started
goal: "Static analysis tools that detect RC imbalances, COW correctness issues, and ABI violations in LLVM IR"
inspired_by:
  - "Roc ROC_CHECK_MONO_IR (type-checks IR after specialization)"
  - "Swift SIL ARC optimizer (GlobalARCSequenceDataflow.cpp — dataflow for RC patterns)"
  - "LLVM opt -verify-each (per-pass verification)"
depends_on: ["01", "02", "03"]
sections:
  - id: "04.1"
    title: "Static RC Balance Analysis"
    status: not-started
  - id: "04.2"
    title: "COW Operation Correctness"
    status: not-started
  - id: "04.3"
    title: "ABI Conformance Checking"
    status: not-started
  - id: "04.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Codegen Audit & Analysis

**Status:** Not Started
**Goal:** Static analysis tools that examine LLVM IR for correctness issues — RC balance verification, COW operation sequencing, and ABI conformance — catching bugs before runtime.

**Context:** Many codegen bugs are structurally visible in the IR. A function that calls `ori_rc_alloc` but never calls `ori_rc_dec` is definitely leaking. A function that calls `ori_rc_dec` on a pointer after passing it to a COW function (which might have freed it) is a potential double-free. These patterns can be detected statically by analyzing the IR, without running the program.

**Reference implementations:**
- **Roc** `ROC_CHECK_MONO_IR`: Type-checks the mono IR after specialization — catches internal consistency violations
- **Swift** `lib/SILOptimizer/ARC/GlobalARCSequenceDataflow.cpp`: Dataflow analysis that tracks retain/release sequences to prove correctness
- **LLVM** `opt -verify-each`: Runs module verification after every optimization pass

**Depends on:** Section 01 (uses `ir-dump.sh` for IR capture), Section 02 (cross-references with runtime trace for validation), Section 03 (uses phase dump annotations for richer analysis).

---

## 04.1 Static RC Balance Analysis

**File(s):** `diagnostics/codegen-audit.sh` (new script)

Analyze LLVM IR to verify that every RC allocation has a matching deallocation path. This is a more sophisticated version of `rc-stats.sh` (Section 01.4) that tracks *which* pointers are alloc'd and whether they're properly dec'd.

- [ ] Create `diagnostics/codegen-audit.sh` with file argument
  ```bash
  # Usage: diagnostics/codegen-audit.sh <file.ori> [--strict] [--function <name>]
  # Output: Per-function RC balance analysis with pointer tracking
  ```
- [ ] Parse LLVM IR to extract:
  - `ori_rc_alloc` calls → mark pointer as "live"
  - `ori_rc_inc` calls → note increment
  - `ori_rc_dec` calls → note decrement, check for "already freed"
  - `ori_rc_free` calls → mark pointer as "dead"
- [ ] Track pointer aliasing through `extractvalue`, `insertvalue`, `load`, `store`
- [ ] Detect: alloc without dec (leak), dec on unknown pointer (potential UAF), double dec (double-free)
- [ ] Output:
  ```
  === RC Audit: @main ===
  %list.data (alloc at line 4)
    → passed to ori_list_push_cow (COW: may free internally)
    → ori_rc_dec at line 12 ⚠ POTENTIAL DOUBLE-FREE
       (pointer was passed to COW function which may have freed it)

  %push.val.f2 (extracted from push result at line 8)
    → passed to ori_list_reverse_cow (COW: may free internally)
    → ori_rc_dec at line 18 ⚠ POTENTIAL DOUBLE-FREE

  %reverse.val.f2 (extracted from reverse result at line 14)
    → ori_rc_dec at line 22 ✓ OK

  Summary: 2 potential double-frees, 0 leaks
  ===
  ```
- [ ] `--strict` mode: treat COW functions as always-freeing (pessimistic, catches more bugs)
- [ ] Test: run on the `push(3).reverse()` program — should flag the potential double-free

---

## 04.2 COW Operation Correctness

**File(s):** `diagnostics/codegen-audit.sh` (extend from 04.1)

Verify that COW operations in the IR follow correct sequencing patterns.

- [ ] **Rule 1: COW input must not be used after the call**
  - After `ori_list_push_cow(data, ...)`, the original `data` pointer should not be loaded from or passed to another function (except `ori_rc_dec` for cleanup)
  - Detect: `%data` used after `ori_list_push_cow(%data, ...)` → warning
- [ ] **Rule 2: COW output must extract from the output alloca**
  - The result of a COW function is in the `out_ptr` alloca, not the return value
  - Detect: using COW function return value (it's `void`) → error
- [ ] **Rule 3: Original list RC dec must happen after COW call, not before**
  - If the original list is RC dec'd before the COW call, the COW function receives freed memory
  - Detect: `ori_rc_dec(%original)` before `ori_list_push_cow(%original, ...)` → error
- [ ] **Rule 4: COW functions must receive valid (len, cap, data) triples**
  - len ≤ cap (invariant)
  - data is non-null when len > 0
  - These can only be verified at runtime, but the IR structure can be checked
- [ ] Output violations with IR line references and suggested fixes

---

## 04.3 ABI Conformance Checking

**File(s):** `diagnostics/codegen-audit.sh` (extend from 04.1)

Verify that function calls in the IR conform to the Ori ABI conventions.

- [ ] **Struct return by Sret**: Functions returning structs > 16 bytes should use `sret` parameter, not direct return
  - Detect: `define { i64, i64, ptr } @_ori_*()` → warning (should use sret for >16 byte structs)
- [ ] **Large struct loads**: No `load { i64, i64, ptr }, ptr` for structs > 16 bytes in JIT mode (FastISel bug)
  - Detect: aggregate loads > 16 bytes → warning with reference to the FastISel workaround
- [ ] **Runtime function signatures**: Verify that calls to `ori_*` runtime functions match their declared signatures
  - Detect: `call void @ori_list_push_cow(ptr, i64, i64, ptr, i64, i64, ptr)` has correct param count and types
- [ ] **Calling convention**: Verify `nounwind` functions are never called with `invoke` (should be `call`)
- [ ] Output conformance report:
  ```
  ABI Audit: test.ori
  ✓ No large aggregate loads
  ✓ Runtime function signatures match declarations
  ⚠ ori_list_push_cow called with 6 args (expected 7) at line 15
  ```

---

## 04.4 Completion Checklist

- [ ] `codegen-audit.sh` performs RC balance analysis on any `.ori` file
- [ ] RC audit detects known-bad patterns: leak, double-free, use-after-COW
- [ ] COW correctness rules detect sequencing violations
- [ ] ABI conformance detects parameter count mismatches and large aggregate loads
- [ ] All checks produce actionable output with IR line references
- [ ] `--strict` mode enables pessimistic analysis for maximum coverage
- [ ] Tested on 5+ programs: clean (no warnings), leaky, double-free, COW bug, ABI mismatch
- [ ] `./test-all.sh` green

**Exit Criteria:** Running `diagnostics/codegen-audit.sh` on the `push(3).reverse()` program produces a report that flags the exact line in the IR where the potential double-free occurs, with a clear explanation of why the pattern is suspicious.
