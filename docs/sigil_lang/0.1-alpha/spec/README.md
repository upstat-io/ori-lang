# Sigil Language Specification

This is the **formal language specification** for Sigil — the authoritative definition of what constitutes valid Sigil code.

| This Specification | Design Documentation |
|--------------------|---------------------|
| Defines WHAT the language IS | Explains WHY decisions were made |
| Formal, precise, normative | Tutorial, explanatory, informative |
| For implementers and tool authors | For learners and users |
| `spec/` | `design/` |

**Compilers, linters, formatters, and other tools MUST conform to this specification.**

## Contents

| Section | Description |
|---------|-------------|
| [01-notation](01-notation.md) | EBNF notation conventions |
| [02-source-code](02-source-code.md) | Source representation, encoding |
| [03-lexical-elements](03-lexical-elements.md) | Tokens, keywords, literals |
| [04-constants](04-constants.md) | Constant expressions, config variables |
| [05-variables](05-variables.md) | Variable bindings, mutability |
| [06-types](06-types.md) | Type system |
| [07-properties-of-types](07-properties-of-types.md) | Type identity, assignability |
| [08-declarations](08-declarations.md) | Functions, types, traits |
| [09-expressions](09-expressions.md) | Operators, conditionals, lambdas |
| [10-patterns](10-patterns.md) | Built-in patterns |
| [11-built-in-functions](11-built-in-functions.md) | Core functions |
| [12-modules](12-modules.md) | Module system |
| [13-testing](13-testing.md) | Mandatory testing |
| [14-capabilities](14-capabilities.md) | Effect system |

## Terminology

| Term | Meaning |
|------|---------|
| **must** | Absolute requirement |
| **must not** | Absolute prohibition |
| **should** | Recommendation |
| **may** | Optional behavior |
| **error** | Compile-time failure |
| **undefined** | Implementation-defined |

## Relationship to Other Documentation

This version (`0.1-alpha`) contains:

```
0.1-alpha/
├── spec/      ← You are here (normative)
├── design/    ← Rationale and philosophy
├── guide/     ← User tutorials
└── modules/   ← Standard library docs
```

See [Design Documentation](../design/00-index.md) for explanations of why features work the way they do.

## AI Guidance

See [CLAUDE.md](CLAUDE.md) for spec-specific writing guidance.

Synchronization rules are defined in `.claude/rules/sigil-lang-docs.md`.
