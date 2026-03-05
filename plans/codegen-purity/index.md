---
reroute: true
name: "Codegen Purity"
full_name: "Codegen Purity: Hand-Written Assembly Quality at -O0"
status: active
---

# Codegen Purity Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Source:** All findings from code journeys 1–12 (`plans/code-journeys/`)
> **Last reviewed:** 2026-03-05

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Block Merging & CFG Simplification
**File:** `section-01-block-merging.md` | **Status:** In Progress (3/4 subsections complete)

```
basic block, unconditional branch, br label, block merging, CFG simplification
select instruction, if/else diamond, phi node, single-predecessor phi
break bridge, dead phi, trivial block, sequential block, let-binding boundary
block_merge/mod.rs, arc_emitter, emit_function.rs, terminators.rs
```

---

### Section 02: Function Attributes
**File:** `section-02-function-attributes.md` | **Status:** In Progress

```
noreturn, nounwind, noundef, cold, function attribute, LLVM attribute
ori_panic_cstr, ori_str_from_raw, indirect closure call, main wrapper
derived methods, derive_codegen, nounwind analysis, fixed-point
fastcc, calling convention, exception table, personality function
Attr enum, Attr::Noreturn, Attr::Nounwind, add_noreturn_attribute
runtime_functions.rs, nounwind.rs, function_compiler, runtime_decl, attributes.rs
apply_attr, is_rt_fn_nounwind, is_rt_fn_noreturn, entry_point.rs
```

---

### Section 03: Arithmetic Correctness
**File:** `section-03-arithmetic-correctness.md` | **Status:** Complete

```
unary negation, overflow check, INT_MIN, ssub.with.overflow
integer overflow, panic, arithmetic, negate, Neg trait
operators.rs, arithmetic.rs
```

---

### Section 04: ARC Closure Lifecycle
**File:** `section-04-arc-closure-lifecycle.md` | **Status:** Complete

```
closure, environment, rc_dec, rc_alloc, memory leak, ARC pipeline
closure capture, live range, refcount, drop, make_adder
ori_arc, drop_gen.rs, arc_emitter
```

---

### Section 05: Sum Type Payload Extraction
**File:** `section-05-payload-extraction.md` | **Status:** Not Started

```
sum type, variant, payload, destructuring, match expression
alloca, store, GEP, load, extractvalue, union payload
[N x i64], record variant, enum, Option, decision tree
arc_emitter, instr_dispatch.rs, construction.rs, element_fn_gen.rs
```

---

### Section 06: Dead Code Pruning
**File:** `section-06-dead-code-pruning.md` | **Status:** Not Started

```
dead code, dead load, struct field, list field, unused field
noreturn, unreachable, cleanup after panic, DCE
aggregate load, per-field GEP, surgical extraction
arc_emitter, instr_dispatch.rs, emit_function.rs, apply.rs, aggregates.rs
```

---

### Section 07: Constant Deduplication
**File:** `section-07-constant-dedup.md` | **Status:** Not Started

```
string constant, global, overflow message, unnamed_addr, deduplication
integer overflow on addition, duplicate global, constant pool
ir_builder, constants.rs
```

---

### Section 08: Loop IR Quality
**File:** `section-08-loop-ir-quality.md` | **Status:** Not Started

```
loop, CSE, common subexpression, duplicate computation
loop-invariant, phi node, LICM, hoisting
range iteration, bounds check, specialization, 1..=n
for_range.rs, loops.rs, emit_function.rs, operators.rs, control_flow
```

---

### Section 09: Tail Call Optimization
**File:** `section-09-tail-call.md` | **Status:** Not Started

```
tail call, TCO, tail recursion, musttail, stack overflow
recursive, gcd, fastcc, loop conversion
function_compiler, terminators.rs
```

---

### Section 10: Verification
**File:** `section-10-verification.md` | **Status:** Not Started

```
code journey, verification, purity, -O0, assembly quality
dual execution, eval vs AOT, regression, test matrix
all 12 journeys, hand-written quality, C-level assembly
ir_quality.rs, permanent regression tests, un-ignore
preflight, environment lock, verification-meta, finding-closure matrix, unresolved-id ledger
build/codegen-purity/current, artifact capture, opt-21 verify
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Block Merging & CFG Simplification | `section-01-block-merging.md` | In Progress |
| 02 | Function Attributes | `section-02-function-attributes.md` | In Progress |
| 03 | Arithmetic Correctness | `section-03-arithmetic-correctness.md` | Complete |
| 04 | ARC Closure Lifecycle | `section-04-arc-closure-lifecycle.md` | Complete |
| 05 | Sum Type Payload Extraction | `section-05-payload-extraction.md` | Not Started |
| 06 | Dead Code Pruning | `section-06-dead-code-pruning.md` | Not Started |
| 07 | Constant Deduplication | `section-07-constant-dedup.md` | Not Started |
| 08 | Loop IR Quality | `section-08-loop-ir-quality.md` | Not Started |
| 09 | Tail Call Optimization | `section-09-tail-call.md` | Not Started |
| 10 | Verification | `section-10-verification.md` | Not Started |
