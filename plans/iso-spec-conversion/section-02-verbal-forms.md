---
section: "02"
title: "Verbal Forms"
status: not-started
goal: "Convert all normative verbal forms to ISO/IEC conventions (must→shall)"
depends_on: []
sections:
  - id: "02.1"
    title: "Verbal Form Mapping"
    status: not-started
  - id: "02.2"
    title: "Conversion Rules"
    status: not-started
  - id: "02.3"
    title: "Apply to All Spec Files"
    status: not-started
  - id: "02.4"
    title: "Update Terminology Tables"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Verbal Forms

**Status:** Not Started
**Goal:** Every normative requirement in the spec uses ISO verbal forms (`shall`/`shall not`/`should`/`may`), with no remaining instances of `must`/`must not` used as requirements.

**Context:** ISO/IEC Directives, Part 2 (Tables 3–7) mandate specific verbal forms. The most impactful change: `must` is reserved for external constraints (laws, physics) — requirements within the document use `shall`. The current spec uses `must` (~160 occurrences) for requirements, which is the Go specification convention but not ISO.

**Reference implementations:**
- **ISO/IEC Directives, Part 2**, Tables 3–7: Verbal form definitions
- **ISO/IEC 9899 (C)**: Uses `shall` throughout for requirements
- **ECMA-334 (C#)**: Uses `shall` throughout

**Depends on:** Nothing — can be done independently and in parallel with Section 04.

---

## 02.1 Verbal Form Mapping

The ISO verbal form system:

| ISO Form | Meaning | Current Ori Usage | Change Required |
|----------|---------|-------------------|-----------------|
| **shall** | Requirement | `must` | `must` → `shall` |
| **shall not** | Prohibition | `must not` | `must not` → `shall not` |
| **should** | Recommendation | Rarely used | Keep as-is |
| **should not** | Discouraged | Not used | No change |
| **may** | Permission | Used correctly | Keep as-is |
| **can** | Possibility/capability | Not distinguished | Audit; use `can` only for capability |

**Key distinction**: `must` in ISO means "external constraint the document cannot control" (e.g., "implementations must comply with applicable export regulations"). Within the spec's own rules, `shall` is correct.

- [ ] Document the mapping in a conversion guide for future spec authors

---

## 02.2 Conversion Rules

Not every `must` is a simple substitution. Context matters:

- [ ] **Direct substitution** (~90% of cases):
  ```
  BEFORE: The return type must match the declared type.
  AFTER:  The return type shall match the declared type.
  ```

- [ ] **Negative form**:
  ```
  BEFORE: Variables must not shadow constants.
  AFTER:  Variables shall not shadow constants.
  ```

- [ ] **Exceptions — keep `must` if it refers to external constraints**:
  - "Source files must be valid UTF-8" — this is arguably an external constraint on input; however, ISO convention would still use `shall` since this IS the spec's own requirement. Convert it.

- [ ] **Do NOT convert**:
  - `must` in code comments (`// error: return type must match`)
  - `must` in error message strings
  - `must` in prose describing user-facing compiler output

---

## 02.3 Apply to All Spec Files

**File(s):** All `docs/ori_lang/0.1-alpha/spec/*.md` files

Files ranked by occurrence count (highest first):

| File | `must` count | Notes |
|------|-------------|-------|
| `10-patterns.md` | 21 | Highest — many constraints |
| `09-expressions.md` | 19 | |
| `06-types.md` | 14 | |
| `08-declarations.md` | 11 | |
| `14-capabilities.md` | 10 | |
| `11-built-in-functions.md` | 10 | |
| `07-properties-of-types.md` | 9 | |
| `15-memory-model.md` | 8 | |
| `13-testing.md` | 7 | |
| Others | ~40 total | |

- [ ] Convert each file, verifying context for each `must` occurrence
- [ ] Do NOT blindly search-and-replace — check each occurrence is a normative requirement
- [ ] Preserve `must` in code blocks and error messages

---

## 02.4 Update Terminology Tables

The spec defines its own terminology in two places:

**`README.md` terminology table:**
```markdown
| Term | Meaning |
|------|---------|
| must | Requirement |
| must not | Prohibition |
| may | Optional |
| error | Compile-time failure |
```

**`01-notation.md` terminology table:**
```markdown
| Term | Meaning |
|------|---------|
| must | Absolute requirement |
| must not | Absolute prohibition |
| may | Optional |
| error | Compile-time failure |
| panic | Run-time failure |
```

- [ ] Update both tables to use ISO verbal forms:
  ```markdown
  | Term | Meaning |
  |------|---------|
  | shall | Requirement (ISO/IEC Directives, Part 2) |
  | shall not | Prohibition |
  | should | Recommendation |
  | may | Permission |
  | can | Possibility or capability |
  | error | Compile-time failure |
  | panic | Run-time failure |
  ```

- [ ] Remove the duplicate — the terminology table should live in Clause 1 (Notation, now Clause 5) and not be repeated in README.md

---

## 02.5 Completion Checklist

- [ ] `grep -rn '\bmust\b' docs/ori_lang/0.1-alpha/spec/*.md` returns only occurrences in code blocks or error messages — zero normative `must` remaining in prose
- [ ] `grep -rn '\bshall\b' docs/ori_lang/0.1-alpha/spec/*.md` returns ~160 matches (replacing former `must` uses)
- [ ] Terminology tables updated in all locations
- [ ] No `must not` in normative prose (all converted to `shall not`)
- [ ] Spot-check 5 files to verify context-appropriate conversion

**Exit Criteria:** Running `grep -Pn '(?<![`/])\bmust\b(?![`])' docs/ori_lang/0.1-alpha/spec/*.md` (excluding code/comments) returns 0 matches in normative prose.
