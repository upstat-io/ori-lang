# Proposal: Provability-Gated Automatic Vectorization

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-16
**Affects:** Compiler (canonicalizer, ARC/AIMS analysis, LLVM codegen), evaluator (dual-execution parity), spec (Clause 20.8.4 cross-reference, new optimization-guarantee appendix)
**Depends On:** intrinsics-capability-proposal.md, intrinsics-v2-byte-simd-proposal.md
**Amends:** iterator-performance-semantics-proposal.md

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
| **No loop-carried dependence — data AND control** | Value semantics on the *sequence* (each element reassigned, not mutated in place; no aliased mutable view) AND the body carries no loop-carried dependence through other state: no read-then-write of a captured mutable variable, no stateful iterator adaptor (`scan`, an accumulating `fold` body, index-carrying adaptors), no `x[i] = f(x[i-1])` recurrence | Lane *i* cannot observe a write produced by lane *j* — through the data **or** through body-external state |
| **Lane type is a *primitive* `Value`** | The element type is one of the SIMD-mappable primitives in the `Intrinsics` validity table — `int`, `float`, `byte` (each a `Value` with bitwise-copy, no ARC, no `Drop`). A multi-field `Value` *struct* (e.g. `Point: Value`) does NOT map to a single SIMD lane — vectorizing it would require a struct-of-arrays transform, which is a Non-Goal | Element fits one SIMD lane; no per-element refcount or destructor to sequence |
| **Loop body is pure (or effect-uniform)** | No `uses` capability inside the body, AND no mutation of any binding that outlives a single iteration; only reads of loop-invariant captures | No side effect whose ordering across lanes is observable |
| **Reduction operator is reorder-safe (reductions only)** | The combining operator is associative **and observably order-independent** — see §4. Element-wise maps/zips do not reduce and skip this row | A vector reduction reorders the combine; this is only sound when reordering changes no observable result (including panic behavior) |
| **Trip count is statically analyzable** | `[T, max N]` capacity, range bounds, or `.len()` of a value-semantic sequence | Compiler can emit a vector body + scalar tail with a known split |
| **Body maps to vector ops** | Each operation in the body has a SIMD form in the `Intrinsics` validity table (Clause 20.8.4) | A lowering target exists for every lane operation |

**Element-wise independence is NOT reduction associativity.** Conditions 1–3 establish that *lanes are independent* — sufficient for element-wise maps and zips. A *reduction* additionally reorders the combining operator across lanes, which conditions 1–3 do NOT license; the reorder-safe row (condition 4) is the separate, stricter gate reductions must also pass (§4 specifies it).

**Purity is NOT panic-freedom — element-wise panic ordering.** A body can be pure (no `uses` effect) yet still *panic*: checked-integer overflow, division by zero, an out-of-bounds index inside the lambda. Scalar iteration panics at the **lowest-index** element that panics, after computing all prior elements; a vector body computes a whole lane group at once and can surface a *different* lane's panic, or panic on a lane a short-circuiting scalar chain (`find` / `any` / `take_while`) would never have reached. Either changes the observable `PanicInfo` (which element, or whether a panic occurs at all). Therefore, for DEFAULT element-wise vectorization the body must additionally be **panic-free** — the compiler proves no element operation can panic (no checked-overflow, no div-by-zero, no fallible index). A panic-capable element body vectorizes only when (a) the lowering preserves lowest-index-first panic semantics (a masked / sequenced emission), or (b) the function carries `#fast_math`, which — as with reductions — accepts the reordered panic as documented, requested behavior. Panic-free bodies (the common case: `x -> x * k` over `float`, comparisons, bitwise ops) are unaffected and vectorize by default.

**The guarantee, precisely.** When the gate holds, the compiler MUST either (a) emit the vector lowering, or (b) record a cost-model veto (§5) that is queryable via `ori explain --vectorization <fn>`. "Eligible-but-vetoed" is never silent: every gate-satisfying loop is either vectorized or carries a machine-readable reason it was not. When the gate does NOT hold, the loop runs scalar — silently and correctly, never with a hard error (non-vectorization is an optimization outcome, not a diagnostic; there is no `E`-code for "could not vectorize").

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

// Reorder-safe reduction (max) -> vector reduction. Eligible: max is associative,
// total, and panic-free, so lane order changes nothing observable (§4).
@peak (xs: [int]) -> int =
    xs.iter().fold(initial: 0, op: (acc, x) -> max(acc, x))

// Predicate count -> vector compare + mask popcount. Eligible: lane count is order-free.
@count_positive (xs: [int]) -> int =
    xs.iter().filter(predicate: x -> x > 0).count()

// Checked-integer SUM is NOT eligible by default — reassociating partial sums can
// move an overflow panic (§4). Vectorizes only under #fast_math, or when the compiler
// proves no overflow (e.g. a bounded [byte, max N] sum into a wider accumulator).
@sum (xs: [int]) -> int =
    xs.iter().fold(initial: 0, op: (acc, x) -> acc + x)   // default-scalar
```

Each *eligible* loop lowers to a `simd_*` vector body plus a scalar tail for the remainder lanes, reusing the exact register-allocation guarantee `intrinsics-v2` already specifies (`[T, max N]` classified Scalar in SIMD context: no heap, no RC).

### 3. What does not vectorize (and stays correct)

- Loops whose body calls a `uses`-effecting function (I/O, `Random`, `Clock`) — ordering across lanes would be observable.
- Bodies over non-`Value` element types (`str`, `[T]`, heap structures) — no lane representation, and ARC sequencing per element is observable.
- Loops with a genuine loop-carried dependence the value model does not eliminate (e.g. a running recurrence `x[i] = f(x[i-1])`).
- Data-dependent / unbounded trip counts with no analyzable tail split.

In every case the fallback is the ordinary scalar loop. **There is no `E`-code for "could not vectorize"** — non-vectorization is never an error, because vectorization is an optimization, not a semantic.

### 4. Reduction reassociation and determinism — where "pure optimization" has teeth

A vector reduction reorders the combining operator across lanes. That reorder is sound only when it changes **no observable result** — and in Ori "observable" includes panics, not just values. Two operators that are mathematically associative are still not equally safe to reassociate:

**Floating-point arithmetic is not associative.** A vectorized `simd_sum` over `float` produces a different bit pattern than the scalar left-fold, so reassociating it is an observable change.

**Integer arithmetic reassociation is observable through overflow panics.** Ori panics on integer overflow (per spec — `int` add/mul are checked). Reassociating `(a + b) + c` into `a + (b + c)`, or summing lanes pairwise, changes *whether and where* an intermediate overflow panic fires — even when the final mathematical sum is identical. A panic is an observable result, so integer `+`/`*` reductions are **NOT** freely reassociable, contrary to a naive "integers are exact" intuition. (This is the trap: integer addition is associative over ℤ, but Ori's `int` is checked i64, and the check is observable.)

The stance — reorder-safe operators vs reorder-unsafe operators:

| Reduction operator | Reorder-safe by default? | Why |
|---|---|---|
| Bitwise `&` / `\|` / `^`, `min`, `max` over `int`/`byte` | **Yes** — vectorized when eligible | Associative AND total (no overflow, no panic, no rounding); lane order changes nothing observable |
| `min` / `max` over `float` | **Yes** — vectorized when eligible | Ori defines a *total* order on `float` (`NaN > all`, `+0.0`/`-0.0` ordered per the prelude `Comparable` contract), so float `min`/`max` is associative and order-independent — unlike float `+`/`*` |
| Integer `+` / `*` (checked) | **No** — default-scalar | Reassociation can change overflow-panic occurrence (observable) |
| Floating-point `+` / `*` | **No** — default-scalar | Not associative; reassociation changes the bit result (observable) |
| Element-wise `map` / `zip`-`map` (panic-free body, any primitive lane type) | **Yes** — vectorized when eligible | No reduction occurs; each lane computes the identical scalar result (panic-capable bodies are gated separately — see §1 element-wise panic ordering) |

**Lowering dependency — horizontal reduce.** A reorder-safe *reduction* needs a horizontal-combine primitive. The approved Intrinsics v2 surface provides element-wise `simd_min`/`simd_max`/`simd_and`/`simd_or`/`simd_xor` but only ONE horizontal reduce, `simd_sum`. Vectorizing a `min`/`max`/bitwise reduction therefore depends on a small Intrinsics extension — a horizontal `simd_reduce_min`/`simd_reduce_max`/`simd_reduce_and`/`…` family — that v2 does not yet define. This is folded into this proposal's Intrinsics-substrate dependency (see Roadmap Impact): the automatic layer cannot lower a min/max reduction until that horizontal primitive exists. Element-wise maps and `simd_sum`-backed reductions need no extension.

- **Reorder-unsafe reductions auto-vectorize ONLY with explicit opt-in** via the `#fast_math` attribute (§5) — OR when the compiler can *prove* the reduction cannot overflow (e.g. a bounded `[byte, max N]` sum into a wider accumulator), in which case the integer case becomes reorder-safe and needs no attribute.
- Element-wise vectorization and reorder-safe reductions are the always-on default; they need no attribute and change nothing observable.

**`#fast_math` is defined as a parity-preserving reassociation license, not a per-backend divergence.** When a function carries `#fast_math`, reorder-unsafe reductions in it MAY be reassociated — but **both** backends MUST adopt the **same** compiler-defined reduction order (e.g. a fixed lane-tree shape). The evaluator does NOT keep computing a precise scalar left-fold while LLVM reassociates — that asymmetry is exactly the parity break this attribute must avoid. Instead, `#fast_math` redefines the reduction's *specified* semantics to "combine in the compiler's reduction-tree order," and both `ori_eval` and `ori_llvm` implement that one order. The result is deterministic and identical across backends; it merely differs from the naive scalar left-fold — which is precisely what the developer opted into. `#fast_math` likewise accepts the reordered integer-overflow-panic behavior as documented, requested semantics.

This makes the determinism contract explicit rather than implicit: Ori never silently trades a different value — or a different panic — for speed, and the speed-for-precision trade, when taken, is taken identically in both backends.

### 5. The cost-model veto

A satisfied gate makes a loop *eligible*; a small backend cost model decides whether vectorizing actually helps (tiny known trip counts, scalar-cheaper bodies, or targets lacking the needed feature can veto). The veto is the single exception to "vectorized when the gate holds." To keep the guarantee meaningful, testable, and non-hollow:

- **The veto is diagnosable, never silent.** Every eligible-but-vetoed loop is reportable via `ori explain --vectorization <fn>`, which states that the loop passed the gate and the specific cost-model reason it was not vectorized (trip count below threshold, target lacks feature, scalar body cheaper). A guarantee whose exception is invisible is hollow; making the exception queryable is what keeps "guaranteed when profitable" a real, auditable contract rather than a backend's private discretion.
- The cost model is **conservative and documented** — it vetoes only when the scalar form is demonstrably not slower.
- A `#vectorize` attribute **forces** vectorization past the cost-model veto (still subject to the correctness gate — it can never force an unsafe vectorization).
- A `#no_vectorize` attribute **opts a function out** entirely (escape valve for measured regressions or bit-exact requirements).
- A `#fast_math` attribute licenses reorder-unsafe reductions per §4 (parity-preserving: both backends adopt the same reduction order).

These attributes use the `#name(...)` attribute grammar of the approved Simplified Attribute Syntax proposal.

### 6. Lowering and the dual-execution parity contract

Automatic vectorization is a transformation on canonical IR / ARC IR that produces the same `simd_*` operations the manual `Intrinsics` API lowers — there is **one** vector lowering in `ori_llvm`, reached by two paths.

The parity contract is the load-bearing invariant:

> Automatic vectorization MUST be observably indistinguishable from the program's *specified* semantics. For every program, `ori_eval` and `ori_llvm` MUST produce identical observable results — values AND panics.

Two regimes, both parity-safe by construction:

- **Default vectorization** (element-wise maps/zips + reorder-safe reductions per §4): the transformation does not change the combine order, so the evaluator runs the ordinary **scalar** loop and the LLVM backend runs the vector form, and the two are provably equal. The evaluator needs no SIMD interpreter here.
- **`#fast_math` reductions** (reorder-unsafe combine, §4): the *specified* reduction semantics become "combine in the compiler's defined reduction-tree order," and **both** backends implement that one order. Parity is preserved because eval does not stay on a divergent scalar left-fold — it adopts the same defined order LLVM does. The result is deterministic across backends; it differs only from a naive scalar fold, which is the documented `#fast_math` semantics.

In neither regime does eval compute one answer while LLVM computes another. Because the gate (§1) admits a loop only when lanes are independent, and reorder-unsafe reductions are excluded from the default and made backend-symmetric under `#fast_math`, the vector and scalar forms are provably equal — parity holds *by construction*, not by case-by-case verification. (Manual `Intrinsics` calls are a separate surface and need their own eval support — that is the approved intrinsics work, not this proposal.)

**Baseline assumption — stated explicitly.** Parity-by-construction inherits the *pre-existing* scalar-execution parity baseline: `ori_eval` and `ori_llvm` are already required to agree bit-for-bit on the scalar program's floating-point and checked-integer arithmetic (the existing dual-execution-parity invariant). This proposal does not establish that baseline; it relies on it. If the scalar baseline ever diverges between backends, that is a pre-existing parity bug to fix independently — auto-vectorization layered on top would inherit the divergence, not introduce it.

This is the same shape the Iterator Performance proposal already relies on: copy elision and deforestation are guaranteed *because* they are observably transparent. Auto-vectorization joins that set under the same discipline.

---

## Drawbacks

- **Implementation surface.** The provability gate spans the canonicalizer / ARC analysis (dependence + purity proof) and `ori_llvm` (vector lowering, scalar tail, cost model). It also presumes the `Intrinsics` capability is implemented, which it currently is not in any backend — this proposal is blocked on that substrate.
- **A guarantee is a contract.** Promising "this vectorizes" means a regression that silently descalarizes an eligible loop is a *compiler bug*, not a missed optimization. That raises the testing bar: every guaranteed-eligible pattern needs a pin that fails if vectorization regresses.
- **Reduction subtlety leaks to users.** The `#fast_math` / default-scalar-reduction split is the honest design, but it means neither a `float` `sum` nor a checked-`int` `sum` is vectorized by default (FP for non-associativity, integer for overflow-panic observability — §4), which a performance-focused user may find surprising given how parallel a sum *looks*. The alternative (silent reassociation that moves a rounding result or an overflow panic) is worse, but the surprise is real and must be documented prominently. Reorder-safe reductions (`min`/`max`/bitwise) and all element-wise maps *do* vectorize by default, which softens it.
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
- **New optimization-guarantee appendix** (or extension of the Iterator Performance "Compiler Optimizations" section): move vectorization from "Not Guaranteed" to "Guaranteed when the provability gate (this proposal §1) holds," with the gate conditions and the reduction-reassociation stance specified normatively.
- **Errata on `iterator-performance-semantics-proposal.md` (approved) — NOT a rewrite.** That proposal's "Not Guaranteed → Vectorization (SIMD) for numeric operations" line is superseded by this one. Under the standard proposal errata convention, approved proposals are never edited in place; approval of this proposal MUST add an `## Errata (added YYYY-MM-DD)` block to the iterator proposal pointing here, stating that vectorization is now a provability-gated guarantee. The `**Amends:**` header records the relationship; the errata block records it on the amended side.
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

Resolved during this review (recorded here for traceability, no longer open):

- **Reassociable-reduction spelling** → a function-level `#fast_math` attribute (§4/§5). A narrower per-reduction method (`.sum_fast()`) MAY be added later as sugar, but the attribute is the primitive.
- **Cost-model observability** → `ori explain --vectorization <fn>` reports gate-pass + veto reason (§1/§5). The veto is never silent; this is in-scope, not a follow-up.
- **Stateful-adaptor / captured-mutable boundary** → excluded by gate condition 1 (§1): `scan`, accumulating-`fold` bodies, index-carrying adaptors, and captured-mutable bodies carry loop-carried dependence and do not vectorize.

Still open:

- **Eligibility edge of fusion shapes.** `map`, `zip`+`map`, `filter`+`count`, reorder-safe `fold` (bitwise/`min`/`max`) are guaranteed-eligible. The precise line for `flat_map` (variable per-element fan-out) and `take_while`/`skip_while` (data-dependent termination) within a fused chain is the remaining boundary question. (Resolve during review.)
- **Provable-no-overflow promotion.** How aggressively does the compiler prove a checked-integer reduction cannot overflow (promoting it to reorder-safe without `#fast_math`)? The bounded-`[byte, max N]`-into-wider-accumulator case is clearly provable; the general case bounds the analysis effort. (Resolve during implementation.)
- **Width selection policy.** Default to the 128-bit portable baseline (matching `std.bytes`) and widen via `cpu_has_feature`, or target the widest available width per function? (Resolve during implementation.)
- **Reduction-tree shape under `#fast_math`.** The exact lane-tree order both backends adopt (pairwise tree vs. N lane accumulators + horizontal combine). Must be *one fixed order* both backends implement (§4/§6); which order is the implementation-phase choice. (Resolve during implementation.)
