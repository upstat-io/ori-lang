# Proposal: Logical Shift Operator `>>>`

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 14, Annex A grammar, Annex B operator-rules, Annex E), compiler (parser operator synthesis, type system, registry, evaluator, VM, LLVM codegen, `ori_canon` const-fold, `ori_repr` range analysis), stdlib (prelude trait), formatter
**Depends On:** none
**Amends:** const-generic-bounds-proposal.md (approved — errata), compound-assignment-proposal.md (approved — errata), operator-traits-proposal.md (approved — errata), representation-optimization-proposal.md (approved — errata)
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors), wide-integer-literals-proposal.md (sibling successor), std-math-bit-operations-proposal.md (sibling successor — the largest `>>>` consumer in the corpus), wrapping-shift-proposal.md (draft — supplies the non-panicking LEFT shift this proposal deliberately does not; required by three of the five consumers below), stdlib-random-rng-proposal.md (draft — a motivating consumer), stdlib-math-api-proposal.md (draft — its written-out PCG generator depends on zero-fill shift), limbs-trait-proposal.md (draft — cross-limb shifts depend on zero-fill shift), stdlib-json-native-parser-proposal.md (draft — bit-scanning consumer), comparable-hashable-traits-proposal.md (approved — defines `hash_combine` over `<<` and `>>`; unaffected by this proposal), intrinsics-v2-byte-simd-proposal.md (approved — governing `Intrinsics` declaration; unaffected), operator-method-naming-proposal.md (approved — supplies the method-naming convention this proposal follows)

---

## Summary

Ori's `>>` is an arithmetic (sign-propagating) right shift in every executor today, but no normative spec text says so, and Annex E currently says something that contradicts it. This proposal does two things. First, it states the existing behavior normatively — `>>` shifts the two's-complement bit pattern right and replicates the sign bit into vacated high bits for `int`, while `byte >>` fills with zero because `byte` is unsigned — and amends Annex E normatively so the contradiction is removed. Second, it adds `>>>`, a distinct logical (zero-fill) right shift, so pure bit-manipulation algorithms can express a zero-fill shift without masking gymnastics. No existing program changes meaning.

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

This example is the sharpest illustration of the fill problem. It is not an example this proposal makes compile. Three separate things block it today and this proposal removes exactly one:

| Blocker in the example | Owner |
|---|---|
| `>> 30` and `>> 27` mean sign-fill | **this proposal** |
| `0xBF58476D1CE4E5B9` exceeds the literal range and is a compile error | `wide-integer-literals-proposal.md` |
| `wrapping_mul` is approved but unshipped — `library/std/math/mod.ori` is a 47-line all-comment stub | `overflow-behavior-proposal.md` implementation |

Stating that plainly is the point: the fill defect is real and independent, and it is the only one this proposal claims.

There is no correct rewriting of a zero-fill shift that is also readable. Masking after the shift requires a `k`-dependent mask the caller must construct, and the natural construction `(1 << (64 - k)) - 1` itself traps at two boundary values under the existing left-shift rules: at `k == 0` the count is `64`, which exceeds the operand's bit width (`14-expressions.md:419`, `1 << 64`), and at `k == 1` the expression is `1 << 63`, which is a shift overflow (`14-expressions.md:418`). Both are panics, for two different reasons.

### Consumers blocked today

Corpus facts, read from the cited lines:

| Consumer | Line | What it needs | Unblocked by `>>>` alone? |
|---|---|---|---|
| `drafts/std-math-bit-operations-proposal.md:33,38-42` | `rotate_left` and `count_ones` reference bodies | `>>>` **and** a non-panicking `<<` | **No** |
| `drafts/stdlib-math-api-proposal.md:604-606` | a written-out PCG using `>> 18`, `>> 27`, `>> 59` | `>>>` **and** a non-panicking `<<` — `:606` is `(xorshifted >> rot) \| (xorshifted << ((-rot) & 31))` | **No** |
| `drafts/limbs-trait-proposal.md:319` | cross-limb `a >> 1` | `>>>`; but `:318`, one line above, is `a << 64` | **No** |
| `drafts/stdlib-random-rng-proposal.md:149` | `[0.0, 1.0)` float from "the top 53 bits of a 64-bit draw" | `>>>` only | **Yes** |
| `drafts/stdlib-json-native-parser-proposal.md:451` | `in_string = (string_mask >> 63) & 1 == 1` — a mask scan assuming zero fill | `>>>` only | **Yes** |

**`>>>` alone does not unblock three of these five.** Each of the first three needs a left shift that may set bit 63 without trapping, which no approved or draft proposal supplied when this document was written. `wrapping-shift-proposal.md` is the sibling that does, and it is a genuine prerequisite for those three, not a nicety. What `>>>` unblocks on its own is the right-shift half of every one of them plus the two consumers that need nothing else. This is stated here rather than buried because the withdrawn predecessor's central defect was overstating what one change delivers.

Two citation corrections carried from review of an earlier revision:

- `drafts/limbs-trait-proposal.md:318` shifts a `U256`, a `Limbs` type, not an `int`. `14-expressions.md:424`'s shift-count rule is scoped to `int`, so `a << 64` is not directly a violation of that clause — but the limbs draft states no shift-count rule for `Limbs` types at all, so its correctness under any fill convention is unestablished. It is listed as a consumer of zero-fill semantics on the strength of `:319`, not `:318`.
- `drafts/stdlib-json-native-parser-proposal.md:747` was cited in an earlier revision. That line is a bullet in a roadmap tree diagram, not bit-scanning code. The real consumer is `:451`, cited above.

### When This Matters

Any algorithm treating an `int` as 64 unsigned bits: hashing, PRNG, bit-set iteration, encoding and decoding, fixed-point normalization, bignum limb manipulation. Each is a natural stdlib citizen under the lean-core principle; none can be written correctly today.

---

## Goals and Non-Goals

**Goals:**

- Add `>>>` as a distinct logical (zero-fill) right-shift operator, with its compound-assignment form `>>>=`.
- State normatively, where the operator is defined, that `>>` on `int` is arithmetic (sign-propagating) and that `>>` on `byte` is zero-fill.
- Amend `annex-e-system-considerations.md:29` **normatively** so its unsigned-bits sentence and the sign-propagating right shift stop contradicting each other.
- Define the operator's trait, const-evaluation admission, panic contract, and value-range transfer function.
- Keep every change additive: no existing program changes meaning; no existing conformance pin is invalidated.

**Non-Goals:**

- **Changing `>>`.** Its behavior is unchanged; only its specification changes from silent-and-contradicted to explicit.
- **An unsigned integer type.** Annex E's single-signed-type decision stands.
- **A logical LEFT shift.** `<<` vacates low bits, which are filled with zero under every fill convention, so there is no logical-versus-arithmetic question for `<<`.
- **Changing `<<`'s overflow contract.** `<<` panics on overflow today. Making that non-panicking is `wrapping-shift-proposal.md`, deliberately kept separate — see Alternative 5.
- **Reconciling the pre-existing `0..62` versus `0..63` left-shift-count contradiction.** Noted in Spec & Grammar Impact; neither statement is touched.
- **Moving bit operations out of `Intrinsics`** — that is `std-math-bit-operations-proposal.md`.
- **64-bit literal patterns** — that is `wide-integer-literals-proposal.md`.

---

## Design

### 1. `>>` is arithmetic — stated, not changed

Grounding for the behavior claim, from implementation and conformance pins rather than spec prose:

| Layer | Evidence | Verdict |
|---|---|---|
| Conformance corpus | `tests/spec/expressions/operators_bitwise.ori:234-239` — `@shr_negative_value () -> int = -16 >> 3;` asserted `expected: -2`, source comment `// Arithmetic right shift preserves sign` | arithmetic |
| Evaluator | `compiler/ori_eval/src/operators/mod.rs:255` — `BinaryOp::Shr => a.checked_shr(b)` on a signed `i64` | arithmetic |
| Evaluator unit pin | `compiler/ori_eval/src/tests/operators_tests.rs:397-402` — `i64::MIN >> 63 == -1` | arithmetic |
| VM | `compiler/ori_vm/src/execute/primitives.rs:110-124` — `shift` calls `left.as_int()?.checked_shr(amount)` on `i64` | arithmetic |
| LLVM codegen | `compiler/ori_llvm/src/codegen/ir_builder/checked_ops/shift.rs:156` — `build_right_shift(lhs_int, rhs_int, true, name)`; the `true` selects `ashr` | arithmetic |
| Range analysis | `compiler/ori_repr/src/range/transfer/bitwise.rs:52-64` — `range_shr` documented and implemented as arithmetic right shift | arithmetic |
| `byte` evaluator | `compiler/ori_eval/src/operators/mod.rs:401-407` — the shift is applied to a `u8` | zero-fill |
| Spec prose | `operator-rules.md:337` states only panic conditions; `14-expressions.md:415-424` states only shift-count validity | silent |
| Annex E | `annex-e-system-considerations.md:29` — "Bitwise operations treat the value as unsigned bits" | **contradicts every row above** |

Add to `14-expressions.md` and to the `operator-rules.md` shift definition:

- For `int`, `>>` performs an arithmetic right shift: the two's-complement bit pattern shifts right and each vacated high bit takes the value of the original sign bit.
- For `byte`, `>>` performs a zero-fill shift, because `byte` is an unsigned 8-bit type.

```ori
let $a = -16 >> 3;            // -2   (sign propagates)
let $b = 0xF0 as byte >> 4;   // 0x0F (byte is unsigned; zero fill)
```

The `byte` half of this becomes normative over **zero** conformance coverage. `tests/spec/expressions/operators_bitwise.ori:255-268` has the entire byte block commented out, listing `byte >> int` among the TODO cases, with the stated reason "the `as` type conversion operator is not yet implemented in the parser". That reason is stale: `as` **is** implemented (`compiler/ori_parse/src/grammar/expr/postfix/mod.rs:385` produces `ExprKind::Cast`, exercised at `tests/spec/expressions/type_conversion.ori:67-85`). Reviving and completing that block is a required deliverable here, not a follow-up — see Conformance pins. The pins table below reads clean only because the newly-normative surface is currently untested; that is a reason to write tests, not a reason to claim coverage.

### 2. `>>>` is a logical right shift

`a >>> b` shifts `a`'s two's-complement bit pattern right by `b` places and fills every vacated high bit with zero, for every `int` value including negative ones. On `byte`, `>>>` is defined identically to `>>` (both zero-fill), so a generic bit-manipulation helper written over `>>>` behaves uniformly across both primitives.

```ori
let $x = -1;         // all 64 bits set
let $y = x >>> 60;   // 15
let $z = x >> 60;    // -1
```

Precedence and associativity are identical to `<<` and `>>`: the shift level in the precedence table, left-associative, binding tighter than the range operators and looser than `+` and `-`.

**`>>>` returns `int`.** It is not a coercion to an unsigned type — Ori has none. `-1 >>> 0 == -1`, not `18446744073709551615`. This differs from JavaScript's `>>>`, whose result is a `uint32`, and the difference has already produced a transcription defect in a sibling draft; see Prior Art.

### 3. Tokenization — parser-side synthesis, not a lexer token

**`>>>` is not a lexer token, and neither is `>>`.** The shipped lexer scans every `>` as a single token: `compiler/ori_lexer_core/src/raw_scanner/mod.rs:100` is `b'>' => self.single(start, RawTag::Greater)`, with no lookahead. There is no `Shr` token in the token stream at all.

This is deliberate and load-bearing. `compiler/ori_parse/src/grammar/expr/operators.rs:16-21` records the reason: *"The lexer produces individual `>` tokens to enable parsing nested generics like `Result<Result<T, E>, E>`."* The regression it fixes is pinned at `compiler/ori_parse/src/grammar/ty/tests.rs:234-240`, whose comment reads *"This was previously broken because `>>` was lexed as a single `Shr` token."*

So Ori has no `<T>>` closing hazard **because the lexer refuses maximal munch**, not because the grammar is unambiguous. An earlier revision of this document asserted the latter and then proposed adding a maximal-munch `>>>` token — which would reintroduce precisely the defect the lexer design eliminated. `Option<Option<Option<int>>>` scans as three `Greater` tokens; a maximal-munch `>>>` token would consume them as a shift inside a type and fail `ty/tests.rs:234-240`. Both the claim and the mechanism are **retracted**.

Compound shift operators are synthesized in the parser from adjacent single `>` tokens, using no-whitespace adjacency predicates in `compiler/ori_parse/src/cursor/mod.rs`:

| Predicate | Line | Tokens | Adjacency checked |
|---|---|---|---|
| `is_shift_right` | `:259` | `>` `>` | `next_is_adjacent()` |
| `is_greater_equal` | `:268` | `>` `=` | `next_is_adjacent()` |
| `is_shift_right_assign` | `:277` | `>` `>` `=` | `next_is_adjacent()` **and** `flags[pos + 2].is_adjacent()` |

`>>>` adds a fourth predicate (three `>`, adjacency on both gaps) and `>>>=` a fifth (four tokens, adjacency on all three gaps).

**The ordered greedy chain is the real obligation.** `compiler/ori_parse/src/grammar/expr/operators.rs:192-204` dispatches `TAG_GT` in a fixed order, with the comment *"Check `>>=` first: if we greedily match `>>` here, `parse_expr_inner` can't see `>>=`."* Longer forms must be tested before their prefixes. The extended order:

| Order | Form | Tokens | Action in `infix_binding_power` |
|---|---|---|---|
| 1 | `>>>=` | 4 | return `None` — defer to the compound-assignment path |
| 2 | `>>>` | 3 | return `(SHIFT, SHIFT, BinaryOp::ShrLogical, 3)` |
| 3 | `>>=` | 3 | return `None` — defer (existing) |
| 4 | `>=` | 2 | return `(COMPARISON, ..., BinaryOp::GtEq, 2)` (existing) |
| 5 | `>>` | 2 | return `(SHIFT, SHIFT, BinaryOp::Shr, 2)` (existing) |
| 6 | `>` | 1 | fall through to `OPER_TABLE` (existing) |

Steps 4 and 5 are mutually disjoint — they differ at the second token — so their relative order is free, which is why the shipped code may check `>=` before `>>`. Every other pair stands in a prefix relation and the order above is forced. `compiler/ori_parse/src/grammar/expr/mod.rs:133-136` gains a `>>>=` arm beside the existing `is_shift_right_assign` arm, consuming four tokens instead of three.

Generic-close disambiguation is unaffected: type context never enters `infix_binding_power`, and the new predicates are the same adjacency-gated shape the existing `>>` synthesis already uses.

**Consequence for Migration.** The claim that `>>>` and `>>>=` are new forms no existing program can contain is true under parser-side synthesis and would be **false** under a lexer token, because a lexer token changes how existing `>` runs in type position are scanned. Migration's non-breaking claim rests entirely on this section.

### 4. The operator trait

`>>` dispatches through `Shr`. The declaration lives in the prelude, not the spec:

```ori
// library/std/prelude.ori:221-225 (existing, unchanged)
pub trait Shr<Rhs = int> {
    type Output

    @shift_right (self, rhs: Rhs) -> Self.Output
}
```

The `Rhs = int` default is what makes `byte >> int -> byte` well-typed today. `>>>` gets an exact sibling, added immediately after `Shr`:

```ori
// Logical right shift: a >>> b -> a.shift_right_logical(rhs: b)
pub trait ShrLogical<Rhs = int> {
    type Output

    @shift_right_logical (self, rhs: Rhs) -> Self.Output
}
```

Method name follows `operator-method-naming-proposal.md` (approved), which replaced abbreviations with descriptive verb phrases (`shl` -> `shift_left`, `shr` -> `shift_right`); trait names stay short (`Shl`, `Shr`, `BitXor`), so `ShrLogical` is the consistent trait spelling.

**Registry naming — an existing divergence this proposal must not deepen.** `compiler/ori_registry/src/defs/int.rs:212-218` and `compiler/ori_registry/src/defs/byte.rs:135-141` both register the **abbreviated** `"shr"` with trait `Some("Shr")`, while `operator-rules.md:537` and `14-expressions.md:527` spell the method `shift_right`. The approved rename is unshipped, so registry and spec already disagree. This proposal does not resolve that dispute and does not get to be silent about it either: the `ShrLogical` registry entry **shall** use whichever spelling the `Shr` entry carries at implementation time, so the two move together when the rename lands. Registering `shift_right_logical` beside an abbreviated `shr` would make the divergence permanent by making it non-uniform.

A user type implementing `ShrLogical` gets `>>>` on its own terms, exactly as `Shr` works today. This proposal pins the built-in `impl int: Shr` and `impl int: ShrLogical` semantics; it does not and cannot constrain what a user impl computes.

**Recorded as unverified.** Whether `>>` on primitives currently routes through registry trait dispatch or a primitive fast path was not established. `MethodDef` entries exist at the two registry lines above, and `compiler/ori_types/src/operator.rs` and `compiler/ori_eval/src/interpreter/operator_dispatch.rs` both carry `BinaryOp::Shr` arms, but whether `ShrLogical` needs a registry entry, a type-checker fast path, or both is an implementation question this proposal does not settle. It is listed under Unresolved Questions rather than asserted.

### 5. `BinaryOp::ShrLogical` — the blast radius, enumerated

`compiler/ori_repr/src/range/transfer/mod.rs:236` documents *"Exhaustive match on both `BinaryOp` (23 variants)"* and `:121` records *"Exhaustive match (no `_` arm) ensures new variants cause compile errors."* Adding a variant to `compiler/ori_ir/src/ast/operators.rs:17` therefore produces a compile error at every consumer, which is the desired behavior and also the work item. Consumers carrying a `BinaryOp::Shr` arm today:

| Crate | Sites | Obligation |
|---|---|---|
| `ori_parse` | `grammar/expr/operators.rs`, `grammar/expr/mod.rs` | synthesis + compound-assignment desugar (§3, §6) |
| `ori_types` | `src/operator.rs` | operator-to-trait resolution |
| `ori_registry` | `defs/int.rs`, `defs/byte.rs` | `MethodDef` entries (§4) |
| `ori_canon` | `const_fold/mod.rs`, `const_fold/arithmetic.rs` | constant folding (§7) |
| `ori_eval` | `operators/mod.rs`, `interpreter/operator_dispatch.rs` | `int` and `byte` evaluation |
| `ori_vm` | `execute/primitives.rs`, `bytecode/op.rs` | opcode + execution |
| `ori_llvm` | `codegen/arc_emitter/operators/{mod.rs,strategy.rs}` | `lshr` emission |
| `ori_repr` | `range/transfer/{mod.rs,bitwise.rs}`, `narrowing/overflow.rs` | transfer function (§9) |

A conformance pin asserting that every executor agrees on `>>>` for the same inputs is listed under Conformance pins. Registration drift across these eight crates is the failure mode that pin exists to catch.

### 6. Compound assignment `>>>=`

`a >>>= b` desugars to `a = a >>> b` at parse time, matching every other compound-assignment form (`operator-rules.md:565-569`). The desugar site is `compiler/ori_parse/src/grammar/expr/mod.rs:133-136`, which calls `desugar_compound_assign` with an explicit token count — three for `>>=`, four for `>>>=`. Target mutability rules are unchanged: the target shall be a non-`$` binding.

Conformance pins sit beside the existing `>>=` pins at `tests/spec/expressions/compound_assignment.ori:252-272`.

### 7. Const-evaluation admission

`grammar.ebnf:683-687` defines `const_expr` with an explicit operator list that includes `<<` and `>>`. `>>>` joins it:

```ebnf
const_expr = literal
           | "$" identifier
           | const_expr ( arith_op | comp_op | "&&" | "||" | "&" | "|" | "^" | "<<" | ">>" | ">>>" ) const_expr
           | unary_op const_expr
           | "(" const_expr ")" .
```

`const-generic-bounds-proposal.md:382` lists `<<` and `>>` among the bitwise operators admitted in bound expressions; `>>>` joins them, and that proposal gains an erratum recording it.

Const evaluation of `>>>` follows the same discipline as `>>`: an out-of-range shift count is a compile-time error rather than a runtime panic. An earlier revision attributed that to `E1033`. **That is the wrong code.** `const-generic-bounds-proposal.md:382` maps `E1033` to **Overflow**, and §8 establishes that `>>>` has no overflow condition. The correct disposition is the shift-count diagnostic named in §8, raised at compile time in const position. Which existing const-evaluation code carries it is an `ori_diagnostic` decision left to implementation.

### 8. Error handling

`>>>` inherits `>>`'s panic contract verbatim (`operator-rules.md:337`, `14-expressions.md:415-424`):

- Negative shift count: panic.
- Shift count at or beyond the operand's bit width — 64 for `int`, 8 for `byte`: panic.
- **No overflow condition.** A logical right shift discards low bits and fills high bits with zero; every result is representable in the operand's type. Unlike `<<`, there is no shift-overflow panic.

The diagnostic for a `>>>` count violation names `>>>` explicitly rather than reusing the `>>` message, and states the operand's bit width and the offending count, so a reader can tell which operator trapped and why.

### 9. Value-range transfer function — `ori_repr`

`compiler/ori_repr/src/range/transfer/bitwise.rs:52-64` computes `range_shr` with `four_corner_fold` (`compiler/ori_repr/src/range/transfer/mod.rs:70-90`), which evaluates the operation at the four `(value-endpoint, shift-endpoint)` combinations and takes the signed min and max. That is sound for arithmetic shift because `x >> k` is monotone non-decreasing in the signed value `x` at fixed `k` and monotone in `k` at fixed `x`; a function monotone in each argument separately attains its extremes at the corners of a rectangle. **This proposal does not change `range_shr` and does not question its soundness.**

**A four-corner fold is NOT sound for `>>>`.** `logical_shr` is monotone in the *unsigned* interpretation of the value, and a signed interval spanning the sign boundary is not an interval under unsigned order. Worked counterexample at `a = [-4, 3]`, `k = 1` fixed:

| `v` | `-4` | `-3` | `-2` | `-1` | `0` | `1` | `2` | `3` |
|---|---|---|---|---|---|---|---|---|
| `v >>> 1` | `2^63-2` | `2^63-2` | `2^63-1` | `2^63-1` | `0` | `0` | `1` | `1` |

The corners are `v = -4`, giving `2^63-2`, and `v = 3`, giving `1`. The fold reports `[1, 2^63-2]`, which excludes the reachable minimum `0` **and** the reachable maximum `2^63-1`. Consumers at `compiler/ori_repr/src/range/transfer/mod.rs:269` and `compiler/ori_repr/src/narrowing/overflow.rs` narrow types and elide overflow checks on such a range. That is a miscompilation vector, not a specification nicety.

**Both dimensions shall be split.** Two single-dimension repairs are tempting and both are wrong:

- Splitting only the **value** interval at the sign boundary leaves `k == 0` wrong: at `k == 0` a negative value's result is negative, at `k >= 1` it is non-negative, so the two are incomparable in the signed order the fold uses. It is also inert on any single-value witness such as `a = [-1, -1]`.
- Splitting only the **shift** interval at `k == 0` leaves the counterexample above wrong, because that witness is at fixed `k = 1` and never engages a shift split. Enumeration found 294 unsound rectangles for this variant over a small domain, the first at `a = [-6, 0]`, `b = [0, 1]`.

The correct transfer function splits both:

```
range_shr_logical(a: ValueRange, b: ValueRange) -> ValueRange:
    if a is not Bounded, or b is not Bounded: return Top
    if b.lo < 0 or b.hi >= 64: return Top

    # Value dimension: split at the sign boundary, so signed order and
    # unsigned order agree within each half.
    value_halves = [ (a.lo, -1), (0, a.hi) ]   if a.lo < 0 and a.hi >= 0
                   else [ (a.lo, a.hi) ]

    # Shift dimension: split at k == 0, so every cell's results are either
    # all identity or all non-negative.
    shift_halves = [ (0, 0), (1, b.hi) ]       if b.lo == 0 and b.hi >= 1
                   else [ (b.lo, b.hi) ]

    result = Bottom
    for (vl, vh) in value_halves:
        for (kl, kh) in shift_halves:
            corners = [ logical_shr(vl, kl), logical_shr(vl, kh),
                        logical_shr(vh, kl), logical_shr(vh, kh) ]
            result = join(result, Bounded { lo: min(corners), hi: max(corners) })
    return result

logical_shr(v: i64, k: i64) -> i64 = ((v as u64) >> k) as i64
```

Soundness argument, stated cell by cell so a reviewer can check it. Each cell is one value half crossed with one shift half:

| Cell | Why the corner fold is exact there |
|---|---|
| non-negative values, any `k` | `v as u64 == v` and `v >>> k <= v < 2^63`, so signed and unsigned interpretations coincide on both input and output. Monotone non-decreasing in `v`, non-increasing in `k`. |
| negative values, `k == 0` | The operation is the identity, so the cell's image is the half itself, `[vl, vh]`. |
| negative values, `k >= 1` | `v as u64` lies in `[2^63, 2^64)` and is order-preserving with the signed order on an all-negative half; `(v as u64) >> k < 2^63` for `k >= 1`, so every output is non-negative and signed order agrees with unsigned order on outputs. Monotone non-decreasing in `v`, non-increasing in `k`. |

Each cell's fold is therefore exact, and the hull of a union of exact hulls is the exact hull of the union. The result is not merely sound but **exact** — strictly tighter than the precision class of the existing arithmetic-shift fold.

Verified by exhaustive enumeration over every `(a.lo, a.hi)` in `[-6, 6]` crossed with every `(b.lo, b.hi)` in `[0, 4]`, compared against the true image of each rectangle: zero unsound results and zero inexact results for the two-dimension split, against 294 unsound results for the shift-only variant on the same domain. A randomized sweep over intervals anchored at `int.min`, `int.max`, `-1`, `0`, and `1` with shift ranges drawn from `[0, 63]` found no unsound and no inexact result.

Presenting a wrong soundness proof as a checkable argument is worse than presenting none, so that enumeration is a required deliverable: the range-transfer unit tests under Conformance pins reproduce it, including both rejected single-dimension variants as negative controls.

### 10. Representation optimization

`annex-e-system-considerations.md:27` permits the compiler to use a narrower machine representation than the canonical 64-bit `int`. `>>>` is width-sensitive: `-1 >>> 1` is `2^63 - 1` at 64 bits and `2^31 - 1` at 32 bits.

The **semantic** width, not the representation width, is normative. `representation-optimization-proposal.md:96-118` establishes the as-if rule — condition 2 requires operation preservation, condition 3 forbids a conforming program from distinguishing the optimized representation. Its canonical-representation table at `:126` records `int`'s contract as 64-bit signed two's complement with bitwise operations treating the value as 64 unsigned bits.

That is sufficient for `>>>` **only if** shifts are inside the preserved-operation set, and the operation list at `representation-optimization-proposal.md:104-110` does not enumerate bitwise or shift operations. Arithmetic right shift is width-invariant under sign extension, so the omission has been harmless; `>>>` is not, so it stops being harmless. The errata below closes this explicitly rather than relying on the general sentence at `:126`.

### 11. Formatter

`>>>` is a binary operator: spaces on both sides, break-before-operator when a line exceeds 100 characters, per `annex-d-formatting.md`. `>>>=` follows the compound-assignment spacing rule. No new formatting decision is required.

---

## Drawbacks

- **A third shift operator.** Readers must distinguish `>>` from `>>>`, and the distinction is invisible at a glance in dense bit code. Java, C#, and JavaScript accepted this cost; the alternative — one operator whose fill depends on a type the language does not have — is worse. Kotlin declined the cost and paid a different one; see Prior Art.
- **Parser surface, not lexer surface.** Two new synthesis predicates and two new positions in an ordered greedy chain whose ordering is already load-bearing (§3). Less surface than a new lexer token, more than the "one new token" an earlier revision priced.
- **A second range-transfer function.** `range_shr_logical` is genuinely harder to get right than `range_shr`: two of the three obvious repairs are unsound (§9), and one of those was independently proposed by two reviewers of an earlier revision. It is the highest-risk item here.
- **A new `BinaryOp` variant crosses eight crates.** §5 enumerates them. Exhaustive matches turn the omissions into compile errors rather than silent gaps, which converts the risk from correctness into effort.
- **`byte >>` becomes normative over currently-dead test coverage**, whose stated reason for being dead is stale (§1).
- **The distinction can be forgotten.** A user writing `>>` where `>>>` was meant gets a silently wrong answer for negative inputs, which is exactly today's failure mode. This proposal makes the correct spelling available; it cannot make the wrong one loud. A lint suggesting `>>>` when a shifted value is subsequently masked with a positive constant is a candidate follow-up, not part of this proposal.

---

## Alternatives Considered

### Alternative 1: Redefine `>>` as logical

Rejected. It is a breaking change to shipped, pinned behavior:

- It invalidates `tests/spec/expressions/operators_bitwise.ori:234-239` and `compiler/ori_eval/src/tests/operators_tests.rs:397-402`.
- It silently changes `hash_combine`, which `comparable-hashable-traits-proposal.md:228-234` defines in pure Ori as `seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))`. Every derived `Hashable` hash value would change, across the spec, that approved proposal, `library/std/prelude.ori`, and LLVM derive codegen (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops/wrapper_cmp.rs:336` emits `ashr` for that `>> 2`).
- It makes `range_shr`'s existing four-corner fold unsound (§9), turning a spec change into a miscompilation vector in already-shipped code.
- It contradicts near-universal prior art (see Prior Art).

Choosing `>>>` discharges every one of these at zero migration cost.

### Alternative 2: Leave `>>` fill unspecified and require callers to mask

Rejected. The mask depends on the shift count, and constructing it traps at two boundary values (Motivation). It also leaves the base operator's meaning undefined — and worse than undefined, since Annex E:29 currently entitles a conforming implementation to zero-fill `>>` (§Errata). "Unspecified" is not the status quo; a live contradiction is.

### Alternative 3: A `std.math` function instead of an operator

`shift_right_logical(value:, amount:)` as a plain function needs no grammar change, and Kotlin's `ushr` is exactly this shape in a language with Ori's constraint. Rejected as the primary mechanism: bit-mixing code is operator-dense, and every published reference for these algorithms is written in operator form. A function call at every shift makes transcription errors harder to catch by inspection — the same reviewability argument that governs 64-bit constants in `wide-integer-literals-proposal.md`. The function form remains expressible as a library wrapper over the operator for anyone who prefers it. Kotlin's counter-example is engaged directly in Prior Art rather than dismissed here.

### Alternative 4: An unsigned 64-bit type

Rejected. `annex-e-system-considerations.md:29` commits to a single signed integer type. A `u64` would introduce conversion rules, literal-suffix syntax, and mixed-arithmetic questions across the entire numeric surface — disproportionate to expressing a fill mode.

### Alternative 5: Bundle the non-panicking left shift into this proposal

Rejected, though three of the five cited consumers need both. This proposal is additive and invalidates no pin; a change to `<<`'s overflow contract is neither, and it reopens the `0..62` versus `0..63` contradiction this proposal deliberately leaves alone. Bundling them forces one approval verdict onto two different risk profiles — the specific failure that sank the withdrawn predecessor, which bundled three items and let the unsound one drag down the salvageable ones. `wrapping-shift-proposal.md` owns it. The two are independently approvable in either order, and neither blocks the other.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:**

- A new operator is grammar surface: parser synthesis predicates, a production in `shift_expr` (`grammar.ebnf:489`), a `BinaryOp` variant, a const-expression admission.
- The fill semantics are enforced by every executor; they are not expressible as a library definition over existing operators. That is precisely the gap this proposal fills.
- The `ori_repr` range-transfer function is compiler-internal static analysis.

**Missing features that would enable purity:** Not applicable — this proposal *is* the missing-feature request. Its output is that the right-shift half of a class of bit-manipulation algorithms becomes writable as pure Ori stdlib rather than as compiler built-ins. The left-shift half needs `wrapping-shift-proposal.md`.

**Recommendation:** Proceed as a minimal compiler and spec change. The trait declaration lands in `library/std/prelude.ori`; the operator, its transfer function, and its normative text land in the compiler and spec.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `grammar.ebnf:80` `bit_op` | Add `">>>"` |
| `grammar.ebnf:489` `shift_expr` | `shift_expr = add_expr { ( "<<" \| ">>" \| ">>>" ) add_expr } .` |
| `grammar.ebnf:683-687` `const_expr` | Add `">>>"` to the binary-operator alternation |
| Compound-assignment production | Add `">>>="` alongside `">>="` |
| Clause 14 (`14-expressions.md`) | State `>>` arithmetic for `int` and zero-fill for `byte`; define `>>>`; add `>>>` to the precedence table at the shift level; add `a >>> b` -> `ShrLogical` -> `shift_right_logical(rhs:)` to the desugaring table at `:527` |
| Annex B (`operator-rules.md`) | Add `n1 >>> n2 => shift_right_logical` to the EVALUATION block at `:337`; add `>>> -> ShrLogical -> shift_right_logical(self, rhs:)` to the dispatch table at `:537`; add `>>>= desugars via ShrLogical` at `:569`; state the fill rule for both `>>` and `>>>` |
| **Annex E (`annex-e-system-considerations.md:29`)** | **Normative amendment** — see Errata |
| Annex D (`annex-d-formatting.md`) | Add `>>>` and `>>>=` to the binary-operator and compound-assignment spacing lists |
| Clause 10 (`10-declarations.md:28`) | Add `ShrLogical` to the operator-trait list |
| `library/std/prelude.ori` | Declare `pub trait ShrLogical<Rhs = int>` after `Shr` at `:221-225` |
| Error codes | One diagnostic for a `>>>` shift-count violation, or a parameterized reuse of the existing shift diagnostic naming the operator |

### Pre-existing contradiction — noted, not changed

`14-expressions.md:424` states that valid **left**-shift counts are `0 to 62` "when the result shall remain representable"; `operator-rules.md:341` states `int width = 64 bits (valid shift: 0..63)` with only a width gate and no result-overflow condition. The disagreement is confined to `<<`: both documents give right shift `0..63` (`14-expressions.md:424`, "For right shift, counts 0 to 63 are valid"). This proposal touches neither statement and does not depend on the resolution. It is recorded so a reviewer does not read its survival as an oversight. `wrapping-shift-proposal.md`, which changes `<<`'s contract, is the natural place to reconcile it and takes that obligation.

### Errata

Four approved proposals are amended, plus one normative spec edit. Each proposal gains an errata block per the project's errata convention; none is rewritten. An earlier revision of this document amended three approved proposals with **no errata rows at all** — the predecessor was withdrawn partly for aiming its errata at the wrong proposal, and omitting them entirely is not an improvement.

| Approved proposal | Erratum |
|---|---|
| `const-generic-bounds-proposal.md` | `:382`'s bitwise-operator set admitted in const-generic bound expressions gains `>>>`. Its `E1033` overflow mapping is unchanged and does **not** cover `>>>`, which has no overflow condition (§7, §8) |
| `compound-assignment-proposal.md` | Its compound-assignment operator set gains `>>>=`, desugaring via `ShrLogical`. Consistent with that proposal's own framing of compound assignment as pure syntactic sugar with no new semantics |
| `operator-traits-proposal.md` | Its operator-trait framework gains `ShrLogical`, with method `shift_right_logical(self, rhs:)` and default `Rhs = int` |
| `representation-optimization-proposal.md` | Its preserved-operation list at `:104-110` is recorded as covering bitwise and shift operations, so the semantic 64-bit width — not the representation width — governs `>>>` (§10). Its `:126` restatement of the Annex E sentence carries the same carve-out as the spec edit below, so the two do not drift |

**The Annex E amendment is normative, not a NOTE.** `annex-e-system-considerations.md:29` states, as normative text: *"There is no separate unsigned integer type. Bitwise operations treat the value as unsigned bits."* Making `>>` normatively sign-propagating contradicts it directly — an implementation reading only that sentence is entitled to zero-fill `>>`, which every shipped executor does not do.

Under the ISO/IEC Directives style the spec follows, a NOTE is informative and shall not carry requirements; it therefore cannot resolve a contradiction between two normative statements. An earlier revision of this document proposed exactly that disposition and it is **retracted**. Annex E:29 shall be amended so that its normative text carves out the right-shift fill rule: bitwise operations treat the value as unsigned bits, except that `>>` propagates the sign bit into vacated high positions, with `>>>` named as the zero-filling right shift. `representation-optimization-proposal.md:126` restates the same sentence in `int`'s canonical-representation contract and receives the identical carve-out through its erratum above.

### Conformance pins — enumerated

Searched `tests/spec/**`, `compiler/ori_eval/**`, `compiler/ori_vm/**`, `compiler/ori_llvm/**`, and `compiler/ori_repr/**` for `>>` and `Shr` behavior pins. Pins found and their disposition:

| Pin | Asserts | Disposition |
|---|---|---|
| `tests/spec/expressions/operators_bitwise.ori:234-239` | `-16 >> 3 == -2` | UNCHANGED — this proposal keeps `>>` arithmetic |
| `tests/spec/expressions/operators_bitwise.ori:241-249` | `1 >> 64` panics; `1 >> -1` panics | UNCHANGED |
| `compiler/ori_eval/src/tests/operators_tests.rs:397-402` | `i64::MIN >> 63 == -1` | UNCHANGED |
| `tests/spec/expressions/compound_assignment.ori:252-272` | `>>=` basic and chain | UNCHANGED |
| `tests/spec/expressions/operators_bitwise.ori:255-268` | **nothing — the byte block is commented out** | REVIVED (§1) |

No pin is invalidated. New pins required:

- `>>>` on negative values; at count `0` (identity, including `-1 >>> 0 == -1`, which distinguishes Ori from JavaScript); at count `63`; panic at count `64` and at negative counts.
- `byte >>` zero-fill, from the revived block — including the `0xF0 as byte >> 4 == 0x0F` example in §1, which has no test today.
- `byte >>>`, defined identically to `byte >>`.
- `>>>=` basic and chain, beside the existing `>>=` pins.
- `>>>` in a const-generic bound; an out-of-range `>>>` count in const position rejected at compile time.
- Tokenization regression pins: `Option<Option<Option<int>>>` still parses as a nested type — the `ty/tests.rs:234-240` shape extended to a three-`>` run; and `a >>> b`, `a >>>= b`, `a >>= b`, `a >= b`, and whitespace-separated `a > > b` each parse to the intended form.
- Cross-executor parity on every value case above: evaluator, VM, and LLVM/native shall agree bit-for-bit, along with any further admitted executor.
- `ori_repr` range-transfer unit tests reproducing §9's enumeration: the `a = [-4, 3]`, `b = [0, 2]` witness; the `a = [-1, -1]`, `b = [0, 63]` case; an exhaustive small-domain comparison against the true image asserting both soundness and exactness; and negative controls showing that the value-only and shift-only splits each fail.

---

## Prior Art

Cross-language behavior of `>>` on a signed operand:

| Language | `>>` on signed | Separate logical shift | Result type of the logical form |
|---|---|---|---|
| Java | arithmetic | `>>>` operator | same signed type (`int` / `long`) |
| C# | arithmetic | `>>>` operator, added in C# 11 | same signed type |
| JavaScript | arithmetic | `>>>` operator | **unsigned `uint32`** — `-1 >>> 0` is `4294967295` |
| Kotlin | arithmetic | `ushr` **named infix function** | same signed type |
| Rust | arithmetic on signed, logical on unsigned | none — the operand type selects | — |
| Go | arithmetic on signed, logical on unsigned | none — the operand type selects | — |
| Swift | arithmetic on signed, logical on unsigned | none — the operand type selects | — |
| Zig | arithmetic on signed, logical on unsigned | none — the operand type selects | — |
| C++20 | arithmetic, mandated since C++20; implementation-defined for negative left operands earlier and in C | none — the operand type selects | — |

Two clusters, and Ori falls in the first by construction:

- Languages with **both** signed and unsigned integer types — Rust, Go, Swift, Zig, C, C++ — need no second mechanism: the operand's type selects the fill. Ori deliberately has no unsigned type (`annex-e:29`), so this route is closed.
- Languages with **one** signed integer type at the operator level — Java, JavaScript, Kotlin — added a second mechanism. Two chose an operator; one chose a function.

**Ori's result type follows Java and Kotlin, not JavaScript.** `>>>` returns `int`, so `-1 >>> 0 == -1`. JavaScript's returns an unsigned `uint32`, which is why the JavaScript idiom `x >>> 0` coerces a value to unsigned — an idiom that is a **no-op in Ori**. This is not a theoretical distinction: `drafts/std-math-bit-operations-proposal.md:33` currently writes `rotate_left` as `(value << k) | (value >>> (64 - k) >>> 0)`, transcribing that JavaScript idiom into a language where it does nothing. Flattening the logical-shift column to a binary yes/no is what makes that error easy to make, so the result-type column is carried explicitly.

### Kotlin — the sharpest counter-case, engaged rather than omitted

Kotlin is a JVM language with the same one-signed-type-at-the-operator-level constraint as Java, and it is the language best positioned to copy Java's `>>>`. It declined, exposing `shr` and `ushr` as named infix functions instead. That is a direct challenge to Alternative 3's rejection, and a prior-art section whose predecessor was faulted for inversion cannot omit it.

The Kotlin choice is coherent in Kotlin because Kotlin has **no** shift operators at all — `shl`, `shr`, `ushr`, `and`, `or`, `xor` are all infix functions, so `ushr` is uniform with its siblings. Ori is not in that position: `<<`, `>>`, `&`, `|`, `^` are operators today, so a function-only `shift_right_logical` would be the single non-operator member of an operator family. Uniformity argues for `>>>` in Ori for the same reason it argued for `ushr` in Kotlin. The prior art is genuinely split; this is the ground for choosing, and Kotlin is evidence the choice is not forced.

### C# — the rationale, corrected

An earlier revision attributed C#'s `>>>` to a "JVM-style bytecode idiom". That is wrong: C# targets CIL, not JVM bytecode. C# 11 added `>>>` for **generic math** — `IShiftOperators<TSelf, TOther, TResult>` requires a shift operation that is well-defined when the operand is a type parameter that may be signed or unsigned, which a fill-mode-by-operand-type rule cannot supply. That is a stronger argument for a distinct operator than the one previously given, and structurally closer to Ori's situation than the JVM framing was.

### Verified issue-corpus entries

- `zig#20367` *"Proposal: Make right shift mode explicit for signed integers"* — **open**. A language whose signed shift mode is implicit, still litigating whether to make it explicit.
- `zig#5220` *"Proposal: Explicit Shift Operators"* — **open**. Same pressure, broader scope.
- `go#19113` *"proposal: spec: allow signed shift counts"* — **closed, completed**. Cited only for the narrow point that shift semantics needed explicit spec text rather than inference; it concerns shift COUNTS, not fill mode.

Deliberately NOT cited: `go#44664` *"spec: clarify that signed integers>=0 are permitted as shift counts"* is a closed PR about shift counts, not fill mode; the withdrawn `pure-bit-operations-proposal.md` cited it as evidence about fill semantics, which it does not support.

Not verifiable: a fourth Zig issue (`zig#21709`) was suggested during review as the most on-point hit. It does not appear in this repository's issue corpus under any search performed, so it is not cited. A reviewer with upstream access may add it.

**Grounding note.** The Java, Kotlin, C#, and JavaScript rows are from language-reference knowledge; no Java or Kotlin repository is present in the reference corpus searched, so they are recorded as **not independently corpus-verified**. The Rust, Go, Zig, and Swift rows are corpus-verifiable.

---

## Migration / Breaking Changes

None. Every change is additive:

- `>>` behavior is unchanged; only its specification becomes explicit. Annex E:29's amendment removes a contradiction rather than changing behavior — every shipped executor already sign-propagates (§1), so no implementation changes and no program changes meaning.
- `>>>` and `>>>=` are synthesized in the parser from adjacent `>` tokens (§3). No existing program can contain them: a run of three adjacent `>` in expression position is a parse error today, and in type position the parser's type-argument path — not `infix_binding_power` — consumes the run. **This claim depends on parser-side synthesis and would be false under a lexer token.**
- `ShrLogical` is a new prelude trait name; no existing identifier collides.
- `range_shr` is untouched; `range_shr_logical` is a new sibling.
- `hash_combine` and every derived `Hashable` value are unchanged.

### Absence claims and the searches that establish them

Surfaces searched for every claim below: `docs/ori_lang/proposals/drafts/`, `docs/ori_lang/proposals/approved/`, `docs/ori_lang/v2026/spec/`, `library/`, `compiler/`, `tests/`.

| Claim | Result |
|---|---|
| `ShrLogical` / `shift_right_logical` unused | zero hits across all six surfaces |
| `>>>` does not appear as an Ori operator | **41 files contain the three-character sequence.** Within `docs/ori_lang/proposals/` and `docs/ori_lang/v2026/spec/` there are exactly four: this document; the withdrawn `pure-bit-operations-proposal.md`; `drafts/std-math-bit-operations-proposal.md`, which uses `>>>` in Ori code blocks at `:33,38-42` and in prose at `:7,24,112,276,292,301`; and `approved/existential-types-proposal.md:39`. The remaining files are under `compiler/`, `tools/`, `docs/`, and `tests/`, none as an Ori operator |

Two corrections to an earlier revision's version of this claim, both material:

- **`drafts/std-math-bit-operations-proposal.md` was omitted.** It is this proposal's own declared sibling and the largest `>>>` consumer in the corpus. Omitting the most relevant hit while reporting a sweep is the same defect the grounding discipline exists to prevent, and it is corrected here rather than softened.
- **`existential-types-proposal.md` is in `approved/`, not drafts, and it is not unrelated.** Its `:39` is `MapIterator<FilterIterator<TakeIterator<...>>>` — an Ori type expression carrying a three-`>` glyph run, exactly the construct §3's tokenization analysis governs. It is the corpus's own witness that three-`>` runs occur in type position, and characterizing it as an unrelated coincidence was wrong.

---

## Roadmap Impact

Implementation crosses the parser, type system, registry, evaluator, VM, LLVM codegen, `ori_canon` const-fold, `ori_repr`, formatter, prelude, and spec — the eight crates §5 enumerates plus `ori_fmt`. A feature plan scaffolded on approval owns the phase breakdown. Two items carry the risk:

- The `ori_repr` transfer function (§9), which needs the two-dimension split and the enumeration-based tests, not a corner fold.
- The parser's ordered greedy chain (§3), whose existing ordering is already load-bearing and whose regression surface is nested generic types.

---

## Unresolved Questions

- **Whether `>>` dispatch is registry-routed or fast-pathed for primitives** (§4), which determines whether `ShrLogical` needs a registry entry, a type-checker fast path, or both. Recorded as unverified rather than assumed.
- **Registry method spelling.** §4 requires `ShrLogical`'s entry to match `Shr`'s spelling at implementation time; which spelling that is depends on whether the approved `shr` -> `shift_right` rename has shipped. This proposal constrains the relationship, not the value.
- **Diagnostic-code allocation.** Whether the `>>>` shift-count violation gets a new code or reuses the existing shift diagnostic parameterized by operator name is an `ori_diagnostic` decision, and the same question covers the const-position form (§7).
- **A `>>`-versus-`>>>` lint.** Whether the compiler should warn when a `>>` result is immediately masked with a positive constant — a strong signal that zero fill was intended — is deliberately out of scope.

`byte >>>` is **not** an unresolved question. §2 defines it normatively as identical to `byte >>`. An earlier revision listed it as open while its own §2 stated it normatively; the normative text governs and the question is closed.
