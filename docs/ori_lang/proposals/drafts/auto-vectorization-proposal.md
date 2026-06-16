# Proposal: Provability-Gated Automatic Vectorization

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-16
**Affects:** Compiler (canonicalizer, ARC/AIMS analysis, LLVM codegen), evaluator (dual-execution parity), spec (Clause 20.8.4 cross-reference, new optimization-guarantee appendix)
**Depends On:** intrinsics-capability-proposal.md, intrinsics-v2-byte-simd-proposal.md

---

## Summary

Ori should automatically vectorize eligible loops **by default**, leaning on SIMD harder than mainstream systems compilers do — not by exposing more intrinsics, but by *guaranteeing* vectorization wherever the language can already *prove* it is safe. Ori's value semantics, capability effects, the `Value` trait, and `[T, max N]` give the compiler aliasing and purity proofs that C and Rust cannot hand LLVM, so loops that LLVM auto-vectorizes only opportunistically (or not at all) become a specified, dependable transformation in Ori. The already-approved `Intrinsics` capability remains the manual escape hatch underneath this layer. Automatic vectorization is defined as a **pure optimization**: it never changes a program's observable result, which makes dual-execution parity with the evaluator hold by construction.

---

## Motivation

### The gap is already named in the spec corpus

The approved **Iterator Performance and Semantics** proposal lists the compiler optimizations Ori *guarantees* — copy elision, inline expansion, deforestation, loop fusion — and then explicitly carves out what it does **not** guarantee:

> **Not Guaranteed**
> - Parallelization of sequential iteration
> - **Vectorization (SIMD) for numeric operations**
> - Custom optimizations for specific patterns
>
> These may be added in future compiler versions but are not part of the language specification.

This proposal converts that deferred "future compiler version" into a specified guarantee. The natural home for vectorization is the same "Guaranteed Optimizations" table iterator fusion already lives in — the only thing missing is the correctness gate that decides *when* it fires.

### Mainstream auto-vectorization is a fragile best-effort

In C, C++, Rust, Swift, and Zig, auto-vectorization is a best-effort LLVM pass operating on a language that cannot prove non-aliasing. The compiler must either prove no two pointers in a loop overlap (usually impossible without `restrict`/`noalias`) or insert runtime alias checks and a scalar fallback. The result is unpredictable: small, innocuous-looking source changes silently turn vectorization off, and there is no language-level promise that any particular loop vectorizes.

This is not hypothetical fragility — it shows up as a steady stream of real defects:

- Idiomatic iterator chains that fail to vectorize even when the math is trivially parallel (e.g. a `take` + `sum` reduction that LLVM leaves scalar).
- Target-specific miscompiles and alignment faults from auto-vectorized memory operations on AArch64.
- Performance regressions where a struct-layout change flips vectorization off with no diagnostic.

A developer who *needs* the speedup has no recourse but to drop to raw intrinsics and hand-write the vector loop — exactly the ceremony Ori's mission says the compiler should absorb.

### Ori can promise what C cannot

Ori's mission is explicit: *safety, performance, and ergonomics are the compiler's problem, not the developer's*, with **automatic optimization gated on provable correctness**. Vectorization is the canonical case. The language already hands the compiler the proofs that make it safe:

```ori
// Idiomatic Ori. No annotations, no intrinsics, no `unsafe`.
@scale (xs: [float], k: float) -> [float] =
    xs.iter().map(transform: x -> x * k).collect()
```

Because `xs` has value semantics (no aliased mutable view can exist), `map` is pure, and the element type is `Value`, the compiler can *prove* every lane is independent and lower this to a vector loop — deterministically, as a guarantee, with no `restrict` annotation and no runtime alias check. The same code in C requires `restrict` on the output pointer and still only *might* vectorize.

The thesis: **Ori leans on SIMD more than most compilers because it can prove more than most compilers.** The differentiator is the proof surface, not the intrinsic surface.

---

## Goals and Non-Goals

**Goals:**

- Define automatic vectorization as a **default-on, pure optimization** that never alters observable program behavior.
- Specify the **provability gate** — the language-level facts (value semantics, `Value` lane type, capability purity, statically-bounded trip count) that license vectorization, and make a satisfied gate a *guarantee*, not a heuristic.
- Lower auto-vectorized loops to the **same `Intrinsics` SIMD surface** the manual API already defines (one canonical vector lowering, two entry points: automatic and manual).
- Preserve **dual-execution parity**: the evaluator and LLVM backend produce identical observable results, achieved by treating vectorization as a scalar-equivalent transformation.
- Take an explicit, deterministic stance on **floating-point reduction reassociation**.

**Non-Goals:**

- Auto-**parallelization** across threads/cores (still explicitly not guaranteed; `parallel(...)` remains the concurrency path).
- Designing the manual intrinsic surface — that is the already-approved `Intrinsics` v1/v2 work this proposal sits on top of.
- Outer-loop / nested vectorization, gather-scatter on arbitrary index expressions, and auto-vectorization across capability-effecting calls — named here as deliberate boundaries, candidates for a follow-up.
- A user-facing scheduling DSL (Halide-style). Ori's promise is "it just vectorizes when provable," not "you hand-schedule the vector plan."

---

## Design

### 0. The three-layer model

This proposal adds the top layer to an existing two-layer stack:

```
Layer 3  Automatic vectorization      compiler proves safety, vectorizes by default   <-- THIS PROPOSAL
              |  lowers to
Layer 2  Intrinsics capability        uses Intrinsics { simd_add, Mask<$N>, ... }      (approved, manual escape hatch)
              |  backed by
Layer 1  std.bytes / stdlib            high-level SIMD-backed functions, no capability  (approved)
```

A developer writes ordinary Ori. The compiler proves the loop is data-parallel and emits the vector lowering. When the proof fails — or when the developer wants explicit control over width and instruction selection — they drop to Layer 2 `Intrinsics`, which is unchanged.

### 1. The provability gate

A loop (a `for`/`while`/`loop` over a sequence, or a fused iterator chain per the Iterator Performance proposal) is **eligible** for automatic vectorization when ALL of the following are proven at compile time:

| Condition | Proven from | Why it licenses vectorization |
|---|---|---|
| **No loop-carried dependence on the data** | Value semantics: each element is reassigned, not mutated in place; no aliased mutable view of the sequence exists | Lane *i* cannot observe a write from lane *j* |
| **Lane type is `Value`** | The `Value` trait (bitwise-copy, no ARC, no `Drop`, fixed size) | Element fits a SIMD lane; no per-element refcount or destructor to sequence |
| **Loop body is pure (or effect-uniform)** | No `uses` capability inside the body, OR only reads of loop-invariant captures | No side effect whose ordering across lanes is observable |
| **Trip count is statically analyzable** | `[T, max N]` capacity, range bounds, or `.len()` of a value-semantic sequence | Compiler can emit a vector body + scalar tail with a known split |
| **Body maps to vector ops** | Each operation in the body has a SIMD form in the `Intrinsics` validity table (Clause 20.8.4) | A lowering target exists for every lane operation |

When every condition holds, vectorization is **guaranteed** (the compiler MUST emit it, modulo the cost-model veto in §5). When any condition fails, the loop runs scalar — silently and correctly, never with a hard error.

This is the inversion of the C model: instead of *assuming* aliasing and bailing unless proven otherwise, Ori *assumes* independence because the type system already forbids the aliasing that would violate it.

### 2. What vectorizes

```ori
// Elementwise map over a Value type -> vector map. Always eligible.
@normalize (xs: [float]) -> [float] =
    let m = xs.iter().max();
    xs.iter().map(transform: x -> x / m).collect()

// Elementwise zip -> vector add. Eligible: both inputs value-semantic, body pure.
@add_vectors (a: [float], b: [float]) -> [float] =
    a.iter().zip(other: b).map(transform: (x, y) -> x + y).collect()

// Integer reduction -> vector reduction. Eligible: int reassociation is exact.
@sum (xs: [int]) -> int =
    xs.iter().fold(initial: 0, op: (acc, x) -> acc + x)

// Predicate count -> vector compare + mask popcount. Eligible.
@count_positive (xs: [int]) -> int =
    xs.iter().filter(predicate: x -> x > 0).count()
```

Each lowers to a `simd_*` vector body plus a scalar tail for the remainder lanes, reusing the exact register-allocation guarantee `intrinsics-v2` already specifies (`[T, max N]` classified Scalar in SIMD context: no heap, no RC).

### 3. What does not vectorize (and stays correct)

- Loops whose body calls a `uses`-effecting function (I/O, `Random`, `Clock`) — ordering across lanes would be observable.
- Bodies over non-`Value` element types (`str`, `[T]`, heap structures) — no lane representation, and ARC sequencing per element is observable.
- Loops with a genuine loop-carried dependence the value model does not eliminate (e.g. a running recurrence `x[i] = f(x[i-1])`).
- Data-dependent / unbounded trip counts with no analyzable tail split.

In every case the fallback is the ordinary scalar loop. **There is no `E`-code for "could not vectorize"** — non-vectorization is never an error, because vectorization is an optimization, not a semantic.

### 4. Floating-point determinism — the reassociation stance

This is the one place where "pure optimization" has teeth. SIMD reductions reassociate floating-point operations, and FP addition is not associative, so a vectorized `simd_sum` over `float` can produce a *different bit pattern* than the scalar left-fold. Under Ori's value-semantics observability model, a different result is an **observable difference** — which would break dual-execution parity and the pure-optimization guarantee.

The stance:

- **Integer and bitwise reductions** reassociate **freely** — the result is bit-identical regardless of lane order. Always vectorized when eligible.
- **Elementwise floating-point operations** (`map`, `zip`-`map`) vectorize freely — each lane computes the identical scalar result; no reassociation occurs.
- **Floating-point reductions that require reassociation** (`fold`/`sum`/`product` over `float`) are **NOT auto-vectorized by default** — doing so would change the observable result.
- A developer opts into FP-reassociated reductions explicitly via a `#fast_math` attribute on the function (or a narrower `.sum_fast()` / reassociable-fold form — exact spelling is an Unresolved Question). Inside that opt-in, FP reductions vectorize and the changed rounding is the *documented, requested* behavior.

This makes the determinism contract explicit rather than implicit: Ori never silently trades a different floating-point answer for speed.

### 5. The cost-model veto

A satisfied gate makes a loop *eligible*; a small backend cost model decides whether vectorizing actually helps (tiny known trip counts, scalar-cheaper bodies, or targets lacking the needed feature can veto). The veto is the single exception to "guaranteed when proven." To keep the guarantee meaningful and testable:

- The cost model is **conservative and documented** — it vetoes only when the scalar form is demonstrably not slower.
- A `#vectorize` attribute **forces** vectorization past the cost-model veto (still subject to the correctness gate — it can never force an unsafe vectorization).
- A `#no_vectorize` attribute **opts a function out** entirely (escape valve for measured regressions or bit-exact requirements).

### 6. Lowering and the dual-execution parity contract

Automatic vectorization is a transformation on canonical IR / ARC IR that produces the same `simd_*` operations the manual `Intrinsics` API lowers — there is **one** vector lowering in `ori_llvm`, reached by two paths.

The parity contract is the load-bearing invariant:

> Automatic vectorization MUST be observably indistinguishable from the scalar program. The evaluator (`ori_eval`) executes the **scalar** form of every auto-vectorized loop; the LLVM backend executes the vector form. For every program, the two MUST produce identical observable results.

Because the gate (§1) admits a loop only when lanes are independent, and because FP reassociation is excluded by default (§4), the vector and scalar forms are provably equal — so parity is satisfied *by construction*, not by case-by-case verification. The evaluator does not need a SIMD interpreter for auto-vectorization; it runs the scalar loop. (Manual `Intrinsics` calls are a separate surface and need their own eval support — that is the approved intrinsics work, not this proposal.)

This is the same shape the Iterator Performance proposal already relies on: copy elision and deforestation are guaranteed *because* they are observably transparent. Auto-vectorization joins that set under the same discipline.

---

## Drawbacks

- **Implementation surface.** The provability gate spans the canonicalizer / ARC analysis (dependence + purity proof) and `ori_llvm` (vector lowering, scalar tail, cost model). It also presumes the `Intrinsics` capability is implemented, which it currently is not in any backend — this proposal is blocked on that substrate.
- **A guarantee is a contract.** Promising "this vectorizes" means a regression that silently descalarizes an eligible loop is a *compiler bug*, not a missed optimization. That raises the testing bar: every guaranteed-eligible pattern needs a pin that fails if vectorization regresses.
- **Floating-point subtlety leaks to users.** The `#fast_math` / default-scalar-reduction split is the honest design, but it means a `float` `sum` is *not* vectorized by default, which a performance-focused user may find surprising. The alternative (silent reassociation) is worse, but the surprise is real and must be documented prominently.
- **Cost-model trust.** A "guarantee with a cost-model veto" is only as credible as the veto is conservative. If the cost model is too aggressive, the guarantee becomes hollow; the `#vectorize` force-attribute is the safety valve but shifts burden back to the user.

---

## Alternatives Considered

### Alternative 1: Intrinsics only — no automatic layer

Ship the approved `Intrinsics` capability and stop there; let users hand-write every vector loop. **Rejected:** this is precisely the ceremony the Ori mission says the compiler should absorb. Every other systems language already offers manual intrinsics; doing only that is not "leaning on SIMD more than most compilers," it is leaning on it exactly as much as everyone else.

### Alternative 2: Best-effort auto-vectorization (the LLVM/C model)

Rely on LLVM's existing auto-vectorizer with no language-level guarantee — vectorize when the backend happens to. **Rejected:** it throws away Ori's central advantage. Ori *can prove* non-aliasing the C frontend cannot, so settling for best-effort inherits all the fragility (silent descalarization, no promise) while using none of the proof surface that makes Ori different.

### Alternative 3: Explicit opt-in annotation (Julia `@simd` / Mojo `vectorize()`)

Vectorize only loops the developer annotates. **Rejected as the default**, retained as the `#vectorize` *force* path. Opt-in contradicts the mission ("the compiler's problem, not the developer's") and re-introduces ceremony. The provability gate already guarantees safety, so the safe default is on, not off. Annotation is reserved for overriding the cost model, not for granting permission.

### Alternative 4: A scheduling DSL (Halide-style)

Let users describe the vector schedule (tile, split, vectorize widths) separately from the algorithm. **Rejected:** enormous surface area for a general-purpose language, and it re-imposes the "developer balances performance" burden Ori rejects. Halide's model is right for image pipelines, wrong for Ori's "it just vectorizes" promise.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:** Automatic vectorization is a compile-time static-analysis + codegen transformation. It requires:

- Loop-carried-dependence and purity proofs over canonical/ARC IR (static analysis — compiler-only).
- Vector lowering, scalar-tail generation, and target feature selection in `ori_llvm` (codegen — compiler-only).
- A cost model in the backend.

None of this is expressible in pure Ori; it is intrinsic compiler machinery, matching the "Static analysis: YES requires compiler" and "new codegen: compiler" rows of the purity table.

**Missing features that would enable purity:** None applicable — the user-facing *manual* counterpart (the `Intrinsics` capability and `std.bytes`) already lives at the library/capability boundary and is approved. This proposal is the compiler-internal optimization layer above it, which by nature cannot be pure Ori.

**Recommendation:** Proceed as a compiler feature, explicitly blocked on the `Intrinsics` capability being implemented first (it is approved but currently unimplemented across `ori_types`, `ori_eval`, `ori_llvm`). Auto-vectorization lowers to that surface and cannot land before it exists.

---

## Spec & Grammar Impact

- **No grammar changes** for the default behavior — auto-vectorization is invisible at the syntax level (ordinary loops and iterator chains).
- **New attributes** `#vectorize`, `#no_vectorize`, `#fast_math` join the attribute grammar (companion to existing `#derive`/`#repr`/`#cfg` attributes; canonical attribute-order placement per Annex D).
- **New optimization-guarantee appendix** (or extension of the Iterator Performance "Compiler Optimizations" section): move vectorization from "Not Guaranteed" to "Guaranteed when the provability gate (this proposal §1) holds," with the gate conditions and the FP-reassociation stance specified normatively.
- **Clause 20.8.4 cross-reference:** auto-vectorization lowers to the `Intrinsics` SIMD surface; the validity table (T × N combinations) bounds which lane types/widths the automatic layer can target.
- **Dual-execution parity clause:** add auto-vectorization to the set of observably-transparent transformations the evaluator/LLVM parity invariant covers.

---

## Roadmap Impact

Blocked on the `Intrinsics` capability (v1 + v2) being implemented across the type checker, evaluator, and LLVM backend — currently approved but unbuilt. Implementation phases, once unblocked, decompose cleanly:

1. Dependence + purity + `Value`-lane proof in the canonicalizer/ARC analysis (the gate).
2. Vector lowering + scalar tail in `ori_llvm`, reusing the `Intrinsics` lowering.
3. Cost model + `#vectorize`/`#no_vectorize`/`#fast_math` attributes.
4. Dual-execution parity test corpus (every guaranteed-eligible pattern gets a positive pin + a negative "stays scalar / stays correct" pin).

Phase decomposition is for `/create-plan` at approval time, not this proposal.

---

## Prior Art

| Language / system | Vectorization model | What aliasing/purity buys it |
|---|---|---|
| **C / C++** (LLVM/GCC) | Best-effort auto-vectorizer + runtime alias checks; `restrict` to promise non-aliasing | Without `restrict`, the compiler must prove or guard against aliasing; most loops fall back to scalar or pay for runtime checks. The aliasing it cannot disprove is the ceiling. |
| **Fortran** | Strong auto-vectorization; language forbids argument aliasing by default | The non-aliasing *language rule* is exactly why Fortran historically out-vectorized C for numeric kernels. Ori's value semantics reproduce this guarantee structurally rather than by fiat. |
| **ISPC** | SPMD-on-SIMD: program written per-lane, compiler maps lanes to vector | Explicit lane-parallel model; gets guaranteed vectorization by making the programmer write in lane terms. Ori instead infers lane-independence from value semantics. |
| **Julia** (`@simd`) | Opt-in macro that permits reassociation and asserts no loop-carried dependence | Pushes the *safety obligation onto the programmer* via the annotation. Ori moves that obligation to the type system and keeps the default on. |
| **Mojo** (`vectorize()`) | Explicit vectorize higher-order function over a parametric width | Manual, width-parametric; closest to Ori's Layer-2 `Intrinsics`. Ori adds the automatic Layer 3 above it. |
| **Rust** (LLVM) | Best-effort autovec; iterator chains *sometimes* vectorize | Even with the borrow checker forbidding aliased `&mut`, the proof does not reach LLVM as a vectorization license, so idiomatic chains (e.g. `take`+`sum`) are observed to miss vectorization. |
| **Swift** (LLVM) | Best-effort; field-sensitive analysis improvements over time | Same best-effort ceiling; vectorization quality tracks LLVM analysis, not a language guarantee. |

The cross-language reconnaissance (intelligence graph over rust/swift/zig/go/koka issue corpora) surfaced auto-vectorization as a recurring *fragility* theme — missing vectorization on idiomatic iterator reductions and target-specific autovec miscompiles — corroborating the "best-effort is unreliable" motivation. Those issue references are discovery signals; the normative claims above rest on established compiler-design facts (Fortran's no-alias rule, ISPC's SPMD model, Julia's `@simd` semantics), not on unverified issue numbers.

**Ori's distinguishing position:** Fortran gets strong vectorization from a *language rule* forbidding aliasing; Ori gets the same from value semantics, but extends it with capability-purity (effect-free bodies are provably reorderable) and the `Value` trait (a precise lane-eligibility predicate). The combination — guaranteed, default-on, proof-gated, with FP determinism preserved unless explicitly waived — is not offered by any of the systems above.

---

## Unresolved Questions

- **Reassociable-reduction spelling.** Is FP-reassociated reduction opted into via a function-level `#fast_math` attribute, a distinct method (`.sum_fast()` / `.fold_reassoc()`), or a per-reduction flag? (Resolve during review.)
- **Iterator-chain eligibility boundary.** Exactly which fused iterator shapes (per the Iterator Performance proposal) are guaranteed-eligible vs. cost-model-discretionary? `map`, `zip`+`map`, `filter`+`count`, integer `fold` are clearly in; where is the line for `scan`, `flat_map`, stateful adaptors? (Resolve during review.)
- **Cost-model observability.** Should the compiler offer a diagnostic / query (e.g. `ori explain --vectorization <fn>`) reporting whether a loop vectorized and, if not, which gate condition failed? Strong ergonomics argument; scope question for this proposal vs. a tooling follow-up. (Resolve during review.)
- **Width selection policy.** Default to the 128-bit portable baseline (matching `std.bytes`) and widen via `cpu_has_feature`, or target the widest available width per function? (Resolve during implementation.)
- **Reduction trees vs. lane accumulators.** For eligible integer reductions, the exact lowering shape (pairwise tree vs. N lane accumulators + horizontal sum) — bit-exact for integers either way, so an implementation-phase decision. (Resolve during implementation.)
