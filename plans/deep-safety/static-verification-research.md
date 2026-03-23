# Static Contract Verification Research

Research into the path from runtime contracts (`pre()`/`post()`) to static proofs for the Ori compiler's deep safety initiative.

---

## 1. Dafny — Microsoft's Verification-Aware Language

### How `requires`/`ensures` Works

Dafny attaches preconditions (`requires`) and postconditions (`ensures`) directly to method signatures. The verifier checks every method in isolation using assume/guarantee reasoning: it assumes the precondition, asserts the postcondition, and for every called method, asserts its precondition and assumes its postcondition.

```dafny
method SquareRoot(N: nat) returns (r: nat)
    ensures r * r <= N < (r + 1) * (r + 1)
{
    r := 0;
    while (r + 1) * (r + 1) <= N
        invariant r * r <= N
    {
        r := r + 1;
    }
}
```

Key annotations:
- `requires` — preconditions on inputs
- `ensures` — postconditions on outputs (can name return value)
- `invariant` — loop invariants (critical — most annotation burden comes from these)
- `decreases` — termination proofs (expression that strictly decreases each iteration)
- `assert` — inline proof hints to guide Z3
- `modifies` — frame conditions (what heap objects a method may change)

### Z3 Integration

Dafny compiles to the Boogie intermediate verification language, which generates verification conditions (VCs) dispatched to Z3. The pipeline is: Dafny -> Boogie -> SMT-LIB2 -> Z3.

Concrete performance from Dafny 4.0.0 with Z3 4.12.1:
- Simple function verification: ~0.28s using ~84K resource units (RU)
- Assertion batch durations: 0.06–0.19s per batch
- Recommended resource limits: `--max-resource-count 200000` per function
- Recommended coefficient of variation: <20% (for reproducibility)

### Annotation Burden in Practice

This is the central cost of Dafny:

- **Loop invariants** are the dominant annotation cost. Every while loop needs an invariant, and finding the right one is the hard part of verification.
- **DARe tool evaluation** on 252 library programs found 88% of proof guidance lines (assertions, invariants, lemma-calls) were "dead annotations" not required for verification — suggesting overannotation is common in practice.
- **CompCert comparison**: the verified C compiler's proofs take **8x more lines** than the compiler itself (Coq proofs, not Dafny, but illustrative of the general overhead).
- **AWS Cedar authorization engine**: Dafny model is ~1/6 the lines of production code — but this is a model, not the production code itself being verified.
- **OOPSLA 2025 study** (14 experienced Dafny users): SMT solver "often requires hints in the form of assertions, creating a burden for the proof engineer." Users report writing specification and implementation concurrently to minimize cost.
- **AI-assisted annotation** (2025): dafny-annotator using LLMs achieves 86% correct proofs on DafnyBench (750+ programs, ~53K LOC).

### What Dafny CANNOT Verify

1. **Non-linear integer arithmetic**: Multiplication where both sides are variables, division, modulus — all undecidable. Z3 uses incomplete heuristics and frequently gives up.
2. **Heavy quantifiers**: `forall` and `exists` over infinite domains — undecidable in general. Dafny handles many cases but can timeout.
3. **Non-termination**: Dafny requires termination proofs (`decreases` annotations). Programs that intentionally don't terminate (servers, event loops) need special handling.
4. **Concurrency**: Dafny has limited concurrent verification support. No built-in reasoning about lock ordering, deadlock freedom, or data races.
5. **Floating-point**: IEEE 754 arithmetic is not well-supported by SMT solvers.
6. **Complex heap reasoning**: While Dafny supports heap via dynamic frames, complex linked data structures require significant manual annotation.

### Verification Instability ("Butterfly Effect")

A critical practical issue: the same verification problem can take seconds or hours depending on formulation. Minor syntactic changes — reordering declarations, adding unrelated code — can cause Z3 to timeout on previously-verified code. This makes Dafny verification fragile in CI pipelines. Dafny 4.x added resource counting to detect and manage this instability.

### Real-World Adoption

- AWS: Encryption SDK, Cedar authorization engine, s2n-tls (continuous formal verification)
- Amazon's s2n team reports continuous re-verification on every code change
- Microsoft Research: internal projects, Azure components
- VMware: network verification
- Academic: >750 programs in DafnyBench

---

## 2. Creusot — Deductive Verification for Rust

### Approach

Creusot translates annotated Rust programs into WhyML (the language of the Why3 verification platform), which then dispatches proof obligations to multiple SMT solvers and interactive provers.

### Handling Ownership/Borrowing in Proofs

Creusot's key innovation is the **prophecy model** via the `^` (final) operator in its specification language Pearlite. Given a mutable reference `b`, the expression `^b` denotes the value the referenced variable will have when the borrow's lifetime expires. This lets you specify what a function does to its mutable references:

```rust
#[requires(v.len() > 0)]
#[ensures((^v).len() == v.len() + 1)]
fn push_copy_of_first(v: &mut Vec<i32>) {
    let first = v[0];
    v.push(first);
}
```

Creusot **relies on Rust's borrow checker** for ownership discipline — it doesn't re-verify aliasing, it trusts rustc's guarantees and builds on them.

### What It Can/Cannot Verify

**Can verify:**
- Functional correctness of safe Rust code
- Generic code
- Code using Rust's ownership/borrowing model
- Complex data structures via the prophecy model
- Loop invariants, termination

**Cannot verify:**
- **Unsafe Rust** — fundamental limitation
- Does not handle raw pointer manipulation
- Not fully automated — requires manual specification effort

### Specification Syntax (Pearlite)

```rust
#[requires(R)]
#[ensures(E)]
fn my_function(b: u32) -> bool { .. }
```

Pearlite supports: quantifiers (`forall`, `exists`), logical implication (`==>`), logical equality, labels, the final operator `^`, and model access `@`.

### Annotation Burden

No published comprehensive burden measurements. Creusot is designed to minimize annotation by leveraging Rust's type system, but loop invariants and complex functional correctness still require significant specification effort.

---

## 3. Prusti — Automated Verification for Rust

### Viper Backend

Prusti translates Rust's MIR (mid-level IR) into VIR (Viper Intermediate Representation), based on Implicit Dynamic Frames — a variant of separation logic. Viper then dispatches to Z3 or Silicon (symbolic execution backend).

### Annotation Syntax

```rust
use prusti_contracts::*;

#[ensures(result >= a && result >= b)]
#[ensures(result == a || result == b)]
fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}
```

Functions used in specifications must be marked `#[pure]`.

### Annotation Burden — Measured

- **Up to 24% of code size** (worst case)
- **Average 14%** of code size
- Liquid typing approach reduces this to near-zero for lightweight verification use cases
- Verification time: grows super-linearly with code size (significant disadvantage vs. Verus)

### What Fraction of Rust Can It Handle?

Prusti supports a significant subset of safe Rust: structs, enums, references, generics, trait dispatch, iterators, closures (limited). It **cannot** reason about unsafe Rust.

### Comparison with Creusot

| Aspect | Prusti | Creusot |
|--------|--------|---------|
| Backend | Viper (IDF/separation logic) | Why3 (WhyML) |
| Mutation model | Separation logic | Prophecies (`^` operator) |
| Unsafe Rust | No | No |
| Automation | Higher for simple properties | Requires more manual spec |
| Annotation burden | Measured: avg 14% | Not formally measured |
| Verification scaling | Super-linear | Not measured |

---

## 4. Verus — Rust Verification with SMT

### How It Differs from Prusti/Creusot

Verus is the most distinctive of the three. Key differences:

1. **Extended Rust syntax** — uses a `verus!` macro to extend Rust with verification constructs. Specifications are written in Rust-like syntax, not a separate logic.
2. **Linear ghost types** — ghost objects represent verification-only permissions (e.g., memory access rights). Rust's linear type system ensures these are tracked correctly.
3. **Mode system** — distinguishes `spec` (unchecked, erased), `proof` (checked for linearity, erased), and `exec` (compiled) code.
4. **Unsafe Rust support** — can verify unsafe code by attaching `requires` clauses that encode safety preconditions.

### The `proof` Block Concept

```rust
verus! {
    fn octuple(x1: i8) -> (x8: i8)
        requires -16 <= x1 < 16,
        ensures x8 == 8 * x1,
    {
        let x2 = x1 + x1;
        let x4 = x2 + x2;
        x4 + x4
    }
}
```

Ghost code (`requires`, `ensures`, `assert`, `assume`) is erased before compilation — zero runtime overhead.

Proof blocks allow writing verification lemmas that are checked but produce no executable code:

```rust
proof fn lemma_sum(n: nat)
    ensures sum(n) == n * (n + 1) / 2,
    decreases n,
{
    if n > 0 {
        lemma_sum((n - 1) as nat);  // recursive proof step
    }
}
```

### Linear Ghost Code

Ghost objects represent abstract rights (e.g., permission to access a memory cell). Rust's ownership system ensures these permissions are unique and properly transferred. This allows reasoning about:
- Doubly-linked lists
- Reference-counted pointers
- Concurrent algorithms (locks, message queues, memory allocators)
- Raw pointer manipulation (conditionally safe via contracts)

### Performance: Verification Times (SOSP 2024)

The landmark SOSP 2024 paper provides concrete numbers:

- **Case studies**: 6,100 lines of implementation + 31,000 lines of proof (~35K lines total)
- **Proof-to-code ratio**: approximately **5:1** (5 lines of proof per line of implementation)
- **Verification speed**: **3x to 61x faster** than state of the art
- **Linear scaling**: ~1.6ms per verification push (remains linear, unlike Prusti's super-linear)
- **Kernel verification**: ~20 seconds for verified OS components, individual functions max ~4 seconds with runtime checks on
- **Full proofs**: ~30 minutes with all runtime checks off (vs. hours for comparable tools)

### Real Systems Verified

1. **IronKV** — distributed key-value store (port from IronFleet), 10x faster verification
2. **Concurrent memory allocator** — mimalloc-based
3. **NUMA-aware concurrent data structure replication** (Node Replication)
4. **Persistent memory storage systems** — key-value store + append-only logs
5. **OS page table management** — with hardware memory translation modeling
6. **Asterinas OSTD** — formally-verified OS standard library for safe Rust

---

## 5. RefinedRust — Foundational Verification for Rust

### Coq/Iris Approach

RefinedRust defines a refinement type system proven sound in the Coq proof assistant, using the Iris separation logic framework. The workflow:

1. Translate annotated Rust code into a Coq-embedded model
2. Check adherence to the RefinedRust type system using separation logic automation
3. All proofs are machine-checked by Coq — the TCB (trusted computing base) is Coq itself

### What It Can Verify About Unsafe Rust

RefinedRust is the **first tool** that simultaneously:
1. Handles real (surface) Rust code
2. Provides proof automation for both safe AND unsafe code
3. Outputs machine-checkable proofs

Verified examples include a variant of Rust's `Vec` implementation involving "intricate reasoning about unsafe pointer-manipulating code."

### Type System vs. SMT Approach

| Aspect | RefinedRust (Type System) | Verus/Prusti/Creusot (SMT) |
|--------|--------------------------|---------------------------|
| Backend | Coq + Iris | Z3/Why3/Viper |
| Trust base | Coq kernel (tiny) | SMT solver (large) |
| Automation | Semi-automated | More automated |
| Unsafe Rust | Yes (main strength) | Verus: yes; others: no |
| Proof artifacts | Machine-checkable Coq proofs | No exportable proofs |
| Scalability | Lower (interactive proving) | Higher |
| Annotation burden | Highest (Coq proof scripts) | Lower |

### Limitations

- Verification is significantly more labor-intensive than SMT-based tools
- Scaling to large codebases is challenging
- No published burden measurements or benchmark comparisons
- Limited to specific Rust patterns currently

---

## 6. Liquid Haskell — Refinement Types

### How Refinement Predicates Work

Refinement types are base types augmented with logical predicates:

```haskell
{-@ type Pos   = {v:Int | v > 0}   @-}
{-@ type NZero = {v:Int | v /= 0}  @-}
{-@ type Even  = {v:Int | v mod 2 = 0} @-}

{-@ safeDiv :: Int -> {v:Int | v /= 0} -> Int @-}
safeDiv x y = x `div` y

{-@ incr :: x:Int -> {v:Int | v = x + 1} @-}
incr x = x + 1
```

The predicates form function contracts:
```haskell
{-@ addOdds :: x:Odd -> y:Odd -> Even @-}
addOdds x y = x + y
```

### Z3 Integration

Liquid Haskell converts refinements to SMT constraints in a restricted logic (EUFLIA — equality, uninterpreted functions, linear integer arithmetic) and sends them to Z3. The key insight: by restricting predicates to a decidable logic fragment, verification is **always decidable** (modulo Z3 timeouts) — unlike Dafny's general first-order logic.

### Bounds, Alignment, Capability Invariants

**Bounds checking** — primary strength:
```haskell
{-@ type ArrayN a N = {i:Nat | i < N} -> a @-}
{-@ get :: n:Nat -> i:{Nat | i < n} -> ArrayN a n -> a @-}
```

**Alignment** — expressible via modular arithmetic predicates:
```haskell
{-@ type Aligned N = {v:Int | v mod N = 0} @-}
```

**Capability invariants** — limited. Refinement types can express value-level properties but not higher-order resource tracking. You can say "this integer is positive" but not "this handle has not been closed."

### Annotation Burden — Measured

- **~1 line of termination hints per 100 lines of source** for basic verification
- For full functional correctness: significantly more
- Verified ~10,000 lines of Haskell library code
- Key limitation: "Liquid Haskell lacks support for many standard features of Haskell"

### The "Gradual Refinement" Approach

Liquid Haskell supports gradual typing for refinements — you can start with no annotations and progressively add precision. Unrefined types are treated optimistically. This is the most relevant model for Ori's path:

1. Start with no refinements — existing code works unchanged
2. Add refinements to critical functions — bounds checks, non-null, alignment
3. Inference propagates refinements automatically where possible
4. Gradually increase coverage as the codebase matures

### Lazy Evaluation Complication

The classical refinement type translation is unsound under lazy evaluation. Liquid Haskell must track which binders reduce to values. **This is irrelevant for Ori** (strict evaluation), making Ori a better target for refinement types than Haskell.

---

## 7. CBMC / KLEE / Symbolic Execution

### CBMC — Bounded Model Checking

CBMC translates C programs (with assertions and loop unrolling) into a SAT/SMT formula, then checks if any execution can violate an assertion within a given bound (loop iterations, recursion depth).

**Can it verify contract-like properties?** Yes, within bounds:
- Array bounds checks: yes
- Null pointer dereferences: yes
- Arithmetic overflow: yes
- Buffer overflows: yes
- User-defined assertions: yes
- Complex invariants: limited by bound

**Scalability limits:**
- Loop unrolling is exponential in bound depth
- Large loop iterations cause memory exhaustion before verification even starts
- CBMC "fails even before starting the verification process due to insufficient memory for loop unwinding when dealing with programs containing large loop iterations"
- Practical limit: ~thousands of lines of C, not millions

**FFI verification relevance:** CBMC is directly applicable to verifying C code called via FFI. Ori's Deep FFI could use CBMC-style bounded checking on the C side of FFI boundaries.

### KLEE — Symbolic Execution

KLEE symbolically executes LLVM bitcode, exploring all feasible paths.

**Path explosion problem:**
- Paths grow exponentially with program size and branching depth
- Loops with symbolic conditions are the primary scalability killer
- Nested conditionals compound the problem
- Z3 constraint solving for heap-manipulating programs is incomplete

**State of the art (2024-2025):**
- Path prioritization (Empc) covers 19.6% more basic blocks than KLEE's best strategy
- LLM-assisted approaches augment symbolic execution (2025)
- Hybrid techniques combine fuzzing with symbolic execution
- Still limited to ~10K LOC in practice for exhaustive coverage

### Kani — Rust Model Checker (AWS)

Built on CBMC, Kani verifies Rust code including unsafe:
- Translates Rust MIR to GOTO programs (CBMC's IR)
- Verifies memory safety, UB absence, user assertions
- Used on AWS Firecracker, s2n-quic
- **Bounded**: verification in presence of loops requires bounds
- **Monomorphic only**: cannot verify generic code directly
- Does not support concurrency

---

## 8. The Realistic Path for Ori

### Current State

Ori's `pre()`/`post()` contracts are currently **runtime checks only**:

```ori
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= a div b;
```

Semantics: evaluate pre() conditions, execute body, evaluate post() conditions, panic on failure.

### Which Contract Forms Are Easy to Verify Statically?

**Tier 1 — Decidable, no SMT needed** (compiler dataflow analysis):
- Constant comparisons: `pre(n > 0)` where n is a literal or const at the call site
- Null/None checks: `pre(x != None)` after a pattern match established non-None
- Boolean flags: `pre(is_initialized)` tracked through control flow
- Simple bounds on const generics: `pre($N > 0)` — already known at compile time

**Tier 2 — Decidable with refinement type inference** (linear arithmetic + Z3):
- Integer bounds: `pre(0 <= index && index < len)` — Liquid Haskell's core strength
- Non-zero division: `pre(divisor != 0)` — classic refinement type application
- Alignment: `pre(addr % alignment == 0)` — modular arithmetic in EUFLIA
- Capacity: `pre(list.len() < capacity)` — linear arithmetic
- Range membership: `pre(lo <= val && val <= hi)` — interval analysis

**Tier 3 — Requires full SMT, may timeout** (first-order logic + Z3):
- Relational postconditions: `post((f, t) -> f.balance + t.balance == from.balance + to.balance)`
- Data structure invariants: sorted, balanced, heap-ordered
- Complex arithmetic: `post(r -> r * r <= n)` with non-linear multiplication
- Protocol compliance: state machine transitions
- Lock ordering: complex capability interactions

**Tier 4 — Undecidable/impractical** (needs interactive proofs or is fundamentally impossible):
- Non-linear arithmetic with variable multiplication
- Quantified heap properties
- Termination of arbitrary recursive functions
- Information flow / security properties
- Concurrency correctness (deadlock, livelock, starvation)

### Minimal Infrastructure for Static Contract Verification

**Phase 1 — Refinement types on const generics (Year 1)**

Ori already has const generics (`$N: int`) with constraints (`where N > 0`). Extend this to function parameters:

```ori
// Today: runtime check
@safe_index (list: [T], index: int) -> T
    pre(0 <= index && index < list.len())
= list[index];

// Tomorrow: refinement type, checked at compile time
@safe_index (list: [T], index: {int | 0 <= index && index < list.len()}) -> T
= list[index];
```

Implementation:
1. Parse refinement predicates on parameter types (reuse expression parser)
2. At call sites, check if the refinement is statically provable from context
3. If provable: elide runtime check
4. If not provable: keep runtime check (graceful degradation)
5. Optional warning when a contract cannot be statically verified

This is the **Liquid Haskell model** applied to Ori. Key advantage: Ori's strict evaluation avoids Liquid Haskell's lazy evaluation unsoundness.

**Phase 2 — SMT integration for Tier 2 contracts (Year 1-2)**

Integrate Z3 (or an equivalent SMT solver) as an optional verification backend:
1. Translate `pre()`/`post()` to SMT-LIB2 queries
2. For Tier 2 contracts (linear arithmetic), Z3 decides in milliseconds
3. Cache verification results per function signature (Salsa-compatible)
4. Incremental re-verification: only re-verify functions whose contract or body changed
5. Report: "Contract statically verified" / "Contract requires runtime check" / "Contract may be violated" (with counterexample)

Estimated compile-time impact: 1-5ms per function with contracts (based on Verus's ~1.6ms/push scaling).

**Phase 3 — Full contract verification mode (Year 2-3)**

Optional `ori check --verify` mode that attempts to prove all contracts:
1. Translate function bodies to verification conditions
2. Dispatch to Z3 with configurable timeout (default: 5s per function)
3. Report results: verified / unverified / counterexample found
4. Support manual proof hints via `assert` statements in function bodies
5. Support `decreases` annotations for termination proofs

### Can Ori Start with "Refinement Types on Const Generics"?

**Yes, and this is the recommended starting point.** Reasons:

1. Const generics already have constraints (`where N > 0`) — this is already a primitive refinement
2. The existing `pre()`/`post()` syntax maps directly to refinement type annotations
3. Ori's expression-based design and value semantics simplify reasoning (no aliasing, no mutation of shared state)
4. ARC memory management eliminates dangling pointer reasoning entirely
5. Strict evaluation avoids Liquid Haskell's unsoundness problem

The natural progression:
```
const generic bounds → parameter refinements → return refinements → inference → full SMT
```

### Realistic 3-Year Timeline

**Year 1: Foundation**
- Q1-Q2: Implement refinement type syntax on function parameters
- Q2-Q3: Build abstract interpretation for Tier 1 contracts (dataflow, no SMT)
- Q3-Q4: Z3 integration for Tier 2 contracts (linear arithmetic)
- Q4: Incremental verification cache (Salsa integration)
- Milestone: `pre(bounds_check)` statically eliminated at call sites

**Year 2: Coverage**
- Q1-Q2: Refinement type inference (propagate refinements from callees to callers)
- Q2-Q3: `post()` verification via weakest precondition calculus
- Q3-Q4: Loop invariant support (manual annotation, similar to Dafny)
- Q4: `ori check --verify` command with configurable strictness
- Milestone: Full verification mode for Tier 2 contracts

**Year 3: Maturity**
- Q1-Q2: AI-assisted annotation (like Dafny's dafny-annotator, using LLMs)
- Q2-Q3: Cross-function modular verification (assume/guarantee)
- Q3-Q4: Capability interaction verification (lock ordering, resource protocols)
- Q4: Integration with Deep FFI (verify C-side contracts via CBMC/Kani-style checking)
- Milestone: Kernel-level code with statically verified contracts

---

## 9. Key Costs and Tradeoffs

### Compile Time Impact

| Tool | Verification Time | Scaling |
|------|------------------|---------|
| Verus | ~1.6ms per push (linear) | Best in class |
| Dafny | 0.06-0.3s per assertion batch | Mostly linear |
| Prusti | 14% annotation overhead avg | Super-linear |
| Liquid Haskell | Fast for decidable fragments | Near-instant for EUFLIA |
| CBMC/Kani | Seconds to minutes per function | Exponential with loops |
| seL4 (Coq) | Hours for full proof | Non-incremental |

**For Ori**: targeting Phase 1-2, expect 1-10ms per function with contracts. For a 1000-function project, this adds 1-10 seconds to compilation — acceptable. Full SMT verification (Phase 3) could add 30-60 seconds for large projects.

### False Positive Rates

SMT-based verification is **sound but incomplete**: it may report unverifiable contracts that are actually correct. This manifests as:
- "Unknown" results when Z3 times out
- Spurious failures from abstraction (erasing heap knowledge, imprecise types)
- Non-reproducible results (Z3's internal heuristics are sensitive to input ordering)

Mitigation strategies:
1. Restrict to decidable fragments (Liquid Haskell approach) — zero false positives for EUFLIA
2. Resource limits with reproducibility checking (Dafny 4's coefficient of variation < 20%)
3. Graceful degradation: unverifiable contracts fall back to runtime checks (never a compilation failure)

### Incremental Verification

Critical for Ori's Salsa-based architecture:
- **Function-level granularity**: only re-verify functions whose body or contract changed
- **Signature-level caching**: if a function's signature (with contracts) is unchanged, callers don't need re-verification
- **SMT solver caching**: Z3 supports incremental solving (push/pop), reusing learned lemmas
- **Salsa integration**: verification results as Salsa-tracked queries — automatic invalidation and recomputation

### Z3 Timeout/Undecidability Issues

The fundamental challenge:
- Non-linear integer arithmetic is **undecidable**
- Quantifiers over infinite domains are **undecidable**
- Z3 uses heuristics that may succeed or fail non-deterministically
- The "butterfly effect": minor code changes can cause previously-fast verification to timeout

Practical mitigations:
1. Resource limits per query (Dafny: 200K RU)
2. Timeout per function (recommended: 5-10 seconds)
3. Deterministic resource counting (not wall-clock time)
4. Z3's `smt.arith.solver=6` for better non-linear arithmetic handling
5. Split complex verification conditions into smaller obligations

### The "Specification Gap"

Properties that are hard to state formally:
- "This function is correct" (requires a formal specification of correctness)
- "The system is secure" (requires a formal security model)
- "The code is performant" (not a logical property)
- "The API is ergonomic" (subjective)
- "The error messages are helpful" (not formalizable)

For Ori's purposes, the specification gap is manageable because:
1. `pre()`/`post()` are already formal specifications — users wrote them
2. The question is whether the compiler can verify them, not whether they exist
3. Ori can start with verifying the contracts users already write
4. No need to invent specifications — just verify the ones that exist

### Annotation Overhead Summary

| System | Proof:Code Ratio | Annotation % | Notes |
|--------|-----------------|-------------|-------|
| seL4 (Coq/Isabelle) | **20:1** | — | 200K lines proof for 10K lines code |
| CompCert (Coq) | **8:1** | — | Verified C compiler |
| Verus (SMT) | **5:1** | — | 31K proof for 6.1K impl |
| Prusti (Viper) | — | **14% avg, 24% max** | Annotation as % of code size |
| Liquid Haskell (Z3) | — | **~1%** for basic | 1 line hint per 100 LOC |
| Dafny (Z3) | — | **Highly variable** | Depends on property complexity |
| Ori target (Phase 1) | — | **~2-5%** | Simple bounds/null checks only |
| Ori target (Phase 3) | — | **~10-15%** | Full contract verification |

---

## 10. Integration Recommendations for Ori

### What to Build

1. **Refinement type annotations** — extend `pre()`/`post()` to be verification targets, not just runtime checks
2. **Abstract interpreter** — dataflow analysis for Tier 1 (no SMT dependency)
3. **Z3 integration** — optional, for Tier 2+ contracts
4. **Verification cache** — Salsa-tracked, function-level granularity
5. **Graceful degradation** — unverifiable contracts remain runtime checks
6. **`ori check --verify`** — explicit verification mode

### What NOT to Build

1. **Do NOT build an interactive prover** — that's RefinedRust/Coq territory, 100x the effort
2. **Do NOT require all contracts to be statically verifiable** — the runtime fallback is essential
3. **Do NOT make Z3 a hard dependency** — it should be optional for the verification pass
4. **Do NOT verify termination by default** — too high annotation burden for too little benefit
5. **Do NOT attempt concurrency verification initially** — it requires fundamentally different techniques

### Ori's Unique Advantages for Verification

1. **Value semantics**: no aliasing, no shared mutable state — simplifies reasoning dramatically
2. **Expression-based**: no statements, no side effects in expressions — cleaner VCs
3. **ARC memory**: no dangling pointers, no use-after-free — eliminates a major verification domain
4. **Strict evaluation**: no lazy evaluation unsoundness (unlike Haskell)
5. **Existing contracts**: `pre()`/`post()` already exist — just need to make them static
6. **Capability system**: effects are tracked — can be incorporated into verification conditions
7. **Const generics with constraints**: already a primitive form of refinement types
8. **Salsa architecture**: natural incremental verification granularity

### The Core Insight

Ori's `pre()`/`post()` contracts are already specifications. The path to static verification is not "add specifications" but "verify existing specifications." This is fundamentally different from tools like Dafny (where you write a new program in a verification language) or Verus (where you extend Rust with verification macros). Ori's contracts are already in the language — the compiler just needs to learn to prove them.

---

## Sources

### Dafny
- [Dafny Verification Optimization](https://dafny.org/latest/VerificationOptimization/VerificationOptimization)
- [Dafny GitHub](https://github.com/dafny-lang/dafny)
- [DafnyBench: A Benchmark for Formal Software Verification (POPL 2025)](https://popl25.sigplan.org/details/dafny-2025-papers/15/DafnyBench-A-Benchmark-for-Formal-Software-Verification)
- [dafny-annotator: AI-Assisted Verification](https://dafny.org/blog/2025/06/21/dafny-annotator/)
- [DafnyPro: LLM-Assisted Automated Verification](https://arxiv.org/html/2601.05385)
- [On the Impact of Formal Verification on Software Development (OOPSLA 2025)](https://dl.acm.org/doi/10.1145/3763181)

### Creusot
- [Creusot GitHub](https://github.com/xldenis/creusot)
- [Creusot: A Foundry for the Deductive Verification of Rust Programs](https://jhjourdan.mketjh.fr/pdf/denis2022creusot.pdf)
- [Creusot INRIA Technical Report](https://inria.hal.science/hal-03737878v1/document)

### Prusti
- [The Prusti Project: Formal Verification for Rust](https://pm.inf.ethz.ch/publications/AstrauskasBilyFialaGrannanMathejaMuellerPoliSummers22.pdf)
- [Prusti GitHub](https://github.com/viperproject/prusti-dev)
- [Prusti User Guide](https://viperproject.github.io/prusti-dev/user-guide/basic.html)

### Verus
- [Verus Guide: requires/ensures](https://verus-lang.github.io/verus/guide/requires_ensures.html)
- [Verus: A Practical Foundation for Systems Verification (SOSP 2024)](https://dl.acm.org/doi/10.1145/3694715.3695952)
- [Verus: Verifying Rust Programs using Linear Ghost Types (OOPSLA 2023)](https://arxiv.org/abs/2303.05491)
- [Verus GitHub](https://github.com/verus-lang/verus)
- [Verus Projects](https://verus-lang.github.io/verus/publications-and-projects/)
- [CMU Blog: Verus](https://www.cs.cmu.edu/~csd-phd-blog/2023/rust-verification-with-verus/)

### RefinedRust
- [RefinedRust Project Page](https://plv.mpi-sws.org/refinedrust/)
- [RefinedRust: A Type System for High-Assurance Verification (PLDI 2024)](https://iris-project.org/pdfs/2024-pldi-refinedrust.pdf)

### Liquid Haskell
- [Liquid Haskell Refinement Types Course](https://nikivazou.github.io/lh-course/Lecture_01_RefinementTypes.html)
- [LiquidHaskell: Experience with Refinement Types in the Real World](https://goto.ucsd.edu/~nvazou/real_world_liquid.pdf)
- [Liquid Haskell Tutorial](https://ucsd-progsys.github.io/liquidhaskell-tutorial/book.pdf)
- [Gradual Liquid Type Inference](https://arxiv.org/abs/1807.02132)
- [Refinement-Types Driven Development: A study (2025)](https://arxiv.org/html/2509.15005)

### CBMC / KLEE / Kani
- [CBMC: C Bounded Model Checker](https://www.cprover.org/cbmc/)
- [Kani Rust Verifier (AWS)](https://github.com/model-checking/kani)
- [Verify the Safety of the Rust Standard Library (AWS)](https://aws.amazon.com/blogs/opensource/verify-the-safety-of-the-rust-standard-library/)
- [Rust Verification Tool Suitability Survey](https://rust-lang.github.io/rust-project-goals/2024h2/std-verification.html)

### Verification Overhead
- [seL4: Formal Verification of an OS Kernel](https://www.sigops.org/s/conferences/sosp/2009/papers/klein-sosp09.pdf)
- [The Verification Gap](https://concerningquality.com/verification-gap/)
- [SMT-based verification via summary repair](https://link.springer.com/article/10.1007/s10703-023-00423-0)

### General
- [F*: A Proof-Oriented Programming Language](https://fstar-lang.org/)
- [Programming Z3](https://theory.stanford.edu/~nikolaj/programmingz3.html)
- [Symbolic Execution in Practice: A Survey (2025)](https://arxiv.org/pdf/2508.06643)
