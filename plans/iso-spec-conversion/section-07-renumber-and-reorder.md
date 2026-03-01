---
section: "07"
title: "Renumber and Reorder"
status: not-started
goal: "Renumber all clauses for ISO structure, reorder for logical flow, add hierarchical sub-clause numbering"
depends_on: ["01", "06"]
sections:
  - id: "07.1"
    title: "Final Clause Sequence"
    status: not-started
  - id: "07.2"
    title: "File Renaming"
    status: not-started
  - id: "07.3"
    title: "Frontmatter Updates"
    status: not-started
  - id: "07.4"
    title: "Hierarchical Sub-clause Numbering"
    status: not-started
  - id: "07.5"
    title: "README Regeneration"
    status: not-started
  - id: "07.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Renumber and Reorder

**Status:** Not Started
**Goal:** All spec files renumbered to reflect ISO clause structure (1–4 preliminary, 5+ technical, Annexes A–E). Files renamed to match. Frontmatter updated. README regenerated. Hierarchical sub-clause numbering applied within each file.

**Context:** This is the critical path section. The new preliminary clauses (Section 01) consume numbers 1–4, and annex extraction (Section 06) removes 3 files from the main sequence. All remaining files must be renumbered and potentially reordered for ISO-standard logical flow: types before expressions, declarations before patterns, control flow grouped with expressions, etc.

**Reference implementations:**
- **ISO/IEC 9899 (C)**: Logical flow — Environment → Language → Library
- **ECMA-334 (C#)**: Logical flow — Lexical → Basic concepts → Types → Variables → Conversions → Patterns → Expressions → Statements → Namespaces → Classes → ...

**Depends on:** Section 01 (Preliminary Clauses) and Section 06 (Annexes) must be settled first.

---

## 07.1 Final Clause Sequence

The definitive mapping from current to new clause numbers. This table is the single source of truth for the renumbering.

| New # | New Filename | Title | Old Source | Change |
|-------|-------------|-------|------------|--------|
| — | `foreword.md` | Foreword | *New* | — |
| — | `introduction.md` | Introduction | `index.md` | Rework |
| 01 | `01-scope.md` | Scope | *New* | Create |
| 02 | `02-normative-references.md` | Normative references | *New* | Create |
| 03 | `03-terms-and-definitions.md` | Terms and definitions | *New* | Create |
| 04 | `04-conformance.md` | Conformance | *New* | Create |
| 05 | `05-notation.md` | Notation | `01-notation.md` | Rename |
| 06 | `06-source-code.md` | Source code | `02-source-code.md` | Rename |
| 07 | `07-lexical-elements.md` | Lexical elements | `03-lexical-elements.md` | Rename |
| 08 | `08-types.md` | Types | `06-types.md` | Rename |
| 09 | `09-properties-of-types.md` | Properties of types | `07-properties-of-types.md` | Rename |
| 10 | `10-declarations.md` | Declarations | `08-declarations.md` | Rename |
| 11 | `11-blocks-and-scope.md` | Blocks and scope | `17-blocks-and-scope.md` | Rename+reorder |
| 12 | `12-constants.md` | Constants | `04-constants.md` | Rename+reorder |
| 13 | `13-variables.md` | Variables | `05-variables.md` | Rename+reorder |
| 14 | `14-expressions.md` | Expressions | `09-expressions.md` | Rename |
| 15 | `15-patterns.md` | Patterns | `10-patterns.md` | Rename |
| 16 | `16-control-flow.md` | Control flow | `19-control-flow.md` | Rename+reorder |
| 17 | `17-errors-and-panics.md` | Errors and panics | `20-errors-and-panics.md` | Rename+reorder |
| 18 | `18-modules.md` | Modules | `12-modules.md` | Rename+reorder |
| 19 | `19-testing.md` | Testing | `13-testing.md` | Rename |
| 20 | `20-capabilities.md` | Capabilities | `14-capabilities.md` | Rename |
| 21 | `21-memory-model.md` | Memory model | `15-memory-model.md` | Rename |
| 22 | `22-concurrency-model.md` | Concurrency model | `23-concurrency-model.md` | Rename |
| 23 | `23-program-execution.md` | Program execution | `18-program-execution.md` | Rename+reorder |
| 24 | `24-constant-expressions.md` | Constant expressions | `21-constant-expressions.md` | Rename+reorder |
| 25 | `25-conditional-compilation.md` | Conditional compilation | `25-conditional-compilation.md` | Same |
| 26 | `26-ffi.md` | Foreign function interface | `24-ffi.md` | Rename+reorder |
| 27 | `27-reflection.md` | Reflection | `27-reflection.md` | Same |
| A | `annex-a-grammar.md` | Formal grammar (normative) | `grammar.ebnf` wrapper | Section 06 |
| B | `annex-b-operator-rules.md` | Operator rules (normative) | `operator-rules.md` | Section 06 |
| C | `annex-c-built-in-functions.md` | Built-in functions (normative) | `11-built-in-functions.md` | Section 06 |
| D | `annex-d-formatting.md` | Formatting (informative) | `16-formatting.md` | Section 06 |
| E | `annex-e-system-considerations.md` | System considerations (informative) | `22-system-considerations.md` | Section 06 |
| — | `bibliography.md` | Bibliography | *New* | Section 06 |

**Reordering rationale:**
- Blocks/scope (was 17, now 11): Scoping rules belong with declarations, before expressions
- Constants/variables (were 04-05, now 12-13): Depend on types and declarations being defined
- Control flow (was 19, now 16): Groups with expressions and patterns
- Modules (was 12, now 18): Comes after all core language features
- Concurrency (was 23, now 22): Groups with memory model
- Program execution (was 18, now 23): Logical end of core language, before advanced features
- FFI (was 24, now 26): Moved after conditional compilation

---

## 07.2 File Renaming

- [ ] Create a rename script or execute renames in dependency order
- [ ] Use `git mv` to preserve history
- [ ] Handle files that don't move (25, 27) — no rename needed
- [ ] Handle files that move to annexes — already handled in Section 06
- [ ] Verify no orphaned files remain after renaming

**Rename commands** (execute in order):
```bash
# Phase 1: Move old files to temp names (avoid conflicts)
git mv 01-notation.md tmp-notation.md
git mv 02-source-code.md tmp-source-code.md
# ... etc for all files

# Phase 2: Move temp to final names
git mv tmp-notation.md 05-notation.md
git mv tmp-source-code.md 06-source-code.md
# ... etc
```

---

## 07.3 Frontmatter Updates

Every renamed file needs its YAML frontmatter updated:

```yaml
# BEFORE
---
title: "Notation"
description: "Ori Language Specification — Notation"
order: 1
section: "Foundations"
---

# AFTER
---
title: "Notation"
description: "Ori Language Specification — Clause 5: Notation"
order: 5
section: "Language"
---
```

- [ ] Update `order:` to match new clause number
- [ ] Update `description:` to include clause number
- [ ] Update `section:` categories to match ISO grouping:
  - "Front Matter": Foreword, Introduction
  - "Preliminary": Scope, Normative References, Terms, Conformance
  - "Language": Notation through Reflection (clauses 5–27)
  - "Annexes": Annexes A–E
  - "References": Bibliography

---

## 07.4 Hierarchical Sub-clause Numbering

Within each file, H2 and H3 headings should carry explicit hierarchical numbers.

Current:
```markdown
# Notation          ← H1, file-level
## Productions      ← H2, unnumbered
## Operators        ← H2, unnumbered
```

ISO format:
```markdown
# 5 Notation
## 5.1 Productions
## 5.2 Operators
### 5.2.1 Binary operators
```

- [ ] Add clause number to each H1 heading
- [ ] Add hierarchical numbers to each H2 (N.1, N.2, ...)
- [ ] Add hierarchical numbers to each H3 (N.M.1, N.M.2, ...)
- [ ] This is the most labor-intensive part — touch every heading in every file

**Approach**: process files one at a time, top to bottom, adding numbers. Use the TOC structure already implicit in the markdown headings.

---

## 07.5 README Regeneration

**File(s):** `docs/ori_lang/0.1-alpha/spec/README.md`

The README serves as the master table of contents. It must be regenerated to reflect the new structure.

- [ ] Regenerate the README with:
  - ISO-style clause listing (Foreword, Introduction, §1–§27, Annexes A–E, Bibliography)
  - Updated filenames
  - Updated section groupings
  - Updated terminology table (using `shall` verbal forms)

```markdown
# Ori Language Specification

## Clauses

| Clause | Title |
|--------|-------|
| — | [Foreword](foreword.md) |
| — | [Introduction](introduction.md) |
| §1 | [Scope](01-scope.md) |
| §2 | [Normative references](02-normative-references.md) |
| §3 | [Terms and definitions](03-terms-and-definitions.md) |
| §4 | [Conformance](04-conformance.md) |
| §5 | [Notation](05-notation.md) |
| ... | ... |
| §27 | [Reflection](27-reflection.md) |

## Annexes

| Annex | Title | Type |
|-------|-------|------|
| A | [Formal grammar](annex-a-grammar.md) | Normative |
| B | [Operator rules](annex-b-operator-rules.md) | Normative |
| C | [Built-in functions](annex-c-built-in-functions.md) | Normative |
| D | [Formatting](annex-d-formatting.md) | Informative |
| E | [System considerations](annex-e-system-considerations.md) | Informative |

## References

| | |
|---|---|
| — | [Bibliography](bibliography.md) |
```

---

## 07.6 Completion Checklist

- [ ] All files renamed to new numbering scheme
- [ ] All frontmatter `order:` values match clause numbers
- [ ] All H1 headings include clause number
- [ ] All H2/H3 headings include hierarchical sub-clause numbers
- [ ] README regenerated with complete ISO structure
- [ ] `git log --follow` confirms file history preserved
- [ ] No files with old numbering remain (except annexes which use letter prefix)

**Exit Criteria:** `ls docs/ori_lang/0.1-alpha/spec/` shows files numbered 01–27 plus foreword, introduction, annexes A–E, bibliography, grammar.ebnf, and README. All headings carry hierarchical numbers.
