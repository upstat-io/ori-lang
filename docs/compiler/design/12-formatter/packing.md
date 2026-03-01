---
title: "Packing"
description: "Ori Compiler Design — Container Single-Line vs Multi-Line Decisions"
order: 1202
section: "Formatter"
---

# Packing

Layer 2 of the formatter determines how containers are laid out. A "container"
is any syntactic construct that holds a delimited sequence of items: function
arguments, list literals, struct fields, map entries, match arms, and so on.
The packing layer decides whether a container should be rendered entirely on
one line (Fit) or with each item on its own line (Stack).

## Packing Enum

Every container resolves to one of two packing modes:

```rust
pub enum Packing {
    /// All items on a single line, separated by commas and spaces.
    /// Example: `f(a: 1, b: 2, c: 3)`
    Fit,

    /// Each item on its own line, indented by one level.
    /// Example:
    ///   f(
    ///       a: 1,
    ///       b: 2,
    ///       c: 3,
    ///   )
    Stack,
}
```

There is no hybrid mode (e.g., "bin-packing" where items fill lines greedily).
Ori follows the all-or-nothing approach used by gofmt and Prettier's
`--prose-wrap` mode: either everything fits on one line, or each item gets its
own line. This eliminates ambiguity and produces stable, diff-friendly output.

## ConstructKind

The packing layer classifies containers by their syntactic role:

```rust
pub enum ConstructKind {
    /// Function arguments: `f(a: 1, b: 2)`
    FnArgs,

    /// Function parameters: `@f (x: int, y: int) -> int`
    FnParams,

    /// Generic type parameters: `<T, U, V>`
    Generics,

    /// Where clauses: `where T: Eq, U: Clone`
    WhereClauses,

    /// List literal items: `[1, 2, 3]`
    ListItems,

    /// Map literal entries: `{"a": 1, "b": 2}`
    MapEntries,

    /// Struct field definitions: `type P = { x: int, y: int }`
    FieldDefs,

    /// Struct literal fields: `Point { x: 1, y: 2 }`
    FieldValues,

    /// Match arms: `match x { 0 -> "zero", 1 -> "one" }`
    MatchArms,

    /// Enum variants: `type Color = Red | Green | Blue`
    Variants,

    /// Import items: `use std.math { sqrt, abs, floor }`
    ImportItems,

    /// Tuple elements: `(1, "hello", true)`
    TupleItems,

    /// Chain of method calls: `x.map(f:).filter(p:).collect()`
    MethodChain,
}
```

Different construct kinds may have different packing thresholds or special
rules. For example, `MatchArms` are always stacked regardless of width, while
`FnArgs` use the standard width-based decision.

## The Packing Decision

### `determine_packing(kind: ConstructKind, inline_width: usize, current_column: usize) -> Packing`

This is the central function of the packing layer. It takes:

1. **`kind`**: What type of container this is
2. **`inline_width`**: The measured width if all items were on one line (from
   Pass 1)
3. **`current_column`**: The current column position on the output line

And returns `Fit` or `Stack`.

The default logic:

```
if inline_width + current_column <= MAX_LINE_WIDTH {
    Fit
} else {
    Stack
}
```

### Always-Stacked Constructs

Certain construct kinds are always stacked, regardless of their inline width.
These are identified before the width check:

- **`MatchArms`**: Match expressions always stack their arms for readability.
  Even a two-arm match like `match b { true -> 1, false -> 0 }` is stacked.
- **`try` blocks**: The `try { ... }` construct always breaks.
- **`recurse`, `parallel`, `spawn`, `nursery`**: These expression-level
  constructs with multiple named arguments are always stacked.

For these constructs, the inline width is set to `usize::MAX` during Pass 1,
which guarantees `inline_width + current_column > MAX_LINE_WIDTH` for any
column value. This is simpler than adding special-case checks in the packing
logic.

### Width-Based Constructs

Most constructs use the width-based decision:

- **`FnArgs`**: Fit if the call fits on one line, stack otherwise.
- **`FnParams`**: Fit if the parameter list fits, stack otherwise.
- **`ListItems`**: Fit if the list literal fits, stack otherwise.
- **`MapEntries`**: Fit if the map literal fits, stack otherwise.
- **`FieldDefs`/`FieldValues`**: Fit if the struct fits, stack otherwise.
- **`Generics`**: Fit if the generic parameter list fits, stack otherwise.
- **`TupleItems`**: Fit if the tuple fits, stack otherwise.
- **`ImportItems`**: Fit if the import list fits, stack otherwise.

## `is_simple_item` Predicate

The packing layer uses a simplicity heuristic to influence decisions for
nested containers:

```rust
pub fn is_simple_item(item: &Expr) -> bool
```

An item is "simple" if it is:
- A literal (int, float, string, bool, char)
- An identifier
- A simple field access (`x.y`)
- A short unary expression (`-x`, `!flag`)

An item is "not simple" if it contains:
- Nested containers (lists, maps, structs)
- Lambda expressions
- Function calls with arguments
- Binary expressions
- Match/try/loop expressions

The simplicity predicate is not directly used in the `determine_packing`
function, but it influences the inline width measurement. A container of
simple items has a predictable, small inline width. A container with complex
items tends to have a large inline width and will naturally stack.

## Independent Breaking

A critical design principle: **nested containers make independent packing
decisions based on their own width, not their parent's state.**

Consider:

```ori
@process (items: [Item]) -> [Result] =
    items.map(transform: i -> validate(input: i, strict: true));
```

The outer method chain and the inner lambda are independent containers. The
chain might stack (putting `.map(...)` on its own line) while the lambda's
argument list stays inline, or vice versa. Each container's packing is
determined solely by whether its own inline width fits in the remaining space
at the column where it starts.

This means the formatter does not need to "negotiate" between parent and child
containers. Each container is formatted in isolation, producing stable output
that does not change when unrelated parts of the expression are modified.

## Stacking Layout

When a container is stacked, the formatter applies:

1. **Opening delimiter** on the current line
2. **Indent** by `INDENT_WIDTH` (4 spaces)
3. Each **item on its own line**, followed by a trailing comma
4. **Dedent** back to the original indentation
5. **Closing delimiter** on its own line

Example:

```ori
@create_user (
    name: str,
    email: str,
    age: int,
    role: Role,
) -> User = { ... }
```

The trailing comma on the last item is controlled by `FormatConfig::trailing_comma`.
With the default `Always` policy, every stacked item gets a trailing comma. This
produces cleaner diffs when items are added or reordered.

## Empty Containers

Empty containers are always rendered without any spacing or newlines:

- `()` — empty parens
- `[]` — empty list
- `{}` — empty braces (note: in expression context, `{}` is an empty map)

These are never stacked, regardless of any configuration.

## Single-Item Containers

A container with exactly one item follows the normal width-based decision. A
single-item container can be either `Fit` or `Stack`:

- `Fit`: `f(value: x)` — the single argument fits inline
- `Stack`: used when the single argument is itself a complex expression that
  exceeds the line width

There is no special case for single-item containers.
