---
paths:
  - "**spec**"
---

# Ori Language Specification

Style: ISO/IEC Directives, Part 2. Sync rules in `.claude/rules/ori-lang.md`.

## Spec vs Design
- Specification: what IS valid Ori (normative, formal)
- Design (`../design/`): explains WHY (tutorial tone)

**Never tutorial language. Never "you" or "best practice".**

## Writing Style
- **DO**: declarative sentences, _italics_ terms, `backticks` syntax, "X shall be Y"
- **DON'T**: "you can", rhetorical questions, motivation, verbose

## Normative Keywords (ISO/IEC Directives, Part 2)
- `shall` — requirement
- `shall not` — prohibition
- `should` — recommendation
- `may` — permission
- `can` — possibility or capability
- `error` — compile-time failure
- `panic` — run-time failure

## Notes and Examples
- Notes: `NOTE  Text.` (informative, no requirements)
- Numbered: `NOTE 1  `, `NOTE 2  ` when multiple per subclause
- Examples: `EXAMPLE`, `EXAMPLE 1`, `EXAMPLE 2` (informative)
- Notes and examples shall not contain `shall` requirements

## Grammar & Operator Rules
- `grammar.ebnf` — syntax (EBNF), referenced as Annex A
- `operator-rules.md` — semantics, referenced as Annex B

**Reference via annex:**
```markdown
> **Grammar:** See [Annex A](grammar.md) §A.SECTION_NAME
> **Rules:** See [Annex B](operator-rules.md) §B.OPERATOR_NAME
```

## EBNF Conventions
`snake_case` names | `"keyword"` tokens | `|` alt | `[ ]` opt | `{ }` repeat | `.` terminates

## Spec Structure
- Foreword + Introduction (unnumbered front matter)
- Clauses 1–4: Preliminary (Scope, Normative references, Terms, Conformance)
- Clauses 5–27: Language specification
- Annexes A–E: Grammar (norm.), Operator rules (norm.), Built-ins (norm.), Formatting (info.), System (info.)
- Bibliography

`README.md` lists all. `grammar.ebnf` and `operator-rules.md` are companion files.

## Clause Numbering
- H1: `# N Title`
- H2: `## N.M Subsection`
- H3: `### N.M.P Sub-subsection`

## Checklist
- Update `grammar.ebnf` if syntax changed
- Update `operator-rules.md` if operator changed
- Update `README.md` if adding/renaming sections
- Use `NOTE` for informative content (not `> **Note:**`)
- Use `EXAMPLE` for code examples (not `Valid:` / `Invalid:`)
- SYNC: design docs, guide, modules

## Template
See `docs/ori_lang/v2026/spec/_template.md`
