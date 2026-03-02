---
plan: "spec-depth"
title: "Specification Depth: Fill Gaps to Go-Spec Level"
status: not-started
references:
  - "docs/ori_lang/v2026/spec/"
  - "https://go.dev/ref/spec"
  - "plans/iso-spec-conversion/"
---

# Specification Depth: Fill Gaps to Go-Spec Level

## Mission

Bring every clause of the Ori v2026 specification to the depth and precision of the Go language specification. The Go spec is the gold standard for a self-contained, implementor-grade language spec — a competent engineer can implement Go from the spec alone. The Ori spec currently reads as a feature catalogue (what exists) rather than a normative reference (how it works precisely, in every case). This plan closes that gap clause by clause.

## Architecture

```
Ori v2026 Specification
├── §6  Source Code            [SKELETAL → Full]     Section 01
├── §7  Lexical Elements       [LIGHT → Full]        Section 02
├── §12 Constants              [LIGHT → Full]     ┐
├── §13 Variables              [LIGHT → Full]     ├─ Section 03
├── §10 Declarations           [LIGHT → Full]     ┘
├── §11 Blocks and Scope       [LIGHT → Full]        Section 04
├── §14 Expressions            [MODERATE → Full]     Section 05
├── §16 Control Flow           [SEVERELY LIGHT]      Section 06
├── §23 Program Execution      [SEVERELY LIGHT]      Section 07
├── (new) Runtime Panics       [MISSING]              Section 07
├── Annex C Built-in Functions  [MODERATE → Full]     Section 08
├── (cross-cutting) Conversions [SCATTERED → Unified] Section 05
└── Verification                                      Section 09
```

## Design Principles

**Self-containment**: Every edge case should be resolvable from the spec alone, without consulting the compiler source. If a reader asks "what happens when X?" and the spec doesn't answer, that's a gap.

**Precision over brevity**: The Go spec uses more words to eliminate ambiguity. "A variable declaration creates one or more variables, binds corresponding identifiers to them, and gives each a type and an initial value" is more precise than "Variables are storage locations identified by name." Ori should match this.

**Edge cases are first-class**: Overflow, NaN, empty collections, zero-length strings, nil-equivalent values — these deserve explicit treatment, not "see §X" cross-references that lead to nothing.

## Section Dependency Graph

```
Section 01 (Source Code)
    ↓
Section 02 (Lexical Elements)
    ↓
Section 03 (Constants, Variables, Declarations)
    ↓
Section 04 (Blocks and Scope) ←──── Section 05 (Expressions)
    ↓                                     ↓
Section 06 (Control Flow) ───────→ Section 07 (Program Execution + Panics)
    ↓
Section 08 (Built-in Functions)
    ↓
Section 09 (Verification)
```

- Sections 01-02 are foundational — identifiers, literals, and tokens must be precisely defined before anything references them.
- Sections 03-04 can be worked somewhat in parallel.
- Section 05 (Expressions) is the largest and can be worked independently after 01-02.
- Section 06-07 depend on expression semantics being solid.
- Section 08 (Built-ins) depends on all prior sections for type and evaluation rules.
- Section 09 verifies the whole spec is internally consistent.

## Implementation Sequence

```
Phase 1 — Foundations
  └─ Section 01: §6 Source Code (~100 lines to add)
  └─ Section 02: §7 Lexical Elements (~500 lines to add)

Phase 2 — Binding Semantics
  └─ Section 03: §12 Constants, §13 Variables, §10 Declarations (~600 lines to add)
  └─ Section 04: §11 Blocks and Scope (~200 lines to add)

Phase 3 — Core Semantics
  └─ Section 05: §14 Expressions (~400 lines to add)
  └─ Section 06: §16 Control Flow (~800 lines to add)

Phase 4 — Execution Model
  └─ Section 07: §23 Program Execution + Runtime Panics (~400 lines to add)
  └─ Section 08: Annex C Built-in Functions (~300 lines to add)

Phase 5 — Verification
  └─ Section 09: Internal consistency, cross-reference audit
```

**Why this order:**
- Phase 1 establishes the lexical foundation everything else references.
- Phase 2 defines what bindings mean before expressions use them.
- Phase 3 is the bulk — expression and control flow semantics.
- Phase 4 ties it all together with execution semantics.
- Phase 5 catches inconsistencies introduced by the expansion.

## Estimated Effort

| Section | Target Clause(s) | Current Lines | Est. Lines to Add | Complexity |
|---------|-------------------|---------------|-------------------|------------|
| 01 Source Code | §6 | 47 | ~80 | Low |
| 02 Lexical Elements | §7 | 282 | ~500 | Medium |
| 03 Bindings & Declarations | §10, §12, §13 | 976 | ~600 | High |
| 04 Blocks and Scope | §11 | 240 | ~200 | Medium |
| 05 Expressions | §14 | 1115 | ~400 | High |
| 06 Control Flow | §16 | 295 | ~800 | High |
| 07 Program Execution | §23 + new | 95 | ~400 | Medium |
| 08 Built-in Functions | Annex C | 481 | ~300 | Medium |
| 09 Verification | — | 0 | ~50 | Low |
| **Total** | | **3531** | **~3300** | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Source Code | `section-01-source-code.md` | Not Started |
| 02 | Lexical Elements | `section-02-lexical-elements.md` | Not Started |
| 03 | Bindings and Declarations | `section-03-bindings-declarations.md` | Not Started |
| 04 | Blocks and Scope | `section-04-blocks-scope.md` | Not Started |
| 05 | Expressions | `section-05-expressions.md` | Not Started |
| 06 | Control Flow | `section-06-control-flow.md` | Not Started |
| 07 | Program Execution and Panics | `section-07-execution-panics.md` | Not Started |
| 08 | Built-in Functions | `section-08-builtins.md` | Not Started |
| 09 | Verification | `section-09-verification.md` | Not Started |
