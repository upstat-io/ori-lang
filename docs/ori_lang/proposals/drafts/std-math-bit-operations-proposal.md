# Proposal: `std.math.bits` — Bit Operations as Pure Functions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 20, Annex E), stdlib (`library/std/math/`), capabilities, compiler (intrinsic lowering)
**Depends On:** none at approval time. Implementation of the pure-Ori reference bodies consumes `wrapping_add` / `wrapping_mul` (approved in overflow-behavior-proposal.md, not yet shipped) and the logical shift `>>>` (logical-shift-operator-proposal.md, draft). Neither blocks approval: the five functions ship as intrinsic-lowered `std.math` functions, and the pure-Ori bodies are reference specifications, not the delivery mechanism.
**Supersedes:** none
**Amends:** intrinsics-capability-proposal.md (approved — errata), intrinsics-v2-byte-simd-proposal.md (approved — errata; the GOVERNING declaration)
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors), logical-shift-operator-proposal.md (sibling successor), wide-integer-literals-proposal.md (sibling successor), stdlib-math-api-proposal.md (draft — owns the `std.math` submodule manifest this proposal extends), limbs-trait-proposal.md (draft — requests `widening_mul` INTO `Intrinsics`, the opposite direction; reconciled below), stdlib-random-rng-proposal.md (draft — the motivating consumer), stdlib-json-native-parser-proposal.md (draft — a second `count_trailing_zeros` consumer), representation-optimization-proposal.md (approved — canonical `int` width and the as-if rule), overflow-behavior-proposal.md (approved — supplies `wrapping_*`), comparable-hashable-traits-proposal.md (approved — `hash_combine` is the precedent for a pure-Ori bit-mixing function in the prelude)

---

## Summary

The `Intrinsics` capability bundles three unlike things: SIMD (target-width-dependent), `cpu_has_feature` (queries ambient machine state), and five bit operations (`count_ones`, `count_leading_zeros`, `count_trailing_zeros`, `rotate_left`, `rotate_right`) that are platform-invariant and effect-free. This proposal moves the five into a new pure `std.math.bits` submodule, removes them from `Intrinsics` as a declared breaking change, and writes the governing criterion — *platform-invariant plus no ambient effect implies pure, not capability-gated* — as normative spec text. The argument is performance and effect-honesty, not expressibility: with wrapping arithmetic and a defined logical shift, all five are writable in pure Ori.

---

## Motivation

### The argument is performance, not expressibility

All five operations are expressible in pure Ori once `wrapping_*` (approved) and `>>>` (proposed) exist. Reference bodies:

```ori
use std.math { wrapping_mul };

// Rotate: two shifts and an OR, on the 64-bit pattern.
@rotate_left (value: int, amount: int) -> int = {
    let $k = amount & 63;

    (value << k) | (value >>> (64 - k) >>> 0)
}

// Population count: classic SWAR, five masked folds.
@count_ones (value: int) -> int = {
    let $a = value - ((value >>> 1) & 0x5555_5555_5555_5555);
    let $b = (a & 0x3333_3333_3333_3333) + ((a >>> 2) & 0x3333_3333_3333_3333);
    let $c = (b + (b >>> 4)) & 0x0F0F_0F0F_0F0F_0F0F;

    wrapping_mul(a: c, b: 0x0101_0101_0101_0101) >>> 56
}
```

These bodies compile to roughly a dozen instructions. `count_ones` lowers to a single `popcnt` instruction on x86-64 with SSE4.2 and to `CNT` plus a horizontal add on aarch64; `rotate_left` lowers to a single `rol`. The gap is a factor of ten or more on a hot loop, which is why every systems language exposes these as compiler-recognized operations rather than leaving them to library SWAR.

So the honest case for keeping them compiler-known is **performance**. The withdrawn draft's framing — "Can be pure Ori? NO" — was overstated and is retracted here.

### The second argument is effect-honesty

A capability exists to make an ambient dependency or effect visible in a signature. `uses Intrinsics` in a signature tells a reader and the effect system that the function depends on ambient hardware. A seeded pseudo-random generator does not. Today it must either declare a capability it does not use, or be written in the compiler instead of in Ori.

The capability also propagates transitively: every caller of a pure algorithm that rotates a word inherits `uses Intrinsics`, which is exactly the noise the capability system exists to prevent.

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

Searched `docs/ori_lang/proposals/drafts/`, `docs/ori_lang/proposals/approved/`, `docs/ori_lang/v2026/spec/`, `library/`, and `compiler/` for every reference to the five operations:

| Consumer | Reference | Needs |
|---|---|---|
| `drafts/stdlib-random-rng-proposal.md` | its pure `Rng` declares "no capability" as an explicit goal | `rotate_left` |
| `drafts/stdlib-json-native-parser-proposal.md:747` | SIMD bitmask scanning | `count_trailing_zeros`, alongside SIMD |
| `drafts/limbs-trait-proposal.md:322` | `a.leading_zeros()` — CLZ across limbs, spelled as a method | a leading-zeros operation |
| `approved/intrinsics-capability-proposal.md:216-226` | the original declaration | — |
| `approved/intrinsics-v2-byte-simd-proposal.md:113-118` | the GOVERNING declaration | — |

The JSON-parser consumer is a partial data point only: it needs `count_trailing_zeros` *with* SIMD, so it does not by itself demonstrate independence from `Intrinsics`. The RNG consumer does.

### When This Matters

Any pure algorithm over 64-bit values: PRNGs, hashing, checksums, bit-set iteration, fixed-point normalization, encoding and decoding, bignum limb inspection. Each is a natural stdlib citizen under the lean-core principle.

---

## Goals and Non-Goals

**Goals:**

- Land the five operations as pure, capability-free functions in a new `std.math.bits` submodule.
- Remove them from `Intrinsics` as a **declared breaking change** — one spelling, one implementation, no substitution divergence.
- Write the classification criterion as normative spec text, so future intrinsic additions are decided by rule rather than re-litigated case by case.
- Errata BOTH `intrinsics-capability-proposal.md` and `intrinsics-v2-byte-simd-proposal.md`, the latter being the governing declaration.
- Guarantee intrinsic lowering: the pure functions are compiler-recognized and lower to `popcnt` / `rol` / `clz` / `ctz` where the target provides them.

**Non-Goals:**

- **SIMD and `cpu_has_feature`.** Both stay `uses Intrinsics`. This proposal narrows nothing about them.
- **Changing what any bit operation computes.** Semantics are unchanged; only the home and the gating change.
- **A widening 64x64 -> 128 multiply.** Requested by `limbs-trait-proposal.md`; classified below but not delivered here.
- **`byte` variants.** Left open (see Unresolved Questions); the motivating consumers need only `int`.
- **The `std.math` module's other contents.** `stdlib-math-api-proposal.md` owns them.
- **Defining `>>>` or wide literals.** Sibling proposals own those.

---

## Design

### 1. The classification criterion — normative

Add to `20-capabilities.md` as normative text:

> An operation shall be gated by a capability if and only if it has an ambient dependency or an observable effect. An operation whose result is a total, deterministic function of its arguments, identical on every target, and which observes and modifies nothing outside its arguments, shall be a plain function and shall not require a capability.

Applying the criterion to the current `Intrinsics` membership:

| Group | Members | Platform-dependent? | Ambient effect? | Verdict |
|---|---|---|---|---|
| SIMD | `simd_add<T, $N>`, `simd_shuffle<$N>`, ... | Yes — availability and lane width vary by target | No, but width selection is target-visible | capability |
| Hardware query | `cpu_has_feature (feature: str) -> bool` | Yes | Yes — reads ambient machine state | capability |
| Bit operations | `count_ones`, `count_leading_zeros`, `count_trailing_zeros`, `rotate_left`, `rotate_right` | No — identical result on every target | No | **plain function** |

The criterion applied to the one pending case elsewhere in the corpus:

| Pending case | Source | Criterion verdict |
|---|---|---|
| `widening_mul (a: int, b: int) -> (int, int)` | `drafts/limbs-trait-proposal.md:339`, `:540`, `:551` — requests it INTO `Intrinsics` | **plain function.** Total, deterministic, identical on every target, no ambient effect. It belongs in `std.math.bits` beside the five, not in `Intrinsics`. |

Writing the criterion normatively is what makes this a decided question rather than a recurring one. This proposal does not deliver `widening_mul`; it fixes where it lands when someone does.

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
| `count_ones(value:)` | Number of set bits. `count_ones(value: 0) == 0`; `count_ones(value: -1) == 64`. |
| `count_leading_zeros(value:)` | Number of zero bits above the highest set bit. `count_leading_zeros(value: 1) == 63`; **`count_leading_zeros(value: 0) == 64`**; `count_leading_zeros(value: -1) == 0`. |
| `count_trailing_zeros(value:)` | Number of zero bits below the lowest set bit. `count_trailing_zeros(value: 1) == 0`; **`count_trailing_zeros(value: 0) == 64`**; `count_trailing_zeros(value: int.min) == 63`. |
| `rotate_left(value:, amount:)` / `rotate_right(value:, amount:)` | Circular shift. The amount is normalized as `amount & 63`, which is total over every `int` amount including negative and `>= 64`. |

The zero cases are called out because they are the classic underspecification (x86's `BSR`/`BSF` leave the destination undefined for a zero input, and LLVM's `ctlz`/`cttz` take an `is_zero_poison` flag). Ori pins them to `64`; the lowering emits the non-poison form.

Rotate's modulo-64 normalization is **already approved text** at `intrinsics-capability-proposal.md:326-330` ("Rotation amounts are taken modulo 64"; `Intrinsics.rotate_left(value: 1, amount: 65)  // Same as amount: 1`). It is cited, not re-legislated. The `amount & 63` spelling is that rule made exact for negative amounts.

### 4. Removal from `Intrinsics` — declared breaking change

The five methods are **removed** from the `Intrinsics` trait. This is a breaking change and is declared as one.

The alternative — retaining them as forwarders to `std.math.bits` — is rejected because it is not semantics-preserving. `Intrinsics` is substitutable: `20-capabilities.md:447` documents a `def impl Intrinsics` plus an `EmulatedIntrinsics` provider, and `intrinsics-capability-proposal.md:137` shows `with Intrinsics = EmulatedIntrinsics {} in`. A substituted provider's `rotate_left` is observed by `Intrinsics.rotate_left(...)` callers and NOT by `std.math.bits.rotate_left` callers. Two spellings that diverge under substitution are two semantics, not one — the exact SSOT violation the "thin forwarder" framing was meant to avoid.

Making them individually unoverridable is also rejected: no per-method unoverridability mechanism exists in Ori's capability system, and inventing one to preserve a spelling that should not exist is the wrong trade.

Removal keeps one spelling, one implementation, one semantics.

### 5. Name resolution — no ambiguity exists

Reviewer feedback on the withdrawn draft asserted an unresolved bare-name collision: a function declaring `uses Intrinsics` while importing `use std.math { rotate_left }` would have two same-named, same-signature callables in scope with no resolution rule.

**Checked against the corpus; the premise is false.** Capability methods are invoked through the capability name, never bare:

- `20-capabilities.md:17` — `@fetch (url: str) -> Result<Response, Error> uses Http = Http.get(url);`
- `20-capabilities.md:327` — `Clock.now()`
- `intrinsics-capability-proposal.md:233` — `Intrinsics.count_ones(value: n) == 1`
- `intrinsics-capability-proposal.md:329` — `Intrinsics.rotate_left(value: 1, amount: 65)`

`Intrinsics.rotate_left(...)` and `rotate_left(...)` are lexically distinct call forms. No ambiguity is created, with or without the removal in §4; after the removal the qualified form ceases to resolve at all. No new resolution rule is needed, and none is proposed.

This is recorded so the concern is not re-raised at reviewer cost.

### 6. `std.math` submodule placement

`library/std/math/mod.ori` is a 47-line file that is 100% comments, beginning `// TODO: Implement mathematical functions`, sketching float-only functions. Nothing in `std.math` is implemented. This proposal therefore **creates** its landing zone rather than moving into an existing home; the withdrawn draft's "move them to `std.math`" framing was inaccurate and is corrected here.

`stdlib-math-api-proposal.md:672-698` declares a strict one-submodule-per-category manifest for `std/math/mod.ori`, with rows for `constants`, `error`, `basic`, `rounding`, `power`, `log`, `trig`, `hyperbolic`, `float`, `stats`, `interp`, `compare`, `combinatorics`, and `random`. There is **no `bits` row**. This proposal adds one:

```ori
// std/math/mod.ori — added row
pub use "./bits" { rotate_left, rotate_right, count_leading_zeros, count_trailing_zeros, count_ones }
```

Placement in the manifest: after `compare`, before `combinatorics`. Integer-domain siblings in that manifest (`gcd`, `lcm`, `factorial` under `basic`) establish that `std.math` is not float-only despite the stub's sketch.

Coordination obligation: `stdlib-math-api-proposal.md` is a draft. Whichever of the two is approved second adds the other's rows. This proposal declares the `bits` row as its own; it does not otherwise touch that manifest.

### 7. Intrinsic lowering — the performance guarantee

The five are **compiler-recognized**: the compiler lowers a call to the target's instruction where one exists, and to the reference body otherwise.

| Function | x86-64 | aarch64 | Fallback |
|---|---|---|---|
| `count_ones` | `popcnt` (SSE4.2) | `CNT` + horizontal add | SWAR body |
| `count_leading_zeros` | `lzcnt` (BMI1) / `bsr` + correction | `CLZ` | de Bruijn / SWAR body |
| `count_trailing_zeros` | `tzcnt` (BMI1) / `bsf` + correction | `RBIT` + `CLZ` | de Bruijn body |
| `rotate_left` / `rotate_right` | `rol` / `ror` | `ROR` | two shifts + OR |

The recognition is by canonical symbol identity, registered in `ori_registry` — not by name-sniffing at a codegen site, which would be `LEAK:scattered-knowledge`. Every executor (evaluator, VM, LLVM/native, compiled WebAssembly, JIT) produces bit-identical results; the lowering choice is a physical projection, never a semantic one.

A performance regression pin belongs in `tests/benchmarks/`: a `count_ones` hot loop must not regress against the intrinsic-lowered baseline.

### 8. Representation optimization

`annex-e-system-considerations.md:27` permits a narrower machine representation. All five functions are width-sensitive: `count_leading_zeros(value: 1) == 63` at 64 bits and `31` at 32 bits.

The **semantic** 64-bit width is normative, not the representation width. `representation-optimization-proposal.md:96-118`'s as-if rule already requires operation preservation and forbids any conforming program from distinguishing the representation, and its canonical table at `:126` fixes `int`'s contract as 64-bit two's complement with bitwise operations treating the value as 64 unsigned bits. These five are bitwise operations under that sentence. The errata below records them explicitly so a narrowing pass cannot read the omission as license: the operation list at `representation-optimization-proposal.md:104-110` does not currently enumerate bitwise or shift operations.

### Error Handling

None of the five has an error condition:

- `count_*` are total over every `int`, with the zero cases pinned in §3.
- Rotates are total over every `amount` via `amount & 63`; no shift-overflow condition applies, because a rotate cannot lose information.

The asymmetry with shifts — which panic on an out-of-range count while rotates silently wrap — is inherited from the approved rotate text and is deliberate: an out-of-range shift count loses the value, while an out-of-range rotate amount does not. Recorded here because the asymmetry is undefended in the approved text.

---

## Drawbacks

- **A declared breaking change to approved surface.** Removing five methods from `Intrinsics` breaks any code calling `Intrinsics.rotate_left(...)`. Mitigating: a corpus search for call sites (`library/`, `tests/`, `compiler/`) found **zero** — `Intrinsics` is unimplemented, so the breakage is documentary. The governance cost of erratum-ing two approved proposals is real regardless.
- **Two approved proposals need errata.** `intrinsics-capability-proposal.md` and its successor `intrinsics-v2-byte-simd-proposal.md` both declare the five. Patching only one leaves the governing declaration stale, which is exactly the defect that sank the withdrawn draft.
- **A new normative classification rule constrains future design.** Once "platform-invariant plus no ambient effect implies pure" is normative, an intrinsic that genuinely wants capability gating for a reason outside the criterion needs the criterion amended. That is the intended effect and also its cost.
- **Compiler recognition is a maintenance surface.** Five symbols must stay registered and their lowerings correct across every executor. A registry drift silently costs the performance the proposal exists to deliver — hence the benchmark pin in §7.
- **`std.math` does not exist.** This proposal creates the first real content in a module whose manifest is itself a draft. Ordering between the two drafts is a coordination cost.

---

## Alternatives Considered

### Alternative 1: Leave the five gated; let pure algorithms declare `uses Intrinsics`

Rejected. It makes a capability declare something untrue, and the untruth propagates transitively through every caller. That is precisely the noise the capability system exists to prevent.

### Alternative 2: Keep `Intrinsics` methods as thin forwarders

Rejected — not semantics-preserving. §4 gives the argument: `Intrinsics` is substitutable (`20-capabilities.md:447`; `intrinsics-capability-proposal.md:137`), so a substituted provider is observed by one spelling and not the other. Two spellings that diverge under substitution are two semantics.

### Alternative 3: Specify the five methods unoverridable while retaining them on `Intrinsics`

Rejected. No per-method unoverridability mechanism exists in Ori's capability system. Inventing one — and the resolution, diagnostic, and handler-validation surface it implies — to preserve a spelling that should not exist is a poor trade against a removal whose measured blast radius is zero call sites.

### Alternative 4: Write them as ordinary pure-Ori library functions with no compiler recognition

The reference bodies in Motivation are correct and shippable once `wrapping_*` and `>>>` land. Rejected as the delivery mechanism: it forfeits `popcnt` / `rol` on a hot path, and every systems language treats these as compiler-recognized for exactly that reason. The bodies remain valuable as executable specifications and as the fallback lowering.

### Alternative 5: Move only the rotates

`rotate_left` is the only operation the motivating consumer requires; the other four are moved because they share the purity argument. Rejected as a scope reduction: the four `count_*` operations satisfy the §1 criterion identically, so leaving them behind would make `Intrinsics` membership arbitrary again the moment the criterion is written down. A rule that its author does not apply consistently is not a rule.

### Alternative 6: A new `Bits` capability for the five

Rejected. It renames the problem. A capability with no ambient dependency and no effect carries no information; it is signature noise that still propagates transitively.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY — and the distinction is the proposal's substance.

- **Semantically**: YES. Given `wrapping_*` (approved) and `>>>` (proposed), all five are writable in pure Ori. The Motivation section gives working bodies for two of them; the remaining three are classic de Bruijn / SWAR library code.
- **As delivered**: NO. Compiler recognition for intrinsic lowering is compiler surface, and removing methods from a capability trait is capability surface. Neither is expressible in a library.

**If not, why:**

- Intrinsic lowering to `popcnt` / `rol` / `clz` / `ctz` requires the compiler to recognize the canonical symbols.
- Removing five methods from `Intrinsics` changes the capability's declared surface, which lives in the spec.
- The §1 classification criterion is normative spec text.

**Missing features that would enable purity:** `wrapping_add` / `wrapping_mul` (approved, unshipped) and the `>>>` operator (sibling draft). With both, the reference bodies are complete pure Ori — which is why the argument for compiler involvement is performance, not expressibility. `hash_combine` (`comparable-hashable-traits-proposal.md:228-234`) is the precedent: a bit-mixing function written in pure Ori using `<<` and `>>`.

**Recommendation:** Proceed as a minimal capability-surface and spec change, plus one new stdlib submodule. The pure-Ori bodies ship as the reference specification and the fallback lowering.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| Clause 20 (`20-capabilities.md:403-408`) | **Remove** the five bit-operation method declarations from the `Intrinsics` trait. SIMD (`:390-401`) and `cpu_has_feature` (`:410-411`) unchanged |
| Clause 20 (new normative paragraph) | The §1 classification criterion, plus a NOTE citing the three-way `Intrinsics` partition as the worked example |
| Annex E (`annex-e-system-considerations.md`) | NOTE recording that the five bit operations are `std.math.bits` functions operating on the canonical 64-bit semantic width, not the representation width |
| `library/std/math/bits.ori` | NEW — the five functions |
| `library/std/math/mod.ori` | Add the `pub use "./bits" { ... }` row; the file is currently a 47-line all-comment TODO stub |
| `std.math` module doc (`docs/ori_lang/v2026/modules/`) | Document the five, including the pinned zero cases |
| **No grammar change** | Plain function declarations and calls; no new production |

### Errata — BOTH declarations

`intrinsics-v2-byte-simd-proposal.md` is the **governing** version: `intrinsics-capability-proposal.md:433-435` carries an errata block marking itself superseded by it. Patching only v1 leaves the live declaration stale.

- `approved/intrinsics-capability-proposal.md` — errata narrowing its §Operations block (`:212-226`) and its bit-operation-safety text (`:326-330`): the five move to `std.math.bits`; the rotate modulo-64 rule survives verbatim as the `std.math.bits` rule.
- `approved/intrinsics-v2-byte-simd-proposal.md` — errata removing the five from its trait declaration (`:113-118`); SIMD and `cpu_has_feature` unchanged.
- `approved/representation-optimization-proposal.md` — errata recording that its preserved-operation list (`:104-110`) covers bitwise and shift operations, so the semantic 64-bit width governs all five.

### Conformance pins

Searched `tests/spec/**`, `library/`, and `compiler/` for call sites and behavior pins of the five: **none found**. `Intrinsics` is unimplemented, `library/std/math/` contains only `mod.ori` (a TODO stub) and a `rand/` stub, and no spec test exercises the operations.

New pins required: each function's value on `0`, `1`, `-1`, `int.min`, `int.max`; rotate at amounts `0`, `1`, `63`, `64`, `65`, `-1`; the pinned `count_*(0) == 64` cases; `rotate_left` then `rotate_right` round-trip at every amount class; evaluator and LLVM parity on all of the above; a negative pin asserting that `Intrinsics.rotate_left(...)` no longer resolves after removal; a `tests/benchmarks/` pin that `count_ones` in a hot loop does not regress against the intrinsic-lowered baseline.

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

**Not one mainstream language gates these behind an effect, capability, or permission.** Every one treats them as ordinary callable surface that the compiler recognizes and lowers. Go's `math/bits` is the closest structural match to this proposal: a plain stdlib package, compiler-recognized, no gate — which is also where the `std.math.bits` name comes from.

The grouping in Ori's `Intrinsics` reads as implementation locality — all five are compiler intrinsics, as are SIMD and `cpu_has_feature` — rather than a semantic category. `intrinsics-capability-proposal.md` gives no semantic rationale for placing bit operations alongside SIMD, which supports that reading.

No relevant issue-corpus entries surfaced. The issue corpus of reference language implementations was searched over popcount, rotate, and bit-intrinsic phrasings and returned nothing on-point, so no issue citations appear rather than approximate ones.

---

## Migration / Breaking Changes

**This proposal contains a declared breaking change.**

| Change | Blast radius | Migration |
|---|---|---|
| Five methods removed from the `Intrinsics` trait | **Zero call sites.** Searched `library/`, `tests/`, `compiler/`, and `docs/ori_lang/v2026/spec/` for `Intrinsics.rotate_left` / `.rotate_right` / `.count_ones` / `.count_leading_zeros` / `.count_trailing_zeros`; every hit is prose inside the two `Intrinsics` proposals themselves. `Intrinsics` is unimplemented | `Intrinsics.rotate_left(value: v, amount: k)` becomes `use std.math { rotate_left };` then `rotate_left(value: v, amount: k)`. Callers drop `uses Intrinsics` when it was declared solely for these |
| Two approved proposals gain errata | documentary | per the Errata section above |
| `library/std/math/mod.ori` gains its first real export | none — the file is an all-comment TODO stub | none |

SIMD and `cpu_has_feature` callers are unaffected. Any code declaring `uses Intrinsics` for SIMD keeps it.

---

## Roadmap Impact

Implementation touches `library/std/math/` (new submodule), `ori_registry` (canonical symbol identity for recognition), `ori_llvm` and every other admitted executor (lowering plus parity), Clause 20, Annex E, and three errata blocks. A feature plan scaffolded on approval owns the phase breakdown. The lowering-plus-parity phase is the load-bearing one: five operations across every executor, with the zero cases as the sharp edge.

---

## Unresolved Questions

- **`byte` variants.** `overflow-behavior-proposal.md` specifies `byte` variants of the wrapping functions. Whether `count_ones_byte` / rotate-on-`byte` are wanted is left open; the motivating consumers need only `int`. If added, `rotate` on `byte` normalizes as `amount & 7`.
- **`widening_mul` delivery.** §1's criterion classifies it as a plain function belonging in `std.math.bits`, contradicting `limbs-trait-proposal.md:339,551`, which requests it in `Intrinsics`. This proposal fixes the classification but does not deliver the operation; reconciling the limbs draft is that draft's obligation and is flagged here so it is not missed.
- **`leading_zeros` spelling.** `limbs-trait-proposal.md:322` spells it as a method on a value (`a.leading_zeros()`) while this proposal uses a free function (`count_leading_zeros(value:)`). `operator-method-naming-proposal.md` favors descriptive names; `overflow-behavior-proposal.md:254-259` favors free functions with named arguments over methods on integers. Free function is proposed on those grounds; a method form on `Limbs` types is a separate surface and does not conflict.
- **Diagnostic for the removed methods.** Whether `Intrinsics.rotate_left(...)` should produce a targeted "moved to `std.math`" diagnostic rather than a generic unknown-method error is an `ori_diagnostic` decision. A targeted message is preferred over a generic unknown-method error and would need its own friendly-content regression pin.
- **Recognition threshold.** Whether the compiler must recognize all five on every target or may fall back to the pure body per-operation per-target is an implementation policy; the semantic requirement is only bit-identical results.
