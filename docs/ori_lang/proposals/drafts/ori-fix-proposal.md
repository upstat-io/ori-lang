# Proposal: `ori fix` — Apply-Driver for Machine-Applicable Suggestions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-26
**Affects:** Compiler driver (`oric` — new `fix` subcommand), `ori_diagnostic` (suggestion serialization), `ori_test_harness` (producer-correctness corpus gate), tooling, spec (Annex D interaction; CLI surface), guide
**Related:** block-tail-value-discipline-proposal.md (a consumer; routes its void-tail `;` normalization + `Never`-tail spelling through `ori fix`), built-in-lint-format-on-compile-proposal.md, redundant-trailing-unit-normalization-proposal.md

---

## Summary

`ori fix` is a new compiler-driver subcommand that applies the compiler's existing **machine-applicable** suggestions to source files. Ori already produces structured suggestions (`Applicability::MachineApplicable` + `structured_suggestions: Vec<Suggestion>` in `ori_diagnostic`), but there is no driver that applies them. `ori fix` reads a program, type-checks it, collects every `MachineApplicable` suggestion, applies them per-suggestion atomically, **re-type-checks the rewritten file and rolls it back if new diagnostics appear**, and iterates to a bounded fixpoint — mirroring `cargo fix`/`rustfix` in full, not just its overlap-skip. It is the apply-half of the suggestion infrastructure, which the codebase already names `ori fix` as the intended consumer of.

`ori fix` is cross-cutting: it serves **every** diagnostic that carries a machine-applicable suggestion, not any single feature. It is proposed on its own (rather than bundled into a consumer proposal) so each consumer declares a `Depends On:` edge instead of re-specifying an apply-driver.

---

## Motivation

### The Problem in Practice

The compiler builds structured, machine-applicable suggestions but cannot apply them. The data + the applicability mark exist (`Applicability::MachineApplicable`, `Suggestion { substitutions: Vec<Substitution>, applicability }`), and the codebase already names `ori fix` as the intended consumer — but no driver applies them:

```
error[Exxxx]: <a machine-applicable diagnostic>
  --> a.ori:3:18
   |
 3 | <offending source>
   |                  ^ <suggested edit>
   = suggestion (machine-applicable): <the fix>
```

The `suggestion (machine-applicable)` is computed and serialized, then has no consumer — a human applies it by hand.

**Honest state of the producer surface.** Today `Applicability::MachineApplicable` is constructed at exactly **one** production site — `Suggestion::machine_applicable`, called only from the lexer confusable/typo path (`oric/src/problem/lex.rs`). `ori_types` reporting forwards suggestions but does not yet mark any `MachineApplicable`. So the apply infrastructure exists, but the **producer surface is nascent (one lexer path today)** — this proposal is NOT "stranded data piling up"; it is "build the apply-half so future producer investment (e.g. the block-tail `;` normalization) is worthwhile and safe." The driver and the producer surface grow together.

### When This Matters

- **Any future diagnostic with a deterministic fix** — `;` insertion/removal, import normalization, deprecated-form rewrites, redundant-token deletion. The producer marks the suggestion `MachineApplicable`; `ori fix` applies it safely.
- **Migrations.** When a proposal tightens a canonical form (e.g. the void-tail `;` normalization in `block-tail-value-discipline-proposal.md`), a one-shot `ori fix` over a tree applies the new canonical form mechanically.
- **AI-generated code.** A generator that emits a non-canonical-but-legal form is normalized by `ori fix` to the canonical shape.
- **Consumers needing a type-dependent rewrite.** A normalization that needs the tail/callee type cannot live in the parse-only `ori fmt`; it needs a post-type-check driver — `ori fix`.

### Why not `ori fmt`

`ori fmt` is parse-only and type-free by contract (one canonical shape from syntax alone). A type-dependent rewrite cannot run there. `ori fix` runs *after* type-checking. The canonical pipeline is parse → type-check → `ori fix` (type-dependent + suggestion-apply, with re-check) → `ori fmt` (syntactic formatting).

---

## Goals and Non-Goals

**Goals:**

- Provide `ori fix [paths...]` that applies every `MachineApplicable` suggestion the compiler emits, post-type-check, with a **post-apply re-check + rollback** correctness guard.
- Converge to a **bounded fixpoint** (apply → re-check → re-collect → repeat, up to a max-iteration bound, with cycle detection) so interacting producers reach a stable state.
- Be atomic at **suggestion** granularity (a `Suggestion`'s `Vec<Substitution>` is all-or-none) AND at **file** granularity (a file is fully rewritten + verified, or rolled back).
- Apply only `MachineApplicable` suggestions; never a suggestion needing human judgment.
- Enforce producer correctness via a spec-corpus gate (every emitted `MachineApplicable` suggestion, applied, must still parse + type-check).
- Compose cleanly with `ori fmt` (canonical order: `ori fix` then `ori fmt`).

**Non-Goals:**

- NOT a linter or new diagnostic source — `ori fix` applies suggestions other phases produce; it adds no checks.
- NOT a refactoring engine — no rename/extract/semantic transform beyond applying emitted suggestions.
- NOT `ori fmt` — formatting stays parse-only in the formatter.
- NOT a replacement for the proposal gate — a suggestion that changes spec-defined behavior still routes through proposals.

---

## Design

### CLI Surface

```
ori fix [paths...] [--check] [--diff] [--max-iterations=N] [--suggestion-class=machine-applicable]
```

- `paths...` — files or directories to fix (default: the current package).
- `--check` — report what would change, write nothing; exit code per §Exit codes (distinguishes would-modify from producer-conflict).
- `--diff` — print the unified diff instead of writing files.
- `--max-iterations=N` — fixpoint iteration bound (default per the runtime; cycle reported if hit).
- `--suggestion-class=machine-applicable` (default + only value initially) — applicability filter; reserved for a future `--suggestion-class=maybe-incorrect` opt-in, out of scope here.

### Suggestion-application protocol (per file, iterated to fixpoint)

1. Parse + type-check the file through the normal front-end (same as `ori check`); record the baseline diagnostic set.
2. Collect every emitted suggestion whose applicability is `MachineApplicable`. A `Suggestion` carries `substitutions: Vec<Substitution>` (an **atomic edit-set**, e.g. an import rename touching both declaration and use).
3. **In-file invariant:** every `Substitution.span` MUST resolve to the file under rewrite (`Substitution.span` is bare byte offsets with no file identity). Any suggestion with a span that does not resolve to this file is **skipped and reported** — never applied at this file's offsets.
4. **Per-suggestion conflict resolution:** if ANY substitution of a suggestion overlaps ANY substitution of another suggestion, drop the **whole** conflicting suggestion(s) (preserving each suggestion's all-or-none semantics) and report the conflict. Never apply a partial suggestion.
5. Apply the surviving suggestions' edits to the source buffer in one pass (highest offset first).
6. **Post-apply verification (the correctness guard):** re-parse + re-type-check the rewritten buffer. If it now fails to compile OR emits any diagnostic absent from the step-1 baseline, **roll back the whole file** (discard the buffer) and report. Only a clean re-check commits the buffer.
7. **Fixpoint iteration:** if the committed buffer still emits `MachineApplicable` suggestions, repeat from step 2, up to `--max-iterations`. If the bound is hit without convergence, roll back to the last stable state and report a non-convergence (cycle) for the file.
8. Write the converged buffer atomically per file (temp file + rename), or emit diff / check-result per flag.

### Convergence (replaces a bare idempotence assertion)

`ori fix` converges to a **bounded fixpoint**, matching `cargo fix`. Single-pass idempotence is NOT assumed: a producer whose applied edit re-triggers a different `MachineApplicable` diagnostic is handled by iteration (step 7). Convergence is the driver's contract; the producer obligation it rests on (a suggestion should not re-fire on its own applied output) is enforced by the §Producer-correctness gate, and non-convergence within the iteration bound is reported (cycle), never silently looped. `--check` reports the converged result, so it is a usable CI gate (it does not report "dirty forever" under interacting producers).

### Producer-correctness gate

Once `ori fix` auto-applies, every `MachineApplicable` mark is a load-bearing correctness contract — a mis-mark silently rewrites source. Enforcement: a spec-corpus test (in `ori_test_harness`) that, for every emitted `MachineApplicable` suggestion in the corpus, applies it and asserts the result still parses + type-checks (the post-apply contract, enforced at test time). A producer that emits a suggestion failing this gate is a compiler bug caught in CI, not at a user's `ori fix` invocation.

### Batching behavior

- **Suggestion atomicity:** all-or-none per suggestion (step 4).
- **File atomicity:** a file is fully rewritten + post-apply-verified or rolled back (step 6); no partial writes.
- **Single-file suggestion constraint:** a `MachineApplicable` suggestion's substitutions MUST all target one file (today's producers are single-file; the constraint is stated so a future cross-file suggestion does not silently half-apply across the tree). A cross-file suggestion is skipped + reported until suggestion-level cross-file atomicity is designed.
- **Cross-file independence:** files are fixed independently; one file's conflict/rollback does not block another's fix. A multi-file migration can therefore leave a partial-tree result (some files fixed, some skipped-on-conflict); the run reports it, and it is the user's signal to fix the reported producer conflict and re-run.
- A run reports: files fixed, files unchanged, files rolled-back (post-apply re-check failed), files skipped-on-conflict (with spans), files non-converged (cycle).

### Interaction with `ori fmt`

- Canonical pipeline: parse → type-check → `ori fix` → `ori fmt`.
- `ori fix` makes semantic-form edits (apply a suggestion, type-dependent); `ori fmt` makes whitespace/layout edits (parse-only). `ori fmt` runs after, re-flowing any layout the edits disturbed.
- **One-home invariant:** each normalization registers in exactly ONE home — `ori fix` if it needs a resolved type, `ori fmt` otherwise. The two are ordered (fix precedes fmt) and partitioned by the needs-a-resolved-type rule; they operate on adjacent syntactic surfaces (e.g. block-tail's `()` deletion in `ori fmt` vs `;` insertion in `ori fix`), so the partition is by the home rule, not "disjoint by construction." A normalization registered in both homes is a bug the one-home invariant forbids.

### Exit codes

- `0` — nothing to change (converged / clean).
- `1` — would-modify (under `--check`) OR modified (without `--check`).
- `2` — producer-conflict or non-convergence or post-apply rollback (a compiler producer-bug surfaced; distinct from benign drift so CI can route the two differently).

### Error Handling

- A file that fails to parse or type-check at step 1 is reported and left untouched.
- Post-apply re-check failure → whole-file rollback + report (step 6).
- Overlapping suggestions → whole-suggestion skip + report (step 4).
- No new error codes for the user's program — `ori fix` consumes existing diagnostics; its operational outcomes are tool-level messages + the exit codes above.

---

## Drawbacks

- **New driver surface.** A new subcommand with CLI, fixpoint loop, post-apply re-check, and atomic-write machinery — justified by the otherwise-unusable suggestion infrastructure.
- **Producer discipline is load-bearing.** `ori fix` is only as safe as the `MachineApplicable` markings. A mis-marked suggestion that survives the post-apply re-check (i.e. the wrong edit still type-checks but changes semantics) would be applied. The post-apply re-check + the producer-correctness gate bound this far better than an applicability filter alone, but they do not catch a wrong-yet-still-type-checking edit; producers MUST mark conservatively. (This is the honest residual — the post-apply guard catches edits that break compilation, not edits that compile-but-mean-something-else.)
- **Two normalization homes.** `ori fmt` (syntactic) vs `ori fix` (type-dependent). The one-home invariant + the fix→fmt ordering make this tractable, but a contributor must know which home a new normalization belongs to.
- **Fixpoint cost.** Iterating to convergence re-type-checks per iteration; bounded by `--max-iterations`. The common case converges in one iteration.

---

## Alternatives Considered

### Alternative 1: Apply suggestions inside `ori fmt`

Rejected: `ori fmt` is parse-only and type-free by contract; type-dependent suggestions cannot run there without giving the formatter a type-checker dependency the formatter's design forbids.

### Alternative 2: Per-feature mini-fixers

Rejected — SRP/DRY: every feature would re-implement span-application, atomic write, post-apply re-check, fixpoint, and conflict handling. The apply-driver is one capability; centralize it.

### Alternative 3: Apply-only, no post-apply re-check (the first draft)

Rely on the applicability filter + overlap-skip alone. Rejected after review: that defends only against *overlapping* edits, while the dominant apply-driver failure mode is a single *non-overlapping* wrong-but-machine-applicable edit (rust#141082 "use ::item incorrect for editions 2018+", rust#147303, rust#120838). `cargo fix`/`rustfix` defend this with apply→recompile→revert-on-new-errors + iterate-to-fixpoint; omitting that half and trusting the applicability mark is circular. The post-apply re-check + rollback + fixpoint is now core protocol, not an optional gate.

### Alternative 4: Do nothing (suggestions stay advisory)

Rejected: it leaves infrastructure the compiler pays to compute unusable, and blocks consumers (block-tail's void-tail normalization) that require a post-type-check apply-driver.

---

## Purity Analysis

**Can be pure Ori?** NO.
**If not, why:** Applying compiler-emitted suggestions requires the front-end (parse + type-check) + the structured-suggestion data in `ori_diagnostic` / `ori_types`. It is a driver capability, not a library feature.
**Missing features that would enable purity:** None — suggestion application is inherently a toolchain concern.
**Recommendation:** Proceed as a compiler-driver (`oric`) subcommand. Lean: one new subcommand consuming existing suggestion data + a corpus gate; no new diagnostics, no grammar change, no new keyword. Matches the `cargo fix`/`rustfix` split.

---

## Spec & Grammar Impact

- **Grammar:** UNCHANGED — `ori fix` rewrites source through existing suggestions; no syntax.
- **Annex D (`annex-d-formatting.md`):** Note the `ori fix` → `ori fmt` pipeline ordering + the one-home invariant so consumers place a normalization in the correct home.
- **CLI / driver docs:** Document the `fix` subcommand, flags, the post-apply-re-check + bounded-fixpoint + atomicity contract, exit codes, the `MachineApplicable`-only default.
- **`ori_test_harness`:** Add the producer-correctness corpus gate (apply every emitted `MachineApplicable` suggestion; assert the result parses + type-checks).
- **Error codes:** None — `ori fix` consumes existing diagnostics; tool-level outcomes use exit codes, not user-program error codes.

---

## Prior Art

- **Rust — `cargo fix` / `rustfix`** — the direct model, adopted in FULL: `rustc` emits `Applicability::MachineApplicable` structured suggestions; `rustfix` (1) skips overlapping machine-applicable edits, (2) **recompiles after applying and reverts the batch when new errors surface**, and (3) **applies iteratively to a fixpoint** across passes. This proposal adopts all three (overlap-skip → per-suggestion conflict-skip; recompile-revert → post-apply re-check + rollback; iterate-to-fixpoint → bounded fixpoint). The dominant failure mode — a single non-overlapping wrong machine-applicable edit (rust#141082, rust#147303, rust#120838) — is defended by mechanism (2), not the applicability filter.
- **Rust — `clippy --fix`** — applies clippy's machine-applicable suggestions through the same `rustfix` machinery; one apply-driver serves many producers.
- **Go — `gofmt -r` / `go fix`** — toolchain-grade mechanical rewriting; `ori fix` is its type-dependent sibling.
- **Idempotence / convergence + atomic write** — `gofmt` / `rustfmt` guarantee idempotent atomic rewriting; `cargo fix` guarantees bounded-fixpoint convergence with revert-on-error. `ori fix` adopts the `cargo fix` convergence model (not bare single-pass idempotence) because its producer set is open.

---

## Unresolved Questions

- **`--suggestion-class=maybe-incorrect` opt-in:** whether to ever apply non-`MachineApplicable` suggestions under an explicit opt-in (with review), or stay machine-applicable-only. Recommendation: machine-applicable-only for the first version; defer the opt-in.
- **Format-on-compile integration:** if `built-in-lint-format-on-compile-proposal.md` is approved, whether `ori fix`'s machine-applicable normalizations run automatically as part of that phase, or stay an explicit `ori fix` invocation. Resolve jointly with that proposal.
- **Cross-file suggestion atomicity:** the first version constrains `MachineApplicable` suggestions to single-file (skip+report cross-file). Designing suggestion-level cross-file atomicity (all-or-none across multiple files) is deferred until a real cross-file producer exists.
