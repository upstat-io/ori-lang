---
plan: "ffi-boundary-safety"
title: "FFI Boundary Safety — Deep FFI Part 2"
plan_type: compiler
status: queued
reviewed: false
implements:
  - "docs/ori_lang/proposals/approved/ffi-boundary-safety-proposal.md"
supersedes:
  - "docs/ori_lang/proposals/superseded/c-header-import-proposal.md (design superseded)"
references:
  - "docs/ori_lang/proposals/approved/ffi-boundary-safety-proposal.md"
  - "docs/ori_lang/proposals/approved/deep-ffi-proposal.md"
  - "docs/ori_lang/proposals/approved/platform-ffi-proposal.md"
  - "docs/ori_lang/proposals/approved/capability-unification-generics-proposal.md"
  - "docs/ori_lang/proposals/approved/stateful-mock-testing-proposal.md"
  - "docs/ori_lang/proposals/drafts/capability-propagation-completion-proposal.md"
  - ".claude/rules/aims-rules.md §§1–9"
  - ".claude/rules/canon.md §7.1 Five Load-Bearing Invariants"
  - "docs/ori_lang/v2026/spec/26-ffi.md"
  - "compiler/ori_arc/src/aims/interprocedural/extract.rs"
  - "compiler/ori_parse/src/grammar/item/extern_def.rs"
sections:
  - id: "01"
    title: "AIMS FFI Contract Extension"
  - id: "01.R"
    title: "§01 Third Party Review Findings"
  - id: "02"
    title: "Header-Assisted Extern Blocks — #header(...)"
  - id: "02.R"
    title: "§02 Third Party Review Findings"
  - id: "03"
    title: "Compile-Time Struct Layout Verification"
  - id: "03.R"
    title: "§03 Third Party Review Findings"
  - id: "04"
    title: "#borrow_from(param) — Lifetime Annotations"
  - id: "04.R"
    title: "§04 Third Party Review Findings"
  - id: "05"
    title: "Callback Capability Propagation"
  - id: "05.R"
    title: "§05 Third Party Review Findings"
  - id: "06"
    title: "FFI-Aware Diagnostics"
  - id: "06.R"
    title: "§06 Third Party Review Findings"
  - id: "07"
    title: "Handler-Mocking Formalization + Record/Replay"
  - id: "07.R"
    title: "§07 Third Party Review Findings"
  - id: "08"
    title: "Spec, Grammar, and ori-syntax.md Sync"
  - id: "08.R"
    title: "§08 Third Party Review Findings"
blocked_by:
  - "capability-propagation-completion-proposal.md (draft) — blocks §05 only; §01–04, §06, §07 may proceed independently"
---

# FFI Boundary Safety — Deep FFI Part 2

## Mission

Make Ori's mission claim "memory safety across the entire program surface, including FFI" (`missions.md §Ori`, §AIMS) hold in implementation. Approved Deep FFI made the FFI *surface* safe (declarative marshalling, ownership annotations, `#free`, handler-based mocking). This plan extends that safety *across* the boundary: the AIMS lattice analysis sees FFI ownership contracts as typed interprocedural facts (not opaque conservative calls), header-derived signatures eliminate transcription drift, struct layouts are verified against C headers at compile time, returned-pointer lifetimes bind to named parameters via `#borrow_from`, callback capabilities propagate through registration, and handler-mocking formalization gains partial-mock strict mode plus record/replay.

**Mission Success Criteria:**

1. An `extern` function declared `owned CPtr` consumed by another FFI call generates **zero unnecessary RC operations** in emitted LLVM IR (AIMS sees the single-owner chain end-to-end).
2. A mismatch between an Ori `#repr("c")` struct and its C header struct produces a compile-time E4015 error pinpointing both sides of the mismatch with field-by-field diff — **never a runtime crash**.
3. A `#borrow_from(p)`-annotated return outliving parameter `p`'s scope is rejected at compile time with E4018, **never returning a dangling pointer**.
4. A C callback whose Ori registration uses capabilities not declared on the callback parameter type is rejected at compile time with E4019, **never a runtime capability-missing failure**.
5. A user running `ori test` with `with FFI("lib") = record(to: "…") in { … }` produces a deterministic transcript file; the same tests under `replay(from: "…")` produce identical observable behavior **without the real C library present**.
6. Five new spec subsections (one per §1/§2/§3/§4/§5/§6 of the approved proposal) land in `docs/ori_lang/v2026/spec/26-ffi.md`; grammar productions for `#header`, `#header_path`, `#borrow_from`, `#verify_layout`, `#strict`, and callback `uses` lists land in `grammar.ebnf`; `.claude/rules/ori-syntax.md` quick reference reflects the new attribute surface.
7. `ffi-boundary-safety-proposal.md` §1 AIMS extension is implemented *without* a parallel RC emission path, shadow escape enum, or uniqueness tracker — the extension extends `ParamContract` / `ReturnContract` / `EffectSummary` fields on `MemoryContract`, preserving AIMS Invariant #5 (unified model — `canon.md §7.1`, `CLAUDE.md §AIMS`).

Every criterion above maps to ≥1 section below. Every section below connects upward to ≥1 criterion.

## Architecture

```
            ┌────────────────────────────────────────────┐
            │  Approved Deep FFI (platform-ffi, deep-ffi)│
            │   extern "c" from "lib" { owned|borrowed   │
            │   #error(...) #free(...) handler{...} }    │
            └───────────────────┬────────────────────────┘
                                │  (this plan layers on top)
                                ▼
   ┌──────────────────────────────────────────────────────────────┐
   │  §01  AIMS FFI Contract Extension                            │
   │  extern decl → MemoryContract fragment (ParamContract /      │
   │  ReturnContract / EffectSummary) consumed by lattice         │
   │  analysis. NO parallel RC path. Invariant #5 load-bearing.   │
   └───────────┬──────────────────────────────────────────────────┘
               │
     ┌─────────┼────────────────────────┬──────────────────┐
     ▼         ▼                        ▼                  ▼
┌──────────┐ ┌────────────────┐ ┌─────────────────┐ ┌────────────────┐
│ §02      │ │ §04            │ │ §05             │ │ §06            │
│ #header  │ │ #borrow_from   │ │ Callback caps   │ │ FFI-aware      │
│ libclang │ │ Locality::     │ │ (subset check,  │ │ diagnostics    │
│ parse    │ │ Borrowed(p)    │ │  BLOCKED_BY     │ │ (E4015-E4025,  │
│          │ │                │ │  capability-    │ │  C header spans)│
│          │ │                │ │  propagation-   │ │                │
│          │ │                │ │  completion)    │ │                │
└─────┬────┘ └────────────────┘ └─────────────────┘ └────────────────┘
      │
      ▼  (depends-on)
┌──────────────────┐
│ §03 Struct       │
│ layout           │
│ verification     │
│ (E4015)          │
└──────────────────┘

                    ┌────────────────────────────────┐
                    │ §07 Handler-Mocking            │
                    │ • 7a #strict partial-mock      │
                    │ • 7b stateful+FFI state        │
                    │ • 7c record/replay (std.ffi)   │
                    └────────────────────────────────┘

                    ┌────────────────────────────────┐
                    │ §08 Spec / Grammar / Syntax    │
                    │ sync — cross-cutting, lands    │
                    │ sections incrementally         │
                    └────────────────────────────────┘
```

## Design Decisions

### D1 — AIMS extension mechanism

**Decision.** Extend `MemoryContract` / `ParamContract` / `ReturnContract` / `EffectSummary` with FFI-origin flags (to be pinned in §01 design log). DO NOT introduce a parallel RC emission path, shadow escape enum, or FFI-specific uniqueness tracker.

**Rationale.** AIMS Invariant #5 (`CLAUDE.md §AIMS`, `canon.md §7.1`, `arc.md §"Non-Negotiable Invariants"`): "New capabilities must either (a) extend a lattice dimension, (b) extend a contract field on MemoryContract/ParamContract/ReturnContract/EffectSummary, or (c) feed the lattice-driven analysis as a typed pre-pass input. What they must NOT do is spawn an independent RC emission path, a parallel escape enum, or a shadow uniqueness tracker that bypasses the lattice." Option (b) applies here — extern declarations produce contract fragments that the existing interprocedural SCC fixpoint consumes.

**Evidence from prior art.** Lean 4's `ownParamsUsingArgs` (`arc.md §"Advanced Optimization Patterns"`) treats FFI-equivalent borrow hints as contract fields, not as a parallel path. Swift's ARC optimizer handles `__bridge_retained`/`__bridge_transfer` via the same lattice the native-Swift optimizer uses. Koka's handler composition extends the effect-row type system, not a side structure.

### D2 — Header import surface

**Decision.** Block-level `#header("file.h")` attribute on `extern "c" from "lib" { ... }` blocks, with an **explicit function list inside the block**. No namespace import (`@cImport`-style) and no `ori bindgen` tool.

**Rationale.** Preserves the approved Deep FFI principle "explicit boundaries: FFI calls are clearly marked, not hidden" (`deep-ffi-proposal.md §"Why not auto-import C headers?"`) while eliminating the admitted signature-transcription boilerplate. Supersedes `c-header-import-proposal.md` (now in `superseded/`).

### D3 — Callback capability propagation sequencing

**Decision.** §05 is BLOCKED_BY `capability-propagation-completion-proposal.md`. §01–04, §06, §07 proceed independently.

**Rationale.** `capability-propagation-completion-proposal.md` documents that capability propagation is only partially shipped today (LLVM/AOT stubbed, stateful handlers partial, marker capability enforcement partial). Callback capability subset verification (§05) is unimplementable against that incomplete foundation. The BLOCKED_BY edge is explicit so `/continue-roadmap` sequences §05 correctly.

### D4 — Replay determinism model

**Decision.** `replay` returns recorded values verbatim for matching call sequences, INCLUDING values from non-deterministic C calls (`time()`, `rand()`, etc.). Divergence is surfaced by `ReplayDivergenceError`. No separate "non-determinism detected" warning.

**Rationale.** The point of replay is deterministic test runs. Tests needing real non-deterministic behavior drop out of replay mode. See approved proposal §7c rule 5.

### D5 — Diagnostic error code range

**Decision.** Reserve E4015–E4025 as an FFI sub-range inside the E4xxx ARC/FFI-boundary family (per `CLAUDE.md §Key Paths`: E4xxx = ARC; FFI boundary safety is an ARC-adjacent concern — ownership, uniqueness, layout). E4001–E4005 remain ARC-lowering-specific.

**Rationale.** FFI boundary safety is primarily an AIMS/ARC-interaction concern (ownership contracts, uniqueness, memory-layout). Putting its codes in the ARC family keeps the mental model coherent. If a separate FFI range (e.g., E7xxx) is wanted later, E4015–E4025 can be renumbered via a routine sweep.

## Dependency Graph

```
§01  AIMS FFI Contract Extension
       │
       ├── provides ParamContract/ReturnContract FFI fields
       │
       ▼
     §02  #header(...) blocks ──────┐
       │                            │
       │                            ▼
       │                          §03  Struct layout verification
       │                            (uses §02's libclang pipeline)
       │
       ▼
     §04  #borrow_from(p)
       (extends AIMS Locality::Borrowed(p); uses §01 contract plumbing)

     §05  Callback capabilities
       BLOCKED_BY capability-propagation-completion-proposal.md
       (depends on §01 for callback parameter contract plumbing)

     §06  Diagnostics — lands incrementally with §01–§05
     §07  Handler mocking — independent of §01–§06 (stdlib work + #strict)
     §08  Spec/grammar/syntax sync — cross-cutting, ships per section
```

## Implementation Sequence

Recommended order (matches approved proposal §Roadmap Impact):

| Order | Section | Weeks | Depends on | Notes |
|-------|---------|-------|------------|-------|
| 1 | §01 | 4–6 | — | Foundation; §02, §04, §05 consume its plumbing |
| 2 | §02 | 6–8 | §01 | Adds libclang dep; largest single work item |
| 3 | §03 | 3–4 | §02 | Reuses libclang pipeline |
| 4 | §04 | 3–4 | §01 | AIMS locality extension |
| 5 | §06 | 2–3 | §01–§05 incrementally | Secondary-span diagnostics — can land per section as each one emits its E4xxx |
| 6 | §07 | 3–4 | (independent) | Stdlib + `#strict` type-check flag — can parallelize with §02 |
| 7 | §05 | 3–4 | §01, **capability-propagation-completion** | LAST; blocked until capability-propagation infra lands |

§08 (spec/grammar/syntax sync) runs incrementally: each implementation section co-commits the relevant spec clause, grammar production, and `ori-syntax.md` entries.

## Known Bugs / Findings From /review-draft-proposal

Filed during proposal review (2026-04-20); addressed by this plan:

- **C1 / C2 (resolved via proposal edits):** conflict with `c-header-import-proposal.md` resolved via `Supersedes:` header; `capability-propagation-completion` added as declared dependency for §05.
- **C3 → §01 design-log requirement:** the exact `ParamContract`/`ReturnContract`/`EffectSummary` schema for FFI-origin facts MUST be pinned down in a §01 design log BEFORE implementation begins. Invariant #5 is load-bearing.
- **C4 → §04 multi-borrow example:** revised proposal now includes concrete example (`utf8_common_prefix`) and restricts `#borrow_from` to top-level parameters; §04 inherits this restriction.
- **C5 → §02 header path resolution:** deterministic search order (explicit `#header_path` > `pkg-config` > system includes > E4020) specified in proposal; §02 must implement in that order.
- **C6 / C7:** errata citation correct; `ori bindgen` deferred; proposal text updated.
- **C8 → §03 diagnostic example revised:** version-attribution removed; diagnostics show only field-diff + header line.
- **C9 → §07c replay determinism:** rule added: replay is deterministic-by-construction.

## Plan Type

`compiler`. Touches `compiler/ori_arc`, `compiler/ori_types`, `compiler/ori_llvm`, `compiler/ori_diagnostic`, `compiler/ori_parse`, plus `library/std/ffi` (new module), plus spec/grammar. Spec proposal gate (`/create-draft-proposal` → `/review-draft-proposal`) already satisfied — the proposal was approved 2026-04-20 via the standard gate.

## Reference Files (Grounded)

- `compiler/ori_arc/src/aims/interprocedural/extract.rs` — MemoryContract extraction site for §01
- `compiler/ori_arc/src/aims/contract/mod.rs` — `ParamContract`/`ReturnContract`/`EffectSummary` definitions
- `compiler/ori_arc/src/aims/intraprocedural/mod.rs` — AimsStateMap consumer
- `compiler/ori_parse/src/grammar/item/extern_def.rs` — existing extern block parser
- `compiler/ori_diagnostic/src/error_code/mod.rs` — E4001–E4005 already present; E4015–E4025 free
- `docs/ori_lang/v2026/spec/26-ffi.md` — spec clause home for new subsections
- `docs/ori_lang/v2026/spec/grammar.ebnf` — grammar productions
- `library/std/` — no `ffi/` directory exists yet (created in §07)
- `.claude/rules/ori-syntax.md` — quick-reference Deep FFI section

## Quick Reference

| Section | Goal | Key files | Exit criterion |
|---------|------|-----------|----------------|
| §01 | AIMS sees FFI ownership | `ori_arc/src/aims/interprocedural/`, `aims/contract/` | Owned-chain FFI program generates zero spurious RC ops |
| §02 | Header-assisted signatures | `ori_parse/src/grammar/item/extern_def.rs`, new libclang wrapper crate | Signatures auto-derived; drift detected on recompile |
| §03 | Struct layout verification | §02 libclang pipeline; `ori_repr` cross-check | SQLite-style layout diff produces E4015 at compile time |
| §04 | Borrow-from lifetime | `ori_arc/src/aims/lattice/` Locality extension; `ori_types` scope check | Outliving return is E4018 at compile time |
| §05 | Callback capability subset | `ori_types/src/check/capabilities/`; requires capability-propagation-completion | Subset violation is E4019 at compile time |
| §06 | FFI diagnostics | `ori_diagnostic/src/errors/E40{15-25}.md` + secondary-span C header support | All E40xx FFI diagnostics show C header span when `#header` is in use |
| §07 | Handler mocking | `library/std/ffi/trace.ori` (new); `ori_types` for `#strict` | Record/replay round-trip verified; `#strict` mocks reject unmocked calls at compile time |
| §08 | Spec/grammar/syntax sync | `spec/26-ffi.md`, `grammar.ebnf`, `.claude/rules/ori-syntax.md` | Each implementation section co-commits its surface |

## Estimated Effort

24–33 weeks total (6–8 months) assuming sections execute sequentially in the recommended order. §05 cannot start until `capability-propagation-completion` lands, which itself is not yet scheduled — §05's wall-clock is "after that plan completes + 3–4 weeks." The other sections (§01–§04, §06, §07, §08) have no external blockers.
