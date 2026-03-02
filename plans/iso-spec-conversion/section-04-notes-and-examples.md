---
section: "04"
title: "Notes and Examples"
status: not-started
goal: "Convert all notes and examples to ISO labeling conventions"
depends_on: []
sections:
  - id: "04.1"
    title: "Note Format Conversion"
    status: not-started
  - id: "04.2"
    title: "Example Format Conversion"
    status: not-started
  - id: "04.3"
    title: "Normative/Informative Labeling"
    status: not-started
  - id: "04.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Notes and Examples

**Status:** Not Started
**Goal:** All notes use `NOTE` / `NOTE N` format. All code examples use `EXAMPLE` / `EXAMPLE N` labeling. Every informative element is clearly distinguished from normative text.

**Context:** ISO standards distinguish normative text (binding requirements) from informative elements (clarification). Notes and examples are always informative — they illustrate but do not impose requirements. The current spec uses `> **Note:**` blockquotes (9 occurrences) and unlabeled "Valid:" / "Invalid:" code blocks. ISO convention uses `NOTE` and `EXAMPLE` labels, numbered when multiple appear in the same subclause.

**Reference implementations:**
- **ISO/IEC 9899 (C)**: "NOTE — ...", "EXAMPLE 1", "EXAMPLE 2"
- **ECMA-334 (C#)**: "> *Note*: ...", "> *Example*: ..."
- **ISO/IEC Directives, Part 2**: Notes and examples are informative, shall not contain requirements

**Depends on:** Nothing — can be done in parallel with Section 02.

---

## 04.1 Note Format Conversion

**File(s):** All spec files containing `> **Note**` blockquotes

Current format (9 occurrences):
```markdown
> **Note:** Some clarifying information here.
```

ISO format:
```markdown
NOTE  Some clarifying information here.
```

Or when multiple notes appear in the same subclause:
```markdown
NOTE 1  First note.

NOTE 2  Second note.
```

- [ ] Convert all `> **Note:**` blockquotes to `NOTE` format
- [ ] Number notes when multiple appear in the same subclause
- [ ] Verify no note contains a `shall` requirement (notes are informative)

Files with notes to convert:
| File | Count |
|------|-------|
| `10-patterns.md` | 3 |
| `22-system-considerations.md` | 2 |
| `20-errors-and-panics.md` | 1 |
| `15-memory-model.md` | 1 |
| `09-expressions.md` | 1 |
| `06-types.md` | 1 |

---

## 04.2 Example Format Conversion

**File(s):** All spec files containing code blocks

Current format:
```markdown
Valid:

\`\`\`ori
@add (a: int, b: int) -> int = a + b;
\`\`\`

Invalid:

\`\`\`ori
@add (a: int, b: int) = a + b;  // error: missing return type
\`\`\`
```

ISO format:
```markdown
EXAMPLE 1

\`\`\`ori
@add (a: int, b: int) -> int = a + b;
\`\`\`

EXAMPLE 2  The following is not valid because the return type is omitted:

\`\`\`ori
@add (a: int, b: int) = a + b;  // error: missing return type
\`\`\`
```

- [ ] Replace "Valid:" / "Invalid:" labels with `EXAMPLE` / `EXAMPLE N`
- [ ] For invalid examples, add a brief explanatory sentence before the code block
- [ ] Number examples when multiple appear in the same subclause
- [ ] Preserve `// error:` comments within code blocks (these are part of the example)

**Decision point**: Some spec sections have many examples per subclause. Numbering all of them may be verbose. Options:
- **(a)** Number all examples strictly (ISO-compliant but verbose)
- **(b)** Number only when multiple examples appear in the same subclause (practical)

**Recommended:** Option (b) — number only when needed for disambiguation.

---

## 04.3 Normative/Informative Labeling

Beyond notes and examples, ensure the overall normative/informative distinction is clear:

- [ ] Add a statement in the Foreword or Introduction:
  ```
  In this document, notes and examples are informative and do not
  contain requirements. Normative text uses the verbal forms defined
  in Clause 5 (Notation).
  ```

- [ ] Audit for notes that accidentally contain requirements:
  - A note saying "implementations must..." is wrong — either move to normative text or rephrase as observation
  - `grep -n 'NOTE.*shall' docs/ori_lang/0.1-alpha/spec/*.md` should return 0

---

## 04.4 Completion Checklist

- [ ] Zero `> **Note:**` blockquotes remaining (all converted to `NOTE` format)
- [ ] Zero bare "Valid:" / "Invalid:" labels (all converted to `EXAMPLE` format)
- [ ] Notes numbered when multiple per subclause
- [ ] Examples numbered when multiple per subclause
- [ ] No note contains a `shall` requirement
- [ ] Normative/informative distinction stated in front-matter

**Exit Criteria:** `grep -rn '> \*\*Note' docs/ori_lang/0.1-alpha/spec/*.md` returns 0. All code examples preceded by `EXAMPLE` label.
