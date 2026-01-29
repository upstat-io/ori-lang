---
title: "Algorithm Overview"
description: "Ori Formatter Design — Core Formatting Algorithm"
order: 1
---

# Algorithm Overview

The Ori formatter uses a width-based breaking algorithm. The core principle is simple: render inline if it fits, break if it doesn't.

## High-Level Algorithm

```
function format(node):
    if is_always_stacked(node):
        return render_stacked(node)

    inline_repr = render_inline(node)
    if width(inline_repr) <= 100:
        return inline_repr
    else:
        return render_broken(node)
```

Each AST node type defines:
1. **Inline rendering** — How to render on a single line
2. **Broken rendering** — How to render when broken across lines
3. **Always-stacked** — Whether to skip inline attempt (for `run`, `try`, `match` arms, etc.)

## Two-Pass Approach

The formatter operates in two passes:

### Pass 1: Measure

Calculate the inline width of each node without producing output. This is a bottom-up traversal:

```
width(BinaryExpr(left, op, right)) = width(left) + width(op) + width(right) + 4
                                     // +4 for spaces around operator
```

Widths can be cached on the AST for efficiency.

### Pass 2: Render

Top-down rendering that decides inline vs broken based on measured widths and current column position:

```
function render(node, current_column):
    if is_always_stacked(node):
        emit_stacked(node, current_column)
    else if current_column + width(node) <= 100:
        emit_inline(node)
    else:
        emit_broken(node, current_column)
```

## Context Tracking

The formatter tracks:

| State | Purpose |
|-------|---------|
| `current_column` | Position on current line (0-indexed) |
| `indent_level` | Current nesting depth (multiply by 4 for spaces) |

## Always-Stacked Constructs

Some constructs bypass the width check and always use stacked format:

| Construct | Reason |
|-----------|--------|
| `run` / `try` | Sequential blocks; stacking shows execution order |
| `match` arms | Pattern matching; one arm per line aids readability |
| `recurse` | Named parameters pattern |
| `parallel` / `spawn` | Concurrency patterns |
| `nursery` | Structured concurrency pattern |

## Independent Breaking

Nested constructs break independently based on their own width:

```ori
// Outer call breaks (exceeds 100), but inner call fits - stays inline
let result = process(
    data: transform(input: fetch(url: endpoint), options: defaults),
    config: settings,
)

// Inner call also exceeds 100 - it breaks too
let result = process(
    data: transform(
        input: fetch(url: api_endpoint),
        options: default_transform_options,
        validator: schema_validator,
    ),
    config: settings,
)
```

## Width Constants

| Construct | Width Formula |
|-----------|---------------|
| Identifier | `name.len()` |
| Integer literal | `text.len()` |
| String literal | `text.len() + 2` (quotes) |
| Binary expr | `left + 3 + right` (space-op-space) |
| Function call | `name + 1 + args_width + separators + 1` |
| Named argument | `name + 2 + value` (`: `) |
| Struct literal | `name + 3 + fields_width + separators + 2` (` { ` + ` }`) |
| List | `2 + items_width + separators` (`[` + `]`) |
| Map | `2 + entries_width + separators` (`{` + `}`) |

## Trailing Commas

Trailing comma rules are deterministic:

| Format | Trailing Comma |
|--------|----------------|
| Single-line | Forbidden |
| Multi-line | Required |

## Body Placement

When a construct breaks and has a body (function, lambda, binding):

1. If body fits on the current line, keep it there
2. If body exceeds 100, indent to next line

```ori
// Body fits on return type line
) -> Result<T, E> = do_work()

// Body exceeds 100, indent to next line
) -> Result<T, E> =
    compute_something_complex(input: data)
```

## First Operand Rule

For binary expressions and chains, keep the first operand on the `let` line:

```ori
// Binary - first operand stays with let
let result = first_value + second_value
    - third_value

// Chain - initial value stays with let
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
```

## List Wrapping

Lists distinguish between simple and complex items:

- **Simple items** (literals, identifiers): wrap multiple per line
- **Complex items** (structs, calls, collections): one per line

```ori
// Simple items - wrap
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    11, 12, 13, 14, 15,
]

// Complex items - one per line
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
]
```

## Output Production

The formatter produces output through an emitter interface:

```rust
trait Emitter {
    fn emit(&mut self, text: &str);
    fn emit_newline(&mut self);
    fn emit_indent(&mut self, level: usize);
    fn emit_space(&mut self);
}
```
