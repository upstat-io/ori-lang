---
section: "06"
title: "Annexes"
status: not-started
goal: "Extract companion and supplementary material into properly labeled ISO annexes"
depends_on: []
sections:
  - id: "06.1"
    title: "Annex A — Formal Grammar (normative)"
    status: not-started
  - id: "06.2"
    title: "Annex B — Operator Rules (normative)"
    status: not-started
  - id: "06.3"
    title: "Annex C — Built-in Functions (normative)"
    status: not-started
  - id: "06.4"
    title: "Annex D — Formatting (informative)"
    status: not-started
  - id: "06.5"
    title: "Annex E — System Considerations (informative)"
    status: not-started
  - id: "06.6"
    title: "Bibliography"
    status: not-started
  - id: "06.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Annexes

**Status:** Not Started
**Goal:** Five annexes (A–E) exist with proper normative/informative labeling. Built-in functions, formatting, and system considerations moved from main clause sequence to annexes. Grammar and operator rules wrapped in annex structure. Bibliography file created.

**Context:** ISO standards use annexes for material that supports the main clauses but is either too large for inline inclusion (grammar) or supplementary (formatting guidelines, implementation notes). Each annex is explicitly labeled normative or informative. Currently, grammar.ebnf and operator-rules.md are companion files without annex structure, and sections 11 (built-ins), 16 (formatting), and 22 (system considerations) are inline clauses better suited as annexes.

**Reference implementations:**
- **ISO/IEC 9899 (C)**: Annexes A–M (A=Grammar informative, D=Limits normative, F=Floating-point normative, J=Portability informative)
- **ECMA-334 (C#)**: Annexes A–F (grammar, documentation comments, etc.)

**Depends on:** Nothing — can be created independently.

---

## 06.1 Annex A — Formal Grammar (normative)

**File(s):** `docs/ori_lang/v2026/spec/annex-a-grammar.md` (new wrapper), `grammar.ebnf` (unchanged)

The grammar.ebnf file stays as-is. The annex adds a wrapper with proper labeling and introductory text.

- [ ] Create `annex-a-grammar.md`:
  ```yaml
  ---
  title: "Annex A — Formal Grammar"
  description: "Ori Language Specification — Annex A (normative)"
  order: 100
  section: "Annexes"
  ---
  ```

- [ ] Content:
  ```markdown
  # Annex A (normative) — Formal Grammar

  This annex defines the complete formal grammar of the Ori language
  in Extended Backus-Naur Form (EBNF). The notation conventions are
  defined in Clause 5.

  The grammar file is: [grammar.ebnf](grammar.md)

  ## A.1 Lexical grammar
  ## A.2 Syntactic grammar
  ...
  ```

- [ ] Decide: inline the EBNF or keep as separate file with reference
  - **Recommended**: keep `grammar.ebnf` as separate file, reference from annex (matches current workflow, tools can parse EBNF directly)

---

## 06.2 Annex B — Operator Rules (normative)

**File(s):** `docs/ori_lang/v2026/spec/annex-b-operator-rules.md` (rename/wrap `operator-rules.md`)

- [ ] Rename or wrap `operator-rules.md` to `annex-b-operator-rules.md`
- [ ] Add annex heading:
  ```markdown
  # Annex B (normative) — Operator Rules

  This annex defines the typing and evaluation rules for all operators.
  ```
- [ ] Update YAML frontmatter with annex metadata
- [ ] Apply verbal form conversion (shall/shall not) if not already done

---

## 06.3 Annex C — Built-in Functions (normative)

**File(s):** Current `11-built-in-functions.md` → `annex-c-built-in-functions.md`

Built-in functions are normative (implementations shall provide them) but are better as an annex because they are a reference table, not core language syntax.

- [ ] Move `11-built-in-functions.md` to `annex-c-built-in-functions.md`
- [ ] Add annex heading:
  ```markdown
  # Annex C (normative) — Built-in Functions

  This annex defines the functions, methods, and types provided by the
  standard prelude.
  ```
- [ ] Update frontmatter
- [ ] Update all cross-references pointing to the old file

---

## 06.4 Annex D — Formatting (informative)

**File(s):** Current `16-formatting.md` → `annex-d-formatting.md`

Formatting rules (style guide) are informative — they guide `ori fmt` behavior but do not affect program semantics.

- [ ] Move `16-formatting.md` to `annex-d-formatting.md`
- [ ] Add annex heading:
  ```markdown
  # Annex D (informative) — Formatting

  This annex defines the formatting conventions applied by the `ori fmt`
  tool. These conventions are informative; non-conformance to formatting
  does not affect program validity.
  ```
- [ ] Update frontmatter
- [ ] Update cross-references

---

## 06.5 Annex E — System Considerations (informative)

**File(s):** Current `22-system-considerations.md` → `annex-e-system-considerations.md`

System considerations (platform behavior, optimization notes) are informative.

- [ ] Move `22-system-considerations.md` to `annex-e-system-considerations.md`
- [ ] Add annex heading:
  ```markdown
  # Annex E (informative) — System Considerations

  This annex describes implementation considerations for different
  target platforms and optimization levels.
  ```
- [ ] Update frontmatter
- [ ] Update cross-references

---

## 06.6 Bibliography

**File(s):** `docs/ori_lang/v2026/spec/bibliography.md` (new)

ISO standards end with a Bibliography listing informative (non-normative) references.

- [ ] Create `bibliography.md`:
  ```markdown
  # Bibliography

  The following documents are provided for informational purposes and
  are not normatively referenced by this document.

  [1] The Rust Programming Language — rust-lang.org
  [2] The Go Programming Language Specification — go.dev/ref/spec
  [3] Koka: Programming with Row-polymorphic Effect Types
  [4] Lean 4: The Lean 4 Reference Manual
  ```

---

## 06.7 Completion Checklist

- [ ] `annex-a-grammar.md` exists with normative label and grammar reference
- [ ] `annex-b-operator-rules.md` exists with normative label
- [ ] `annex-c-built-in-functions.md` exists (moved from `11-built-in-functions.md`)
- [ ] `annex-d-formatting.md` exists (moved from `16-formatting.md`) with informative label
- [ ] `annex-e-system-considerations.md` exists (moved from `22-system-considerations.md`) with informative label
- [ ] `bibliography.md` exists
- [ ] Old files (`11-built-in-functions.md`, `16-formatting.md`, `22-system-considerations.md`) removed
- [ ] No cross-references point to removed files

**Exit Criteria:** Six annex/bibliography files exist. Old files removed. `grep -rn '11-built-in\|16-formatting\|22-system' docs/ori_lang/v2026/spec/*.md` returns 0 (no stale references).
