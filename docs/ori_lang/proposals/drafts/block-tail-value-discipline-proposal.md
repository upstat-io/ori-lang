# Proposal: Block-Tail Value Discipline

**Status:** Draft
**Author:** Eric
**Created:** 2026-06-26
**Affects:** Compiler (type checker, canonicalization), formatter (`ori_fmt`), tooling (`ori fix`), spec (Clause 7, Clause 11, Clause 16), grammar (Annex A), Annex D (formatting)
**Amends:** block-expression-syntax.md
**Related:** optional-semicolon-after-block-expressions-proposal.md, redundant-trailing-unit-normalization-proposal.md

---

## Summary

At a block tail, ground the value/effect distinction in the tail expression's **type** and make the trailing `;` enforce it. A no-`;` simple (non-block-ending) tail whose **type is `void`** is ill-formed — it must carry `;`; a no-`;` simple tail of **non-`void`** type is the block's produced value, and a **`Never`** tail is a value tail by coercion. The **type checker** enforces this (diagnostics D1/D2); the **canonical formatter** reinforces it with a *syntactic* blank-line rule (a blank line above the no-`;` value, none above `;`-statements). This tightens the one currently-ambiguous case — a no-`;` void simple tail, legal today — so that, for a simple tail, **no `;` ⟺ the block produces a non-`void` value**.

---

## Motivation

Ori is expression-based: a block's value is its last expression, identified by the absence of a trailing `;` (`block-expression-syntax.md §Last Expression Is the Value`, spec §11.12, §7.8.1, §16.0.2). The design already wanted **two visual signals** to make the result expression unmistakable: no semicolon, and a formatter-enforced blank line above it.

Today the semicolon signal is only *one-directional*:

- A produced **value** tail reliably has **no** `;` — a stray `;` makes the block `void`, which the type checker rejects against a non-`void` return type.
- But a **void** tail does **not** reliably have a `;`. Both forms below are currently legal and both yield a `void` block:

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

When the effect line and the value line are typographically indistinguishable, the value "hangs off" the body with no marker. The fix is not a `return` keyword (that imports a control-flow jump Ori does not have — see Alternatives), but making the tail's **type** decide the spelling and having the compiler enforce it.

### When This Matters

- **Every multi-statement function or method** that mixes effectful calls with a produced result.
- **Void-returning bodies** that end in a side-effecting call — the currently-ambiguous case.
- **Tooling and readers**: a type-checked rule plus a canonical blank line gives a reliable, machine-checkable signal in formatted source, rather than a convention the compiler does not back.

---

## Goals and Non-Goals

**Goals:**

- Make the no-`;` simple-tail spelling reliable: for a simple (non-`}`-ending) tail, **no `;` ⟺ the block produces a non-`void` value** (or diverges).
- Tighten the one ambiguous case (a no-`;` void simple tail) so it is ill-formed and must carry `;`, enforced by the **type checker** (D1).
- Add the formatter's missing **syntactic** negative rule: no blank line above a `;`-terminated tail statement (complementing the existing blank-line-above-the-no-`;`-value rule).
- Ship two targeted diagnostics (D1, D2) that name the value/effect cause and the exact fix, replacing the oblique `found void` mismatch on the void side.

**Non-Goals:**

- No `return` keyword and no early-exit-from-function mechanism (rejected in Alternatives).
- No change to the block-value *semantics* — the last no-`;` expression is still the block value, exactly as today.
- No change to the optional-`;`-after-`}` rule for void/`Never` block-ending tails (spec §11.12.1) — this proposal scopes around it.
- No change to the braceless expression-bodied declaration terminator (`@f (...) -> R = expr;`).
- No new type-checker dependency inside `ori_fmt` — the formatter stays parse-only (see Spec & Grammar Impact).

---

## Design

The governing distinction is the tail expression's **type**. The earlier "role, not type" framing was inaccurate: a no-`;` non-`void` tail *is* the block's value (you cannot drop its result at the tail — dropping requires a `;`, which makes it a statement), and a no-`;` `void` tail is the ambiguous case this proposal removes. The decision therefore reduces to a type test, which is why it lives in the type checker (after type resolution), not the parser.

### Rule 1 — Produced-value tail (no-`;`, non-`void` type)

A simple or block-ending tail expression with **no** trailing `;` whose type is **non-`void`** is the block's produced value:

- it is the block value (unchanged semantics), and
- the formatter places a **blank line above it** (the existing positive rule; keyed *syntactically* on "last expression, no `;`").

### Rule 2 — Void simple tail must carry `;` (the new well-formedness rule)

A **simple** (non-`}`-ending) tail expression whose **type is `void`** shall be terminated by `;`. A no-`;` void simple tail is **ill-formed** (diagnostic D1). This is a **type-checker** rule — it fires after type resolution, because deciding "the tail is `void`" needs the tail's type, which the parser does not have. It is not a parse-time grammar change (the grammar already admits both spellings; see Spec & Grammar Impact).

This is purely a *spelling* tightening: `setup(); cleanup()` → `setup(); cleanup();`. The block still produces `void`; one currently-legal spelling (a no-`;` void simple tail) becomes ill-formed so that a no-`;` simple tail unambiguously signals a non-`void` value.

The class is the tail's **type**, not its syntactic shape — a void-typed bare identifier (`x`), field/index access (`a.b`, `arr[i]`), or any other void simple expression at the tail is covered, not only calls and assignments.

### Rule 2a — Diverging (`Never`) tails are value tails

A tail whose type is **`Never`** (`panic(...)`, `todo()`, `unreachable()`, or any `Never`-returning call) is a **value tail**: written **without** `;`, its `Never` coerces to the declared return type. It is not subject to Rule 2 (it carries no `void` value to require a `;`).

Honest caveat (the spelling is type-dependent): a `Never`-returning call (`abort()`) and a `void`-returning call (`cleanup()`) are syntactically identical (`name(args)`); which one omits `;` is decided by the **callee's return type**, not by surface form. So the no-`;` signal means "produces a non-`void` value **or** diverges" — it is reliable *in type-checked source* (the type checker accepts exactly one spelling per tail), not derivable from the bare token stream. In a `-> void` body a `Never` tail may also be written `panic("x");` (a `Never` value coerced to `void`, then a statement); both are well-formed, and the **canonical** form is the no-`;` value spelling (`ori fmt` normalizes to it). `break` / `break value` are NOT `Never` tails (they produce values in labeled blocks and `for...yield`) and are out of scope.

### Rule 3 — Block-ending (`}`) tails

When the tail's last token is `}` (`match { }`, `if...then { }`, `for...do { }`, `while...do { }`, `loop { }`, `unsafe { }`, `block:label { }`, bare `{ }`), the `;` rule depends on the tail's type:

- **Non-`void` `}`-ending tail** → it is the block value (Rule 1): **no `;`** (a `;` would discard it and make the block `void`, a type error against a non-`void` return type). To deliberately discard a non-`void` `}`-ending value, the `;` is **required** — exactly as for a simple discarded value.
- **Void / `Never` `}`-ending tail** → the `;` stays **optional** per spec §11.12.1 (preserving the approved fix that killed the `};`-after-`}` friction). Rule 2's mandatory `;` is NOT extended to `}`-ending tails.

The formatter's blank-line rule (Rule 1, syntactic) carries the value/effect signal across `}`-ending shapes. The residual gap: a **void `}`-ending tail** has only the no-`;` spelling under §11.12.1's latitude *or* a `;`; the blank line is the sole differentiator, and a void `}`-tail that an author conceptually intends as "the answer" has no distinct signal (it is always void). This is the price of preserving the approved optional-`;` rule (see Drawbacks).

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
    match e {                     // void tail: `;` optional (§11.12.1); no blank line above
        Click(p) -> on_click(p:)
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

| Tail (simple, non-`}`) | `;` | Blank line above |
|---|---|---|
| Non-`void` value (Rule 1) | none | yes |
| `Never` / diverging (Rule 2a) | none (canonical) | yes |
| `void`, discarded (Rule 2) | **required** (D1) | no |

| Tail (block-ending, `}`) | `;` | Blank line above |
|---|---|---|
| Non-`void` value (Rule 1/3) | none | yes |
| Non-`void`, discarded (Rule 3) | **required** | no |
| `void` / `Never` (Rule 3) | optional (§11.12.1) | per blank-line rule (residual ambiguity) |

The blank line (syntactic, keyed on `;`-presence) is the formatter signal; the type-checker rules (Rule 2 / D1, D2) are the *enforced* guarantee for simple tails.

### Semantics

No runtime or type-system semantics change. The block value is still the last no-`;` expression; an all-`;` block is still `void`. Rule 2 only **removes one currently-legal spelling** (a no-`;` void simple tail); it does not change what any well-formed program evaluates to.

### Error Handling

Two type-checker diagnostics flank the value/effect boundary. **Both gate their suggested fix on whether toggling `;` actually reconciles the block-value type with the declared return type** — if it does not, the standard type-mismatch diagnostic fires instead.

**D1 — void simple tail missing `;`.** A simple (non-`}`-ending) tail of type `void` with no `;`:

```
error: this void expression must end with `;`
  --> snake.ori:7:5
   |
 7 |     cleanup()
   |     ^^^^^^^^^ add `;` — `cleanup()` returns `void`, so it is a statement, not the block value
   |
   = note: omit `;` only on the expression that IS the block's (non-void) value
```

D1 fires only when the tail's type is `void` (so adding `;` yields a well-formed void block). If the body's declared return type is non-`void` and the void tail is the *only* tail, adding `;` will not satisfy the return type — the general "expected `T`, found `void`" mismatch fires instead, not D1.

**D2 — `;` on the tail of a non-void body.** A simple tail terminated by `;` whose removal would make the (then non-`void`) expression match the declared return type:

```
error: this `;` discards the block's value, but `apply_discount` must produce `int`
  --> discount.ori:6:38
   |
 6 |     Order { ...order, total: capped };
   |                                      ^ remove this `;` to make this expression the return value
   |
   = note: a trailing `;` turns the tail into a discarded statement, so the block produces `void`
```

D2 fires only when the `;`-terminated tail's *own* type equals the declared return type (so removing `;` fixes it). A `;`-terminated void tail in a non-`void` body (`{ compute(); log(); }`, `log()` → `void`, body `-> int`) is NOT a D2 case — removing `;` would still leave `void` ≠ `int`; the general mismatch fires.

### Symmetric case — non-void return types

The semicolon selects the block's value type; the declared return type decides which spelling is legal:

| Body | Valid simple tail | Invalid |
|---|---|---|
| `-> void` | `cleanup();` (void, Rule 2) | `cleanup()` no `;` → D1 |
| `-> int` | `capped` no `;` (int value) | `capped;` → block `void` → D2; `"hi"` no `;` → wrong-type mismatch |
| any (diverging) | `panic("x")` no `;` (`Never` coerces) | — |

The `-> int` direction is already enforced today; D2 sharpens its message. The `-> void` direction is where Rule 2 / D1 adds the new teeth.

---

## Drawbacks

- **It is a (small) breaking change — broader than just function bodies.** Rule 2 fires on the simple void tail of **every** block (function bodies, void `if`/`match` arm blocks, `for...do` bodies, void-position `let = { }` blocks), not only `@main`. Each `void` block ending in a no-`;` simple expression must add `;`. The migration is mechanical but type-dependent — see the next point.
- **Migration is `ori fix`, not `ori fmt`.** Deciding where to insert `;` needs the tail's type (void-call → insert; non-void value → leave; `Never`-call → leave); the three look identical syntactically. So the one-shot migration runs as `ori fix` applying D1's machine-applicable suggestion **after type-checking**, NOT as a parse-only `ori fmt` pass. `ori_fmt` stays type-free (it only places the syntactic blank line and, with the related proposal, deletes a redundant `()`).
- **The no-`;` signal means "value OR diverges," and only in type-checked source.** Because `Never`-calls and value-calls are syntactically identical, the no-`;` spelling is not derivable from the bare token stream — the type checker enforces exactly one spelling per tail. The signal is reliable in checked/formatted source, not in arbitrary hand-written text.
- **The braceless `= expr;` form breaks "no `;` ⟺ value" globally.** Inside braces the value omits `;`; braceless it carries `;`. Scoped, but a real one-way-to-do-things wart (Rule 4 vs Alternative 3).
- **The void `}`-ending tail keeps a residual ambiguity.** Preserving §11.12.1's optional-`;` means a void `}`-ending tail's value/effect status rests on the formatter blank line alone. Fully removing it would require Alternative 2 (mandate `;` after `}`), which reverts an approved fix.

---

## Alternatives Considered

### Alternative 1: `return` keyword for methods

Rejected: `return` is a control-flow jump everywhere it exists; Ori has no early-exit-from-function mechanism (exits are `break`, `?`, `panic`), and a function body is structurally an expression whose value *is* its tail. A mandatory tail-only `return` that cannot jump miscommunicates control flow; an optional `return` adds a second way to do one thing and does not remove the bare-tail spelling. See `block-expression-syntax.md §Not return`.

### Alternative 2: Mandate `;` on all void tails, including `}`-ending

Restores a fully bidirectional semicolon signal (and removes the void-`}`-tail residual ambiguity). Rejected: it re-introduces `};` after a closing brace, reverting `optional-semicolon-after-block-expressions-proposal.md` (approved 2026-04-13). The blank-line rule carries the signal for `}`-ending tails. (If review decides the residual ambiguity is unacceptable, this is the lever to reconsider.)

### Alternative 3: Drop `;` on braceless value bodies (global signal)

Make `@double (x: int) -> int = x * 2` (no `;`) the value form. Rejected: it makes function declarations the only `= …` declarations without a terminator, breaking the "every declaration ends in `;`" consistency `let $x = 5;` and constants share — trading block-level for declaration-level inconsistency.

---

## Composition with `redundant-trailing-unit-normalization`

The draft `redundant-trailing-unit-normalization-proposal.md` (a **formatter** pass that DELETES a trailing literal `()` result after ≥1 `;`-statement) and this proposal (a **type-checker** well-formedness rule) both touch the **void `()` tail** — they are NOT disjoint, but they operate at different layers and **converge on the canonical all-`;` void block**.

The literal `()` has type `void`, so by Rule 2 a no-`;` `()` simple tail is ill-formed (D1). The two proposals interact as follows:

| Input | This proposal alone | + `redundant-trailing-unit` (formatter) | Canonical |
|---|---|---|---|
| `{ work(); () }` | D1: add `;` → `{ work(); (); }` (well-formed, but a noisy `();`) | formatter deletes the no-`;` `()` → `{ work(); }` (no D1 ever fires) | `{ work(); }` |
| `{ work(); cleanup() }` (void call) | D1: add `;` → `{ work(); cleanup(); }` | not a literal `()` → formatter does not fire; D1 → `{ work(); cleanup(); }` | `{ work(); cleanup(); }` |

- **Layering, not disjointness.** `redundant-trailing-unit` runs in `ori fmt` (deletes the no-`;` `()` *before* it reaches the type checker, so D1 never sees it); D1 is a type-checker error on a void tail the formatter cannot delete (an effectful `cleanup()` has observable behavior and must be kept + `;`-terminated). For the `()` input the formatter acts first; for an effectful void call only D1 acts. They never both transform the same input.
- **Standalone behavior (honest).** Without `redundant-trailing-unit`, this proposal alone canonicalizes `{ work(); () }` to `{ work(); (); }` — well-formed, but it leaves the noisy `();` the sibling proposal exists to remove. So the two are **recommended to land together** for the cleanest void-block form, and this proposal does NOT claim the `()` case is fully resolved on its own.
- **Not commutative as two formatter passes** — which is why `redundant-trailing-unit` stays a formatter deletion and Rule 2 stays a type-checker rule, on opposite sides of the parse/type boundary, rather than two competing `ori fmt` transforms.

This supersedes the earlier "disjoint triggers / fully resolved" claim, which an independent review verified as incorrect for the `()` tail.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** This is a type-checker well-formedness rule (Rule 2 / D1, D2) plus a syntactic formatter rule (the negative blank-line rule) plus an `ori fix` migration. None is expressible as a library.
**Missing features that would enable purity:** None — well-formedness checks, canonical-format rules, and migrations are inherently compiler/tooling concerns.
**Recommendation:** Proceed as a compiler + formatter + `ori fix` proposal. No new keywords. The type-checker change is narrow (a void simple tail must carry `;`). The formatter change is one new *syntactic* blank-line rule (keyed on `;`-presence — no type information enters `ori_fmt`). The type-dependent migration lives in `ori fix`, which runs after type-checking.

---

## Spec & Grammar Impact

- **Clause 11 (`11-blocks-and-scope.md` §11.12 / §11.12.1):** Add that a simple (non-`}`-ending) tail expression of type `void` shall be terminated by `;`. Preserve §11.12.1's optional-`;`-after-`}` for void/`Never` `}`-ending tails; state that a non-`void` `}`-ending discarded tail requires `;`.
- **Clause 7 (`07-lexical-elements.md` §7.8.1 "Block semicolons"):** Update the block-semicolon rule to record the new void-simple-tail constraint (currently unconditional).
- **Clause 16 (`16-control-flow.md` §16.0.1 + §16.0.2 "Result expressions"):** §16.0.2 states the unconditional last-no-`;`-expression-is-the-value rule; update it (and the §16.0.1 expression-statement framing) so it does not contradict the new well-formedness constraint.
- **Grammar (`grammar.ebnf`, `statement` production):** UNCHANGED. The production already admits both `expression ";"` (statement) and the trailing `[ expression ]` block-value slot; this proposal adds a type-checker well-formedness constraint over the existing grammar, not a new production.
- **Annex D (`annex-d-formatting.md`):** Add the *syntactic* negative blank-line rule — no blank line above a `;`-terminated tail statement — complementing the existing positive rule (blank line above the no-`;` last expression). No type information is consulted.
- **`.claude/rules/ori-syntax.md`:** Update the "Semicolon rule" and "Block expressions" entries.
- **Diagnostics (`ori_diagnostic`):** Allocate a type-checker code for D1 (void simple tail missing `;`) and a type-checker code for D2 (`;` on the tail of a non-void body), each with `ori --explain` docs and a machine-applicable suggestion. Both gate on type-reconciliation (above). D1's suggestion is what `ori fix` applies during migration.
- **Tooling (`ori fix`):** The migration applier consumes D1's machine-applicable suggestion to insert `;` on legacy void simple tails, post-type-check. (`built-in-lint-format-on-compile` interaction: type-check + D1 run before any format-on-compile `;`-insertion; the canonical pipeline is parse → type-check (D1/D2) → `ori fix` suggestion application → `ori fmt` blank-line/`()` normalization.)

---

## Prior Art

- **Rust** — Ori's direct ancestor: tail without `;` is the block value; with `;` is a discarded statement. Rust does not *mandate* `;` on a discarded void tail (it leaves it to `clippy::needless_return` / `unused_must_use`). Ori tightens what Rust leaves to lint — but, like Rust, the value/effect spelling is type-dependent, not derivable from surface form alone.
- **Ruby, Scala** — expression-oriented, last expression is the value, `return` optional and discouraged; newline-terminated, so no semicolon signal.
- **Gleam, Elm, Roc, Koka** — functional, expression-oriented, no `return`, no discarded-tail terminator (few bare effect-statements).
- **Go** — statement-oriented, mandatory `;` + explicit `return`; the opposite design point.
- **Zig** (sentiment) — `zig#1677`, `zig#292`, `zig#629` repeatedly litigated the value-vs-statement-at-block-tail boundary precisely because the terminator depends on what the tail *is*. This proposal acknowledges that dependence (the rule is type-based and compiler-enforced) rather than claiming the signal is locally legible from syntax.

---

## Unresolved Questions

- **Braceless scoping (Rule 4 vs Alternative 3):** scope to block bodies (recommended, Rule 4) and keep the braceless `= expr;` terminator, OR go global (Alternative 3). The recommendation is Rule 4; the review gate ratifies it.
- **Void `}`-ending residual ambiguity:** accept it (recommended — preserves the approved optional-`;` fix; the blank line carries the signal) OR adopt Alternative 2 (mandate `;` after `}`). The recommendation is to accept it.
- **`()` standalone vs dependency:** land with `redundant-trailing-unit-normalization` for the cleanest `{ work(); }` form (recommended), OR ship alone and accept `{ work(); (); }` as the canonical `()`-tail form. Recommendation: land together; this proposal does not depend on it for correctness, only for cleanliness.
