---
paths:
  - "**/docs/ori_lang/**"
---

# Ori Documentation

## Sync Rules

**If `spec/` changed:**
- Sync to `.claude/rules/ori-syntax.md` if syntax/types/patterns affected
- Ask: "Create draft proposal?"

**If `.claude/rules/ori-syntax.md` changed:**
- Verify consistent with `spec/`
- If new feature, update spec first

**If changing syntax:**
- Update `grammar.ebnf` (Annex A companion)
- Update ALL example code
- Update `.claude/rules/ori-syntax.md`

**If changing operator behavior:**
- Update `operator-rules.md` (Annex B companion)
- Verify: `ori_types/infer/expr/`, `ori_eval/interpreter/`

**Spec conventions (ISO/IEC Directives, Part 2):**
- Use `shall`/`shall not` for requirements (not `must`)
- Use `NOTE` for informative text (not `> **Note:**`)
- Use `EXAMPLE` for code examples (not `Valid:`/`Invalid:`)
- Hierarchical clause numbers: `## N.M Subsection`

## Never Do
- Examples that don't match spec
- Update docs without updating `.claude/rules/ori-syntax.md`
