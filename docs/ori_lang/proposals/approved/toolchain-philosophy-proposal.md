# Proposal: Toolchain Philosophy

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Approved:** 2026-07-21
**Affects:** Developer tooling, build system, package management, compiler driver (`oric`), spec governance, CI, release packaging
**Related:** single-semantic-authority-tooling-proposal.md (draft), self-contained-toolchain-proposal.md (draft), task-runner-tooling-proposal.md (draft), package-version-resolution-proposal.md (approved), stdlib-philosophy-proposal.md (approved), lsp-implementation-proposal.md (approved)

---

## Summary

Ori ships **one** opinionated developer tool — the `ori` binary — and every development action (`build`, `run`, `test`, `fmt`, `lint`, `fix`, and future tooling) is a subcommand of it. There is one canonical default for each task and no decision fatigue to get a working result, while operational flags that a real toolchain genuinely needs (linker/target selection, editor preferences) remain available. This proposal is the **philosophy umbrella** for Ori's toolchain: it states outcome-level invariants that the concrete tooling proposals (cache lifecycle / GC, build orchestration, versioning, self-contained packaging) each cite and derive from. It is the tooling-layer peer of the approved `stdlib-philosophy-proposal.md` and the draft `trait-philosophy-proposal.md`.

It applies the same stance stdlib-philosophy takes toward libraries — "batteries included, one canonical choice, no decision fatigue" — to the developer toolchain, using **Go** as the primary north star for tooling-as-a-first-class-design-constraint and **Zig** as the sharper reference for cache and incremental-compilation discipline.

---

## Motivation

### The Problem in Practice

A developer's experience of a language is not only its syntax — it is the daily loop of building, testing, formatting, and shipping. When that loop is fragmented across many tools, each with its own configuration, the language taxes every user before they write a line of interesting code.

Two failure modes recur in existing ecosystems:

**1. Tool fragmentation (the JavaScript/Rust surface).** A developer must assemble a working toolchain from separately-versioned pieces: a build tool, a formatter, a linter, a test runner, a task runner, a package manager — each configured independently, each capable of disagreeing with the others. Rust mitigates this with `cargo` subcommands, but formatting (`rustfmt`), linting (`clippy`), and the language server (`rust-analyzer`) remain distinct engines with independent release cadences, and the analyzer famously re-derives type information separately from `rustc` and disagrees with it.

**2. Ungoverned build-state accumulation.** Rust's `target/` directory is the canonical example: a per-project, never-garbage-collected cache that stacks an incremental-compilation cache, full debug information, and every historical build artifact, with no automatic eviction. On a developer machine with a dozen workspaces, "disposable" cache silently grows to hundreds of gigabytes because *disposable* was never made *automatically disposed*. The design decision that produced this is not "having a cache" — it is failing to assign an **owner** to the cache's lifecycle.

```
# The failure a new Ori user must never see:
$ ori build hello.ori
error: linker 'link.exe' not found        # tool fragmentation — needs an external toolchain

$ du -sh target/                           # ungoverned accumulation
63G     target/
#   40G  incremental cache — never pruned
#   ~90% of each 210MB binary — debug info the user will never read
#   10 historical copies of the same binary — never garbage-collected
```

### When This Matters

- **Every new user**, at the first `ori build`, `ori test`, and `ori fmt` — the getting-started loop is the language's first impression.
- **AI code authors.** Ori is designed AI-written-first. An automated author acts on the toolchain's feedback loop (build errors, test results, lint output) as its primary signal. Fragmentation and drift between tools are not UX papercuts for an LLM author — they are correctness hazards that propagate at scale.
- **CI and long-lived developer machines**, where ungoverned cache accumulation becomes an operational cost nobody scheduled.

### Why Now

The alpha phase is when users first build workflows around the toolchain. Establishing the one-tool, self-governing story before those workflows harden avoids retrofitting it against installed habits — the same reasoning `stdlib-philosophy-proposal.md` applied to the library surface.

---

## Goals and Non-Goals

**Goals:**

- Establish, as a governed invariant, that Ori ships **one** tool with unified subcommands and one canonical default for each task.
- Name the outcome-level invariants (below) that concrete tooling proposals must satisfy and cite.
- Fix the design lesson that **whoever creates build/cache state owns its eviction lifecycle** — no ungoverned accumulation.
- Anchor the toolchain philosophy in the project's cross-cutting mission inventory on approval, alongside the Stdlib, Testing, and Tooling missions.

**Non-Goals:**

- This proposal does **not** specify the concrete mechanism of any facet. The cache-GC algorithm, the build-orchestration event model, the versioning semantics, and the self-contained-packaging plan are each owned by their own proposal. This document states the outcomes they answer to; it does not implement them.
- It does **not** re-decide the semantic-authority invariant. `single-semantic-authority-tooling-proposal.md` already governs the *engine* half. This umbrella cites that invariant and sits above it; it must not duplicate or weaken it.
- It does **not** propose new surface syntax, keywords, or type-system changes.
- It does **not** re-decide any facet's mechanism a cited proposal has already settled. Where an invariant here points in a direction a cited proposal has not adopted (orchestration, versioning), the invariant is stated as a permitted capability the facet MAY realize through its own successor/amendment — never as a mechanism this umbrella imposes.

---

## Design

The philosophy is expressed as seven invariants, stated at outcome level. Each concrete tooling proposal declares which invariants it realizes and owns the mechanism.

### T1 — One Tool

Ori ships a single binary, `ori`. Every first-class development action is a subcommand: `ori build`, `ori run`, `ori test`, `ori fmt`, `ori lint`, `ori fix`, and future tooling. There is no separate formatter binary, linter binary, or package-manager binary to install, version, or configure independently. The compiler driver (`oric`) is the implementation locus; `ori` is the user-facing surface.

*North star:* Go's `go` command and Zig's `zig` — one download, one command, every action a subcommand.

### T2 — One Canonical Default (no decision fatigue)

For each task there is one canonical default, so a user gets a correct, working result without a decision to make. This bans *gratuitous style and layout options* — the failure mode where a formatter or build layout offers a matrix of cosmetic choices (the approved zero-options `ori fmt` stance per Annex D + `remove-dot-prefix-proposal.md` is the model). It does **not** ban *operational configuration* that a real toolchain genuinely requires:

- **Operational flags are permitted** — linker selection, target triple, self-contained mode, and similar build-behavior flags (owned by their facets, e.g. `self-contained-toolchain`). No cross-compiler can be flag-free.
- **Editor and experience preferences are permitted** — the approved `lsp-implementation-proposal.md` configuration surface (inlay hints, diagnostic delay, format-on-save, memory limits, and its precedence chain) is legitimate per-user preference, not a canonicality violation.

The distinction is: **canonical task behavior has one default and no cosmetic options; operational and editor-preference configuration remains available.** The north stars themselves confirm this — Go carries build tags and `GOFLAGS`; Zig's `build.zig` is real, fully-configurable code.

### T3 — Single Semantic Authority (cited, not re-decided)

All semantic tooling is the same compiler engine, never a re-implementation. This invariant is **owned by `single-semantic-authority-tooling-proposal.md`**; it is cited here only to place it within the umbrella. See that proposal for the definition, motivation, and enforcement. This umbrella never restates or weakens it.

### T4 — The Creator Owns the Lifecycle

Any tool that creates build or cache state owns that state's eviction policy. Ori's build tool must not produce a cache that only a human remembers to clean. Stated at outcome level:

- The build/cache is **bounded and automatically self-evicting** — the tool that wrote it reclaims it under a defined policy on by default, so cache never accumulates without limit.
- Debug information is **separable** — the shipped artifact is the code, not the code dominated by debug symbols the user will never read.

*Anti-pattern explicitly rejected:* a per-project cache with no automatic eviction that grows without bound (Rust's `target/`). The concrete eviction algorithm, whether artifacts are content-addressed or updated in place (the Zig direction is one option), and the debug-info split mechanism are all owned by a dedicated cache-lifecycle proposal; this invariant is the outcome that proposal satisfies, not a mechanism it imposes.

### T5 — Self-Contained

The toolchain is a single download that compiles Ori to native executables with zero external dependencies **on the common path**, with documented platform exceptions. *Owned by `self-contained-toolchain-proposal.md`*; cited here as a facet of the one-tool promise. That proposal documents the exceptions its design commits to (notably a small macOS Xcode Command Line Tools install, plus phased residual platform CRT/libc needs); T5 asserts the common-path outcome, not a stronger zero-dependency guarantee than the facet delivers. North stars: Go (internal linker, pure-Go runtime) and Zig (bundled LLD, bundled libc headers).

### T6 — Orchestration Is a First-Class Capability

The toolchain **MAY** expose first-class, cross-platform task and lifecycle orchestration — named tasks and, where a facet chooses to provide them, lifecycle events — rather than requiring external orchestrators. This is stated as a permitted capability, not an imposed mechanism: the existing `task-runner-tooling-proposal.md` deliberately starts from simple string commands and explicitly defers pre/post hooks and ordering. T6 does not override that decision. A richer orchestration model, if adopted, is owned by a **successor proposal** that supersedes the task-runner design on its own merits; this umbrella names the capability as a north star, not the hook-and-ordering mechanism.

### T7 — Reproducible and Deterministic

The same manifest produces identical builds; resolution is deterministic and never silently guesses. This determinism invariant is **already satisfied** by the approved `package-version-resolution-proposal.md` (its Design Principle #4). This umbrella asserts *only* determinism — an outcome both an exact-pinning model and any future model satisfy. Any reconsideration of the resolution *policy* itself is out of scope here and is owned entirely by a separate versioning amendment, which will carry the proper `Amends:` linkage to the approved proposal. The umbrella states no directional preference on resolution policy.

### Facet map

| Facet | Invariant(s) | Owning proposal | Relationship |
|---|---|---|---|
| Unified `ori` tool + canonical defaults | T1, T2 | **this proposal** | defines |
| Semantic authority (one engine) | T3 | single-semantic-authority-tooling (draft) | cite only — do not duplicate |
| Cache / build-state GC lifecycle | T4 | *new proposal to follow (no owner yet)* | this umbrella is its stated outcome contract |
| Self-contained packaging | T5 | self-contained-toolchain (draft) | facet |
| Build orchestration (capability) | T6 | task-runner-tooling (draft) → successor for a richer model | permitted capability; successor supersedes, does not extend |
| Versioning / resolution | T7 | package-version-resolution (approved); policy reconsideration → forthcoming amendment | determinism cited; any policy change amended by a forthcoming proposal |

---

## Drawbacks

- **Opinionation has a cost.** "One canonical default" means users who want a different formatter style or build layout are told no. This is the deliberate stdlib-philosophy trade — canonical choices over cosmetic flexibility — and it concentrates responsibility on the core team to get the defaults right. (T2 deliberately preserves operational and editor configuration, so this cost is bounded to cosmetic/style options.)
- **A single tool is a single point of failure and a single release bottleneck.** Bundling fmt/lint/test/build into one binary means a regression in one surface ships in the same artifact as the others. stdlib-philosophy's "independently versioned packages" answer does not apply to the tool itself; mitigations (fast release cadence, strong test coverage of each surface) are a standing obligation, not a solved problem.
- **Self-contained packaging grows the download.** Bundling a linker and platform headers (T5) trades a larger initial download for zero common-path external dependencies. This is Go/Zig's accepted trade, and it carries documented platform exceptions.
- **An umbrella can over-constrain its facets.** Stated too rigidly, an invariant could forbid a better mechanism a facet later discovers. This proposal mitigates that by stating T4/T6/T7 as outcomes or permitted capabilities rather than mechanisms, and by deferring every directional policy question (orchestration hooks, versioning policy) to the owning facet's own successor/amendment.

---

## Alternatives Considered

### Alternative 1: No umbrella — let each tooling proposal stand alone

Author cache-GC, orchestration, versioning, and self-contained-packaging as independent proposals with no shared philosophy. **Rejected:** without a stated SSOT, each proposal re-faces the same one-tool-vs-many and who-owns-the-cache questions, and they drift. stdlib-philosophy exists as an umbrella for precisely this reason.

### Alternative 2: Fold the semantic-authority invariant into this proposal

Absorb `single-semantic-authority-tooling-proposal.md` here so the whole tooling philosophy is one document. **Rejected:** that draft already governs the engine invariant with its own motivation and enforcement; folding it in would either duplicate it (SSOT violation) or supersede a standing draft without cause. The umbrella cites it (T3); it does not consume it.

### Alternative 3: State a versioning-policy direction in the umbrella

An earlier framing had the umbrella pre-declare a "pluggable versioning semantics" direction in tension with the approved exact-pinning policy. **Rejected:** a "defines no mechanism" umbrella must not pre-decide a resolution policy that reverses an approved proposal, especially before the amending proposal exists. T7 therefore asserts only the determinism invariant (already approved and satisfied by both an exact-pinning model and any future model). Any reconsideration of resolution policy is owned entirely by a separate versioning amendment that will carry the `Amends:` linkage — the umbrella states no directional preference.

---

## Purity Analysis

**Can be pure Ori?** NO — this proposal governs the compiler driver and toolchain, which is compiler-level infrastructure, not a library surface.

**If not, why:** The `ori` tool, its subcommand dispatch, build orchestration, cache lifecycle, and self-contained packaging live in `oric` and the runtime/build layer. They are not expressible as pure-Ori stdlib because they *are* the tool that compiles and runs Ori.

**Missing features that would enable purity:** N/A — this is deliberately toolchain infrastructure, not a candidate for the lean-core / rich-libraries split.

**Recommendation:** Proceed as a governance/philosophy proposal. It requests no new compiler *language* features; it establishes outcome-level invariants the toolchain implementation and its facet proposals must satisfy. On approval it is recorded as a governed toolchain-philosophy invariant in the project's cross-cutting mission inventory (peer to the Stdlib, Testing, and Tooling missions), and each facet proposal cites the invariant(s) it realizes.

---

## Spec & Grammar Impact

- **No grammar changes.** No new productions, keywords, or syntax.
- **No normative spec-clause changes** to the language surface (Clauses 1–27). The toolchain is described in the guide/tooling docs and governed by this proposal + its facets, not by the language spec.
- **Mission anchor (on approval):** the toolchain philosophy is recorded as a cross-cutting **Toolchain** mission in the project's mission inventory, stating the seven invariants as the toolchain's north star, with a conflict-resolution rule (one tool / one canonical default wins over cosmetic configurability; the creator owns the cache lifecycle).

---

## Prior Art

- **Go — the gold standard for unified tooling.** `go` is one command with subcommands (`build`, `test`, `fmt`, `vet`, `mod`); a shared, auto-GC'd build cache (`$GOCACHE`) reused across all projects; single static-binary output; and a self-contained toolchain (internal linker, pure-Go runtime) that builds on a fresh machine. Go is the primary north star for T1, T2, T4, T5. Go retains operational configuration — build tags, `GOFLAGS`, selectable `vet` analyzers — consistent with T2's canonical-default-not-zero-config stance. *Verified against the `go` reference repo indexed in the intelligence graph and Go's published `cmd/go` documentation.*
- **Zig — the sharpest cache/incremental discipline.** `zig` is one command; `build.zig` is a build system written in Zig (real, configurable code, not a declarative DSL); the cache is content-addressed with an explicit manifest, and the project's stated direction is in-place incremental compilation rather than accumulating historical artifacts. Zig bundles LLD and libc headers for self-contained cross-compilation. Zig is the primary reference for T4 and a north star for T5. *Verified against the `zig` reference repo; cache-locking design signal `zig#9258`.*
- **Rust / Cargo — the mixed case that motivates T3 and T4.** Cargo's subcommand UX is good prior art for T1, but formatting, linting, and the language server are separate engines; `rust-analyzer` re-implements type inference independently of `rustc` (the T3 anti-pattern, per single-semantic-authority). And `target/` is the canonical ungoverned-accumulation failure (the T4 anti-pattern): per-project, never auto-GC'd, though Cargo's global registry cache did gain scheduled auto-GC before per-`target/` GC. *Verified against the `rust` issue corpus indexed in the intelligence graph.*
- **TypeScript / Roslyn — single-engine tooling.** `tsserver` *is* `tsc` in watch mode; C#'s Roslyn is compiler-as-a-service shared by the IDE, analyzers, and `csc`. Both make the editor and the build structurally incapable of disagreeing about program meaning — the proven realization of T3 (cross-referenced by single-semantic-authority-tooling-proposal.md). *Verified against published Roslyn/tsserver architecture documentation.*
- **Swift, Lean — convergence toward one-tool.** Swift ships a `swift format` subcommand (`swift#75502`) with requests for more (`swift#82056`); Lean's `lake` unifies `lake test` + `lake lint` (`lean4#4261`). Peer ecosystems are moving toward unified tooling, not away — no regret/deprecation sentiment against the one-tool model surfaced in the issue corpora. *Verified against the `swift` and `lean4` issue corpora indexed in the intelligence graph.*

---

## Unresolved Questions

- **Subcommand surface boundary.** Which actions are first-class `ori` subcommands versus delegated to the orchestration layer (T6)? Resolves during the orchestration facet/successor proposal.
- **Debug-info separation mechanism (T4).** Split-debug sidecar vs strip-by-default-with-opt-in vs both — the concrete choice belongs to the cache-lifecycle facet proposal, not here.
- **Versioning reconciliation (T7).** Whether and how the resolution policy is reconsidered is owned entirely by the forthcoming versioning amendment, which will carry the `Amends:` linkage. Out of scope for this umbrella.
- **Global cache location and sharing model (T4).** Per-user global cache directory, its path/override convention, and cross-project sharing semantics — owned by the cache-lifecycle facet proposal.
- **Enforcement of T3 for future tooling.** How a new tool is prevented from standing up a parallel analyzer — owned by single-semantic-authority-tooling-proposal.md, referenced here.
