---
title: "Layer 2: Container Packing"
description: "Ori Formatter Design — Container Packing Decisions"
order: 3
section: "Layers"
---

# Layer 2: Container Packing

The packing layer decides how to format containers (lists, function args, struct fields, etc.) — whether to keep them inline or break to multiple lines, and how to arrange items when broken.

## Architecture

```
ConstructKind ──┐
                │
has_trailing_comma ──┼──▶ determine_packing() ──▶ Packing
has_comments ───┤
has_empty_lines ──┘
```

## Key Types

### Packing

The four fundamental packing strategies:

```rust
pub enum Packing {
    /// Try single line; if doesn't fit, one item per line.
    /// Default for most containers.
    FitOrOnePerLine,

    /// Try single line; if doesn't fit, pack multiple per line.
    /// For simple lists (literals, identifiers).
    FitOrPackMultiple,

    /// Always one item per line (user indicated via trailing comma).
    AlwaysOnePerLine,

    /// Always stacked (blocks, try, match, etc.).
    AlwaysStacked,
}
```

### ConstructKind

Enumeration of all container types (22 kinds):

```rust
pub enum ConstructKind {
    // Always Stacked (Spec lines 78-90)
    RunTopLevel,    // run { ... } at function body level
    Try,            // try { ... }
    Match,          // match expr { ... }
    Recurse,        // recurse(...)
    Parallel,       // parallel(...)
    Spawn,          // spawn(...)
    Nursery,        // nursery(...)

    // Width-Based: One Per Line When Broken
    FunctionParams,      // @foo (x: int, y: int)
    FunctionArgs,        // foo(x: 1, y: 2)
    GenericParams,       // <T, U>
    WhereConstraints,    // where T: Clone
    Capabilities,        // uses Http, FileSystem
    StructFieldsDef,     // type Foo = { x: int }
    StructFieldsLiteral, // Point { x: 1 }
    SumVariants,         // A | B | C
    MapEntries,          // { "key": value }
    TupleElements,       // (a, b, c)
    ImportItems,         // use "./foo" { a, b }

    // Width-Based: Multiple Per Line
    ListSimple,     // [1, 2, 3] - literals/identifiers

    // Width-Based: One Per Line
    ListComplex,    // [foo(), bar()] - structs/calls

    // Context-Dependent
    RunNested,      // run { ... } inside expression
    MatchArms,      // match arm list (always one per line)
}
```

### Separator

What separates items in a container:

```rust
pub enum Separator {
    /// Comma: `a, b, c`
    Comma,

    /// Space: `a b c` (rare)
    Space,

    /// Pipe: `A | B | C` (sum variants)
    Pipe,
}
```

## Decision Logic

The `determine_packing()` function encodes the decision tree:

```rust
pub fn determine_packing(
    construct: ConstructKind,
    has_trailing_comma: bool,
    has_comments: bool,
    has_empty_lines: bool,
    _item_count: usize,
) -> Packing {
    // 1. Always-stacked constructs
    if construct.is_always_stacked() {
        return Packing::AlwaysStacked;
    }

    // 2. Empty lines → preserve vertical spacing
    if has_empty_lines {
        return Packing::AlwaysOnePerLine;
    }

    // 3. Trailing comma → user intent to break
    if has_trailing_comma {
        return Packing::AlwaysOnePerLine;
    }

    // 4. Comments force breaking
    if has_comments {
        return Packing::AlwaysOnePerLine;
    }

    // 5. Simple lists can pack multiple per line
    if matches!(construct, ConstructKind::ListSimple) {
        return Packing::FitOrPackMultiple;
    }

    // 6. Default: try inline, else one per line
    Packing::FitOrOnePerLine
}
```

## Packing Strategies in Detail

### FitOrOnePerLine (Default)

Try to fit everything on one line. If it doesn't fit, put each item on its own line with trailing comma.

```ori
// Fits inline:
@foo (x: int, y: int) -> int

// Doesn't fit, one per line:
@process_data (
    input: [DataRecord],
    config: ProcessingConfig,
    output: OutputChannel,
) -> Result<ProcessingStats, ProcessingError>
```

Used for: function params/args, struct fields, generic params, map entries, tuples.

### FitOrPackMultiple

Try to fit on one line. If it doesn't fit, pack multiple items per line (for simple items).

```ori
// Fits inline:
[1, 2, 3, 4, 5]

// Doesn't fit, pack multiple per line:
[
    1, 2, 3, 4, 5,
    6, 7, 8, 9, 10,
    11, 12, 13, 14, 15,
]
```

Used for: lists containing only literals and identifiers.

### AlwaysOnePerLine

Always format with one item per line, regardless of width.

```ori
// User wrote trailing comma - honor their intent:
[
    1,
    2,
    3,
]

// Has comments inside:
[
    x,  // first item
    y,  // second item
]
```

Triggered by: trailing comma, comments inside container, empty lines between items.

### AlwaysStacked

Always use stacked format with specific rules. Never goes inline.

```ori
// blocks always stack:
{
    let x = compute()
    let y = process(x)
    x + y
}

// match always stacks:
match value {
    Some(x) -> process(x)
    None -> default()
}
```

Used for: blocks `{ }`, `try { }`, `match`, `recurse`, `parallel`, `spawn`, `nursery`.

## Simple Item Detection

The `is_simple_item()` function determines if an expression is "simple" for packing purposes:

```rust
pub fn is_simple_item(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);
    matches!(
        &expr.kind,
        ExprKind::Ident(_)
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::None
    )
}
```

Simple items: identifiers, literals (int, float, string, char, bool, duration, size), unit, none.

Complex items: calls, struct literals, nested collections, conditionals, etc.

## Usage

### In Width Calculator

```rust
pub fn list_construct_kind(arena: &ExprArena, items: &[ExprId]) -> ConstructKind {
    if all_items_simple(arena, items) {
        ConstructKind::ListSimple
    } else {
        ConstructKind::ListComplex
    }
}
```

### In Formatter

```rust
fn format_list(&mut self, items: &[ExprId]) {
    let construct = list_construct_kind(self.arena, items);
    let packing = determine_packing(
        construct,
        self.has_trailing_comma,
        self.has_comments,
        self.has_empty_lines,
        items.len(),
    );

    match packing {
        Packing::FitOrOnePerLine => {
            if self.fits_inline(items) {
                self.emit_inline_list(items);
            } else {
                self.emit_one_per_line(items);
            }
        }
        Packing::FitOrPackMultiple => {
            if self.fits_inline(items) {
                self.emit_inline_list(items);
            } else {
                self.emit_packed_list(items);
            }
        }
        Packing::AlwaysOnePerLine => self.emit_one_per_line(items),
        Packing::AlwaysStacked => self.emit_stacked(items),
    }
}
```

## Helper Methods

### ConstructKind Methods

```rust
impl ConstructKind {
    /// Check if always stacked (never inline)
    pub fn is_always_stacked(self) -> bool {
        matches!(self, RunTopLevel | Try | Match | Recurse | Parallel | Spawn | Nursery | MatchArms)
    }

    /// Check if uses comma separators
    pub fn uses_commas(self) -> bool {
        !matches!(self, SumVariants)  // Only sum variants use |
    }

    /// Check if run construct (top-level or nested)
    pub fn is_run(self) -> bool {
        matches!(self, RunTopLevel | RunNested)
    }

    /// Human-readable name for debugging
    pub fn name(self) -> &'static str {
        // Returns "function params", "match arms", etc.
    }
}
```

### Packing Methods

```rust
impl Packing {
    /// Can try inline first?
    pub fn can_try_inline(self) -> bool {
        matches!(self, FitOrOnePerLine | FitOrPackMultiple)
    }

    /// Always forces multiline?
    pub fn always_multiline(self) -> bool {
        matches!(self, AlwaysOnePerLine | AlwaysStacked)
    }

    /// Allows packing multiple per line?
    pub fn allows_packing(self) -> bool {
        matches!(self, FitOrPackMultiple)
    }
}
```

## Spec Reference

This layer implements:
- Lines 58-92: Width-based and always-stacked rules
- Lines 225-242: Simple vs complex items
- Line 75: Multiple per line for simple lists
- Line 76: One per line for complex lists
