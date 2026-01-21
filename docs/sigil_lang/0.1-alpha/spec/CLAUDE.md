# AI Guidance for Sigil Specification

**IMPORTANT: Synchronization rules are defined in `.claude/rules/sigil-lang-docs.md`.**

Any spec change MUST be synchronized with design docs, guide, and module docs.

---

## This is a SPECIFICATION

| Specification (here) | Design (`../design/`) |
|---------------------|----------------------|
| Defines what IS valid Sigil | Explains WHY decisions were made |
| Normative, authoritative | Informative, explanatory |
| Formal, precise language | Tutorial tone, best practices |
| "An identifier is..." | "You can use identifiers to..." |

**Never use tutorial language. Never say "you" or "best practice".**

## Writing Style

### Do Use
```markdown
An identifier is a sequence of letters, digits, and underscores.

The type of a binary expression `a + b` is determined by...

It is a compile-time error if the operand types are incompatible.

A function declaration introduces a new binding in the current scope.
```

### Do Not Use
```markdown
You can use identifiers to name things.

When you write `a + b`, you get back...

Don't use incompatible types or you'll get an error.

Functions let you organize your code into reusable pieces.
```

## Grammar Productions

Always use EBNF notation:

```ebnf
production_name = expression .
```

Conventions:
- Production names in `snake_case`
- Terminals in double quotes: `"keyword"`
- Alternatives with `|`
- Optional with `[ ]`
- Repetition with `{ }`
- Grouping with `( )`

## Terminology

| Term | Meaning |
|------|---------|
| must | Absolute requirement |
| must not | Absolute prohibition |
| shall | Same as must |
| should | Recommendation |
| may | Optional |
| error | Compile-time failure |
| undefined | Implementation-defined |

## Section Structure

```markdown
# Major Section

Brief normative introduction.

## Subsection

### Grammar

\`\`\`ebnf
production = ... .
\`\`\`

### Semantics

Normative definitions here.

### Constraints

- It is an error if X.
- Y must satisfy Z.

### Examples

> **Note:** The following examples are informative.

\`\`\`sigil
// example code
\`\`\`
```

## Cross-References

```markdown
See [Types](06-types.md).
See [Expressions § Operators](09-expressions.md#operators).
See [Design: Error Handling](../design/05-error-handling/index.md).
```

## Checklist for Spec Changes

- [ ] Update grammar productions if syntax changed
- [ ] Use formal language throughout
- [ ] Mark informative sections with `> **Note:**`
- [ ] Update cross-references within spec
- [ ] **SYNC: Update corresponding design docs**
- [ ] **SYNC: Update guide if user-facing**
- [ ] **SYNC: Update modules if stdlib affected**

## Common Mistakes

1. **Tutorial language**: "You can..." → "A program may..."
2. **Missing grammar**: Syntax without EBNF production
3. **Unmarked informative content**: Always use `> **Note:**`
4. **Forgetting sync**: Spec change without design doc update
