# Proposal: Call-Site Method-Generics Grammar Alignment

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-06-16
**Approved:** 2026-06-16
**Affects:** grammar, parser (`ori_parse`), type system (`ori_types`), spec (Clause 14 Expressions, Clause 27 Reflection)
**Depends On:** method-generics-grammar-alignment-proposal.md (approved), const-generics-proposal.md (approved — partially superseded by capability-unification-generics-proposal; its bound syntax is `:` and its call-site const arguments are bare values, NOT `with`-bounds — see "Dependency Status" below)

---

## Summary

Add call-site grammar for explicit type/const arguments on generic calls — a
type-argument list after the callee name, before the call parentheses:
`obj.method<T>(arg)`, `value.is<int>()`, `xs.to_fixed<$N>()`, and the
free-function form `replicate<_, 5>(value: x)` that `const-generics-proposal.md`
already approves. The approved `method-generics-grammar-alignment-proposal.md`
shipped definition-site grammar (`@method<T, $N: int> (...) -> R`) but explicitly
deferred the call site to "a follow-up proposal" because disambiguating `<`
(type-argument list) from `<` (less-than) is non-trivial. This proposal resolves
that disambiguation via speculative parse with resolve-time fallback and defines
the call-site production for both method calls and free-function calls.

> Title note: the filename and title are retained for continuity with the
> deferring proposal that named this follow-up. The scope covers all call-site
> type arguments (method + free-function), not method calls alone — see Goals.

---

## Motivation

Definition-site method generics are approved and parseable today:

```ori
impl<U> Stream<U> {
    @batch<T, $N: int> (self, transform: (U) -> T) -> [T, max N] = ...
}
```

But there is no way to **call** such a method when the type/const arguments
cannot be inferred from the value arguments alone.

### The Problem in Practice

```ori
// `to_fixed<$N>()` cannot infer N from arguments — N is the target capacity.
let r: [int, max 2] = xs.take(count: 2).to_fixed<$N>();   // unparseable today
```

Today the parser, after the member name `to_fixed`, only checks for `(`
(`ori_parse/src/grammar/expr/postfix.rs`). A following `<` falls through to the
comparison path, so `to_fixed<$N>()` parses as the nonsense chain
`(((to_fixed < $N) > ()) ...)` and produces cascading type errors
("expected `bool`, found `()`"). The same gap makes the reflection examples in
`27-reflection.md §27.4.1` (`value.is<int>()`, `value.downcast<int>()`)
unparseable, and it strands the free-function call-site forms
`const-generics-proposal.md` approves (`replicate<_, 5>(value:)`, `f<10>`).

### When This Matters

- Const-generic methods whose const parameter is a target, not an argument
  (`to_fixed<$N>()`, `try_to_fixed<$N>()` from `fixed-capacity-list-proposal.md`).
- Type-driven methods with no value argument to infer from (`value.is<int>()`,
  `parse<int>()`, `collect<C>()` when the collection type is not otherwise
  constrained).
- Free-function generic calls whose const/type arguments are targets, not
  inferable from value arguments (`replicate<_, 5>(value:)` per
  `const-generics-proposal.md`).
- Any generic call where the caller wants to pin the instantiation explicitly
  rather than rely on inference.

---

## Goals and Non-Goals

**Goals:**

- A grammar production for call-site type arguments on BOTH method calls
  (`receiver . name type_args ( args )`) and free-function calls
  (`name type_args ( args )`). Scope widened from method-only during review:
  `const-generics-proposal.md` already approves the free-function form, so a
  method-only scope would strand an approved surface (the failure Alternative 3
  rejects). One disambiguation mechanism covers both positions.
- A **sound** disambiguation rule distinguishing a type-argument list from a `<`
  comparison, with no new sigil (`<...>` matches definition-site spelling), that
  NEVER silently reinterprets an existing valid comparison expression.
- Parser support feeding the existing call-inference type-argument paths (the
  same machinery top-level generic function calls already use), reusing the
  existing `type_args` grammar production.

**Non-Goals:**

- Const-generic **value flow** through the type system (`[T, max N]` capacity
  propagation, `to_fixed<$N>` runtime). That is `const-generics-proposal.md` /
  BUG-02-023 — this proposal delivers only the *grammar* the call site needs.
- Turbofish on type paths (`Type::<T>` / `Type::<T>::assoc`) — call-site type
  arguments on method and free-function CALLS only.
- Changing definition-site grammar (already shipped).

---

## Design

### Syntax

A type-argument list may appear after a callee name (method member name or
free-function name), before the call argument list.

```ori
receiver.method<T1, T2>(arg1, arg2)
value.is<int>()
xs.to_fixed<$N>()
m.downcast<str>()
replicate<_, 5>(value: x)        // free-function call-site form (const-generics)
parse<int>(text)                 // free-function type-driven form
```

Type arguments reuse the existing `type_args` / `type_or_const` grammar
(`grammar.ebnf:355`): a comma-separated list of `type_or_const`, each of which is
a `type` or a `const_expr`. A const argument is a bare value or const expression
(`5`, `N`, `$N` as a `const_expr` reference to an in-scope const parameter) — the
const-generics call-site spelling (`replicate<_, 5>`), NOT a declaration-form
`$N : const_type`. An `_` placeholder requests inference of that one argument
position. The list MUST be immediately followed by `(` — call-site type
arguments attach only to a **call**.

### Grammar

This proposal REUSES the existing `type_args` production rather than defining a
new one. `grammar.ebnf:355` already provides:

```ebnf
type_args     = "<" type_or_const { "," type_or_const } ">" .
type_or_const = type | const_expr .
```

The change is to attach `[ type_args ]` into the existing call-bearing postfix
and primary productions, between the callee name and the call argument list:

```ebnf
(* method call: extend the existing postfix_op (grammar.ebnf:445) *)
postfix_op    = "." member_name [ type_args ] [ call_args ]
              | (* ...existing postfix alternatives unchanged... ) .

(* free-function call: extend the call-bearing primary path *)
(* an identifier primary may carry [ type_args ] before call_args *)
```

Notes:

- `member_name = identifier | keyword | int_literal` (`grammar.ebnf:452`) is
  retained — call-site type arguments are accepted after an `identifier`
  member name only. A tuple-index member (`t.0`) followed by `type_args` is a
  parse error (an `int_literal` member name cannot be a generic call target).
- No new `type_arg` / `const_arg` non-terminals are introduced. The `$N`
  call-site reference is already a `const_expr` (`grammar.ebnf:669`); declaration
  syntax (`const_param = "$" identifier ":" const_type`, `grammar.ebnf:259`)
  remains declaration-only.

### Disambiguation (the core decision)

A pure trailing-`(` lookahead is **unsound**: the valid chained comparison
`recv.field < c > (expr)` — today parsed left-associatively as
`((recv.field < c) > (expr))` — has exactly a `<` … balanced-`>` … `(` shape, so
a "commit to type_args when a balanced `>` is followed by `(`" rule would
silently re-parse it as the generic call `recv.field<c>(expr)`. That is the
ambiguity Rust's turbofish exists to avoid; a deterministic syntactic shortcut
cannot resolve it.

Instead, the parser uses **speculative parse with resolve-time fallback**,
grounded on existing parser machinery:

1. After a callee name, if the next token is `<`, take a snapshot
   (`parse.md §SN-3` snapshot / `try_parse` speculation) and attempt to parse a
   `type_args` list in type context (`parse.md §CF-1` `IN_TYPE` flag, which makes
   `>` close a generic rather than act as comparison) followed by `(`. This is
   the same identifier-then-`<` dispatch the channel constructors already use
   (`parse.md §KW-1`, `match_channel_kind`).
2. If the speculative `type_args` parse succeeds AND is immediately followed by a
   call argument list `(`, AND the resulting generic call resolves (the callee
   names a generic method/function for which the supplied arguments are valid),
   commit to the generic call.
3. Otherwise, restore the snapshot and parse `<` as the less-than operator (the
   current comparison path).

The resolve-time check in step 2 is what makes this sound: when both a generic
call and a comparison chain are syntactically possible (the `recv.field < c >
(expr)` shape), the comparison reading is preserved unless the type-arg reading
is the only one that resolves. This is the Swift `UnresolvedSpecializeExpr`
parse-then-resolve separation applied to the commit decision, and the
TypeScript speculative-parse-with-backtracking shape — not a syntactic shortcut.

### Migration / Breaking Changes

For the overwhelming majority of source this is purely additive: `obj.method<T>(arg)`
was a parse error before and is a generic call now. The single behavioral edge is
the `recv.field < c > (expr)` shape, where both a comparison and a generic call
are syntactically possible. The resolve-time fallback preserves the comparison
reading unless only the generic-call reading type-checks, so currently-compiling
comparison code keeps its meaning. The lexer already emits `>` as single tokens
(see `>>` below), so no tokenization changes.

### Semantics

Call-site type arguments bind positionally to the callee's declared type/const
parameters (definition order), with `_` placeholders requesting inference at
that position. A mixed binder `<T, $N: int>` binds positionally: the first
argument to `T`, the second to `$N`; either may be `_` to infer that position
while supplying the other (consistent with `const-generics-proposal.md`'s
`replicate<_, 5>`). The bound arguments instantiate the callee scheme through the
**existing** type-argument inference paths in `InferEngine` (the same ones used
for explicit top-level generic function calls per
`method-generics-grammar-alignment-proposal.md §Instantiation`).
Explicitly-supplied (non-`_`) arguments override inference; `_` placeholders and
unsupplied trailing parameters are inferred from value arguments and the expected
type.

### Error Handling

- Arity mismatch (MORE type arguments than declared callee parameters): reuse the
  existing arity diagnostic family (E2004-class), specialized to type parameters.
  Supplying FEWER than declared is NOT an arity error — unspecified trailing
  parameters and `_` placeholders are inferred per the `_`-placeholder rule.
- A `type_args` list not followed by `(` after a callee name parses as a
  comparison via the fallback (it is not a call), not a parse error — the
  speculative parse simply does not commit.
- Const-argument validation (a const argument against a declared `$N: int`
  parameter) is owned by const-generics value flow (`const-generics-proposal.md`);
  this proposal's grammar only delivers the parsed argument.

---

## Drawbacks

- **Parser complexity.** Speculative parse with snapshot/rewind is more involved
  than the current single-token `(` check. It reuses existing `IN_TYPE` (CF-1),
  snapshot (SN-3), and channel-dispatch (KW-1) machinery rather than adding new
  infrastructure.
- **Surface-area growth.** Another call-site form to teach, and a second/third
  place (besides definition sites) where `<...>` means type arguments.
- **Speculative-parse cost.** A `<` after a callee name triggers a speculative
  type-args parse plus, in the ambiguous `< … > (` shape, a resolve check.
  Bounded; the common comparison case (`a.b < c`) fails the speculative parse at
  the first non-type token and falls back immediately.

---

## Alternatives Considered

### Alternative 1: Rust-style turbofish marker (`obj.method::<T>(arg)`)

Introduce an explicit `::<>` (or other sigil) so no speculation is needed — `::<`
unambiguously starts a type-argument list. Rejected: Ori's definition site spells
generics `<T>` with no `::`; a call site spelled `::<T>` would be a jarring
inconsistency, and the whole point of `method-generics-grammar-alignment` was to
align spellings. Speculative parse with resolve-time fallback keeps `<T>` uniform
while remaining sound.

### Alternative 2: Pure deterministic trailing-`(` lookahead

Commit to `type_args` whenever a balanced `>` is immediately followed by `(`,
with no resolve-time check. Rejected during review: unsound — it silently
re-parses the valid comparison `recv.field < c > (expr)` as a generic call. The
resolve-time fallback (the chosen design) is required for soundness.

### Alternative 3: Require type arguments only via a type-annotated binding

Force callers to drive instantiation through the expected type
(`let r: [int, max 2] = xs.to_fixed();`) and never write call-site type args.
Rejected: it does not work when there is no binding/expected-type context
(`value.is<int>()` as a sub-expression), and it cannot express a const target
the value arguments do not carry.

### Alternative 4: Keep deferring (status quo)

Leave the call site unparseable. Rejected: it permanently strands approved
features — `27-reflection.md §27.4.1`'s `value.is<int>()`, the fixed-capacity
`to_fixed<$N>()` method, and `const-generics-proposal.md`'s free-function
`replicate<_, 5>` form remain uncallable, so definition-site method generics and
the approved const-generics call-site forms are half a feature.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** This is surface syntax — a grammar production plus parser
disambiguation. It necessarily touches `grammar.ebnf` and `ori_parse`, and feeds
`ori_types` call inference. No library construct can add call-site type-argument
syntax.
**Missing features that would enable purity:** None applicable (syntax cannot be
a library feature).
**Recommendation:** Proceed as a compiler/grammar feature. Scope is deliberately
narrow (grammar + parse + wiring into existing inference); const-generic value
flow stays in its own proposal.

---

## Dependency Status

- `method-generics-grammar-alignment-proposal.md` — Approved. Provides the
  definition-site grammar + the `InferEngine` type-argument instantiation paths
  this proposal feeds.
- `const-generics-proposal.md` — Approved (partially superseded by
  `capability-unification-generics-proposal.md`). The superseding errata replaced
  only the "Allowed Const Types" whitelist (now `Eq + Hashable` capability check)
  and reverted bound syntax to `:`; its syntax, monomorphization, inference, and
  const-bounds sections remain valid, and the call-site const-argument form
  (bare values, `replicate<_, 5>`) is intact. Bounds use `:` everywhere
  (`capability-unification-generics-proposal.md` §"`with` simplification": `with`
  has exactly one meaning — expression-level capability provision
  `with Cap = Expr in Expr`; there is no declaration-level or bound-position
  `with`). This proposal's examples use `:` for bounds and bare values for
  call-site const arguments accordingly.

---

## Spec & Grammar Impact

- `grammar.ebnf`: attach `[ type_args ]` into the existing `postfix_op`
  (`:445`, method-call path) and the call-bearing identifier-primary path
  (free-function path), reusing the existing `type_args` / `type_or_const`
  productions (`:355`). No new `type_args` / `type_arg` / `const_arg`
  non-terminals.
- AST: `ExprKind::MethodCall` / `ExprKind::MethodCallNamed` and the
  free-function call node gain an optional type-argument carrier field threaded
  into `InferEngine` instantiation. (AST/IR touchpoint — surfaced during review;
  the grammar alone does not deliver this.)
- Clause 14 (Expressions), call subclause: document call-site type arguments
  (method + free-function), the speculative-parse + resolve-time disambiguation,
  and positional binding with `_` placeholders.
- Clause 27 (Reflection) §27.4.1: its `value.is<int>()` / `value.downcast<int>()`
  examples become parseable; cross-reference this clause.
- `operator-rules.md`: note the `<` disambiguation interaction (a `<` after a
  callee name is a type-argument list iff the speculative type-args-then-`(`
  parse succeeds AND the resulting generic call resolves; otherwise `<` is
  less-than).

---

## Prior Art

- **Rust** — uses the `::<>` *turbofish* precisely because bare `obj.method<T>()`
  is ambiguous with comparison; Rust chose an explicit marker. `rustc_parse`
  carries `check_turbofish_missing_angle_brackets` and uses snapshot/restore for
  speculative recovery. Ori rejects the marker (Alternative 1) but adopts the
  snapshot-based speculative parse.
- **TypeScript** — parses type-argument lists **speculatively**: it attempts a
  type-argument parse and backtracks to a relational expression on failure
  (TS #1406 "speculative parsing when trying to parse type argument list";
  #60874 generic-syntax-ambiguity). This proposal's mechanism is the same shape:
  speculative parse, fall back to comparison on failure — with a resolve-time
  check for the residual `< … > (` ambiguity.
- **Swift** — represents `obj.method<T>` as an `UnresolvedSpecializeExpr`
  resolved later in type-checking (swift `UnresolvedSpecializeExpr` in
  `AST/Expr.h`), separating parse from resolution. This proposal applies the same
  parse-then-resolve separation to the commit decision.

---

## Resolved During Review (formerly Unresolved Questions)

- **`>>` handling — resolved (non-problem in Ori).** Per `parse.md §LB-1` the
  lexer ALWAYS emits `>` as a single token; `>>` is synthesized only in the Pratt
  expression layer (`parse.md §PR-4`). Nested generics (`m.cast<Map<str, int>>()`)
  already lex as separate `>` tokens — no lexer change and no Rust/C++/Java
  token-split is needed; the speculative type-args parse must simply not invoke
  PR-4 `>>` synthesis in `IN_TYPE` context. The one genuine residual is a shift
  expression INSIDE a const argument (`m.f<$N >> 1>(x)`): the speculative parser
  parses `const_expr` in `IN_TYPE` context, where `>>` is the shift operator
  within a parenthesized/precedence-bounded const expression and the type-arg
  list's closing `>` is the depth-zero token — the const expression must be
  parenthesized (`<($N >> 1)>`) to use a top-level shift, matching how other
  languages require parens for shifts in type-argument position.
- **Partial type arguments — resolved (`_` placeholder).** A call may supply a
  subset of type/const arguments using `_` placeholders to infer specific
  positions (`m.method<int, _>()`, `replicate<_, 5>`), consistent with
  `const-generics-proposal.md`. Supplying FEWER arguments than declared infers the
  unspecified trailing parameters. Mixed type/const binders bind positionally.
- **Free-function / associated-function turbofish — resolved (in scope).**
  Free-function call-site type arguments are now a Goal (the approved
  const-generics forms require them). Associated-function / type-path turbofish
  (`Type::<T>`) remains a Non-Goal.
- **Const-argument grammar — resolved (reuse `type_or_const`).** Call-site const
  arguments are `const_expr` values via the existing `type_or_const` production;
  no new `const_arg` non-terminal. `$N` is a valid `const_expr` reference at a
  call site.
