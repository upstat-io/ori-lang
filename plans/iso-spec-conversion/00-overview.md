---
plan: "iso-spec-conversion"
title: "ISO/IEC Spec Format Conversion: Exhaustive Implementation Plan"
status: not-started
references:
  - "docs/ori_lang/0.1-alpha/spec/"
  - "https://www.iso.org/sites/directives/current/part2/index.xhtml"
  - "https://github.com/dotnet/csharpstandard (ECMA-334)"
---

# ISO/IEC Spec Format Conversion: Exhaustive Implementation Plan

## Mission

Convert the Ori Language Specification from its current Go-inspired flat Markdown format to the standard ISO/IEC document structure used by ISO/IEC 9899 (C), ECMA-334/ISO/IEC 23270 (C#), and ISO/IEC 14882 (C++). The spec content remains unchanged — this is a structural, terminological, and organizational conversion, not a content rewrite.

## Architecture

```
BEFORE (Go-style flat):                    AFTER (ISO/IEC structure):

index.md (overview+conformance)            foreword.md
README.md (TOC)                            introduction.md
01-notation.md                             01-scope.md
02-source-code.md                          02-normative-references.md
03-lexical-elements.md                     03-terms-and-definitions.md
04-constants.md                            04-conformance.md
...                                        05-notation.md
27-reflection.md                           06-source-code.md
grammar.ebnf (inline)                      07-lexical-elements.md
operator-rules.md (inline)                 ...
                                           27-reflection.md
                                           annex-a-grammar.md (normative)
                                           annex-b-operator-rules.md (normative)
                                           annex-c-built-in-functions.md (normative)
                                           annex-d-formatting.md (informative)
                                           annex-e-system-considerations.md (informative)
                                           bibliography.md
```

## Design Principles

1. **Content preservation**: No semantic changes to the spec. Every rule, constraint, and example stays. This is a format migration, not a rewrite.

2. **ISO Directives Part 2 compliance**: Follow the structure and verbal forms mandated by ISO/IEC Directives, Part 2 for standards documents. Use `shall`/`shall not` for requirements, `NOTE`/`EXAMPLE` labeling, normative/informative distinction, and hierarchical clause numbering.

3. **Tooling compatibility**: YAML frontmatter is preserved — it serves the publishing pipeline and is orthogonal to spec content. The ISO structure lives in the content, not the metadata format.

## Section Dependency Graph

```
Section 01 (Preliminary Clauses) ──────────────────────┐
     │                                                  │
Section 02 (Verbal Forms) ─── can start independently   │
     │                                                  │
Section 03 (Terms & Definitions) ── needs content scan  │
     │                                                  │
Section 04 (Notes/Examples) ── can start independently  │
     │                                                  │
Section 05 (Cross-references) ── needs final numbering ─┤
     │                                                  │
Section 06 (Annexes) ── needs section 01 numbering ─────┤
     │                                                  │
Section 07 (Renumber & Reorder) ── depends on 01, 06 ──┘
     │
Section 08 (Template & Tooling Updates)
     │
Section 09 (Verification)
```

- Sections 02 and 04 are independent — can be done in any order or in parallel.
- Section 01 (preliminary clauses) and 06 (annexes) establish the structural skeleton.
- Section 07 (renumbering) depends on 01 and 06 being finalized.
- Section 05 (cross-references) must be done last before verification, after final numbering.
- Section 03 (terms) requires a full scan of all spec files to extract defined terms.

**Cross-section interactions:**
- **Section 01 + Section 07**: The new preliminary clauses consume clause numbers 1–4, which shifts all existing section numbers. Both must land together.
- **Section 06 + Section 07**: Extracting annexes removes files from the main sequence, affecting the renumbering.

## Implementation Sequence

```
Phase 0 - Preparation
  └─ 03: Scan all spec files, extract defined terms into inventory

Phase 1 - Independent text transforms (parallelizable)
  └─ 02: must → shall verbal form conversion (all files)
  └─ 04: Note/Example reformatting (all files)
  Gate: All verbal forms and note/example formatting pass audit

Phase 2 - Structural skeleton
  └─ 01: Create preliminary clauses (scope, normative refs, terms, conformance)
  └─ 06: Extract annexes (grammar, operator rules, built-ins, formatting, system)
  Gate: All new files exist, old files marked for removal

Phase 3 - Reorganization  [CRITICAL PATH]
  └─ 07: Renumber all clauses, reorder for ISO flow, update frontmatter
  └─ 05: Update all cross-references to use new clause numbers
  Gate: All internal links resolve, no broken references

Phase 4 - Finalization
  └─ 08: Update template, README, tooling references, .claude/rules
  └─ 09: Full verification pass
  Gate: No broken links, consistent numbering, verbal form audit clean
```

**Why this order:**
- Phase 0-1 are pure text transforms — no structural changes, easily reversible.
- Phase 2 creates the new skeleton files and identifies what moves to annexes.
- Phase 3 is the critical path because renumbering affects every file and every cross-reference simultaneously.
- Phase 4 ensures tooling and documentation stay in sync.

## Current Spec Inventory

| Metric | Count |
|--------|-------|
| Spec files (content) | 27 numbered + README + index + template |
| Companion files | grammar.ebnf, operator-rules.md |
| `must` occurrences to convert | ~160 |
| `> **Note:**` blockquotes | ~9 |
| Cross-references (inter-section links) | ~60 |
| Total spec size | ~450 KB |

## Estimated Effort

| Section | Complexity | Depends On |
|---------|------------|------------|
| 01 Preliminary Clauses | Medium | — |
| 02 Verbal Forms | Low | — |
| 03 Terms & Definitions | Medium | — |
| 04 Notes/Examples | Low | — |
| 05 Cross-references | Medium | 07 |
| 06 Annexes | Medium | — |
| 07 Renumber & Reorder | High | 01, 06 |
| 08 Template & Tooling | Low | 07 |
| 09 Verification | Medium | All |

## Clause Mapping (Current → ISO)

| New # | ISO Title | Current Source | Change Type |
|-------|-----------|---------------|-------------|
| — | Foreword | *New* | Create |
| — | Introduction | `index.md` reworked | Rework |
| 1 | Scope | *New* (from `index.md`) | Create |
| 2 | Normative references | *New* | Create |
| 3 | Terms and definitions | *New* (extracted) | Create |
| 4 | Conformance | *New* (from `index.md`) | Create |
| 5 | Notation | `01-notation.md` | Renumber |
| 6 | Source code | `02-source-code.md` | Renumber |
| 7 | Lexical elements | `03-lexical-elements.md` | Renumber |
| 8 | Types | `06-types.md` | Renumber + reorder |
| 9 | Properties of types | `07-properties-of-types.md` | Renumber |
| 10 | Declarations | `08-declarations.md` | Renumber |
| 11 | Blocks and scope | `17-blocks-and-scope.md` | Renumber + reorder |
| 12 | Constants | `04-constants.md` | Renumber + reorder |
| 13 | Variables | `05-variables.md` | Renumber + reorder |
| 14 | Expressions | `09-expressions.md` | Renumber |
| 15 | Patterns | `10-patterns.md` | Renumber |
| 16 | Control flow | `19-control-flow.md` | Renumber + reorder |
| 17 | Errors and panics | `20-errors-and-panics.md` | Renumber |
| 18 | Modules | `12-modules.md` | Renumber + reorder |
| 19 | Testing | `13-testing.md` | Renumber |
| 20 | Capabilities | `14-capabilities.md` | Renumber |
| 21 | Memory model | `15-memory-model.md` | Renumber |
| 22 | Concurrency model | `23-concurrency-model.md` | Renumber + reorder |
| 23 | Program execution | `18-program-execution.md` | Renumber + reorder |
| 24 | Constant expressions | `21-constant-expressions.md` | Renumber + reorder |
| 25 | Conditional compilation | `25-conditional-compilation.md` | Same |
| 26 | Foreign function interface | `24-ffi.md` | Renumber + reorder |
| 27 | Reflection | `27-reflection.md` | Same |
| A | Formal grammar (normative) | `grammar.ebnf` | Annex wrapper |
| B | Operator rules (normative) | `operator-rules.md` | Annex wrapper |
| C | Built-in functions (normative) | `11-built-in-functions.md` | Move to annex |
| D | Formatting (informative) | `16-formatting.md` | Move to annex |
| E | System considerations (informative) | `22-system-considerations.md` | Move to annex |
| — | Bibliography | *New* | Create |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Preliminary Clauses | `section-01-preliminary-clauses.md` | Not Started |
| 02 | Verbal Forms | `section-02-verbal-forms.md` | Not Started |
| 03 | Terms and Definitions | `section-03-terms-and-definitions.md` | Not Started |
| 04 | Notes and Examples | `section-04-notes-and-examples.md` | Not Started |
| 05 | Cross-references | `section-05-cross-references.md` | Not Started |
| 06 | Annexes | `section-06-annexes.md` | Not Started |
| 07 | Renumber and Reorder | `section-07-renumber-and-reorder.md` | Not Started |
| 08 | Template and Tooling | `section-08-template-and-tooling.md` | Not Started |
| 09 | Verification | `section-09-verification.md` | Not Started |
