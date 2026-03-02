---
title: "Formatting Rules"
description: "Ori Compiler Design — Line Breaking and Formatting Rules"
order: 1203
section: "Formatter"
---

# Formatting Rules (Layers 2-4)

This document covers how the formatter decides when to break lines, how to indent
broken content, and how to pack items inside containers. These decisions span three
layers of the formatter architecture: Layer 2 (Packing), Layer 3 (Shape), and
Layer 4 (Breaking Rules).

## The Core Algorithm

The formatter uses a two-pass, width-based algorithm:

1. **Measure pass** (bottom-up): `WidthCalculator` computes the inline width of every
   AST node -- the number of characters it would occupy if rendered on a single line.
   Results are cached in an `FxHashMap<ExprId, usize>`. Some constructs (`match`, `try`,
   `recurse`, `parallel`, `spawn`, `nursery`) return the sentinel value `ALWAYS_STACKED`
   (`usize::MAX`), meaning they never render inline.

2. **Render pass** (top-down): `Formatter::format()` checks the pre-calculated width
   against the current position. The decision is:

```
if width == ALWAYS_STACKED:
    emit_stacked(expr)       // always multi-line
elif column + width <= 100:
    emit_inline(expr)        // fits on this line
else:
    emit_broken(expr)        // break to multi-line
```

This width-based approach avoids backtracking and ensures linear formatting time.

## Width Calculation

Width formulas are defined per `ExprKind` variant in the `width/` module. Each
submodule handles a category of expressions:

| Module | Covers |
|--------|--------|
| `literals.rs` | `int_width`, `float_width`, `bool_width`, `string_width`, `char_width` |
| `compounds.rs` | `duration_width`, `size_width` |
| `operators.rs` | `binary_op_width`, `unary_op_width` |
| `calls.rs` | `call_width`, `call_named_width`, `method_call_width`, `method_call_named_width` |
| `collections.rs` | `list_width`, `map_width`, `struct_width`, `tuple_width`, `range_width` |
| `control.rs` | `if_width`, `for_width`, `block_width`, `assign_width`, `with_capability_width` |
| `wrappers.rs` | `ok_width`, `err_width`, `some_width`, `loop_width`, `try_width`, `cast_width` |
| `patterns.rs` | `binding_pattern_width`, `for_binding_pattern_width` |
| `helpers.rs` | `accumulate_widths()`, `COMMA_SEPARATOR_WIDTH` (= 2, for ", ") |

Representative width formulas:

| Construct | Formula |
|-----------|---------|
| Identifier | `name.len()` |
| Integer literal | `text.len()` |
| String literal | `text.len() + 2` (quotes) |
| Binary expression | `left + 3 + right` (space-op-space for most ops) |
| Function call | `name + 1 + args_width + separators + 1` |
| Named argument | `name + 2 + value` (name + ": " + value) |
| Struct literal | `name + 3 + fields_width + separators + 2` (`" { "` + fields + `" }"`) |
| List | `2 + items_width + separators` (`[` + items + `]`) |

If any sub-expression returns `ALWAYS_STACKED`, the parent also returns `ALWAYS_STACKED`.

## Container Packing (Layer 2)

### Packing Strategies

When a container does not fit on one line, the `Packing` enum determines how to
lay out its items:

**`FitOrOnePerLine`** (default) -- try inline, then one item per line:
```ori
// Inline:
@foo (x: int, y: int) -> int

// Broken:
@foo (
    x: int,
    y: int,
    z: int,
) -> int
```

**`FitOrPackMultiple`** -- try inline, then pack multiple simple items per line:
```ori
// Inline:
[1, 2, 3, 4, 5]

// Broken:
[
    1, 2, 3, 4, 5,
    6, 7, 8, 9, 10,
]
```

**`AlwaysOnePerLine`** -- user intent forces multi-line:
```ori
[
    1,
    2,
    3,
]
```

**`AlwaysStacked`** -- construct is always multi-line regardless of width.

### Construct Kinds

The `ConstructKind` enum classifies every container type for packing decisions:

- **Always stacked**: `RunTopLevel`, `Try`, `Match`, `Recurse`, `Parallel`, `Spawn`,
  `Nursery`, `MatchArms`.
- **Width-based, one per line**: `FunctionParams`, `FunctionArgs`, `GenericParams`,
  `WhereConstraints`, `Capabilities`, `StructFieldsDef`, `StructFieldsLiteral`,
  `SumVariants`, `MapEntries`, `TupleElements`, `ImportItems`, `ListComplex`, `RunNested`.
- **Width-based, pack multiple**: `ListSimple` (all items are simple literals/identifiers).

### Simple vs Complex Items

The `is_simple_item()` function in `packing/simple.rs` classifies expressions:

- **Simple**: `Int`, `Float`, `String`, `Char`, `Bool`, `Duration`, `Size`, `Ident`,
  `None`, `Unit`. These can pack multiple per line when a list breaks.
- **Complex**: Everything else (calls, method calls, struct literals, binary expressions,
  nested collections). These go one per line when broken.

### User Intent Signals

Three metadata signals override the base packing strategy to `AlwaysOnePerLine`:

1. **Trailing comma**: The user placed a trailing comma after the last item.
2. **Comments**: There are comments between items inside the container.
3. **Empty lines**: There are blank lines between items.

Any of these signals means the user wants multi-line layout, and the formatter respects
that even if the content would fit on one line.

### Separators

The `Separator` enum controls what appears between items:

| Separator | Inline | Broken |
|-----------|--------|--------|
| `Comma` | `", "` | `","` + newline |
| `Space` | `" "` | newline |
| `Pipe` | `" \| "` | newline + `"\| "` prefix |

Sum type variants use `Pipe`; everything else uses `Comma`.

## Shape Tracking (Layer 3)

The `Shape` struct tracks three values as the formatter descends into nested constructs:

- **`width`**: Characters remaining on the current line.
- **`indent`**: Current indentation level in spaces.
- **`offset`**: Position from the start of the line.

Key operations:

| Operation | Effect |
|-----------|--------|
| `consume(n)` | Reduce remaining width by n, increase offset by n |
| `indent(spaces)` | Increase indent, reduce width |
| `dedent(spaces)` | Decrease indent, increase width |
| `next_line(max)` | Reset to indent position with full remaining width |
| `fits(w)` | Check if w characters fit in remaining width |
| `for_nested(config)` | Fresh width from current indent (independent breaking) |
| `for_block(config)` | Indent + next_line (for block bodies) |

### Independent Breaking

The most important design property of Shape is **independent breaking**. When the
formatter creates a shape for a nested construct via `for_nested()`, it calculates
fresh available width from the current indentation level, not from the current consumed
position. This means:

```ori
// The inner call fits, so it stays inline even though the outer breaks:
let result = run(
    process(items.map(x -> x * 2)),
    validate(result),
)
```

Without independent breaking, the inner `process(...)` call would also break simply
because the outer `run(...)` broke, leading to unnecessary vertical expansion.

## Breaking Rules (Layer 4)

Layer 4 contains Ori-specific rules for constructs that need special formatting logic
beyond simple width-based inline/broken decisions.

### MethodChainRule

All-or-nothing method chain breaking. When a method chain does not fit on one line,
every `.method()` call breaks to its own line. No selective breaking.

```ori
// Inline (fits):
items.map(x -> x * 2).filter(x -> x > 0)

// Broken (all break together):
items
    .map(x -> x * 2)
    .filter(x -> x > 0)
    .take(n: 10)
```

Constants: `MIN_CHAIN_LENGTH = 2` (single method calls do not trigger chain logic).

The `collect_method_chain()` function walks backward through `MethodCall`/`MethodCallNamed`
nodes to build a `MethodChain` struct containing the receiver and ordered list of
`ChainedCall` entries.

### ShortBodyRule

Bodies under ~20 characters stay with their `yield`/`do` keyword. This prevents
awkward formatting where a lone identifier appears on its own line:

```ori
// Good (short body stays with yield):
for user in users yield user

// Bad (would happen without ShortBodyRule):
for user in users yield
    user
```

The `suggest_break_point()` function returns one of:
- `NoBreak` -- entire expression fits inline.
- `AfterYield` -- complex body breaks after `yield`/`do`, indented on next line.
- `BeforeFor` -- short body stays with `yield`, but line is too long, so break before `for`.

### BooleanBreakRule

When a boolean expression has 3 or more top-level `||` clauses, each clause gets its
own line with `||` at the start:

```ori
// 2 clauses (no break unless exceeds width):
if a || b then x

// 3+ clauses (break with leading ||):
if user.active && user.verified
    || user.is_admin
    || user.bypass_check then x
```

Threshold: `OR_THRESHOLD = 3`.

The `collect_or_clauses()` function walks the `Binary { op: Or }` chain to collect
all top-level `||` operands in order.

### ChainedElseIfRule

Kotlin-style if-else-if formatting. The first `if` stays with the assignment, and
each else clause appears on its own line:

```ori
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

The `collect_if_chain()` function walks through nested `If` expressions to build an
`IfChain` struct with the initial condition/then, a list of `ElseIfBranch` entries,
and an optional final else.

The broken renderer checks whether each `if cond then branch` segment fits on the
current line. If it does, the segment renders inline on the `else` line. If not, the
`then` branch breaks to an indented next line.

### NestedForRule

Rust-style indentation for nested `for` loops. Each nesting level gets its own line
with incremented indentation:

```ori
for user in users yield
    for permission in user.permissions yield
        for action in permission.actions yield
            action.name
```

The `collect_for_chain()` function walks through nested `For` expressions to build a
`ForChain` with a list of `ForLevel` entries and the final body expression.

### ParenthesesRule

Parentheses are added when semantically required and preserved when written by the user
(with a known limitation: the AST does not currently track explicit user parentheses).

Required positions (`needs_parens()`):
- **Receiver**: `(for x in items yield x).fold(...)` -- complex expressions need parens
  before `.method()`.
- **Call target**: `(x -> x * 2)(5)` -- lambda/binary/if expressions need parens
  before `(args)`.
- **Iterator source**: `for x in (inner) yield x` -- for/if/lambda/let need parens
  after `in`.

### LoopRule

Loop bodies that contain complex constructs (`try`, `match`, `for`, nested `loop`)
always break:

```ori
// Simple body (can inline):
loop(if done then break else continue)

// Complex body (always breaks):
loop(
    run(
        let input = read_line(),
        if input == "quit" then break else continue,
    )
)
```

## Rule Interaction and Priorities

When multiple rules could apply to the same expression, the formatter resolves them
through a clear priority order:

1. **Always-stacked** constructs (`ALWAYS_STACKED` width) take precedence over everything.
   `match`, `try`, `recurse`, `parallel`, `spawn` never attempt inline rendering.

2. **Width check** determines inline vs broken. If the expression fits, it renders inline
   regardless of what breaking rules might say.

3. **Breaking rules** apply only within `emit_broken()`. The broken renderer checks for
   method chains, boolean breaks, chained else-if, nested for, and loop complexity to
   decide the specific multi-line layout.

4. **Packing strategy** applies within container rendering. The construct kind and user
   intent signals determine whether items go one-per-line or pack multiple per line.

5. **Spacing rules** (Layer 1) apply at the token level within both inline and broken
   rendering. They are the final layer consulted before text is emitted.

## Indentation Strategy

All indentation uses spaces (4 per level, configurable via `FormatConfig.indent_size`).
Tabs in the source are converted to spaces during preprocessing.

Indentation increases in these contexts:
- Block bodies (`{ ... }`).
- Broken container items (function args, list elements, struct fields).
- Broken binary expression continuations (operator on new line).
- Broken lambda bodies (after `->`).
- Broken let initializers (after `=`).
- Broken for bodies (after `yield`/`do`).
- Broken with-capability bodies (after `in`).
- Nested for loops (each level adds one indent).

The `FormatContext` provides `indent()`, `dedent()`, and `with_indent(f)` (RAII-style)
for managing indentation state. The `Shape` struct tracks indentation for width calculations.
