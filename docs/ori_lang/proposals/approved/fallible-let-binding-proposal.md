# Proposal: Fallible Let Binding (`let?`)

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-07-17
**Approved:** 2026-07-17
**Affects:** Grammar (`let_expr`), spec (Clause 13 variables, Clause 15 patterns, Clause 16 §16.0.3 + §16.5 error propagation, Annex D formatting), parser (`let?` adjacency-composed like `as?`), type checker, canonicalizer, evaluator, all execution backends, formatter (`ori_fmt`), diagnostics (E2001 help text)
**Depends On:** —
**Amends:** let-list-pattern-refutability-proposal.md (adds the sanctioned fallible variant; Resolution A and E2001 for plain `let` are unchanged)
**Related:** simplified-bindings-proposal.md

---

## Summary

`let?` is the fallible variant of `let`, following the existing `as` / `as?` convention: appending `?` marks the form that can fail. A `let?` binding accepts any refutable pattern; on match it binds like `let`, and on mismatch it propagates `None` exactly like `?` on an `Option` (spec §16.5.2) — to the enclosing function or `try` boundary. An error-preservation invariant makes any shape that could silently discard a live `Err` a compile error steering to the `?` operator (which owns error propagation). Plain `let` remains irrefutable; E2001 remains correct and gains a help line suggesting `let?`.

---

## Motivation

Two frictions force deeply nested `match` pyramids for straight-line unwrapping code.

### The Problem in Practice

The by-value iterator protocol returns `(Option<Self.Item>, Self)`. The `Option` is buried inside a tuple, so neither `?` nor `??` can reach it, and refutable patterns are illegal in `let` (E2001). Consuming five elements requires five nested `match` levels — from `tests/spec/traits/iterator/double_ended.ori`:

```ori
@list_interleaved_full () -> [int] = {
    let iter = [1, 2, 3, 4, 5].iter();
    match iter.next() {
        (Some(f1), iter2) -> match iter2.next_back() {
            (Some(b1), iter3) -> match iter3.next() {
                (Some(f2), iter4) -> match iter4.next_back() {
                    (Some(b2), iter5) -> match iter5.next() {
                        (Some(m), _) -> [f1, b1, f2, b2, m],
                        _ -> []
                    },
                    _ -> []
                },
                _ -> []
            },
            _ -> []
        },
        _ -> []
    }
}
```

With `let?`, the same function is flat:

```ori
@list_interleaved_full () -> Option<[int]> = {
    let iter = [1, 2, 3, 4, 5].iter();
    let? (Some(f1), iter) = iter.next();
    let? (Some(b1), iter) = iter.next_back();
    let? (Some(f2), iter) = iter.next();
    let? (Some(b2), iter) = iter.next_back();
    let? (Some(m), _) = iter.next();

    Some([f1, b1, f2, b2, m])
}

// caller picks the default:
list_interleaved_full() ?? []
```

### When This Matters

- **Any nested-Option / tuple-wrapped-Option shape** — the `?` operator only reaches a top-level `Option`/`Result`; `let?` reaches one at any pattern depth.
- **Sequential unwrapping chains** — parsers, iterator threading, config lookup chains, any "N fallible steps then combine" function.
- **List destructuring** — let-list-pattern-refutability-proposal.md (Resolution A, STRICT) correctly made `let [a, b] = xs;` illegal and left `match` as the only recourse. `let? [a, b] = xs;` and `let? [head, ..tail] = xs;` restore first-class list destructuring in honestly-fallible form.
- **Refutable variant extraction** — `let? Circle(radius:) = shape;` (top-level `Ok(..)`/`Some(binding)` forms are `?`'s territory — error and warning respectively; see Design).

---

## Goals and Non-Goals

**Goals:**

- One general, pattern-depth-agnostic early-exit binding form with zero per-line ceremony.
- Reuse the `?` mental model verbatim — no new propagation semantics, no new keywords.
- Keep plain `let` irrefutable (Resolution A untouched).

**Non-Goals:**

- A local-fallback clause (`let? ... else <expr>`) — deferred; `??` at the call boundary covers value defaults (see Unresolved Questions).
- Refutable patterns in function parameters or `for` loop variables — the Clause 15 context table rows for those positions are unchanged.
- A `while let`-style loop-conditional form. A `let?` mismatch inside a loop body exits the enclosing FUNCTION (or `try` boundary), never the loop — loop-scoped early exit remains `break` / labeled-block territory (see Drawbacks).
- Changing the `Iterator` trait signature or adding stdlib adapter methods (see Alternatives).

---

## Design

### Syntax

```ebnf
let_expr = "let" binding_pattern [ ":" type ] "=" expression
         | "let?" fallible_pattern [ ":" type ] "=" expression .
```

- `"let?"` is written with no interior space. Mechanism: parser-level composition of the `let` keyword token and an ADJACENT `?` token — the same mechanism that implements `as?` today — with a normative adjacency requirement (no whitespace between the tokens) applying to BOTH `let?` and `as?`. The shipped parser currently accepts `as ?` with interior space, diverging from `grammar.ebnf`'s quoted `"as?"` terminal; that divergence is a parser bug fixed alongside this proposal (spec is SSOT). Annex D states the no-interior-space rule normatively for both forms.
- The plain form keeps today's `binding_pattern` (irrefutable shapes); behavior unchanged.
- `fallible_pattern` is the full `match_pattern` grammar (literals, variants, structs, tuples, lists, ranges, or-patterns, at-patterns), extended with the `$` immutability marker at every identifier BINDING position. Sketch (full production lands in `grammar.ebnf` at spec-sync):

```ebnf
fallible_pattern = literal_pattern | f_binding | wildcard_pattern | f_variant
                 | f_struct | f_tuple | f_list | range_pattern | f_or | f_at .
f_binding        = [ "$" ] identifier .
f_at             = [ "$" ] identifier "@" fallible_pattern .
```

  where `f_variant` / `f_struct` / `f_tuple` / `f_list` mirror their `match_pattern` counterparts with `fallible_pattern` elements, the list rest slot is `".." [ [ "$" ] identifier ]` (anonymous rest `[a, ..]` stays legal, matching `match_pattern`), `f_or` joins `fallible_pattern` alternatives with `|`, and `range_pattern` endpoints keep `const_pattern` (`"$" identifier` = constant reference) unchanged. The `$`-admitting positions are exactly: standalone bindings, variant/struct/tuple/list element bindings, the list rest binding, and the at-pattern binder (`let? (Some($x), rest) = ...`). At spec-sync the production is authored as a PARAMETERIZED extension of `match_pattern` (one pattern grammar, `$` admitted at binding positions only in `let?` context) — never a mirrored second family, so future pattern-form changes touch one production.
- The `[ ":" type ]` annotation ascribes the SCRUTINEE type (checked against `expression` before pattern matching), not any individual binding — the only reading that generalizes to multi-binding patterns.
- **`$` disambiguation** — `match_pattern` already uses `$identifier` as a compile-time-constant REFERENCE inside range-pattern endpoints (`const_pattern`). In `fallible_pattern`, `$identifier` at an identifier BINDING position is the immutability marker (the `binding_pattern` meaning — `let?` is a let-form and sides with `let`'s convention); the constant-reference reading remains only where `const_pattern` is grammatically admitted (range endpoints). A standalone `$name` equality pattern is not part of `fallible_pattern` — constant-equality dispatch uses a `match` guard (`match v { x if x == $MAX -> ..., _ -> ... }`), since `match_pattern` admits no standalone `$name` equality form either.
- **Constant-shadow diagnostic** — a `fallible_pattern` `$`-binding whose name shadows a VISIBLE `$` constant (module-level or outer-scope) produces a warning: `warning: 'let? ... $MAX ...' binds a new $MAX, shadowing the constant $MAX` with `help: to compare against the constant, use a match guard: 'x if x == $MAX'`. This forecloses the constant-pinning misread (a user trained by range-endpoint `$MIN..$MAX` syntax expecting Erlang-style pinning).
- Arm-level guards (`pattern if cond`) are a `match`-arm construct, not a pattern, and are not part of `let?`.

```ori
let? (Some(x), rest) = iter.next();      // tuple-wrapped Option — ? cannot reach this
let? [first, second, ..rest] = xs;       // list destructuring returns, fallibly
let? Circle(radius:) = shape;            // variant refinement
let? Some(42) = lookup(key);             // literal refinement
```

- A `let?` whose pattern is an `Ok`-head over a `Result` is a compile ERROR steering to `?` (per the Semantics error-preservation invariant — `let? Ok((a, b)) = f();` ≡ `let (a, b) = f()?;`, which preserves the error). A bare top-level `Some(binding)` over an `Option` is legal but produces a WARNING suggesting `expr?` (`let? Some(x) = opt;` ≡ `let x = opt?;`) — one-way-to-do-things; `let?` is for the shapes `?` cannot reach.

### Semantics

`let? PAT = expr;` followed by the rest of the block desugars to:

```ori
match expr {
    PAT -> { /* rest of the enclosing block */ },
    _ -> /* propagate per §16.5 */
}
```

The mismatch arm is a terminating expression (`Never`, spec §16.6). `let?` has exactly ONE propagation carrier, governed by one rule and one invariant:

1. **Propagation rule** — pattern mismatch propagates `None`, exactly as `?` on an `Option` does (§16.5.2). The enclosing function's return type shall be compatible with the propagated `Option`. There is no shape-selected carrier: `let?` never propagates an `Err` itself — shapes where an `Err` would need propagating are compile errors steering to the `?` operator, which already owns that job.

2. **Error-preservation invariant** — no `let?` mismatch path may silently discard a live `Err` value. The compiler computes, per pattern shape, whether a value containing an `Err` — at the scrutinee's top level OR at any nested `Result`-typed position the pattern inspects with a refutable sub-pattern — can take the mismatch branch. If it can, the `let?` is a compile-time error whose help line names the exact equivalent or decomposition:
   - An `Ok` head with an IRREFUTABLE interior (`let? Ok((a, b)) = fetch(url);`) — error; `help: use the ? operator: 'let (a, b) = fetch(url)?;'`. The suggested form is semantically identical and preserves the `Err` with trace recording — `?` already owns this shape completely.
   - An `Ok` head with a REFUTABLE interior (`let? Ok(Some(x)) = g();`) — error; `help: unwrap with ? first: 'let inner = g()?; let? Some(x) = inner;'` (or convert: `.ok()`).
   - An `Err` head with a refutable interior (`let? Err(Timeout) = poll();`) — error (the mismatch may be a DIFFERENT live `Err`); `help: bind the whole error: 'let? Err(e) = poll();' then match e`.
   - A refutable sub-pattern inspecting a nested `Result`-typed position (`let? (Ok(cfg), rest) = pair;`, `let? Some(Ok(x)) = h();`) — error; decompose first.
   - Shapes that PASS over `Result`: an `Err` head with irrefutable interior (`let? Err(e) = f();` — None on `Ok`; genuinely inexpressible via `?`), and a nested `Result` position bound WHOLE by an irrefutable binding (`let? (Some(a), res) = t;` — binding a `Result` never discards it).
   - Or- and at-patterns: the invariant is evaluated over the WHOLE pattern — the mismatch region is the complement of the union of all or-alternatives, and at-patterns are transparent. So `let? Ok(x) | Err(Timeout) = f();` is an error (its mismatch region holds `Err(non-Timeout)` — a live `Err`), while `let? Ok(x) | Err(e) = f();` is irrefutable (warning: use `let`).

3. **`try` interaction** — inside a `try` block, propagation targets the `try` boundary rather than the enclosing function, exactly as `?` does (§16.7.3). The carrier-compatibility requirement is unchanged from `?`: where a `None`-carrier propagation would be ill-typed at a `Result`-typed `try` boundary for `expr?`, the same `let?` mismatch is equally ill-typed with the same diagnostic — `let?` introduces no new boundary-absorption semantics.

The §16.5 spec subsection shall include a short shape table (pattern shape over `Result` → legal / error + suggested form) so readers never run the invariant analysis unaided. Division of labor in one line: **`?` owns error propagation; `let?` owns refutable binding with `None` propagation; the invariant's errors are the seam between them.**

A `let?` is a block statement: it shall appear only in block-statement position (a new §16.0.3 clause row confines it exactly as plain `let`). When a `let?` is the final statement of a block, the match arm's body is empty and the block's value is `void`, per §16.0.3's existing let-terminated-block rule.

Bindings introduced by `PAT` are in scope for the remainder of the enclosing block, with mutability per the `$` marker (simplified-bindings-proposal.md). Shadowing rules are those of `let`.

### Type Rules

- The pattern is checked against the scrutinee type exactly as a `match` arm pattern is.
- Exhaustiveness is trivially satisfied (implicit `_` arm); no non-exhaustive-match diagnostic can arise. (The proposal names no error code here: the compile-time producer currently emits the overloaded E3002 while the registry's named runtime code E6040 is dormant — code allocation is an implementation-time decision.)
- **Or-pattern binding consistency** — the existing `match` rule carries over: bindings in or-patterns shall appear in all alternatives with the same names and types (Clause 15). `let?` adds one requirement the pre-`$`-marker rule could not state: the `$` immutability marker shall agree per binding name across all alternatives (`let? Some($x) | Fallback($x) = e;` is legal; mixing `$x` and `x` across alternatives is an error).
- An **irrefutable** pattern under `let?` produces a warning ("pattern is irrefutable; use `let`") — the `?` is inert and misleading, the dual of E2001.
- `let?` in a context that cannot absorb the `None` propagation (function return type incompatible with `Option`) is a compile-time error, reusing the existing `?`-context diagnostic semantics; a dedicated code may be allocated during implementation.

### Error Handling / Diagnostics

- E2001 (refutable pattern in `let`) gains a help line: `help: if the pattern is allowed to fail, use 'let?' — mismatch propagates like '?'`. Prior art shows this suggestion channel is load-bearing (rust-lang/rust#122404 tracks Rust failing to suggest its equivalent form).
- The irrefutable-`let?` warning carries the inverse help: `help: this pattern always matches; use 'let'`.
- The redundant-form warning (bare top-level `Some(binding)` over `Option`) carries: `help: this is equivalent to 'let <name> = expr?'`.
- The error-preservation errors (Semantics rule 2) each carry the exact suggested form named there (`let <interior> = expr?;` for `Ok`-heads; decomposition or `.ok()` for the rest) — specified here, not left to implementation, since the suggestion channel is the load-bearing surface (rust#122404).

### Composition

- `let? PAT = expr?;` composes: the `?` applies to `expr` first, then the pattern applies to the unwrapped value.
- `let?` inside labeled blocks, loops, and `try` follows the same rules as `?` in those positions; `break`/`continue` are unaffected.
- `??` remains the value-level default operator; `fallible_fn() ?? fallback` is the idiomatic way to recover a default from a `let?`-using function.

---

## Drawbacks

- **Invisible control flow at the binding site.** A mismatch exits the function with no arrow, no `else`, no block — one character (`?`) carries the early exit. This is the same trade `expr?` already made; `let?` extends an accepted convention rather than introducing a new risk class, but it does widen the surface where a reader must notice a `?`.
- **A third binding form.** `let`, `let $`, and now `let?` (each composable). The refutability split (`let` = always matches, `let?` = may fail) is a real distinction users must learn, though it replaces learning "why is my `let` rejected with E2001".
- **Silent typo hazard.** A misspelled variant name in a `let?` pattern silently propagates `None` instead of erroring at the site. Mitigated by exhaustive variant checking (an unknown variant name is still a type error; only *reachable-but-unintended* patterns degrade silently) and by the irrefutable-pattern warning.
- **Full-pipeline cost.** Parser, type checker, canonicalizer (desugar), evaluator, and every backend see the new form — though the desugar target (`match` + existing `?` propagation) means no backend needs new runtime machinery. One honest caveat: the desugar is DEFINITIONAL, not literal — `match_pattern` carries no `$` markers, so the canonicalizer's internal pattern representation gains per-binding mutability to express the desugared form.
- **The invariant's errors add a learning surface.** Several `Result`-shaped patterns are compile errors rather than "just working" — the price of the one-carrier design (a shape-selected `Err` carrier was considered and REJECTED: every shape it would serve is already exactly `let <interior> = expr?;`, and no shipped language has field experience with shape-selected propagation carriers). Each error's help line names the exact working form, so the surface teaches itself.
- **Loop bodies: mismatch exits the function, not the loop.** Users arriving from Rust's `while let` may expect loop-scoped exit; `let?` propagates to the function (or `try`) boundary from any depth, loops included. Loop-scoped patterns stay on `match` + `break` / labeled blocks; a loop-conditional form is an explicit Non-Goal.

---

## Alternatives Considered

### Alternative 1: stdlib adapters `try_next` / `try_next_back` on Iterator

Default methods returning `Option<(Self.Item, Self)>` put the `Option` outside the tuple where `?` can reach it. Minimal and worthwhile on its own terms, but narrow: it cures only the iterator protocol, not list destructuring, nested variants, or any other refutable-binding shape. Rejected as the *primary* cure; may still land independently as ergonomic stdlib surface.

### Alternative 2: Rust-style `let PAT = expr else { ... }`

General, proven, but: (a) repeats an `else { ... }` block on every binding — five bindings, five fallback clauses; (b) the brace-block requirement is foreign to Ori's `if c then a else b` expression style; (c) shipping a second let-form distinct from `?` propagation creates the diagnostic-confusion overhead both Rust and Swift carry today — rust-lang/rust#122404 (open: `let else` not suggested for refutable let patterns) and swiftlang/swift#81728 (open: improve diagnostics for `let…else` without `guard`) document the cost of near-identical binding forms competing in one grammar.

### Alternative 3: Swift-style `guard let ... else ...`

Reads well and the `guard` keyword signals intent, but adds a keyword, keeps the per-line `else` clause, and duplicates what `?` already means in Ori. Same two-form confusion cost as Alternative 2 (swift#81728 is precisely a guard-vs-bare-`let…else` confusion report).

### Alternative 4: Elixir-style `with`-chain block

One block chaining many refutable bindings with a shared fallback. Ori's `with` keyword is already taken by capability binding (`with Cap = impl in expr`), so this needs a new context-sensitive keyword and an entire new construct with its own scoping rules — the heaviest option, and its single shared fallback is less precise than per-binding propagation.

### Alternative 5: Kotlin/Zig-style unwrap-or-diverge operator only

Ori already has this: `??` unwraps (`Option<T> ?? T → T`, COALESCE-UNWRAP) and `Never` coerces to any type, so `opt ?? panic(msg: "...")` and `opt ?? break:done value` work today for a *top-level* `Option` value. It cannot reach a pattern — `(Option<Item>, Self)` stays out of reach — so it does not generalize.

### Alternative 6: Relax E2001 (refutable `let` panics on mismatch)

Rejected outright: reverses the deliberate Resolution A of let-list-pattern-refutability-proposal.md and hides a panic in every destructuring line. Erlang's badmatch-crash semantics is the cautionary prior art.

---

## Purity Analysis

**Can be pure Ori?** NO
**If not, why:** New surface syntax (grammar production change) plus static analysis (pattern refutability classification, propagation-context typing). Both are compiler-only categories under the purity principle.
**Missing features that would enable purity:** None plausible — binding forms are irreducibly syntactic.
**Recommendation:** Proceed as compiler feature. The implementation is a thin layer: one grammar token, a desugar to existing `match`, and reuse of the existing `?` propagation machinery in the type checker and all backends. Alternative 1 (stdlib adapters) remains available as an independent, pure-Ori complement.

---

## Spec & Grammar Impact

| Surface | Change |
|---|---|
| `grammar.ebnf` | `let_expr` gains a second alternative: `"let?" fallible_pattern [ ":" type ] "=" expression` with `"let?"` as a single terminal; new `fallible_pattern` production (`match_pattern` + `$` markers at binding positions) |
| `13-variables.md` | New subsection: fallible bindings (`let?`), semantics + examples; §13.4's existing sentence "Refutable list shapes shall be bound through `match`" is amended to name `let?` as the binding-form recourse |
| `15-patterns.md` | Context table gains a row: `let?` binding — any pattern (refutable OK); E2001 text gains the `let?` help pointer; the "Guards" bullet under Refutable patterns gains a clarifying note (guards are an arm-level construct, not a pattern form admitted in bindings) |
| `16-control-flow.md` §16.0.3 | `let?` confined to block-statement position exactly as plain `let`; let-terminated-block value rule extended |
| `16-control-flow.md` §16.5 | New subsection: `None` propagation from fallible bindings + the error-preservation invariant with its shape table; §16.6 gains the mismatch arm as a terminating-expression producer; the §16.7.1 NOTE "There is no `if let` syntax" is amended to point at `let?` for refutable bindings |
| Annex D (formatting) | `let?` (and `as?`) written with no interior space |
| Diagnostics | E2001 help line; irrefutable-`let?` warning; redundant-form warning; `.ok()` context suggestion; error-preservation errors; `?`-context error reuse |

**Propagation audit at approval** (per proposals.md §Errata): `let-list-pattern-refutability-proposal.md` gains an errata block — its "use `match` instead" recourse framing and the E2001 help text it fixed become partially stale once `let?` lands (Resolution A itself is untouched: plain `let` stays irrefutable).

---

## Prior Art

All issue references verified against the live trackers on 2026-07-17.

- **Haskell (do-notation / MonadFail)** — the direct semantic ancestor: a failed pattern bind in a `Maybe`-monad `do` block yields `Nothing`, which is `let?`'s propagation rule surfaced explicitly by a `?` marker instead of implicitly by monadic context. The cautionary half of the same record: GHC split `MonadFail` out of `Monad` (8.0) because IMPLICIT pattern-bind failure — invisible at the binding site, active in every `do` block — was judged a wart. `let?` is designed against that history: failure is opt-in per binding and visible as the one-character marker, exactly the property the implicit form lacked. Notably, `Either` has no lawful `MonadFail` (it cannot manufacture an error value) — the same fact that led this proposal to reject an `Err`-propagating carrier and route `Result` shapes to `?` instead.
- **Rust (`let-else`, RFC 3137)** — general refutable-let with a mandatory diverging `else` block. Works, but the separate form needs its own diagnostics channel: rust-lang/rust#122404 (open) — E0005 does not suggest `let else` for refutable let patterns.
- **Swift (`guard let ... else`)** — dedicated early-exit keyword; compiler enforces the else block diverges. swiftlang/swift#81728 (open) — diagnostics confusion between `guard let…else` and bare `let…else` shows the cost of multiple near-identical binding forms.
- **Kotlin (`?: return` elvis)** / **Zig (`orelse`)** — expression-level unwrap-or-diverge; value-only, cannot destructure patterns. Ori's existing `?? <Never-expr>` is this feature.
- **Elixir/Erlang (`with` special form)** — chained refutable binds with shared `else`; on mismatch returns the unmatched value itself, a dynamically-typed trick that does not transplant to a statically-typed language without an error-union type per chain.
- **Gleam (`use` expressions)** — generalized continuation sugar that flattens callback nesting including `result.try` chains; powerful but introduces continuation-passing semantics far heavier than a binding form.
- **OCaml (binding operators, `let*`/`let+`)** — user-defined monadic lets; flattens chains but requires explicit monad plumbing per type.
- **Ori (internal)** — `as` vs `as?` (infallible vs fallible cast) establishes the exact naming convention `let?` extends; `expr?` (§16.5) supplies the complete propagation semantics `let?` reuses.

---

## Unresolved Questions

- **Optional local fallback** — should a later amendment add `let? PAT = expr else <never-expr>` for functions that do not return `Option`? Deferred: labeled blocks + `??` cover the known cases; adding it later is backward-compatible.
- **Dedicated error codes** — whether the propagation-context error reuses the existing `?`-context code, and which codes the error-preservation errors and warnings receive. Resolve during implementation.
