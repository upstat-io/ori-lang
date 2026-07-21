# Proposal: Pure Bit Operations

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 7, Clause 14, Clause 20, Annex B, Annex E), stdlib, capabilities
**Depends On:** overflow-behavior-proposal.md (approved — supplies `wrapping_add` / `wrapping_sub` / `wrapping_mul`; this proposal consumes them and does not redefine them)
**Related:** intrinsics-capability-proposal.md (approved — currently owns the bit operations), stdlib-random-rng-proposal.md (draft — the motivating consumer)

---

## Summary

Make Ori's existing 64-bit bit-manipulation surface usable from pure, capability-free code. Three narrow changes: expose the five platform-invariant bit operations (`count_leading_zeros`, `count_trailing_zeros`, `count_ones`, `rotate_left`, `rotate_right`) outside the `Intrinsics` capability; pin `>>` as a logical shift at the operator definition, matching the rule Annex E already states; and provide a way to express 64-bit constants above `2^63 - 1`, following the precedent the spec already sets for `int.min`. No new operation is invented — every item makes an existing capability reachable, an existing rule explicit, or an existing idiom general.

---

## Motivation

A pure function that manipulates 64-bit values cannot be written in Ori today, even though every operation it needs already exists.

### The Problem in Practice

A seeded pseudo-random generator is the canonical case: pure, deterministic, no ambient effect, and entirely bit arithmetic.

```ori
// Intended: a pure, capability-free seeded generator.
type Rng: Value, Eq = { s0: int, s1: int, s2: int, s3: int }

impl Rng {
    @next (self) -> (int, Rng) = {
        // 1. Needs a rotate. `rotate_left` EXISTS but is `uses Intrinsics`,
        //    so a `uses`-free method may not call it.
        let result = rotate_left(value: wrapping_mul(a: self.s1, b: 5), amount: 7);

        // 2. Hand-rolling the rotate does not help: `<<` panics on overflow.
        //    `(x << k) | (x >> (64 - k))` traps whenever the shift sets bit 63.

        // 3. Needs a >> that fills with zeros. The spec never says whether
        //    `>>` is arithmetic or logical, so this is unspecified today.
        let mixed = (self.s0 ^ (self.s0 >> 30));

        // 4. Needs a 64-bit constant. This literal is a COMPILE ERROR:
        //    0xBF58476D1CE4E5B9 exceeds the 0 .. 2^63-1 literal range.
        let scrambled = wrapping_mul(a: mixed, b: 0xBF58476D1CE4E5B9);

        (result, self)
    }
}
```

Each blocker is an accessibility or specification gap, not a missing capability:

| Blocker | Current state |
|---|---|
| Rotate requires a capability | `@rotate_left (value: int, amount: int) -> int` exists at `20-capabilities.md:407` — gated `uses Intrinsics` |
| Shift substitute panics | `14-expressions.md:415-424` — `1 << 63` is a shift-overflow panic; left-shift counts are 0..62 |
| `>>` fill behavior unstated | `operator-rules.md:337` gives only panic conditions; Annex E states the rule but the operator definition does not |
| 64-bit constants unwritable | `07-lexical-elements.md` — a literal outside `0 .. 2^63-1` is a compile-time error |

### When This Matters

Any pure algorithm over 64-bit values: pseudo-random generation, hashing, checksums, bit-set and popcount algorithms, fixed-point arithmetic, encoding and decoding. Each is a natural stdlib citizen under the lean-core principle, and each is currently forced either to declare a capability it does not need or to be written in the compiler instead of in Ori.

The concrete blocked consumer is `stdlib-random-rng-proposal.md`, whose pure seeded core states `no capability` as an explicit goal.

---

## Goals and Non-Goals

**Goals:**

- Make the five platform-invariant bit operations callable from pure, `uses`-free code.
- State `>>` fill behavior normatively where the operator is defined.
- Provide an expression for 64-bit constants above `2^63 - 1`.
- Keep every change additive: no existing program changes meaning.

**Non-Goals:**

- **Wrapping arithmetic.** `wrapping_add` / `wrapping_sub` / `wrapping_mul` are already approved in `overflow-behavior-proposal.md` and already carry roadmap work items. This proposal consumes them.
- **A widening 64x64 -> 128 multiply.** Genuinely absent from the corpus, but avoidable — bounded-integer draws can use rejection sampling over compare and modulo. Deliberately not requested.
- **An unsigned integer type.** Annex E's single-signed-type decision stands; item 3 follows the existing `int.min` associated-constant idiom instead.
- **SIMD or CPU feature detection.** Both remain `uses Intrinsics`; this proposal narrows nothing about them.
- **Changing what any bit operation computes.** Semantics are unchanged; only reachability and specification change.

---

## Design

### 1. Bit operations become pure functions

The `Intrinsics` capability currently bundles three unlike things:

| Group | Members | Platform-dependent? | Ambient effect? |
|---|---|---|---|
| SIMD | `simd_*_f32x4`, `simd_*_i64x4`, ... | Yes — availability and width vary by target | No, but width selection is target-visible |
| Bit operations | `count_leading_zeros`, `count_trailing_zeros`, `count_ones`, `rotate_left`, `rotate_right` | **No** — identical results on every target | **No** |
| Hardware queries | `cpu_has_feature` | Yes | **Yes** — queries ambient machine state |

A capability exists to make an effect or an ambient dependency visible in a signature. SIMD carries target-width dependence; `cpu_has_feature` queries the running machine. The five bit operations carry neither: each is a total, deterministic function from `int` (and for rotates, a second `int`) to `int`, with the same value on every platform. They were grouped with the others because all are compiler intrinsics — an implementation-locality grouping, not a semantic one.

Move them to `std.math` as ordinary pure functions:

```ori
// std.math — no capability required
pub @rotate_left (value: int, amount: int) -> int
pub @rotate_right (value: int, amount: int) -> int
pub @count_leading_zeros (value: int) -> int
pub @count_trailing_zeros (value: int) -> int
pub @count_ones (value: int) -> int
```

`Intrinsics` retains the same five method names so existing `uses Intrinsics` code continues to compile unchanged; the trait methods become thin forwarders to the `std.math` functions. SIMD and `cpu_has_feature` are untouched.

Rotates are total over every `amount`, including negative and `>= 64`: the amount is reduced modulo 64 before rotating. A rotate cannot lose information, so no shift-overflow condition applies and no panic is defined.

### 2. `>>` is a logical shift

`annex-e-system-considerations.md:29` already states the governing rule: *"There is no separate unsigned integer type. Bitwise operations treat the value as unsigned bits."* The operator definition at `operator-rules.md:337` does not restate it, and the spec never uses the words *arithmetic shift*, *logical shift*, *sign extend*, or *zero fill* anywhere.

State it where the operator is defined: `>>` shifts the two's-complement bit pattern right and fills vacated high bits with zero, for every `int` including negative values. `<<` is unchanged, including its existing overflow panic.

```ori
let x = -1;        // all 64 bits set
let y = x >> 60;   // 0b1111 == 15, NOT -1
```

This is a clarification, not a behavior change: Annex E already required it, and no normative text ever specified sign extension.

### 3. 64-bit constants above `2^63 - 1`

`07-lexical-elements.md` restricts integer literals to `0 .. 2^63 - 1`, and its own NOTE establishes how the language handles a value that cannot be written: *"The minimum `int` value (-2^63) cannot be written as a literal ... It is available as the associated constant `int.min`."*

Generalize that idiom with a compile-time constructor from a bit pattern:

```ori
$int.from_bits(0xBF58476D1CE4E5B9)   // const fn; the argument is a bit pattern
```

`from_bits` accepts any 64-bit hexadecimal or binary pattern and yields the `int` with exactly those bits, reinterpreting values at or above `2^63` as their two's-complement negatives. It is a const function, usable wherever a constant expression is required, so pinned algorithm constants stay legible as the published hexadecimal values rather than being hand-converted to negative decimals.

`int.min` is retained and becomes definitionally `$int.from_bits(0x8000000000000000)`.

### Error Handling

- `from_bits` with a pattern wider than 64 bits: compile-time error, consistent with the existing literal-range error.
- `from_bits` with a decimal (non-bit-pattern) argument outside the literal range: compile-time error; the existing literal rule is unchanged, and `from_bits` is explicitly a bit-pattern constructor.
- Rotates: total; no error condition.
- `>>` and `<<`: existing panic conditions unchanged (negative count, count at or beyond width, left-shift overflow).

---

## Drawbacks

- **The bit operations gain a second spelling.** `std.math.rotate_left` and `Intrinsics.rotate_left` both exist during and after migration. This is one-way-to-do-things pressure. Mitigated by making the trait method a forwarder, so there is one implementation and one semantics; the duplication is nominal.
- **Amending an approved proposal.** `intrinsics-capability-proposal.md` is approved and would need errata narrowing its bit-operations section. That is real governance cost for a change that is, semantically, a no-op.
- **`from_bits` adds surface to a deliberately small numeric core.** A reader must now know two ways a 64-bit constant can appear. The alternative, hand-converted negative decimals, is worse for review and for matching published algorithm constants, but the surface cost is genuine.
- **Pinning `>>` forecloses a future arithmetic-shift operator without a second name.** Languages that kept the question open (see Prior Art) ended up debating a separate operator. Choosing now means a future arithmetic shift needs its own spelling.

---

## Alternatives Considered

### Alternative 1: Leave rotate gated; let pure algorithms declare `uses Intrinsics`

Rejected. It makes a capability declare something untrue. `uses Intrinsics` in a signature tells a reader and the effect system that the function depends on ambient hardware; a seeded generator does not. It would also propagate the capability transitively through every caller of a pure algorithm, which is precisely the noise the capability system exists to avoid.

### Alternative 2: Introduce an unsigned 64-bit type

Rejected. Annex E's single-signed-integer decision is deliberate and load-bearing across the language. A `u64` would introduce conversion rules, literal-suffix syntax, and arithmetic-mixing questions across the whole numeric surface — an outcome disproportionate to expressing a handful of constants.

### Alternative 3: Write the constants as negative decimals

`0xBF58476D1CE4E5B9` is expressible today as `-4658895280553007687`. Rejected as the primary mechanism: it silently diverges from every published reference for these algorithms, is unreviewable by inspection, and invites transcription errors in exactly the values where an error is undetectable by testing short sequences.

The hazard is not hypothetical. The first draft of this paragraph stated the conversion as `-4688729468158715975`, which is wrong; the error was caught only by machine-checking the subtraction, not by reading. A reviewer cannot verify a 19-digit negative decimal against a published hexadecimal constant by inspection, and a wrong PRNG constant still produces plausible-looking output. The mechanism remains available and this proposal does not forbid it, but it is unsuitable as the standard way to write pinned algorithm constants.

### Alternative 4: Leave `>>` unspecified and require callers to mask

A caller wanting logical behavior could write `(x >> k) & mask`. Rejected: the mask depends on `k`, the construction is error-prone, and it leaves the base operator's meaning genuinely undefined — every consumer would be guessing, and two implementations could disagree while both conforming.

### Alternative 5: Add a distinct `>>>` logical-shift operator

Rejected. Annex E already commits to unsigned-bits semantics for bitwise operations, so `>>` is not free to mean arithmetic shift; adding `>>>` would create two operators where the spec has already decided the rule for one. It also adds grammar surface for a distinction the language has chosen not to draw.

---

## Purity Analysis

**Can be pure Ori?** NO — all three items are compiler and spec surface by construction.

**If not, why:**

- Bit operations are compiler intrinsics; changing where they are callable from is a compiler and capability-surface change.
- `>>` fill behavior is a specification statement, enforced by codegen.
- A bit-pattern constant constructor is a const-evaluation and lexical-surface feature.

**Missing features that would enable purity:** Not applicable — this proposal *is* the missing-feature request. It lets a large class of downstream algorithms be pure Ori stdlib rather than compiler built-ins. The precedent is `hash_combine`, which is a compiler built-in today specifically because wrapping arithmetic was unreachable from Ori source; with this proposal plus the approved wrapping functions, that class of algorithm becomes expressible in the standard library.

**Recommendation:** Proceed as a minimal compiler and spec change, justified by the number of pure-Ori library algorithms it unblocks.

---

## Spec & Grammar Impact

- **Clause 7 (`07-lexical-elements.md`)** — literal range is unchanged; add a NOTE pointing to `int.from_bits` for 64-bit patterns, parallel to the existing `int.min` NOTE.
- **Clause 14 (`14-expressions.md`)** — state that `>>` fills vacated high bits with zero for all `int` values; `<<` panic conditions unchanged.
- **Annex B (`operator-rules.md:337`)** — restate the logical-fill rule at the `shift_right` definition so the operator table is self-contained.
- **Clause 20 (`20-capabilities.md:404-408`)** — the five bit operations remain listed on `Intrinsics` as forwarders; add a note that the canonical definitions are the pure `std.math` functions and that no capability is required to call them.
- **Annex E** — unchanged; item 2 makes its existing rule normative at the operator.
- **`std.math` module doc** — document the five pure functions and `int.from_bits`.
- **No grammar change.** `int.from_bits(...)` is an ordinary associated-function call; no new production.
- **Errata** — `intrinsics-capability-proposal.md` gains an errata entry narrowing its bit-operations section to forwarders.

---

## Prior Art

Verified against the intelligence graph's issue corpus; entries below state only what the cited titles support.

- **Zig — right-shift mode is a live design question.** `zig#20367` *"Proposal: Make right shift mode explicit for signed integers"* (open) and `zig#5220` *"Proposal: Explicit Shift Operators"* (open) show a language that left signed-shift mode implicit and is still litigating it. Ori is better positioned: Annex E already decided the rule, so item 2 records an existing decision rather than making a new one.
- **Go — shift semantics required a spec clarification.** `go#44664` *"spec: clarify that signed integers>=0 are permitted as shift counts"* shows shift behavior needing explicit spec text rather than being left to inference.
- **Rust** — `rotate_left` / `rotate_right` / `count_ones` / `leading_zeros` are inherent methods on integer primitives, requiring no capability, feature gate, or import. Wrapping arithmetic is likewise ordinary methods. This is the shape item 1 adopts.
- **Rust hexadecimal literals** — a `u64` literal such as `0xBF58476D1CE4E5B9` is written directly because an unsigned type exists. Ori declines that route (Alternative 2), so `from_bits` supplies the equivalent expressiveness without a second integer type.

---

## Unresolved Questions

- **Migration horizon for the `Intrinsics` forwarders.** Are the trait methods retained permanently for source compatibility, or deprecated after a transition? This proposal retains them; it does not decide their long-term fate.
- **Naming.** `int.from_bits` follows `int.min`. Alternatives such as a `0x...u` literal suffix or `int.from_hex` were not surveyed in depth and may read better.
- **Byte variants.** `overflow-behavior-proposal.md` specifies byte variants of the wrapping functions. Whether the bit operations need `byte` counterparts is left open; the motivating consumer needs only `int`.
- **Should `count_*` move at all?** Only `rotate_left` is required by the motivating consumer. The other four are moved because they share the same purity argument, but a narrower proposal moving only the rotates is a defensible reduction in scope.
