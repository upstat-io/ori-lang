# Proposal: `ori fix` — Apply-Driver for Machine-Applicable Suggestions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-26
**Affects:** Compiler driver (`oric` — new `fix` subcommand), `ori_diagnostic` (suggestion serialization), tooling, spec (Annex D interaction; CLI surface), guide
**Related:** block-tail-value-discipline-proposal.md (a consumer; routes its void-tail `;` normalization + `Never`-tail spelling through `ori fix`), built-in-lint-format-on-compile-proposal.md, redundant-trailing-unit-normalization-proposal.md

---

## Summary

`ori fix` is a new compiler-driver subcommand that applies the compiler's existing **machine-applicable** suggestions to source files. Ori already produces structured suggestions (`MachineApplicable` / `structured_suggestions` in `ori_diagnostic` / `ori_types`), but there is no driver that applies them — they are emitted and then ignored. `ori fix` reads a program, type-checks it, collects every `MachineApplicable` suggestion, and rewrites the source so the suggested edits are applied, idempotently and atomically per file. It is the general-purpose apply-half of the suggestion infrastructure, the way `cargo fix` is the apply-half of `rustc`'s suggestions.

`ori fix` is cross-cutting: it serves **every** diagnostic that carries a machine-applicable suggestion, not any single feature. It is proposed on its own (rather than bundled into a consumer proposal) so each consumer declares a `Depends On:` edge instead of re-specifying an apply-driver.

---

## Motivation

### The Problem in Practice

The compiler already builds structured, machine-applicable suggestions but cannot apply them:

```
error[E2034]: this `;` discards the block's value, but `f` must produce `int`
  --> a.ori:3:18
   |
 3 |     compute() + 1;
   |                  ^ remove this `;` to make this expression the return value
   = suggestion (machine-applicable): delete `;`
```

The `suggestion (machine-applicable)` is computed, serialized, and then has no consumer — a human must apply it by hand. Every diagnostic with a fix the compiler already knows how to make is a missed automation: the data exists, the driver does not.

### When This Matters

- **Any diagnostic with a deterministic fix** — `;` insertion/removal, import normalization, deprecated-form rewrites, redundant-token deletion. The fix is already computed; only the apply-driver is missing.
- **Migrations.** When a proposal tightens a canonical form (e.g. the void-tail `;` normalization in `block-tail-value-discipline-proposal.md`), a one-shot `ori fix` over a tree applies the new canonical form mechanically instead of by hand.
- **AI-generated code.** A generator that emits a non-canonical-but-legal form is normalized by `ori fix` to the canonical shape, keeping checked-in source uniform regardless of what produced it (the stated goal of `built-in-lint-format-on-compile-proposal.md`).
- **Consumers that need a type-dependent rewrite.** Some normalizations need the tail/callee type (e.g. insert `;` only on a *void* tail), so they cannot live in the parse-only `ori fmt`. They need a post-type-check driver — `ori fix`.

### Why not `ori fmt`

`ori fmt` is parse-only and type-free by contract (one canonical shape from syntax alone). A type-dependent rewrite (apply a suggestion that depends on an expression's resolved type) cannot run there. `ori fix` runs *after* type-checking, so it can apply suggestions the type checker computed. The two compose: `ori fix` applies type-dependent suggestions, then `ori fmt` applies syntactic formatting.

---

## Goals and Non-Goals

**Goals:**

- Provide `ori fix [paths...]` that applies every `MachineApplicable` suggestion the compiler emits for the given source, post-type-check.
- Be idempotent: running `ori fix` on already-fixed source produces byte-identical output (no edit on the second pass).
- Be atomic per file: a file is either fully rewritten with all its applicable suggestions or left untouched (no partial writes).
- Apply only `MachineApplicable` suggestions by default; never apply a suggestion the compiler marked as needing human judgment.
- Compose cleanly with `ori fmt` (run `ori fix` then `ori fmt`; the canonical pipeline is parse → type-check → `ori fix` → `ori fmt`).

**Non-Goals:**

- NOT a linter or a new diagnostic source — `ori fix` applies suggestions other phases already produce; it adds no new checks.
- NOT a refactoring engine — no rename, no extract, no semantic transformation beyond applying emitted suggestions.
- NOT `ori fmt` — formatting stays in the parse-only formatter; `ori fix` is the type-dependent / suggestion-apply half.
- NOT a replacement for the proposal gate — a suggestion that changes spec-defined behavior still routes through proposals; `ori fix` only mechanizes already-sanctioned edits.

---

## Design

### CLI Surface

```
ori fix [paths...] [--check] [--suggestion-class=machine-applicable] [--diff]
```

- `paths...` — files or directories to fix (default: the current package).
- `--check` — report what would change, exit non-zero if any file would be modified, write nothing (CI gate).
- `--diff` — print the unified diff instead of writing files.
- `--suggestion-class=machine-applicable` (default and only value initially) — the applicability filter; reserved for a future `--suggestion-class=maybe-incorrect` opt-in that is out of scope here.

### Suggestion-application protocol

1. Parse + type-check each input file through the normal pipeline (same front-end as `ori check`).
2. Collect every emitted suggestion whose applicability is `MachineApplicable`. Each suggestion is a set of `(span, replacement)` edits (the existing `structured_suggestions` shape in `ori_diagnostic`).
3. Sort edits by span; reject (skip with a reported conflict) any two `MachineApplicable` edits whose spans overlap — overlapping machine-applicable edits indicate a producer bug, not an apply-time choice.
4. Apply non-overlapping edits to the source buffer in a single pass (highest offset first, so earlier offsets stay valid).
5. Write the rewritten buffer atomically per file (temp file + rename), or emit diff / check-result per flag.

### Idempotence

`ori fix` is idempotent: after one pass, the applied suggestions no longer fire (the source is already in the suggested form), so a second pass collects zero edits and writes nothing. This is the same idempotence contract `ori fmt` carries; `ori fix --check` on already-fixed source exits zero.

### Batching behavior

- Per-file atomicity: each file's edits are applied as one atomic write; a file with a conflict (overlapping machine-applicable edits) is left untouched and reported, not partially written.
- Cross-file independence: files are fixed independently; one file's conflict does not block another's fix.
- A run reports: files fixed, files unchanged, files skipped-on-conflict (with the conflicting spans).

### Interaction with `ori fmt`

- Canonical pipeline: parse → type-check → `ori fix` (type-dependent + suggestion-apply edits) → `ori fmt` (syntactic formatting).
- `ori fix` makes semantic-form edits (apply a suggestion); `ori fmt` makes whitespace/layout edits. Running `ori fmt` after `ori fix` re-flows any layout the edits disturbed.
- A consumer normalization that is purely syntactic belongs in `ori fmt`; one that needs a resolved type belongs in `ori fix`. The two never apply the same edit (no double-apply) because their inputs are disjoint by construction (type-dependent vs parse-only).

### Error Handling

- A file that fails to parse or type-check is reported and left untouched — `ori fix` never applies suggestions to a file the front-end rejected.
- Overlapping `MachineApplicable` edits in one file → the file is skipped, the conflict reported (a producer bug to fix, surfaced not silently swallowed).
- No new error codes for the user's program — `ori fix` consumes existing diagnostics; its own operational failures (IO, conflict) are tool-level messages.

---

## Drawbacks

- **New driver surface.** `ori fix` is a new subcommand with its own CLI, flags, and atomic-write machinery — real surface to maintain, justified by the otherwise-stranded suggestion infrastructure.
- **Producer discipline.** `ori fix` is only as good as the `MachineApplicable` markings; a suggestion mis-marked machine-applicable would be applied wrongly. The conflict-skip + the applicability filter bound this, but it puts a correctness obligation on every suggestion producer.
- **Two normalization homes.** Some normalizations live in `ori fmt` (syntactic) and some in `ori fix` (type-dependent). A contributor must know which home a new normalization belongs to; the rule (needs a resolved type? → `ori fix`) is simple but is a thing to know.

---

## Alternatives Considered

### Alternative 1: Apply suggestions inside `ori fmt`

Make the formatter apply machine-applicable suggestions. Rejected: `ori fmt` is parse-only and type-free by contract; type-dependent suggestions cannot run there without giving the formatter a type-checker dependency, which the formatter's design forbids.

### Alternative 2: Per-feature mini-fixers

Let each feature (block-tail, redundant-`()`, etc.) ship its own apply logic. Rejected — SRP / DRY: every feature would re-implement span-application, atomic write, idempotence, and conflict handling. The apply-driver is one capability; centralize it.

### Alternative 3: Do nothing (suggestions stay advisory)

Leave suggestions human-applied. Rejected: it strands infrastructure the compiler already pays to compute, and it blocks consumers (e.g. the block-tail void-tail normalization) that require a post-type-check apply-driver.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** Applying compiler-emitted suggestions requires the compiler's front-end (parse + type-check) and access to the structured-suggestion data in `ori_diagnostic` / `ori_types`. It is a driver capability, not a library feature.
**Missing features that would enable purity:** None — suggestion application is inherently a toolchain concern.
**Recommendation:** Proceed as a compiler-driver (`oric`) subcommand. Lean change: one new subcommand consuming existing suggestion data; no new diagnostics, no grammar change, no new keyword.

---

## Spec & Grammar Impact

- **Grammar:** UNCHANGED — `ori fix` rewrites source through existing suggestions; it introduces no syntax.
- **Annex D (`annex-d-formatting.md`):** Note the `ori fix` → `ori fmt` pipeline ordering (type-dependent suggestions, then syntactic formatting) so consumers place a normalization in the correct home.
- **CLI / driver docs:** Document the `fix` subcommand, flags, idempotence + atomicity contract, and the `MachineApplicable`-only default.
- **Error codes:** None — `ori fix` consumes existing diagnostics; it allocates no user-program error codes.

---

## Prior Art

- **Rust — `cargo fix` / `rustfix`** — the direct model: `rustc` emits `Applicability::MachineApplicable` structured suggestions; `rustfix` applies them; `cargo fix` drives it over a crate. The exact split this proposal mirrors (compiler computes the suggestion; a separate driver applies it). `rustfix` skips overlapping machine-applicable edits — the precedent for this proposal's conflict-skip rule.
- **Rust — `clippy --fix`** — applies clippy's machine-applicable lint suggestions through the same `rustfix` machinery; evidence that one apply-driver serves many suggestion producers.
- **Go — `gofmt -r` / `go fix`** — `go fix` applies known API-migration rewrites; `gofmt -r` applies rewrite rules. Toolchain-grade mechanical rewriting, the model Ori's formatter already follows; `ori fix` is its type-dependent sibling.
- **Idempotence + atomic write** — `gofmt` / `rustfmt` both guarantee idempotent, atomic-per-file rewriting; `ori fix` adopts the same contract.

---

## Unresolved Questions

- **`--suggestion-class=maybe-incorrect` opt-in:** whether to ever apply non-`MachineApplicable` suggestions under an explicit opt-in (with review), or keep `ori fix` machine-applicable-only forever. Recommendation: machine-applicable-only for the first version; defer the opt-in.
- **Format-on-compile integration:** if `built-in-lint-format-on-compile-proposal.md` is approved, whether `ori fix`'s machine-applicable normalizations run automatically as part of that phase, or stay an explicit `ori fix` invocation. Resolve jointly with that proposal.
- **Conflict surfacing:** whether an overlapping-machine-applicable-edits conflict is a hard tool error (exit non-zero) or a per-file skip-and-report. Recommendation: skip-and-report per file, non-zero overall exit so CI notices the producer bug.
