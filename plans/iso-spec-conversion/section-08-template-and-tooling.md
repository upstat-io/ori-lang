---
section: "08"
title: "Template and Tooling"
status: not-started
goal: "Update all tooling references, templates, and rules files to reflect ISO structure"
depends_on: ["07"]
sections:
  - id: "08.1"
    title: "Update _template.md"
    status: not-started
  - id: "08.2"
    title: "Update .claude/rules/spec.md"
    status: not-started
  - id: "08.3"
    title: "Update .claude/rules/ori-lang.md"
    status: not-started
  - id: "08.4"
    title: "Update .claude/rules/ori-syntax.md"
    status: not-started
  - id: "08.5"
    title: "Update CLAUDE.md References"
    status: not-started
  - id: "08.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Template and Tooling

**Status:** Not Started
**Goal:** All templates, rules files, and documentation references updated to reflect the ISO clause structure, verbal forms, and annex organization.

**Context:** The spec conversion changes file names, clause numbers, verbal forms, and structural conventions. Several tooling files reference the spec directly and must be updated to stay in sync.

**Depends on:** Section 07 (Renumber and Reorder) — needs final file names and clause numbers.

---

## 08.1 Update _template.md

**File(s):** `docs/ori_lang/0.1-alpha/spec/_template.md`

- [ ] Update template to use ISO conventions:
  ```markdown
  # N Section Title

  One-line definition.

  > **Grammar:** See [Annex A](annex-a-grammar.md) §A.SECTION_NAME

  ## N.1 Subsection

  Brief description. Technical terms in _italics_ on first use.
  Syntax in `backticks`.

  - Constraint using shall/may keywords.
  - Another constraint.

  EXAMPLE

  \`\`\`ori
  // Conforming
  valid_example()
  \`\`\`

  EXAMPLE  The following is not valid because [reason]:

  \`\`\`ori
  invalid_example()  // error: explanation
  \`\`\`

  NOTE  Clarifying information.

  ## N.2 Another Subsection

  See [Clause M](MM-related.md) for details.
  ```

- [ ] Replace `must`/`must not` with `shall`/`shall not` in template
- [ ] Replace `> **Note:**` with `NOTE` format
- [ ] Replace `Valid:` / `Invalid:` with `EXAMPLE` format
- [ ] Update grammar reference to use annex format

---

## 08.2 Update .claude/rules/spec.md

**File(s):** `.claude/rules/spec.md`

- [ ] Update verbal form list: `shall` instead of `must`
- [ ] Update section count: "27 numbered clauses (01-27) plus 4 preliminary clauses and 5 annexes"
- [ ] Update grammar/operator rules reference to annex format
- [ ] Update template reference
- [ ] Update checklist to include annex and cross-reference conventions

---

## 08.3 Update .claude/rules/ori-lang.md

**File(s):** `.claude/rules/ori-lang.md`

- [ ] Update sync rules to reference new file names
- [ ] Update grammar reference to annex format
- [ ] Update operator-rules reference to annex format

---

## 08.4 Update .claude/rules/ori-syntax.md

**File(s):** `.claude/rules/ori-syntax.md`

- [ ] Update spec reference at top: "Spec is authoritative: `docs/ori_lang/0.1-alpha/spec/` (clauses 1–27, annexes A–E)"
- [ ] Verify all section cross-references use new clause numbers

---

## 08.5 Update CLAUDE.md References

**File(s):** `CLAUDE.md` (project root)

- [ ] Update key paths section if spec file paths changed
- [ ] Update any spec section references to use new clause numbers

---

## 08.6 Completion Checklist

- [ ] `_template.md` uses ISO conventions (shall, NOTE, EXAMPLE, annex refs)
- [ ] `.claude/rules/spec.md` reflects ISO structure
- [ ] `.claude/rules/ori-lang.md` uses new file names
- [ ] `.claude/rules/ori-syntax.md` uses new clause numbers
- [ ] `CLAUDE.md` references updated
- [ ] No stale file names in any rules/tooling file

**Exit Criteria:** `grep -rn '01-notation\|06-types\|11-built-in\|16-formatting\|22-system' .claude/rules/ CLAUDE.md` returns 0 (no references to old file names).
