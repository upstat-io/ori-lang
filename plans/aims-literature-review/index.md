# AIMS Literature Review Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Oxidizing OCaml (Modal Memory Management)
**File:** `section-01-oxidizing-ocaml.md` | **Status:** Complete

```
OxCaml, modal memory, mode axes, affinity, uniqueness, locality
stack allocation, in-place update, mode inference, mode constraints
uniqueness mode, locality mode, local allocations, escape analysis
closure capture, modalities, graded calculus, deep mode property
AIMS Locality dimension, HeapEscaping, BlockLocal, FunctionLocal
```

---

### Section 02: FP² (Fully in-Place Functional Programming)
**File:** `section-02-fp2.md` | **Status:** Complete

```
FIP, fully in-place, FP-squared, reuse credit, allocation balance
certification condition, frame-limited, garbage-free, no-alloc
FipContract, MemoryContract, EffectClass, may_alloc
in-place theorem, precondition, output proof obligation
AIMS reuse emission, ShapeClass, ReusableCtor, CollectionBuffer
owned environment, borrowed environment, linear, atoms, unboxed tuples
store semantics, Theorem 2, FIP ⊂ FBIP ⊂ λ^fip, TRMReC
constructor arity, DMATCH!, per-arm token balance, Schorr-Waite
```

---

### Section 03: FIPTree (Constructor Contexts)
**File:** `section-03-fiptree.md` | **Status:** Complete

```
FIPTree, constructor context, context hole, first-class context
top-down algorithm, in-place BST, zipper, functional essence
ContextHole, ShapeClass, opportunity creation, Stage 3
unfinished structure, partially-applied constructor, hole
AIMS normalize, TRMC candidate, shape dimension
```

---

### Section 04: TRMC (Tail Recursion Modulo Context)
**File:** `section-04-trmc.md` | **Status:** Complete

```
TRMC, tail recursion modulo context, equational approach
context laws, accumulator parameter, constructor accumulator
soundness criterion, profitability, law before optimization
AIMS normalize, opportunity creation, Stage 3, pre-analysis
Perceus heap semantics, continuation, modular rewriting
```

---

### Section 05: Perceus for OCaml (Evaluation Methodology)
**File:** `section-05-perceus-ocaml.md` | **Status:** Complete

```
Perceus OCaml, evaluation methodology, same-compiler comparison
backend swap, regression gate, metric isolation
old-vs-AIMS comparison, aims-shadow, ShadowComparisonReport
hard gate vs observed metric, Section 08, verification doctrine
compilation speed, RC count, allocation count, benchmark discipline
```

---

### Section 06: Linearity and Uniqueness
**File:** `section-06-linearity-uniqueness.md` | **Status:** Complete

```
linearity, uniqueness, entente cordiale, linear types, unique types
used-once, not-shared, consumption vs aliasing, future vs past
Consumption dimension, Uniqueness dimension, lattice distinction
transfer rule conflation, demand vs aliasing guarantee
AIMS AccessClass, Borrowed, Owned, MaybeShared, Unique
```

---

### Section 07: Quantitative Type Theory
**File:** `section-07-quantitative-type-theory.md` | **Status:** Complete

```
QTT, quantitative type theory, resource semiring, usage annotation
cardinality, demand, 0-1-omega, graded modal type theory
semiring composition, algebraic usage, resource algebra
AIMS Cardinality, Absent, Once, Many, seq_add, alt_join
compositional usage tracking, type-level resource
```

---

### Section 08: Lean 4 Borrow Inference
**File:** `section-08-lean4-borrow.md` | **Status:** Complete

```
Lean 4, borrow inference, IR/Borrow.lean, RC.lean
monotone, SCC iteration, contract extraction, conservative inference
interprocedural summary, owned vs borrowed, parameter classification
AIMS MemoryContract, ParamContract, interprocedural.rs
convergence, widening, fixpoint, call graph
```

---

### Section 09: GHC Demand Analysis
**File:** `section-09-ghc-demand.md` | **Status:** Complete

```
GHC, demand analysis, backward reasoning, DmdAnal.hs
seq_add, alt_join, branch join, control flow composition
once across control flow, alternative vs sequential
loop edges, exceptional edges, cardinality inference
AIMS backward dataflow, Cardinality, worklist, block analysis
```

---

### Section 10: Concurrent Immediate RC
**File:** `section-10-concurrent-rc.md` | **Status:** Complete

```
concurrent RC, immediate reference counting, CIRC
runtime abstraction, thread-local, atomic, biased
future concurrent, deferred decrement, epoch
AIMS boundary, runtime interface, ori_rt, Stage 5
what to keep out, complexity boundary
```

---

### Section 11: Cyclic RC for Immutable Data
**File:** `section-11-cyclic-rc.md` | **Status:** Complete

```
cyclic RC, deeply immutable, frozen cycles, SCC collection
cycle detection, backup tracing, lazy mark-scan
AIMS boundary, future frozen graph, Stage 5
runtime interface, ori_rt, extension point
```

---

### Section 12: Double-Ended Bit-Stealing
**File:** `section-12-bit-stealing.md` | **Status:** Complete

```
bit-stealing, double-ended, ADT representation, pointer tagging
low bits, high bits, compact representation, unboxing
Shape output, repr optimization, downstream of AIMS
AIMS ShapeClass, ValueRepr, ArcClass, representation
Stage 4, future repr optimizer
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Oxidizing OCaml (Modal Memory) | `section-01-oxidizing-ocaml.md` |
| 02 | FP² (Fully in-Place) | `section-02-fp2.md` |
| 03 | FIPTree (Constructor Contexts) | `section-03-fiptree.md` |
| 04 | TRMC (Tail Recursion Modulo Context) | `section-04-trmc.md` |
| 05 | Perceus for OCaml (Evaluation Methodology) | `section-05-perceus-ocaml.md` |
| 06 | Linearity and Uniqueness | `section-06-linearity-uniqueness.md` |
| 07 | Quantitative Type Theory | `section-07-quantitative-type-theory.md` |
| 08 | Lean 4 Borrow Inference | `section-08-lean4-borrow.md` |
| 09 | GHC Demand Analysis | `section-09-ghc-demand.md` |
| 10 | Concurrent Immediate RC | `section-10-concurrent-rc.md` |
| 11 | Cyclic RC for Immutable Data | `section-11-cyclic-rc.md` |
| 12 | Double-Ended Bit-Stealing | `section-12-bit-stealing.md` |
