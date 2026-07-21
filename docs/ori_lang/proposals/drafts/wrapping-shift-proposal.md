# Proposal: Wrapping Shift Operations

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 14, Annex B operator-rules), stdlib (`library/std/math/`), compiler (intrinsic recognition for one function)
**Depends On:** logical-shift-operator-proposal.md (draft — supplies `>>>`, over which `wrapping_shr_logical` is defined)
**Related:** overflow-behavior-proposal.md (approved — establishes the `wrapping_*` family, its `std.math` home, and the free-function-with-named-arguments spelling this proposal follows), std-math-bit-operations-proposal.md (draft — its `rotate_left` reference body is the primary consumer), limbs-trait-proposal.md (draft — consumer), stdlib-math-api-proposal.md (draft — consumer; also owns the `std.math` submodule manifest), stdlib-random-rng-proposal.md (draft — transitive consumer through `rotate_left`), pure-bit-operations-proposal.md (withdrawn — documented this gap at `:64-65` without proposing a cure), wide-integer-literals-proposal.md (draft — sibling successor of the withdrawn proposal; unrelated to this one except in Purity Analysis)

---

## Summary

Ori's `<<` panics whenever a shifted bit would leave the 64-bit range, and every shift operator panics on a count at or beyond the operand's width. Both are correct defaults. Neither is usable by bit-manipulation code, where discarding shifted-out bits is the intended semantics and a count of exactly `64` arises naturally at the boundary of a rotate. This proposal adds three `std.math` free functions — `wrapping_shl`, `wrapping_shr`, `wrapping_shr_logical` — that mask the shift count to `count & 63` and discard shifted-out bits, making them total over every pair of `int` arguments. It changes no operator. It also reconciles a pre-existing contradiction between two normative statements of `<<`'s shift-count range, resolved against the implementation.

---

## Motivation

### The Problem in Practice

A 64-bit rotate is the smallest complete example, and it is the operation a sibling proposal publishes as a normative reference body:

```ori
// drafts/std-math-bit-operations-proposal.md:33, as currently written
@rotate_left (value: int, amount: int) -> int = {
    let $k = amount & 63;

    (value << k) | (value >>> (64 - k) >>> 0)
}
```

This body panics on two independent inputs, and neither is an edge case a caller can avoid:

| Input | What traps | Rule |
|---|---|---|
| any `value` whose bit `64 - k` is set | `value << k` overflows | `14-expressions.md:415,418` — shift overflow |
| `k == 0` | `value >>> (64 - 0)` is a count of `64` | `14-expressions.md:419`, `operator-rules.md:337` — count at or beyond width |

`amount: 0` is on the sibling proposal's own pin list. The published reference body fails its own pins, and that proposal's §7 declares the reference body to be the **fallback lowering** — so on any target without a native rotate instruction, the rotate would panic on its documented cases.

The withdrawn `pure-bit-operations-proposal.md:64-65` recorded this blocker precisely — *"Hand-rolling the rotate does not help: `<<` panics on overflow"* — and proposed no cure. This proposal is that cure.

### There is no workaround

- **Masking before the shift** does not help: the panic is on the shifted-out bits, and the value that must survive is the low `64 - k` bits, which requires the mask `(1 << (64 - k)) - 1` — an expression that itself traps at `k == 0` (count `64`) and at `k == 1` (`1 << 63` overflows).
- **A conditional guarding `k == 0`** removes one of the two panics and leaves the value-overflow panic untouched for every other `k`.
- **Multiplying by a power of two** is the correct identity (`a << k` is `a * 2^k` modulo `2^64`) and `wrapping_mul` is approved — but the multiplier for `k = 63` is `2^63`, which is not writable as an `int` today, and selecting it needs a 64-arm dispatch. See Purity Analysis; this is why the argument for compiler involvement is performance, not expressibility.

### Consumers blocked today

Corpus facts, read from the cited lines:

| Consumer | Line | Construct | Why it traps |
|---|---|---|---|
| `drafts/std-math-bit-operations-proposal.md:33` | `rotate_left` reference body, which its §7 makes the fallback lowering | `(value << k) \| (value >>> (64 - k) >>> 0)` | value overflow, and count `64` at `k == 0` |
| `drafts/std-math-bit-operations-proposal.md:38-42` | `count_ones` SWAR body | `value - (...)`, `(a & m) + (...)` | plain `-` and `+` underflow and overflow at `int.min`; needs the approved `wrapping_sub` / `wrapping_add`, not this proposal |
| `drafts/limbs-trait-proposal.md:318` | cross-limb shift | `a << 64` | count at or beyond width for `int`; the draft states no shift-count rule for `Limbs` types at all |
| `drafts/stdlib-math-api-proposal.md:606` | written-out PCG generator | `(xorshifted >> rot) \| (xorshifted << ((-rot) & 31))` | value overflow on the left half |
| `drafts/stdlib-random-rng-proposal.md` | pinned xoshiro256\*\*, whose `next` is `rotl(s[1] * 5, 7)` | transitively, through `rotate_left` | as above |

The `count_ones` row is listed because it appears in the same reference-body block and is a **different** defect with a **different** owner: its cure is the already-approved `wrapping_sub` and `wrapping_add`, not anything proposed here. Naming it prevents this proposal from being read as covering it.

### When This Matters

Any algorithm that treats an `int` as 64 unsigned bits and expects shifted-out bits to be discarded: rotates, hash mixers, pseudo-random generators, bignum limb shifts, bit-set iteration, fixed-point normalization, encoding and decoding. Each is a natural stdlib citizen under the lean-core principle.

---

## Goals and Non-Goals

**Goals:**

- Add `wrapping_shl`, `wrapping_shr`, and `wrapping_shr_logical` as `std.math` free functions, total over every pair of `int` arguments.
- Mask the shift count as `count & 63`, matching the already-approved rotate normalization, so a count of `64` is `0` rather than a panic.
- Discard bits shifted out of the 64-bit range rather than panicking.
- Guarantee that `rotate_left` and `rotate_right` become writable as non-panicking pure Ori over these three functions.
- Reconcile the pre-existing `14-expressions.md:424` versus `operator-rules.md:341` contradiction about `<<`'s valid shift-count range, resolved against the implementation.
- Keep every change additive: no operator changes, no existing program changes meaning, no existing conformance pin is invalidated.

**Non-Goals:**

- **Changing `<<`, `>>`, or `>>>`.** The operators keep their panic contracts exactly. Panicking is the right default for a shift written in ordinary arithmetic code; these functions are the opt-in for code that wants the other semantics. This mirrors `overflow-behavior-proposal.md`, which left `+` panicking and added `wrapping_add` beside it.
- **`checked_shl` and `saturating_shl`.** The approved family carries `checked_*` and `saturating_*` alongside `wrapping_*` for add, sub, and mul, so family symmetry would suggest them. No consumer in the corpus needs either, and a saturating shift has no single obvious meaning — saturating on the value, on the count, or both. Both are additive later. See Alternatives.
- **`byte` variants.** `overflow-behavior-proposal.md` specifies `byte` variants of the wrapping arithmetic functions, so `byte` shift variants are a coherent future addition with mask `count & 7`. No consumer needs them; left open.
- **A wrapping LEFT-shift operator.** An operator form would need a new token, a trait, and a range-transfer function for a semantics that is not the default. The function form matches the settled spelling for this family.
- **Shift semantics for `Limbs` types.** `limbs-trait-proposal.md:318`'s `a << 64` is a shift on a `U256`, not an `int`. That draft states no shift-count rule for its own types; supplying one is its obligation, not this proposal's.

---

## Design

### 1. The three functions

Declared in `std.math`, beside the approved `wrapping_add` / `wrapping_sub` / `wrapping_mul`:

```ori
pub @wrapping_shl (a: int, b: int) -> int
pub @wrapping_shr (a: int, b: int) -> int
pub @wrapping_shr_logical (a: int, b: int) -> int
```

Called with named arguments, which `14-expressions.md:163` requires for direct calls:

```ori
use std.math { wrapping_shl, wrapping_shr_logical };

@rotate_left (value: int, amount: int) -> int = {
    let $k = amount & 63;

    wrapping_shl(a: value, b: k) | wrapping_shr_logical(a: value, b: 64 - k)
}
```

The parameter names `a` and `b` follow `overflow-behavior-proposal.md:149-151` (`@wrapping_add (a: int, b: int) -> int`) rather than a shift-specific `value` / `amount` pair, so the whole `wrapping_*` family reads uniformly. This is deliberate and is the one place where family consistency was preferred over local descriptiveness.

### 2. Semantics — pinned

Let `k = b & 63`. All three operate on the 64-bit two's-complement pattern of `a`.

| Function | Result |
|---|---|
| `wrapping_shl(a:, b:)` | the pattern of `a` shifted left by `k`, with bits shifted beyond bit 63 **discarded**, and vacated low bits zero |
| `wrapping_shr(a:, b:)` | the pattern of `a` shifted right by `k`, with vacated high bits taking the value of `a`'s sign bit — the `>>` fill rule |
| `wrapping_shr_logical(a:, b:)` | the pattern of `a` shifted right by `k`, with vacated high bits zero — the `>>>` fill rule |

**All three are total.** There is no input pair for which any of them panics: `b & 63` is in `0..63` for every `int` `b`, including negative values and values at or above `64`, and no shift of a 64-bit pattern by `0..63` produces an unrepresentable `int` once shifted-out bits are discarded.

```ori
wrapping_shl(a: 1, b: 63)          //  int.min   — the bit survives; `1 << 63` panics
wrapping_shl(a: 3, b: 63)          //  int.min   — the high bit of `3` is discarded
wrapping_shl(a: 1, b: 64)          //  1         — count masks to 0
wrapping_shl(a: 1, b: -1)          //  int.min   — count masks to 63
wrapping_shr_logical(a: -1, b: 64) //  -1        — count masks to 0
wrapping_shr_logical(a: -1, b: 1)  //  int.max
wrapping_shr(a: -16, b: 3)         //  -2        — sign propagates, as `>>` does
```

### 3. Why the count is masked, not merely clamped or checked

Masking is load-bearing, not a convenience. The rotate identity requires a count of exactly `64` to behave as `0`:

- At `amount == 0`, `rotate_left` evaluates `wrapping_shr_logical(a: value, b: 64 - 0)`. With masking, that is `value >>> 0`, which is `value`, and `value | value` is `value` — the correct identity rotate.
- Without masking, `64` is out of range under every alternative disposition: a panic reproduces the defect; a clamp to `63` gives `value >>> 63`, which is wrong; returning `0` gives `value | 0`, which happens to be right for the left-shift half at `k == 0` but is wrong for the general case and makes the two halves obey different rules.

Only masking makes the standard rotate identity hold at every `amount` with no special case.

`count & 63` is also the rule Ori has already approved for a related operation: `intrinsics-capability-proposal.md:326-330` states *"Rotation amounts are taken modulo 64"* with the worked example `Intrinsics.rotate_left(value: 1, amount: 65)  // Same as amount: 1`. Using the same normalization for the shift functions keeps rotate and its constituent shifts on one rule instead of two. The `& 63` spelling is that rule made exact for negative counts, where `-1 & 63` is `63` under two's complement.

### 4. `wrapping_shl` is compiler-recognized; the other two are pure Ori

The three functions are not equally hard, and saying so is the honest framing:

| Function | Body | Needs compiler support? |
|---|---|---|
| `wrapping_shr` | `a >> (b & 63)` | **No.** The masked count is always in `0..63`, so `>>` never panics. A one-line pure-Ori body |
| `wrapping_shr_logical` | `a >>> (b & 63)` | **No**, once `>>>` exists. Same argument. Depends on `logical-shift-operator-proposal.md` |
| `wrapping_shl` | — | **Yes.** `a << (b & 63)` still panics on value overflow, which is precisely the behavior being opted out of. There is no expression over existing operators that discards the overflow |

So the compiler surface this proposal actually requests is **one function**. `wrapping_shl` is recognized by canonical symbol identity registered in `ori_registry` — not by name-sniffing at a codegen site — and lowers to the target's native shift instruction, which discards shifted-out bits natively on every architecture Ori targets. The other two ship as ordinary library code in the same module.

`wrapping_shr` and `wrapping_shr_logical` are nonetheless declared here rather than left to callers. Two reasons: a caller writing `a >> (b & 63)` inline reproduces the masking rule at every site, which is the duplication a stdlib exists to prevent; and the rotate identity needs all three to agree on one normalization, which is enforceable when they are one family and merely conventional when they are not.

### 5. The `<<` shift-count contradiction — reconciled

Two normative statements disagree about `<<`, and this proposal is the one that must state precisely what `wrapping_shl` is the non-panicking counterpart to, so the reconciliation lands here.

| Source | Statement |
|---|---|
| `14-expressions.md:415` | "Shift operations panic when the shift count is negative, exceeds the bit width, or the result overflows." |
| `14-expressions.md:418` | `1 << 63;  // panic: shift overflow (result doesn't fit in signed int)` |
| `14-expressions.md:424` | "valid shift counts are 0 to 62 for left shift when the result shall remain representable. For right shift, counts 0 to 63 are valid." |
| `operator-rules.md:336` | `n1 << n2 => shift_left   [n2 < 0 -> panic, n2 >= width -> panic]` |
| `operator-rules.md:341` | `int width = 64 bits (valid shift: 0..63)` |

Each document holds half the truth, established against the implementation rather than by choosing a document:

- **The count range is `0..63`, not `0..62`.** `compiler/ori_patterns/src/value/scalar_int.rs:131-141` rejects only `shift >= 64` on the count axis. A count of `63` is valid whenever the value survives it, which is not a vacuous case: `-1 << 63` evaluates to `int.min` and `0 << 63` evaluates to `0`, both confirmed by running the compiler. `14-expressions.md:424`'s "0 to 62" is therefore **wrong**, and `operator-rules.md:341` is right.
- **The value-overflow panic is real and `<<`-only.** `scalar_int.rs:136-139` computes `wrapping_shl` and then verifies `result.wrapping_shr(shift) == self`, returning `None` when bits were lost. `1 << 63` panics for this reason, not because `63` is an invalid count. Running `1 << 63` through the LLVM backend produces `ori panic: integer overflow on left shift`, so the evaluator and the compiled path agree. `operator-rules.md:336` omits this condition entirely and is therefore **incomplete**, while `14-expressions.md:415,418` is right.

The reconciliation this proposal lands in both documents:

- `<<` panics on a negative count, on a count at or beyond the operand's bit width, and on value overflow — bits shifted beyond the operand's range.
- `>>` and `>>>` panic on a negative count and on a count at or beyond the bit width. Neither has a value-overflow condition.
- The valid count range for every shift operator on `int` is `0..63`, and on `byte` is `0..7`.

`14-expressions.md:424` is corrected to state the count range as `0..63` for both directions, with the value-overflow condition stated separately as a property of `<<` rather than folded into the count range. `operator-rules.md:336` gains the overflow condition for `<<`.

This is a **spec correction**, not a behavior change: no executor changes, and no program changes meaning. It is in scope here because `wrapping_shl` is defined as the function that removes exactly the value-overflow panic, and that panic is currently absent from one of the two documents that define it.

### 6. Error handling

None of the three functions has an error condition. All are total over every `(a, b)` pair of `int` values, as §2 establishes. There is no diagnostic to allocate and no panic to specify.

The asymmetry with the operators — `<<` panics where `wrapping_shl` does not — is the entire point of the proposal and matches the shape `overflow-behavior-proposal.md` already established for `+` versus `wrapping_add`.

### 7. Landing zone

`overflow-behavior-proposal.md:96-101` places the wrapping family in `std.math` directly:

```ori
use std.math {
    saturating_add, saturating_sub, saturating_mul,
    wrapping_add, wrapping_sub, wrapping_mul,
    checked_add, checked_sub, checked_mul
}
```

These three join that import list. They are shifts, not a new category, and splitting them into a separate submodule would put `wrapping_shl` in a different place from `wrapping_mul` for no reason a caller can predict.

`library/std/math/mod.ori` is currently a 47-line file that is 100% comments, beginning `// TODO: Implement mathematical functions`, sketching float-only functions. The approved `wrapping_*` functions themselves are unshipped — a search of `docs/ori_lang/v2026/spec/` and `library/` returns zero occurrences. So this proposal, like its approved dependency, is specifying into a module that does not yet exist.

`stdlib-math-api-proposal.md:672-698` lists a per-category submodule manifest for `std/math/mod.ori` with no row for the approved `wrapping_*` / `saturating_*` / `checked_*` functions at all. That manifest is therefore already incomplete with respect to approved text, independent of this proposal. Coordination obligation: whichever of the two drafts is approved second adds the other's rows. This proposal claims no new manifest row, because it adds no new submodule.

---

## Drawbacks

- **Two spellings for one concept.** A reader must know that `a << k` and `wrapping_shl(a: a, b: k)` differ, and the difference is invisible at the call site. This is the identical cost `overflow-behavior-proposal.md` already accepted for `+` versus `wrapping_add`, and the mitigation is the same: the panicking form is the default, so the opt-out is the one that must be written explicitly.
- **Masked counts hide caller bugs.** `wrapping_shl(a: x, b: 1000)` silently shifts by `40` instead of reporting that `1000` is nonsense. This is inherent to totality, and it is the same trade the approved rotate normalization already made. A caller who wants the count validated should use the operator, which panics.
- **`b & 63` on a negative count is surprising.** `wrapping_shl(a: x, b: -1)` shifts left by `63`. It is total and consistent with the approved rotate rule, but no reader guesses it. It is pinned and documented rather than left to inference.
- **An incomplete family.** Shipping `wrapping_*` without `checked_*` and `saturating_*` makes the shift row of the family table ragged next to the add / sub / mul rows. The alternative is shipping API with no consumer.
- **A third proposal in a dependency chain.** `std-math-bit-operations-proposal.md`'s reference bodies need this **and** `logical-shift-operator-proposal.md`. Three drafts must land for one rotate to be writable, and the ordering constraint is real.
- **One more compiler-recognized symbol.** `wrapping_shl` joins the set whose registry entry and lowering must stay correct across every executor. Registry drift silently costs the performance the recognition exists to deliver.

---

## Alternatives Considered

### Alternative 1: A masked-count variant only, leaving value overflow to panic

`shl_masked(a:, b:)` that masks the count but still panics on value overflow. Rejected: it fixes only the `k == 0` half of the rotate defect. Every rotate whose left half sets bit 63 still traps, which is most of them — for a rotate by `k`, the left half sets bit 63 whenever bit `63 - k` of the input is set. The value-discarding behavior is the larger half of the request, not the smaller.

### Alternative 2: A value-wrapping variant only, leaving the count to panic

`wrapping_shl` that discards overflow bits but panics on a count at or beyond `64`. Rejected: the rotate identity at `amount == 0` needs `>>> 64` to mean `>>> 0`, and `amount: 0` is on the consumer's own pin list. Without masking, every caller writes the same `if k == 0` special case, which is the duplication a stdlib function exists to eliminate — and getting it wrong is silent.

### Alternative 3: A wrapping left-shift OPERATOR

A `<<%` or similar. Rejected. `overflow-behavior-proposal.md:255-259` evaluated exactly this API-shape question for this exact family and settled it:

| Approach | Example | Problem |
|---|---|---|
| Operators | `a +% b` | Adds cryptic syntax |
| Methods | `a.wrapping_add(b)` | Integers are not struct types |
| **Functions** | `wrapping_add(a: x, b: y)` | Clear, explicit, no new syntax |

An operator would additionally need a token, a trait, const-expression admission, and a range-transfer function, for a semantics that is not the default. Re-deciding a settled question would create a second source of truth for the family's spelling.

### Alternative 4: Add `checked_shl` and `saturating_shl` for family symmetry

Deferred rather than rejected. `checked_shl` returning `Option<int>` is well-defined and would compose with the approved `checked_*` row. `saturating_shl` is not well-defined without an extra decision — saturating on the value (clamp to `int.min` / `int.max`), on the count (clamp to `63`), or both — and the three answers disagree. No consumer in the corpus needs either. Adding them later is purely additive.

### Alternative 5: Make these `Intrinsics` capability methods

Rejected on the approved criterion. `capability-unification-generics-proposal.md:203-210` (approved 2026-02-20) distinguishes structural from environmental properties, and its **Mockable** row is the discriminator: an environmental capability is one the caller provides and may substitute (`with Http = Mock in`). A substituted `wrapping_shl` returning different bits is a broken implementation, not a legitimate environment. These functions are not caller-determined, not caller-provided, and not meaningfully mockable, so they fail every `uses` column and belong as plain functions. `std-math-bit-operations-proposal.md` reaches the same conclusion for the five bit operations on the same approved ground.

### Alternative 6: Write the rotate with a conditional instead of masked shifts

```ori
@rotate_left (value: int, amount: int) -> int = {
    let $k = amount & 63;

    if k == 0 then value else (value << k) | (value >>> (64 - k))
}
```

Rejected. It removes the count panic and leaves the value-overflow panic on `value << k` for every non-zero `k`, so it does not work. Even if `<<` discarded overflow, pushing the special case to every call site is the duplication §4 exists to prevent.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY — and the split is the proposal's substance.

- **`wrapping_shr` and `wrapping_shr_logical`: YES.** Each is a one-line body over an existing operator and a mask (§4). They ship as ordinary library code. `wrapping_shr_logical` depends on `>>>` from `logical-shift-operator-proposal.md`; `wrapping_shr` depends on nothing unshipped.
- **`wrapping_shl`: NO, as delivered.** Semantically it is expressible — `a << k` is `a * 2^k` modulo `2^64`, and `wrapping_mul` is approved — but the multiplier for `k = 63` is `2^63`, not writable as an `int` literal today, and selecting the multiplier from a runtime `k` needs a 64-arm dispatch. A shift that lowers to one machine instruction would become a table lookup and a multiply. The honest case for compiler involvement is therefore **performance**, not expressibility, which is the same correction `std-math-bit-operations-proposal.md` applied to its own framing.

**If not, why:**

- Recognizing `wrapping_shl` as a canonical symbol and lowering it to the target's native shift is compiler surface.
- Correcting the `<<` count-range and overflow statements (§5) is spec surface.

**Missing features that would enable purity:** `>>>` (sibling draft) for `wrapping_shr_logical`, which this proposal declares as a dependency. Nothing would make `wrapping_shl` a *practical* library function; the multiply identity above makes it a *possible* one, which is why the argument is performance.

**Recommendation:** Proceed as one compiler-recognized function, two library functions, and one spec correction. This is the smallest shape that unblocks the cited consumers.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `14-expressions.md:415-424` | Correct the shift-count range to `0..63` for both directions (§5); state the value-overflow condition separately as a property of `<<`; keep the `1 << 63` example, whose behavior is unchanged, and correct its explanation to name value overflow rather than an invalid count |
| `operator-rules.md:336` | Add the value-overflow panic condition to `n1 << n2`; `n1 >> n2` is unchanged |
| `operator-rules.md:341` | Unchanged — `0..63` is correct as written |
| `library/std/math/` | Declare the three functions beside the approved `wrapping_add` / `wrapping_sub` / `wrapping_mul` |
| `std.math` module doc (`docs/ori_lang/v2026/modules/`) | Document the three, including the count-masking rule and the negative-count case |
| **No grammar change** | Plain function declarations and calls; no new production, no new token, no new operator |
| **No new error code** | All three functions are total (§6) |

### Errata

| Approved proposal | Erratum |
|---|---|
| `overflow-behavior-proposal.md` | Its `wrapping_*` family gains three shift members. Its `std.math` import list (`:96-101`) and signature block (`:145-155`) extend by `wrapping_shl`, `wrapping_shr`, `wrapping_shr_logical`. The `checked_*` and `saturating_*` rows deliberately do **not** gain shift members; see Alternative 4 |
| `intrinsics-capability-proposal.md` | Its modulo-64 rotation-amount rule (`:326-330`) is cited, not amended: the `count & 63` normalization in §3 is that rule applied to shifts, so the two stay on one normalization |

No other approved proposal is amended. `logical-shift-operator-proposal.md` is a dependency, not an amendment target, and it carries its own errata for the proposals it touches.

### Conformance pins

Searched `tests/spec/**`, `library/`, and `compiler/` for `wrapping_shl`, `checked_shl`, `saturating_shl`, `rotating_shl`, and `shl_wrap` as Ori-language surface: **zero hits**, so no pin is invalidated. The surfaces searched are enumerated under Absence claims below.

Existing pins that stay green, because no operator changes:

| Pin | Asserts |
|---|---|
| `tests/spec/expressions/operators_bitwise.ori:234-249` | `-16 >> 3 == -2`; `1 >> 64` panics; `1 >> -1` panics |
| `compiler/ori_eval/src/tests/operators_tests.rs:397-402` | `i64::MIN >> 63 == -1` |

New pins required:

- `wrapping_shl` at counts `0`, `1`, `62`, `63`, `64`, `65`, `-1`, and `int.min`; on values `0`, `1`, `-1`, `int.min`, `int.max`; specifically `wrapping_shl(a: 1, b: 63) == int.min` and `wrapping_shl(a: 3, b: 63) == int.min`, which is where the discard is observable.
- `wrapping_shr` and `wrapping_shr_logical` over the same count and value classes, including `wrapping_shr_logical(a: -1, b: 1) == int.max` and `wrapping_shr(a: -1, b: 1) == -1`, which is where the two fill rules separate.
- Totality: no `(a, b)` pair in the classes above panics, for all three.
- The rotate identity end to end: `rotate_left` written over these three, checked against a reference at every `amount` class including `0`, `64`, `-1`, and `65`, and its round-trip with `rotate_right`.
- Negative pins asserting the operators are unchanged: `1 << 63` still panics; `1 >> 64` still panics; `-1 >>> 0` is still `-1`.
- A pin that `-1 << 63` and `0 << 63` succeed, which is the empirical basis of the §5 count-range correction and is currently unpinned.
- Cross-executor parity on every value case above: evaluator, VM, and LLVM/native shall agree bit-for-bit, along with any further admitted executor.

---

## Prior Art

| Language | Non-panicking shift | Count handling | Value handling |
|---|---|---|---|
| Rust | `i64::wrapping_shl`, `wrapping_shr`, plus `checked_*`, `overflowing_*` | count masked by `& 63` | bits shifted out are discarded |
| Go | `<<` is itself non-panicking | count `>= 64` yields `0` (no mask) | bits discarded |
| C / C++ | none | count `>= width` is undefined behavior | signed left-shift overflow is undefined behavior before C++20, well-defined after |
| Java | `<<` is itself non-panicking | count masked by `& 63` for `long`, `& 31` for `int` | bits discarded |
| JavaScript | `<<` is itself non-panicking | count masked by `& 31` | operates on `int32`; bits discarded |
| Zig | `<<` traps on overflow; `<<\|` saturates; `@shlWithOverflow`, `@shlExact` | count type is bounded so an out-of-range count is a compile error | `<<` traps, `<<\|` saturates |

**Rust is the closest structural match and the model this proposal follows**: a panicking default operator plus an explicit `wrapping_*` opt-out that masks the count and discards the bits. Ori's `overflow-behavior-proposal.md` already adopted that shape for arithmetic; this extends it to shifts, which is the row Rust has and Ori's approved family is missing.

Two design points worth naming rather than assuming:

- **Go masks nothing** — a count of `64` yields `0` rather than `1`. That is a defensible alternative to masking, and it is the one that breaks the rotate identity: `value >> 64` giving `0` makes `rotate_left(value, 0)` return `value | 0`, correct by accident on the left half and wrong in general. Masking is chosen because it makes the identity structural (§3).
- **Zig makes an out-of-range count a compile error** by typing the count, which is stronger than either. Ori's `int` count type cannot express that bound, so the choice is between panicking (the operator, retained) and masking (these functions).

### Issue-corpus entries

Found by searching the reference-implementation issue corpus available in this repository over shift-overflow and wrapping-shift phrasings. Titles and states are as the corpus records them; the full discussions were not read, and each is cited only for the narrow point stated:

- `zig#159` *"explicit wrapping integer operations instead of w types"* — **closed, completed**. Cited for the narrow point that a language moved from type-encoded wrapping to explicit per-operation wrapping, which is the shape this proposal adopts.
- `zig#46` *"integer wrapping"* — **closed, completed**. Cited as the originating discussion of that direction.
- `zig#9949` *"Sat shl neg rhs"* — **closed** PR. Cited for the narrow point that a negative shift count is a real design question a saturating shift had to answer, which is why §3 pins the `-1 & 63 == 63` case rather than leaving it to inference.
- `lean4#328` *"fix: bitwise shift overflow of UInt types"* — **closed** PR. Cited for the narrow point that shift-overflow behavior is a recurring correctness defect and not a settled detail.

**Grounding note.** The Rust, Go, Zig, and Swift rows are corpus-verifiable. The Java, JavaScript, C, and C++ rows are from language-reference knowledge; no Java, JavaScript, or C repository with the relevant specification text is present in the reference corpus searched, so those rows are recorded as **not independently corpus-verified**.

---

## Migration / Breaking Changes

None. Every change is additive:

- No operator changes. `<<`, `>>`, and `>>>` keep their exact panic contracts, and every conformance pin over them stays green.
- Three new `std.math` functions. No existing program can call them.
- The §5 spec reconciliation is a **correction**, not a behavior change: it states what every executor already does. `14-expressions.md:424`'s "0 to 62" is wrong today against the implementation, and no program's behavior changes when it is corrected.

### Absence claims and the searches that establish them

Surfaces searched for every claim below: `docs/ori_lang/proposals/drafts/`, `docs/ori_lang/proposals/approved/`, `docs/ori_lang/v2026/spec/`, `library/`, `compiler/`, `tests/`.

| Claim | Result |
|---|---|
| No Ori-language `wrapping_shl` / `checked_shl` / `saturating_shl` / `rotating_shl` / `shl_wrap` surface exists | Zero hits in `proposals/drafts/`, `proposals/approved/`, `v2026/spec/`, `library/`, and `tests/`. The hits under `compiler/` are Rust standard-library method names invoked on Rust integer types inside the compiler's own implementation — `ori_patterns/src/value/scalar_int.rs:131,136`, `ori_canon/src/const_fold/arithmetic.rs:120`, `ori_llvm/src/codegen/ir_builder/checked_ops/shift.rs:16`, `ori_vm/src/execute/primitives.rs:115`, `ori_eval/src/operators/mod.rs:252`, `ori_repr/src/range/transfer/bitwise.rs:45`, `ori_types/src/const_eval.rs:304`, and their tests. None is an Ori-language or stdlib surface |
| No approved proposal changes `<<`'s overflow behavior | Every approved proposal mentioning `shift`, `shl`, `shr`, or `<<` was classified. `operator-traits-proposal.md` and `operator-method-naming-proposal.md` govern dispatch and naming; `compound-assignment-proposal.md` adds `<<=` as sugar with, in its own words, no new semantics; `grammar-sync-formalization-proposal.md` and `range-step-proposal.md` govern precedence and grammar; `const-generics-proposal.md` and `const-generic-bounds-proposal.md` admit shifts in const expressions without stating an overflow rule; the remainder are example code or unrelated senses of the word "shift". **No approved proposal states a left-shift overflow rule at all** |
| `overflow-behavior-proposal.md` says nothing about shifts | A search of that file for `shl`, `shr`, `shift`, `<<`, and `>>` returns **zero hits**. It covers add, sub, mul, div, and negation only. Any claim that it settles the shift contract is false, and this proposal does not make one |
| `wrapping_add` / `wrapping_mul` are approved but unshipped | Zero occurrences in `docs/ori_lang/v2026/spec/` or `library/`. `library/std/math/mod.ori` is 47 lines of comments beginning `// TODO: Implement mathematical functions` |

---

## Roadmap Impact

Implementation touches `library/std/math/` (three declarations, two of them with pure-Ori bodies), `ori_registry` (canonical symbol identity for `wrapping_shl`), every executor that must lower or evaluate `wrapping_shl`, and two spec clauses. A feature plan scaffolded on approval owns the phase breakdown.

Ordering: this proposal and `logical-shift-operator-proposal.md` can be approved in either order, but `wrapping_shr_logical` cannot be implemented before `>>>` exists. `wrapping_shl` and `wrapping_shr` have no such constraint. `std-math-bit-operations-proposal.md`'s reference bodies need both this proposal and `>>>` before they compile, and its `count_ones` body additionally needs the approved `wrapping_sub` and `wrapping_add` to be shipped.

---

## Unresolved Questions

- **`byte` variants.** Whether `wrapping_shl` and siblings should have `byte` forms with mask `count & 7`, matching the `byte` variants `overflow-behavior-proposal.md` specifies for the wrapping arithmetic functions. No consumer needs them; left open.
- **`checked_shl`.** Whether to add it now for family symmetry or when a consumer appears. Alternative 4 defers it; the decision is reversible and purely additive either way.
- **Whether `wrapping_shr` and `wrapping_shr_logical` should also be compiler-recognized.** Their pure-Ori bodies already lower to a mask and a shift, so recognition would save at most the mask when the count is a known constant. Left to implementation as a performance question, not a semantic one.
- **Recognition threshold for `wrapping_shl`.** Whether the compiler must recognize it on every target or may fall back to a library body per target is an implementation policy; the semantic requirement is only bit-identical results across executors.
