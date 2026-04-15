---
paths:
  - "**fmt**"
  - "**ori_fmt**"
---

# Formatter (ori_fmt)

## Pipeline Position

Not a compiler phase — operates on the AST (post-parse) for source formatting. Input: `Module` + `ExprArena` + `StringInterner`. Independent of type checking. Invoked via `ori fmt` CLI or `ori_compiler::format_module()`.

## 5-Layer Architecture

| Layer | Module | Responsibility |
|---|---|---|
| 1. Spacing | `spacing` | Declarative O(1) token spacing rules |
| 2. Packing | `packing` | Container packing decisions (fit/break) |
| 3. Shape | `shape` | Width tracking through recursion |
| 4. Breaking | `rules` | Ori-specific breaking rules |
| 5. Orchestration | `formatter` | Main formatter coordinating all layers |

## Algorithm

Two-pass, width-based breaking:
1. **Measure pass**: bottom-up traversal calculating inline width of each node
2. **Render pass**: top-down rendering deciding inline vs broken based on width

Core principle: render inline if it fits (≤100 chars), break otherwise.

## Constants

- `INDENT_WIDTH` — indentation step (4 spaces)
- `MAX_LINE_WIDTH` — line width limit (100 chars)
- `ALWAYS_STACKED` — constructs that always use stacked layout (match, try, recurse, parallel, spawn, nursery)

## Idempotency

Formatting is idempotent — `format(format(input)) == format(input)`. A non-idempotent format is a bug.

## Comment Preservation

The formatter preserves comments through a separate comment-aware pipeline: `lex_with_comments` → parse → `format_module_with_comments_and_config`. Comments are associated with the nearest AST node and re-emitted at the correct position in the formatted output.

## Public API

- `format_module(module, arena, interner) -> String` — basic formatting
- `format_module_with_comments(module, arena, interner, comments) -> String` — comment-preserving
- `format_module_with_comments_and_config(module, arena, interner, comments, config) -> String` — full API
- `format_module_with_config(module, arena, interner, config) -> String` — configurable, no comments
- `format_expr(expr_id, arena, interner) -> String` — single expression
- `format_incremental(source, regions) -> String` — partial formatting
- `apply_regions(source, formatted, regions) -> String` — region-based application
- `tabs_to_spaces(source) -> String` — whitespace normalization

## Key Types

- `FormatConfig` — formatting configuration (width, indent, trailing commas policy)
- `FormatContext` — per-formatting-session state
- `TrailingCommas` — trailing comma policy enum
- `Shape` — width tracking state
- `Packing` — fit/break decision result
- `WidthCalculator` — bottom-up width computation

## Formatting Rules (from ori-syntax.md §Formatting)

4 spaces, 100 char limit, trailing commas multi-line only. See `ori-syntax.md §Formatting` for the complete user-facing formatting rules.
