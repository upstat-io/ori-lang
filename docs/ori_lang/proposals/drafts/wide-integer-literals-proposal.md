# Proposal: Wide Integer Literals

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** spec (Clause 7, Clause 8), compiler (lexer token provenance, parser literal narrowing, `ori_fmt` emit and width paths), formatter
**Depends On:** none
**Amends:** representation-optimization-proposal.md (approved — no change required; cited only)
**Related:** pure-bit-operations-proposal.md (withdrawn — this proposal is one of its three successors, replacing its `int.from_bits` construct), logical-shift-operator-proposal.md (sibling successor — its `>>>` is required before a wide constant is *usable*, see Drawbacks), wrapping-shift-proposal.md (draft — sibling successor of the same withdrawn proposal), std-math-bit-operations-proposal.md (sibling successor), stdlib-random-rng-proposal.md (draft — the motivating consumer; its pinned SplitMix64 constants are unwritable today), stdlib-math-api-proposal.md (draft — declares `$int_max` / `$int_min` at `:120-121,681`, a separate spelling dispute this proposal does not join), limbs-trait-proposal.md (draft — bignum limb masks), stdlib-json-native-parser-proposal.md (draft — SWAR bitmask constants), overflow-behavior-proposal.md (approved — its `:228-231` constant-overflow rule governs arithmetic, not reinterpretation), unsafe-operation-gating-proposal.md (draft — its `:61` `__transmute` gating governs memory reinterpretation, not literal notation), comparable-hashable-traits-proposal.md (approved — `hash_combine`'s `0x9e3779b9` is within the current range and unaffected)

---

## Summary

Ori restricts every integer literal to `0 .. 2^63 - 1`. That is correct for decimal literals, which denote a numeric magnitude. It is wrong for hexadecimal and binary literals, which denote a bit pattern. This proposal permits `hex_lit` and `bin_lit` — and only those two forms — to carry a full 64-bit pattern, valued as its two's-complement `int`. `decimal_lit` keeps its existing range and error. The grammar already parses unbounded hex and binary; the restriction is prose-only. The work is not in the rule but in radix provenance: the cooked token discards radix today, and `ori fmt` consequently rewrites every hex literal to decimal, so provenance must reach the formatter before the feature is anything but inert.

---

## Motivation

### The Problem in Practice

Published bit-mixing algorithms specify their constants in hexadecimal, because the constants are bit patterns, not quantities. SplitMix64 — the seed expander every modern PRNG design uses — pins three:

```ori
// Every one of these three lines is a COMPILE ERROR today.
let $golden_gamma = 0x9E3779B97F4A7C15;
let $mix_a        = 0xBF58476D1CE4E5B9;
let $mix_b        = 0x94D049BB133111EB;
```

All three exceed `int.max`, which is `2^63 - 1` = `9223372036854775807`. Each pattern, its unsigned decimal value, and its two's-complement signed value (`pattern - 2^64`):

| Bit pattern | Unsigned decimal | Two's-complement signed |
|---|---|---|
| `0x9E3779B97F4A7C15` | `11400714819323198485` | `-7046029254386353131` |
| `0xBF58476D1CE4E5B9` | `13787848793156543929` | `-4658895280553007687` |
| `0x94D049BB133111EB` | `10723151780598845931` | `-7723592293110705685` |

### The available workaround is a review hazard

The same values are expressible today as the negative decimals in the third column — their two's-complement equivalents.

A reviewer cannot verify a 19-digit negative decimal against a published hexadecimal constant by inspection. A wrong PRNG constant still produces plausible-looking output, so short-sequence testing does not catch it either. The hazard is demonstrated, not hypothetical: the withdrawn `pure-bit-operations-proposal.md:174` asserted `0xBF58476D1CE4E5B9 == -4688729468158715975`, which is wrong; the correct value is `-4658895280553007687`, and the error was caught only by machine-checking the subtraction.

### The compiler already advertises this proposal's rule

Running the current compiler on a 65-bit literal produces:

```
error[E0003]: hexadecimal integer literal overflows `int`
  |
1 | @a () -> int = 0x1_0000_0000_0000_0000;
  |                ^^^^^^^^^^^^^^^^^^^^^^^ value exceeds maximum integer
  |
  = help: use a smaller value (maximum is 0xFFFFFFFFFFFFFFFF)
```

The shipped help text names `0xFFFFFFFFFFFFFFFF` as the maximum hexadecimal literal. **That is wrong today** — the true maximum is `0x7FFFFFFFFFFFFFFF`, and `0x8000000000000000` is rejected by a different error at a different phase. It becomes correct exactly under this proposal. The diagnostic was written against the rule a reader would expect, not the rule the language implements, which is evidence that the current rule is the surprising one.

### When This Matters

Any code where a constant is a bit pattern: PRNG seed expanders and multipliers, hash mixing constants, SWAR masks, bignum limb masks, protocol magic numbers, packed-field masks and sentinels. In every case the published form is hexadecimal or binary, and the value routinely has bit 63 set.

---

## Goals and Non-Goals

**Goals:**

- Permit `hex_lit` and `bin_lit` to denote any 64-bit pattern, valued as its two's-complement `int`.
- Keep `decimal_lit`'s `0 .. 2^63 - 1` range and its existing compile-time error unchanged.
- Preserve radix provenance from the lexer through to `ori fmt`, so a hex literal survives formatting as a hex literal.
- Keep every accepted program's meaning unchanged, and make the one case that would otherwise change meaning a compile error rather than a silent reinterpretation.

**Non-Goals:**

- **Widening `decimal_lit`.** A decimal literal denotes a magnitude; `18446744073709551615` meaning `-1` would be a genuine readability regression, and `9223372036854775808` meaning `int.min` would be actively misleading.
- **An unsigned integer type.** `annex-e-system-considerations.md:29`'s single-signed-type decision stands.
- **A literal suffix** (`0x...u`, `0x...i64`). No suffix syntax exists in Ori's grammar and none is requested.
- **Arbitrary-precision or 128-bit literals.** The bound is exactly 64 bits, matching `int`'s canonical representation.
- **Any change to arithmetic, overflow, or conversion rules.** This proposal changes notation only.
- **Making the SplitMix64 example compile.** Writing the constant is necessary and not sufficient; see Drawbacks.
- **Deciding a canonical digit-grouping convention for `ori fmt`.** Provenance must reach the formatter (§4); whether the formatter then normalizes `0x8000000000000000` to `0x8000_0000_0000_0000` is left open.

---

## Design

### 1. The rule — value-based, not digit-count-based

Replace the single sentence at `07-lexical-elements.md:237` with a form-dependent rule:

- A **decimal** integer literal shall represent a value in the range `0` to `2^63 - 1`. A literal value outside this range is a compile-time error. (Unchanged.)
- A **hexadecimal or binary** integer literal shall denote a 64-bit pattern. A literal whose **value** requires more than 64 bits is a compile-time error. A pattern with bit 63 set denotes the corresponding negative `int` under two's-complement interpretation.

**The rule is on the value, not on the count of written digits, and the distinction is observable.** `07-lexical-elements.md:224` prohibits leading zeros in **decimal** literals only, so a hexadecimal literal may carry arbitrarily many leading zeros. `0x00000000000000000FF` is 19 hex digits — 76 written bits — and it **compiles today and evaluates to `255`**, confirmed by running the compiler. A digit-count rule would reject it, breaking a program that works. An earlier revision of this proposal wrote the rule as "a literal whose written digits require more than 64 bits"; that phrasing is **retracted**.

The lexer already implements the value reading: `compiler/ori_lexer/src/parse_helpers/mod.rs:10-27` accumulates into a `u64` with `checked_mul` and `checked_add`, so leading zeros cost nothing and overflow is detected on the value.

```ori
let $a = 0xBF58476D1CE4E5B9;          // -4658895280553007687 as an int
let $b = 0x7FFF_FFFF_FFFF_FFFF;       //  int.max  (bit 63 clear)
let $c = 0x8000_0000_0000_0000;       //  int.min  (bit 63 set)
let $d = 0xFFFF_FFFF_FFFF_FFFF;       //  -1
let $e = 0x0000_0000_0000_0000_00FF;  //  255      (leading zeros are free)
let $f = 0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;  // int.min

let $g = 18446744073709551615;        // COMPILE ERROR: decimal, out of range (unchanged)
let $h = 0x1_0000_0000_0000_0000;     // COMPILE ERROR: value needs 65 bits
```

The existing digit-separator rules (`07-lexical-elements.md:230-233`) and hex-digit case-insensitivity (`:241`) are unchanged.

### 2. Negating a wide literal is a compile error

`-0x8000000000000000` is **accepted today and means `int.min`.** This is the one place where the relaxation would silently change an accepted program's meaning, and it is the sharpest defect in this proposal's design space.

The mechanism, read from source: `compiler/ori_parse/src/grammar/expr/mod.rs:362-397` folds `-<integer literal>` into a single `ExprKind::Int` node. It matches `TokenKind::Int(n)` where `n: u64`, and the cooker has already collapsed decimal, hex, and binary into that one variant (`compiler/ori_lexer/src/cooker/numeric.rs:28`), so **the fold is radix-blind**. `i64::try_from(2^63)` fails, the `n == I64_MIN_ABS` arm fires, and the result is `i64::MIN`. Confirmed by running the compiler: `-0x8000000000000000` type-checks and evaluates to `-9223372036854775808`.

Under §1, `0x8000000000000000` denotes `int.min` in its own right, so `-0x8000000000000000` would become negation *of* `int.min`. Three dispositions were available and two are unacceptable:

| Disposition | Result | Verdict |
|---|---|---|
| Keep the `I64_MIN_ABS` fold for hex and binary | `0x8000000000000000` and `-0x8000000000000000` both denote `int.min`, so `-x == x` for exactly one literal | Incoherent. Rejected |
| Drop the fold for hex and binary | `-0x8000000000000000` becomes `-(int.min)`, an overflow — a runtime panic in expression position, a compile error in const position | A silent behavior change on an accepted program. Rejected |
| **Forbid `-<wide hex or binary literal>`** | compile error, with a diagnostic naming the fix | **Adopted** |

**Normative rule.** A unary minus applied directly to a hexadecimal or binary literal whose value is at least `2^63` is a compile-time error. The diagnostic states that the literal already denotes a negative `int` under two's-complement valuation, names the value it denotes, and directs the reader to drop the minus sign — or, when a magnitude was intended, to write the decimal form. It carries a friendly-content regression pin.

This is narrow by construction. `-0xFF` keeps working — it is pinned at `tests/spec/lexical/int_literals.ori:235-238` (`-0xFF == -255`) and its value is far below `2^63`. Only a literal that already sets bit 63 is affected, and for such a literal the minus sign never expressed the author's intent under either rule.

The rule converts the one silent meaning change into a loud compile error and leaves the `I64_MIN_ABS` fold untouched for its decimal use. An earlier revision of this proposal declined to forbid the form on the grounds that it "composes, but reads badly"; that permissiveness is precisely what created the break, and it is **retracted**.

**No conformance pin exercises this case.** `tests/spec/lexical/int_literals.ori:235-238` is the only negated-hex pin and exercises only the `i64::try_from` happy path; `:242-247` is the only `I64_MIN_ABS` pin and uses the **decimal** spelling. A search of `tests/` and `compiler/` for `8000000000000000` returns zero hits. So without the rule above, this change would land undetected.

### 3. Interaction with the constant-overflow rule — §2 closes the gap

`overflow-behavior-proposal.md:228-231` makes overflow in a compile-time constant *expression* an error. That rule governs **arithmetic** on constants. A wide hex literal performs no arithmetic: it is notation for a bit pattern that is already a valid `int`. `0xFFFF_FFFF_FFFF_FFFF` does not compute `18446744073709551615` and then overflow; it denotes `-1` directly, the way `0xFF as byte` denotes `255` without arithmetic.

An earlier revision stated the two rules "do not overlap". They overlap at exactly one point: **negation is arithmetic on a constant**, and `grammar.ebnf:683-687` admits `unary_op const_expr`. So `-<wide literal>` sits in both rules' scope. §2's prohibition resolves it before either rule applies, by rejecting the form at the literal level. With §2 in place the non-overlap claim holds; without it, it was false at the one case that mattered.

### 4. Radix provenance — the prerequisite, not a follow-up

**`ori fmt` rewrites every hexadecimal and binary literal to decimal today.** Confirmed by running `ori fmt`:

| Input | Output |
|---|---|
| `0xFF` | `255` |
| `0b1010` | `10` |
| `-0xFF` | `-255` |
| `-0x8000000000000000` | `-9223372036854775808` |

So a formatted file loses `0x9E3779B97F4A7C15` and gains `-7046029254386353131` — **precisely the 19-digit unreviewable decimal this proposal exists to eliminate.** Without provenance, the feature is inert on any file that has been formatted, and `ori fmt` has no user options to opt out (one canonical shape is a deliberate property of the formatter). An earlier revision asserted under Annex D that "`0x8000_0000_0000_0000` and `0x8000000000000000` are both canonical"; neither survives, and neither does any narrow hex literal. That claim is **retracted**, and this is a **blocking prerequisite**, not an Unresolved Question.

**Where provenance is lost.** `compiler/ori_ir/src/token/kind.rs:21-22` declares `Int(u64)`, and `compiler/ori_lexer/src/cooker/numeric.rs:28` cooks `RawTag::Int`, `RawTag::HexInt`, and `RawTag::BinInt` all into it. The raw-tag layer distinguishes them; the cooked token does not. `compiler/ori_ir/src/ast/expr/mod.rs:110-112` then declares `ExprKind::Int(i64)`, which also carries no radix.

**The mechanism: widen `TokenFlags`, do not add a `TokenKind` discriminant.** An earlier revision proposed a distinct `TokenKind::RadixInt(u64)` on the argument that a type-level split forces every consumer to handle it explicitly. **That argument is inverted, and the inversion is verifiable by reading the sites.**

- The six narrowing sites are `if let` and `let-else` bindings on `TokenKind::Int(n)`, not exhaustive matches. `grammar/expr/mod.rs:373` is `if let TokenKind::Int(n) = *self.cursor.peek_next_kind()`. A new discriminant makes such a site **fall through silently**, not fail to compile. The claimed safety does not exist.
- Roughly a dozen further consumers match the discriminant and would change behavior silently. Enumerated from source: `grammar/expr/primary/literals.rs:15` and `grammar/expr/patterns/literal_patterns.rs:13` are `TokenSet` const-builders, so hex would stop being a literal-start and pattern-start token; `literal_patterns.rs:177` (range-pattern start), `primary/block_map.rs:64` (block-versus-map disambiguation), `item/function/mod.rs:259` (clause-pattern), `cursor/identifiers.rs:66` (tuple-field member name), `attr/repr.rs:41` and `attr/compile_fail.rs:158,170` (attributes), `ori_ir/src/token/kind.rs:190` (`can_start_expr`), `kind_display.rs:89,411`, and **`ori_fmt/src/spacing/category.rs:372`**, where every hex literal would silently fall out of its spacing category.
- A new tag additionally has to land **below 128**: `grammar/expr/postfix/mod.rs:96-103` returns `false` for tags `>= 128`, so a tag in the free high range would silently disable `0xFF.method()` and `0xFF as float`.

`TokenFlags` (`compiler/ori_ir/src/token/index.rs:8-42`) is a `#[repr(transparent)] struct TokenFlags(u8)` with all eight bits assigned, so a spare bit is genuinely unavailable — but widening it to `u16` is the smaller and safer change. It leaves every discriminant untouched, so all of the consumers above keep working by construction, and a narrowing site that forgets to consult the flag rejects a wide literal with the existing error, which is a **loud** failure rather than a silent one. The cost is one byte per token in a parallel array.

Two radix bits are needed, not one: the formatter has to reproduce `0x` versus `0b`.

**Provenance must reach the formatter, and stop there.** `ori_fmt` formats from the parse tree, so the flag alone is insufficient — `Formatter` holds `arena: &'a ExprArena` (`ori_fmt/src/formatter/mod.rs:114-129`) and dispatches on `ExprKind::Int`. Four non-test sites consume the value:

| Site | Path |
|---|---|
| `ori_fmt/src/formatter/inline/mod.rs:38` -> `formatter/literals.rs:62-66` | emit |
| `ori_fmt/src/width/mod.rs:116` (`int_width`) | width |
| `ori_fmt/src/declarations/parsed_types/mod.rs:223` (`ctx.emit(n.to_string())`) | emit, inside a type — `[T, max 0xFF]` |
| `ori_fmt/src/declarations/parsed_types/mod.rs:256` (`n.to_string().len()`) | width, same context |

The register of review findings described this as the emit and width paths; there are **four** sites across two subsystems, because const-generic arguments inside types are formatted by a separate path.

**Radix shall not enter `ExprKind` or any downstream IR.** Radix is a property of the *source notation*, not of the value. `ExprKind::Int(i64)` is mirrored by `ori_canon`'s `Int(i64)` at four further sites; threading radix through them would put a formatting concern into canonical IR and create a second source of truth for a value that is already fully determined. The correct carrier is a **sparse side table on `ExprArena` keyed by `ExprId`**, populated by the parser only for hexadecimal and binary literals. `ExprArena` (`compiler/ori_ir/src/arena/mod.rs:83-97`) is already a parallel-array design with several side vectors, so this matches its existing shape, costs nothing for the common decimal case, and is visible to `ori_fmt` through the arena it already holds.

### 5. Implementation — where the narrowing is

**The lexer already carries the full 64-bit value.** `parse_helpers/mod.rs:10-27` returns `None` only on `u64` overflow; `cooker/numeric.rs:17-45` emits `TokenKind::Int(n)` with `n: u64`. So `0xBF58476D1CE4E5B9` already reaches the parser intact.

**The narrowing is in the parser, at six sites:**

| Site | Context |
|---|---|
| `ori_parse/src/grammar/expr/primary/literals.rs:68` | expression literal — the primary site |
| `ori_parse/src/grammar/expr/mod.rs:379-388` | negation folding, with the `I64_MIN_ABS` special case (§2) |
| `ori_parse/src/grammar/expr/patterns/literal_patterns.rs:34` | negative pattern literal |
| `ori_parse/src/grammar/expr/patterns/literal_patterns.rs:65` | positive pattern literal |
| `ori_parse/src/grammar/expr/patterns/literal_patterns.rs:189` | range-pattern endpoint, negative |
| `ori_parse/src/grammar/expr/patterns/literal_patterns.rs:209` | range-pattern endpoint, positive |

Each rejects with `E1002 "integer literal too large"`. With provenance available, each becomes: decimal -> `i64::try_from(n)` as today; hexadecimal or binary -> `n as i64`.

### 6. Positions that are not expressions

Three positions accept an integer literal outside ordinary expression context and need explicit dispositions.

**Fixed-capacity list capacity.** `08-types.md:360` requires `N` to be "a compile-time constant: a positive integer literal or a `$` constant binding". Capacity parses as an ordinary expression through `literals.rs:68` and so inherits the relaxation, which makes `[int, max 0x8000_0000_0000_0000]` a well-formed literal denoting a **non-positive** value. Disposition: the capacity rule is a **type-level** constraint, not a lexical one. The literal is well-formed; the capacity is rejected by Clause 8's positivity requirement, as a type error at the same phase and with the same diagnostic as `[int, max -1]`. Clause 8 is added to the Spec Impact table below, which an earlier revision omitted. The same disposition covers a `where N > 0` bound.

**Range patterns.** `0x0000_0000_0000_0000 ..= 0xFFFF_FFFF_FFFF_FFFF` denotes `0 ..= -1` — an inverted, empty range matching nothing, where an author writing it plainly intends "every bit pattern". This is a real trap the notation makes easy to write. Disposition: the range is inverted under the ordinary `int` ordering and is treated exactly as `0 ..= -1` written in decimal. `15-patterns.md:220` states that "the compiler warns about empty range patterns (where `lo > hi`)". **That warning does not fire today** — `match x { 255..=-1 -> 1, _ -> 0 }` compiles clean, verified by running the compiler. That is a pre-existing spec/implementation divergence, independent of this proposal and reported separately; this proposal does not depend on the warning existing, and adds a pin for the wide-hex spelling once it does.

**Attributes.** `ori_parse/src/grammar/attr/repr.rs:41` flows the raw `u64` into `ReprAttr::Aligned(u64)` with **no narrowing at all**, so `#repr("aligned", 0x8000000000000000)` already parses today, violating the current `07-lexical-elements.md:237` rule. The value reaches `ori_repr/src/pipeline/mod.rs:476` as `u32::try_from(n).unwrap_or(u32::MAX)`. Disposition: this is a **pre-existing** gap between the normative literal rule and its enforcement, not one this proposal creates. The Clause 7 edit shall state that two's-complement valuation is a property of the **literal token**, so it applies uniformly wherever a literal appears, and that alignment values are separately constrained by their own attribute rule. Making `repr`'s narrowing match the stated rule is a required deliverable here, because this proposal is the one that makes the literal rule form-dependent and therefore the one that must say which form `repr` accepts.

### 7. `int.min` / `int.max`

This proposal deliberately does not resolve the associated-constant question. Facts established by reading:

- `07-lexical-elements.md:239` names `int.min` as an associated constant.
- `compiler/ori_registry/src/defs/int.rs:160-176` defines `min` and `max` as two-argument *methods* on `int`, not associated constants. No associated constant is implemented.
- `drafts/stdlib-math-api-proposal.md:120-121,681` declares `$int_max` / `$int_min` as module constants, a third spelling.

Under this proposal, `int.min` and `int.max` become writable directly as `0x8000_0000_0000_0000` and `0x7FFF_FFFF_FFFF_FFFF`, which removes the *urgency* from the dispute without settling it. The dispute belongs to `stdlib-math-api-proposal.md`.

What this proposal does require is that the NOTE at `07-lexical-elements.md:239` be corrected. It states that `int.min` "cannot be written as a literal because the positive value 2^63 exceeds the literal range". `int.min` **is** writable today as `-9223372036854775808`, folded at `grammar/expr/mod.rs:382-388` and pinned at `tests/spec/lexical/int_literals.ori:242-247`. The NOTE is inaccurate independent of this proposal.

### 8. `__transmute` gating

`unsafe-operation-gating-proposal.md:57-61` places `__transmute<S, T>` under Tier 2 gating. That gate protects **memory reinterpretation** — reading bytes at one type as another, which can produce an invalid value of the destination type or violate its layout invariants.

A wide hex literal is not memory reinterpretation. It is source notation. Every 64-bit pattern is a valid `int` — the type has no invalid patterns, no niche, no layout invariant to violate — and there is no source value being reinterpreted. No gating applies, and none is requested.

### 9. Representation optimization

`annex-e-system-considerations.md:27` permits a narrower machine representation. A wide hex literal denotes a value in `int`'s canonical semantic range `[-2^63, 2^63 - 1]`, so it constrains representation exactly as any other `int` constant does. The **semantic** 64-bit width is what the literal is written against; the representation width is unobservable under the as-if rule of `representation-optimization-proposal.md:96-118`. No new interaction, and no erratum is required.

### Error Handling

| Condition | Diagnostic |
|---|---|
| Decimal literal outside `0 .. 2^63 - 1` | Existing `E1002` "integer literal too large", raised in the parser at `i64::try_from` failure. Message improved to name the decimal restriction and point at the hexadecimal form |
| Hexadecimal or binary literal whose value needs more than 64 bits | Existing **`E0003`**, raised in the **lexer** at `u64` overflow (`ori_lexer/src/lex_error/mod.rs:116-122,175-178`; `HexIntOverflow` / `BinIntOverflow`). **No new code and no message change is required** — the shipped text already reads "hexadecimal integer literal overflows `int`" with help "maximum is 0xFFFFFFFFFFFFFFFF", which is exactly this proposal's rule |
| Unary minus on a hexadecimal or binary literal with bit 63 set (§2) | **New.** States that the literal already denotes a negative `int`, names the denoted value, and directs the reader to drop the minus |
| Hexadecimal or binary literal with no digits after the prefix | Existing error, unchanged (`07-lexical-elements.md:235`) |

An earlier revision assigned the >64-bit case to `E1002` and located it in the lexer. Both halves were wrong: `E1002` is the **parser's** `u64`-to-`i64` narrowing error, and `E0003` is the **lexer's** `u64` overflow error. They are distinct codes at distinct phases, verified by running the compiler on both inputs. Under this proposal the parser's `E1002` retires from the hexadecimal and binary paths entirely and `E0003` becomes their only range error.

Each message gets a friendly-content regression pin.

---

## Drawbacks

- **Writing the constant does not make the algorithm work.** `>>` is **arithmetic** in every executor (`tests/spec/expressions/operators_bitwise.ori:234-239`; `ori_eval/src/operators/mod.rs:255`). SplitMix64, the stated motivator, is `(z ^ (z >> 30)) * 0xBF58476D1CE4E5B9` with a zero-fill `>>`. Under this proposal the constant compiles and the algorithm silently computes the **wrong result** where previously it was a compile error — a strictly worse failure mode in isolation. This proposal is only safe to use together with `logical-shift-operator-proposal.md`, which supplies `>>>`. That is why the Motivation above stops at the constants and does not claim the algorithm.
- **Two literal forms with different range rules.** A reader must know that `0xFFFFFFFFFFFFFFFF` is legal while `18446744073709551615` is not. The asymmetry is principled — magnitude versus bit pattern — but it must be taught.
- **A hex literal can silently be negative.** `0x9E3779B97F4A7C15 < 0` surprises anyone reading hexadecimal as an unsigned magnitude. This is inherent to a single-signed-type language, and it is exactly Java's `long` behavior.
- **Comparisons on wide hex constants read wrong.** `0xFFFFFFFFFFFFFFFF > 0` is `false`. The mitigation an earlier revision offered — that bit-pattern constants are used with bitwise operators "where signedness does not enter" — is **false**, as the first bullet establishes. There is no mitigation beyond the `>>>` dependency; the hazard is real and is stated rather than argued away.
- **Radix provenance is real plumbing.** A `TokenFlags` widening, a sparse arena side table, six parser sites, and four `ori_fmt` sites across two subsystems (§4). This is substantially more than the one-line change an initial reading of the grammar suggests.
- **One accepted form becomes a compile error.** §2 rejects `-<wide hex literal>`. It is the smallest possible break and it is loud, but it is not literally zero.

---

## Alternatives Considered

### Alternative 1: A const constructor — `int.from_bits(0x...)`

This was the withdrawn `pure-bit-operations-proposal.md`'s item 3. Rejected, and it does not work as designed:

- The argument `0xBF58476D1CE4E5B9` is itself an integer literal, rejected by the very rule that proposal declared unchanged. A function call cannot rescue an argument that is a compile error to write.
- `int` has zero associated functions anywhere in the spec. `10-declarations.md:566-574` governs only `impl Type: Trait`; there is no stated coherence rule for an inherent `impl` on a primitive, and `10-declarations.md:472` closes the extension route. The construct would establish inherent-impls-on-primitives with no coherence story.
- `grammar.ebnf:683-687` `const_expr` admits literals, `"$" identifier`, binary and unary operators, and parentheses — **no call production**. A `from_bits` call is not writable where a constant expression is required.
- `overflow-behavior-proposal.md:251-261` already settled this API-shape family, rejecting operator and method forms in favor of free functions with named arguments. An associated function on a primitive is the declined shape.

The lexical relaxation discharges all four, adds no API, and establishes no new precedent.

### Alternative 2: A distinct `TokenKind::RadixInt(u64)`

Rejected; §4 gives the argument. The type-level-safety premise is inverted, because the narrowing sites are `if let` bindings that fall through rather than exhaustive matches that fail, while roughly a dozen discriminant-matching consumers — including a `TokenSet` const-builder and the formatter's spacing category — would change behavior silently. A new tag additionally has to stay below `128` to avoid disabling postfix operators on hex literals.

### Alternative 3: Emit a distinct kind only when the value exceeds `int.max`

A narrower version of Alternative 2: keep `TokenKind::Int` for every literal that compiles today and use the new kind only for wide ones, so the regression surface is zero. Genuinely safe on the discriminant axis, and rejected only because it does not solve the *other* half of the problem: `ori fmt` decimalizes **narrow** hex literals too (`0xFF` -> `255`), so provenance is needed for every hexadecimal and binary literal, not only wide ones. A mechanism scoped to wide literals leaves the formatter defect in place.

### Alternative 4: Write the constants as negative decimals

Available today; rejected as the standard mechanism, for the reviewability reasons in Motivation. This proposal does not forbid it — a negative decimal remains a legal way to write any `int`.

### Alternative 5: A literal suffix — `0x...u`

Rejected. Ori has no literal-suffix syntax anywhere in its grammar; introducing one for a single case adds lexical surface, and the suffix would mean "interpret these bits as unsigned" in a language with no unsigned type. The suffix would be a lie about the resulting type. See Prior Art for the honest cost of declining it.

### Alternative 6: An unsigned 64-bit type

Rejected. `annex-e:29` commits to a single signed integer type. Adding `u64` to express constants would import conversion rules, mixed-arithmetic rules, and a second literal range across the entire numeric surface.

### Alternative 7: Widen decimal literals too

Rejected. A decimal literal denotes a magnitude. `18446744073709551615` denoting `-1` is a readability regression with no compensating benefit — nobody publishes a bit pattern in decimal. Keeping decimal bounded is what makes the hexadecimal relaxation principled rather than arbitrary.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:**

- Literal notation is lexical and syntactic surface. A library cannot change what a literal means.
- The narrowing decision lives in `ori_parse`; the range error lives in `ori_lexer` and `ori_diagnostic`; radix provenance crosses `ori_ir`, `ori_lexer`, `ori_parse`, and `ori_fmt`.

**Missing features that would enable purity:** Not applicable — this proposal is a notation change. It carries no runtime component, no type-system change, and no new API.

**Recommendation:** Proceed as a spec, parser, and formatter change. It is the smallest of the successors to `pure-bit-operations-proposal.md` in *rule* surface and not in *implementation* surface: one spec paragraph, one negation prohibition, a token-flag widening, a sparse arena side table, six parser sites, and four formatter sites.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `07-lexical-elements.md:237` | Split into a decimal rule (unchanged) and a hexadecimal/binary rule (64-bit pattern, **value**-based bound, two's-complement valuation); state that the valuation is a property of the literal token, so it applies in every position a literal may appear (§6) |
| `07-lexical-elements.md` (new) | The §2 prohibition on unary minus applied to a wide hexadecimal or binary literal |
| `07-lexical-elements.md:239` (NOTE) | Correct: `int.min` IS writable as `-9223372036854775808`, folded at `ori_parse/src/grammar/expr/mod.rs:382-388` and pinned at `tests/spec/lexical/int_literals.ori:242-247`, and under this proposal also as `0x8000_0000_0000_0000` |
| `07-lexical-elements.md` EXAMPLE block (`:235`) | Add a wide-hex example, a leading-zeros example, a rejected 65-bit example, and a rejected negated-wide-literal example |
| **Clause 8 (`08-types.md:360`)** | State that `[T, max N]`'s positivity requirement is a type-level constraint evaluated on the literal's **value**, so a wide hexadecimal capacity is a well-formed literal rejected as a non-positive capacity (§6) |
| `grammar.ebnf` | **No change.** `hex_lit` (`:100`) and `bin_lit` (`:101`) already parse unbounded digit sequences. Separately: `grammar.ebnf:101` names the production `bin_lit` while `07-lexical-elements.md:220` names it `binary_lit`. Pre-existing drift; this edit is the natural moment to align them, and this proposal adopts `bin_lit` to match the grammar |
| **Annex D (`annex-d-formatting.md`)** | **Change required.** `ori fmt` shall preserve a literal's radix. An earlier revision recorded "No change" here while the header declared the formatter under `Affects:`; that was inconsistent and is corrected (§4) |
| Annex E | No change. The `int` canonical range is unchanged; only its notation widens (§9) |
| Error codes | No new code for the range error — `E0003` already covers it (§Error Handling). One new diagnostic for the §2 prohibition |

### Errata

No approved proposal requires an erratum. `overflow-behavior-proposal.md` (§3), `unsafe-operation-gating-proposal.md` (§8), and `representation-optimization-proposal.md` (§9) are each cited and each unchanged — the interactions are analyzed and found empty, which is a different outcome from being unexamined. The §2 prohibition is what keeps the `overflow-behavior` interaction empty rather than overlapping.

### Conformance pins — enumerated

Searched `tests/spec/**`, `compiler/ori_lexer/**`, `compiler/ori_parse/**`, and `compiler/ori_fmt/**` for literal-range and radix pins:

| Pin | Asserts | Disposition |
|---|---|---|
| `compiler/ori_lexer/src/cooker/tests.rs:164` (`hex_integer`) | `0xFF` cooks to `Int(255)` | UNCHANGED |
| `compiler/ori_lexer/src/cooker/tests.rs:172` (`binary_integer`) | `0b1010` cooks to `Int(10)` | UNCHANGED |
| `compiler/ori_lexer/src/cooker/tests.rs:180` (`binary_integer_with_underscores`) | `0b1111_0000` cooks to `Int(240)` | UNCHANGED |
| `compiler/ori_lexer/src/cooker/tests.rs:196-200` (`integer_overflow`) | decimal beyond `u64` errors at cook time | UNCHANGED |
| `compiler/ori_lexer/src/lex_error/tests.rs:348-350` | `IntOverflow` / `HexIntOverflow` / `BinIntOverflow` map to `E0003` | UNCHANGED |
| `tests/spec/lexical/int_literals.ori:235-238` | `-0xFF == -255` | UNCHANGED — `0xFF` is far below `2^63`, so §2 does not reach it |
| `tests/spec/lexical/int_literals.ori:242-247` | `-9223372036854775808 == int.min` | UNCHANGED — decimal spelling, `I64_MIN_ABS` fold retained |
| Parser `E1002` sites (six, §5) | decimal beyond `i64` rejected | Decimal path UNCHANGED; hexadecimal and binary path changes from reject to reinterpret |

An earlier revision cited `cooker/tests.rs:238` as a hex round-trip pin. That line is inside `hex_int_no_digits_is_error`, which asserts `TokenKind::Error` on the input `0x`. The real round-trip pins are the three listed above.

No pin asserting that a wide **hex** literal is rejected exists anywhere in `tests/spec/**`, and no pin exercises `-0x8000000000000000` in any file (searched `tests/` and `compiler/` for `8000000000000000`: zero hits).

New pins required:

- Values: `0xFFFF_FFFF_FFFF_FFFF == -1`; `0x8000_0000_0000_0000 == int.min`; `0x7FFF_FFFF_FFFF_FFFF == int.max`; the equivalent binary forms; `0x0000_0000_0000_0000_00FF == 255` (leading zeros, value-based rule).
- Rejections: a 65-bit hexadecimal literal rejected with `E0003`; a decimal literal above `2^63 - 1` still rejected with `E1002` and the improved message; `-0x8000000000000000` rejected under §2 with its friendly-content pin; `-0xFFFF_FFFF_FFFF_FFFF` likewise.
- Positions: a wide hex literal in pattern position; as a range-pattern endpoint; in a const-generic bound; as a `[T, max N]` capacity, rejected as non-positive (§6); in `#repr("aligned", N)`, accepted or rejected per §6's disposition.
- **Formatter round-trip**, which is the pin that makes the feature non-inert: `ori fmt` on a file containing `0xFF`, `0b1010`, `0x9E3779B97F4A7C15`, and `0x8000_0000_0000_0000` reproduces each literal in its original radix. This pin fails today for all four.
- Evaluator and LLVM parity on the value of each accepted literal.

---

## Prior Art

| Language | Full-width signed pattern as a literal | Per-literal marker required? |
|---|---|---|
| Java | `long x = 0xBF58476D1CE4E5B9L;` — legal; hexadecimal, binary, and octal literals may set the sign bit, while a decimal literal may not exceed `Long.MAX_VALUE` | **Yes** — the `L` suffix |
| C# | `long x = unchecked((long)0xBF58476D1CE4E5B9);` | Yes — `unchecked` plus a cast; C# also has native `ulong`, so the pressure is lower |
| C / C++ | a hexadecimal constant exceeding `long long`'s range takes an unsigned type by the standard's type-assignment rules; an explicit cast yields the signed pattern | Yes — the cast |
| Rust | `0xBF58476D1CE4E5B9u64 as i64`, or `-4658895280553007687i64` | Yes — a literal suffix |
| Go | `int64(-0x40A7B892E31B1A47)`, or an untyped-constant expression that must fit the target type on conversion | Yes — the conversion |
| Swift | `Int(bitPattern: 0xBF58476D1CE4E5B9 as UInt)` | Yes — an explicit bit-pattern initializer |
| Zig | `@bitCast(@as(u64, 0x9E3779B97F4A7C15))` — `comptime_int` must coerce to a concrete type, and a value exceeding `i64` will not coerce to `i64` | Yes — an un-elidable `@bitCast` |

**The distinguishing structural fact, corrected.** An earlier revision claimed Java is the closest structural match because it has "one signed integer type at the notation level". That is inaccurate: Java has **four** signed integer widths. The parallel that actually holds — and it fully supports this design — is that Java has **no unsigned integer type**, so a full-width pattern has nowhere else to live and must be expressible as a signed literal. Every other language in the table has either an unsigned type or a coercion ceremony to carry the pattern.

**The honest cost of Alternative 5, stated.** Every language surveyed requires *some* per-literal marker: a suffix, a cast, a conversion, or a `@bitCast`. Under this proposal Ori would be the only one where a sign-bit-setting literal carries **no lexical marker at all** distinguishing `0x9E37…` (negative) from `0x1E37…` (positive). Zig is the sharpest contrast, because its `@bitCast` is deliberately un-elidable and is the strongest available argument *against* implicit reinterpretation. That argument is answered rather than avoided: Ori has one integer type, so a marker would carry no type information — a suffix would say "unsigned" in a language with no unsigned type, which is Alternative 5's rejection ground. The cost is accepted knowingly, and the §2 prohibition removes the one case where the missing marker would change an existing program's meaning.

No relevant issue-corpus entries were found. The issue corpus of reference language implementations was searched over literal-range and hex-literal phrasings and returned no on-point results, so no issue citations appear rather than approximate ones.

**Grounding note.** No Java repository is present in the on-disk reference corpus, so the JLS rule above is recorded as **not independently verified**. The Rust, Go, and Zig rows are corpus-verifiable; the C#, C/C++, and Swift rows are from language-reference knowledge.

---

## Migration / Breaking Changes

Almost entirely additive. One form is deliberately rejected.

- The change converts a class of currently-rejected programs into accepted ones. **No accepted program changes value.**
- Decimal literals are untouched.
- `ori fmt`'s output changes for every file containing a hexadecimal or binary literal: today it emits decimal, and after this proposal it preserves the radix. That is a change to formatter output on existing files, and it is the defect being fixed rather than a regression (§4).
- **`-<hexadecimal or binary literal with bit 63 set>` becomes a compile error** (§2). This is the sole break. Blast radius: searched `library/`, `tests/`, `compiler/`, and `docs/` for `8000000000000000` — zero occurrences, so no in-tree code is affected. The migration is to drop the minus sign; the diagnostic says so.
- No prelude, trait, API, or runtime surface changes.

### Absence claims and the searches that establish them

Surfaces searched for every claim: `docs/ori_lang/proposals/drafts/`, `docs/ori_lang/proposals/approved/`, `docs/ori_lang/v2026/spec/`, `library/`, `compiler/`, `tests/`.

| Claim | Result |
|---|---|
| No hexadecimal literal above `2^63 - 1` exists in Ori source | Zero hits — none can compile today |
| No pin asserts that a wide hexadecimal literal is rejected | Zero hits in `tests/spec/**` |
| No pin exercises `-0x8000000000000000` or `-0b1000…0` | Zero hits for `8000000000000000` across `tests/` and `compiler/` |
| `limbs-trait-proposal.md` is a wide-hex consumer | **Overstated in an earlier revision.** The single hit is `:311`, `U256.from_str("0xFF00FF00FF")` — a *string* argument, and the value is well within range. The draft is a plausible future consumer of wide masks; it is not a current one, and it is listed under `Related:` on that basis rather than cited as evidence |

---

## Roadmap Impact

Implementation touches `ori_ir` (`TokenFlags` widening, `ExprArena` side table), `ori_lexer` (set the radix flag), `ori_parse` (six narrowing sites, the §2 prohibition, populate the side table), `ori_fmt` (four emit and width sites), `ori_diagnostic` (two message improvements plus one new diagnostic), `ori_repr` (the `repr` alignment narrowing, §6), Clause 7, Clause 8, and Annex D. A feature plan scaffolded on approval owns the phase breakdown.

The formatter work is the load-bearing phase and shall not be sequenced after release of the lexical relaxation: shipping the relaxation without radix provenance produces a language where the feature works until the file is formatted.

---

## Unresolved Questions

- **A canonical grouping convention for wide hexadecimal.** Once `ori fmt` preserves radix, whether it should also normalize `0x8000000000000000` to `0x8000_0000_0000_0000` is a formatter question this proposal does not decide. Both spellings are accepted by the lexer; the formatter must pick one, and picking it is in scope for the Annex D edit, not for this document.
- **`#repr("aligned", N)` disposition.** §6 requires the `repr` narrowing to match the stated literal rule; whether an out-of-range alignment is a lexical error, an attribute-validation error, or a `ori_repr` error is an implementation choice constrained only by producing a diagnostic rather than the current silent `unwrap_or(u32::MAX)`.
- **`int.min` / `int.max` spelling.** Owned by `stdlib-math-api-proposal.md`; this proposal only requires the inaccurate NOTE at `07-lexical-elements.md:239` be corrected (§7).
- **Production-name alignment.** `bin_lit` versus `binary_lit` is pre-existing drift between the grammar and Clause 7. This proposal adopts `bin_lit`; whether the rename lands here or in a grammar-sync pass is a sequencing choice.
