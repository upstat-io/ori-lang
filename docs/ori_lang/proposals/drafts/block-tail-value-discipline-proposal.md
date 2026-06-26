# Proposal: Block-Tail Value Discipline

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-26
**Affects:** Compiler (parser), formatter (`ori_fmt`), spec (Clause 11, Clause 16), grammar (Annex A), Annex D (formatting)
**Amends:** block-expression-syntax.md, optional-semicolon-after-block-expressions-proposal.md

---

## Summary

At the tail of a block body, make the trailing `;` and the formatter's blank-line placement carry one reliable, bidirectional signal: whether the tail expression **is the block's produced value** or is **evaluated for effect and discarded**. A produced-value tail carries no `;` and a blank line above it; a discarded/effect tail carries a `;` (for simple, non-block-ending tails) and no blank line above it. This formalizes what the spec already half-encodes and tightens the one case where the signal is currently ambiguous — a void/discarded simple-call tail, where the `;` is presently optional.

---

## Motivation

Ori is expression-based: a block's value is its last expression, identified by the absence of a trailing `;` (`block-expression-syntax.md §Last Expression Is the Value`, spec §11.12). The design already wanted **two visual signals** to make the result expression unmistakable: no semicolon, and a formatter-enforced blank line above it.

Today those signals are only *one-directional*:

- A produced **value** tail reliably has **no** `;` — a stray `;` makes the block `void`, which the type checker rejects against a non-`void` return type.
- But a **void/discarded** tail does **not** reliably have a `;`. Both forms below are currently legal and both yield a `void` block:

```ori
@main () -> void = {
    setup();
    cleanup()       // legal today: no `;`, block value is cleanup()'s void
}

@main () -> void = {
    setup();
    cleanup();      // also legal today: `;`, statement, block is void
}
```

Because the no-`;` form is legal on a void tail, "no semicolon" does **not** reliably mean "this produces a value." A reader (or an AI generator) scanning a multi-statement body cannot trust the semicolon alone to tell value from effect.

### The Problem in Practice

The discomfort is sharpest in multi-line methods, where the eye has to hunt for which line is the value:

```ori
@apply_discount (order: Order) -> Order = {
    let $rate = lookup_rate(order:);
    let $capped = min(left: order.subtotal * rate, right: order.ceiling);
    record_metric(order:, capped:)      // effect — but is THIS the value?

    Order { ...order, total: capped }   // the actual value
}
```

When the effect line and the value line are typographically indistinguishable, the value "hangs off" the body with no marker. The fix is not a `return` keyword (that imports a control-flow jump Ori does not have — see Alternatives), but making the *existing* signals reliable in both directions.

### When This Matters

- **Every multi-statement function or method** that mixes effectful calls with a produced result.
- **Void-returning bodies** that end in a side-effecting call — the currently-ambiguous case.
- **AI code generation**, where a reliable, mechanical "no `;` + blank line above ⟺ value" rule is easier to emit and verify than an implicit convention.

---

## Goals and Non-Goals

**Goals:**

- Make the block-tail semicolon and blank-line signals reliable and bidirectional: the syntax alone tells produced-value from discarded-effect.
- Tighten the one ambiguous case (simple void/discarded tail) so it requires `;`.
- Add the formatter's missing negative rule: no blank line above a discarded/void tail.
- Ship two targeted diagnostics (D1, D2) that name the value/effect cause and the exact fix, replacing the oblique `found void` mismatch on the non-void side.

**Non-Goals:**

- No `return` keyword and no early-exit-from-function mechanism (out of scope; rejected in Alternatives).
- No change to the block-value *semantics* — the last no-`;` expression is still the block value, exactly as today.
- No change to the optional-`;`-after-`}` rule for block-ending tails (spec §11.12.1) — this proposal scopes around it, not over it.
- No change to the declaration terminator on the braceless expression-bodied form (`@f (...) -> R = expr;`).

---

## Design

The governing distinction is **role, not type**: is the tail expression the block's *produced value*, or is it *evaluated for effect and its value discarded*? Per spec §16.0.1, "any expression terminated by `;` is used as a statement (its value is discarded)." This proposal keys every rule on that produce-vs-discard axis, which correctly covers the case of discarding a value-returning call (a `void` body may end by calling a value-returning function and dropping its result — that tail is discarded, not produced).

### Rule 1 — Produced-value tail

A tail expression that **is** the block's value:

- carries **no** trailing `;` (unchanged from today), and
- has a **blank line above it** (unchanged — `ori fmt` already enforces this in setup+result blocks).

### Rule 2 — Discarded/effect tail

A tail expression **evaluated for effect** (its value discarded) — this includes void-returning calls, value-returning calls whose result is dropped, assignments, and diverging calls such as `panic(...)`:

- carries a trailing `;` **when its last token is not `}`** (the new tightening — see Rule 3 for the `}` carve-out), and
- has **no blank line above it**.

### Rule 3 — Block-ending tails keep the optional `;`

When the tail's last token is `}` (`match { }`, `if...then { }`, `for...do { }`, `while...do { }`, `loop { }`, `unsafe { }`, `block:label { }`, bare `{ }`), the trailing `;` remains **optional** per spec §11.12.1. Rule 2's mandatory `;` applies only to simple (non-`}`-ending) tails — call statements and assignments. This preserves the approved optional-semicolon rule, which exists specifically to kill the `};`-after-a-closing-brace friction.

For block-ending tails the **blank-line rule (Rules 1/2) is the carrier of the value/effect signal**, since the semicolon is uncommitted there:

```ori
@step (g: Game) -> Game = {
    let $advanced = advance(g:);

    match advanced.event {        // value tail: blank line above, no `;`
        Some(e) -> handle(e:)
        None    -> advanced
    }
}

@dispatch (e: Event) -> void = {
    log(e:);
    match e {                     // effect tail: no blank line above, `;` optional (}-ending)
        Click(p) -> on_click(p:)
        Key(k)   -> on_key(k:)
    }
}
```

### Rule 4 — Scope: block bodies only

These rules govern the **tail position inside a block body `{ }`**. The braceless expression-bodied declaration form is unaffected:

```ori
@double (x: int) -> int = x * 2;      // `;` here is the DECLARATION terminator, always present
```

The `;` in `= expr;` terminates the *declaration* (the same role it plays in `let $x = 5;` and constant declarations), not a block-internal statement. Keeping it preserves the invariant that every declaration ends in `;`, and it does not muddy the block-internal signal because it is a different grammatical position. (The alternative — dropping `;` on braceless value bodies for a fully global signal — is considered and rejected below.)

### Signal summary

| Tail | `;` (simple, non-`}`) | `;` (block-ending, `}`) | Blank line above |
|---|---|---|---|
| Produced value | none | none | yes |
| Discarded / effect | required | optional (§11.12.1) | no |

Three mutually-reinforcing signals (semicolon, blank line, and the value itself) all encode the same distinction; the blank line is the universal carrier across all tail shapes, the semicolon the reinforcing carrier for simple tails.

### Semantics

No runtime or type-system semantics change. The block value is still the last no-`;` expression; an all-`;` block is still `void`. Rule 2's tightening only **removes a currently-legal spelling** (a no-`;` simple-call void tail), it does not change what any well-formed program evaluates to.

### Error Handling

Two diagnostics flank the value/effect boundary, one per direction. Both are named deliverables of this proposal (with positive and negative regression pins).

**D1 — discarded simple tail missing `;` (the new void-side teeth).** A simple (non-`}`-ending) tail whose value is discarded but which omits the required `;`:

```
error: tail expression evaluated for effect must end with `;`
  --> snake.ori:7:5
   |
 7 |     cleanup()
   |     ^^^^^^^^^ add `;` here — this call's result is discarded, so it is a statement, not the block value
   |
   = note: omit `;` only on the expression that IS the block's value
```

**D2 — `;` on the tail of a non-void body (the symmetric, targeted message).** Today this surfaces as an oblique type mismatch (`expected int, found void`) because the trailing `;` makes the block produce `void`. This proposal adds a targeted diagnostic that detects the shape — a tail terminated by `;` in a body whose declared return type is non-`void` — and points at the real fix:

```
error: this `;` discards the block's value, but `apply_discount` must produce `int`
  --> discount.ori:6:38
   |
 6 |     Order { ...order, total: capped };
   |                                      ^ remove this `;` to make this expression the return value
   |
   = note: a trailing `;` turns the tail into a discarded statement, so the block produces `void`
```

The wrong-*type* tail (a no-`;` tail whose type is neither the declared return type nor `Never`) remains the standard type-mismatch diagnostic — no special message is warranted, since the type checker already names the expected and found types precisely.

### Symmetric case — non-void return types

The semicolon selects the block's value type, and the declared return type decides which spelling is legal. The two directions are mirror images:

| Body | Valid tail spelling | Invalid spellings |
|---|---|---|
| `-> void` | `cleanup();` (discarded) | `cleanup()` no `;` becomes ill-formed under Rule 2 (D1) |
| `-> int` | `capped` no `;` (produced value) | `capped;` → block is `void`, type error (D2); `"hi"` no `;` → wrong-type mismatch |

The `-> int` direction is already enforced by the type checker today (the block-value type must equal the declared return type); this proposal only sharpens its message (D2). The `-> void` direction is where the new well-formedness rule (Rule 2 / D1) adds teeth. Diverging tails (`panic`, `todo`, `unreachable`) are the carve-out tracked in Unresolved Questions: in a non-`void` body the diverging tail is written **without** `;` so its `Never` type coerces to the return type — it is a value tail, not a discarded one.

The companion `ori fmt` pass enforces the blank-line placement (blank line above a produced-value tail; none above a discarded tail), consistent with Ori's single-canonical-format design.

---

## Drawbacks

- **It is a (small) breaking change.** Programs ending a `void` body with a no-`;` simple call — `@main () -> void = { ...; cleanup() }` — become ill-formed and must add the `;`. This is mechanically migratable (`ori fmt` can apply it) but it does churn existing code, including stdlib.
- **Two terminator positions remain.** The braceless `= expr;` form keeps its declaration `;` even for value bodies, so "no `;` ⟺ value" holds *inside braces* but not for the one-liner form. This is defensible (different grammatical role) but is a learnable nuance, not zero.
- **The semicolon signal is partial for block-ending tails.** Because §11.12.1's optional-`;` is preserved, a `}`-ending void tail can appear with or without `;`; the value/effect distinction there rests on the blank line alone. Full bidirectional `;` would require mandating `;` after `}`, which this proposal declines (it would revert an approved fix).
- **Surface-area growth in the formatter.** A new negative blank-line rule is one more thing `ori fmt` must enforce and one more thing to test.

---

## Alternatives Considered

### Alternative 1: `return` keyword for methods

Add an explicit `return` for method bodies (mandatory or optional). Rejected: `return` is a control-flow jump everywhere it exists; Ori has no early-exit-from-function mechanism (exits are `break`, `?`, `panic`), and a function body is structurally an expression whose value *is* its tail, identical to `if`/`match`. A mandatory tail-only `return` that cannot jump miscommunicates control flow to humans and AI; an optional `return` adds a second way to do one thing (one-way-to-do-things is a stated language principle) and does not even remove the bare-tail spelling. (Full analysis in the discussion that produced this proposal; see also `block-expression-syntax.md §Not return`.)

### Alternative 2: Mandate `;` on all void tails, including `}`-ending

Make every discarded tail carry `;`, restoring a fully bidirectional semicolon signal. Rejected: it re-introduces `};` after a closing brace, partially reverting `optional-semicolon-after-block-expressions-proposal.md` (approved 2026-04-13), which removed exactly that friction. The blank-line rule already carries the signal for block-ending tails, so the semicolon mandate is unnecessary there.

### Alternative 3: Drop `;` on braceless value bodies (global signal)

Make `@double (x: int) -> int = x * 2` (no `;`) the value form and require `;` only on braceless void bodies, so "no `;` ⟺ value" holds everywhere including one-liners. Rejected (tentatively — see Unresolved Questions): it makes function declarations the only `= …` declarations without a terminator, breaking the "every declaration ends in `;`" consistency that `let $x = 5;` and constants share. It trades block-level consistency for declaration-level inconsistency. Scoping the rules to block bodies (Rule 4) avoids the trade.

---

## Composition with `redundant-trailing-unit-normalization`

The draft `redundant-trailing-unit-normalization-proposal.md` occupies the adjacent void-block-tail design space and shares the same "no `return` keyword" lineage (`block-expression-syntax.md §Not return`). The two are **orthogonal-but-aligned**: they normalize *different* tail shapes via *disjoint* triggers and converge to the same canonical void form.

| Tail shape | `redundant-trailing-unit` | This proposal (Rule 2) | Composed canonical form |
|---|---|---|---|
| Literal `()` unit after ≥1 `;`-statement (`{ work(); () }`) | formatter DELETES the `()` | not in scope (no call/effect) | `{ work(); }` — all-`;`, `void` |
| Void/discarded simple CALL or assignment tail (`{ work(); cleanup() }`) | not in scope (not a literal `()`) | requires `;` → `{ work(); cleanup(); }` | `{ work(); cleanup(); }` — all-`;`, `void` |

- **No conflict — disjoint triggers.** `redundant-trailing-unit` fires ONLY on a literal `()` result expression; this proposal's Rule 2 fires ONLY on a void/discarded *call* or assignment tail. No input matches both.
- **Same destination.** Both converge a void-block tail to the canonical all-`;` form — a `()` tail is deleted, a void-call tail gains its `;` — and neither yields a produced-value tail. The two are complementary realizations of one "make void-block endings canonical without a `return`" intent.
- **Formatter ordering.** Apply `redundant-trailing-unit`'s `()` deletion FIRST (it removes the tail outright); this proposal's `;`-mandate then has nothing to act on for that case. The order is observational only (disjoint triggers make it commutative in effect).
- **No dependency.** Each proposal stands alone and is independently approvable. They are RECOMMENDED to land together for a single coherent void-block-tail story, but neither blocks the other.

This resolves Unresolved Question #4.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** This is a surface-grammar and formatter change. It alters the `statement` grammar production (the trailing-`;` requirement on simple tail statements) and adds a blank-line rule to `ori_fmt`. Neither is expressible as a library.
**Missing features that would enable purity:** None applicable — syntax and canonical-format rules are inherently compiler/tooling concerns.
**Recommendation:** Proceed as a compiler + formatter proposal. No new keywords; the parser change is narrow (mandate `;` on a simple tail statement whose value is discarded), and the formatter change is one new blank-line invariant.

---

## Spec & Grammar Impact

- **Clause 11 (`11-blocks-and-scope.md` §11.12 / §11.12.1):** Add that a simple (non-`}`-ending) tail expression evaluated for effect shall be terminated by `;`; the block value is the sole tail that omits `;`. Preserve §11.12.1's optional-`;`-after-`}` unchanged; cross-reference the produce-vs-discard axis.
- **Clause 16 (`16-control-flow.md` §16.0.1):** Reinforce the produce-vs-discard framing already present ("any expression terminated by `;` is used as a statement").
- **Grammar (`grammar.ebnf`, `statement` production):** Distinguish a simple tail statement (requires `;`) from a block-ending tail statement (optional `;`, per the existing §11.12.1 production) from the block-value expression (no `;`).
- **Annex D (`annex-d-formatting.md`):** Add the negative blank-line rule — no blank line above a discarded/effect tail — complementing the existing positive rule (blank line above a produced-value tail).
- **`.claude/rules/ori-syntax.md`:** Update the "Semicolon rule" and "Block expressions" entries to state the produce-vs-discard signal.
- **Diagnostics (`ori_diagnostic`):** Allocate a parser/well-formedness code for D1 (simple discarded tail missing `;`) and a type-checker code for D2 (`;` on the tail of a non-void body), each with `ori --explain` extended docs and the structured "remove/add the `;`" suggestion. D2 specializes the existing tail-produces-`void` mismatch rather than replacing the general type-mismatch path.

---

## Prior Art

- **Rust** — Ori's direct ancestor here. Rust blocks are expression-oriented: the tail expression without `;` is the block value; an expression with `;` is a statement whose value is discarded. Rust does not *mandate* `;` on a discarded tail call the way this proposal does for simple tails, and Rust's tooling (`clippy::needless_return`, `unused_must_use`) layers conventions on top. Ori tightens what Rust leaves to lint.
- **Ruby, Scala** — expression-oriented, last expression is the value, `return` optional and discouraged at the tail. No semicolon discipline (newline-terminated), so no analogue to the value/effect semicolon signal.
- **Gleam, Elm, Roc, Koka** — functional, expression-oriented, no `return`; the block/let-chain tail is the value. Closest in spirit to Ori's structural-value model; none carries an explicit discarded-tail terminator because they have few or no bare effect-statements.
- **Go** — statement-oriented with mandatory `;` (inserted by the lexer) and an explicit `return`; the opposite design point. Cited as the contrast: Ori keeps structural value + tooling-enforced single format rather than statement-oriented explicit return.

The novel element — using the trailing `;` plus a canonical-formatter blank line as a *bidirectional* value/effect signal — is enabled by Ori's single-canonical-format design (`ori fmt` owns blank-line placement, so the blank line is a hard invariant rather than author discretion). No surveyed language pairs both signals this way.

---

## Unresolved Questions

- **Braceless scoping (Rule 4 vs Alternative 3):** Resolve during review — scope the rules to block bodies (recommended, Rule 4) and keep the braceless `= expr;` declaration terminator, or go global (Alternative 3) and drop `;` on braceless value bodies. The recommendation is Rule 4; this is the primary design decision the review gate should ratify.
- **Diverging tails (`panic`, `todo`, `unreachable`, `break`):** Confirm these are treated as discarded/effect tails (Rule 2 — `;` when simple) rather than value tails via `Never`-coercion. Expected: effect tails. Resolve during review.
- **Migration tooling:** Confirm `ori fmt` can mechanically apply both the `;`-insertion (Rule 2) and the blank-line normalization (Rules 1/2) so the breaking change is a one-shot reformat. Resolve during implementation.
- **Interaction with `redundant-trailing-unit-normalization` (draft):** RESOLVED — see `## Composition with redundant-trailing-unit-normalization`. The two have disjoint triggers (literal `()` deletion vs void-call `;`-mandate) and converge to the canonical all-`;` void form; no conflict, no dependency, recommended to land together.
