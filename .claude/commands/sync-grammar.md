---
name: sync-grammar
description: Update docs/ori_lang/v2026/spec/grammar.ebnf to match the spec
allowed-tools: Read, Grep, Glob, Edit, Write
---

# Update Grammar EBNF

Update `docs/ori_lang/v2026/spec/grammar.ebnf` to accurately reflect the current language specification.

## Overview

The grammar.ebnf file is the **single source of truth** for Ori's formal syntax. It must stay synchronized with the prose descriptions in the spec files.

## Target Files

```
docs/ori_lang/v2026/spec/grammar.ebnf      # Syntax (EBNF)
docs/ori_lang/v2026/spec/operator-rules.md # Semantics (type/eval rules)
```

**Note:** If operator precedence, associativity, or type rules change, update BOTH files.

## Source Files

Read these spec files to extract grammar information:

| File | Grammar Sections |
|------|------------------|
| `02-source-code.md` | Source structure, Unicode |
| `03-lexical-elements.md` | Tokens, keywords, operators, literals, comments |
| `04-constants.md` | Config variables, const expressions |
| `05-variables.md` | Let bindings, assignment, destructuring |
| `06-types.md` | Type syntax, generics, function types |
| `08-declarations.md` | Functions, types, traits, impls, tests |
| `09-expressions.md` | All expression forms |
| `10-patterns.md` | Match patterns, compiler patterns (run/try/match/etc.) |
| `12-modules.md` | Imports, re-exports, extensions |
| `13-testing.md` | Test declarations, attributes |
| `14-capabilities.md` | Uses clauses, with expressions |
| `18-program-execution.md` | @main signatures |
| `19-control-flow.md` | break, continue, loops |
| `20-errors-and-panics.md` | catch pattern |
| `21-constant-expressions.md` | Const functions |

## EBNF Notation

The grammar uses these conventions (matching the file header):

```
production = expression .     // Production definition (terminated with .)
"keyword"                     // Literal token
|                             // Alternation
[ ]                           // Optional (0 or 1)
{ }                           // Repetition (0 or more)
( )                           // Grouping
/* comment */                 // Informative note
```

Production names use `snake_case`.

## Update Process

1. **Read the current grammar.ebnf** to understand existing structure

2. **Read each spec file** and extract syntax definitions:
   - Look for code blocks showing syntax
   - Look for prose describing valid syntax forms
   - Note any syntax changes from recent spec updates

3. **Compare and identify discrepancies**:
   - Missing productions (spec has syntax not in grammar)
   - Outdated productions (grammar doesn't match spec)
   - Missing alternatives in existing productions
   - Incorrect cross-references in comments

4. **Update grammar.ebnf**:
   - Add new productions where needed
   - Update existing productions to match spec
   - Update section comments with correct file references
   - Maintain alphabetical order within sections where applicable

5. **Verify consistency**:
   - All productions referenced are defined
   - No orphan productions (defined but never referenced)
   - Cross-references to spec files are accurate

## Grammar Sections

The grammar.ebnf is organized into these sections:

| Section | Content |
|---------|---------|
| LEXICAL GRAMMAR | Characters, tokens, comments, identifiers, keywords, operators, literals |
| SOURCE STRUCTURE | Source file, imports, re-exports, extensions |
| DECLARATIONS | Functions, types, traits, impls, tests, config |
| TYPES | Type expressions, generics, function types |
| EXPRESSIONS | All expression forms, operators, control flow |
| PATTERNS | Match patterns, compiler patterns (run/try/match/etc.) |
| CONSTANT EXPRESSIONS | Compile-time expressions |
| PROGRAM ENTRY | @main function signatures |

## Key Things to Check

### Lexical Grammar
- All reserved keywords listed (check `03-lexical-elements.md`)
- Context-sensitive keywords (patterns) listed separately
- All operators with correct precedence comments
- All literal forms (int, float, string, char, bool, duration, size)

### Declarations
- Function syntax including generics, where clauses, uses clauses
- Type definition syntax (struct, sum, newtype)
- Trait syntax with associated types and default methods
- Impl block syntax (inherent and trait impls)
- Test syntax with attributes and targets

### Expressions
- All binary operators with correct precedence chain
- Postfix operators (field access, indexing, calls, `?`)
- Control flow (`if`, `for`, `loop`, `break`, `continue`)
- Lambda syntax (simple and typed)
- With expression for capabilities

### Patterns
- All match pattern forms (literal, binding, wildcard, variant, struct, tuple, list, range, or, at)
- Compiler patterns (run, try, match, for, catch, recurse, parallel, spawn, timeout, cache, with)
- Binding patterns for destructuring

## Example Update

If the spec adds a new pattern form like `timeout`:

1. Find the `pattern_name` production:
   ```
   pattern_name = "recurse" | "parallel" | "spawn" | "timeout" | "cache" | "with" .
   ```

2. Verify `timeout` is included (it already is in this case)

3. If the syntax changed, update the relevant production

4. Update comments if the spec file reference changed

## When to Update operator-rules.md

Update `operator-rules.md` alongside `grammar.ebnf` when:
- Operator precedence or associativity changes
- New operators added
- Operator type rules change (e.g., new type combinations)
- Operator evaluation semantics change
- Never type behavior changes

The `operator-rules.md` file contains:
- Type inference rules (premises → conclusion)
- Evaluation rules (pattern ⇒ result)
- Precedence table
- Trait dispatch mapping

## Output

After updating, report:
- Productions added/modified/removed (grammar.ebnf)
- Rules added/modified/removed (operator-rules.md)
- Any inconsistencies found between spec, grammar, and rules
