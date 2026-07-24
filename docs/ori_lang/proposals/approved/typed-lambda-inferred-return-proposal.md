# Proposal: Typed-Parameter Lambdas with Inferred Return Type

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-05-31
**Approved:** 2026-05-31
**Affects:** Grammar, parser (`ori_parse`), spec (Clause 14), type system (inference only — no new rules)

---

## Summary

Add a third lambda form, `(x: T) -> body`, where a lambda may declare typed
parameters while leaving its return type to be inferred. Today the grammar
forces a return-type annotation (and an `=`) the moment any parameter is typed.
This proposal removes that asymmetry: typed-parameter lambdas infer their return
type exactly as untyped lambdas already do. The explicit-return form
`(x: T) -> RetT = body` is retained for the cases where the author wants to state
the return type.

---

## Motivation

Ori has two lambda forms today (`grammar.ebnf:550-553`):

```ebnf
simple_lambda = lambda_params "->" expression .
typed_lambda  = "(" [ typed_param { "," typed_param } ] ")" "->" type "=" expression .
lambda_params = identifier | "(" [ identifier { "," identifier } ] ")" .
```

An untyped lambda infers its return type:

```ori
let inc = x -> x + 1;            // return type inferred as int
let add = (a, b) -> a + b;       // return type inferred
```

But the moment a single parameter carries a type annotation, the author is
forced to *also* annotate the return type and add `=`:

```ori
let inc = (x: int) -> int = x + 1;   // required today
```

### The Problem in Practice

There is no `(x: int) -> x + 1` form. Annotating a parameter — often done purely
to document or constrain that one parameter — drags in two unrelated obligations
(return-type annotation + `=`). This is an ergonomic cliff with no semantic
justification: the compiler can infer the return type of a typed-parameter lambda
just as easily as it infers the return type of an untyped one.

The asymmetry is sharp enough that it is the form authors *expect* to work. The
conformance corpus is written against the inferred-return typed form throughout:

```ori
// tests/spec/expressions/lambdas.ori
let f = (x: int) -> x * 2;

// tests/spec/inference/generics.ori
let f = (x: int) -> x + 1;
```

These files currently fail to parse (`ori test tests/`) — the parser enters the
`typed_lambda` branch on seeing a typed parameter and then reports `E1005`
("expected return type after `->`") or `E1017` ("expected `=` after typed lambda
return type") because no return type / `=` follows. The corpus is not wrong about
what *should* work; the grammar simply lacks the form.

### When This Matters

Any time a lambda needs a parameter type but the return type is obvious from the
body — which is the common case. Examples: constraining a numeric parameter
(`(n: int) -> n * factor`), pinning a parameter to a trait object, or
disambiguating an overloaded callback. Forcing a redundant return annotation in
all of these cases is friction the language can remove.

---

## Design

### Syntax

Unify the two lambda productions into one with an optional, explicit return type:

```ebnf
lambda        = lambda_params "->" lambda_tail .
lambda_params = bare_params | typed_params .
bare_params   = identifier | "(" [ identifier { "," identifier } ] ")" .
typed_params  = "(" [ typed_param { "," typed_param } ] ")" .
lambda_tail   = type "=" expression        (* explicit return type *)
              | expression .               (* inferred return type *)
```

This yields four shapes, three of which exist today and one of which is new:

```ori
x -> x + 1                       // untyped params, inferred return    (existing)
(a, b) -> a + b                  // untyped params, inferred return    (existing)
(x: int) -> int = x * 2          // typed params, explicit return      (existing)
(x: int) -> x * 2                // typed params, inferred return      (NEW)
```

Typed and untyped parameters do not mix within a single lambda (unchanged from
today — `typed_param` and bare `identifier` parameter lists remain distinct).

### Semantics

The new form is purely a surface-syntax addition. At the type level a
typed-parameter inferred-return lambda is identical to the existing untyped
inferred-return lambda except that each parameter's type is fixed by its
annotation rather than inferred. The return type is unified from the body
expression by ordinary Hindley-Milner inference — the same machinery that already
types `x -> x + 1`. No new type rule, no new error class, no runtime change.

The explicit-return form `(x: T) -> RetT = body` is unchanged: `RetT` is the
declared return type and the body is checked against it.

### Disambiguation — the design crux

After `->`, the parser must decide whether the following tokens are a return-type
annotation (`type "=" expression`) or a body expression (`expression`). The
disambiguator is the `=`:

1. Take a parser snapshot.
2. Speculatively parse a `type` (the parser already has speculative-parse /
   snapshot machinery — `ori_parse/src/snapshot/`).
3. If a type parses **and** the very next token is `=`, commit to the
   explicit-return form: consume `=`, then parse the body expression.
4. Otherwise restore the snapshot and parse the tail as a body **expression**
   (inferred return).

The `=` is the unambiguous delimiter. This mirrors the existing grammar, which
already reserves `=` in the `typed_lambda` production for exactly this role, and
it mirrors function-declaration syntax (`@f (x: int) -> int = body`), so
explicit-return lambdas read identically to function declarations.

Worked cases:

| Lambda | Speculative type parse | `=` follows? | Outcome |
|---|---|---|---|
| `(x: int) -> x * 2` | fails (`x * 2` is not a type) | — | body `x * 2`, return inferred |
| `(x: int) -> int = x` | succeeds (`int`) | yes | return `int`, body `x` |
| `(x: int) -> Foo == y` | succeeds (`Foo`) | no (`==`) | body `Foo == y`, return inferred |
| `(x: int) -> make()` | fails (`make()` is a call, not a type) | — | body `make()`, return inferred |

### The bare `-> Type` edge case

`(x: int) -> int` (a parseable type, no `=`, nothing after) is the one shape the
rule must pin down. Under the disambiguator the speculative type parse succeeds
but no `=` follows, so the parser restores and tries to parse `int` as a body
expression. A bare type-name keyword (`int`, `bool`, …) is not a value
expression, so this fails — and the lambda is genuinely incomplete (the author
meant `(x: int) -> int = <body>`). The parser shall emit a targeted diagnostic:

```
error: lambda has a return type but no body
  (x: int) -> int
              ^^^ add `= <body>`, or drop the return type for an inferred-return lambda
```

This preserves a precise message for the most likely typo while keeping the
common inferred-return path unambiguous. `=` after a parseable type is **always**
the return-type delimiter — never an assignment body — consistent with today's
`typed_lambda`.

### Error Handling

- The new form introduces no new error codes.
- `E1005` / `E1017` (the current "missing return type / `=`" errors) are no longer
  emitted for `(x: T) -> body`; they remain reachable only for the bare
  `-> Type`-with-no-body edge case, re-pointed at the targeted diagnostic above.
- Body type errors surface through ordinary inference against the parameter types,
  identical to untyped lambdas.

---

## Alternatives Considered

### Alternative 1: Migrate the corpus to the explicit-return form

Rewrite every `(x: T) -> body` in `tests/` and `library/` to
`(x: T) -> RetT = body`. This keeps the grammar unchanged and is purely
mechanical. Rejected as the primary path because it entrenches the asymmetry the
corpus authors instinctively wrote against, leaves the ergonomic cliff in the
language, and treats a symptom (corpus lagging the grammar) rather than the
design gap (the grammar lagging the obvious surface). It is the fallback if this
proposal is rejected.

### Alternative 2: Status quo — keep requiring explicit return on typed lambdas

Do nothing; typed-parameter lambdas always require `-> RetT = body`. Rejected: it
is an inconsistency with no semantic basis (the compiler infers return types
already) and diverges from every comparison language (see Prior Art).

### Alternative 3: Require an explicit return type on *all* lambdas

Remove inferred return entirely, forcing `x -> int = x + 1`. Rejected outright —
maximally verbose, breaks the existing untyped form, and contradicts Ori's
inference-first design.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** This is new surface syntax plus a parser disambiguation rule —
the canonical "requires compiler" category under the lean-core principle (new
syntax ⇒ YES; only constructs needing special syntax or static analysis live in
the compiler).
**Missing features that would enable purity:** none applicable; syntax cannot live
in a library.
**Recommendation:** Proceed as a compiler feature. The change is confined to the
grammar and `ori_parse`; the type checker, evaluator, and codegen are unaffected
because the new form lowers to the same AST a typed lambda already produces (a
`Lambda { params, ret_ty: INVALID, body }` node, identical to the untyped
inferred-return case).

---

## Spec & Grammar Impact

- **`grammar.ebnf`** (`lambda`, `simple_lambda`, `typed_lambda` productions):
  replace the two-production form with the unified `lambda` / `lambda_tail`
  grammar above.
- **`14-expressions.md §lambda` (line 149)**: the sentence "A lambda expression is
  `x -> expr`, `(a, b) -> expr`, `() -> expr`, or `(x: Type) -> Type = expr`"
  gains the `(x: Type) -> expr` form; add the disambiguation rule and the bare
  `-> Type` edge-case diagnostic as normative text.
- Developer-facing syntax quick references: add `(x: int) -> x * 2` to the
  Lambdas examples (post-approval documentation sync, not a spec edit).
- No change to Clause 8 (types), Clause 15 (patterns), or any operator rule.

---

## Prior Art

Return-type inference for typed-parameter anonymous functions is the norm; Ori's
current mandatory annotation is the outlier.

- **Rust** — `|x: i32| x * 2` infers the return type; the explicit form
  `|x: i32| -> i32 { x * 2 }` is opt-in. Typed parameters never force a return
  annotation.
- **Swift** — closures infer their return: `{ (x: Int) in x * 2 }`. The explicit
  `-> Int` is optional.
- **Gleam** — `fn(x: Int) { x * 2 }` infers the return type; `fn(x: Int) -> Int`
  is optional.
- **TypeScript** — arrow functions infer return types from the body even with
  typed parameters: `(x: number) => x * 2`. The graph corpus shows return-type
  inference treated as a baseline capability (e.g. `typescript#40750`,
  `swift#11478`, `roc#9145` all concern *quality* of lambda return inference, not
  whether it exists).

Ori's explicit-return form maps cleanly onto Rust's `-> T { … }` and Swift's
`-> T in …`; this proposal simply makes the inferred-return path available with
typed parameters, matching the default behavior of all four languages.

---

## Errata (added 2026-05-31)

> **Bare `-> Type` edge case — refined during implementation.** The §Design and
> §Error Handling text proposed a targeted parse-time diagnostic for
> `(x: int) -> int` (a return type with no `=` and no body). That shape is
> syntactically indistinguishable at parse time from the valid inferred-return
> form `(x: int) -> x` (both are a single name followed by a lambda terminator),
> so a parse-time rejection is not achievable without also rejecting valid
> lambdas. Implemented behavior: `(x: int) -> int` parses as an inferred-return
> lambda whose body is the type name `int`, and the type-name-as-value fails type
> inference (`E2005`) where the body is checked. Spec `14-expressions.md` §Lambda
> was updated to describe this; no targeted parse diagnostic is emitted.
