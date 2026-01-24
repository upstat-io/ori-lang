# Proposal: Named-Argument `if` Syntax

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-23

---

## Summary

Change `if` from keyword-based syntax to a `function_exp` pattern with named arguments, making it consistent with other Sigil patterns like `map`, `filter`, and `fold`.

```sigil
// Current syntax
if x > 0 then "positive" else "negative"

// Proposed syntax
if(
    condition: x > 0,
    then: "positive",
    .else: "negative",
)
```

---

## Motivation

### The Problem

Sigil's `if` expression uses special keyword syntax:

```sigil
if condition then expr else expr
```

This is inconsistent with Sigil's pattern-based approach where constructs like `map`, `filter`, `fold`, and `match` use named arguments:

```sigil
map(
    over: items,
    transform: x -> x * 2,
)

filter(
    over: items,
    predicate: x -> x > 0,
)
```

The inconsistency means:
- Two different syntactic styles to learn
- `if` requires special keyword parsing
- AI/tooling must handle `if` differently from patterns
- Formatting rules differ (keywords inline vs. named args stacked)

### Sigil's Core Principles

1. **Explicit over implicit** — Named arguments (`condition:`, `then:`, `.else:`) are self-documenting
2. **AI-first / tooling-friendly** — One element per line enables surgical edits
3. **Consistency** — All constructs should follow the same patterns
4. **Predictable formatting** — Named arguments stack vertically per Sigil rules

### The Sigil Way

If `map`, `filter`, `fold`, and `match` use named arguments, `if` should too. This creates a uniform syntax where control flow and data transformation follow the same conventions.

---

## Design

### New Syntax

`if` becomes a `function_exp` pattern with three named arguments:

```sigil
if(
    condition: bool,
    then: T,
    .else: T,
) -> T
```

### Basic Examples

**Simple condition:**
```sigil
if(
    condition: x > 0,
    then: "positive",
    .else: "negative",
)
```

**With expressions:**
```sigil
if(
    condition: user.is_authenticated,
    then: show_dashboard(user),
    .else: redirect_to_login(),
)
```

**Nested in other expressions:**
```sigil
let message = if(
    condition: count == 0,
    then: "no items",
    .else: str(count) + " items",
)
```

### Else-If Chains

Nested `if` expressions in the `.else:` branch:

```sigil
if(
    condition: x > 0,
    then: "positive",
    .else: if(
        condition: x < 0,
        then: "negative",
        .else: "zero",
    ),
)
```

**Longer chain:**
```sigil
if(
    condition: status == "pending",
    then: handle_pending(),
    .else: if(
        condition: status == "active",
        then: handle_active(),
        .else: if(
            condition: status == "completed",
            then: handle_completed(),
            .else: handle_unknown(),
        ),
    ),
)
```

### Real-World Example

**Before (current syntax):**
```sigil
@binary_search (list: [int], target: int, low: int, high: int) -> int = run(
    let mid = (low + high) div 2,
    let mid_value = list[mid],
    if mid_value == target then mid
    else if mid_value > target then @binary_search(list, target, low, mid - 1)
    else @binary_search(list, target, mid + 1, high)
)
```

**After (proposed syntax):**
```sigil
@binary_search (list: [int], target: int, low: int, high: int) -> int = run(
    let mid = (low + high) div 2,
    let mid_value = list[mid],
    if(
        condition: mid_value == target,
        then: mid,
        .else: if(
            condition: mid_value > target,
            then: @binary_search(list, target, low, mid - 1),
            .else: @binary_search(list, target, mid + 1, high),
        ),
    ),
)
```

---

## Formatting Rules

Named arguments in `if` follow standard Sigil formatting:

1. **Always stack vertically** — Each named argument on its own line
2. **4-space indentation** — Standard Sigil indent
3. **Trailing commas** — Always on multi-line constructs
4. **Nested `if`** — Indent normally within `.else:`

**Correct:**
```sigil
if(
    condition: x > 0,
    then: "positive",
    .else: "negative",
)
```

**Incorrect (inline not allowed for named args):**
```sigil
if(condition: x > 0, then: "positive", .else: "negative")
```

---

## Keyword Changes

### Current Keywords

**Reserved:**
```
if, then, else, ...
```

### Proposed Keywords

**Reserved (removed):**
```
then, else  // No longer needed
```

**Context-sensitive (patterns):**
```
if, match, map, filter, fold, ...  // if joins the pattern keywords
```

The keyword `if` moves from reserved to context-sensitive, meaning it's only special within pattern contexts.

---

## Benefits

### 1. Consistency with Patterns

All control flow and data transformation use the same syntax:

```sigil
// Control flow
if(
    condition: expr,
    then: expr,
    .else: expr,
)

match(value,
    Pattern -> expr,
    _ -> default,
)

// Data transformation
map(
    over: items,
    transform: fn,
)

filter(
    over: items,
    predicate: fn,
)
```

### 2. AI-Friendly Editing

Each component is on its own line with a clear label:

```sigil
if(
    condition: x > 0,    // Line 1: condition
    then: "positive",    // Line 2: then branch
    .else: "negative",    // Line 3: else branch
)
```

AI can modify `then:` without touching other lines. Diffs are clean and readable.

### 3. Self-Documenting

Named arguments make intent explicit:
- `condition:` — This is the condition being tested
- `then:` — This executes when true
- `.else:` — This executes when false

No ambiguity about which part is which.

### 4. Simplified Grammar

Removes special-case parsing for `if-then-else` keywords. The grammar becomes more uniform — `if` is just another pattern with named arguments.

### 5. Formatting Consistency

Named arguments follow existing formatting rules. No special cases for how `if` expressions should be formatted.

---

## Drawbacks

### 1. More Verbose for Simple Cases

**Current (compact):**
```sigil
if x > 0 then x else -x
```

**Proposed (verbose):**
```sigil
if(
    condition: x > 0,
    then: x,
    .else: -x,
)
```

The proposed syntax is 5 lines vs. 1 line.

### 2. Unfamiliar to Newcomers

Most programmers expect `if-then-else` or `if { } else { }` syntax. The named-argument style may feel foreign initially.

### 3. Nested Conditionals Indent Deeply

Long else-if chains create deep nesting:

```sigil
if(
    condition: a,
    then: x,
    .else: if(
        condition: b,
        then: y,
        .else: if(
            condition: c,
            then: z,
            .else: default,
        ),
    ),
)
```

**Mitigation:** For complex branching, `match` is often clearer anyway.

---

## Alternatives Considered

### 1. Keep Current Syntax

Maintain `if cond then expr else expr`.

**Rejected because:** Inconsistent with pattern-based approach, requires special parsing, different formatting rules.

### 2. C-Style Braces

```sigil
if (x > 0) {
    "positive"
} else {
    "negative"
}
```

**Rejected because:** Sigil is expression-based, not statement-based. Braces imply statements.

### 3. Hybrid Approach

Named arguments but allow inline for simple cases:

```sigil
// Simple (inline allowed)
if(condition: x > 0, then: x, .else: -x)

// Complex (stacked)
if(
    condition: complex_expr,
    then: complex_result,
    .else: other_result,
)
```

**Rejected because:** Inconsistent with Sigil's rule that named arguments always stack vertically. Creates formatting ambiguity.

---

## Migration

### Code Changes

All existing `if-then-else` expressions must be rewritten:

**Before:**
```sigil
if x > 0 then x else -x
```

**After:**
```sigil
if(
    condition: x > 0,
    then: x,
    .else: -x,
)
```

### Tooling

`sigil fmt` would handle migration automatically during the transition period.

---

## Specification Changes

### Files to Update

1. **`spec/03-lexical-elements.md`**
   - Remove `then`, `else` from reserved keywords
   - Move `if` to context-sensitive keywords

2. **`spec/10-patterns.md`**
   - Add `if` to `function_exp` patterns list
   - Document `condition:`, `then:`, `.else:` arguments

3. **`spec/09-expressions.md`**
   - Update conditional expression grammar
   - Remove `if-then-else` production
   - Reference `if` pattern instead

4. **`design/02-syntax/03-patterns-overview.md`**
   - Add `if` to pattern examples
   - Explain rationale for consistency

5. **`CLAUDE.md`**
   - Update Expressions section
   - Update Keywords section
   - Update Patterns section

---

## Summary

This proposal changes `if` from keyword-based syntax to a `function_exp` pattern:

| Aspect | Current | Proposed |
|--------|---------|----------|
| Syntax | `if cond then expr else expr` | `if(condition:, then:, .else:)` |
| Keywords | `if`, `then`, `else` reserved | `if` context-sensitive |
| Formatting | Inline or wrapped | Always stacked (named args) |
| Consistency | Special case | Matches all patterns |

**Benefits:** Consistency, AI-friendly, self-documenting, simpler grammar.

**Costs:** More verbose, unfamiliar syntax, deeper nesting.

The change aligns `if` with Sigil's pattern-based philosophy: explicit, predictable, tooling-friendly.
