---
section: "01"
title: "Preliminary Clauses"
status: not-started
goal: "Create the four mandatory ISO front-matter clauses plus Foreword and Introduction"
depends_on: []
sections:
  - id: "01.1"
    title: "Foreword"
    status: not-started
  - id: "01.2"
    title: "Introduction"
    status: not-started
  - id: "01.3"
    title: "Clause 1 — Scope"
    status: not-started
  - id: "01.4"
    title: "Clause 2 — Normative References"
    status: not-started
  - id: "01.5"
    title: "Clause 3 — Terms and Definitions"
    status: not-started
  - id: "01.6"
    title: "Clause 4 — Conformance"
    status: not-started
  - id: "01.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Preliminary Clauses

**Status:** Not Started
**Goal:** Four mandatory ISO preliminary clauses (Scope, Normative References, Terms and Definitions, Conformance) exist as dedicated files, plus Foreword and Introduction as unnumbered front-matter. Content extracted from existing `index.md` and `README.md` where applicable.

**Context:** ISO/IEC Directives, Part 2 mandates that every standards document begin with Scope (Clause 1), Normative References (Clause 2), and Terms and Definitions (Clause 3). The C standard adds Conformance as Clause 4; ECMA-334 (C#) uses the same pattern. The current Ori spec has conformance buried in 4 lines of `index.md` and no dedicated scope, references, or terms clause.

**Reference implementations:**
- **ISO/IEC 9899 (C)**: Clauses 1–4 — Scope, Normative references, Terms/definitions/symbols, Conformance
- **ECMA-334 (C#)**: §1–§5 — Scope, Normative references, Terms and definitions, General description, Conformance
- **ISO/IEC 14882 (C++)**: Clauses 1–4 — same pattern

**Depends on:** Nothing — this can start immediately.

---

## 01.1 Foreword

**File(s):** `docs/ori_lang/v2026/spec/foreword.md` (new)

The Foreword is unnumbered front-matter. In ISO standards it identifies the issuing body, edition history, and relationship to prior versions. For Ori's alpha spec, this is lightweight.

- [ ] Create `foreword.md` with YAML frontmatter:
  ```yaml
  ---
  title: "Foreword"
  description: "Ori Language Specification — Foreword"
  order: -2
  section: "Front Matter"
  ---
  ```

- [ ] Content to include:
  - Identification: "This document specifies the Ori programming language, version 2026."
  - Status: "This is an alpha specification. Breaking changes are expected."
  - Structure guide: "Clauses 1–4 define scope, references, terminology, and conformance. Clauses 5–27 define the language. Annexes A–E provide supplementary material."
  - Notation: "In this document, clauses and subclauses are numbered hierarchically (e.g., §8.3.2)."

---

## 01.2 Introduction

**File(s):** `docs/ori_lang/v2026/spec/introduction.md` (new, replaces `index.md`)

The Introduction is unnumbered informative front-matter. It provides the design philosophy and reading guidance currently in `index.md`.

- [ ] Create `introduction.md` with YAML frontmatter:
  ```yaml
  ---
  title: "Introduction"
  description: "Ori Language Specification — Introduction"
  order: -1
  section: "Front Matter"
  ---
  ```

- [ ] Move design philosophy content from `index.md`:
  - "Lean Core, Rich Libraries" principle
  - Core vs library table
  - Reading guide for the spec

- [ ] Remove `index.md` or convert it to a redirect/stub

---

## 01.3 Clause 1 — Scope

**File(s):** `docs/ori_lang/v2026/spec/01-scope.md` (new)

ISO Clause 1 defines what the document specifies and what it does not. Currently absent from the Ori spec.

- [ ] Create `01-scope.md` with content modeled on ISO/IEC 9899 §1:

  ```markdown
  # Scope

  This document specifies:

  - the representation of Ori programs;
  - the syntax and constraints of the Ori language;
  - the semantic rules for interpreting Ori programs;
  - the standard library types, traits, and functions available to
    conforming programs;
  - the restrictions and limits imposed by a conforming implementation.

  This document does not specify:

  - the mechanism by which Ori programs are compiled or executed;
  - the mechanism by which Ori programs receive input or produce output;
  - the size or complexity of an Ori program that exceeds the capacity
    of any specific implementation;
  - implementation-defined extensions.
  ```

---

## 01.4 Clause 2 — Normative References

**File(s):** `docs/ori_lang/v2026/spec/02-normative-references.md` (new)

Lists external standards referenced normatively by the spec. Ori depends on Unicode (for `str`/`char`) and IEEE 754 (for `float`).

- [ ] Create `02-normative-references.md` with references:
  - **The Unicode Standard** — character encoding for `str` and `char` types
  - **IEEE 754 / IEC 60559** — floating-point arithmetic for `float` type
  - **LLVM Language Reference** — target IR semantics (if applicable to normative spec)

- [ ] Use ISO reference format:
  ```markdown
  The following documents are referred to in the text in such a way that
  some or all of their content constitutes requirements of this document.

  - **The Unicode Standard, Version 15.0** — Unicode Consortium.
    Character encoding and properties for `str` and `char` types.

  - **IEC 60559:2020** — *Floating-point arithmetic*.
    Semantics of the `float` type.
  ```

---

## 01.5 Clause 3 — Terms and Definitions

**File(s):** `docs/ori_lang/v2026/spec/03-terms-and-definitions.md` (new)

See Section 03 of this plan for the full extraction process. This clause collects all defined terms used across the spec.

- [ ] Create `03-terms-and-definitions.md` with stub structure
- [ ] Populate with terms extracted in Section 03 of this plan
- [ ] Use ISO entry format:
  ```markdown
  ## 3.1 expression

  syntactic construct that computes a value (Clause 14)

  ## 3.2 type

  classification determining the set of values and operations applicable
  to an entity (Clause 8)
  ```

---

## 01.6 Clause 4 — Conformance

**File(s):** `docs/ori_lang/v2026/spec/04-conformance.md` (new)

Expand the 4-line conformance section from `index.md` into a proper clause. Model on ISO 9899 §4.

- [ ] Create `04-conformance.md` with expanded content:
  - **Conforming implementation**: define what it means
  - **Conforming program**: define strictly conforming vs conforming
  - **Undefined behavior / unspecified behavior / implementation-defined behavior**: define each category (Ori may not need all three initially, but the framework should exist)
  - **Extensions**: "A conforming implementation may have extensions, provided they do not alter the behavior of any conforming program."

- [ ] Migrate existing conformance text from `index.md`:
  ```
  Implementations shall:
  - Accept conforming programs
  - Reject non-conforming programs with diagnostics
  - Produce specified behavior

  Extensions shall not alter conforming program behavior.
  ```
  (Note the `must` → `shall` conversion here.)

---

## 01.7 Completion Checklist

- [ ] `foreword.md` exists with correct frontmatter and content
- [ ] `introduction.md` exists, contains design philosophy from `index.md`
- [ ] `01-scope.md` exists with scope definition
- [ ] `02-normative-references.md` exists with Unicode and IEEE 754 references
- [ ] `03-terms-and-definitions.md` exists with stub (populated by Section 03)
- [ ] `04-conformance.md` exists with expanded conformance definitions
- [ ] `index.md` either removed or converted to redirect stub
- [ ] No content lost from `index.md` — all migrated to new files

**Exit Criteria:** Six new files created. Running `grep -r "conforming" docs/ori_lang/v2026/spec/04-conformance.md` returns matches. The `index.md` design philosophy content appears verbatim in `introduction.md`.
