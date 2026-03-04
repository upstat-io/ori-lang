---
reroute: true
name: "Codegen Purity"
full_name: "Codegen Purity: Hand-Written Assembly Quality at -O0"
status: active
---

# Codegen Purity Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Source:** All findings from code journeys 1–12 (`plans/code-journeys/`)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Block Merging & CFG Simplification
**File:** `section-01-block-merging.md` | **Status:** Not Started

```
basic block, unconditional branch, br label, block merging, CFG simplification
select instruction, if/else diamond, phi node, single-predecessor phi
break bridge, dead phi, trivial block, sequential block, let-binding boundary
arc_emitter, terminators.rs, construction.rs, element_fn_gen.rs
```

---

### Section 02: Function Attributes
**File:** `section-02-function-attributes.md` | **Status:** Not Started

```
noreturn, nounwind, noundef, cold, function attribute, LLVM attribute
ori_panic_cstr, ori_str_from_raw, indirect closure call, main wrapper
derived methods, derive_codegen, nounwind analysis, fixed-point
fastcc, calling convention, exception table, personality function
runtime_functions.rs, nounwind.rs, function_compiler, runtime_decl
```

---

### Section 03: Arithmetic Correctness
**File:** `section-03-arithmetic-correctness.md` | **Status:** Not Started

```
unary negation, overflow check, INT_MIN, ssub.with.overflow
integer overflow, panic, arithmetic, negate, Neg trait
operators.rs, arithmetic.rs
```

---

### Section 04: ARC Closure Lifecycle
**File:** `section-04-arc-closure-lifecycle.md` | **Status:** Not Started

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
arc_emitter, construction.rs, element_fn_gen.rs
```

---

### Section 06: Dead Code Pruning
**File:** `section-06-dead-code-pruning.md` | **Status:** Not Started

```
dead code, dead load, struct field, list field, unused field
noreturn, unreachable, cleanup after panic, DCE
aggregate load, per-field GEP, surgical extraction
arc_emitter, construction.rs, aggregates.rs
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
for_range.rs, arc_emitter, control_flow
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

| ID | Title | File |
|----|-------|------|
| 01 | Block Merging & CFG Simplification | `section-01-block-merging.md` |
| 02 | Function Attributes | `section-02-function-attributes.md` |
| 03 | Arithmetic Correctness | `section-03-arithmetic-correctness.md` |
| 04 | ARC Closure Lifecycle | `section-04-arc-closure-lifecycle.md` |
| 05 | Sum Type Payload Extraction | `section-05-payload-extraction.md` |
| 06 | Dead Code Pruning | `section-06-dead-code-pruning.md` |
| 07 | Constant Deduplication | `section-07-constant-dedup.md` |
| 08 | Loop IR Quality | `section-08-loop-ir-quality.md` |
| 09 | Tail Call Optimization | `section-09-tail-call.md` |
| 10 | Verification | `section-10-verification.md` |
