---
name: sync-spec
description: Update the spec docs/ori_lang/v2026/spec follow spec format with the changes just made or user instructions
allowed-tools: Read, Grep, Glob, Edit, Write
---

# Update Ori Language Specification

Update the language specification at `docs/ori_lang/v2026/spec/` to reflect changes just made or follow user instructions.

## Target Directory

```
docs/ori_lang/v2026/spec/
```

## Spec Files

| File | Content |
|------|---------|
| `01-scope.md` | Language scope |
| `02-normative-references.md` | Normative references |
| `03-terms-and-definitions.md` | Terms and definitions |
| `04-conformance.md` | Conformance requirements |
| `05-notation.md` | Notation conventions, EBNF syntax |
| `06-source-code.md` | Source structure, Unicode |
| `07-lexical-elements.md` | Tokens, keywords, operators, literals, comments |
| `08-types.md` | Type syntax, generics, function types |
| `09-properties-of-types.md` | Type properties, traits |
| `10-declarations.md` | Functions, types, traits, impls, tests |
| `11-blocks-and-scope.md` | Scoping rules |
| `12-constants.md` | Config variables, const expressions |
| `13-variables.md` | Let bindings, assignment, destructuring |
| `14-expressions.md` | All expression forms |
| `15-patterns.md` | Match patterns, compiler patterns |
| `16-control-flow.md` | break, continue, loops |
| `17-errors-and-panics.md` | catch pattern, panic behavior |
| `18-modules.md` | Imports, re-exports, extensions |
| `19-testing.md` | Test declarations, attributes |
| `20-capabilities.md` | Uses clauses, with expressions |
| `21-memory-model.md` | ARC, ownership, reference semantics |
| `22-concurrency-model.md` | Concurrency model |
| `23-program-execution.md` | @main signatures |
| `24-constant-expressions.md` | Const functions |
| `25-conditional-compilation.md` | Target/config attributes |
| `26-ffi.md` | Foreign function interface |
| `27-reflection.md` | Compile-time reflection |
| `grammar.ebnf` | Formal grammar (single source of truth for syntax) |
| `operator-rules.md` | Formal operator semantics (type rules, eval rules, precedence) |

## Writing Style — CRITICAL

The spec is **formal, declarative, authoritative**. Follow the Go Language Specification style.

### DO Write
```markdown
An identifier is a sequence of letters, digits, and underscores.

The type of a binary expression `a + b` is determined by...

It is a compile-time error if the operand types are incompatible.

A function declaration introduces a new binding in the current scope.
```

### DO NOT Write
```markdown
You can use identifiers to name things.

When you write `a + b`, you get back...

Don't use incompatible types or you'll get an error.

Functions let you organize your code into reusable pieces.
```

### Key Rules

1. **No tutorial language** — Never use "you", "we", "let's", "useful for"
2. **Declarative sentences** — State what IS, not how to use it
3. **Technical precision** — Use exact terminology
4. **_Italics_** for technical terms on first use
5. **`Backticks`** for syntax elements
6. **Direct constraints** — "X must be Y", "It is an error if..."

### Normative Keywords

| Term | Meaning |
|------|---------|
| must | Absolute requirement |
| must not | Absolute prohibition |
| shall | Same as must |
| should | Recommendation |
| may | Optional |
| may not | Prohibited |
| error | Compile-time failure |

## Section Structure

```markdown
# Major Section

Brief normative introduction.

> **Grammar:** See [grammar.ebnf](grammar.md) § SECTION_NAME

## Subsection

### Semantics

Normative definitions here.

### Constraints

- It is an error if X.
- Y must satisfy Z.

### Examples

> **Note:** The following examples are informative.

\`\`\`ori
// example code
\`\`\`
```

## Grammar & Rules References

**Do not inline EBNF in spec files.** Reference the formal files:

```markdown
> **Grammar:** See [grammar.ebnf](grammar.md) § SECTION_NAME
> **Rules:** See [operator-rules.md](operator-rules.md) § OPERATOR_NAME
```

Where `SECTION_NAME` matches headers in grammar.ebnf (LEXICAL GRAMMAR, TYPES, DECLARATIONS, EXPRESSIONS, PATTERNS).
Where `OPERATOR_NAME` matches headers in operator-rules.md (Coalesce, Arithmetic, Comparison, etc.).

## Proposal Gate — MANDATORY

Spec files are protected by the proposal gate hook (`.claude/hooks/block-spec-edits.sh`). Before running this command, ensure:

1. An approved proposal exists in `docs/ori_lang/proposals/approved/`
2. Set the bypass: `export ORI_SPEC_PROPOSAL=<proposal-filename>.md`

Without this, all Edit/Write calls to spec files will be **blocked by the hook**. This is intentional — spec changes without approved proposals are never allowed. See `.claude/rules/spec.md` §Proposal Gate.

## Update Process

1. **Query the intelligence graph for affected symbols** — before identifying
   spec files, run a blast-radius check on every symbol the spec-edit might
   affect:

   @.claude/skills/dual-tpr/compose-intel-summary.md

   Query `callers` on symbols referenced in the spec change. If the edit
   changes operator-rules.md section X, run
   `scripts/intel-query.sh --human callers "<relevant symbol>" --repo ori`
   to see every site that interprets the rule. This prevents silent behavior
   drift when a spec change ships without updating an implementation call site.

2. **Identify affected spec files** based on what changed

3. **Read the relevant spec files** to understand current content

4. **Update spec content** following the formal style:
   - Add new sections for new language features
   - Update existing sections for modified behavior
   - Ensure constraints are listed in "Constraints" subsections
   - Mark informative content with `> **Note:**`

5. **Update operator-rules.md** if operator behavior changed (type rules, eval rules, precedence)

6. **Note grammar.ebnf** — if syntax changed, note that `/sync-grammar` needs to run (grammar.ebnf is owned by `/sync-grammar`, not this command)

7. **Verify cross-references** within spec files are accurate

## Specification vs Design Docs

| Specification (here) | Design (`../design/`) |
|---------------------|----------------------|
| Defines what IS valid Ori | Explains WHY decisions were made |
| Normative, authoritative | Informative, explanatory |
| Formal, precise language | Tutorial tone, best practices |
| "An identifier is..." | "You can use identifiers to..." |

## Checklist

- [ ] Used formal, declarative language (no "you", "we", "let's")
- [ ] Added grammar reference if syntax introduced
- [ ] Marked informative content with `> **Note:**`
- [ ] Listed constraints explicitly
- [ ] Updated cross-references
- [ ] Noted if grammar.ebnf needs updating (owned by `/sync-grammar`)
- [ ] Updated operator-rules.md if operator behavior changed

## Output

Report what was updated:
- Which spec files were modified
- Sections added or changed
- Whether grammar.ebnf needs updating (delegate to `/sync-grammar`)
- Whether operator-rules.md was updated
- Any cross-references updated
