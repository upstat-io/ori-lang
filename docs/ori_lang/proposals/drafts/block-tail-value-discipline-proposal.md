# Proposal: Block-Tail Value Discipline

**Status:** Draft
**Author:** Eric
**Created:** 2026-06-26
**Affects:** Compiler (type checker, canonicalization), formatter (`ori_fmt`), tooling (`ori fix` — **NEW command, hard prerequisite**: no `fix` driver exists today, only the `MachineApplicable` suggestion infrastructure; the migration + the `Never`-tail canonicalization require it), spec (Clause 7, Clause 11, Clause 16), Annex D (formatting) — grammar UNCHANGED (the rule is a type-checker constraint over the existing `statement` production)
**Amends:** block-expression-syntax.md
**Related:** optional-semicolon-after-block-expressions-proposal.md, redundant-trailing-unit-normalization-proposal.md

---

## Summary

At a block tail, ground the value/effect distinction in the tail expression's **type** and make the trailing `;` enforce it. A no-`;` simple (non-block-ending) tail whose **type is `void`** is ill-formed — it must carry `;`; a no-`;` simple tail of **non-`void`** type is the block's produced value, and a **`Never`** tail is a value tail by coercion. The **type checker** enforces this (diagnostics D1/D2); the **canonical formatter** reinforces it with a *syntactic* blank-line rule (a blank line above the no-`;` value, none above `;`-statements). This tightens the one currently-ambiguous case — a no-`;` void simple tail, legal today — so that, for a simple tail **other than the literal `()`** (the empty-void idiom, carved out below), **no `;` ⟺ the block produces a non-`void` value**.

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

- Make the no-`;` simple-tail spelling reliable: for a simple (non-`}`-ending) tail **other than the literal `()`**, **no `;` ⟺ the block produces a non-`void` value** (or diverges). The literal `()` (the empty-void idiom) is the one carved-out exception (Rule 2).
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

A **simple** (non-`}`-ending) tail expression whose **type is `void`** shall be terminated by `;`, **except the literal unit expression `()`** (carved out below). A no-`;` void simple tail (other than `()`) is **ill-formed** (diagnostic D1). This is a **type-checker** rule — it fires after type resolution, because deciding "the tail is `void`" needs the tail's type, which the parser does not have. It is not a parse-time grammar change (the grammar already admits both spellings; see Spec & Grammar Impact).

This is purely a *spelling* tightening: `setup(); cleanup()` → `setup(); cleanup();`. The block still produces `void`; one currently-legal spelling (a no-`;` void simple tail) becomes ill-formed so that a no-`;` simple tail unambiguously signals a non-`void` value.

The class is the tail's **type**, not its syntactic shape — a void-typed bare identifier (`x`), field/index access (`a.b`, `arr[i]`), or any other void simple expression at the tail is covered, not only calls and assignments.

**The `void` is the tail's type against the block's expected/inferred type.** Rule 2 applies to *every* block, not only function bodies (void `if`/`match` arm blocks, `for...do` bodies, void-position `let = { }` blocks); for a sub-block the "type" is the block's expected/inferred type from context, not a function's declared return type.

**Carve-out — the literal unit `()`.** A no-`;` literal `()` tail is **NOT** ill-formed under Rule 2. `()` is the canonical empty-void-block idiom (`{ () }`; per Clause 14, bare `{ }` is an empty *map* literal, so `()` is how an empty void block is written), and a trailing `()` after `;`-statements is handled by the `redundant-trailing-unit-normalization` formatter pass (which deletes it). Forcing `()` to `();` would both break the `{ () }` idiom and fight that sibling pass; carving it out keeps the empty-void idiom intact and leaves the redundant-`()` case to the formatter (see Composition).

### Rule 2a — Diverging (`Never`) tails are value tails

A tail whose type is **`Never`** (`panic(...)`, `todo()`, `unreachable()`, or any `Never`-returning call) is a **value tail**: written **without** `;`, its `Never` coerces to the declared return type. It is not subject to Rule 2 (it carries no `void` value to require a `;`).

Honest caveat (the spelling is type-dependent): a `Never`-returning call (`abort()`) and a `void`-returning call (`cleanup()`) are syntactically identical (`name(args)`); which one omits `;` is decided by the **callee's return type**, not by surface form. So the no-`;` signal means "produces a non-`void` value **or** diverges" — reliable *in canonical source*, not derivable from the bare token stream. In a `-> void` body **both** spellings of a `Never` tail are well-formed: `panic("x")` (no `;`, `Never` value) and `panic("x");` (`Never` coerced to `void`, then a statement) — so for `Never` tails the *type checker* accepts two spellings; the **canonical** form is the no-`;` value spelling, selected by **`ori fix`** (NOT `ori fmt`). Choosing the canonical spelling requires the callee's return type (`Never` vs `void`), so it is a type-dependent normalization and lives in `ori fix` post-type-check, exactly like the D1 `;`-insertion migration — `ori_fmt` stays parse-only and never adds or drops a tail `;`. `break` / `break value` are NOT `Never` tails (they produce values in labeled blocks and `for...yield`) and are out of scope.

### Rule 3 — Block-ending (`}`) tails

When the tail's last token is `}` (`match { }`, `if...then { }`, `for...do { }`, `while...do { }`, `loop { }`, `unsafe { }`, `block:label { }`, bare `{ }`), the `;` rule depends on the tail's type:

- **Non-`void` `}`-ending tail** → it is the block value (Rule 1): **no `;`** (a `;` would discard it and make the block `void`, a type error against a non-`void` return type). To deliberately discard a non-`void` `}`-ending value, the `;` is **required** — exactly as for a simple discarded value.
- **Void / `Never` `}`-ending tail** → the `;` stays **optional**: a no-`;` `}`-ending tail is the block value per the block-value rule (§11.12 / §16.0.2), and a `;`-terminated one is a discarded statement (§11.12.1) — both yield `void`, so for a void `}`-tail the `;` is genuinely optional. Rule 2's mandatory `;` is NOT extended to `}`-ending tails (this preserves the approved fix that killed the `};`-after-`}` friction).

The residual ambiguity — stated honestly: the formatter's blank-line rule is *syntactic* — it places a blank line above **any** no-`;` last expression. So a no-`;` void `}`-ending tail gets a blank line **too**, identical to a no-`;` non-void value `}`-tail. The blank line therefore does **not** distinguish a void `}`-tail from a value `}`-tail — a void `}`-tail written without `;` is indistinguishable from a produced-value `}`-tail. To avoid the mis-signal an author can write the `;` (making it a statement, no blank line), but §11.12.1 leaves that optional. This residual gap is the price of preserving the approved optional-`;` rule; Rule 2's enforced signal covers only **simple** void tails, where the `;` is mandatory (see Drawbacks). The two example annotations below differ only in the author's `;` choice, not in any type-driven formatter behavior.

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
        Click(p) -> on_click(p:)  // indistinguishable from a value }-tail (the residual ambiguity)
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
| Literal `()` (void idiom, carved out) | none — legal (formatter may delete a redundant one) | yes (syntactic) |

| Tail (block-ending, `}`) | `;` | Blank line above |
|---|---|---|
| Non-`void` value (Rule 1/3) | none | yes |
| Non-`void`, discarded (Rule 3) | **required** | no |
| `void` / `Never` (Rule 3) | optional (§11.12.1) | yes if no `;` (syntactic — does NOT distinguish from a value tail: residual ambiguity) |

The blank line (syntactic, keyed on `;`-presence) is the formatter signal; the type-checker rules (Rule 2 / D1, D2) are the *enforced* guarantee for simple tails.

### Semantics

No runtime or type-system semantics change. The block value is still the last no-`;` expression; an all-`;` block is still `void`. Rule 2 only **removes one currently-legal spelling** (a no-`;` void simple tail); it does not change what any well-formed program evaluates to.

### Error Handling

Two type-checker diagnostics flank the value/effect boundary. **Both gate their suggested fix on whether toggling `;` makes the block value satisfy the block's expected/inferred type** — by **assignability**, including coercion (`Never` coerces to any type), NOT literal type equality; and the "expected type" is the function's declared return type for a function body, or the context-expected/inferred type for a sub-block. If toggling `;` does not reconcile, the standard type-mismatch diagnostic fires instead.

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
error: this `;` discards the block's value, but `apply_discount` must produce `Order`
  --> discount.ori:6:38
   |
 6 |     Order { ...order, total: capped };
   |                                      ^ remove this `;` to make this expression the return value
   |
   = note: a trailing `;` turns the tail into a discarded statement, so the block produces `void`
```

D2 fires only when removing the `;` makes the tail's own type **assignable** to the block's expected type (so the removal actually fixes it). `{ compute(); panic("x"); }` in a `-> int` body **is** a D2 case (removing `;` leaves `Never`, which coerces to `int`). A `;`-terminated *void* tail in a non-`void` body (`{ compute(); log(); }`, `log()` → `void`, body `-> int`) is NOT a D2 case — removing `;` leaves `void`, not assignable to `int`; the general mismatch fires. D2 is scoped to **simple** tails; an accidental `;` discarding a non-`void` `}`-ending value (`match e { … };` in a non-`void` body) falls to the general mismatch rather than D2 — a coverage asymmetry called out in Drawbacks.

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
- **The literal `()` carve-out is a hole in the simple-tail biconditional.** Because `()` (type `void`) is carved out of Rule 2, a no-`;` `()` simple tail is legal — so "no `;` simple ⟺ non-`void` value" has one exception. The exception is intentional (it preserves the empty-void `{ () }` idiom and defers the redundant `()` to the formatter), but it means a reader cannot treat the simple-tail no-`;` signal as exception-free.
- **The carve-out is syntactic inside a type-based rule.** Rule 2 keys on type, but the carve-out keys on the literal token `()`: `{ work(); () }` is legal while `{ work(); u }` (`let $u = ()`, `u: void`) is D1-ill-formed — the same void value, opposite treatment by spelling.
- **D2 coverage is simple-tail-only.** An accidental `;` discarding a non-`void` `}`-ending value (`match e { … };` in a non-`void` body) falls to the general "expected `T`, found `void`" mismatch, not the targeted D2 — even though `}`-ending value tails are the motivating multi-line case.
- **Braceless void control-flow flips the `;` requirement on brace presence.** A void `if c then foo()` (non-`}`-ending) is a simple tail → Rule 2 → `;` required; the same logic as `if c then { foo() }` is `}`-ending → Rule 3 → `;` optional. Adding/removing branch braces silently changes the `;` requirement.
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

The draft `redundant-trailing-unit-normalization-proposal.md` (a **formatter** pass that DELETES a trailing literal `()` result after ≥1 `;`-statement) and this proposal (a **type-checker** well-formedness rule) operate on **different tail shapes** and never act on the same input — because Rule 2 **carves the literal `()` out** (above):

| Input | This proposal alone | + `redundant-trailing-unit` (formatter) | Canonical |
|---|---|---|---|
| `{ work(); () }` (literal unit) | no D1 (`()` is carved out) → stays `{ work(); () }`, legal | formatter deletes the redundant `()` → `{ work(); }` | `{ work(); }` |
| `{ work(); cleanup() }` (void call) | D1: add `;` → `{ work(); cleanup(); }` | not a literal `()` → formatter does not fire; D1 → `{ work(); cleanup(); }` | `{ work(); cleanup(); }` |

- **Disjoint inputs (after the `()` carve-out).** Rule 2/D1 never fires on the literal `()` (it is carved out), so D1 and the formatter's `()` deletion never touch the same tail: the formatter owns the redundant `()`; D1 owns the effectful void call (which the formatter cannot delete — `cleanup()` has observable behavior). No pipeline-ordering dependency: D1 sees no `()` regardless of whether the formatter runs before or after type-check.
- **Standalone behavior (honest).** Without `redundant-trailing-unit`, `{ work(); () }` stays legal (the `()` is the no-`;` block value, type `void`, carved out of D1) — not the noisy `{ work(); (); }`. The two are **recommended to land together** so the formatter erases the redundant `()`, but this proposal does not force `()` to `();` on its own.
- **Empty-void block preserved.** The sole-`()` block `{ () }` (the canonical empty-void idiom per Clause 14, since `{ }` is an empty map) is **unaffected** — `()` is carved out of D1, so `{ () }` stays well-formed.

This supersedes the earlier "disjoint triggers / fully resolved" claim. An independent review showed the prior framing was incorrect for the `()` tail; the `()` carve-out (Rule 2) is the cure — D1 owns void *calls*, the formatter owns the redundant literal `()`.

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
- **Tooling (`ori fix` — NEW, a hard prerequisite):** there is no `fix` command today (only `MachineApplicable`/suggestion infrastructure in `ori_diagnostic` / `ori_types`, with no applying driver). This proposal must deliver `ori fix` before its migration or its canonical `Never` spelling is reachable. The type-dependent normalizations live here, post-type-check: (a) inserting `;` on a legacy void simple tail (D1's machine-applicable suggestion), and (b) dropping `;` to the canonical no-`;` form on a `Never` tail (Rule 2a). Both need the tail/callee type, so neither is an `ori_fmt` action. Canonical pipeline: parse → type-check (D1/D2) → `ori fix` (the two type-dependent normalizations) → `ori fmt` (syntactic blank-line placement + the sibling proposal's literal-`()` deletion). The `()` carve-out (Rule 2) means D1 never touches `()`, so this order has no `()` ambiguity regardless of where `ori fmt` sits.
- **Spec-clause placement:** locate the NORMATIVE void-simple-tail well-formedness rule in the **type-checking** clause (the rule is type-conditional), and leave Clause 7 §7.8.1 / Clause 11 §11.12 / Clause 16 §16.0.2 (currently unconditional, syntactic) carrying only a **forward pointer** to it — do NOT embed a type-dependent constraint in the lexical/structural clauses.

---

## Prior Art

- **Rust** — Ori's direct ancestor: tail without `;` is the block value; with `;` is a discarded statement. Rust does not *mandate* `;` on a discarded void tail (it leaves it to `clippy::needless_return` / `unused_must_use`). Ori tightens what Rust leaves to lint — but, like Rust, the value/effect spelling is type-dependent, not derivable from surface form alone. **Novelty (stated plainly):** no surveyed language makes a no-`;` void tail a hard *error* — Rust uses a lint, Gleam/Elm/Roc/Koka have no discarded-tail terminator, Go's mandatory `;` is a different model, and the Zig issues are unresolved litigation, not precedent. Ori would be the sole expression-based language to promote this from lint-grade style to a well-formedness error (plus a breaking migration). Defensible under one-way-to-do-things, but genuinely novel, not merely a "tightening."
- **Ruby, Scala** — expression-oriented, last expression is the value, `return` optional and discouraged; newline-terminated, so no semicolon signal.
- **Gleam, Elm, Roc, Koka** — functional, expression-oriented, no `return`, no discarded-tail terminator (few bare effect-statements).
- **Go** — statement-oriented, mandatory `;` + explicit `return`; the opposite design point.
- **Zig** (sentiment) — `zig#1677`, `zig#292`, `zig#629` repeatedly litigated the value-vs-statement-at-block-tail boundary precisely because the terminator depends on what the tail *is*. This proposal acknowledges that dependence (the rule is type-based and compiler-enforced) rather than claiming the signal is locally legible from syntax.

---

## Unresolved Questions

- **Braceless scoping (Rule 4 vs Alternative 3):** scope to block bodies (recommended, Rule 4) and keep the braceless `= expr;` terminator, OR go global (Alternative 3). The recommendation is Rule 4; the review gate ratifies it.
- **Void `}`-ending residual ambiguity:** accept it (recommended — preserves the approved optional-`;` fix) OR adopt Alternative 2 (mandate `;` after `}`). Note the honest limit: for a void `}`-ending tail the syntactic blank line does **not** distinguish it from a value `}`-tail (both no-`;` last expressions get a blank line); the enforced signal (Rule 2/D1) covers only **simple** void tails. The recommendation is to accept the residual.
- **`()` standalone vs dependency:** `()` is carved out of Rule 2, so standalone `{ work(); () }` stays legal (no D1, no forced `();`). Land with `redundant-trailing-unit-normalization` (recommended) so the formatter erases the redundant `()` → `{ work(); }`; without it, the redundant `()` simply remains (not a correctness issue). No dependency.
- **Polymorphic / type-parameter tail (DECIDED — exempt):** a no-`;` simple tail whose type is an **unresolved type variable** (`@f<T> (...) -> T = { effect(); produce_t() }`, tail `: T`) is **exempt from Rule 2 / D1**. D1 fires only when the tail's type resolves to a **concrete `void`** at the definition's type-check; a tail of abstract type `T` is never `void` at that point, so it is well-formed and the rule stays in the type checker (NOT monomorphization — no per-instantiation error, no action-at-a-distance). Residual (documented, not enforced): a `T = void` instantiation yields a no-`;` void simple tail that D1 does not catch; the invariant "no-`;` simple ⟺ non-`void`" therefore holds for **concretely-typed** tails, not type-variable tails. (Open during review only: confirm whether Clause 9 even permits `void` as a type argument — if not, the residual cannot arise at all and the exemption is vacuous; if so, the exemption above governs.)
