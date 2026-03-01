---
paths:
  - "**spec**"
---

# Ori Language Specification

Style: [Go Language Specification](https://go.dev/ref/spec). Sync rules in `.claude/rules/ori-lang.md`.

## Spec vs Design
- Specification: what IS valid Ori (normative, formal)
- Design (`../design/`): explains WHY (tutorial tone)

**Never tutorial language. Never "you" or "best practice".**

## Writing Style
- **DO**: declarative sentences, _italics_ terms, `backticks` syntax, "X must be Y"
- **DON'T**: "you can", rhetorical questions, motivation, verbose

## Normative Keywords
- `must` — absolute requirement
- `must not` — absolute prohibition
- `should` — recommendation
- `may` — optional
- `error` — compile-time failure

## Grammar & Operator Rules
- `grammar.ebnf` — syntax (EBNF)
- `operator-rules.md` — semantics

**Reference, don't inline:**
```markdown
> **Grammar:** See [grammar.ebnf](...) SS SECTION_NAME
> **Rules:** See [operator-rules.md](...) SS OPERATOR_NAME
```

## EBNF Conventions
`snake_case` names | `"keyword"` tokens | `|` alt | `[ ]` opt | `{ }` repeat | `.` terminates

## Spec Files
27 numbered sections (01-27, skipping 26). `README.md` lists all. `grammar.ebnf` and `operator-rules.md` companion files.

## Checklist
- Update `grammar.ebnf` if syntax changed
- Update `operator-rules.md` if operator changed
- Update `README.md` if adding/renaming sections
- Mark informative: `> **Note:**`
- SYNC: design docs, guide, modules

## Template
See `docs/ori_lang/0.1-alpha/spec/_template.md`
