# Proposal: Logical Shift Operator `>>>`

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 14, Annex A grammar, Annex B operator-rules, Annex E), compiler (lexer, parser, type system, evaluator, LLVM codegen, `ori_repr` range analysis), stdlib (prelude trait), formatter
**Depends On:** none
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors), wide-integer-literals-proposal.md (sibling successor), std-math-bit-operations-proposal.md (sibling successor), stdlib-random-rng-proposal.md (draft — a motivating consumer), stdlib-math-api-proposal.md (draft — its written-out PCG generator depends on zero-fill shift), limbs-trait-proposal.md (draft — cross-limb shifts depend on zero-fill shift), stdlib-json-native-parser-proposal.md (draft — bit-scanning consumer), comparable-hashable-traits-proposal.md (approved — defines `hash_combine` over `<<` and `>>`; unaffected by this proposal), representation-optimization-proposal.md (approved — as-if rule and canonical representation width), intrinsics-v2-byte-simd-proposal.md (approved — governing `Intrinsics` declaration), operator-method-naming-proposal.md (approved — supplies the method-naming convention this proposal follows), operator-traits-proposal.md (approved — the operator-trait framework), overflow-behavior-proposal.md (approved — shift-overflow panic contract)

---

## Summary

Ori's `>>` is an arithmetic (sign-propagating) right shift in every executor today, but no normative spec text says so. This proposal does two things. First, it states the existing behavior normatively: `>>` shifts the two's-complement bit pattern right and replicates the sign bit into vacated high bits, for `int`; `byte >>` fills with zero because `byte` is unsigned. Second, it adds `>>>`, a distinct logical (zero-fill) right shift, so pure bit-manipulation algorithms can express a zero-fill shift without masking gymnastics. No existing program changes meaning.

---

## Motivation

### The Problem in Practice

Bit-mixing algorithms — hashes, checksums, pseudo-random generators, bignum limb shifts, SWAR scanners — are specified over unsigned 64-bit words. Ori has one integer type, signed `int`. Every published algorithm step of the form `x >> k` means zero-fill. Written in Ori today, it means sign-fill, and silently produces the wrong value whenever the high bit is set.

```ori
use std.math { wrapping_mul };

// SplitMix64's finalizer, as published: `z ^= z >> 30` is a ZERO-FILL shift.
@mix (z: int) -> int = {
    let $a = z ^ (z >> 30);        // WRONG when z < 0: fills with ones, not zeros
    let $b = wrapping_mul(a: a, b: 0xBF58476D1CE4E5B9);

    b ^ (b >> 27)                  // WRONG for the same reason
}
```

There is no correct rewriting that is also readable. Masking after the shift requires a `k`-dependent mask the caller must construct, and the natural construction of that mask (`(1 << (64 - k)) - 1`) itself panics under the existing left-shift overflow rule when `k == 0` (`14-expressions.md:415-424`).

### Consumers blocked today

These are corpus facts, not projections:

- `drafts/stdlib-math-api-proposal.md:604-606` writes out a PCG generator using `>> 18`, `>> 27`, `>> 59` on values whose high bit is set by construction.
- `drafts/limbs-trait-proposal.md:319-327` performs a cross-limb `a >> 1`, which is only correct with zero fill.
- `drafts/stdlib-random-rng-proposal.md:149` constructs a `[0.0, 1.0)` float from "the top 53 bits of a 64-bit draw" — a construction that yields a negative intermediate under sign fill.
- `drafts/stdlib-json-native-parser-proposal.md:747` scans bitmasks, where zero fill is the assumed semantics.

### When This Matters

Any algorithm treating an `int` as 64 unsigned bits: hashing, PRNG, bit-set iteration, encoding/decoding, fixed-point normalization, bignum limb manipulation. Each is a natural stdlib citizen under the lean-core principle; none can be written correctly today.

---

## Goals and Non-Goals

**Goals:**

- Add `>>>` as a distinct logical (zero-fill) right-shift operator, with its compound-assignment form `>>>=`.
- State normatively, where the operator is defined, that `>>` on `int` is arithmetic (sign-propagating) and that `>>` on `byte` is zero-fill.
- Define the operator's trait, const-evaluation admission, panic contract, and value-range transfer function.
- Keep every change additive: no existing program changes meaning; no existing conformance pin is invalidated.

**Non-Goals:**

- **Changing `>>`.** Its behavior is unchanged; only its specification changes from silent to explicit.
- **An unsigned integer type.** Annex E's single-signed-type decision stands.
- **A logical LEFT shift.** `<<` is fill-agnostic: it vacates low bits, which are always filled with zero regardless of sign convention. Only its overflow contract differs across languages, and that is settled by `overflow-behavior-proposal.md` and unchanged here.
- **Reconciling the pre-existing `0..62` vs `0..63` left-shift-count contradiction** (see Spec & Grammar Impact); this proposal notes it and touches neither statement.
- **Moving bit operations out of `Intrinsics`** — that is `std-math-bit-operations-proposal.md`.
- **64-bit literal patterns** — that is `wide-integer-literals-proposal.md`.

---

## Design

### 1. `>>` is arithmetic — stated, not changed

Grounding for the behavior claim (implementation and conformance pins, not spec prose):

| Layer | Evidence | Verdict |
|---|---|---|
| Conformance corpus | `tests/spec/expressions/operators_bitwise.ori:234-239` — `@shr_negative_value () -> int = -16 >> 3;` asserted `expected: -2`, with the source comment `// Arithmetic right shift preserves sign` | arithmetic |
| Evaluator | `compiler/ori_eval/src/operators/mod.rs:255-258` — `BinaryOp::Shr => a.checked_shr(b)` on a signed `i64` | arithmetic |
| Evaluator unit pin | `compiler/ori_eval/src/tests/operators_tests.rs:397-402` — `i64::MIN >> 63 == -1` | arithmetic |
| LLVM codegen | `compiler/ori_llvm/src/codegen/ir_builder/checked_ops/shift.rs:156` — `build_right_shift(lhs_int, rhs_int, true, name)`; the `true` selects `ashr` | arithmetic |
| Range analysis | `compiler/ori_repr/src/range/transfer/bitwise.rs:52-64` — `range_shr` documented and implemented as arithmetic right shift | arithmetic |
| Spec prose | `operator-rules.md:337` states only panic conditions; `14-expressions.md:415-424` states only shift-count validity | silent |

Add to `14-expressions.md` and to the `operator-rules.md` shift definition:

- For `int`, `>>` performs an arithmetic right shift: the two's-complement bit pattern shifts right and each vacated high bit takes the value of the original sign bit.
- For `byte`, `>>` performs a zero-fill shift, because `byte` is an unsigned 8-bit type. This matches `compiler/ori_eval/src/operators/mod.rs:406`, where the shift is applied to a `u8`.

```ori
let $a = -16 >> 3;    // -2   (sign propagates)
let $b = 0xF0 as byte >> 4;   // 0x0F (byte is unsigned; zero fill)
```

### 2. `>>>` is a logical right shift

`a >>> b` shifts `a`'s two's-complement bit pattern right by `b` places and fills every vacated high bit with zero, for every `int` value including negative ones. On `byte`, `>>>` is defined identically to `>>` (both zero-fill), so a generic bit-manipulation helper written over `>>>` behaves uniformly across both primitives.

```ori
let $x = -1;         // all 64 bits set
let $y = x >>> 60;   // 15
let $z = x >> 60;    // -1
```

Precedence and associativity are identical to `<<` and `>>`: level 6 in the precedence table, left-associative, binding tighter than the range operators and looser than `+`/`-`.

### 3. The operator trait

`>>` dispatches through `Shr`, which the spec references (`operator-rules.md:537`, `14-expressions.md:527`, `10-declarations.md:28`) but never writes out. The declaration is in the prelude:

```ori
// library/std/prelude.ori:221-225 (existing, unchanged)
pub trait Shr<Rhs = int> {
    type Output

    @shift_right (self, rhs: Rhs) -> Self.Output
}
```

The `Rhs = int` default is what makes `byte >> int -> byte` well-typed today. `>>>` gets an exact sibling, added to `library/std/prelude.ori` immediately after `Shr`:

```ori
// Logical right shift: a >>> b -> a.shift_right_logical(rhs: b)
pub trait ShrLogical<Rhs = int> {
    type Output

    @shift_right_logical (self, rhs: Rhs) -> Self.Output
}
```

Method name follows `operator-method-naming-proposal.md` (approved), which replaced abbreviations with descriptive verb phrases (`shl` -> `shift_left`, `shr` -> `shift_right`); trait names stay short (`Shl`, `Shr`, `BitXor`), so `ShrLogical` is the consistent trait spelling.

Built-in implementations: `impl int: ShrLogical` and `impl byte: ShrLogical`, both with `type Output = Self`, registered in `compiler/ori_registry/src/defs/int.rs` and `compiler/ori_registry/src/defs/byte.rs` alongside the existing `shr` entries (`byte.rs:135-141` is the shape to mirror).

A user type implementing `ShrLogical` gets `>>>` on its own terms, exactly as `Shr` works today. This proposal pins the built-in `impl int: Shr` and `impl int: ShrLogical` semantics; it does not and cannot constrain what a user impl computes.

### 4. Compound assignment `>>>=`

`a >>>= b` desugars to `a = a >>> b` at parse time, matching every other compound-assignment form (`operator-rules.md:565-569`; the parse-time desugar site is `ori_parse`). Target mutability rules are unchanged: the target must be a non-`$` binding.

Conformance pin sits beside the existing `>>=` pins at `tests/spec/expressions/compound_assignment.ori:252-272`.

### 5. Const-evaluation admission

`grammar.ebnf:683-687` defines `const_expr` with an explicit operator list that includes `<<` and `>>`. `>>>` is added to that list:

```ebnf
const_expr = literal
           | "$" identifier
           | const_expr ( arith_op | comp_op | "&&" | "||" | "&" | "|" | "^" | "<<" | ">>" | ">>>" ) const_expr
           | unary_op const_expr
           | "(" const_expr ")" .
```

`const-generic-bounds-proposal.md:382` lists `<<` and `>>` among bitwise operators admitted in bound expressions; `>>>` joins them. Const evaluation of `>>>` follows the same overflow discipline as `>>`: an out-of-range shift count is a compile-time error rather than a runtime panic, per that proposal's `E1033` constant-overflow rule.

### 6. Error handling

`>>>` inherits `>>`'s panic contract verbatim (`operator-rules.md:337`, `14-expressions.md:415-424`):

- Negative shift count: panic.
- Shift count at or beyond the operand's bit width (64 for `int`, 8 for `byte`): panic.
- No overflow condition: a logical right shift cannot lose the value into an unrepresentable range, so unlike `<<` there is no shift-overflow panic.

The diagnostic text for a `>>>` count violation names `>>>` explicitly rather than reusing the `>>` message, so a reader can tell which operator trapped.

### 7. Value-range transfer function — `ori_repr`

`compiler/ori_repr/src/range/transfer/bitwise.rs:52-64` computes `range_shr` with `four_corner_fold` (defined at `compiler/ori_repr/src/range/transfer/mod.rs:70-90`, which folds the four `(value-endpoint, shift-endpoint)` products and takes their min and max). That is sound for arithmetic shift because, at a fixed shift count, `x >> k` is monotonically non-decreasing in the signed value `x`.

**A four-corner fold is NOT sound for `>>>`.** Logical shift is monotone in the *unsigned* interpretation of the value, and the signed interval `[-1, 0]` is not an unsigned interval. Folding its corners gives `min(-1 >>> 1, 0 >>> 1) = min(2^63 - 1, 0) = 0` and `max(...) = 2^63 - 1`, which happens to be wide here; but the general failure is real: for `[-1, -1] >>> 1` the fold is exact, while for a mixed-sign interval the fold's endpoints are computed from values that are not the unsigned extremes of the interval. Any consumer that narrows a type or elides an overflow check on such a range (`compiler/ori_repr/src/range/transfer/mod.rs:269` dispatch; `narrowing/overflow.rs`) would act on an interval that excludes reachable values. That is a miscompilation vector, not a specification nicety.

The correct transfer function, stated explicitly:

```
range_shr_logical(a: ValueRange, b: ValueRange) -> ValueRange:
    if b is not Bounded, or b.lo < 0, or b.hi >= 64: return Top
    if a is not Bounded: return Top

    # Split the value interval at the sign boundary. Within each half,
    # signed order and unsigned order agree, so a corner fold is exact.
    halves = []
    if a.lo <= -1 and a.hi >= 0:
        halves = [ (a.lo, -1), (0, a.hi) ]
    else:
        halves = [ (a.lo, a.hi) ]

    result = Bottom
    for (lo, hi) in halves:
        corners = [ logical_shr(lo, b.lo), logical_shr(lo, b.hi),
                    logical_shr(hi, b.lo), logical_shr(hi, b.hi) ]
        result = join(result, Bounded { lo: min(corners), hi: max(corners) })
    return result

logical_shr(v: i64, k: i64) -> i64 = ((v as u64) >> k) as i64
```

Soundness argument, stated so a reviewer can check it: for a fixed `k`, `logical_shr(., k)` is monotone non-decreasing in the unsigned value; each half of the split is an interval under unsigned order (the negative half maps to `[2^63, 2^64)` in increasing order, the non-negative half to `[0, 2^63)`), so the half's endpoints are its unsigned extremes and the endpoint pair bounds the half. For a fixed `v`, `logical_shr(v, .)` is monotone non-increasing in `k`, so the shift endpoints bound the shift dimension. Joining the two halves covers the whole input interval.

`k == 0` is the case that makes the split necessary rather than cosmetic: `x >>> 0 == x`, so the negative half's result is negative and a single fold spanning the sign boundary would report a hull that is correct only by accident of `Top`-like width. The split makes correctness structural.

### 8. Representation optimization

`annex-e-system-considerations.md:27` permits the compiler to use a narrower machine representation than the canonical 64-bit `int`. `>>>` is width-sensitive: `-1 >>> 1` is `2^63 - 1` at 64 bits and `2^31 - 1` at 32 bits.

The **semantic** width, not the representation width, is normative. `representation-optimization-proposal.md:96-118` already establishes this as the as-if rule — condition 2 requires operation preservation and condition 3 forbids any conforming program from distinguishing the optimized representation. Its canonical-representation table (`:126`) already records `int`'s contract as "64-bit signed two's complement ... Bitwise operations treat as 64 unsigned bits". `>>>` is a bitwise operation under that sentence, so no rule change is needed; the errata below records that `>>>` is in scope so a future narrowing pass cannot read the omission as license.

### 9. Formatter

`>>>` is a binary operator: spaces on both sides, break-before-operator when a line exceeds 100 characters, per `annex-d-formatting.md`. `>>>=` follows the compound-assignment spacing rule. No new formatting decision is required.

---

## Drawbacks

- **A third shift operator.** Readers must now distinguish `>>` from `>>>`, and the distinction is invisible at a glance in dense bit code. Java, C#, and JavaScript accepted this cost; the alternative — one operator whose fill depends on a type the language does not have — is worse.
- **Grammar surface.** `>>>` is a new token, and `>>>=` a second. The lexer must not mis-tokenize `a >> >b` or a future generic-close sequence; Ori has no `<T>>` closing hazard today (generics close with `>` and the parser is not template-ambiguous like C++), but the maximal-munch rule for `>`-runs must be stated when the token is added.
- **A second range-transfer function.** `range_shr_logical` is genuinely harder to get right than `range_shr`, as §7 shows. That is a real implementation cost and the highest-risk item in this proposal.
- **The distinction can be forgotten.** A user writing `>>` where `>>>` was meant gets a silently wrong answer for negative inputs, which is exactly today's failure mode. This proposal makes the correct spelling available; it cannot make the wrong one loud. A lint suggesting `>>>` when a shifted value is subsequently masked with a positive constant is a candidate follow-up, not part of this proposal.

---

## Alternatives Considered

### Alternative 1: Redefine `>>` as logical

Rejected. It is a breaking change to shipped, pinned behavior:

- It invalidates `tests/spec/expressions/operators_bitwise.ori:234-239` and `compiler/ori_eval/src/tests/operators_tests.rs:397-402`.
- It silently changes `hash_combine`, which `comparable-hashable-traits-proposal.md:228-234` defines in pure Ori as `seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))`. Every derived `Hashable` hash value would change, across the spec, the approved proposal, `library/std/prelude.ori`, and LLVM derive codegen (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops/wrapper_cmp.rs:336` emits `ashr` for that `>> 2`).
- It makes `range_shr`'s existing four-corner fold unsound (§7), turning a spec change into a miscompilation vector in already-shipped code.
- It contradicts near-universal prior art (see Prior Art).

Choosing `>>>` discharges every one of these at zero migration cost.

### Alternative 2: Leave `>>` fill unspecified and require callers to mask

Rejected. The mask depends on the shift count, and constructing it is itself panic-prone (`(1 << (64 - k)) - 1` traps at `k == 0` under the existing left-shift overflow rule). It also leaves the base operator's meaning undefined, so two conforming implementations could disagree.

### Alternative 3: A `std.math` function instead of an operator

`shift_right_logical(value:, amount:)` as a plain function needs no grammar change. Rejected as the primary mechanism: bit-mixing code is operator-dense, and every published reference for these algorithms is written in operator form. A function call at every shift makes transcription errors harder to catch by inspection — the same reviewability argument that governs 64-bit constants in `wide-integer-literals-proposal.md`. The function form remains expressible as a library wrapper over the operator for anyone who prefers it.

### Alternative 4: An unsigned 64-bit type

Rejected. `annex-e-system-considerations.md:29` commits to a single signed integer type. A `u64` would introduce conversion rules, literal-suffix syntax, and mixed-arithmetic questions across the entire numeric surface — disproportionate to expressing a fill mode.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:**

- A new operator is grammar surface: a lexer token, a parser production in `shift_expr` (`grammar.ebnf:489`), a `BinaryOp` variant, a const-expression admission.
- The fill semantics are enforced by the evaluator and by codegen; they are not expressible as a library definition over existing operators (that is precisely the gap this proposal fills).
- The `ori_repr` range-transfer function is compiler-internal static analysis.

**Missing features that would enable purity:** Not applicable — this proposal *is* the missing-feature request. Its output is that a class of bit-manipulation algorithms becomes writable as pure Ori stdlib rather than as compiler built-ins.

**Recommendation:** Proceed as a minimal compiler and spec change. The trait declaration lands in `library/std/prelude.ori`; the operator, its transfer function, and its normative text land in the compiler and spec.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `grammar.ebnf:80` `bit_op` | Add `">>>"` |
| `grammar.ebnf:489` `shift_expr` | `shift_expr = add_expr { ( "<<" \| ">>" \| ">>>" ) add_expr } .` |
| `grammar.ebnf:683-687` `const_expr` | Add `">>>"` to the binary-operator alternation |
| Compound-assignment production | Add `">>>="` alongside `">>="` |
| Clause 14 (`14-expressions.md`) | State `>>` arithmetic for `int` / zero-fill for `byte`; define `>>>`; add `>>>` to the precedence table at level 6; add `a >>> b` -> `ShrLogical` to the desugaring table at `:527` |
| Annex B (`operator-rules.md`) | Add `n1 >>> n2 => shift_right_logical` to the EVALUATION block at `:337`; add `>>> -> ShrLogical -> shift_right_logical(self, rhs:)` to the dispatch table at `:537`; add `>>>= desugars via ShrLogical` at `:569`; state the fill rule for both `>>` and `>>>` |
| Annex D (`annex-d-formatting.md`) | Add `>>>` / `>>>=` to the binary-operator and compound-assignment spacing lists |
| Annex E (`annex-e-system-considerations.md`) | No normative change; `:29`'s "bitwise operations treat the value as unsigned bits" is the sentence `>>>` realizes for right shift, while `>>` remains the sign-propagating form. A NOTE distinguishing the two removes the ambiguity that sentence currently carries |
| Clause 10 (`10-declarations.md:28`) | Add `ShrLogical` to the operator-trait list |
| `library/std/prelude.ori` | Declare `pub trait ShrLogical<Rhs = int>` after `Shr` at `:221-225` |
| Error codes | One new diagnostic for a `>>>` shift-count violation, or a parameterized reuse of the existing shift diagnostic naming the operator |

### Pre-existing contradiction — noted, not changed

`14-expressions.md:424` states valid left-shift counts are `0 to 62`; `operator-rules.md:341` states `int width = 64 bits (valid shift: 0..63)`. These disagree about `<<`. This proposal touches neither statement and does not depend on the resolution: `>>` and `>>>` are both `0..63` under both documents. The contradiction is recorded here so a reviewer does not read its survival as an oversight.

### Conformance pins — enumerated

Searched `tests/spec/**`, `compiler/ori_eval/**`, `compiler/ori_llvm/**`, `compiler/ori_repr/**` for `>>` and `Shr` behavior pins. Pins found and their disposition under this proposal:

| Pin | Asserts | Disposition |
|---|---|---|
| `tests/spec/expressions/operators_bitwise.ori:234-239` | `-16 >> 3 == -2` | UNCHANGED (this proposal keeps `>>` arithmetic) |
| `tests/spec/expressions/operators_bitwise.ori:241-244` | `1 >> 64` panics | UNCHANGED |
| `compiler/ori_eval/src/tests/operators_tests.rs:397-402` | `i64::MIN >> 63 == -1` | UNCHANGED |
| `tests/spec/expressions/compound_assignment.ori:252-272` | `>>=` basic + chain | UNCHANGED |

No pin is invalidated. New pins required: `>>>` on negative values, `>>>` at count 0, `>>>` at count 63, `>>>` panic at count 64 and at negative counts, `>>>` on `byte`, `>>>=` basic and chain, `>>>` in a const-generic bound, evaluator/LLVM parity on all of the above, and `ori_repr` range-transfer unit tests covering the sign-boundary-spanning interval at counts 0, 1, and 63.

---

## Prior Art

Cross-language behavior of `>>` on a signed operand:

| Language | `>>` on signed | Separate logical shift |
|---|---|---|
| Java | arithmetic | `>>>` |
| C# | arithmetic | `>>>` (added in C# 11) |
| JavaScript | arithmetic | `>>>` |
| Rust | arithmetic on signed types, logical on unsigned | none — the operand type selects |
| Go | arithmetic on signed types, logical on unsigned | none — the operand type selects |
| Swift | arithmetic on signed types, logical on unsigned | none — the operand type selects |
| Zig | arithmetic on signed types, logical on unsigned | none — the operand type selects |
| C++20 | arithmetic (mandated since C++20; implementation-defined for negative left operands in earlier standards and in C) | none — the operand type selects |

Two clusters, and Ori falls in the first by construction:

- Languages with **both** signed and unsigned integer types (Rust, Go, Swift, Zig, C, C++) need no second operator: the operand's type selects the fill. Ori deliberately has no unsigned type (`annex-e:29`), so this route is closed.
- Languages with **one** signed integer type at the operator level (Java, JavaScript) — and C#, which has unsigned types but also targets a JVM-style bytecode idiom — added `>>>`. Java added it precisely because `>>` is arithmetic and there was no unsigned type to switch on. That is Ori's exact situation, and `>>>` is the settled mainstream answer to it.

Verified issue-corpus entries:

- `zig#20367` *"Proposal: Make right shift mode explicit for signed integers"* — **open**. A language whose signed shift mode is implicit, still litigating whether to make it explicit.
- `zig#5220` *"Proposal: Explicit Shift Operators"* — **open**. Same pressure, broader scope.
- `go#19113` *"proposal: spec: allow signed shift counts"* — **closed, completed**. Shift semantics needing explicit spec text rather than being left to inference; about shift COUNTS, not fill mode, and cited only for that narrower point.

Deliberately NOT cited: `go#44664` *"spec: clarify that signed integers>=0 are permitted as shift counts"* is a closed PR about shift counts, not fill mode; the withdrawn `pure-bit-operations-proposal.md` cited it as evidence about fill semantics, which it does not support.

Not verifiable: a fourth Zig issue (`zig#21709`) was suggested during review as the most on-point hit. It does not appear in this repository's issue corpus under any search performed, so it is not cited here. A reviewer with upstream access may add it.

---

## Migration / Breaking Changes

None. Every change is additive:

- `>>` behavior is unchanged; only its specification becomes explicit.
- `>>>` and `>>>=` are new tokens that no existing program can contain, because `>>>` does not appear anywhere in the corpus today (searched `docs/ori_lang/v2026/spec/`, `docs/ori_lang/proposals/`, `library/`, `compiler/`, `tests/`; the only hits are prose in the withdrawn `pure-bit-operations-proposal.md` and an unrelated nested-generic type name in `existential-types-proposal.md:39`).
- `ShrLogical` is a new prelude trait name; no existing identifier collides (searched `library/`, `compiler/`, `tests/`, `docs/`).
- `range_shr` is untouched; `range_shr_logical` is a new sibling.
- `hash_combine` and every derived `Hashable` value are unchanged.

---

## Roadmap Impact

Implementation crosses the lexer, parser, type system/registry, evaluator, LLVM codegen, `ori_repr`, formatter, and spec. A feature plan scaffolded on approval owns the phase breakdown. The `ori_repr` transfer function is the item requiring the most care and should carry its own section with dedicated unit-test coverage of the sign-boundary case.

---

## Unresolved Questions

- **Lexing `>`-runs.** `>>>` requires a maximal-munch decision for sequences of `>`. Ori's generics are not template-ambiguous, so no `>>` closing hazard exists today, but the tokenization rule should be stated explicitly when the token lands rather than left to the lexer's incidental behavior.
- **`byte >>>`.** Defined here as identical to `byte >>` (both zero-fill), so generic bit helpers behave uniformly. An alternative is to reject `>>>` on `byte` as redundant. Uniformity is proposed; the reduction is defensible.
- **Diagnostic-code allocation.** Whether the `>>>` shift-count violation gets a new `E0xxx`/`E6xxx` code or reuses the existing shift diagnostic parameterized by operator name is an `ori_diagnostic` decision left to implementation.
- **A `>>`-vs-`>>>` lint.** Whether the compiler should warn when a `>>` result is immediately masked with a positive constant — a strong signal that zero fill was intended — is deliberately out of scope here.
