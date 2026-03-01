---
section: "05"
title: "Cross-references"
status: not-started
goal: "Convert all internal cross-references to use ISO clause number format"
depends_on: ["07"]
sections:
  - id: "05.1"
    title: "Reference Format"
    status: not-started
  - id: "05.2"
    title: "Inventory and Convert"
    status: not-started
  - id: "05.3"
    title: "Grammar/Operator Rule References"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Cross-references

**Status:** Not Started
**Goal:** All internal cross-references use ISO clause number format (`Clause 8`, `§8.3.2`) while retaining markdown links for navigability.

**Context:** The current spec uses markdown-style links: `See [Related Section](XX-related.md)`. ISO standards use clause number references: "as defined in Clause 8" or "see §8.3.2". Since the spec is published as markdown/HTML, we can have both — the clause number for ISO compliance, with the markdown link for navigation.

**Reference implementations:**
- **ISO/IEC 9899 (C)**: "as described in 6.5.3.2", "see 7.22"
- **ECMA-334 (C#)**: "as described in §12.8.7", "see §8.3"

**Depends on:** Section 07 (Renumber and Reorder) — final clause numbers must be settled first.

---

## 05.1 Reference Format

ISO cross-references follow these conventions:

```markdown
<!-- BEFORE -->
See [Types](06-types.md) for details.

<!-- AFTER -->
See [Clause 8](08-types.md) for details.
```

```markdown
<!-- BEFORE -->
See [Patterns](10-patterns.md) for core constructs.

<!-- AFTER -->
See [Clause 15](15-patterns.md) for pattern constructs.
```

For subclause references:
```markdown
<!-- BEFORE -->
See the [Newtypes](06-types.md#newtypes) section.

<!-- AFTER -->
See [§8.5](08-types.md#85-newtypes).
```

- [ ] Define the reference format: `[Clause N](filename.md)` for top-level, `[§N.M](filename.md#anchor)` for subclauses
- [ ] Use `Clause` (capitalized) when referring to a top-level clause
- [ ] Use `§` with number for subclause references
- [ ] Retain markdown link targets for navigation

---

## 05.2 Inventory and Convert

**File(s):** All spec files with cross-references (~60 occurrences across ~10 files)

Files ranked by cross-reference count:

| File | Count | Notes |
|------|-------|-------|
| `README.md` | 26 | TOC — will be regenerated in Section 07 |
| `08-declarations.md` | 9 | |
| `09-expressions.md` | 6 | |
| `10-patterns.md` | 4 | |
| `23-concurrency-model.md` | 3 | |
| `14-capabilities.md` | 3 | |
| `24-ffi.md` | 2 | |
| `20-errors-and-panics.md` | 2 | |
| `15-memory-model.md` | 2 | |
| `11-built-in-functions.md` | 2 | |

- [ ] Build a mapping table: old filename → new clause number (from Section 07)
- [ ] Convert all cross-references using the mapping
- [ ] Verify all links resolve after conversion

---

## 05.3 Grammar/Operator Rule References

The current spec references grammar and operator rules via GitHub URLs:

```markdown
> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § SECTION_NAME
```

In ISO format, these become annex references:

```markdown
> **Grammar:** See Annex A, §A.7
> **Rules:** See Annex B, §B.14
```

- [ ] Convert all grammar references to annex format
- [ ] Convert all operator-rules references to annex format
- [ ] Update the `_template.md` grammar reference format

---

## 05.4 Completion Checklist

- [ ] All cross-references use `Clause N` or `§N.M` format
- [ ] All markdown links still resolve (no broken links)
- [ ] Grammar references point to `Annex A`
- [ ] Operator rule references point to `Annex B`
- [ ] No remaining references to old filenames (e.g., `06-types.md` when it's now `08-types.md`)

**Exit Criteria:** A link-checking script finds 0 broken internal links. `grep -rn '\[[^]]*\]([0-9][0-9]-' docs/ori_lang/0.1-alpha/spec/*.md` shows all links use new numbering.
