---
reroute: true
name: "Semantic Opt"
full_name: "Semantic Optimization Pipeline"
status: active
order: 1
---

# Semantic Optimization Pipeline Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Hygiene & Targeted Fixes
**File:** `section-01-hygiene.md` | **Status:** Not Started

```
traits/mod.rs, TraitRegistry, file split, bloat, 765 lines
aims_pipeline.rs, AimsPipelineConfig, 590 lines, pipeline phases
instr_dispatch.rs, ArcInstr emission, 587 lines, over 500-line limit
BUG-04-029, shift overflow, shl, ashr, checked_ops.rs
hygiene, prerequisite, BLOAT category
```

---

### Section 02: LLVM Metadata Infrastructure
**File:** `section-02-metadata-infra.md` | **Status:** Not Started

```
llvm-sys, metadata, MetadataValue, MDNode, MDBuilder
LLVMSetMetadata, LLVMMetadataAsValue, LLVMMDString, LLVMMDNode
IrBuilder, metadata helpers, instruction metadata
ArcInstr::Project, struct type ID, containing struct
inkwell, raw FFI, LLVM 21.1
```

---

### Section 03: AIMS State Export
**File:** `section-03-aims-export.md` | **Status:** Not Started

```
AimsStateMap, MemoryContract, ParamContract, ReturnContract
ArcIrEmitter, FunctionCompiler, define_phase.rs
noalias, alias.scope, fresh allocation, Uniqueness::Unique
EffectSummary, may_allocate, may_deallocate
run_arc_pipeline, AimsPipelineResult, state map export
```

---

### Section 04: TBAA, Range, Invariant Metadata
**File:** `section-04-tbaa-range.md` | **Status:** Not Started

```
!tbaa, type-based alias analysis, struct field access, struct_gep
!range, bounded returns, Ordering, bool, enum tag
!invariant.load, immutable, borrowed parameter, readonly
metadata emission, load instruction, store instruction
```

---

### Section 05: Structural Derive Normalization
**File:** `section-05-derive-norms.md` | **Status:** Not Started

```
memory(read), derived methods, DerivedTrait, nounwind pattern
is_nounwind_derived, is_readonly_derived, strategy dispatch
reflexive peephole, x == x, x != x, structural Eq
boolean simplification, double negation, !!x, BitAnd, BitOr, BitXor identity
GVN, CSE, common subexpression elimination
NOTE: &&/|| are NEVER PrimOps — lowered to control-flow IR via short-circuit
```

---

### Section 06: Runtime Identity Fixes
**File:** `section-06-runtime-identity.md` | **Status:** Not Started

```
ori_str_concat, OriStr, from_bytes, identity, empty string
SSO, is_sso, heap, slice, ori_str_rc_inc, rc_inc
ori_list_concat_cow, empty list, ownership transfer, consuming
cow_mode, cow_sort, SLICE_FLAG, is_slice_cap
identity element, zero element, algebraic identity
```

---

### Section 07: Algebra Law Schema
**File:** `section-07-algebra-schema.md` | **Status:** Not Started

```
AlgebraLaw, AlgebraDecl, algebra block, laws declaration
Associative, Commutative, Identity, Inverse, DistributesOver, Involutive
TraitDef, TraitItem, TraitEntry, ImplEntry, ImplDef
Zero, One, zero(), one(), identity element
parser, grammar, trait_def.rs, registration, prelude.ori
AlgebraLawIndex, AlgebraPurity, law-export, operator provenance, AimsPipelineConfig plumbing
```

---

### Section 08: Algebra Law Adoption
**File:** `section-08-algebra-adoption.md` | **Status:** Not Started

```
stdlib, prelude, Add, Sub, Mul, Div, Neg
int, float, str, list, bool, set
identity law, commutativity, associativity
Zero::zero, One::one, trait instance
validation, runtime semantics, soundness check
```

---

### Section 09: Algebraic Normalization Pass
**File:** `section-09-normalization.md` | **Status:** Not Started

```
normalization pass, step 3b, AIMS pipeline, pre-ARC
identity elimination, op(x, identity) -> x
double negation, -(-x) -> x, Involutive
commutative canonical ordering, CSE, operand sorting
associativity, flatten, rebuild, tree rebalance
additive inverse, x + (-x) -> zero
purity gate, side effect, user operator
PrimOp, ArcValue::PrimOp, ArcInstr::Let, ArcFunction, BinaryOp, UnaryOp
AlgebraLawIndex, law-export, plumbing, TraitRegistry
reflexive peephole, x == x -> true, x != x -> false (subsumed from 05.2)
bitwise identity, x & -1 -> x, x | 0 -> x, x ^ 0 -> x, annihilator (subsumed from 05.3)
NOTE: &&/|| are NEVER PrimOps — cannot be optimized at ARC IR level
```

---

### Section 10: Advanced Rewrites & Verification
**File:** `section-10-advanced-verification.md` | **Status:** Not Started

```
distributivity, distributes_over, factorization, cost model
multiplicative inverse, Inv, Recip, nonzero, invertible
division ring, field, Ring trait
verification suite, test matrix, soundness
dual-exec parity, eval vs LLVM, behavioral equivalence
ORI_CHECK_LEAKS, valgrind, memory safety
code journey, pipeline integration
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Hygiene & Targeted Fixes | `section-01-hygiene.md` |
| 02 | LLVM Metadata Infrastructure | `section-02-metadata-infra.md` |
| 03 | AIMS State Export | `section-03-aims-export.md` |
| 04 | TBAA, Range, Invariant Metadata | `section-04-tbaa-range.md` |
| 05 | Structural Derive Normalization | `section-05-derive-norms.md` |
| 06 | Runtime Identity Fixes | `section-06-runtime-identity.md` |
| 07 | Algebra Law Schema | `section-07-algebra-schema.md` |
| 08 | Algebra Law Adoption | `section-08-algebra-adoption.md` |
| 09 | Algebraic Normalization Pass | `section-09-normalization.md` |
| 10 | Advanced Rewrites & Verification | `section-10-advanced-verification.md` |
