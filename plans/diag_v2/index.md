# Diagnostic V2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Edit-Distance Suggestions
**File:** `section-01-suggest.md` | **Status:** Not Started

```
suggest, suggestion, did you mean, typo, misspelling
edit distance, Levenshtein, Damerau-Levenshtein, string similarity
similar name, closest match, fuzzy match, approximate match
unknown identifier, unknown function, unknown field, unknown variant
Elm Suggest.hs, spell check, name resolution
threshold, max distance, candidate ranking, weighted edit
```

---

### Section 02: Enhanced Diagnostic Types
**File:** `section-02-enhanced-diagnostics.md` | **Status:** Not Started

```
ExplanationChain, message chain, because, reason, why
DiagnosticMessageChain, TypeScript pattern, nested because
RelatedInformation, cross-file context, secondary location
related span, provenance, context span, definition site
chain rendering, indented because, causal chain
Diagnostic struct, new fields, backward compatible
```

---

### Section 03: Composable Document System (ori_doc)
**File:** `section-03-ori-doc.md` | **Status:** Not Started

```
ori_doc, document tree, composable, pretty print, doc
RocDocAllocator, ven_pretty, BoxAllocator, Wadler
Doc combinators, concat, nest, group, line, text, annotate
Annotation enum, semantic markup, style decoupled
Palette, ANSI, HTML, CI, rendering, color, style
document builder, fluent API, type-aware rendering
alloc, arena, doc allocator, bump allocator
```

---

### Section 04: Expected Context Encoding
**File:** `section-04-expected-context.md` | **Status:** Not Started

```
Expected, ExpectedOrigin, FromContext, FromAnnotation, NoExpectation
context encoding, why expected, type expectation, annotation
Category, value category, what value IS, contextual
Elm Expected pattern, highest impact, error quality
ErrorContext enrichment, ContextKind extension
because annotation, because return type, because argument
conversational errors, contextual messages
```

---

### Section 05: Structural Type Diffing
**File:** `section-05-type-diff.md` | **Status:** Not Started

```
TypeDiff, type diff, structural diff, type comparison
to_diff, Roc pattern, recursive diff, best-in-class
where types diverge, highlight difference, diff rendering
type mismatch detail, expected vs found, structural
nested type diff, function signature diff, generic diff
DiffProblem, diff annotation, same prefix, divergence point
red/green diff, type alignment, visual diff
Pool, Idx, type representation, type traversal
```

---

### Section 06: Production Code Fixes
**File:** `section-06-code-fixes.md` | **Status:** Not Started

```
CodeFix, code fix, auto fix, quick fix, code action
fix registry, fix provider, registered fix, per error code
MachineApplicable, auto-apply, safe fix, ori fix
MaybeIncorrect, suggest fix, human verification
HasPlaceholders, placeholder, user input needed
TextEdit, CodeAction, Substitution, replacement
trailing comma, indentation, formatting fix
type cast, import suggestion, missing argument
did you mean fix, typo fix, rename suggestion
```

---

### Section 07: Reference Traces
**File:** `section-07-reference-traces.md` | **Status:** Not Started

```
reference trace, provenance, reached via, dependency
error path, how reached, through, dependency graph
Zig ResolvedReference, trace, chain of references
Salsa query graph, dependency tracking, incremental
import chain, re-export trace, transitive dependency
circular reference, cycle trace, mutual recursion
```

---

### Section 08: Terminal Emitter V2
**File:** `section-08-emitter-v2.md` | **Status:** Not Started

```
terminal emitter, rich rendering, ANSI, color
ori_doc rendering, document tree, pretty print terminal
type diff rendering, red green, structural diff display
chain rendering, because indentation, nested explanation
multi-line snippet, context lines, gutter, margin
Palette, color scheme, theme, accessibility
width-aware, terminal width, line wrapping, responsive
summary, error count, fixable count, fix available
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | Edit-Distance Suggestions | `section-01-suggest.md` | 1 |
| 02 | Enhanced Diagnostic Types | `section-02-enhanced-diagnostics.md` | 1 |
| 03 | Composable Document System | `section-03-ori-doc.md` | 2 |
| 04 | Expected Context Encoding | `section-04-expected-context.md` | 2 |
| 05 | Structural Type Diffing | `section-05-type-diff.md` | 2 |
| 06 | Production Code Fixes | `section-06-code-fixes.md` | 3 |
| 07 | Reference Traces | `section-07-reference-traces.md` | 3 |
| 08 | Terminal Emitter V2 | `section-08-emitter-v2.md` | 4 |
