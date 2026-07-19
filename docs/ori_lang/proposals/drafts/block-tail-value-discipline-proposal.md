# Proposal: Block-Tail Value Discipline

**Status:** Draft
**Author:** Eric
**Created:** 2026-06-26
**Affects:** Compiler (type checker — diagnostic D2 only), formatter (`ori_fmt`), spec (Clause 7, Clause 11, Clause 16, and the type-checking clause that hosts the normative rule), Annex D (formatting) — grammar UNCHANGED (the discipline is a type-checker diagnostic + a formatter/`ori fix` normalization over the existing `statement` production)
**Amends:** block-expression-syntax.md
**Depends On:** ori-fix-proposal.md (the general-purpose `ori fix` apply-driver; the D1 void-tail `;` normalization and the canonical `Never`-tail spelling route through it)
**Related:** optional-semicolon-after-block-expressions-proposal.md, redundant-trailing-unit-normalization-proposal.md

---

## Summary

At a block tail, ground the value/effect distinction in the tail expression's **type**. A no-`;` simple (non-block-ending) tail of **non-`void`** type is the block's produced value; a **`Never`** tail is a value tail by coercion; a no-`;` simple tail of **`void`** type is a discarded effect whose canonical form carries `;`. Two mechanisms cooperate: the **formatter / `ori fix`** normalizes a no-`;` void simple tail to its `;`-terminated canonical form (D1 — a normalization, NOT a hard error), and the **type checker** keeps the real value-discard diagnostic (D2 — an errant `;` that drops a value the body must produce). The result is a reliable signal **in canonical / formatted source**: for a concrete, non-`()`, simple tail, no `;` means the block produces a non-`void` value (or diverges).

The discipline is intentionally toolchain-grade for the void-tail direction (the formatter owns the `;`), matching every surveyed expression-based language, while keeping the type checker's teeth on the one direction that is a genuine type error (discarding a produced value).

---

## Motivation

Ori is expression-based: a block's value is its last expression, identified by the absence of a trailing `;` (`block-expression-syntax.md §Last Expression Is the Value`, spec §11.12, §7.8.1, §16.0.2). The design already wanted **two visual signals** to make the result expression unmistakable: no semicolon, and a formatter-enforced blank line above it.

Today the semicolon signal is only *one-directional*:

- A produced **value** tail reliably has **no** `;` — a stray `;` makes the block `void`, which the type checker rejects against a non-`void` return type.
- But a **void** tail does **not** reliably carry a `;`. Both forms below are currently legal and both yield a `void` block:

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

Because the no-`;` form is legal on a void tail, "no semicolon on a simple tail" does **not** reliably mean "this produces a (non-`void`) value." A reader scanning a multi-statement body cannot use the semicolon to tell a produced value from a dropped effect.

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

When the effect line and the value line are typographically indistinguishable, the value "hangs off" the body with no marker. The fix is not a `return` keyword (that imports a control-flow jump Ori does not have — see Alternatives), but making the tail's **type** decide the canonical spelling, having the **formatter** normalize the void-tail `;`, and keeping the **type checker**'s diagnostic on the one case that is a real error (discarding a produced value).

### When This Matters

- **Every multi-statement function or method** that mixes effectful calls with a produced result.
- **Void-returning bodies** that end in a side-effecting call — the currently-ambiguous case the formatter normalizes.
- **Tooling and readers**: a canonical formatted shape plus a targeted value-discard diagnostic gives a reliable signal in formatted source, rather than a convention nothing backs.

---

## Goals and Non-Goals

**Goals:**

- Make the no-`;` simple-tail spelling reliable **in canonical / formatted source**: for a **concrete, non-`()`, simple** (non-`}`-ending) tail, no `;` means the block produces a non-`void` value (or diverges). `Never`, `}`-ending tails, the literal `()`, and type-variable tails are documented exceptions (see Signal summary).
- Normalize the one ambiguous case (a no-`;` void simple tail) to its `;`-terminated canonical form via the **formatter / `ori fix`** (D1) — NOT a hard well-formedness error.
- Keep the type checker's teeth on the real error: a `;` that discards a value the body must produce (D2), and **extend D2 to `}`/`]`-ending value tails** so the motivating multi-line case gets a targeted message.
- Add the formatter's missing **syntactic** negative rule: no blank line above a `;`-terminated tail statement (complementing the existing blank-line-above-the-no-`;`-value rule).

**Non-Goals:**

- No `return` keyword and no early-exit-from-function mechanism (rejected in Alternatives).
- No change to the block-value *semantics* — the last no-`;` expression is still the block value, exactly as today.
- No change to the optional-`;`-after-`}` rule for void/`Never` block-ending tails (spec §11.12.1) — this proposal scopes around it and explicitly preserves it.
- No change to the braceless expression-bodied declaration terminator (`@f (...) -> R = expr;`).
- No new type-checker dependency inside `ori_fmt` — the formatter stays parse-only for its blank-line rule; the type-dependent void-tail `;` insertion lives in `ori fix` (post-type-check), not `ori_fmt`.
- No hard well-formedness error for a no-`;` void simple tail — that direction is toolchain-normalized, not rejected.

---

## Design

The governing distinction is the tail expression's **type**. A no-`;` non-`void` tail *is* the block's value (you cannot drop its result at the tail — dropping requires a `;`, which makes it a statement); a no-`;` `void` tail is the ambiguous case the formatter normalizes; a `;` on a tail that *should* be the value is the real error D2 catches. The void-direction normalization needs the tail's type, so it lives in `ori fix` (post-type-check), not the parser and not `ori_fmt`'s parse-only pass.

### Tail-shape classification (normative for the rules below)

A tail expression is classified by its **last token**, applied uniformly:

- **Simple tail** — last token is NOT `}` or `]`. Examples: a call `cleanup()`, a bare identifier `x`, a field/index access `a.b` / `arr[i]`, an operator expression, the literal `()`.
- **Block-ending (`}`/`]`) tail** — last token IS `}` or `]`. This covers BOTH control-flow block-enders (`match { }`, `if...then { }`, `for...do { }`, `while...do { }`, `loop { }`, `unsafe { }`, `block:label { }`, bare `{ }`) AND **data literals** (`Point { x, y }` struct, `{ k: v }` map, `[a, b]` list). The classification is the same "last token" test approved in §11.12.1; this proposal applies it to data literals explicitly so no tail shape is unclassified.

### Rule 1 — Produced-value tail (no-`;`, non-`void` type)

A simple or block-ending tail expression with **no** trailing `;` whose type is **non-`void`** is the block's produced value:

- it is the block value (unchanged semantics), and
- the formatter places a **blank line above it** (the existing positive rule; keyed *syntactically* on "last expression, no `;`").

### Rule 2 — Void simple tail is normalized to `;` (D1 — normalization, not error)

A **simple** (non-`}`-ending) tail expression whose **type is `void`**, written without `;`, is **normalized** to its `;`-terminated canonical form by the formatter / `ori fix` (D1), **except the literal unit expression `()`** (carved out below). A no-`;` void simple tail is **legal** — `ori fix` simply inserts the `;`:

```ori
{ setup(); cleanup() }   ->   { setup(); cleanup(); }   // ori fix inserts the `;`
```

This is purely a *canonical-form* normalization: the block still produces `void` either way; one spelling is selected as canonical. It is **not** a hard well-formedness error — Ori does not reject the no-`;` form, the toolchain normalizes it, matching the model the sibling `redundant-trailing-unit-normalization` proposal uses for the literal `()`.

Deciding "the tail is `void`" needs the tail's type, so D1's normalization lives in `ori fix` (post-type-check), NOT in `ori_fmt`'s parse-only pass and NOT in the parser. `ori_fmt` stays type-free; it never adds or drops a tail `;`.

The class is the tail's **type**, not its syntactic shape — a void-typed bare identifier (`x`), field/index access (`a.b`, `arr[i]`), or any other void simple expression at the tail is covered, not only calls and assignments.

**The `void` is the tail's type against the block's expected/inferred type.** Rule 2 applies to *every* block, not only function bodies (void `if`/`match` arm blocks, `for...do` bodies, void-position `let = { }` blocks); for a sub-block the "type" is the block's expected/inferred type from context, not a function's declared return type.

**Carve-out — the literal unit `()`.** A no-`;` literal `()` tail is **NOT** normalized to `();` under Rule 2. `()` is the canonical empty-void-block idiom (`{ () }`; per Clause 14, bare `{ }` is an empty *map* literal, so `()` is how an empty void block is written), and a trailing `()` after `;`-statements is handled by the `redundant-trailing-unit-normalization` formatter pass (which *deletes* it). Forcing `()` to `();` would both break the `{ () }` idiom and fight that sibling pass; carving it out keeps the empty-void idiom intact and leaves the redundant-`()` case to the formatter (see Composition).

### Rule 2a — Diverging (`Never`) tails are value tails

A tail whose type is **`Never`** (`panic(...)`, `todo()`, `unreachable()`, or any `Never`-returning call) is a **value tail**: written **without** `;`, its `Never` coerces to the declared return type. It is not subject to Rule 2 (it carries no `void` value to normalize).

Honest caveat (the spelling is type-dependent): a `Never`-returning call (`abort()`) and a `void`-returning call (`cleanup()`) are syntactically identical (`name(args)`); which one omits `;` is decided by the **callee's return type**, not by surface form. So the no-`;` signal means "produces a non-`void` value **or** diverges" — reliable *in canonical source*, not derivable from the bare token stream. In a `-> void` body **both** spellings of a `Never` tail are well-formed: `panic("x")` (no `;`, `Never` value) and `panic("x");` (`Never` coerced to `void`, then a statement). The **canonical** form is the no-`;` value spelling, selected by **`ori fix`** (NOT `ori fmt`).

NOTE  The single canonical no-`;` spelling for `Never` tails is an **`ori fix` / formatter guarantee, NOT a type-checker guarantee** — the type checker accepts both spellings. `break` / `break value` are NOT `Never` tails (they produce values in labeled blocks and `for...yield`) and are out of scope.

### Rule 3 — Block-ending (`}`/`]`) tails

When the tail is block-ending (last token `}` or `]` — control-flow block-enders OR data literals per the classification above), the `;` treatment depends on the tail's type:

- **Non-`void` `}`/`]`-ending tail** → it is the block value (Rule 1): **no `;`** (a `;` would discard it and make the block `void`, caught by D2 against a non-`void` return type). To deliberately discard a non-`void` block-ending value, the `;` is **required** — exactly as for a simple discarded value.
- **Void / `Never` `}`-ending tail** → the `;` stays **optional** (spec §11.12.1): a no-`;` `}`-ending tail is the block value per the block-value rule, and a `;`-terminated one is a discarded statement — both yield `void`, so for a void `}`-tail the `;` is genuinely optional. Rule 2's normalization is NOT extended to `}`-ending tails (this preserves the approved fix that killed the `};`-after-`}` friction).

**The residual void-`}`-ending ambiguity — stated honestly (accepted).** The formatter's blank-line rule is *syntactic* — it places a blank line above **any** no-`;` last expression. So a no-`;` void `}`-ending tail gets a blank line **too**, identical to a no-`;` non-void value `}`-tail. The blank line therefore does **not** distinguish a void `}`-tail from a value `}`-tail. To avoid the mis-signal an author can write the `;` (making it a statement, no blank line), but §11.12.1 leaves that optional. This residual gap is the **accepted** price of preserving the approved optional-`;` rule; fully removing it would require Alternative 2 (mandate `;` after `}`), which reverts an approved fix and is NOT adopted. The enforced value-discard diagnostic (D2) DOES reach `}`/`]`-ending VALUE tails (see Error Handling); only the void-`}`-tail value/effect *display* signal carries the residual.

```ori
@step (g: Game) -> Game = {
    let $advanced = advance(g:);

    match advanced.event {        // non-void value tail: no `;`, blank line above
        Some(e) -> handle(e:)
        None    -> advanced
    }
}

@dispatch (e: Event) -> void = {
    log(e:);

    match e {                     // void }-tail, no `;`: the formatter blank-lines it (syntactic) —
        Click(p) -> on_click(p:)  // indistinguishable from a value }-tail (the accepted residual)
        Key(k)   -> on_key(k:)
    }
}
```

### Rule 4 — Scope: block bodies only

These rules govern the tail position **inside a block body `{ }`**. The braceless expression-bodied declaration is unaffected:

```ori
@double (x: int) -> int = x * 2;      // `;` is the DECLARATION terminator, always present
```

The `;` in `= expr;` terminates the *declaration* (the same role it plays in `let $x = 5;` and constant declarations). Consequence (a known limitation, not a bug): "no `;` ⟺ value" holds for simple tails *inside braces* but not for the one-liner form, where a value body carries `;`. A mechanical reader/emitter must branch on braced-vs-braceless to place the value's `;`. The alternative (drop `;` on braceless value bodies) is considered and rejected below.

### Signal summary

The enforced/normalized signal holds **in canonical / formatted source**, scoped to **concrete, non-`()`, simple** tails. The documented exceptions below are why the biconditional is NOT stated flatly.

| Tail (simple, non-`}`/`]`) | `;` (canonical) | Blank line above | Backed by |
|---|---|---|---|
| Non-`void` value (Rule 1) | none | yes | type checker (D2 on errant `;`) |
| `Never` / diverging (Rule 2a) | none (canonical) | yes | `ori fix` (typeck accepts both) |
| `void`, discarded (Rule 2) | **`;` (normalized)** | no | `ori fix` / formatter (D1) |
| Literal `()` (void idiom, carved out) | none — legal (formatter may delete a redundant one) | yes (syntactic) | `redundant-trailing-unit` formatter |
| Type-variable `T` tail (UQ4) | none | yes | exempt — see Unresolved Questions |

| Tail (block-ending, `}`/`]`) | `;` (canonical) | Blank line above | Backed by |
|---|---|---|---|
| Non-`void` value (Rule 1/3) | none | yes | type checker (D2 on errant `;`, extended) |
| Non-`void`, discarded (Rule 3) | **required** | no | type checker (D2) |
| `void` / `Never` (Rule 3) | optional (§11.12.1) | yes if no `;` (syntactic — does NOT distinguish void from value: accepted residual) | — |

Documented exceptions to "no-`;` simple ⟺ non-`void` value": the literal `()` (carved out), `Never` tails (two legal spellings; canonical chosen by `ori fix`), type-variable `T` tails (UQ4), and the entire `}`/`]`-ending class (Rule 3). The biconditional holds for **concrete, non-`()`, simple** tails in canonical source.

### Semantics

No runtime or type-system semantics change. The block value is still the last no-`;` expression; an all-`;` block is still `void`. Rule 2 only **selects a canonical spelling** (a no-`;` void simple tail normalizes to `;`); it does not change what any program evaluates to, and it does not reject any program.

### Error Handling

The type checker keeps ONE diagnostic flanking the value/effect boundary — D2, the real value-discard error. D1 is a normalization (above), not a diagnostic-error. **D2 gates its suggested fix on whether removing `;` makes the block value satisfy the block's expected/inferred type** — by **assignability**, including coercion (`Never` coerces to any type), NOT literal type equality; the "expected type" is the function's declared return type for a function body, or the context-expected/inferred type for a sub-block.

**D2 — `;` on the tail of a non-void body.** A tail terminated by `;` whose removal would make the (then non-`void`) expression match the declared return type. **D2 fires on BOTH simple AND `}`/`]`-ending value tails** (struct / map / list literals and `}`-ending control-flow values), so the motivating multi-line case is covered:

```
error: this `;` discards the block's value, but `apply_discount` must produce `Order`
  --> discount.ori:6:38
   |
 6 |     Order { ...order, total: capped };
   |                                      ^ remove this `;` to make this expression the return value
   |
   = note: a trailing `;` turns the tail into a discarded statement, so the block produces `void`
```

`Order { ...order, total: capped }` is a struct literal — last token `}`, a **block-ending VALUE tail**. D2 fires on it because removing the `;` makes the tail's own type (`Order`) **assignable** to the block's expected type. This is the extension that brings the motivating `}`-ending case under the targeted diagnostic; without it, such a discard fell to the generic "expected `T`, found `void`" mismatch.

D2 fires only when removing the `;` makes the tail's own type **assignable** to the block's expected type (so the removal actually fixes it). `{ compute(); panic("x"); }` in a `-> int` body **is** a D2 case (removing `;` leaves `Never`, which coerces to `int`). A `;`-terminated *void* tail in a non-`void` body (`{ compute(); log(); }`, `log()` → `void`, body `-> int`) is NOT a D2 case — removing `;` leaves `void`, not assignable to `int`; the general mismatch fires.

**Implementation pin (rust#124819).** The closest external analog — a diagnostic on the else-less `if` tail in a `-> ()` function — carried a span-label bug (the wrong span was highlighted). D2's span-pointing diagnostic MUST carry a regression pin for that exact shape so Ori does not reproduce it.

**The void direction (former D1) is a normalization, not an error.** A no-`;` void simple tail is normalized by `ori fix` (insert `;`); a no-`;` void `}`-tail is left as-is per §11.12.1. Neither produces a diagnostic.

### Symmetric case — value vs void return types

The semicolon selects the block's value type; the declared return type decides which spelling is canonical:

| Body | Canonical simple tail | Caught |
|---|---|---|
| `-> void` | `cleanup();` (void, Rule 2 — `ori fix` inserts `;`) | not an error — normalized |
| `-> int` | `capped` no `;` (int value) | `capped;` → block `void` → D2; `"hi"` no `;` → wrong-type mismatch |
| any (diverging) | `panic("x")` no `;` (`Never` coerces) | — |

The `-> int` direction (D2) is the type checker's teeth. The `-> void` direction (Rule 2) is the toolchain normalization.

---

## Drawbacks

- **The void-tail normalization is toolchain-grade, not type-checker-enforced.** A no-`;` void simple tail is legal; only the formatter / `ori fix` makes the `;`-terminated form canonical. So the "no-`;` simple ⟺ value" signal is reliable in **formatted / canonical source**, not in arbitrary hand-written text. This is the deliberate trade for not being the sole language to make a discarded-void-tail a hard error (see Prior Art).
- **It depends on `ori fix`, which does not exist yet.** Both the void-tail `;` normalization (D1) and the canonical `Never`-tail no-`;` spelling (Rule 2a) need the tail/callee type, so they live in `ori fix` (post-type-check), NOT `ori_fmt`. `ori fix` is a separate proposal (`Depends On:`); this proposal's normalizations are unreachable until it lands.
- **The no-`;` signal means "value OR diverges," and only in canonical source.** Because `Never`-calls and value-calls are syntactically identical, the no-`;` spelling is not derivable from the bare token stream.
- **The braceless `= expr;` form breaks "no `;` ⟺ value" globally.** Inside braces the value omits `;`; braceless it carries `;`. Scoped, but a real one-way-to-do-things wart (Rule 4 vs Alternative 3).
- **The literal `()` carve-out is a hole in the simple-tail biconditional, and it is syntactic inside a type-based rule.** Rule 2 keys on type, but the carve-out keys on the literal token `()`: `{ work(); () }` is left for the formatter to *delete* while `{ work(); u }` (`let $u = ()`, `u: void`) is normalized by `ori fix` to `{ work(); u; }` — the same void value, different canonical form by spelling. The carve-out is intentional (it preserves the empty-void `{ () }` idiom and defers the redundant `()` to the formatter) and author-directed, but it means a reader cannot treat the simple-tail no-`;` signal as exception-free. This coherence cost is owned, not hidden.
- **Braceless void control-flow flips the `;` requirement on brace presence.** A void `if c then foo()` (non-`}`-ending) is a simple tail → Rule 2 → `;` normalized in; the same logic as `if c then { foo() }` is `}`-ending → Rule 3 → `;` optional. Adding/removing branch braces silently changes the canonical `;`. The full affected set: braceless void `if`, `while...do e`, `for...do e`, and braced-vs-bare `match` arm bodies — not only `if`. A mechanical emitter must branch on (braced vs braceless) × (tail type) × (`}`-vs-simple) to place a single `;`, a machine-writability cost against §Ori principle 8.
- **The void `}`-ending tail keeps a residual ambiguity (accepted).** Preserving §11.12.1's optional-`;` means a void `}`-ending tail's value/effect *display* status rests on the formatter blank line alone, which does not distinguish it from a value `}`-tail. Fully removing it would require Alternative 2 (mandate `;` after `}`), which reverts an approved fix — NOT adopted. D2 still reaches `}`/`]`-ending VALUE tails; only the void-`}`-tail display signal carries the residual.

---

## Alternatives Considered

### Alternative 1: `return` keyword for methods

Rejected: `return` is a control-flow jump everywhere it exists; Ori has no early-exit-from-function mechanism (exits are `break`, `?`, `panic`), and a function body is structurally an expression whose value *is* its tail. A mandatory tail-only `return` that cannot jump miscommunicates control flow; an optional `return` adds a second way to do one thing and does not remove the bare-tail spelling. Roc, a pure expression-based language, added `return` (roc#7104 / roc#7173) and acquired a fallout tail (roc#9218 "Unhelpful err msg for return in expression position"); for Ori that class lands on dual-execution parity. See `block-expression-syntax.md §Not return`.

### Alternative 2: Mandate `;` on all void tails, including `}`-ending

Restores a fully bidirectional semicolon signal (and removes the void-`}`-tail residual ambiguity). Rejected: it re-introduces `};` after a closing brace, reverting `optional-semicolon-after-block-expressions-proposal.md` (approved 2026-04-13). The blank-line rule carries the display signal for `}`-ending tails; D2 carries the value-discard teeth. (If review decides the residual ambiguity is unacceptable, this is the lever to reconsider — at the cost of reverting an approved decision.)

### Alternative 3: Drop `;` on braceless value bodies (global signal)

Make `@double (x: int) -> int = x * 2` (no `;`) the value form. Rejected: it makes function declarations the only `= …` declarations without a terminator, breaking the "every declaration ends in `;`" consistency `let $x = 5;` and constants share — trading block-level for declaration-level inconsistency.

### Alternative 4: Make D1 a hard well-formedness error (the original framing)

The first draft of this proposal made a no-`;` void simple tail a hard type-checker error. Rejected after review: no surveyed expression-based language makes a discarded-void-tail a hard error (Rust leaves it to `clippy::needless_return`/`unused_unit`; Gleam *warns*, gleam#531; Swift *warns*, swift#85113, and even added swift#65445 to *suppress* the warning when the result is intentionally discarded — i.e. moved toward an opt-out, the opposite of a hard error). A hard error would also impose a breaking migration. The toolchain-normalization design (D1 via `ori fix` / formatter) achieves the canonical-source signal without the novelty risk, the breaking migration, or the no-escape-hatch friction, and matches the sibling `redundant-trailing-unit-normalization` model. The type checker keeps teeth only on D2 (a genuine value-discard type error).

---

## Composition with `redundant-trailing-unit-normalization`

The draft `redundant-trailing-unit-normalization-proposal.md` (a **formatter** pass that DELETES a trailing literal `()` result after ≥1 `;`-statement) and this proposal (a type-based **normalization** + the D2 diagnostic) operate on **different tail shapes** and never act on the same input — because Rule 2 **carves the literal `()` out**:

| Input | This proposal alone | + `redundant-trailing-unit` (formatter) | Canonical |
|---|---|---|---|
| `{ work(); () }` (literal unit) | no D1 normalization (`()` is carved out) → stays `{ work(); () }`, legal | formatter deletes the redundant `()` → `{ work(); }` | `{ work(); }` |
| `{ work(); cleanup() }` (void call) | D1 normalizes → `{ work(); cleanup(); }` | not a literal `()` → formatter does not fire; D1 → `{ work(); cleanup(); }` | `{ work(); cleanup(); }` |

- **Disjoint inputs (after the `()` carve-out).** Rule 2/D1 never normalizes the literal `()` (it is carved out), so D1 and the formatter's `()` deletion never touch the same tail: the formatter owns the redundant `()`; D1 owns the effectful void call (which the formatter cannot delete — `cleanup()` has observable behavior). No pipeline-ordering dependency.
- **Standalone behavior (honest).** Without `redundant-trailing-unit`, `{ work(); () }` stays legal (the `()` is the no-`;` block value, type `void`, carved out of D1) — not the noisy `{ work(); (); }`. The two are **recommended to land together** so the formatter erases the redundant `()`; standalone, the redundant `()` simply remains (not a correctness issue). Both are now the same model (toolchain normalization), so they compose cleanly.
- **Empty-void block preserved.** The sole-`()` block `{ () }` (the canonical empty-void idiom per Clause 14, since `{ }` is an empty map) is **unaffected** — `()` is carved out of D1, so `{ () }` stays well-formed.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** This is a type-checker diagnostic (D2) plus a syntactic formatter rule (the negative blank-line rule) plus an `ori fix` normalization (D1, `Never`-tail spelling). None is expressible as a library.
**Missing features that would enable purity:** None — diagnostics, canonical-format rules, and normalizations are inherently compiler/tooling concerns.
**Recommendation:** Proceed as a compiler (D2 only) + formatter + `ori fix` proposal. No new keywords. The type-checker change is narrow (extend D2 to `}`/`]`-ending value tails). The formatter change is one new *syntactic* blank-line rule (keyed on `;`-presence — no type information enters `ori_fmt`). The type-dependent normalizations live in `ori fix` (separate proposal, `Depends On:`), which runs after type-checking.

---

## Spec & Grammar Impact

- **Type-checking clause (the normative home):** The normative rule is type-conditional, so it lives in the **type-checking clause** — to be named concretely when the spec edit lands (the clauses cited below, 7/8/9/11/16, are lexical / type / structural, none is the checking clause). That clause hosts: D2's value-discard rule (extended to `}`/`]`-ending value tails) and the statement that a no-`;` void simple tail is *normalized* (not rejected). Clauses 7 §7.8.1 / 11 §11.12 / 16 §16.0.2 carry only a **forward pointer** to it.
- **Clause 11 (`11-blocks-and-scope.md` §11.12 / §11.12.1):** Reword the currently-unconditional "last no-`;` expression is the block value" so it does not read as contradicting the void-tail normalization; add a forward pointer to the type-checking clause. Preserve §11.12.1's optional-`;`-after-`}` for void/`Never` `}`-ending tails; state that a non-`void` `}`/`]`-ending discarded tail requires `;` (D2).
- **Clause 7 (`07-lexical-elements.md` §7.8.1 "Block semicolons"):** The block-semicolon rule stays syntactic + unconditional for parsing; add a forward pointer to the type-checking clause for the void-tail canonical-form normalization. No type information enters the lexical clause.
- **Clause 16 (`16-control-flow.md` §16.0.1 + §16.0.2 "Result expressions"):** Reword §16.0.2's unconditional last-no-`;`-expression-is-the-value statement to forward-reference the type-checking clause; the structural statement (a no-`;` void simple tail is *structurally* the result slot) and the canonical-form normalization (the formatter inserts `;`) must not read as contradictory.
- **Grammar (`grammar.ebnf`, `statement` production):** UNCHANGED. The production already admits both `expression ";"` (statement) and the trailing `[ expression ]` block-value slot.
- **Annex D (`annex-d-formatting.md`):** Add the *syntactic* negative blank-line rule — no blank line above a `;`-terminated tail statement — complementing the existing positive rule (blank line above the no-`;` last expression). No type information is consulted.
- **`.claude/rules/ori-syntax.md`:** Update the "Semicolon rule" and "Block expressions" entries (after approval + implementation).
- **Diagnostics (`ori_diagnostic`):** Allocate a type-checker code for D2 (`;` on the tail of a non-void body, simple OR `}`/`]`-ending), with `ori --explain` docs and a machine-applicable suggestion. D2's span-pointing carries the rust#124819 regression pin. No code is allocated for a D1 "error" — D1 is a normalization, surfaced as an `ori fix` machine-applicable suggestion, not a diagnostic-error.
- **Tooling (`ori fix` — separate proposal, `Depends On: ori-fix-proposal.md`):** the type-dependent normalizations live there, post-type-check: (a) inserting `;` on a void simple tail (D1), and (b) dropping `;` to the canonical no-`;` form on a `Never` tail (Rule 2a). Canonical pipeline: parse → type-check (D2) → `ori fix` (the two type-dependent normalizations) → `ori fmt` (syntactic blank-line placement + the sibling proposal's literal-`()` deletion). The `()` carve-out means D1 never touches `()`, so this order has no `()` ambiguity regardless of where `ori fmt` sits.

---

## Prior Art

- **Rust** — Ori's direct ancestor: tail without `;` is the block value; with `;` is a discarded statement. Rust does not *mandate* `;` on a discarded void tail (it leaves it to `clippy::needless_return` / `unused_unit`). This proposal matches Rust's grade: the void direction is toolchain-normalized, not a hard error; the value/effect spelling is type-dependent, not derivable from surface form alone.
- **Swift** — *warns* on an unused expression result (swift#85113), and even added swift#65445 to *suppress* that warning when the result is intentionally discarded — i.e. the most mature precedent moved toward an *opt-out*, not a hard error. This proposal's toolchain-normalization design aligns with that trajectory rather than running against it.
- **Gleam** — gleam#531 *warns* when a result is discarded (answered with a warning + `let _` / `use`, not a terminator). The discarded-result concern is cross-language real; every surveyed language answers it at lint/warning/formatter grade, never a hard error.
- **Ruby, Scala** — expression-oriented, last expression is the value, `return` optional and discouraged; newline-terminated, so no semicolon signal.
- **Elm, Roc, Koka** — functional, expression-oriented, no `return`, no discarded-tail terminator. Roc added `return` and acquired a fallout tail (roc#7104 / roc#7173 / roc#9218 open) — evidence against Alternative 1.
- **Go** — statement-oriented, mandatory `;` + explicit `return`; the opposite design point.
- **Zig** (sentiment) — `zig#1677`, `zig#629`, `zig#3188`, `zig#8856` repeatedly litigated the value-vs-statement-at-block-tail boundary; zig#9758 removed then *restored* block-as-expression entirely. Unresolved litigation, not adopted precedent. This proposal acknowledges the dependence (the rule is type-based; the void direction is toolchain-normalized) rather than claiming the signal is locally legible from syntax.

**Grade summary:** no surveyed expression-based language makes a no-`;` void tail a hard error. This proposal deliberately matches the prevailing grade — toolchain normalization (`ori fix` / formatter) for the void direction, a targeted type-checker diagnostic (D2) only for the genuine value-discard error.

---

## Unresolved Questions

- **Braceless scoping (Rule 4 vs Alternative 3):** scope to block bodies (recommended, Rule 4) and keep the braceless `= expr;` terminator, OR go global (Alternative 3). The recommendation is Rule 4; the review gate ratifies it.
- **Void `}`-ending residual ambiguity (DECIDED — accept):** accepted, preserving the approved optional-`;` fix (§11.12.1). For a void `}`-ending tail the syntactic blank line does not distinguish it from a value `}`-tail; D2 reaches `}`/`]`-ending VALUE tails, so only the void-`}`-tail *display* signal carries the residual. Alternative 2 (mandate `;` after `}`) is the lever to remove it, at the cost of reverting an approved decision — NOT adopted.
- **`()` standalone vs dependency:** `()` is carved out of Rule 2, so standalone `{ work(); () }` stays legal (no D1 normalization, no forced `();`). Land with `redundant-trailing-unit-normalization` (recommended) so the formatter erases the redundant `()` → `{ work(); }`; without it, the redundant `()` simply remains (not a correctness issue). No hard dependency.
- **Polymorphic / type-parameter tail (DECIDED — exempt; residual real and permanent):** a no-`;` simple tail whose type is an **unresolved type variable** (`@f<T> (...) -> T = { effect(); produce_t() }`, tail `: T`) is **exempt from Rule 2 / D1 normalization**. D1 normalizes only when the tail's type resolves to a **concrete `void`** at the definition's type-check; a tail of abstract type `T` is never `void` at that point, so the rule stays in the type checker (NOT monomorphization — no per-instantiation action, no action-at-a-distance). **`void` IS a permitted type argument** (verified against spec clauses 8 and 9 — there is no exclusion of `void` as a type argument; only `Never` carries an explicit type-argument callout; `void` is `Sendable` / `Value` with `Default` `()`). Therefore a `T = void` instantiation yields a no-`;` void simple tail that D1 does not normalize: the residual is **real and permanent**. The canonical-form invariant "no-`;` simple ⟺ non-`void` value" holds for **concretely-typed** tails, not type-variable tails. This is a signal-reliability / normalization-coverage gap, **NOT a soundness break** — the block still produces `void` correctly; nothing mis-evaluates. Making the invariant exception-free would require barring `void` as a type argument, a separate and larger language change this proposal does not assume.
