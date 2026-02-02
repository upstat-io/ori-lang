# Ori Formatter v2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Token Spacing Rules
**File:** `section-01-token-spacing.md` | **Status:** Not Started

```
token, spacing, space, SpaceRule, SpaceAction
binary operator, unary operator, arrow, colon, comma
parentheses, brackets, braces, delimiters
no space, single space, preserve spacing
declarative rules, RulesMap, O(1) lookup
left token, right token, context function
```

---

### Section 02: Container Packing
**File:** `section-02-container-packing.md` | **Status:** Not Started

```
packing, Packing, container, Gleam-style
FitOrOnePerLine, FitOrPackMultiple, AlwaysOnePerLine, AlwaysStacked
trailing comma, comments, empty lines, user intent
simple items, complex items, width-based breaking
run, try, match, recurse, parallel, spawn, nursery
function params, generic params, struct fields, sum variants
list, map, tuple, import items, where constraints
```

---

### Section 03: Shape Tracking
**File:** `section-03-shape-tracking.md` | **Status:** Not Started

```
shape, Shape, width, indent, offset
max_width, indent_size, FormatterConfig
available width, remaining characters
consume, indent, fits, next_line
nested constructs, independent breaking
rustfmt-style, width tracking, recursion
```

---

### Section 04: Breaking Rules
**File:** `section-04-breaking-rules.md` | **Status:** Not Started

```
breaking rules, Ori-specific, BreakingRules
MethodChainRule, method chain, receiver, all methods break
ShortBodyRule, short body, threshold, yield, do
BooleanBreakRule, boolean, ||, or clauses, 3+ clauses
ChainedElseIfRule, else if, Kotlin style, each line
NestedForRule, nested for, Rust-style, indentation increment
ParenthesesRule, parentheses preservation, needs_parens
RunRule, top-level, nested, stacked vs width-based
LoopRule, complex body, run/try/match/for in loop
```

---

### Section 05: Formatter Orchestration
**File:** `section-05-formatter-orchestration.md` | **Status:** Not Started

```
Formatter, orchestration, main formatter
try-inline, format_broken, two-pass
emit, newline, indent, dedent
format_method_chain, format_for, format_if
blank lines, import ordering, module structure
trailing commas, normalize_blank_lines
```

---

### Section 06: Testing & Validation
**File:** `section-06-testing.md` | **Status:** Not Started

```
testing, validation, golden tests, idempotence
property tests, layer tests, integration tests
rule verification, packing tests, shape tests
round-trip, preserves semantics, no regression
spec compliance, formatting spec, 16-formatting.md
```

---

### Section 07: Integration & Polish
**File:** `section-07-integration.md` | **Status:** Not Started

```
integration, CLI, LSP, WASM
spec update, ChainedElseIfRule, Kotlin style
migration, incremental adoption, compatibility
performance, benchmarks, parallel processing
documentation, user guide, style guide
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Token Spacing Rules | `section-01-token-spacing.md` |
| 02 | Container Packing | `section-02-container-packing.md` |
| 03 | Shape Tracking | `section-03-shape-tracking.md` |
| 04 | Breaking Rules | `section-04-breaking-rules.md` |
| 05 | Formatter Orchestration | `section-05-formatter-orchestration.md` |
| 06 | Testing & Validation | `section-06-testing.md` |
| 07 | Integration & Polish | `section-07-integration.md` |
