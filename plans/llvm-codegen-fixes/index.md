# LLVM Codegen Fixes Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Source:** `plans/code-journeys/` — 12 code journey results

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Critical Correctness
**File:** `section-01-critical-correctness.md` | **Status:** Not Started

```
C1, closure, lambda, non-capturing, capturing, argument mismatch, crash
C2, list indexing, xs[0], __index, mono instance, unresolved function
C3, derive Eq, payload sum type, $eq, silent wrong, derive_codegen
C4, Option, match, switch, tag inversion, Some, None, monomorphized, built-in generic
phi_types_blocks, param_index, param_count, inkwell
```

---

### Section 02: UB & Soundness
**File:** `section-02-ub-soundness.md` | **Status:** Not Started

```
M14, None, uninitialized, payload, alloca, poison, LLVM UB
H2, nounwind, unsound, runtime function, ori_iter_from_list, ori_iter_next
M2, nsw, nuw, overflow, wrapping, checked arithmetic, sadd.with.overflow
M9, inclusive range, ..=, INT_MAX, end + step, overflow
```

---

### Section 03: Exception Handling
**File:** `section-03-exception-handling.md` | **Status:** Not Started

```
H1, invoke, landing pad, empty, cleanup, resume, nounwind propagation
M10, _ori_main, nounwind, inconsistent, attribute, monomorphized
M11, orphaned, landing pad, no predecessors, dead, ARC cleanup
rust_eh_personality, personality, unwind
```

---

### Section 04: Alignment
**File:** `section-04-alignment.md` | **Status:** Not Started

```
M5, align 4, align 8, i64, struct field, variant field, list element
load, store, getelementptr, natural alignment
```

---

### Section 05: Variant Codegen
**File:** `section-05-variant-codegen.md` | **Status:** Not Started

```
M7, variant construction, alloca, store, load, roundtrip, insertvalue
M8, identical match arms, deduplicate, switch, payload extraction
sum type, enum, tagged union, Color, Shape, Status
```

---

### Section 06: Struct & Param Codegen
**File:** `section-06-struct-param-codegen.md` | **Status:** Not Started

```
M6, full struct load, partial field access, load_indirect_param, extractvalue
M13, Option-like, iterator, tuple, unnecessary construction
getelementptr, insertvalue, extractvalue, zeroinitializer
```

---

### Section 07: ARC Pipeline
**File:** `section-07-arc-pipeline.md` | **Status:** Not Started

```
M12, duplicate drop, identical, _ori_drop, ori_rc_free, layout
deduplication, drop function, ARC, reference counting
```

---

### Section 08: Loop & Range
**File:** `section-08-loop-range.md` | **Status:** Not Started

```
M4, tail call, tail recursion, musttail, gcd, loop transformation
L5, range, struct, materialized, destructured, insertvalue, extractvalue
L6, duplicate computation, CSE, i + 1
L7, dead phi, loop exit, unused, phi node
```

---

### Section 09: IR Cleanliness
**File:** `section-09-ir-cleanliness.md` | **Status:** Not Started

```
M3, dead branch, br label, unnecessary, after call, every journey
L3, select, branch, phi, trivial if/else, SimplifyCFG
L4, single predecessor, phi, one incoming, redundant
```

---

### Section 10: Prelude & Startup
**File:** `section-10-prelude-startup.md` | **Status:** Not Started

```
M1, prelude, overhead, 10331 bytes, cold start, startup
L1, canonicalizer, expansion, node, growth
L2, decision tree, prelude, lazy, canonicalization
```

---

### Section 11: Verification
**File:** `section-11-verification.md` | **Status:** Not Started

```
code journey, dual-exec, verify, valgrind, test matrix
eval, AOT, behavioral equivalence, regression
12 journeys, 28 findings, correctness
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Critical Correctness | `section-01-critical-correctness.md` |
| 02 | UB & Soundness | `section-02-ub-soundness.md` |
| 03 | Exception Handling | `section-03-exception-handling.md` |
| 04 | Alignment | `section-04-alignment.md` |
| 05 | Variant Codegen | `section-05-variant-codegen.md` |
| 06 | Struct & Param Codegen | `section-06-struct-param-codegen.md` |
| 07 | ARC Pipeline | `section-07-arc-pipeline.md` |
| 08 | Loop & Range | `section-08-loop-range.md` |
| 09 | IR Cleanliness | `section-09-ir-cleanliness.md` |
| 10 | Prelude & Startup | `section-10-prelude-startup.md` |
| 11 | Verification | `section-11-verification.md` |
