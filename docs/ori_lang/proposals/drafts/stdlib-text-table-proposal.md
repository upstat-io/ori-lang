# Proposal: std.text.table — Terminal Table Formatting

**Status:** Draft
**Created:** 2026-04-02
**Author:** Eric (with AI assistance)
**Affects:** Standard library, `library/std/text/table/`
**Depends on:** stdlib-text-api-proposal.md (approved) — requires `std.text.width` for display-width-aware column sizing
**Prior art:** Go `tablewriter`, Python `tabulate`, Rust `tabled`/`comfy-table`, Python `rich.table`

---

## Summary

This proposal defines `std.text.table` — a submodule of `std.text` for rendering column-aligned tabular data in terminals. Supports auto-sizing columns, text alignment, word wrapping within cells, Unicode display-width-aware formatting, ANSI color preservation, and configurable border styles.

---

## Motivation

Presenting tabular data is one of the most common CLI output patterns. Every language's ecosystem has popular third-party table formatters:

- **Go**: `tablewriter` (~3.6K stars) — used by kubectl, terraform, etc.
- **Python**: `tabulate` (~25M monthly downloads), `rich.table` (~50K stars)
- **Rust**: `tabled` (~2K stars), `comfy-table` (~1K stars)
- **JavaScript**: `cli-table3` (~2M weekly downloads)

All of these need display-width-aware column sizing (CJK = 2 columns, emoji = 2 columns). With `std.text.width` providing `display_width`, `std.text.table` can do this correctly out of the box.

---

## Scope

### In Scope

- Column auto-sizing based on content display width
- Text alignment (left, right, center) per column
- Word wrapping within cells (using `std.text.wrap`)
- Configurable border styles (ASCII, Unicode box-drawing, minimal, none)
- Header rows
- ANSI color-safe (colors don't affect width calculations)
- Separator rows
- Maximum column width with truncation

### Out of Scope (Future)

- Cell merging / spanning
- CSV/TSV input parsing (belongs in `std.encoding` or `std.csv`)
- HTML/Markdown/LaTeX output formats
- Interactive/scrollable tables (belongs in a TUI framework)
- Sorting/filtering (application logic, not formatting)

---

## API Sketch

```ori
use std.text.table { Table, BorderStyle }

let table = Table.new(headers: ["Name", "Score", "Grade"])
    |> .add_row(["Alice", "95", "A"])
    |> .add_row(["張三", "88", "B+"])
    |> .add_row(["Bob", "72", "C"])
    |> .border(BorderStyle.Rounded)
    |> .align_column(index: 1, align: Alignment.Right)

print(msg: table.render())

// ╭───────┬───────┬───────╮
// │ Name  │ Score │ Grade │
// ├───────┼───────┼───────┤
// │ Alice │    95 │ A     │
// │ 張三  │    88 │ B+    │
// │ Bob   │    72 │ C     │
// ╰───────┴───────┴───────╯
```

---

## Detailed Design

*To be expanded during full proposal development.*

---

## Open Questions

1. Should `Table` be a builder pattern (fluent API) or a configuration struct?
2. Should there be a streaming mode for large tables (emit rows as they come)?
3. What border styles should ship built-in? (ASCII, Unicode rounded, Unicode sharp, minimal/headerline only, none)
