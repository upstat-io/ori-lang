# Proposal: Single Semantic Authority for Developer Tooling

**Status:** Draft
**Author:** Eric
**Created:** 2026-06-28
**Affects:** Tooling, compiler infrastructure, developer experience, spec governance
**Amends:** lsp-implementation-proposal.md

---

## Summary

Ori's developer tooling — the language server (LSP), linters, and any
semantic (non-syntactic) formatter or editor-facing analysis — MUST BE the
same compiler engine, never a separate reimplementation. The compiler
frontend (lex → parse → typecheck → canonicalize) is the sole authority for
program meaning: name resolution, type inference, trait and operator
resolution, and pattern compilation. Tooling consumes that engine's output;
it never re-derives meaning.

This amendment elevates an architecture decision the approved LSP proposal
already made — reuse the existing Salsa `CompilerDb`, do not fork or wrap
rust-analyzer — into a named, governed invariant that binds *all* current
and future tooling, not just the LSP.

---

## Motivation

The approved `lsp-implementation-proposal.md` chose the right architecture
(reuse `CompilerDb`; reject the rust-analyzer-as-foundation alternative) but
recorded it as a per-feature design decision. As Ori grows additional tooling
— linters, a semantic formatter pass, MCP semantic tooling, editor code
actions — each one re-faces the same fork: reuse the compiler, or stand up a
parallel analyzer. Without a binding invariant, one of them eventually drifts.

The drift has a name. In the Rust ecosystem, `rust-analyzer` is a separate,
from-scratch reimplementation of the type system, distinct from `rustc`. The
two independently re-derive name resolution, trait solving, and inference, and
they routinely disagree: the editor confidently reports an error the build does
not have (and vice versa). The authoritative answer ("does this field exist?
does this trait resolve?") has two implementations, and only one is normative.

The proven alternative is a single shared engine. C#'s Roslyn is
"compiler-as-a-service": the IDE, the analyzers, and `csc` are the same
compiler. TypeScript's `tsserver` *is* `tsc` in watch mode. In both, the editor
and the build are structurally incapable of disagreeing about whether a
symbol, field, or type exists, because exactly one engine computes it.

### Why this matters more for Ori than for prior languages

Ori is designed AI-written-first: the expectation is that language models will
author the majority of Ori code. The editor/LSP feedback loop is therefore not
a human convenience — it is the primary signal an automated author acts on. A
human who sees a stale or wrong diagnostic shrugs and rebuilds; an LLM acting
on a stale or wrong diagnostic propagates the mistake at scale, edits around a
phantom error, or "fixes" code that was already correct. A feedback surface
that can drift from the compiler is a correctness hazard for the whole
premise, not a UX papercut.

The principle is therefore promoted to a load-bearing invariant rather than a
single feature's design note.

---

## Goals and Non-Goals

**Goals:**

- Name the invariant: one engine computes program meaning; tooling consumes it.
- Bind it to ALL tooling (LSP, linters, semantic formatter, future
  editor-facing analysis), not only the LSP.
- Make divergence — a second meaning-deriver — a governance-level violation,
  not a per-feature judgement call.

**Non-Goals:**

- Not a redesign of the LSP — the approved `lsp-implementation-proposal.md`
  architecture (reuse `CompilerDb`, Salsa-incremental, rust-analyzer rejected)
  stands unchanged; this names and generalizes it.
- Not a mandate to build any specific tool now — it constrains HOW tooling is
  built whenever it is built.
- Does not constrain purely-syntactic tooling that needs no semantic
  information (e.g. the `ori fmt` syntactic layer that operates on parse
  output only — that already consumes the single frontend's parse stage and
  derives no meaning of its own).

---

## Design

### The invariant

The frontend (lex → parse → typecheck → canonicalize) is the SOLE source of
program meaning. No consumer of frontend output re-derives that meaning:

- not the evaluator,
- not LLVM codegen,
- not any new backend,
- not any developer tooling (language server, linter, semantic formatter,
  editor analysis).

Every consumer reads the shared engine output (`CanExpr`, the decision-tree
pool, the resolved type pool). The LSP is real-time and never-stale because it
*is* the compiler running incrementally via the existing Salsa `CompilerDb`,
not a parallel analyzer with its own caches.

### Two faces of one invariant

- **Runtime face** — dual-execution parity: the evaluator and LLVM codegen
  produce identical observable behavior (existing invariant, `missions.md`
  §ori_eval).
- **Upstream face** — single semantic authority: one engine computes meaning;
  every consumer, including tooling, trusts it.

This proposal makes the upstream face explicit and extends it to tooling as a
co-equal consumer of the one frontend.

### North-star acceptance gate

An Ori editor diagnostic and `ori check` can never disagree about the
existence or type of any symbol, because exactly one engine computes it. Any
tool that can produce a diagnostic the build does not have (or miss one the
build has) violates the invariant.

### Already-landed enforcement (compiler-internal)

The invariant is already guarded for the existing backends and any future
consumer:

- `canon.md §7.5 Single Semantic Authority` — the load-bearing invariant.
- `impl-hygiene.md PHASE-54` — the reviewer-actionable hygiene rule.
- `finding-categories.md LEAK:frontend-reimplementation` (Critical) — a
  close-blocking reviewer finding when a consumer re-derives meaning.

This proposal is the language-design / governance commitment those guards
serve: the LSP and all future tooling follow this architecture by rule.

---

## Drawbacks

- Coupling tooling to the compiler's internal query surface means tooling
  evolves in lockstep with `CompilerDb` query shapes; a frontend refactor can
  ripple into tooling. This is the intended trade — lockstep is the mechanism
  that prevents drift — but it raises the cost of frontend churn.
- A single engine must serve both batch (build) and latency-sensitive
  (editor) workloads; incremental responsiveness becomes a compiler concern,
  not a separable tooling concern. The approved LSP proposal already accepts
  this via Salsa.
- Forecloses adopting an off-the-shelf analyzer (e.g. wrapping rust-analyzer
  machinery) as a shortcut — by design.

---

## Alternatives Considered

### Alternative 1: Leave it as the LSP proposal's per-feature decision

Rejected: a per-feature decision does not bind the next tool. Linters, the
semantic formatter, and MCP tooling each re-face the fork; one eventually
drifts. The invariant exists to remove the per-feature judgement.

### Alternative 2: Separate analyzer with a parity test against the compiler

Rejected: this is the rust-analyzer model with a bolt-on check. Two engines
plus a parity harness is strictly more surface than one engine, and the parity
check only catches drift after it ships. Single authority makes drift
unrepresentable rather than detectable.

---

## Purity Analysis

**Can be pure Ori?** N/A — this is a compiler/tooling architecture invariant,
not a language-surface feature. It requests no new syntax, no stdlib addition,
and no compiler feature beyond the architectural constraint itself.
**Recommendation:** Proceed as a governance/architecture amendment to the
approved LSP proposal.

---

## Spec & Grammar Impact

- No grammar changes.
- No surface-language clause changes.
- Governance/architecture commitment recorded against the tooling subsystem;
  the compiler-internal invariant already lives in `canon.md §7.5` (rules
  layer), which this proposal cites as the spec-adjacent enforcement home.

---

## Prior Art

- **C# / Roslyn** — "compiler-as-a-service": the IDE, analyzers, and `csc`
  share one compiler. Editor and build cannot disagree about symbol/type
  existence because there is one type checker.
- **TypeScript / tsserver** — the language service *is* `tsc`; the editor runs
  the same checker as the build. Structurally drift-free for existence/type
  questions.
- **Rust / rust-analyzer vs rustc** — the anti-pattern: a separate, from-scratch
  reimplementation of the type system. The two disagree in practice (editor
  reports errors the build lacks, and vice versa); long-running effort to share
  a trait solver / library-ify the frontend is ongoing precisely because the
  split was costly.
- **Ori** — the approved `lsp-implementation-proposal.md` already chose the
  Roslyn/tsserver model (reuse `CompilerDb`, reject rust-analyzer foundation);
  this proposal names the principle behind that choice and binds it to all
  tooling.

---

## Unresolved Questions

- Does the syntactic `ori fmt` layer need an explicit carve-out clause, or is
  "derives no meaning" a sufficient boundary? (Resolves during review.)
- Should MCP semantic tooling (`mcp-semantic-tooling-proposal.md`, draft) be
  named as an explicit consumer of this invariant, or does the general
  "all tooling" binding suffice? (Resolves during review / when that draft
  advances.)
- The incremental-latency budget for the shared engine under editor workloads
  is an implementation concern deferred to whoever builds each tool, not
  resolved here.
