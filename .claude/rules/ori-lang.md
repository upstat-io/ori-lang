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
- Update `grammar.ebnf`
- Update ALL example code
- Update `.claude/rules/ori-syntax.md`

**If changing operator behavior:**
- Update `operator-rules.md`
- Verify: `ori_types/infer/expr/`, `ori_eval/interpreter/`

## Never Do
- Examples that don't match spec
- Update docs without updating `.claude/rules/ori-syntax.md`
