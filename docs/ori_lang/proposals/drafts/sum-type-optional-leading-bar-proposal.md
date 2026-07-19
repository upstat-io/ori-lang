# Proposal: Optional Leading Bar in Sum Type Declarations

**Status:** Draft
**Author:** Eric
**Created:** 2026-07-02
**Affects:** grammar, Compiler (ori_parse, ori_fmt), spec (Clause 8 — Declarations)
**Depends On:** none

---

## Summary

- Allow an optional leading `|` before the first variant of a sum type body, so
  `type E = | A | B | C` is accepted alongside today's `type E = A | B | C`.
- When a sum type is broken across multiple lines, `ori fmt` emits the leading `|`
  so every variant — including the first — sits in an aligned left gutter.
- Single-line sum types that fit within the width limit stay bare (`A | B | C`,
  no leading bar).
- Purely additive syntax relaxation: every program valid today stays valid and
  formats identically on one line.

---

## Problem Statement

Ori borrows the `|` variant separator from the ML family but its grammar currently
treats `|` as a strict *separator* (`sum_body = variant { "|" variant }`), so the
first variant can never carry a leading bar. When a sum type has enough variants to
break across lines, the formatter must emit a bare first variant:

### The Problem in Practice

```ori
// Today: first variant is bare, continuation variants carry the bar.
type Event =
    Click(x: int, y: int)
    | KeyPress(key: char, modifiers: int)
    | Scroll(delta_x: float, delta_y: float)
    | Resize(width: int, height: int);
```

- The first line breaks the visual rhythm — `Click` starts at the indent, the rest
  start after a `| `.
- Reordering variants, adding a variant at the top, or deleting the first variant
  all require editing TWO lines (the moved line AND the new-first line that must
  gain or lose its bar).
- With a leading gutter, every variant line is self-contained:

```ori
// Proposed: leading gutter — every variant line is identical in shape.
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: int)
    | Scroll(delta_x: float, delta_y: float)
    | Resize(width: int, height: int);
```

Now adding, removing, or reordering a variant is a single-line edit — the shape a
formatter should optimize for, and the shape Ori's design principle of dual
optimization for humans and AI-generated code ("atomic line edits") explicitly
targets (`annex-d-formatting.md`).

### When This Matters

- Every sum type with enough variants (or wide enough payloads) to break across
  lines — a common shape for AST/event/message/command enums.
- Diff review: a leading-gutter variant list produces one-line diffs on
  add/remove/reorder instead of two-line diffs.
- Machine generation: emitting one uniform line per variant is simpler and less
  error-prone than special-casing the first.

---

## Goals and Non-Goals

**Goals:**

- Accept an optional leading `|` before the first variant in a sum type body.
- Emit the leading `|` gutter when `ori fmt` breaks a sum type across lines.
- Preserve the format-then-reparse round-trip (Annex D) for both forms.
- Keep every program valid today valid, formatting single-line sum types unchanged.

**Non-Goals:**

- No change to the single-line form: `type E = A | B | C` stays bare when it fits.
- No leading bar for anything other than sum-type variant lists (not match
  or-patterns, not bitwise `|`, not `pre`/`post` contract message separators).
- No configurability — `ori fmt` has exactly one canonical output shape per
  construct, with no user-facing formatting options.
  The leading gutter is the multi-line form, full stop.
- No new semantics — the leading `|` is pure syntax; the produced sum type is
  identical to the bare form.

---

## Design

### Syntax

Grammar production `sum_body` gains an optional leading bar:

```ebnf
(* today *)
sum_body    = variant { "|" variant } .

(* proposed *)
sum_body    = [ "|" ] variant { "|" variant } .
```

Accepted forms (all denote the same type):

```ori
type E = A | B | C;          // bare single-line (unchanged, canonical when it fits)
type E = | A | B | C;        // leading bar single-line (accepted; fmt normalizes to bare)

type E =                     // bare multi-line (accepted; fmt normalizes to leading gutter)
    A
    | B
    | C;

type E =                     // leading-gutter multi-line (canonical multi-line form)
    | A
    | B
    | C;
```

### Semantics

- The leading `|` is syntactic only.
- `[ "|" ] variant { "|" variant }` and `variant { "|" variant }` produce an
  identical `TypeDeclKind::Sum(variants)` — the leading bar consumes no variant
  and carries no meaning.
- No runtime, type, or ARC consequence; this proposal never reaches past the
  parser except through the formatter.

### Formatter rule (`ori_fmt`)

`ori_fmt` normalizes to exactly one canonical shape per fit:

| Fits on one line (≤ width limit) | Broken across lines |
|---|---|
| Bare: `type E = A \| B \| C;` | Leading gutter: first variant prefixed `\| ` like every continuation |

- Single-line: no leading bar (a leading bar on a one-liner is normalized away).
- Multi-line: leading `|` on the first variant, aligned with every continuation `|`
  at the 4-space indent.
- Idempotent: `format(format(x)) == format(x)` for both input shapes.

### Parser (`ori_parse`)

`parse_sum_or_newtype` (`compiler/ori_parse/src/grammar/item/type_decl.rs`) consumes
an optional leading `|` (skipping newlines) before reading the first variant name,
then reuses the existing continuation loop unchanged. A leading `|` with no following
variant is a parse error (E1xxx), same class as a trailing/doubled separator.

### Error Handling

- `type E = | ;` (leading bar, no variant) → parse error: expected a variant after
  `|`.
- `type E = | | A;` (doubled leading bar) → parse error: expected a variant, found
  `|`.
- The newtype/sum disambiguation is unaffected: a leading `|` unambiguously signals a
  sum type (a newtype body is a single type expression, never bar-prefixed).

---

## Drawbacks

- **One more accepted spelling of the same construct.** Ori's design leans on
  "one way to do things"; two *input* spellings (bare vs leading-bar single-line)
  is mild surface growth. Mitigated by the formatter collapsing both to one
  canonical output per fit — the input latitude never produces two committed shapes.
- **Parser must track the leading token for spans.** Adding a leading operator
  invites the exact bug TypeScript filed as
  [microsoft/TypeScript#30995](https://github.com/microsoft/TypeScript/issues/30995)
  — the leading `|` was initially excluded from the union AST node's span. Ori must
  keep the sum-type declaration span covering the leading bar — every lowering step
  must propagate spans to its destination nodes, with no span-free IR nodes outside
  compiler-generated code. This is a known, testable hazard, not an open question.
- **Formatter behavior change on existing multi-line sum types.** Any committed
  `.ori` file with a multi-line sum type reformats (bare first variant → leading
  gutter) on the next `ori fmt`. This is a one-time formatting churn, not a semantic
  change; it is the same class of churn any formatter rule refinement produces.

---

## Alternatives Considered

### Alternative 1: Keep the bare first variant (status quo)

The current formatter emits a bare first variant with continuation bars. Rejected:
it produces two-line diffs on variant add/remove/reorder and breaks the visual
rhythm the leading gutter provides — the readability argument every `|`-using
language's formatter has resolved in favor of the gutter (see Prior Art).

### Alternative 2: Haskell/Elm-style `=`/`|` gutter (first variant introduced by `=`)

Haskell and Elm align the leading token but use `=` for the first variant and `|`
for the rest:

```haskell
data Event
  = Click Int Int
  | KeyPress Char Int
```

Rejected for Ori: Ori's `type E =` already places `=` on the declaration line, so
the first variant sits on its own line under the indent — there is no `=` gutter
slot to fill. A uniform `|` gutter (OCaml/F#/Prettier style) is the natural fit and
keeps every variant line identical.

### Alternative 3: Make the leading bar mandatory multi-line, banned single-line

- Require the leading bar whenever multi-line and forbid it single-line —
  effectively the formatter rule already.
- Making the *grammar* mandate it (vs *accept* it optionally) would reject
  hand-written bare-first-variant multi-line input that `ori fmt` should simply
  normalize.
- Rejected: the grammar should be permissive (accept both), the formatter
  opinionated (emit one).
- Optional-in-grammar + canonical-in-formatter is the same split Ori uses for
  trailing commas.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:**

- Syntax relaxation — it changes what the parser accepts (`grammar.ebnf`
  `sum_body`) and what the formatter emits (`ori_fmt`).
- New/relaxed syntax requires compiler support by definition — under Ori's
  purity principle, new syntax or keywords always require compiler support,
  never a pure-library implementation.
- No library construct can change the grammar.

**Missing features that would enable purity:** none applicable — grammar is not a
library surface.

**Recommendation:** Proceed as a compiler feature. The change is minimal and
localized: one grammar production, one parser call site (`parse_sum_or_newtype`),
one formatter rule (`ori_fmt` sum-type breaking), plus tests. No type-checker,
evaluator, ARC, codegen, or runtime surface is touched.

---

## Spec & Grammar Impact

- **`grammar.ebnf`** — `sum_body` production: `variant { "|" variant }` →
  `[ "|" ] variant { "|" variant }`.
- **Spec Clause 8 (Declarations), Type Definitions** — note the optional leading bar
  and its normalization (bare when single-line, leading gutter when multi-line).
- **`annex-d-formatting.md`** — record the multi-line sum-type canonical shape
  (leading `|` gutter) and the single-line canonical shape (bare).
- No operator-rules.md change — the leading `|` is a declaration-syntax token, not
  an operator, and does not enter the precedence table.

---

## Prior Art

Surveyed across languages that use `|` for sum/union types, the
leading-gutter form is the dominant formatter convention; several accept an optional
leading bar in the grammar itself.

| Language | Leading bar in grammar? | Formatter emits leading bar on first variant? |
|---|---|---|
| **OCaml** | Yes — optional `[ "\|" ]` before the first constructor | Yes — `ocamlformat` default |
| **F#** | Yes — leading `\|` idiomatic on discriminated unions | Yes — `Fantomas` |
| **TypeScript** (union types) | Yes — accepted since [microsoft/TypeScript#12071](https://github.com/microsoft/TypeScript/issues/12071) (closed:completed), implemented in [#12386](https://github.com/microsoft/TypeScript/pull/12386) | Yes — Prettier emits leading `\|` on every member when a union breaks multi-line |
| **Haskell / Elm** | No — first constructor introduced by `=` | Aligned `=`/`\|` gutter (Alternative 2) |
| **Rust / Swift / Scala / Gleam / Zig** | N/A — brace-delimited, comma/newline-separated, no `\|` | N/A |

Implementation lesson from the graph — [microsoft/TypeScript#30995](https://github.com/microsoft/TypeScript/issues/30995)
("Leading |/& is not included in the intersection/union node"): TypeScript's initial
leading-operator support excluded the leading token from the union AST node's span,
breaking span-dependent tooling. Ori must keep the sum-type span covering the leading
`|` — captured as a Drawback + a test obligation above.

---

## Unresolved Questions

- **Resolve during review:** Should a hand-written bare-first-variant *multi-line*
  sum type be accepted-and-normalized (proposed), or rejected as non-canonical input?
  The proposal takes the permissive-grammar / opinionated-formatter position; confirm
  this matches the trailing-comma precedent's spirit.
- **Resolve during implementation:** Exact span coverage of the leading `|` in the
  `TypeDeclKind::Sum` node and its first variant — pin with a parser property test
  guarding against the TS#30995 regression class.
- **Out of scope:** Any leading-separator relaxation for match or-patterns (`P1 | P2`
  in match arms) — a separate surface with its own disambiguation; not addressed here.
