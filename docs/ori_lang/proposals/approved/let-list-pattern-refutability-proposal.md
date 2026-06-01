# Proposal: Resolve `let`-Binding List-Pattern Refutability

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-05-31
**Approved:** 2026-05-31
**Resolution:** A (STRICT) — `15-patterns.md` is authoritative; dynamic-length list patterns stay refutable; `let` stays irrefutable; E2001 is correct.
**Affects:** Spec (Clause 13 variables, Clause 15 patterns), test corpus. Type checker / evaluator / codegen: NO change under Resolution A (E2001 unchanged).
**Resolves:** Spec-internal contradiction between Clause 13 (variables) and Clause 15 (patterns) over whether list-destructuring patterns are legal in `let` bindings.
**Amends:** simplified-bindings-proposal.md (errata — its `let [$head, ..tail] = list` example becomes invalid)

---

## Summary

Two spec clauses disagree on whether list-destructuring patterns are legal in `let` bindings. `13-variables.md` shows `let [head, ..tail] = list;` as a valid core feature; `15-patterns.md` classifies list-with-length patterns as refutable and requires `let` bindings to be irrefutable, making the same form illegal. The compiler implements `15-patterns.md` and rejects `let [a, b, c] = [1, 2, 3]` with E2001. This proposal resolves the contradiction by picking one authoritative rule and aligning the loser clause, the compiler, and the test corpus to it.

---

## Problem Statement

### The contradiction

`13-variables.md:106-107` (normative examples):

```ori
let [head, ..tail] = list;             // head mutable, tail mutable
let [$head, ..tail] = list;            // head immutable, tail mutable
```

`15-patterns.md:273` lists `Lists with length ([a, b])` under **Refutable patterns**, and `15-patterns.md:279` requires:

| Context | Requirement |
|---------|-------------|
| `let` binding | Must be irrefutable |

A refutable pattern is illegal in `let`. So `13-variables.md`'s prominent examples directly contradict `15-patterns.md`'s refutability table.

### Current compiler behavior

The compiler implements `15-patterns.md`. `let [a, b, c] = [1, 2, 3];` fires:

```
error[E2001]: refutable pattern in let-binding: this list pattern requires
exactly 3 elements, but [T] has no compile-time length
  = help: Use `match` instead — `match` arms accept refutable patterns
```

This poisons `tests/spec/patterns/binding_patterns.ori` (the entire "List Destructuring in Let" section), which is authored against `13-variables.md` and assumes the form works.

### The underlying principle

The clean distinction `15-patterns.md` draws:

- **Static-arity patterns are irrefutable** — a tuple `(a, b)` has type `(T, U)` (exactly 2 elements, statically known); a struct `{x, y}` binds known fields; a fixed-capacity `[T, max N]` has a compile-time capacity. These always match.
- **Dynamic-length list patterns are refutable** — a `[T]` (dynamic list) has unknown length at compile time, so a fixed-arity pattern `[a, b, c]` (or a minimum-length pattern `[head, ..tail]`) may fail to match.

`13-variables.md`'s `let [head, ..tail] = list` destructures a dynamic `[T]` with a minimum-length pattern — refutable under this principle.

---

## Design

Pick exactly one of the two resolutions below. The remainder of this section specifies each; the review gate selects.

### Resolution A — STRICT (15-patterns.md is authoritative) — recommended

Dynamic-length list patterns stay refutable; `let` continues to require irrefutable patterns. The compiler's E2001 is correct.

- **Spec:** `15-patterns.md` is unchanged. `13-variables.md` is corrected — the `let [head, ..tail] = list` examples are replaced with either `match` (for dynamic lists) or fixed-capacity `[T, max N]` destructuring (for static-arity lists).
- **Type checker:** no change. E2001 stays.
- **Evaluator / codegen:** no change (no new runtime path).
- **Corpus:** the `binding_patterns.ori` "List Destructuring in Let" tests are rewritten to `match`, or relocated to a `match`-based file. The irrefutable tuple/struct tests pass once the refutable list tests no longer poison the file.
- **Irrefutable list path — target-only.** `let [a, b, c] = v` being legal when `v: [T, max 3]` (fixed-capacity, static arity, irrefutable) is the TARGET state, NOT current behavior. The refutability checker (`ori_types/src/infer/expr/refutability.rs`) currently rejects every non-empty list pattern regardless of scrutinee type, because `[T, max N]` is erased to `Tag::List` during type resolution (capacity-preserving encoding is target-only per `types.md PT-2`/`TL-2`). The static-arity-list-in-`let` path activates only when fixed-capacity erasure lifts — **blocked-by** the fixed-capacity-list encoding work (`fixed-capacity-list-proposal.md` implementation). Until then, ALL list patterns in `let` are refutable and fire E2001.

### Resolution B — ERGONOMIC (relax the rule)

Permit list-destructuring in `let` with **runtime** refutability — a length-mismatch panics at runtime, the same way an out-of-bounds index panics.

- **Spec:** `13-variables.md` is authoritative. `15-patterns.md:273` + `:279` are amended so dynamic-length list patterns are permitted in `let` with documented runtime-panic semantics; rest-patterns `[head, ..tail]` require `length >= fixed-prefix-count`.
- **Type checker:** E2001 no longer fires for list patterns in `let`.
- **Evaluator / codegen:** new runtime length-check + panic path for `let`-bound list patterns (a previously panic-free context gains a panic edge).
- **Corpus:** `binding_patterns.ori` passes as authored; add negative tests pinning the runtime panic on length mismatch.

### Error Handling

- **Resolution A:** E2001 unchanged; its help text already points to `match`.
- **Resolution B:** E2001 removed for `let` list patterns; a new runtime panic (`E6xxx` range) on length mismatch, with a message naming the expected vs actual length.

---

## Alternatives Considered

### A1 — Infer list literals as fixed-capacity in destructuring position (TARGET-ONLY, deferred)

Under Resolution A, `let [a, b, c] = [1, 2, 3]` could be made legal by inferring the literal `[1, 2, 3]` as `[int, max 3]` (static arity) rather than `[int]` (dynamic). The fixed-arity pattern is then irrefutable. This makes the common literal case work without relaxing refutability, while `let [head, ..tail] = some_runtime_list` (RHS is dynamic `[T]`) stays refutable. A1 is a refinement of Resolution A, not a separate resolution — and it is **target-only, deferred**: it depends on capacity-aware refutability, which requires the fixed-capacity encoding (`types.md PT-2`/`TL-2` erasure) to ship first. A1 is NOT delivered by this proposal; it is **blocked-by** the fixed-capacity-list implementation and tracked separately when that lands.

### Rejected: leave the contradiction unresolved

Leaving both clauses as-is keeps `binding_patterns.ori` poisoned and the spec self-contradictory. Rejected — `missions.md §Conflict resolution` requires resolving same-tier spec contradictions, not tolerating them.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** Refutability is a static type-system rule enforced in `ori_types`; the `let` binding form is core syntax. Resolution B additionally requires evaluator + codegen runtime support. Neither can live in a library.
**Recommendation:** Proceed as a compiler + spec change. Resolution A is the minimal change (spec + corpus only, no new runtime path); Resolution B adds an evaluator/codegen runtime-panic path.

---

## Resolution Selected — A (STRICT)

The review gate selected **Resolution A**. Dynamic-length list patterns stay refutable; `let` continues to require irrefutable patterns; the compiler's E2001 is correct and unchanged. Rationale: Ori's conflict-resolution rule makes type safety non-negotiable and prefers explicit-over-implicit and one-way-to-do-things (`missions.md §Ori`); Resolution B's runtime-panic `let` injects an implicit panic edge into a previously panic-free context. The cross-language norm (Rust, Swift, Gleam, OCaml, Elm) is unanimously A.

## Conflict Provenance — Two Approved Proposals

The contradiction is not merely between two spec clauses — each clause faithfully implements a different **approved** proposal:

- `pattern-matching-exhaustiveness-proposal.md` (Approved) is the SSOT `15-patterns.md` implements: "List with length `[a, b]` → Refutable" + "`let` binding → Must be irrefutable" + the explicit `let [a, b] = get_list()` → Error example. Resolution A keeps this intact.
- `simplified-bindings-proposal.md` (Approved) is the SSOT `13-variables.md` implements: its `let [$head, ..tail] = list` example. Resolution A **invalidates** this example, requiring an errata block.

## Spec & Grammar Impact

- `15-patterns.md` §Pattern Refutability (lines 259-281): **unchanged** (Resolution A).
- `13-variables.md` §13.4 Destructuring (lines 96-111): **corrected** — the `let [head, ..tail] = list` / `let [$head, ..tail] = list` examples are replaced with `match` (dynamic lists) or fixed-capacity `[T, max N]` destructuring (static arity).
- `grammar.ebnf`: no change — `let <pattern> = <expr>` already parses list patterns; refutability is a post-parse type-checker rule.

## Propagation Audit — Required Errata

- **`simplified-bindings-proposal.md`** (Approved): add an `## Errata` block superseding its `let [$head, ..tail] = list` example, per `proposals.md §Errata`. Approved proposals are never silently rewritten — the errata records that dynamic-length list destructuring in `let` is now an error and directs readers to `match` / `[T, max N]`.
- The test corpus header in `tests/spec/patterns/binding_patterns.ori` cites `10-patterns.md` (stale clause number; clauses renumbered to 13/15) — correct it during the corpus rewrite.

---

## Prior Art

- **Rust** — a plain `let PATTERN = EXPR` requires an irrefutable pattern; refutable patterns require `if let`, `let ... else { ... }` (let-else, RFC 3137), or `match`. Array patterns `let [a, b, c] = arr` are legal only when `arr: [T; N]` (fixed-size array, static length → irrefutable); slices `&[T]` (dynamic length) require `if let`/`let-else` (refutable). Issues `rust#152938`, `rust#123844` track diagnostic quality for refutable-let. This maps directly onto Ori's `[T, max N]` (≈ `[T; N]`, irrefutable) vs `[T]` (≈ `&[T]`, refutable) — strong support for **Resolution A**.
- **Swift** — `let (a, b) = pair` (tuple, irrefutable) is allowed; array/collection destructuring with a fixed count is not a plain-`let` binding form (use `if case`/`guard case` for refutable matches). Aligns with A.
- **Gleam / OCaml / Elm** — list patterns in `let` are refutable and require an exhaustive `case`/`match`; plain `let` accepts only irrefutable patterns. Aligns with A.

The cross-language norm (Rust, Swift, Gleam, OCaml, Elm) is **Resolution A**: dynamic-length list destructuring is refutable and belongs in `match`/`if let`, not plain `let`. Resolution B (runtime-panic `let`) is the minority choice. The recommendation is A; the review gate makes the binding call.
