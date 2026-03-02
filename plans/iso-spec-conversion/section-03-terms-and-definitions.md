---
section: "03"
title: "Terms and Definitions"
status: not-started
goal: "Extract all defined terms from spec into a formal ISO Terms and Definitions clause"
depends_on: []
sections:
  - id: "03.1"
    title: "Term Inventory"
    status: not-started
  - id: "03.2"
    title: "Entry Format"
    status: not-started
  - id: "03.3"
    title: "Populate Clause 3"
    status: not-started
  - id: "03.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Terms and Definitions

**Status:** Not Started
**Goal:** A dedicated Clause 3 (Terms and Definitions) exists containing all terms defined across the spec, formatted per ISO 10241-1, organized by conceptual hierarchy.

**Context:** ISO standards require a Terms and Definitions clause (Clause 3) that formally defines every technical term used in the document. Terms are formatted as numbered entries with the term, definition, and optional notes. Currently, Ori's defined terms are scattered across individual spec sections (italicized on first use) with no central glossary.

**Reference implementations:**
- **ISO/IEC 9899 (C)** §3: ~30 defined terms (argument, behavior, constraint, etc.)
- **ECMA-334 (C#)** §3: Terms and definitions clause
- **ISO/IEC 14882 (C++)** §3: ~50 defined terms

**Depends on:** Nothing — the scan can begin immediately.

---

## 03.1 Term Inventory

**File(s):** All `docs/ori_lang/v2026/spec/*.md` files

Scan all spec files for terms that are:
1. Italicized on first use (the current convention for defined terms)
2. Used with specific Ori-technical meaning (not general English)
3. Appear in terminology/keyword tables

- [ ] Grep for `_term_` patterns (markdown italics) across all spec files
- [ ] Extract candidate terms from section headings and definition paragraphs
- [ ] Cross-reference with the keyword list in `ori-syntax.md`

**Expected term categories:**

| Category | Example Terms |
|----------|--------------|
| Program structure | expression, statement, declaration, block, module, program |
| Types | type, primitive type, compound type, generic type, type parameter, type argument |
| Values | value, literal, constant, variable, binding, immutable binding |
| Functions | function, clause, parameter, argument, return type, variadic |
| Patterns | pattern, match, guard, exhaustive, irrefutable |
| Traits | trait, implementation, method, associated type, default implementation |
| Memory | reference count, ownership, clone, drop, ARC |
| Effects | capability, handler, effect |
| Testing | test, attached test, floating test, assertion |
| Execution | evaluation, compile-time, run-time, panic, error |

---

## 03.2 Entry Format

ISO 10241-1 specifies the format for terminological entries:

```markdown
## 3.N term_name

definition_text

NOTE 1  Optional clarifying note.

NOTE 2  Another note if needed.

[SOURCE: reference, modified — modification description]
```

Rules:
- [ ] Terms ordered by conceptual hierarchy (not alphabetically — ISO preference)
- [ ] Each definition is a single sentence without the defined term
- [ ] Notes are informative (marked `NOTE`)
- [ ] Cross-references to the clause where the term is fully specified

**Example entries for Ori:**

```markdown
## 3.1 expression

syntactic construct that computes a value (Clause 14)

NOTE  Every expression has a type determined at compile time.

## 3.2 type

classification determining the set of values and applicable operations
for an entity (Clause 8)

## 3.3 trait

named interface declaring a set of methods and associated types that
types may implement (Clause 10)

## 3.4 capability

named effect that a function declares it requires, enabling the caller
to provide or mock the implementation (Clause 20)

## 3.5 pattern

syntactic construct used to test the structure of a value and optionally
bind components to variables (Clause 15)
```

---

## 03.3 Populate Clause 3

**File(s):** `docs/ori_lang/v2026/spec/03-terms-and-definitions.md` (created in Section 01)

- [ ] Add introductory text:
  ```markdown
  For the purposes of this document, the following terms and definitions
  apply.

  ISO/IEC 2382 and IEEE 754 define terms used in this document that are
  not defined here.
  ```

- [ ] Add all extracted terms in hierarchical order
- [ ] Add clause cross-references for each term
- [ ] Verify no circular definitions (term A defined using term B which uses term A)

---

## 03.4 Completion Checklist

- [ ] Clause 3 file exists with at least 20 defined terms
- [ ] Terms organized by concept hierarchy, not alphabetically
- [ ] Every entry has: number, term, definition, clause reference
- [ ] No term is defined using itself
- [ ] Spot-check: 5 random terms verified against their defining clause

**Exit Criteria:** `03-terms-and-definitions.md` contains ≥20 formally defined terms, each with a clause cross-reference, formatted per ISO 10241-1 entry structure.
