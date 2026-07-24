# Proposal: Normalize Away the Redundant Trailing `()` in Void Blocks

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-05-29
**Affects:** Compiler (`ori_fmt`), spec (Annex D — Formatting)
**Related:** optional-semicolon-after-block-expressions-proposal.md, block-expression-syntax.md, built-in-lint-format-on-compile-proposal.md

---

## Summary

The formatter (`ori_fmt`) removes a redundant trailing `()` result expression from a block when at least one `;`-terminated statement precedes it. Per Clause 11.12, a block in which every expression is terminated by `;` already produces `void`, so a trailing `()` in that position is pure visual noise that produces the identical value. This is a formatter normalization only — no new syntax, no new keyword, no `return`, no semantic change, no new error code. Ori stays expression-only.

---

## Motivation

### The Problem in Practice

A `void`-returning function (commonly a test) often ends in a lone `()`:

```ori
@main () -> void = {
    let a = W(s: "shared");
    let b = a;
    let va = match a { W(s) -> s };
    let vb = match b { W(s) -> s };
    assert_eq(actual: va, expected: "shared");
    assert_eq(actual: vb, expected: "shared");
    ()
}
```

That trailing `()` reads as noise. A reader pauses on it — "what is this lone `()` for?" — when it carries no information: the preceding `assert_eq(...)` is already `;`-terminated, so per Clause 11.12 ("A block in which every expression is terminated by `;` produces `void`") the block is *already* `void` without it. The `()` is redundant. The canonical form is simply:

```ori
@main () -> void = {
    let a = W(s: "shared");
    let b = a;
    let va = match a { W(s) -> s };
    let vb = match b { W(s) -> s };
    assert_eq(actual: va, expected: "shared");
    assert_eq(actual: vb, expected: "shared");
}
```

Both forms produce `void`. The second is what the formatter should enforce, the same way `gofmt` silently normalizes away redundant style with zero configuration.

### When This Matters

- **Every `void` function and test** whose body ends in `()` after `;`-terminated work — the most common shape in test suites and `@main`.
- **AI-authored code**, which frequently emits a defensive trailing `()` out of habit from other languages. The formatter erasing it keeps canonical source uniform regardless of what the generator emitted (the stated goal of built-in-lint-format-on-compile-proposal.md).
- **New users** coming from ML/Rust who are unsure whether a `void` block "needs" a final expression. The formatter answering "no" by deleting it teaches the idiom mechanically.

### Relationship to the `return` Question

This proposal is the chosen resolution of a broader discussion about whether Ori should add a `return` keyword to make `void`-function endings "clearer." That discussion concluded **no**:

- Ori's design philosophy favors **one-way-to-do-things** and explicit-over-implicit; a `return` would be a second way to yield a function's value (the implicit tail expression being the first), which is the precise redundancy that makes Rust's `return x;`-at-the-tail "weird" and forces clippy's `needless_return` lint to police it.
- Ori already covers *early exit* — the only job Rust/Roc actually keep `return` for — via `?`, `break value`, `panic`, terminating `match`, and labeled blocks (`block:done { … break:done v }`, Clause 16.4).
- Roc, a pure expression-based language that started without `return`, added it (roc#7104) and acquired a fallout tail (stack corruption, effect-interaction bugs, expression-position errors). For Ori that class of bug lands directly on dual-execution parity (eval vs LLVM).

The real pain was never the absence of `return` — it was the cosmetic `()`. This proposal removes the cosmetic `()` without introducing the redundancy or the parity risk a keyword would.

---

## Design

### Normalization Rule

Add one rule to Annex D (Formatting):

> In a block expression `{ statement* expression? }` (Clause 11.12; grammar `block_expr = "{" { statement } [ expression ] "}"`), when the optional trailing result `expression` is exactly the unit literal `()` **and** at least one `statement` precedes it, `ori_fmt` removes the `()`. The resulting block produces `void` via the all-`;` rule (Clause 11.12).

Because every item preceding the result expression is by definition a `;`-terminated statement, removing the `()` always transforms a `void` block into a `void` block. The change is observably semantics-preserving in every case the rule fires.

### The Empty-Block / Empty-Map Edge Case (load-bearing)

The rule fires **only when at least one statement precedes** the `()`. It must NOT fire on a sole-`()` block:

```ori
{ () }      // MUST stay as-is — do NOT normalize to { }
```

Per Clause 14 (`14-expressions.md`: "The syntax `{ }` (braces with whitespace) is parsed as an empty map literal"), `{ }` is an **empty map literal**, not a `void` block. So `{ () }` is the canonical way to write an empty `void` block, and erasing its `()` would silently change the expression's type from `void` to an empty map. The preceding-statement precondition is what makes the normalization safe; it is not optional.

### Scope — What Is and Isn't Targeted

| Form | Action | Reason |
|------|--------|--------|
| `{ stmt; (); … stmt; () }` (trailing result `()`, ≥1 preceding statement) | Remove the trailing `()` | Redundant; block stays `void` |
| `{ () }` (sole `()`, zero preceding statements) | Keep | Removing yields `{ }` = empty map (type change) |
| `()` as a sub-expression, grouping, argument, or tuple unit elsewhere | Keep | Not a block result expression |
| `();` as a non-final statement | Keep | Not the result expression; out of scope (a separate redundancy if any) |

Only the block's trailing result expression, when it is literally `()`, is in scope.

### Idempotence

The rule is idempotent: formatting already-normalized source produces identical output (the formatter's idempotence invariant). Running it twice removes nothing the first pass left.

### Examples

```ori
// Before                                  // After
@f () -> void = {                          @f () -> void = {
    setup();                                   setup();
    work();                                    work();
    ()                                     }
}

// Branch arms normalize independently:
if c then { a(); () } else { b(); () }     // -> if c then { a(); } else { b(); }

// Preserved (sole () = empty void block, not empty map):
@noop () -> void = { () }                   // unchanged
```

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** Formatting is owned by the `ori_fmt` compiler crate and specified by Annex D (Formatting). A normalization rule is compiler/tooling behavior, not a library feature.
**Missing features that would enable purity:** None applicable — formatting is inherently a toolchain concern.
**Recommendation:** Proceed as a formatter change. It is the leanest possible compiler change: one normalization rule in `ori_fmt`, no grammar change (the grammar already permits both forms), no semantic change, no new error code, no new keyword.

---

## Alternatives Considered

### Alternative 1: Add a `return` keyword (`return ()` / general `return`)

Make `void` endings explicit with `return ()`, or add a general `return` for early exit and tail.

**Rejected.** Violates **one-way-to-do-things**: a second mechanism for yielding a function's value, which is the redundancy clippy's `needless_return` exists to suppress. Ori already covers early exit via `?` / `break value` / `panic` / labeled blocks (Clause 16). Roc demonstrated the correctness tail of bolting `return` onto an expression-based evaluator (roc#7104 + follow-on bugs), which for Ori would threaten dual-execution parity. `return ()` is also strictly more characters than the `;`-termination this proposal already makes canonical.

### Alternative 2: A standalone "redundant `()`" lint (E73xx)

Emit a Clarity lint flagging the redundant trailing `()` instead of auto-removing it.

**Rejected — SRP.** The formatter deleting the `()` (gofmt-style) needs no diagnostic at all; a lint would be a parallel mechanism solving a problem the formatter already solves. It also conflicts with built-in-lint-format-on-compile-proposal.md, whose lint system is **errors-not-warnings with no escape hatches and no "optional" lints**. If that draft is approved, this normalization is simply one of the format-on-compile normalizations applied automatically; if it is not, `ori fmt` applies it on demand. Either way the home is the formatter, not a new lint rule.

### Alternative 3: Make `{ }` unambiguously a `void` block

Drop the empty-map-from-`{ }` parse (Clause 14) so that `()` could always be removed, including the sole-`()` case.

**Rejected.** Far larger change; breaks the empty-map literal that Clause 14 specifies and that existing code relies on. The preceding-statement precondition in this proposal handles the edge case without disturbing map literals.

### Alternative 4: Do nothing (document `;`-termination as preferred)

Leave both forms valid and note in a guide that `;`-termination is idiomatic.

**Rejected.** Leaves the eyesore in real code and offloads a style choice onto every author — the opposite of the formatter's mandate (one canonical shape, no options). A canonical formatter normalizes; it does not advise.

---

## Spec & Grammar Impact

| File | Change |
|------|--------|
| `docs/ori_lang/v2026/spec/annex-d-formatting.md` | Add the normalization rule (trailing-`()`-removal in `void` blocks with the ≥1-preceding-statement precondition + the empty-map edge-case note). |
| `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` | No change — Clause 11.12 already defines both forms as `void`; this is a formatter normalization between two already-valid forms. |
| `grammar.ebnf` | No change — `block_expr = "{" { statement } [ expression ] "}"` already permits the result expression to be present or absent. |
| Error codes | None — semantics-preserving normalization, no diagnostic. |

**Coordination with built-in-lint-format-on-compile-proposal.md:** that draft makes formatting a compiler phase applied on every command. This normalization is one rule that phase would apply automatically; the two compose without duplication.

---

## Prior Art

### Internal (Ori)

- **optional-semicolon-after-block-expressions-proposal.md** (Approved) — the direct precedent: a redundant token (the `;` after a `}`-ending statement) that the toolchain absorbs rather than forcing the author to manage. Same shape, same goal, same author; this proposal is its sibling for the trailing `()`.
- **block-expression-syntax.md** (Approved) — establishes the block-value rule and the all-`;`-block-is-`void` semantics this normalization relies on ("A block where every expression has `;` is a void block (like Rust)"). It also notes "the type checker is the safety net" — the same safety net guarantees the normalization is type-preserving.
- **built-in-lint-format-on-compile-proposal.md** (Draft) — the format-on-compile machinery; this normalization slots in as one of its rules. That draft already assumes the no-`return` design ("Ori functions should be short — expression-based, no `return` for early exit").

### External

- **Go (`gofmt`)** — zero-configuration formatter that silently normalizes redundant style on every run. The model Ori's formatter already follows; trailing-`()` removal is in the same spirit.
- **Rust (clippy `unused_unit`)** — detects an "unneeded unit expression" such as a trailing `()` and removes the final `()`. The closest external analog. Its known failure mode (rust-clippy#9949 — removing `()` breaks when an attribute precedes the unit expression, leaving the attribute unattached) is exactly why this proposal's removal rule is *conservative* (preconditioned on a preceding statement, scoped to the block result expression only).
- **Koka (koka#521)** — open request for a code formatter; an expression-oriented language without a canonical formatter yet, underscoring the value of building normalization into the toolchain from the start.
- **Roc (roc#2240)** — formatter output normalization (trailing-whitespace removal); evidence that expression-based formatters routinely own this class of "delete the redundant" rule.
