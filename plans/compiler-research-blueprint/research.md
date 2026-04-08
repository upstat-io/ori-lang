# Advanced Compiler Architecture and Optimization Strategies

**Source:** `Compiler Research for Ori-Lang Improvement.pdf` (19 pages, ingested 2026-04-07)
**Subtitle:** A Blueprint for Next-Generation Systems Languages
**Status:** Research digest — not yet a plan. Informs future work on AIMS, capabilities, type inference, and codegen.

---

## Thesis

To definitively surpass Rust in **both** runtime performance and developer ergonomics, Ori must integrate four deeply interconnected research frontiers:

1. A **unified abstract interpretation lattice** for AIMS (eliminating phase-ordering)
2. **Perceus + FBIP + TRMc** for free in-place mutation of functional code
3. **System Capless / Reach Capabilities** for capability tracking through generics
4. **Saturating LVars** for parallel Hindley-Milner inference

Plus a **dual-backend codegen pipeline** (Cranelift/TPDE for dev, MLIR → LLVM for release).

---

## 1. AIMS as a Unified Abstract Interpretation Lattice

### The Phase-Ordering Problem

Traditional ARC pipelines insert retain/release, then run isolated COW, borrow inference, uniqueness, and reuse passes sequentially. This causes phase-ordering pathology: a DCE pass that fails to recognize a branch as dead forces the uniqueness analyzer to conservatively assume escape, degrading `Unique → Shared` and injecting atomic RC ops.

### The Unified Lattice

Replace AIMS's sequential pipeline with a single Cartesian-product lattice:

```
D = U × A × C
```

- **U — Uniqueness lattice:** `⊥, Unique(RC=1), Shared(RC>1), Dead(RC=0), ⊤`
- **A — Aliasing lattice:** points-to sets, escape tracking, crucial for `Unique → Shared` transitions across function boundaries and async yield points
- **C — Constant lattice:** SCCP-style ranges and values, operates on the CFG for DCE and branch prediction

Merging these domains into a single monotonic transfer function, the compiler evaluates the CFG until fixed point. Widening operators (∇) at loop headers ensure termination on infinite-height lattices.

### Selective Widening via MVFG + WTO

Applying widening uniformly across all variables is wasteful. A **Modular Value-Flow Graph (MVFG)** aligned with the **Weak Topological Ordering (WTO)** of the CFG identifies the minimal set of variables requiring widening — those participating in value-flow back edges.

**Claimed result:** 41.2% analysis time reduction while maintaining optimal precision for uniqueness analysis (ref 23).

### SCCP Integration

Sparse Conditional Constant Propagation integrated into the unified lattice:
- SCCP assumes conditional branches are **dead** and values are **constant** until proven otherwise (optimistic)
- When SCCP proves a branch dead, the unified lattice immediately prunes it from both **A** and **U** analyses
- Prevents conservative degradation: in a disjoint architecture, the uniqueness pass might see a potential alias in a branch SCCP could have proven unreachable, and mark the object `Shared`
- Unified: the variable maintains `Unique` status across complex control flows

### The Eight Stacked Optimizations (AIMS Facets)

The document enumerates the stacked layers AIMS already claims to implement:

1. **Scalar Optimization** — elide RC for heap-free types (ints, bools, small structs)
2. **Interprocedural Borrow Inference** — whole-module call graph analysis for read-only params
3. **Precise Cleanup** — drop at last-use in SSA graph, not lexical scope end
4. **Drop + Allocate Reuse** — wire free + malloc of identical size together
5. **Surgical Struct Updates** — field-level uniqueness tracking for in-place partial updates
6. **Redundant RC Elimination** — cancel retain/release pairs within basic blocks or across provable paths
7. **O(1) Collection Mutations** — static uniqueness bypasses runtime checks; COW fallback on aliasing
8. **Static Ownership Proof** — the terminal goal: bypass runtime branches entirely when uniqueness is proven at compile time

---

## 2. Perceus + FBIP + TRMc

### The Perceus Resurrection Hypothesis

In standard functional programming, a data object frequently "dies" (becomes unreachable) immediately before a new object of the **exact same size, shape, and type** is allocated. Traditional tracing GCs suffer because they generate ephemeral garbage causing latency spikes. Standard ARC decrements to zero, calls `free()`, then immediately calls `malloc()`.

**Perceus optimization:** when the unified lattice determines an object is unique (`x ↦₁ v`), the compiler is permitted to perform a destructive update — reuse the exact memory address of the dying object for the newly allocated one. Eliminates allocator round-trips on hot paths.

### Tail-Recursion Modulo cons (TRMc)

TRMc rewrites recursive constructor functions into tail-recursive form, allowing them to accumulate arguments and update data structures in-place without growing the call stack.

**Linear chains:** a path from the top of a context to its execution "hole" where every object along the path is unique by construction. Because the top-level dominates the heap allocation and internals are strictly unique and unreachable by any alias, destructive in-place updates cannot cause side effects visible to other parts of the program.

**Claimed result:** balanced red-black tree insertions and complex functional mapping achieve execution speeds competitive with — and occasionally superior to — hand-optimized C++ imperative loops (ref 13).

### Non-Linear Control Flow Handling

Effect handlers and `call/cc` can cause a continuation to resume multiple times, violating uniqueness. Perceus handles this via runtime adaptation:
- If the continuation is unique (`RC = 1`), proceed with fast in-place update
- If non-linear control flow caused `RC > 1`, automatically fall back to copying the context

### Feature Comparison (Paper Table, Page 6)

| Feature | Rust (Borrow Checker + Affine Types) | Ori-Lang (Perceus FBIP + Unified Lattice) |
|---|---|---|
| Memory Guarantee Mechanism | Compile-time lifetime and borrow analysis | Compile-time uniqueness + Precise Runtime RC |
| Mutability Model | Explicit `&mut`. Cannot alias mutable references | Value semantics. Aliasing dynamically triggers COW |
| Complex Data Structures | Graphs/doubly-linked lists require `RefCell` or `unsafe` | Native support via value semantics and explicit arenas |
| Developer Ergonomics | High cognitive load tracking lifetimes | Low cognitive load, writes like functional scripting |
| In-place Updates | Manual imperative mutation required | Automatic extraction of functional intent via TRMc |

---

## 3. Surpassing Rust's Borrow Checker

### Polonius Stagnation

Polonius (Rust's NLL successor) has been blocked from stabilization for nearly 7 years due to severe performance regressions. The location-sensitive analysis leads to combinatorial explosions in memory consumption and compilation time when evaluating complex functions or deeply nested data structures.

**Ori's structural sidestep:** value semantics + explicit capabilities + advanced ARC never attempts to solve complex lifetime constraint equations. The burden of safety shifts from a static reachability solver to the AIMS unified lattice and fallback runtime RC. Caveat: this absence of static lifetimes means the compiler must rely entirely on uniqueness analysis to elide RC overhead; if the lattice is insufficiently precise, the resulting binary will suffer from retain/release thrashing.

### Fractional Uniqueness

Research bridge between functional ownership and high-performance sharing (ref 36):
- Unique reference holds permission `1.0`
- Shared references hold fractional values (e.g., `0.5` and `0.5`)
- Compiler statically tracks dispersion and recombination
- When the lattice proves all fractions mathematically recombined to `1.0` within a thread, allows safe deallocation or in-place mutation without runtime atomic checks

### Lock-Free Reference Counting (LFRC)

Traditional ARC stalls under multi-threaded workloads due to cache-line bouncing on atomic inc/dec. Novel LFRC algorithms mitigate this by deferring RC updates or using thread-local birth eras.

**Hyaline-1S epoch-based reclamation (ref 38):**
- Reference counting strictly during reclamation phase
- Each thread maintains a "birth era"
- Object reclaimed when its implicit lifetime interval does not overlap with any active thread's era
- Ensures safety under stalled threads, dramatically improving throughput of lock-free concurrent queues and maps

---

## 4. Explicit Effects and System Capless

### The Scope Extrusion Problem

In a naive effect system, generic data structures (arrays, sets, futures) lose the precise effect types of their contained elements. Preventing a generic `Set<T>` from leaking a network capability previously required explicit effect annotations on every type parameter — polluting the codebase.

### System Capless (Odersky et al., OOPSLA 2025, ref 40)

State-of-the-art solution via three mathematical concepts:

**1. Boxing**
A box securely encapsulates an impure, capability-capturing value, presenting it to the type system as pure. Executing the enclosed value requires explicit "unboxing," which restores the captured capabilities to the current local scope — preventing invisible leaks.

**2. Existential Capture Sets**
Refines the universal capability cap (which previously blocked generic structures) into an existential capture set. A function returning `Future^{cap}` is treated internally as returning an existentially quantified type: `∃c. Future^c`.

**3. Reach Capabilities (rcaps)**
Lightweight notation (e.g., `@use ops*`) asserts that a value can only capture capabilities strictly reachable through the specific variable container — without exposing the underlying mathematics to the developer.

**Result:** full ergonomic effect polymorphism. Functions that map over a list of effectful closures don't need to explicitly declare the union of all possible effects; the compiler propagates effects through generic boundaries via existential boxes.

### Algebraic Effects and Handler Semantics

Capabilities defined as free monad operators that yield control to a dynamically scoped handler (ref 46). Enables high-performance cooperative multi-threading, custom exception routing, and probabilistic programming models without altering the underlying runtime or OS threads.

---

## 5. Accelerating Type Inference

### Parallel Hindley-Milner via Saturating LVars

Traditional HM is strictly sequential — processing AST nodes one at a time, inherently limiting compilation velocity on multi-core hardware.

**LVars (Lattice Variables, ref 53):**
- Concurrent data structures whose state monotonically advances along a user-defined mathematical lattice
- In type inference, the lattice represents type refinement: polymorphic/generic → strictly unified/concrete
- Because state changes only flow in one direction (toward greater specificity), multiple threads can traverse the AST and attempt to unify types concurrently without traditional locks

**Saturating LVars:** extend this by safely allowing the data structure to release memory during the object's lifetime once a "saturated" (fully unified) state is reached — preventing memory bloat during massive compilations.

**Claimed result:** up to **8.46x parallel speedup** over traditional sequential inference.

### Local Type Inference + Context-Free Session Types

Bidirectional type checking propagates constraints efficiently from expected types down to expressions, eliminating whole-program constraint networks. Synergizes with System Capless: local type inference allows the compiler to instantly resolve existential capture sets of variables locally within a function's scope. Results stored in function summaries, enabling parallel threads to compile dependent functions without waiting for full CFG traversals.

---

## 6. Mandatory Testing and the Virtuous Cycle

### Tests as PGO Data

Embedded mandatory tests enable automatic generation of Profile-Guided Optimization data. Because every function is executed during compilation via its test suite, the compiler can track branch probabilities, loop bounds, and cache access patterns in real-time, feeding directly back into the optimization pipeline.

**Synergy with AIMS:** if tests reveal a functional collection is shared 99% of the time dynamically, the compiler can statically abandon expensive abstract interpretation and directly emit the COW instruction. Conversely, if tests prove a hot-loop exclusively manipulates unique data, the backend can aggressively unroll, confident that FBIP in-place mutations will never trigger an atomic RC stall.

### Capabilities as Native Mocks

By defining side-effects strictly through capabilities (`uses Http`), mocking becomes a native compiler intrinsic. Through `with...in` syntax, a test function injects a deterministic mock directly into the capability slot. Because System Capless mathematically proves that no effects escape their capture sets via reach capabilities, the mock is hermetically sealed. Tests execute synchronously, deterministically, with zero runtime reflection overhead.

---

## 7. Codegen Pipeline: MLIR + Cranelift/TPDE + LLVM

### MLIR's Role in Mid-Level Optimizations

Bridge the semantic gap between a high-level AST utilizing capability tracking/FBIP and low-level hardware-specific LLVM IR via MLIR (Multi-Level Intermediate Representation).

By defining a **custom MLIR dialect for ARC operations, capability bounds, and uniqueness states**, the compiler can perform high-level loop unrolling, polyhedral transformations, and fusion operations **before** the IR is degraded into untyped LLVM instructions.

**Key benefits:**
- **Polyhedral optimizations:** MLIR enables affine loop transformations that maximize cache locality
- **ARC elision in MLIR:** running AIMS over an MLIR dialect allows static elimination of retain/release pairs across complex CFGs **before** LLVM's aggressive `instcombine` phase scrambles structural intent

### Dual-Backend Strategy

| Backend | Primary Focus | Compilation Speed | Execution Performance | Pipeline Integration |
|---|---|---|---|---|
| **LLVM (-O3)** | Peak scalar/vector, advanced vectorization | Very Slow | Exceptional | Production release binaries |
| **Cranelift** | Fast JIT/AOT via equality saturation | Fast | Moderate | Iterative dev/debug |
| **TPDE Variant** | Register allocator heuristic bypass | Ultra Fast | Low | Continuous testing graph |
| **MLIR** | Polyhedral transformations & domain-specific dialects | N/A (mid-tier IR) | N/A | High-level FBIP fusions |

**Empirical data:**
- Cranelift compiles code **20-35% faster** than unoptimized LLVM
- TPDE variants compile **4.27x faster** than Cranelift's default backtracking register allocator (ref 61)
- Cranelift binaries run **1.6-2x slower** than LLVM -O3 at runtime (ref 58)

**Recommended architecture:**
1. **Iterative Development Tier (Cranelift/TPDE):** leveraged exclusively for instant iteration; mandatory dependency-aware testing means developers trigger frequent recompilations; sub-second feedback loop
2. **Production Release Tier (LLVM 21+):** AIMS-processed MLIR fed into modern LLVM for maximum native performance and cross-platform support

---

## 8. Curated Research Papers

The document's most actionable artifact. Ranked by direct leverage on Ori's architecture:

| Rank | Topic | Paper | Application |
|---|---|---|---|
| 1 | Memory Reuse | **Perceus: Garbage Free Reference Counting with Reuse** (Reinking et al.) — `microsoft.com/en-us/research/wp-content/uploads/2020/11/perceus-tr-v1.pdf` (ref 14) | Formalizes precise RC + reuse; foundation for AIMS in-place mutation |
| 2 | Functional Optimization | **FP2: Fully In-Place Functional Programming / Tail Recursion Modulo Context** (Lorenzen) — `antonlorenzen.de/papers/trmc-jfp.pdf` (ref 27) | TRMc derivation rules and linear chain identification |
| 3 | Effect Systems | **What's in the Box: Ergonomic and Expressive Capture Tracking over Generic Data Structures** (Odersky et al., OOPSLA 2025) — `bracevac.org/assets/pdf/oopsla25full.pdf` (ref 40) | System Capless, reach capabilities, existential capture sets — solves scope extrusion |
| 4 | Type Inference | **Parallel Type-checking with Saturating LVars** — `ccs.neu.edu/home/samth/parallel-typecheck-draft.pdf` (ref 53) | 8.46x parallel HM speedup |
| 5 | Abstract Interpretation | **Abstract Interpretation: A Unified Lattice Model for Static Analysis** (Cousot & Cousot, 1977) — `di.ens.fr/~cousot/publications.www/CousotCousot-POPL-77-ACM-p238--252-1977.pdf` (ref 17) | Mathematical bedrock for the AIMS unified lattice |
| 6 | Concurrent Memory | **Fixing Non-blocking Data Structures for Better Compatibility with Memory Reclamation Schemes** (Cohen et al.) — `arxiv.org/html/2504.06254v3` (ref 38) | Hyaline-style epoch-based LFRC |
| 7 | Concurrency Ownership | **Functional Ownership through Fractional Uniqueness** — `researchgate.net/publication/380189367` (ref 36) | Bridge between functional ownership and concurrent sharing |
| 8 | Capability Safety | **Robust and Compositional Verification of Object Capability Patterns** (Swasey et al.) — `pure.mpg.de/.../2550565/content` (ref 52) | Separation logic for capability safety |
| 9 | Capability Calculus | **An Effectful Object Calculus** (ECOOP 2025) — `drops.dagstuhl.de/.../LIPIcs.ECOOP.2025.8` (ref 44) | Free monad semantics for algebraic effects |
| 10 | Backend Speed | **Equality Saturation for Optimizing High-Level Julia IR** — `arxiv.org/html/2502.17075v2` (ref 63) | Cranelift e-graph / EqSat approach |
| 11 | Selective Widening | **Efficient Abstract Interpretation via Selective Widening** — `research-management.mq.edu.au/.../466699559.pdf` (ref 23) | MVFG + WTO for 41.2% analysis speedup |

---

## 9. Provenance and Calibration

### Document Origin

Appears to be an LLM-generated research synthesis (style consistent with Gemini). Implications:

- **Self-referential claims:** repeatedly cites `ori-lang.com` and the upstat-io GitHub repo (refs 5, 11, 12, 15) — some "Ori does X" statements describe your own published design back to you. Treat these as coherence checks on internal consistency, not external validation.
- **Single-paper benchmarks:** claimed performance numbers (8.46x parallel HM, 41.2% selective widening, 4.27x TPDE) reflect specific microbenchmarks, not replicated/averaged results.
- **Citations are real and high-quality:** the OOPSLA 2025 Capless paper, TRMc-JFP, Perceus, Cousot 1977, and Hyaline references are foundational. The reading list is the document's most valuable output.

### What This Document IS

- A structural alignment map between academic research frontiers and Ori's architectural goals
- A curated reading list with direct application notes
- A coherent narrative for *why* the AIMS unified lattice + Perceus + Capless + Saturating LVars are a complementary stack, not disconnected features

### What This Document IS NOT

- A verified benchmark report
- An implementation plan
- External validation of claims Ori already makes about itself

---

## 10. Recommended Next Steps

Not yet committed work — requires user decision before becoming plan items:

1. **Deep-read the top four papers** before finalizing any syntax or architectural decisions that intersect them:
   - Perceus (AIMS ground truth)
   - TRMc-JFP (library/std collection in-place mutation)
   - System Capless (capability-unification-generics-proposal)
   - Saturating LVars (ori_types parallelization)

2. **Cross-check AIMS vs. the unified-lattice claim** — audit `compiler/ori_arc/src/aims/` to determine whether existing passes are already effectively merged or still sequentially staged. If sequential, identify which pairings would benefit most from lattice-level unification (likely SCCP + uniqueness given the BUG-04-019 / TPR-07-016/017 history).

3. **Capability unification decision point** — the `capability-unification-generics-proposal` is in active design per `.claude/rules/ori-syntax.md`. System Capless may inform whether the `:` syntax needs a distinction for reach-capability containers versus structural capability types. **This is the most time-sensitive item.**

4. **MLIR research spike** — evaluate whether `ori_arc` IR could be expressed as an MLIR dialect. This is a large architectural question and should not be committed from one document. Scope: one spike, no production changes.

5. **Cranelift/TPDE investigation** — validates the direction of `plans/llvm-worker-isolation/` but argues for going further: a dev-tier backend separate from the release pipeline. Feasibility study, not commitment.

6. **LFRC / Hyaline for `ori_rt`** — currently single-threaded workloads dominate, but `Sendable` + channels will hit atomic RC thrashing once real concurrent workloads land. File as forward-looking but not urgent.

---

## Related Internal Work

- `.claude/rules/arc.md` — AIMS subsystem invariants (Contracts ↔ Certified realization, behavioral verification)
- `plans/llvm-worker-isolation/` — existing codegen isolation work; MLIR direction would extend this
- `plans/perf-engineering/` — interpreter/compiler performance; LVar parallelization fits here long-term
- `plans/bug-tracker/fix-BUG-04-019.md`, `fix-BUG-04-041.md` — path-sensitive take-project history that motivates the unified lattice argument empirically
- `plans/repr-opt/` — TPR-07-016/017 work which is essentially ad-hoc path-sensitivity; the paper argues this should live at the lattice level
- `docs/ori_lang/proposals/capability-unification-generics-proposal.md` — the proposal most directly affected by the System Capless discussion
