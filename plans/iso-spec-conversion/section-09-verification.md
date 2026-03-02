---
section: "09"
title: "Verification"
status: not-started
goal: "Full audit confirming ISO compliance across all spec files"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
sections:
  - id: "09.1"
    title: "Structural Audit"
    status: not-started
  - id: "09.2"
    title: "Verbal Form Audit"
    status: not-started
  - id: "09.3"
    title: "Cross-reference Integrity"
    status: not-started
  - id: "09.4"
    title: "Content Preservation"
    status: not-started
  - id: "09.5"
    title: "Documentation Sync"
    status: not-started
  - id: "09.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Verification

**Status:** Not Started
**Goal:** Complete audit confirms: all files follow ISO structure, verbal forms are correct, cross-references resolve, no content was lost, and all tooling is in sync.

**Context:** The ISO conversion touches every spec file. This section verifies no regressions — nothing lost, nothing broken, everything consistent.

**Depends on:** All other sections.

---

## 09.1 Structural Audit

- [ ] Verify mandatory preliminary clauses exist:
  - `foreword.md`
  - `introduction.md`
  - `01-scope.md`
  - `02-normative-references.md`
  - `03-terms-and-definitions.md`
  - `04-conformance.md`

- [ ] Verify clause numbering is contiguous (no gaps 1–27)

- [ ] Verify all annexes labeled:
  - `annex-a-grammar.md` — "(normative)"
  - `annex-b-operator-rules.md` — "(normative)"
  - `annex-c-built-in-functions.md` — "(normative)"
  - `annex-d-formatting.md` — "(informative)"
  - `annex-e-system-considerations.md` — "(informative)"

- [ ] Verify `bibliography.md` exists

- [ ] Verify all H1 headings include clause number: `grep -n '^# ' *.md`

- [ ] Verify all H2 headings include sub-clause number: `grep -n '^## ' *.md`

- [ ] Verify YAML frontmatter `order:` matches clause number in every file

---

## 09.2 Verbal Form Audit

- [ ] Zero `must` in normative prose:
  ```bash
  # Should return 0 matches outside code blocks and error messages
  grep -Pn '(?<![`/])\bmust\b(?![`])' docs/ori_lang/0.1-alpha/spec/*.md
  ```

- [ ] Confirm `shall` count is ~160 (matching former `must` count)

- [ ] Spot-check 10 `shall` usages across different files for correct context

- [ ] Verify no `shall` appears in NOTE or EXAMPLE elements (they are informative)

- [ ] Verify terminology table in Clause 5 (Notation) uses `shall`/`shall not`/`should`/`may`

---

## 09.3 Cross-reference Integrity

- [ ] Verify all internal markdown links resolve:
  ```bash
  # Check for links to files that don't exist
  grep -oP '\]\([^)]+\.md[^)]*\)' docs/ori_lang/0.1-alpha/spec/*.md | \
    while read -r link; do
      file=$(echo "$link" | sed 's/.*(\([^#)]*\).*/\1/')
      [ -f "docs/ori_lang/0.1-alpha/spec/$file" ] || echo "BROKEN: $link"
    done
  ```

- [ ] Verify no references to old file names:
  ```bash
  grep -rn '01-notation\|04-constants\|05-variables\|06-types\|07-properties' \
    docs/ori_lang/0.1-alpha/spec/*.md
  ```

- [ ] Verify grammar references point to Annex A

- [ ] Verify operator rule references point to Annex B

- [ ] Verify README TOC links all resolve

---

## 09.4 Content Preservation

The most critical check: no spec content was lost during conversion.

- [ ] Diff total line count before/after (should increase from new files, not decrease from lost content)

- [ ] Verify every original spec section's content exists in the renamed file:
  ```bash
  # For each old→new mapping, verify key content survived
  grep -c "EBNF" docs/ori_lang/0.1-alpha/spec/05-notation.md  # was 01-notation
  grep -c "trait" docs/ori_lang/0.1-alpha/spec/09-properties-of-types.md  # was 07
  # etc.
  ```

- [ ] Verify `index.md` design philosophy content appears in `introduction.md`

- [ ] Verify `index.md` conformance content appears in `04-conformance.md`

- [ ] Verify `grammar.ebnf` content unchanged

---

## 09.5 Documentation Sync

- [ ] `.claude/rules/spec.md` references new structure
- [ ] `.claude/rules/ori-lang.md` references new file names
- [ ] `.claude/rules/ori-syntax.md` references new clause numbers
- [ ] `_template.md` uses ISO conventions
- [ ] `CLAUDE.md` references updated

---

## 09.6 Completion Checklist

- [ ] Structural audit passes (all files exist, numbered correctly, headings numbered)
- [ ] Verbal form audit passes (zero `must` in normative prose)
- [ ] Cross-reference integrity (zero broken links)
- [ ] Content preservation (zero content loss)
- [ ] Documentation sync (all tooling files updated)
- [ ] Full spec is readable end-to-end (manual review)

**Exit Criteria:** All automated checks pass. Manual end-to-end reading confirms the spec is coherent, complete, and follows ISO/IEC Directives Part 2 structure. No content lost from pre-conversion spec.
