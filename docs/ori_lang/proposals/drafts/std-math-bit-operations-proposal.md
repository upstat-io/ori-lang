# Proposal: `std.math.bits` — Bit Operations as Pure Functions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 20, Annex E), stdlib (`library/std/math/`), capabilities, compiler (intrinsic recognition)
**Depends On:** logical-shift-operator-proposal.md (draft — supplies `>>>`), wrapping-shift-proposal.md (draft — supplies `wrapping_shl` / `wrapping_shr_logical`, without which the reference bodies panic). Implementation additionally consumes `wrapping_add` / `wrapping_sub` / `wrapping_mul`, approved in overflow-behavior-proposal.md and not yet shipped.
**Supersedes:** none
**Amends:** intrinsics-capability-proposal.md (approved — errata), intrinsics-v2-byte-simd-proposal.md (approved — errata; the GOVERNING declaration), auto-vectorization-proposal.md (approved — errata), representation-optimization-proposal.md (approved — errata)
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors), logical-shift-operator-proposal.md (sibling successor), wide-integer-literals-proposal.md (sibling successor), capability-unification-generics-proposal.md (approved — supplies the structural-versus-environmental criterion this proposal applies; it does **not** invent one), stdlib-math-api-proposal.md (draft — contests the `std.math` submodule manifest), limbs-trait-proposal.md (draft — three cases indicted by the approved criterion; reconciled below), stdlib-random-rng-proposal.md (draft — the motivating consumer), stdlib-json-native-parser-proposal.md (draft — a second `count_trailing_zeros` consumer), overflow-behavior-proposal.md (approved — supplies `wrapping_*`), comparable-hashable-traits-proposal.md (approved — `hash_combine` is the precedent for a pure-Ori bit-mixing function in the prelude)

---

## Summary

The `Intrinsics` capability bundles three unlike things: SIMD, `cpu_has_feature`, and five bit operations (`count_ones`, `count_leading_zeros`, `count_trailing_zeros`, `rotate_left`, `rotate_right`). Applying the **already-approved** structural-versus-environmental criterion from `capability-unification-generics-proposal.md:203-210` shows the five fail every column of the `uses` side: they are not caller-determined, not caller-provided, and not meaningfully mockable. This proposal moves them into a new pure `std.math.bits` submodule and removes them from `Intrinsics` as a declared breaking change. It writes **no new normative criterion** — the criterion exists and is approved. The argument for keeping them compiler-*recognized* is performance, not expressibility.

---

## Motivation

### The argument is performance, not expressibility

All five operations are expressible in pure Ori once the shift primitives this proposal declares as dependencies exist. Two reference bodies, written so that every operation in them is total:

```ori
use std.math { wrapping_add, wrapping_sub, wrapping_mul, wrapping_shl, wrapping_shr_logical };

// Rotate: two shifts and an OR on the 64-bit pattern.
@rotate_left (value: int, amount: int) -> int = {
    let $k = amount & 63;

    wrapping_shl(a: value, b: k) | wrapping_shr_logical(a: value, b: 64 - k)
}

// Population count: classic SWAR, five masked folds.
@count_ones (value: int) -> int = {
    let $a = wrapping_sub(a: value, b: (value >>> 1) & 0x5555_5555_5555_5555);
    let $b = wrapping_add(a: a & 0x3333_3333_3333_3333, b: (a >>> 2) & 0x3333_3333_3333_3333);
    let $c = wrapping_add(a: b, b: b >>> 4) & 0x0F0F_0F0F_0F0F_0F0F;

    wrapping_mul(a: c, b: 0x0101_0101_0101_0101) >>> 56
}
```

Both bodies were checked against a reference implementation over the full argument classes they are pinned on, including `amount` of `0`, `64`, `65`, and `-1` for the rotate, and `0`, `-1`, `int.min`, `int.max`, and two thousand random values for the population count. Both agree with the reference on every case.

**Every operation in these bodies is total, and that is load-bearing.** An earlier revision of this proposal wrote them with plain `<<`, plain `-`, plain `+`, and a trailing `>>> 0`, and all three defects were real:

| Earlier form | Defect |
|---|---|
| `(value << k) \| (value >>> (64 - k) >>> 0)` | `value << k` panics on shift overflow whenever bit `63 - k` is set; `>>> (64 - k)` is a count of `64` at `k == 0`, which panics. `amount: 0` is on this proposal's own pin list |
| `>>> 0` | The **JavaScript** `uint32`-coercion idiom, transcribed into a language where `>>>` returns `int` and `x >>> 0 == x` for every `x` including negatives. A no-op with no stated purpose, published as executable reference specification |
| `value - ((value >>> 1) & 0x5555…)` and `(a & m) + ((a >>> 2) & m)` | plain `-` and `+` underflow and overflow at `int.min`, which is on this proposal's own pin list. SWAR requires wrapping arithmetic |

The withdrawn `pure-bit-operations-proposal.md:64-65` had already documented the `<<` blocker. Reinstating the refuted construction and building the central thesis on it was the proposal's most serious defect; both bodies above are the repair, and `wrapping-shift-proposal.md` exists to supply the primitives they need.

**The remaining three bodies are not written out here.** `count_leading_zeros`, `count_trailing_zeros`, and `rotate_right` are classic de Bruijn and SWAR library code and are expressible over the same primitives, but no verified body is published in this document, because publishing an unverified body as normative reference specification is exactly the defect above. They are supplied at implementation under the same totality obligation and the same pins.

### Why compiler recognition, then

`count_ones` lowers to a single `popcnt` instruction on x86-64 with SSE4.2 and to `CNT` plus a horizontal add on aarch64; `rotate_left` lowers to a single `rol`. The SWAR body above is roughly a dozen instructions plus the wrapping-arithmetic calls. Every systems language surveyed in Prior Art exposes these as compiler-recognized operations for that reason.

**The size of that gap is not asserted here.** An earlier revision claimed "a factor of ten or more on a hot loop" with no benchmark, citation, or derivation, while carrying a normative spec change, a breaking change to approved surface, three errata, and a multi-executor lowering obligation on that number. The claim is **withdrawn**. What is asserted is the mechanism — one instruction against a dozen-instruction body — and the measurement is a required deliverable: a `tests/benchmarks/` comparison of the recognized lowering against the reference body shall exist before the recognition work is accepted, and its result, not an estimate, is what justifies the recognition surface. The proposal's other arguments do not rest on it.

### The second argument is effect-honesty

A capability exists to make an ambient dependency or effect visible in a signature. `uses Intrinsics` tells a reader and the effect system that a function depends on ambient hardware. A seeded pseudo-random generator does not. Today it must either declare a capability it does not use, or be written in the compiler instead of in Ori.

The capability also propagates transitively: every caller of a pure algorithm that rotates a word inherits `uses Intrinsics`, which is the noise the capability system exists to prevent.

### The Problem in Practice

```ori
// Intended: a pure, capability-free seeded generator.
type Rng: Value, Eq = { s0: int, s1: int, s2: int, s3: int }

impl Rng {
    // `rotate_left` EXISTS but is `Intrinsics.rotate_left`, gated `uses Intrinsics`.
    // A `uses`-free method may not call it, so this method cannot be written today
    // without declaring a capability it does not use.
    @next (self) -> (int, Rng) = {
        let $result = rotate_left(value: wrapping_mul(a: self.s1, b: 5), amount: 7);
        let $advanced = Rng { s0: self.s1, s1: self.s2, s2: self.s3, s3: self.s0 };

        (result, advanced)
    }
}
```

### Consumers — corpus sweep

Searched `docs/ori_lang/proposals/drafts/`, `docs/ori_lang/proposals/approved/`, `docs/ori_lang/v2026/spec/`, `library/`, `compiler/`, and `tests/` for every reference to the five operations:

| Consumer | Reference | Needs |
|---|---|---|
| `drafts/stdlib-random-rng-proposal.md` | pins xoshiro256\*\*, whose `next` is `rotl(s[1] * 5, 7)` — structurally unspecifiable without a rotate — and declares "no capability" as an explicit goal | `rotate_left` |
| `drafts/stdlib-math-api-proposal.md:606` | hand-rolls a rotate as `(xorshifted >> rot) \| (xorshifted << ((-rot) & 31))`, the same panicking `<<` construction repaired above | `rotate_left` |
| `drafts/stdlib-json-native-parser-proposal.md` | SIMD bitmask scanning | `count_trailing_zeros`, alongside SIMD |
| `drafts/limbs-trait-proposal.md:322` | `a.leading_zeros()` — CLZ across limbs, spelled as a method | a leading-zeros operation |
| `approved/intrinsics-v2-byte-simd-proposal.md:324` | **a live call site in approved normative example code** — see Migration | `count_trailing_zeros` |
| `approved/intrinsics-capability-proposal.md:216-226` | the original declaration | — |
| `approved/intrinsics-v2-byte-simd-proposal.md:114-118` | the GOVERNING declaration | — |

The JSON-parser consumer is a partial data point only: it needs `count_trailing_zeros` *with* SIMD, so it does not by itself demonstrate independence from `Intrinsics`. The two RNG consumers do. `stdlib-math-api-proposal.md` was omitted from an earlier revision's table despite hand-rolling the exact operation this proposal delivers.

### When This Matters

Any pure algorithm over 64-bit values: PRNGs, hashing, checksums, bit-set iteration, fixed-point normalization, encoding and decoding, bignum limb inspection. Each is a natural stdlib citizen under the lean-core principle.

---

## Goals and Non-Goals

**Goals:**

- Land the five operations as pure, capability-free functions in a new `std.math.bits` submodule.
- Remove them from `Intrinsics` as a **declared breaking change** — one spelling, one implementation, no substitution divergence.
- Apply the **approved** structural-versus-environmental criterion to the `Intrinsics` membership and to every pending case in the corpus.
- Errata every approved proposal affected: both `Intrinsics` declarations, the live call site's owner, and the representation-optimization operation list.
- Guarantee intrinsic lowering: the pure functions are compiler-recognized and lower to `popcnt` / `rol` / `clz` / `ctz` where the target provides them.

**Non-Goals:**

- **Writing a new classification criterion into the spec.** One exists and is approved. See §1 — this is the largest change from an earlier revision of this document.
- **SIMD, `Mask<$N>`, and `cpu_has_feature`.** All stay `uses Intrinsics`, and §1 shows the approved criterion keeps them there without a carve-out.
- **Changing what any bit operation computes.** Semantics are unchanged; only the home and the gating change.
- **A widening 64x64 -> 128 multiply.** Requested by `limbs-trait-proposal.md`; classified below but not delivered here.
- **`byte` variants.** Left open; the motivating consumers need only `int`.
- **The `std.math` module's other contents.** `stdlib-math-api-proposal.md` owns them.
- **Defining `>>>`, wide literals, or the wrapping shifts.** Sibling proposals own those, and two of them are hard dependencies.

---

## Design

### 1. The classification criterion — approved, not invented

`capability-unification-generics-proposal.md` (**Approved 2026-02-20**) establishes the criterion at `:202-210`:

| | `:` structural | `uses` environmental |
|---|---|---|
| **Determined by** | The entity's shape (fields, structure) | The caller's context (environment) |
| **Who provides it** | The compiler (auto-derived) or the programmer (manual impl) | The caller (via `with...in` or `def impl`) |
| **Propagation** | None — local to the type | Through call chains (transitive) |
| **Mockable** | No (structural truth) | Yes (`with Http = Mock in`) |
| **Syntax position** | Type declarations, generic parameters, where clauses | Function signatures |
| **Example** | `type Point: Eq` / `T: Comparable` | `@fetch () uses Http` |

That proposal carries errata at `:10-18` superseding parts of its impl syntax; the distinction table is untouched by them.

An earlier revision of this document proposed **new normative Clause 20 text** stating that an operation shall be capability-gated *if and only if* it has an ambient dependency or observable effect. That text is **deleted, not repaired**, for two reasons:

- **It re-decided a settled question.** The criterion was approved five months earlier. Inventing a second one creates two sources of truth for the same decision, which is a worse outcome than the ambiguity it was meant to fix. This document cited the approved proposal zero times, and so did its withdrawn predecessor.
- **The invented rule was wrong where the approved one is right.** Reviewers showed that "total, deterministic, identical on every target, observes and modifies nothing outside its arguments" mandates de-gating `simd_add`, because no term in it names a ground for keeping SIMD gated — the emulated fallback produces the same results (`20-capabilities.md:447`), lane count is a caller-supplied const generic, and the rule's own table answered "Ambient effect? No". With `if and only if`, that was binding, and it would have removed the normative basis for the SIMD gate the Non-Goals promise to preserve.

**The approved criterion supplies the ground the invented one lacked: mockability.** Applying it:

| Member | Determined by | Who provides it | Mockable | Verdict |
|---|---|---|---|---|
| SIMD (`simd_add`, `simd_shuffle`, …) | the caller's context | **the caller** — `20-capabilities.md:447` documents a `def impl Intrinsics` plus an `EmulatedIntrinsics` provider; `intrinsics-capability-proposal.md:137` shows `with Intrinsics = EmulatedIntrinsics {} in` | **Yes** — that substitution is the "Mockable: Yes" row verbatim | **capability** |
| `Mask<$N>` methods (`20-capabilities.md:427-437`: `bits`, `any`, `all`, `count`, `first_set`) | reached only through a caller-provided capability | the caller, transitively | Yes, with the provider | **capability** |
| `cpu_has_feature` | ambient machine state | the environment | Yes | **capability** |
| The five bit operations | the argument alone | nobody — there is no provider to substitute | **No** — a substituted `rotate_left` returning different bits is a broken implementation, not a legitimate environment | **plain function** |

SIMD stays gated because the caller provides it, **not** because lane width varies by target. That is the distinction the invented rule could not draw.

**`Mask<$N>` is resolved by the same row, and it is the sharpest test of the criterion.** `20-capabilities.md:433-437` gives `Mask.count()`, which *is* a population count, and `Mask.first_set()`, which *is* a count-trailing-zeros; `:451` records that `Mask.bits()` on aarch64 uses a polyfill. A criterion that made `count_ones(value:)` pure while leaving `Mask.count()` gated on any ground other than a principled one would not be a rule. The approved criterion supplies it: `Mask` values are produced by and consumed through the caller-provided capability, so they inherit its environmental classification. The operation is the same; the provenance is not, and provenance is what the approved table keys on.

**Marker capabilities are not a counterexample.** `Suspend` and `Unsafe` (`20-capabilities.md:284-285`) are non-bindable markers gating on permission and context rather than argument-determinism. They would have been a problem for the invented rule, which keyed on determinism; they are not a problem for the approved one, which keys on caller-provision, and they are listed here because an earlier review raised them against the deleted text.

**The one pending case elsewhere in the corpus:**

| Pending case | Source | Verdict under the approved criterion |
|---|---|---|
| `widening_mul (a: int, b: int) -> (int, int)` | `drafts/limbs-trait-proposal.md:333-341` requests it INTO `Intrinsics` | **plain function.** Not caller-determined, not caller-provided, not mockable. It belongs beside the five, not in `Intrinsics` |

This proposal does not deliver `widening_mul`; it records where it lands, on approved ground rather than on ground this document would otherwise have had to establish.

### 2. The five functions

New submodule `library/std/math/bits.ori`, re-exported from `library/std/math/mod.ori`:

```ori
// std.math.bits — pure, no capability required
pub @rotate_left (value: int, amount: int) -> int
pub @rotate_right (value: int, amount: int) -> int
pub @count_leading_zeros (value: int) -> int
pub @count_trailing_zeros (value: int) -> int
pub @count_ones (value: int) -> int
```

Called with named arguments, which `14-expressions.md:163` requires for direct calls:

```ori
use std.math { rotate_left, count_ones };

@is_power_of_two (n: int) -> bool = n > 0 && count_ones(value: n) == 1;

@mix (x: int) -> int = rotate_left(value: x, amount: 7);
```

### 3. Semantics — pinned

Operating on the 64-bit two's-complement pattern of `int`:

| Function | Semantics |
|---|---|
| `count_ones(value:)` | Number of set bits. `count_ones(value: 0) == 0`; `count_ones(value: -1) == 64`; `count_ones(value: int.min) == 1` |
| `count_leading_zeros(value:)` | Number of zero bits above the highest set bit. `count_leading_zeros(value: 1) == 63`; **`count_leading_zeros(value: 0) == 64`**; `count_leading_zeros(value: -1) == 0` |
| `count_trailing_zeros(value:)` | Number of zero bits below the lowest set bit. `count_trailing_zeros(value: 1) == 0`; **`count_trailing_zeros(value: 0) == 64`**; `count_trailing_zeros(value: int.min) == 63` |
| `rotate_left(value:, amount:)` / `rotate_right(value:, amount:)` | Circular shift. The amount is normalized as `amount & 63`, total over every `int` amount including negative and `>= 64` |

The zero cases are called out because they are the classic underspecification: x86's `BSR` and `BSF` leave the destination undefined for a zero input, and LLVM's `ctlz` and `cttz` take an `is_zero_poison` flag. Ori pins them to `64`; the lowering emits the non-poison form.

Rotate's modulo-64 normalization is **already approved text** at `intrinsics-capability-proposal.md:326-330` ("Rotation amounts are taken modulo 64"; `Intrinsics.rotate_left(value: 1, amount: 65)  // Same as amount: 1`). It is cited, not re-legislated. The `amount & 63` spelling is that rule made exact for negative amounts, and `wrapping-shift-proposal.md` adopts the identical normalization for the shift functions the reference bodies use, so rotate and its constituent shifts stay on one rule.

### 4. Removal from `Intrinsics` — declared breaking change

The five methods are **removed** from the `Intrinsics` trait.

The alternative — retaining them as forwarders to `std.math.bits` — is rejected because it is not semantics-preserving. `Intrinsics` is substitutable (`20-capabilities.md:447`; `intrinsics-capability-proposal.md:137`). A substituted provider's `rotate_left` is observed by `Intrinsics.rotate_left(...)` callers and **not** by `std.math.bits.rotate_left` callers. Two spellings that diverge under substitution are two semantics, which is the SSOT violation the "thin forwarder" framing was meant to avoid.

Making them individually unoverridable is also rejected: no per-method unoverridability mechanism exists in Ori's capability system, and inventing one to preserve a spelling that should not exist is the wrong trade.

Removal keeps one spelling, one implementation, one semantics. Its blast radius is **one call site**, not zero; see Migration.

### 5. Name resolution — the narrow claim, verified

An earlier revision asserted that *"capability methods are invoked through the capability name, never bare"* and used it to dismiss a reported bare-name collision. **The universal claim is false and is withdrawn.**

Counter-evidence: `20-capabilities.md:56` declares `trait Print` and `:279` lists `Print` in the capability table; `:326-327` shows `print(msg: `[{Clock.now()}] {msg}`)` — `Clock.now()` qualified and `print(msg:)` **bare, in the same expression** — and `:467-471` records that programs may use `print` without declaring `uses Print`, licensed by its default `def impl`. `20-capabilities.md:447` establishes that **`Intrinsics` also has a default `def impl`**, which is the exact structural feature that would have to distinguish it from `Print`. So a universal "never bare" rule does not hold, and it does not hold for `Intrinsics` specifically on the grounds given.

What survives, and is verified, is narrower and sufficient:

- The five operations are spelled **qualified** at every occurrence in the corpus — `intrinsics-capability-proposal.md:233` (`Intrinsics.count_ones(value: n)`), `:329` (`Intrinsics.rotate_left(value: 1, amount: 65)`), `intrinsics-v2-byte-simd-proposal.md:324` (`Intrinsics.count_trailing_zeros(value: mask.bits())`). Searched `library/`, `tests/`, `compiler/`, `docs/ori_lang/v2026/spec/`, and both proposal directories.
- After the §4 removal the qualified form ceases to resolve at all, so no two callables with the same name coexist.

No new resolution rule is needed and none is proposed — but that conclusion now rests on the verified narrow claim rather than on a universal one that the corpus refutes. Recorded at this length because the over-broad version was asserted in a review as well as in the proposal, and the correction belongs where the error was made.

### 6. `std.math` submodule placement

`library/std/math/mod.ori` is a 47-line file that is 100% comments, beginning `// TODO: Implement mathematical functions`, sketching float-only functions. Nothing in `std.math` is implemented, and the approved `wrapping_*` functions this proposal depends on are themselves unshipped. This proposal therefore **creates** its landing zone rather than moving into an existing home; the withdrawn draft's "move them to `std.math`" framing was inaccurate.

`stdlib-math-api-proposal.md:672-698` lists a per-category submodule manifest for `std/math/mod.ori`, with rows for `constants`, `error`, `basic`, `rounding`, `power`, `log`, `trig`, `hyperbolic`, `float`, `stats`, `interp`, `compare`, `combinatorics`, and `random`. There is **no `bits` row**. This proposal adds one:

```ori
// std/math/mod.ori — added row
pub use "./bits" { rotate_left, rotate_right, count_leading_zeros, count_trailing_zeros, count_ones }
```

Placement: after `compare`, before `combinatorics`. Integer-domain siblings in that manifest (`gcd`, `lcm`, `factorial` under `basic`) establish that `std.math` is not float-only despite the stub's sketch.

An earlier revision described that manifest as **strict**. Nothing in the file says so — a search for `strict` and `exhaustive` returns no hits — and it is already incomplete, carrying no row for the approved `wrapping_*` / `saturating_*` / `checked_*` functions that `overflow-behavior-proposal.md:96-101,145-155` places in `std.math`. The "no `bits` row" observation is correct and sufficient on its own; the strictness claim is **withdrawn**.

**Three drafts claim overlapping territory in this one module, and they disagree.** `stdlib-math-api-proposal.md:698` names the RNG submodule `random`; `stdlib-random-rng-proposal.md` names it `std.math.rand`; on disk the stub is `library/std/math/rand/mod.ori`. The two drafts define **incompatible `Rng` types** — PCG-XSH-RR against a pinned xoshiro256\*\*. This proposal adopts `stdlib-math-api-proposal.md` as the manifest authority and `stdlib-random-rng-proposal.md` as a beneficiary, and it records that they disagree rather than treating the conflict as settled. Resolving it is those two drafts' obligation. Coordination obligation for this one: whichever of the manifest drafts is approved second adds the other's rows; this proposal declares the `bits` row as its own and touches nothing else in the manifest.

### 7. Intrinsic lowering — the performance mechanism

The five are **compiler-recognized**: the compiler lowers a call to the target's instruction where one exists, and to the reference body otherwise.

| Function | x86-64 | aarch64 | Fallback |
|---|---|---|---|
| `count_ones` | `popcnt` (SSE4.2) | `CNT` + horizontal add | SWAR body (§Motivation) |
| `count_leading_zeros` | `lzcnt` (BMI1) / `bsr` + correction | `CLZ` | de Bruijn / SWAR body |
| `count_trailing_zeros` | `tzcnt` (BMI1) / `bsf` + correction | `RBIT` + `CLZ` | de Bruijn body |
| `rotate_left` / `rotate_right` | `rol` / `ror` | `ROR` | two shifts + OR (§Motivation) |

Recognition is by canonical symbol identity registered in `ori_registry` — not by name-sniffing at a codegen site, which would scatter the knowledge across backends.

**The fallback lowering is a real promise and is now keepable.** An earlier revision made the reference bodies the fallback while those bodies panicked on their own pinned inputs, so any target without a native rotate would have shipped a rotate that traps. The repaired bodies are total, which is why `wrapping-shift-proposal.md` is a hard dependency rather than a convenience: without it the fallback promise must be withdrawn and the five become intrinsic-only with no pure-Ori path.

### 8. Representation optimization

`annex-e-system-considerations.md:27` permits a narrower machine representation. All five functions are width-sensitive: `count_leading_zeros(value: 1) == 63` at 64 bits and `31` at 32 bits.

**The semantic 64-bit width is normative, not the representation width, and that shall be stated as `shall` text.** `representation-optimization-proposal.md:96-118`'s as-if rule requires operation preservation and forbids a conforming program from distinguishing the representation, and its canonical table at `:126` fixes `int`'s contract as 64-bit two's complement with bitwise operations treating the value as 64 unsigned bits. But that operation list at `:104-110` does **not** enumerate bitwise or shift operations, so the general sentence is not sufficient on its own and the errata below closes it explicitly.

An earlier revision asserted this width pin as normative in the body while delivering it as a **NOTE** in the Spec Impact table. Under the ISO/IEC Directives style the spec follows, a NOTE is informative and shall not contain requirements, so the two halves contradicted each other. The Annex E edit lands as normative text.

### Error Handling

None of the five has an error condition:

- `count_*` are total over every `int`, with the zero cases pinned in §3.
- Rotates are total over every `amount` via `amount & 63`; no shift-overflow condition applies, because a rotate cannot lose information.
- The reference bodies are total over every argument, because every operation in them is (§Motivation).

The asymmetry with shifts — which panic on an out-of-range count while rotates silently wrap — is inherited from the approved rotate text and is deliberate: an out-of-range shift count loses the value, while an out-of-range rotate amount does not. Recorded because the asymmetry is undefended in the approved text.

---

## Drawbacks

- **A declared breaking change to approved surface with a live call site.** Removing five methods from `Intrinsics` breaks `approved/intrinsics-v2-byte-simd-proposal.md:324`, inside the prescribed body of `std.bytes.find_byte`. That is normative example code in an approved proposal, and its migration is specified rather than assumed away.
- **Three approved proposals need errata**, plus a fourth for the representation-optimization operation list. Patching only one `Intrinsics` declaration would leave the governing one stale, which is the defect that sank the withdrawn predecessor.
- **Two hard dependencies on unapproved drafts.** `>>>` and the wrapping shifts must both land before the reference bodies compile, and a third approved-but-unshipped dependency (`wrapping_*`) must be implemented. Nothing here ships alone.
- **The performance argument is unmeasured.** The recognition surface is justified by a mechanism, not a number, until the benchmark deliverable in §Motivation exists. A reviewer is entitled to treat the recognition half of this proposal as provisional on that measurement; the de-gating half does not depend on it.
- **An interaction with auto-vectorization that expands an approved analysis's admission set.** See the errata below; neither proposal previously analyzed it.
- **Compiler recognition is a maintenance surface.** Five symbols must stay registered and their lowerings correct across every executor. Registry drift silently costs the performance the proposal exists to deliver.
- **`std.math` does not exist**, and its manifest is contested by two other drafts that disagree with each other (§6).

---

## Alternatives Considered

### Alternative 1: Leave the five gated; let pure algorithms declare `uses Intrinsics`

Rejected. It makes a capability declare something untrue, and the untruth propagates transitively through every caller. Under the approved criterion the five fail every `uses` column, so the gate has no basis, not merely an inconvenient one.

### Alternative 2: Keep `Intrinsics` methods as thin forwarders

Rejected — not semantics-preserving. §4 gives the argument: `Intrinsics` is substitutable, so a substituted provider is observed by one spelling and not the other.

### Alternative 3: Specify the five methods unoverridable while retaining them on `Intrinsics`

Rejected. No per-method unoverridability mechanism exists. Inventing one — and the resolution, diagnostic, and handler-validation surface it implies — to preserve a spelling that should not exist is a poor trade against a removal whose measured blast radius is one documentary call site.

### Alternative 4: Write them as ordinary pure-Ori library functions with no compiler recognition

The reference bodies are correct and shippable once the dependencies land. Rejected as the delivery mechanism: it forfeits `popcnt` and `rol` on a hot path, and every systems language treats these as compiler-recognized for that reason. The bodies remain valuable as executable specifications and as the fallback lowering (§7). If the §Motivation benchmark shows the gap is small, this alternative becomes the right answer and the recognition work should be dropped — which is what makes the measurement a gate rather than a formality.

### Alternative 5: Move only the rotates

`rotate_left` is the only operation the motivating consumer requires; the other four share the classification. Rejected as a scope reduction: under the approved criterion all five land identically, so leaving four behind would make `Intrinsics` membership arbitrary again.

### Alternative 6: A new `Bits` capability for the five

Rejected. It renames the problem. Under the approved criterion a `Bits` capability would be as unfounded as the current `Intrinsics` gating — not caller-determined, not caller-provided, not mockable — and it would still propagate transitively.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY — and the distinction is the proposal's substance.

- **Semantically**: YES, conditionally. Given `wrapping_*` (approved), `>>>` (sibling draft), and the wrapping shifts (sibling draft), all five are writable in pure Ori. Two verified bodies appear in Motivation; three are deferred rather than published unverified.
- **As delivered**: NO. Compiler recognition for intrinsic lowering is compiler surface, and removing methods from a capability trait is capability surface. Neither is expressible in a library.

**If not, why:**

- Intrinsic lowering to `popcnt` / `rol` / `clz` / `ctz` requires the compiler to recognize the canonical symbols.
- Removing five methods from `Intrinsics` changes the capability's declared surface, which lives in the spec.

**Missing features that would enable purity:** `wrapping_add` / `wrapping_sub` / `wrapping_mul` (approved, unshipped); the `>>>` operator; `wrapping_shl` / `wrapping_shr_logical`. With all of them, the reference bodies are complete pure Ori — which is why the argument for compiler involvement is performance, not expressibility. `hash_combine` (`comparable-hashable-traits-proposal.md:228-234`) is the precedent: a bit-mixing function written in pure Ori using `<<` and `>>`.

An earlier revision framed this as "Can be pure Ori? NO". That was overstated and is retracted; the honest framing is the one above, and it is the framing that makes the performance measurement a gate.

**Recommendation:** Proceed as a capability-surface and spec change plus one new stdlib submodule, sequenced after its two draft dependencies. The pure-Ori bodies ship as the reference specification and the fallback lowering.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| Clause 20 (`20-capabilities.md:403-408`) | **Remove** the five bit-operation method declarations from the `Intrinsics` trait. SIMD (`:390-401`), `Mask<$N>` (`:427-437`), and `cpu_has_feature` (`:410-411`) unchanged |
| Clause 20 | **No new normative criterion.** A NOTE may cite `capability-unification-generics-proposal.md:202-210` and record the `Intrinsics` partition as a worked application of it. An earlier revision proposed new normative `if and only if` text; that is deleted (§1) |
| Annex E (`annex-e-system-considerations.md`) | **Normative `shall` text** — the five operate on the canonical 64-bit semantic width, not the representation width (§8). Not a NOTE |
| `library/std/math/bits.ori` | NEW — the five functions |
| `library/std/math/mod.ori` | Add the `pub use "./bits" { ... }` row; the file is currently a 47-line all-comment TODO stub |
| `std.math` module doc (`docs/ori_lang/v2026/modules/`) | Document the five, including the pinned zero cases |
| **No grammar change** | Plain function declarations and calls; no new production |

### Errata — four approved proposals

`intrinsics-v2-byte-simd-proposal.md` is the **governing** version: `intrinsics-capability-proposal.md:433-435` carries an errata block marking itself superseded by it. Patching only v1 leaves the live declaration stale.

| Approved proposal | Erratum |
|---|---|
| `intrinsics-capability-proposal.md` | Narrows its Operations block (`:212-226`) and its bit-operation-safety text (`:326-330`): the five move to `std.math.bits`; the rotate modulo-64 rule survives verbatim as the `std.math.bits` rule |
| `intrinsics-v2-byte-simd-proposal.md` | Removes the five from its trait declaration (`:114-118`); SIMD, `Mask<$N>`, and `cpu_has_feature` unchanged. **Also amends the `std.bytes.find_byte` body at `:324`** — see Migration |
| `auto-vectorization-proposal.md` | Records two interactions this proposal creates, analyzed in neither document before. **(a)** Its provability gate at `:105` admits a loop only when the body is pure or effect-uniform, so removing the five **expands** the auto-vectorizer's admission set: loops that were excluded solely because a rotate or a population count carried `uses Intrinsics` become admissible. **(b)** Its lowering condition at `:108` requires each body operation to have a SIMD form in the Clause 20.8.4 validity table, and there is no `simd_popcount` or `simd_ctlz` there, so a newly-admitted loop containing `count_ones` has no vector lowering target. The erratum shall state that condition (b) is the binding one: such a loop is admitted by (a) and then rejected by (b), which is the correct outcome and not a gap, but it was previously reachable only by accident of the capability gate |
| `representation-optimization-proposal.md` | Records that its preserved-operation list (`:104-110`) covers bitwise and shift operations, so the semantic 64-bit width governs all five (§8) |

`auto-vectorization-proposal.md:258` cites the `std.bytes` module (not `find_byte` specifically) as approved user-facing surface built on `Intrinsics`, and `:219,260` independently record that `Intrinsics` is unimplemented across `ori_types`, `ori_eval`, and `ori_llvm` — which corroborates the zero-implementation-call-sites finding in Migration.

### Conformance pins

Searched `tests/spec/**`, `library/`, and `compiler/` for call sites and behavior pins of the five: **none found**. `Intrinsics` is unimplemented, `library/std/math/` contains only `mod.ori` (a TODO stub) and a `rand/` stub, and no spec test exercises the operations.

New pins required:

- Each function's value on `0`, `1`, `-1`, `int.min`, `int.max`; rotate at amounts `0`, `1`, `63`, `64`, `65`, `-1`; the pinned `count_*(0) == 64` cases; `count_ones(value: int.min) == 1`; `rotate_left` then `rotate_right` round-trip at every amount class.
- **The reference bodies pinned against the recognized lowering** on every case above, which is what makes §7's fallback promise checkable and what would have caught the panicking bodies an earlier revision published.
- A negative pin asserting that `Intrinsics.rotate_left(...)` no longer resolves after removal.
- A `tests/benchmarks/` measurement of the recognized lowering against the reference body — the deliverable §Motivation makes a gate, not merely a regression guard.

**Executor coverage.** An earlier revision promised bit-identical results across evaluator, VM, LLVM/native, compiled WebAssembly, and JIT in §7 while demanding only evaluator-and-LLVM parity in its pins. The pins are corrected to match the promise:

| Executor | Obligation |
|---|---|
| Evaluator | pinned — the semantic oracle |
| VM (`compiler/ori_vm`) | pinned — parity with the evaluator on every case above |
| LLVM / native | pinned — parity, including the recognized-lowering path and the fallback path |
| Compiled WebAssembly | parity where the target is admitted; declared `N/A` with a stated reason otherwise |
| JIT | same disposition as compiled WebAssembly |

A resource-and-leak check is declared `N/A` with a reason: all five are total functions over `int`, a `Value` type with no heap component, so no executor allocates, retains, or frees anything on their behalf. Stating the `N/A` and its ground is the obligation; leaving the leg unmentioned, as an earlier revision did, is not.

---

## Prior Art

| Language | Home for these operations | Capability / gate |
|---|---|---|
| Rust | inherent methods on integer primitives — `i64::count_ones`, `leading_zeros`, `trailing_zeros`, `rotate_left`, `rotate_right` | none: stable, no feature gate, no import |
| Go | `math/bits` package — `OnesCount64`, `LeadingZeros64`, `TrailingZeros64`, `RotateLeft64` | none: an ordinary stdlib package the compiler recognizes and lowers to instructions |
| Swift | properties and methods on `FixedWidthInteger` — `nonzeroBitCount`, `leadingZeroBitCount`, `trailingZeroBitCount` | none |
| Zig | `@popCount`, `@clz`, `@ctz` builtins | none — builtins, not a capability or import |
| C++20 | `<bit>` header — `std::popcount`, `std::countl_zero`, `std::countr_zero`, `std::rotl`, `std::rotr` | none |
| Java | `Long.bitCount`, `numberOfLeadingZeros`, `rotateLeft` — static methods the JIT intrinsifies | none |

**Not one mainstream language gates these behind an effect, capability, or permission.** Every one treats them as ordinary callable surface that the compiler recognizes and lowers. Go's `math/bits` is the closest structural match to this proposal — a plain stdlib package, compiler-recognized, no gate — and is where the `std.math.bits` name comes from.

The grouping in Ori's `Intrinsics` reads as implementation locality — all five are compiler intrinsics, as are SIMD and `cpu_has_feature` — rather than a semantic category. `intrinsics-capability-proposal.md` gives no semantic rationale for placing bit operations alongside SIMD, which supports that reading.

**Grounding note.** The Rust, Go, Zig, and Swift rows are corpus-verifiable against the reference repositories available here. The Java and C++20 rows are from language-reference knowledge; no Java or C++ standard-library repository is present in the corpus searched, so they are recorded as **not independently verified**. An earlier revision presented the whole table without this distinction. The concluding absence claim — that no surveyed language gates these — is an absence claim over an external corpus and is held to the same standard: it is stated from language-reference knowledge for Java and C++20 and from the on-disk corpus for the other four.

No relevant issue-corpus entries surfaced. The issue corpus of reference language implementations was searched over popcount, rotate, and bit-intrinsic phrasings and returned nothing on-point, so no issue citations appear rather than approximate ones.

---

## Migration / Breaking Changes

**This proposal contains a declared breaking change with a live call site in approved surface.**

| Change | Blast radius | Migration |
|---|---|---|
| Five methods removed from the `Intrinsics` trait | **One call site**, in approved normative example code: `approved/intrinsics-v2-byte-simd-proposal.md:324`, `break Some(pos + Intrinsics.count_trailing_zeros(value: mask.bits()))`, inside the prescribed body of `std.bytes.find_byte` (`:311-330`), reached from the approved `std.bytes` API (`:287-301`) | see below |
| Ori-source call sites | **Zero.** Searched `library/`, `tests/`, `compiler/`, and `docs/ori_lang/v2026/spec/` for `Intrinsics.rotate_left` / `.rotate_right` / `.count_ones` / `.count_leading_zeros` / `.count_trailing_zeros`. `Intrinsics` is unimplemented, corroborated by `auto-vectorization-proposal.md:219,260` | none |
| Four approved proposals gain errata | documentary | per the Errata section |
| `library/std/math/mod.ori` gains its first real export | none — the file is an all-comment TODO stub | none |

**Migration for `std.bytes.find_byte`.** An earlier revision dismissed every hit as *"prose inside the two `Intrinsics` proposals themselves"*. That is wrong: `:324` is the sole specified body of an approved library function, and it is the one place the removal actually breaks something. Restating the radius as zero was the error; it is one.

The body's signature is `uses Intrinsics` (`:313`) because the rest of it calls `simd_splat` and `simd_cmpeq`, which stay gated. So the migration is not "drop `uses Intrinsics`" — the capability is still needed — but a change of spelling for one call, and the erratum shall say which:

- **Preferred**: `Mask.first_set()` (`20-capabilities.md:437`), which returns `Option<int>` and is already the capability-side operation for exactly this purpose. The line becomes a use of the mask's own API rather than a bit-twiddle on `mask.bits()`, and it stays inside the capability the function already declares.
- **Alternative**: `use std.math { count_trailing_zeros }` at the module level, then `count_trailing_zeros(value: mask.bits())` bare. Correct, and it mixes a capability-provided mask with a capability-free operation in one expression, which reads less clearly than the first option.

The erratum records the first as the migration and the second as available. Neither was considered in an earlier revision, which is why the call site read as documentary.

SIMD, `Mask<$N>`, and `cpu_has_feature` callers are otherwise unaffected. Any code declaring `uses Intrinsics` for SIMD keeps it.

---

## Roadmap Impact

Implementation touches `library/std/math/` (new submodule), `ori_registry` (canonical symbol identity for recognition), `ori_eval`, `ori_vm`, `ori_llvm` and any further admitted executor (lowering plus parity), Clause 20, Annex E, and four errata blocks. A feature plan scaffolded on approval owns the phase breakdown.

Sequencing is constrained: `logical-shift-operator-proposal.md` and `wrapping-shift-proposal.md` must be approved and implemented before the reference bodies compile, and the approved `wrapping_*` functions must be shipped. Until then the five could ship recognition-only with no fallback, which §7 explicitly declines.

The lowering-plus-parity phase is load-bearing: five operations across every admitted executor, with the pinned zero cases as the sharp edge and the fallback-versus-recognized parity pins as the guard.

---

## Unresolved Questions

- **`byte` variants.** `overflow-behavior-proposal.md` specifies `byte` variants of the wrapping functions. Whether `count_ones` and rotate want `byte` forms is left open; the motivating consumers need only `int`. If added, rotate on `byte` normalizes as `amount & 7`.
- **`widening_mul` delivery.** §1 classifies it as a plain function on approved ground, contradicting `limbs-trait-proposal.md:333-341`, which requests it in `Intrinsics`. This proposal fixes the classification and does not deliver the operation.
- **`limbs-trait` reconciliation is broader than `widening_mul`.** That draft has **three** cases indicted by the approved criterion, not one: `:126` `@add ... uses Intrinsics` and `:164` `@multiply ... uses Intrinsics` on derived impls declare the capability unconditionally, and the narrow-width branch of `@add` invokes only `wrapping_add` and `checked_add` — no `Intrinsics` member at all — while propagating the capability transitively through every caller. That is the untruth capabilities exist to prevent, and it is now indicted by **approved** text rather than by anything this proposal establishes. Reconciling it is that draft's obligation and is flagged here so it is not missed.
- **`leading_zeros` spelling.** `limbs-trait-proposal.md:322` spells it as a method on a value (`a.leading_zeros()`) while this proposal uses a free function. `overflow-behavior-proposal.md:251-261` favors free functions with named arguments over methods on integers. Free function is proposed on those grounds; a method form on `Limbs` types is a separate surface and does not conflict.
- **Diagnostic for the removed methods.** Whether `Intrinsics.rotate_left(...)` should produce a targeted "moved to `std.math`" diagnostic rather than a generic unknown-method error is an `ori_diagnostic` decision. A targeted message is preferred and would need its own friendly-content regression pin.
- **Recognition threshold.** Whether the compiler must recognize all five on every target or may fall back to the pure body per-operation per-target is an implementation policy; the semantic requirement is only bit-identical results across executors.
