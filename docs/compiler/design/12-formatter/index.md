---
title: "Formatter Overview"
description: "Ori Compiler Design — Code Formatter (ori_fmt)"
order: 1200
section: "Formatter"
sidebar_title: "Formatter"
sidebar_order: 12
sidebar_path: "/docs/compiler-design/12-formatter"
---

# Formatter Overview

The `ori_fmt` crate is the code formatter for the Ori programming language. It operates
on parsed AST nodes (not raw source text) and produces canonically formatted output. The
formatter is invoked through the `ori fmt` CLI command and can also be used as a library
for programmatic formatting and LSP integration.

## 5-Layer Architecture

The formatter is organized into five distinct layers, each with a single responsibility.
Higher layers depend on lower layers but never the reverse.

### Layer 1: Spacing (`spacing/`)

Declarative O(1) token spacing rules. Given a pair of adjacent token categories (left, right),
this layer determines whether to emit no space, a single space, a newline, or to preserve
existing spacing. Rules are defined as static `SpaceRule` entries in `spacing/rules.rs` and
compiled into a hash-map-backed `RulesMap` at initialization time.

Key types: `SpaceAction`, `TokenCategory`, `TokenMatcher`, `SpaceRule`, `RulesMap`.

### Layer 2: Packing (`packing/`)

Gleam-style container packing decisions. Determines how to lay out items inside containers
(function params, list elements, struct fields, map entries, etc.). The four packing
strategies are:

- `FitOrOnePerLine` -- try inline, one item per line if it does not fit (default).
- `FitOrPackMultiple` -- try inline, pack multiple simple items per line if broken.
- `AlwaysOnePerLine` -- user signaled multi-line intent (trailing comma, comments).
- `AlwaysStacked` -- constructs like `try`, `match`, `recurse`, `parallel`, `spawn`.

The `determine_packing()` function selects a strategy from the `ConstructKind` enum
and metadata (trailing comma, comments, empty lines).

Key types: `Packing`, `ConstructKind`, `Separator`.

### Layer 3: Shape (`shape/`)

Rustfmt-style width tracking. The `Shape` struct carries three values through the
formatting recursion: remaining `width` on the current line, current `indent` level,
and current `offset` from the line start. Operations include `consume`, `indent`,
`dedent`, `next_line`, `fits`, and `for_nested`. The key design property is
**independent breaking**: nested constructs break based on their own width, not
their parent's. A function call that fits on one line stays inline even inside a
larger construct that breaks.

Key types: `Shape`.

### Layer 4: Rules (`rules/`)

Ori-specific breaking rules for constructs that need special formatting logic beyond
simple width-based decisions. Each rule is a named struct with documented semantics,
thresholds, and decision logic:

| Rule | Behavior |
|------|----------|
| `MethodChainRule` | All-or-nothing: all chain elements break together at every `.` |
| `ShortBodyRule` | Bodies under ~20 chars stay with `yield`/`do` |
| `BooleanBreakRule` | 3+ `\|\|` clauses break with leading `\|\|` |
| `ChainedElseIfRule` | Kotlin-style: first `if` with assignment, else clauses on own lines |
| `NestedForRule` | Rust-style: each nested `for` increases indentation |
| `ParenthesesRule` | Preserve user parens, add when semantically required |
| `LoopRule` | Complex body (try/match/for) forces break |

Key types: `MethodChain`, `ChainedCall`, `IfChain`, `ForChain`, `BreakPoint`, `ParenPosition`.

### Layer 5: Orchestration (`formatter/`)

The main formatting engine that coordinates all layers. Implements a two-pass,
width-based algorithm:

1. **Measure pass** (bottom-up): `WidthCalculator` traverses the AST to compute the inline
   width of each node. Results are cached in an `FxHashMap<ExprId, usize>`. Constructs
   that must always be multi-line return the sentinel `ALWAYS_STACKED` (= `usize::MAX`).

2. **Render pass** (top-down): `Formatter::format()` checks the pre-calculated width against
   the current column position. If the expression fits (column + width <= 100), it calls
   `emit_inline()`. If it does not fit, it calls `emit_broken()`. If the width is
   `ALWAYS_STACKED`, it calls `emit_stacked()`.

Submodules: `inline.rs` (single-line rendering), `broken.rs` (multi-line rendering),
`stacked.rs` (always-multi-line constructs), `patterns.rs` (match/binding patterns),
`literals.rs` (literal value rendering), `helpers.rs` (collection/wrapper helpers).

## CLI Integration

The formatter is invoked through `ori fmt`, implemented in `compiler/oric/src/commands/fmt/`.
The pipeline is:

1. **Preprocess**: `tabs_to_spaces()` normalizes tab characters to spaces.
2. **Lex**: `ori_lexer::lex_with_comments()` produces tokens and a `CommentList`.
3. **Parse**: The parser produces a `Module` AST and an `ExprArena`.
4. **Format**: `format_module_with_comments()` takes the module, comments, arena, and
   interner, then produces the formatted output string.
5. **Write**: The formatted output replaces the original file (or is compared in check mode).

CLI options:
- `ori fmt` -- format all `.ori` files in the current directory (recursive).
- `ori fmt path` -- format a specific file or directory.
- `ori fmt --check` -- exit 1 if any file would change (CI mode).
- `ori fmt --diff` -- show unified diff without modifying files.
- `ori fmt --stdin` -- read from stdin, write to stdout.
- `ori fmt --no-ignore` -- ignore `.orifmtignore` files.

Directory formatting uses `rayon` for parallel file processing.

## Key Design Decisions

### AST-Based, Not Token-Based

The formatter operates on the parsed AST (`ExprArena`, `ExprId`, `ExprKind`), not on a
raw token stream. This means it can make structural decisions (e.g., is this a method chain?
is this a nested for loop?) that a token-level formatter cannot. The trade-off is that the
formatter cannot preserve arbitrary whitespace or comments inline -- comments are handled
through a separate `CommentIndex` that associates comments with AST positions.

### Width-Based Breaking

The core formatting algorithm is width-based: an expression renders inline if it fits
within the 100-character line limit, and breaks to multi-line otherwise. The width of
every expression is pre-computed bottom-up and cached. This two-pass approach avoids
backtracking and ensures O(n) formatting time.

### Independent Nested Breaking

Nested constructs break independently. A function call that fits on one line stays inline
even if the surrounding expression needs to break. This is the same strategy used by
`rustfmt` and prevents unnecessary cascading breaks.

### User Intent Preservation

The formatter preserves user intent in several ways:
- **Trailing commas**: A trailing comma in the source forces multi-line layout (`AlwaysOnePerLine`).
- **Comments inside containers**: Comments between items force multi-line layout.
- **Doc comment reordering**: Doc comments are reordered to canonical order (description,
  members, warnings, examples), but regular comments are left in place.
- **Parentheses**: The `ParenthesesRule` aims to preserve user parentheses, though this is
  currently limited by the AST not tracking explicit parentheses.

### Incremental Formatting

The `incremental` module supports formatting only the declarations that overlap with a
changed byte range. This is designed for LSP integration (format-on-type) and large file
editing. The minimum unit of incremental formatting is a complete top-level declaration.
Changes to imports, constants, or file attributes trigger a full reformat because their
order and grouping affect the entire file.

### Configuration

`FormatConfig` controls three settings:
- `max_width` -- maximum line width (default: 100).
- `indent_size` -- spaces per indentation level (default: 4).
- `trailing_commas` -- `Always` (default), `Never`, or `Preserve`.

These defaults match the Ori formatting spec.

## Emitter Abstraction

Output is produced through the `Emitter` trait, which abstracts over the output destination.
The primary implementation is `StringEmitter`, which builds an in-memory string. The trait
supports `emit()`, `emit_newline()`, `emit_indent()`, and `emit_space()`. The `FormatContext`
wraps an emitter and maintains formatting state (column position, indent level, shape, last
token category). All output flows through the context to keep state synchronized.
