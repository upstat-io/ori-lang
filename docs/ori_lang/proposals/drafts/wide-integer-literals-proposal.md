# Proposal: Wide Integer Literals

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 7), compiler (parser literal narrowing), formatter
**Depends On:** none
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors, replacing its `int.from_bits` construct), logical-shift-operator-proposal.md (sibling successor), std-math-bit-operations-proposal.md (sibling successor), stdlib-random-rng-proposal.md (draft — the motivating consumer; its pinned SplitMix64 constants are unwritable today), stdlib-math-api-proposal.md (draft — declares `$int_max` / `$int_min` constants at `:120-121,681`, a separate spelling dispute this proposal does not join), limbs-trait-proposal.md (draft — bignum limb masks are natural wide-hex constants), stdlib-json-native-parser-proposal.md (draft — SWAR bitmask constants), overflow-behavior-proposal.md (approved — its `:228-231` constant-overflow rule governs arithmetic, not reinterpretation), unsafe-operation-gating-proposal.md (draft — its `:61` `__transmute` gating governs memory reinterpretation, not literal notation), representation-optimization-proposal.md (approved — canonical `int` width), comparable-hashable-traits-proposal.md (approved — `hash_combine`'s `0x9e3779b9` constant is within the current range and unaffected)

---

## Summary

Ori restricts every integer literal to `0 .. 2^63 - 1`. That is correct for decimal literals, which denote a numeric magnitude. It is wrong for hexadecimal and binary literals, which denote a bit pattern. This proposal permits `hex_lit` and `bin_lit` — and only those two forms — to carry a full 64-bit pattern, reinterpreted as its two's-complement `int` value. `decimal_lit` keeps its existing range and error. The grammar already parses unbounded hex and binary; the restriction is prose-only.

---

## Motivation

### The Problem in Practice

Published bit-mixing algorithms specify their constants in hexadecimal, because the constants are bit patterns, not quantities. SplitMix64 — the seed expander every modern PRNG design uses — pins three:

```ori
// Every one of these three lines is a COMPILE ERROR today:
//   "An integer literal shall represent a value in the range 0 to 2^63 - 1."
let $golden_gamma = 0x9E3779B97F4A7C15;
let $mix_a        = 0xBF58476D1CE4E5B9;
let $mix_b        = 0x94D049BB133111EB;
```

All three exceed `int.max`, which is `2**63 - 1` = `9223372036854775807`. Each pattern, its unsigned decimal value, and its two's-complement signed value (`pattern - 2**64`):

| Bit pattern | Unsigned decimal | Two's-complement signed |
|---|---|---|
| `0x9E3779B97F4A7C15` | `11400714819323198485` | `-7046029254386353131` |
| `0xBF58476D1CE4E5B9` | `13787848793156543929` | `-4658895280553007687` |
| `0x94D049BB133111EB` | `10723151780598845931` | `-7723592293110705685` |

### The available workaround is a review hazard

The same values are expressible today as the negative decimals in the third column above — their two's-complement equivalents.

A reviewer cannot verify a 19-digit negative decimal against a published hexadecimal constant by inspection. A wrong PRNG constant still produces plausible-looking output, so short-sequence testing does not catch it either. The hazard is demonstrated, not hypothetical: the withdrawn `pure-bit-operations-proposal.md:174` asserted `0xBF58476D1CE4E5B9 == -4688729468158715975`, which is wrong; the correct value is `-4658895280553007687`, and the error was caught only by machine-checking the subtraction.

### When This Matters

Any code where a constant is a bit pattern: PRNG seed expanders and multipliers, hash mixing constants, SWAR masks, bignum limb masks, protocol magic numbers, packed-field masks and sentinels. In every case the published form is hexadecimal or binary, and the value routinely has bit 63 set.

---

## Goals and Non-Goals

**Goals:**

- Permit `hex_lit` and `bin_lit` to denote any 64-bit pattern, valued as its two's-complement `int`.
- Keep `decimal_lit`'s `0 .. 2^63 - 1` range and its existing compile-time error unchanged.
- Keep published algorithm constants legible in the form they are published in.
- Keep every change additive: no existing program changes meaning.

**Non-Goals:**

- **Widening `decimal_lit`.** A decimal literal denotes a magnitude; `18446744073709551615` meaning `-1` would be a genuine readability regression, and `9223372036854775808` meaning `int.min` would be actively misleading. Decimal keeps its rule.
- **An unsigned integer type.** `annex-e-system-considerations.md:29`'s single-signed-type decision stands.
- **A literal suffix** (`0x...u`, `0x...i64`). No suffix syntax exists in Ori's grammar and none is requested.
- **Arbitrary-precision or 128-bit literals.** The bound is exactly 64 bits, matching `int`'s canonical representation.
- **Any change to arithmetic, overflow, or conversion rules.** This proposal changes notation only.

---

## Design

### 1. The rule

Replace the single sentence at `07-lexical-elements.md:237` with a form-dependent rule:

- A **decimal** integer literal shall represent a value in the range `0` to `2^63 - 1`. A literal value outside this range is a compile-time error. (Unchanged.)
- A **hexadecimal or binary** integer literal shall denote a 64-bit pattern. A literal whose written digits require more than 64 bits is a compile-time error. A pattern with bit 63 set denotes the corresponding negative `int` under two's-complement interpretation.

```ori
let $a = 0xBF58476D1CE4E5B9;          // -4658895280553007687 as an int
let $b = 0x7FFF_FFFF_FFFF_FFFF;       //  int.max  (bit 63 clear)
let $c = 0x8000_0000_0000_0000;       //  int.min  (bit 63 set)
let $d = 0xFFFF_FFFF_FFFF_FFFF;       //  -1
let $e = 0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;  // int.min

let $f = 18446744073709551615;        // COMPILE ERROR: decimal, out of range (unchanged)
let $g = 0x1_0000_0000_0000_0000;     // COMPILE ERROR: 65 bits
```

The existing digit-separator rules (`07-lexical-elements.md:230-233`) and case-insensitivity of hex digits (`:241`) are unchanged.

### 2. Grammar — no change required

`grammar.ebnf:100-101` already parses unbounded hex and binary:

```ebnf
hex_lit = "0" ( "x" | "X" ) hex_digit { [ "_" ] hex_digit } .
bin_lit = "0" ( "b" | "B" ) bin_digit { [ "_" ] bin_digit } .
```

Neither production carries a width bound. The `0 .. 2^63 - 1` restriction lives entirely in prose at `07-lexical-elements.md:237`. Verified by reading both files; no grammar production changes.

### 3. Implementation — where the narrowing actually is

The reviewer-supplied claim that "the sole narrowing is one `i64::try_from` at `literals.rs:68`" was checked and is **partly wrong**. Corrected inventory, from reading the source:

**The lexer already carries the full 64-bit value.** `compiler/ori_lexer/src/parse_helpers/mod.rs:10-27` (`parse_int_skip_underscores`) accumulates into a `u64` with `checked_mul` / `checked_add`, returning `None` only on `u64` overflow. `compiler/ori_lexer/src/cooker/numeric.rs:17-45` (`cook_int_radix`, shared by `cook_int` / `cook_hex_int` / `cook_bin_int`) emits `TokenKind::Int(n)` where `n: u64`. `compiler/ori_ir/src/token/kind.rs:21-22` declares `Int(u64)` with the comment "stored as u64; negation folded in parser". So `0xBF58476D1CE4E5B9` already reaches the parser intact.

**The narrowing is in the parser, at six sites, not one:**

| Site | Context |
|---|---|
| `compiler/ori_parse/src/grammar/expr/primary/literals.rs:68` | expression literal — the primary site |
| `compiler/ori_parse/src/grammar/expr/mod.rs:379-388` | negation folding (`-42`), with an `I64_MIN_ABS` special case |
| `compiler/ori_parse/src/grammar/expr/patterns/literal_patterns.rs:34` | pattern literal |
| `compiler/ori_parse/src/grammar/expr/patterns/literal_patterns.rs:65` | pattern literal |
| `compiler/ori_parse/src/grammar/expr/patterns/literal_patterns.rs:189` | range-pattern endpoint |
| `compiler/ori_parse/src/grammar/expr/patterns/literal_patterns.rs:209` | range-pattern endpoint |

Each rejects with `E1002 "integer literal too large"`.

**The cooked token loses radix provenance, and that is the real work item.** `TokenKind::Int(u64)` carries no radix tag, so the parser cannot today tell `0xBF58476D1CE4E5B9` from a decimal literal of the same value. Permitting hex/bin while keeping decimal bounded therefore requires preserving provenance from the cooker to the parser. `TokenFlags` (`compiler/ori_ir/src/token/index.rs:10-28`) is a `u8` with all eight bits already assigned, so a spare flag bit is not available. Two viable shapes:

- Add a distinct cooked kind, e.g. `TokenKind::RadixInt(u64)`, emitted by `cook_hex_int` / `cook_bin_int` while `cook_int` keeps `TokenKind::Int(u64)`. The raw-tag layer already distinguishes them (the cooker dispatches on separate raw tags), so this is a faithful lift of an existing distinction.
- Widen `TokenFlags` to `u16` and assign a `RADIX_LITERAL` bit.

The first is preferred: it makes the distinction type-level, so every one of the six narrowing sites must handle it explicitly and none can silently keep the old behavior. Selection is an implementation decision for the feature plan; the proposal's requirement is only that provenance survive to the parser.

With provenance available, each narrowing site becomes: decimal -> `i64::try_from(n)` as today; hex/bin -> `n as i64` (two's-complement reinterpretation), with the error condition moved entirely into the lexer, where "more than 64 bits" is already detected as `u64` overflow (`parse_helpers/mod.rs:19-20`).

### 4. Negation folding

`compiler/ori_parse/src/grammar/expr/mod.rs:379-388` folds `-<literal>` into a single `Int` node, with a special case admitting `-9223372036854775808` as `i64::MIN`. Under this proposal a negated wide hex literal (`-0xFFFF_FFFF_FFFF_FFFF`) is negation applied to `-1`, yielding `1`. That composes, but reads badly. The formatter and a lint should discourage negating a wide hex literal; the language does not forbid it.

The `I64_MIN_ABS` special case has a side effect worth recording: `int.min` IS already writable today as `-9223372036854775808`, which the NOTE at `07-lexical-elements.md:239` ("The minimum `int` value (-2^63) cannot be written as a literal ... available as the associated constant `int.min`") does not acknowledge. That NOTE is inaccurate independent of this proposal. Correcting it is in scope for the Clause 7 edit below.

### 5. `int.min` / `int.max`

This proposal deliberately does not resolve the associated-constant question. Facts established by reading:

- `07-lexical-elements.md:239` names `int.min` as an associated constant.
- `compiler/ori_registry/src/defs/int.rs:160-176` defines `min` and `max` as two-argument *methods* on `int`, not associated constants. No associated constant is implemented.
- `drafts/stdlib-math-api-proposal.md:120-121,681` declares `$int_max` / `$int_min` as module constants, a third spelling.

Under this proposal, `int.min` and `int.max` become writable directly as `0x8000_0000_0000_0000` and `0x7FFF_FFFF_FFFF_FFFF`, which removes the *urgency* from the dispute but does not settle it. The dispute belongs to `stdlib-math-api-proposal.md`. What this proposal does require is that the inaccurate NOTE at `:239` be corrected as part of the Clause 7 edit.

### 6. Interaction with the constant-overflow rule

`overflow-behavior-proposal.md:228-231` makes overflow in a compile-time constant *expression* an error (`$big = int.max + 1  // ERROR: constant overflow`). That rule governs **arithmetic** on constants. A wide hex literal performs no arithmetic: it is a notation for a bit pattern that is already a valid `int`. `0xFFFF_FFFF_FFFF_FFFF` does not compute `18446744073709551615` and then overflow; it denotes `-1` directly, the same way `0xFF as byte` denotes `255` without arithmetic.

The two rules do not overlap and neither needs amending. Recorded here so a reviewer does not read the silence as an unexamined interaction.

### 7. Interaction with `__transmute` gating

`unsafe-operation-gating-proposal.md:57-61` places `__transmute<S, T>(value: S) -> T` under Tier 2 gating. That gate protects **memory reinterpretation** — reading bytes at one type as another, which can produce an invalid value of the destination type or violate its layout invariants.

A wide hex literal is not memory reinterpretation. It is source notation. Every 64-bit pattern is a valid `int` — the type has no invalid bit patterns, no niche, no layout invariant to violate. There is no source value being reinterpreted; the literal denotes an `int` and nothing else. No gating applies, and none is requested.

### 8. Representation optimization

`annex-e-system-considerations.md:27` permits a narrower machine representation. A wide hex literal denotes a value in `int`'s canonical semantic range `[-2^63, 2^63 - 1]`, so it constrains representation exactly as any other `int` constant does: the compiler may narrow the storage of a variable holding it when the as-if rule of `representation-optimization-proposal.md:96-118` is satisfied. The **semantic** 64-bit width is what the literal is written against; the representation width is unobservable. No new interaction.

### Error Handling

| Condition | Diagnostic |
|---|---|
| Decimal literal outside `0 .. 2^63 - 1` | Existing `E1002` "integer literal too large". Message improved to name the decimal restriction and point at the hex form: a value that will not fit a decimal literal is writable as hexadecimal. |
| Hex/bin literal requiring more than 64 bits | `E1002`, detected in the lexer at `u64` overflow. Message names the 64-bit bound and shows the written digit count. |
| Hex/bin literal with no digits after the prefix (`0x`, `0b`) | Existing error, unchanged (`07-lexical-elements.md:235`). |

Both messages state the cause plainly and name the fix. Each gets a friendly-content regression pin.

---

## Drawbacks

- **Two literal forms with different range rules.** A reader must know that `0xFFFFFFFFFFFFFFFF` is legal while `18446744073709551615` is not. The asymmetry is principled — magnitude versus bit pattern — but it is an asymmetry, and it must be taught.
- **A hex literal can silently be negative.** `0x9E3779B97F4A7C15 < 0` surprises anyone who reads hexadecimal as an unsigned magnitude. This is inherent to a single-signed-type language and is exactly Java's `long` behavior; the alternative is an unsigned type, rejected above.
- **Comparisons on wide hex constants read wrong.** `0xFFFFFFFFFFFFFFFF > 0` is `false`. Code mixing wide-hex bit patterns with ordering comparisons is a hazard the notation makes easier to write. The mitigation is that bit-pattern constants are used with bitwise operators, where signedness does not enter.
- **Token-provenance plumbing.** Preserving radix from cooker to parser touches `ori_ir`'s token kind and six parser sites (§3). That is more than the one-line change an initial reading suggested.

---

## Alternatives Considered

### Alternative 1: A const constructor — `int.from_bits(0x...)`

This was the withdrawn `pure-bit-operations-proposal.md`'s item 3. Rejected, and it does not work as designed:

- The argument `0xBF58476D1CE4E5B9` is itself an integer literal, rejected at lex time by the very rule that proposal declared unchanged. A function call cannot rescue an argument that is a compile error to write.
- `int` has zero associated functions anywhere in the spec. `10-declarations.md:566-574` governs only `impl Type: Trait`; there is no stated coherence rule for an inherent `impl` on a primitive, and `10-declarations.md:472` closes the extension route ("Extensions cannot define associated functions"). The construct would establish inherent-impls-on-primitives with no coherence story.
- `grammar.ebnf:683-687` `const_expr` admits literals, `"$" identifier`, binary and unary operators, and parentheses — **no call production**. A `from_bits` call is not writable where a constant expression is required.
- `approved/overflow-behavior-proposal.md:254-259` already settled this API-shape family, rejecting operator forms and method forms in favor of free functions with named arguments. An associated function on a primitive is the declined shape.

The lexical relaxation discharges all four objections, adds no API, and establishes no new precedent.

### Alternative 2: Write the constants as negative decimals

Available today; rejected as the standard mechanism, for the reviewability reasons in Motivation. This proposal does not forbid it — a negative decimal remains a legal way to write any `int`.

### Alternative 3: A literal suffix — `0x...u`

Rejected. Ori has no literal-suffix syntax anywhere in its grammar; introducing one for a single case adds lexical surface, and the suffix would mean "interpret these bits as unsigned" in a language with no unsigned type. The suffix would be a lie about the resulting type.

### Alternative 4: An unsigned 64-bit type

Rejected. `annex-e:29` commits to a single signed integer type. Adding `u64` to express constants would import conversion rules, mixed-arithmetic rules, and a second literal range across the entire numeric surface.

### Alternative 5: Widen decimal literals too

Rejected. A decimal literal denotes a magnitude. `18446744073709551615` denoting `-1` is a readability regression with no compensating benefit — nobody publishes a bit pattern in decimal. Keeping decimal bounded is what makes the hex relaxation principled rather than arbitrary.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:**

- Literal notation is lexical and syntactic surface. A library cannot change what a literal means.
- The narrowing decision lives in `ori_parse`; the range error lives in `ori_lexer` and `ori_diagnostic`.

**Missing features that would enable purity:** Not applicable — this proposal is a notation change, the smallest category of compiler surface. It carries no runtime component, no type-system change, no new API.

**Recommendation:** Proceed as a minimal spec and parser change. This is the smallest of the three successors to `pure-bit-operations-proposal.md`: one spec paragraph, one token-provenance lift, six parser sites.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `07-lexical-elements.md:237` | Split into a decimal rule (unchanged) and a hex/bin rule (64-bit pattern, two's-complement valuation) |
| `07-lexical-elements.md:239` (NOTE) | Correct: `int.min` IS writable as `-9223372036854775808` (folded at `ori_parse/src/grammar/expr/mod.rs:382-388`) and, under this proposal, as `0x8000_0000_0000_0000`. The current claim that it "cannot be written as a literal" is inaccurate today |
| `07-lexical-elements.md` EXAMPLE block (`:235`) | Add a wide-hex example and a rejected 65-bit example |
| `grammar.ebnf` | **No change.** `hex_lit` (`:100`) and `bin_lit` (`:101`) already parse unbounded digit sequences |
| Annex D (`annex-d-formatting.md`) | No change. Digit grouping in hex literals stays author's choice; `0x8000_0000_0000_0000` and `0x8000000000000000` are both canonical |
| Annex E | No change. The `int` canonical range is unchanged; only its notation widens |

### Conformance pins — enumerated

Searched `tests/spec/**`, `compiler/ori_lexer/**`, `compiler/ori_parse/**` for literal-range pins:

| Pin | Asserts | Disposition |
|---|---|---|
| `compiler/ori_lexer/src/cooker/tests.rs:196-200` (`integer_overflow`) | decimal literal beyond `u64` errors at cook time | UNCHANGED |
| `compiler/ori_lexer/src/cooker/tests.rs:238` | hex cooking round-trips | UNCHANGED |
| Parser `E1002` "integer literal too large" sites (six, listed in §3) | decimal beyond `i64` rejected | Decimal path UNCHANGED; hex/bin path changes from reject to reinterpret |

No pin asserting that a wide **hex** literal is rejected was found in `tests/spec/**`. New pins required: `0xFFFF_FFFF_FFFF_FFFF == -1`; `0x8000_0000_0000_0000 == int.min`; `0x7FFF_FFFF_FFFF_FFFF == int.max`; the equivalent binary forms; a 65-bit hex literal rejected; a decimal literal above `2^63 - 1` still rejected with the improved message; a wide hex literal in pattern position; a wide hex literal as a range-pattern endpoint; a wide hex literal in a const-generic bound; evaluator and LLVM parity on the value of each.

---

## Prior Art

| Language | Hex literal for a full-width signed pattern |
|---|---|
| Java | `long x = 0xBF58476D1CE4E5B9L;` — legal; hex/binary/octal literals may set the sign bit, while a decimal literal may not exceed `Long.MAX_VALUE`. This is precisely the split this proposal adopts, in a language with the same single-signed-integer-type constraint |
| C# | `long x = unchecked((long)0xBF58476D1CE4E5B9);` — the pattern is writable via `unchecked`; C# also has native `ulong`, so the pressure is lower |
| C / C++ | A hex constant exceeding `long long`'s range takes an unsigned type by the standard's type-assignment rules; an explicit cast yields the signed pattern. The unsigned type carries the notation |
| Rust | `0xBF58476D1CE4E5B9u64 as i64` or `-4658895280553007687i64`. Rust has unsigned types and literal suffixes, so it needs neither mechanism |
| Go | `int64(-0x40A7B892E31B1A47)` or an untyped-constant expression; Go's untyped constants are arbitrary-precision and must fit the target type on conversion |
| Swift | `Int(bitPattern: 0xBF58476D1CE4E5B9 as UInt)` — an explicit bit-pattern initializer, available because `UInt` exists |

The distinguishing structural fact: **every language that solves this without a per-literal ceremony has an unsigned type to hold the pattern.** Ori does not, by deliberate design (`annex-e:29`). Java is the closest structural match — one signed integer type at the notation level — and Java's answer is exactly this proposal's: decimal literals bounded by the signed maximum, hex and binary literals free to carry the full width.

No relevant issue-corpus entries were found. The issue corpus of reference language implementations was searched over literal-range and hex-literal phrasings and returned no on-point results, so no issue citations appear here rather than approximate ones.

---

## Migration / Breaking Changes

None. Every change is additive:

- Every program legal today stays legal with the same meaning. Decimal literals are untouched.
- The change converts a class of currently-rejected programs into accepted ones. No accepted program changes value.
- No prelude, trait, API, or runtime surface changes.
- Searched `library/`, `tests/`, `compiler/`, and `docs/` for a hex literal above `2^63 - 1`: none exists, because none can compile today.

---

## Roadmap Impact

Implementation touches `ori_ir` (token kind), `ori_lexer` (error site), `ori_parse` (six narrowing sites), `ori_diagnostic` (two message improvements), and Clause 7. A feature plan scaffolded on approval owns the phase breakdown. The token-provenance lift is the only item requiring a design choice (§3).

---

## Unresolved Questions

- **Provenance mechanism.** A distinct `TokenKind::RadixInt(u64)` versus widening `TokenFlags` to `u16`. §3 argues for the former; the decision belongs to implementation.
- **Negating a wide hex literal.** `-0xFFFF_FFFF_FFFF_FFFF` is legal and yields `1`. Whether a lint should discourage it is left open.
- **A canonical grouping convention for wide hex.** Whether `ori fmt` should normalize `0x8000000000000000` to `0x8000_0000_0000_0000` is a formatter question this proposal does not decide; both forms are accepted.
- **`int.min` / `int.max` spelling.** Owned by `stdlib-math-api-proposal.md`; this proposal only requires that the inaccurate NOTE at `07-lexical-elements.md:239` be corrected.
