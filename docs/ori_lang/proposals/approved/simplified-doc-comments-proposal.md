# Proposal: Simplified Doc Comment Syntax

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-30
**Approved:** 2026-01-30

---

## Summary

Simplify doc comment syntax by removing the verbose `@param` and `@field` keywords. Use minimal markers that mirror familiar patterns:

```ori
// Before
// #Computes fibonacci using memoization.
// @param n The fibonacci index
// @param x Unused parameter
// @field name The user's name
// !Panics if n is negative
// >fib(n: 10) -> 55

// After
// Computes fibonacci using memoization.
// * n: The fibonacci index
// * x: Unused parameter
// * name: The user's name
// ! Panics if n is negative
// > fib(n: 10) -> 55
```

---

## Motivation

### The Problem

The current doc comment format has unnecessary verbosity:

1. **Redundant keywords** — `@param` and `@field` add noise without value
2. **`#` for description is pointless** — Unmarked comments are obviously descriptions
3. **Inconsistent with Ori's minimalism** — Other Ori syntax is clean and terse

### Current Format

```ori
// #Computes the sum of two integers.
// @param a The first operand.
// @param b The second operand.
// !Panics if overflow occurs.
// >add(a: 1, b: 2) -> 3
@add (a: int, b: int) -> int = a + b
```

Problems:
- `#` adds nothing — it's obviously a description
- `@param` is verbose — 6 characters of noise per parameter
- Doesn't mirror Ori syntax patterns

### Design Goals

1. **Minimal syntax** — The most common case (description) should have zero markers
2. **Familiar patterns** — Use `*` like markdown lists for params/fields
3. **Clear semantics** — Markers only where they add meaning

---

## Design

### Doc Comment Markers

| Marker | Meaning | Usage |
|--------|---------|-------|
| *(none)* | Description | `// This is a description.` |
| `*` | Param or Field | `// * name: Description` |
| `!` | Warning/Panic | `// ! Panics if x is negative` |
| `>` | Example | `// > func(x: 1) -> 2` |

### Syntax

```ebnf
doc_comment     = "//" [ " " ] [ doc_marker ] { unicode_char - newline } newline .
doc_marker      = "*" | "!" | ">" .
member_doc      = "//" " " "*" " " identifier ":" [ " " { unicode_char - newline } ] .
warning_doc     = "//" " " "!" " " { unicode_char - newline } .
example_doc     = "//" " " ">" " " { unicode_char - newline } .
```

Key changes:
- Remove `#` marker for descriptions — unmarked comments are descriptions
- Replace `@param name` with `* name:` — shorter, markdown-like
- Replace `@field name` with `* name:` — same syntax, context determines meaning
- Keep `!` for warnings
- Keep `>` for examples

### Canonical Spacing

The canonical form for member documentation is:
```
// * name: description
```

- Space after `//`
- Space after `*`
- Colon is **always required** (even without description)
- Space before description (when present)

### Non-Documentation Comments

Any comment immediately preceding a declaration is treated as documentation. Comments that are not intended as documentation (e.g., TODO notes, section markers) must be separated from the declaration by a blank line:

```ori
// TODO: refactor this

// Computes the sum.
@add (a: int, b: int) -> int = a + b
```

### Examples

#### Function Documentation

```ori
// Computes fibonacci using memoization.
// This implementation is O(n) instead of O(2^n).
// * n: The fibonacci index (must be non-negative)
// * memo: Whether to use memoization
// ! Panics if n is negative
// > fib(n: 10, memo: true) -> 55
// > fib(n: 0, memo: false) -> 0
@fib (n: int, memo: bool) -> int = recurse(
    condition: n < 2,
    base: n,
    step: self(n - 1, memo) + self(n - 2, memo),
    memo: memo,
)
```

#### Type Documentation

```ori
// A user in the system.
// * id: Unique identifier
// * name: Display name
// * email: Contact email (must be valid)
type User = { id: int, name: str, email: str }
```

#### Simple Cases (Most Common)

```ori
// Returns the absolute value.
@abs (x: int) -> int = if x < 0 then -x else x

// The maximum allowed connections.
let $max_connections = 100
```

No markers needed for simple descriptions.

#### Multi-line Descriptions

```ori
// Parses a JSON string into a structured value.
// Supports all JSON types: objects, arrays, strings,
// numbers, booleans, and null.
// * input: The JSON string to parse
// * strict: Whether to reject trailing commas
// ! Panics if input is not valid JSON
@parse_json (input: str, strict: bool) -> JsonValue = ...
```

#### Examples with Multiple Lines

```ori
// Formats a number with thousands separators.
// * n: The number to format
// > format_number(n: 1000) -> "1,000"
// > format_number(n: 1234567) -> "1,234,567"
// > format_number(n: 42) -> "42"
@format_number (n: int) -> str = ...
```

---

## Formatting Behavior

### Reordering

The formatter reorders doc comments into canonical order:

1. Description (unmarked lines)
2. Parameters/Fields (`*` lines) — in declaration order
3. Warnings (`!` lines)
4. Examples (`>` lines)

**Before formatting:**
```ori
// > add(a: 1, b: 2) -> 3
// * b: Second operand
// ! Panics on overflow
// Adds two numbers.
// * a: First operand
@add (a: int, b: int) -> int = a + b
```

**After formatting:**
```ori
// Adds two numbers.
// * a: First operand
// * b: Second operand
// ! Panics on overflow
// > add(a: 1, b: 2) -> 3
@add (a: int, b: int) -> int = a + b
```

### Param/Field Order

`*` entries are reordered to match the declaration:

```ori
// Before: params in wrong order
// * y: The y coordinate
// * x: The x coordinate
type Point = { x: int, y: int }

// After: matches field order
// * x: The x coordinate
// * y: The y coordinate
type Point = { x: int, y: int }
```

### Context Detection

The formatter determines whether `*` entries are params or fields based on context:

| Following Declaration | `*` Means |
|-----------------------|-----------|
| `@function (...)` | Parameter |
| `type Name = { ... }` | Field |
| `trait Name { ... }` | Method param (within method) |

---

## Comparison

### Before and After

```ori
// BEFORE: Verbose keywords
// #A 2D point in Cartesian space.
// @field x The x coordinate
// @field y The y coordinate
// !Values may overflow on extreme coordinates
type Point = { x: int, y: int }

// #Computes the distance from origin.
// @param self The point
// >point.distance() -> 5.0 (for point 3,4)
impl Point {
    @distance (self) -> float = ...
}

// AFTER: Minimal markers
// A 2D point in Cartesian space.
// * x: The x coordinate
// * y: The y coordinate
// ! Values may overflow on extreme coordinates
type Point = { x: int, y: int }

// Computes the distance from origin.
// * self: The point
// > point.distance() -> 5.0 (for point 3,4)
impl Point {
    @distance (self) -> float = ...
}
```

### Character Savings

Marker overhead comparison:

| Element | Before (marker) | After (marker) | Saved |
|---------|-----------------|----------------|-------|
| Description | `#` | (none) | 1 char |
| Param | `@param ` | `* ` + `:` | 3 chars |
| Field | `@field ` | `* ` + `:` | 3 chars |

Per-param savings compound quickly in well-documented code.

### Visual Comparison

```ori
// Before: Noisy
// #Validates user input.
// @param input The raw input string
// @param rules Validation rules to apply
// @param strict Whether to fail on warnings
// !Panics if rules is empty
// >validate(input: "test", rules: [...], strict: true) -> Ok(...)

// After: Clean
// Validates user input.
// * input: The raw input string
// * rules: Validation rules to apply
// * strict: Whether to fail on warnings
// ! Panics if rules is empty
// > validate(input: "test", rules: [...], strict: true) -> Ok(...)
```

---

## Implementation

### Files to Change

#### Spec Updates
- `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md` — Update comment syntax
- `docs/ori_lang/0.1-alpha/spec/16-formatting.md` — Update doc comment ordering
- `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` — Update grammar

#### Compiler Changes
- `compiler/ori_ir/src/comment.rs` — Update `CommentKind` enum
- `compiler/ori_lexer/src/lib.rs` — Update comment classification
- `compiler/ori_fmt/src/comments.rs` — Update reordering logic
- `compiler/ori_fmt/src/declarations.rs` — Update comment emission

#### Documentation Updates
- `CLAUDE.md` — Update Comments section

### CommentKind Changes

```rust
// Before
pub enum CommentKind {
    Regular,
    DocDescription,  // // #...
    DocParam,        // // @param ...
    DocField,        // // @field ...
    DocWarning,      // // !...
    DocExample,      // // >...
}

// After
pub enum CommentKind {
    Regular,         // // anything not matching below
    DocDescription,  // // text (no marker, before declaration)
    DocMember,       // // * name: ... (param or field)
    DocWarning,      // // ! ...
    DocExample,      // // > ...
}
```

Note: `DocParam` and `DocField` merge into `DocMember` — the formatter uses context to determine which.

### Lexer Changes

```rust
// Before
fn classify_comment(content: &str) -> CommentKind {
    let trimmed = content.trim_start();
    if trimmed.starts_with('#') { return CommentKind::DocDescription; }
    if trimmed.starts_with("@param") { return CommentKind::DocParam; }
    if trimmed.starts_with("@field") { return CommentKind::DocField; }
    if trimmed.starts_with('!') { return CommentKind::DocWarning; }
    if trimmed.starts_with('>') { return CommentKind::DocExample; }
    CommentKind::Regular
}

// After
fn classify_comment(content: &str) -> CommentKind {
    let trimmed = content.trim_start();
    if trimmed.starts_with('*') { return CommentKind::DocMember; }
    if trimmed.starts_with('!') { return CommentKind::DocWarning; }
    if trimmed.starts_with('>') { return CommentKind::DocExample; }
    CommentKind::Regular  // Unmarked = description or regular
}
```

Description detection moves to the formatter, which checks if a `Regular` comment immediately precedes a declaration.

### Formatter Changes

The `extract_param_name` function changes:

```rust
// Before: // @param name description
fn extract_param_name(content: &str) -> &str {
    content.strip_prefix("@param")?.split_whitespace().next()
}

// After: // * name: description
fn extract_member_name(content: &str) -> &str {
    let after_star = content.strip_prefix('*')?.trim_start();
    after_star.split(':').next()?.trim()
}
```

---

## Migration

### Automated Migration

The formatter can auto-migrate old syntax:

```ori
// Input (old format)
// #Description
// @param x The value

// Output (new format)
// Description
// * x: The value
```

### Migration Steps

1. Update lexer to recognize both old and new formats
2. Add deprecation warnings for old format
3. `ori fmt` converts old to new automatically
4. Remove old format support in next minor version

---

## Design Rationale

### Why Remove `#` for Descriptions?

The `#` marker adds no information:
- Comments before a declaration are obviously about that declaration
- The `#` doesn't enable any formatting behavior that couldn't work without it
- It's visual noise

### Why `*` for Params/Fields?

Considered alternatives:

| Syntax | Issue |
|--------|-------|
| `@name` | Conflicts with function sigil `@func` |
| `.name` | Could work, but less familiar |
| `- name` | Could work, markdown-like |
| `* name` | Markdown list syntax, familiar ✓ |

`*` wins because:
1. Familiar from markdown lists
2. Visually lightweight
3. Clear "this is a list item" semantics
4. No conflicts with other Ori syntax

### Why Merge Param and Field?

There's no semantic difference at the doc level:
- Both document a named member
- Context (function vs type) determines which it is
- Separate keywords add complexity without benefit

### Why Keep `!` and `>`?

These markers have clear semantic meaning:
- `!` signals "danger" — warnings, panics, preconditions
- `>` signals "example" — runnable/verifiable code

Unmarked text can't convey these meanings.

---

## Summary

This proposal simplifies doc comments by:

1. **Removing `#`** — Descriptions need no marker
2. **Replacing `@param`/`@field` with `*`** — Shorter, markdown-like
3. **Unifying param/field syntax** — Context determines meaning
4. **Keeping `!` and `>`** — These markers add real value

The result is cleaner, more consistent documentation that aligns with Ori's minimal syntax philosophy.
