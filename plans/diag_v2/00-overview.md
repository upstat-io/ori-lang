# Diagnostic V2: Best-of-Breed Diagnostic Intelligence

> **Description**: Upgrade Ori's diagnostic system from strong *infrastructure* (already in place: `ErrorGuaranteed`, `DiagnosticQueue`, multi-format emission, 67 error codes) to best-in-class *intelligence* — composable document trees, structural type diffing, edit-distance suggestions, context-aware error messages, and production code fixes. Synthesized from patterns across Rust, Go, Zig, Gleam, Elm, Roc, and TypeScript compilers.
>
> **Primary Goal**: Make Ori's error messages *the best in any language*. Every error should tell the user what went wrong, why, and exactly how to fix it — with machine-applicable suggestions wherever possible.

## Reference Compiler Analysis

Patterns were analyzed from seven production compilers and synthesized into a novel design:

| Compiler | Key Insight Adopted |
|----------|-------------------|
| **Rust** (`rustc_errors`) | `EmissionGuarantee` type parameter; `Applicability` 4-level suggestion confidence; `StashKey` multi-phase diagnostic improvement; `Subdiagnostic` derive macro |
| **Go** (`cmd/compile`) | Follow-on suppression; one syntax error per line; error limit (10 max); progressive error building |
| **Zig** (`Compilation.zig`) | `ErrorBundle` packed binary format; `LazySrcLoc` deferred resolution; `ResolvedReference` traces through dependency graph |
| **Gleam** (`compiler-core`) | Three-tier error system (domain → wrapper → `Diagnostic`); `ExtraLabel` for cross-file secondary spans; `codespan_reporting` rendering |
| **Elm** (`Reporting/`) | `Expected = NoExpectation \| FromContext \| FromAnnotation` context encoding (**highest-impact pattern**); `Category` for value classification; Damerau-Levenshtein edit distance in `Suggest.hs`; conversational operator-specific messages |
| **Roc** (`crates/reporting`) | `RocDocAllocator` composable document tree (**key missing piece**); semantic `Annotation` enum decoupled from styling; `Palette` pattern for multi-format rendering; `to_diff` recursive structural type diffing (**best-in-class**) |
| **TypeScript** (`src/compiler`) | `DiagnosticMessageChain` for nested "because..." explanations; `DiagnosticRelatedInformation[]` for cross-file context; `CodeFixProvider` registered per error code |

## Design Philosophy

### 1. Four-Layer Architecture (Preserved from V1)

The existing four-layer separation is correct and preserved. V2 enriches Layers 2 and 3:

```
Layer 1: Problem Detection (per-phase)
  ├── LexProblem, ParseProblem, SemanticProblem  (V1 — unchanged)
  ├── TypeCheckError with ErrorContext            (V1 — enriched in V2)
  └── CodegenProblem (ArcProblem + LlvmProblem)  (V1 — unchanged)

Layer 2: Document Composition (NEW in V2)
  ├── ori_doc: composable document trees          (from Roc/Elm)
  ├── Expected<T>: context encoding               (from Elm)
  ├── TypeDiff: structural type diffing           (from Roc)
  └── suggest: edit-distance suggestions          (from Elm)

Layer 3: Diagnostic Packaging
  ├── Diagnostic: enhanced with chains + related  (V2 enrichment)
  ├── DiagnosticQueue: dedup + follow-on filter   (V1 — unchanged)
  └── CodeFix: production implementations         (V2 new)

Layer 4: Emission
  ├── TerminalEmitter: enhanced rendering         (V2 enrichment)
  ├── JsonEmitter                                 (V1 — unchanged)
  └── SarifEmitter                                (V1 — unchanged)
```

### 2. Intelligence Over Infrastructure

V1 built excellent plumbing (`ErrorGuaranteed`, `DiagnosticQueue`, multi-format emission). V2 fills the intelligence gap — the *content* of error messages, not how they're delivered:

| V1 (Infrastructure) | V2 (Intelligence) |
|---------------------|-------------------|
| Error codes exist | Error messages explain *why* |
| Suggestions are strings | Suggestions are machine-applicable with diffs |
| "Type mismatch: expected X, found Y" | "This is `float` because of the `+` on line 5, but the annotation says `int`" |
| Unknown identifier reported | "Did you mean `similar_name`? (edit distance: 1)" |
| Single error message | "because..." chain explaining reasoning |
| Labels point to code | Type diff highlights *where* types diverge |

### 3. Crate Placement

All new code lives in existing crates — no new crates needed:

| Module | Crate | Rationale |
|--------|-------|-----------|
| `suggest` | `ori_diagnostic` | Utility module, no phase dependencies |
| `ExplanationChain` | `ori_diagnostic` | Extension to core `Diagnostic` type |
| `RelatedInformation` | `ori_diagnostic` | Extension to core `Diagnostic` type |
| `ori_doc` | `ori_diagnostic` | Document tree is part of diagnostic rendering |
| `Expected<T>` | `ori_types` | Context about *type* expectations |
| `TypeDiff` | `ori_types` | Operates on type representations |
| Code fixes | `ori_diagnostic` (trait) + `oric` (impls) | Trait in lib, implementations in compiler |
| Reference traces | `oric` | Requires Salsa query graph access |
| Emitter v2 | `ori_diagnostic` | Renders `ori_doc` trees |

### 4. Salsa Compatibility

All types that may appear in Salsa query results derive `Clone, Eq, PartialEq, Hash, Debug` per v2-conventions §8. Types used only at the rendering boundary (document trees, ANSI output) are exempt.

### 5. Backward Compatibility

V2 is additive — no existing public API is removed or changed. New fields on `Diagnostic` default to empty/None. Existing `into_diagnostic()` implementations continue to work. New intelligence is added incrementally per error code.

## Current Pain Points Addressed

| Pain Point | Root Cause | V2 Solution |
|-----------|-----------|-------------|
| "Type mismatch" gives no context | No tracking of *why* a type is expected | `Expected<T>` context encoding (Section 04) |
| Unknown identifiers have no suggestions | No edit-distance computation | `suggest` module (Section 01) |
| Type errors show full types, not diff | No structural comparison | `TypeDiff` (Section 05) |
| Error messages are flat strings | No composable document system | `ori_doc` trees (Section 03) |
| No "because..." explanations | No chain structure | `ExplanationChain` (Section 02) |
| `CodeFix` registry is empty | No implementations exist | Production fixes (Section 06) |
| No dependency graph context | No trace collection | Reference traces (Section 07) |
| Terminal output is basic | Rendering doesn't use document trees | Emitter v2 (Section 08) |

## Architecture Overview

```
                ┌─────────────────────────────────────────────┐
                │  ori_types (type-aware intelligence)         │
                │                                              │
                │  Expected<T>        WHY a type is expected   │
                │  TypeDiff           WHERE types diverge      │
                │  (enriches TypeCheckError rendering)         │
                └──────────────┬──────────────────────────────┘
                               │
                ┌──────────────▼──────────────────────────────┐
                │  ori_diagnostic (core + intelligence)        │
                │                                              │
                │  mod suggest:       Edit-distance matching   │
                │  mod doc:           Composable document tree │
                │  mod chain:         ExplanationChain         │
                │  struct Diagnostic: + related_info field     │
                │  mod fixes:         CodeFix trait (existing) │
                │  mod emitter:       Terminal v2 rendering    │
                └──────────────┬──────────────────────────────┘
                               │
                ┌──────────────▼──────────────────────────────┐
                │  oric (compiler integration)                  │
                │                                              │
                │  reporting/type_errors.rs:  Uses TypeDiff,   │
                │    Expected<T>, ori_doc for rich messages     │
                │  problem/semantic.rs:  Uses suggest for      │
                │    "did you mean?" on unknown identifiers     │
                │  fixes/:  Production CodeFix implementations │
                │  traces/:  Reference trace collection        │
                └──────────────────────────────────────────────┘
```

## Implementation Tiers

### Tier 1: Quick Wins (High Impact, Low Effort)
- Section 01: Suggest module — Damerau-Levenshtein edit distance — **~100 lines**
- Section 02: Enhanced Diagnostic types — chains + related info — **~200 lines**

### Tier 2: Intelligence Foundation (High Impact, Medium Effort)
- Section 03: ori_doc composable document system — **~500 lines**
- Section 04: Expected context encoding — **~300 lines**
- Section 05: TypeDiff structural type diffing — **~400 lines**

### Tier 3: Production Polish (Medium Impact, Medium Effort)
- Section 06: Production code fixes — **~600 lines** (20+ fix implementations)
- Section 07: Reference traces — **~300 lines**

### Tier 4: Rendering Integration
- Section 08: Terminal emitter v2 — **~400 lines** (renders ori_doc, type diffs)

## Implementation Progress

> Last updated: 2026-02-08. Not started.

| Section | Status | Crate |
|---------|--------|-------|
| 01 | Not Started | `ori_diagnostic` |
| 02 | Not Started | `ori_diagnostic` |
| 03 | Not Started | `ori_diagnostic` |
| 04 | Not Started | `ori_types` |
| 05 | Not Started | `ori_types` |
| 06 | Not Started | `ori_diagnostic` + `oric` |
| 07 | Not Started | `oric` |
| 08 | Not Started | `ori_diagnostic` |

## Dependencies

```
Section 01 (suggest)     — standalone, no dependencies
Section 02 (chains)      — standalone, no dependencies
Section 03 (ori_doc)     — standalone, no dependencies
Section 04 (Expected<T>) — depends on ori_types Pool/Idx
Section 05 (TypeDiff)    — depends on Section 03 (ori_doc) + ori_types Pool/Idx
Section 06 (code fixes)  — depends on Section 01 (suggest for "did you mean?")
Section 07 (traces)      — depends on Section 02 (chains for "because...")
Section 08 (emitter v2)  — depends on Section 03 (ori_doc) + Section 05 (TypeDiff rendering)
```

**Recommended execution order:** 01 → 02 → 03 → 04 → 05 → 06 → 07 → 08

Sections 01, 02, and 03 can be developed in parallel (no interdependencies).

## Relationship to Existing Proposals

- **structured-diagnostics-autofix** (approved 2026-01-28): V2 implements Steps 5-6 of that proposal (upgrade existing diagnostics, add extended fixes). Steps 1-4 are already complete.
- **llvm_v2 Section 15** (complete): Codegen diagnostics (E4xxx/E5xxx) are done. V2 does not touch codegen diagnostics.
- **v2-conventions §5** (Error Shape): V2 follows the WHERE+WHAT+WHY+HOW pattern, enriching the WHY (Expected context) and HOW (code fixes, suggestions).
