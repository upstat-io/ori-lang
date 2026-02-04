---
title: "Overview"
description: "Ori Formatter Design — Implementation Guide"
order: 0
---

# Overview

This documentation describes the design and implementation of the Ori formatter. The formatter produces canonical source code formatting with minimal configuration (only line width is configurable).

## Reference: Go's gofmt

The Ori formatter follows the philosophy established by Go's `gofmt`:

> "No one is 100% happy with gofmt, but people adapt surprisingly quickly to styles that at first seem foreign. We hope people will accept the output precisely because it puts an end to style debates."

**Key principles from gofmt:**

| Principle | gofmt | Ori |
|-----------|-------|-----|
| Configuration | None — deliberately denied | Width only (default 100) |
| Specification | "Implementation is the spec" | Formatter output is canonical |
| Determinism | Idempotent, same input → same output | Same guarantee |
| Debates | Eliminated by design | Eliminated by design |

**Where Ori differs from gofmt:**

- **Line width limit**: gofmt has none; Ori enforces 100 characters
- **Width-based breaking**: gofmt trusts source; Ori breaks automatically at width
- **Indentation**: gofmt uses tabs; Ori uses 4 spaces
- **Always-stacked constructs**: Ori has `run`, `try`, `match`, etc.

## Design Principles

1. **Minimal configuration** — Only line width is configurable (default 100)
2. **Deterministic** — Same input always produces same output
3. **Idempotent** — `format(format(code)) == format(code)`
4. **Semantic preservation** — Only whitespace changes, never meaning
5. **Width-driven breaking** — Lines break when exceeding the configured width

## Core Rules

| Rule | Value | Configurable |
|------|-------|--------------|
| Line width | 100 characters (default) | Yes (`--width=N`) |
| Indentation | 4 spaces, no tabs | No |
| Trailing commas | Required in multi-line, forbidden in single-line | No |
| Blank lines | One between top-level items, no consecutive | No |

## Breaking Philosophy

The formatter follows a simple principle: **inline until max width, then break**.

There are no arbitrary thresholds like "break if more than 3 parameters." Instead:
- Measure the line width
- If it fits within the configured width (default 100), keep it inline
- If it exceeds the width limit, break according to construct-specific rules

Exceptions exist only for constructs that are *always* stacked regardless of width:
- `run` / `try` — sequential blocks always stack
- `match` — arms always stack (scrutinee on first line)

## Documentation Sections

### Algorithm

- [Algorithm Overview](01-algorithm/) — Core formatting algorithm
- [Line Breaking](01-algorithm/line-breaking.md) — When and how to break lines
- [Indentation](01-algorithm/indentation.md) — Indentation rules and nesting

### Constructs

- [Constructs Overview](02-constructs/) — Per-construct formatting rules
- [Declarations](02-constructs/declarations.md) — Functions, types, traits, impls
- [Expressions](02-constructs/expressions.md) — Calls, chains, conditionals, lambdas
- [Patterns](02-constructs/patterns.md) — run, try, match, recurse, parallel
- [Collections](02-constructs/collections.md) — Lists, maps, tuples, structs

### Layers

- [5-Layer Architecture](03-layers/) — Modular architecture overview
- [Layer 1: Token Spacing](03-layers/01-spacing.md) — O(1) declarative spacing rules
- [Layer 2: Container Packing](03-layers/02-packing.md) — Inline vs break decisions
- [Layer 3: Shape Tracking](03-layers/03-shape.md) — Width tracking for fit decisions
- [Layer 4: Breaking Rules](03-layers/04-rules.md) — Ori-specific breaking rules

### Comments

- [Comments](03-comments/) — Comment handling and doc comment ordering

### Implementation

- [Implementation Overview](04-implementation/) — Implementation approach
- [Tooling Integration](04-implementation/#tooling-integration) — Crates, LSP, Playground, editors
- [AST Integration](04-implementation/ast-integration.md) — Working with the Ori AST

### Appendices

- [Edge Cases](appendices/A-edge-cases.md) — Comprehensive edge case examples

## Tooling

| Component | Location | Purpose |
|-----------|----------|---------|
| `ori_fmt` | `compiler/ori_fmt/` | Core formatting logic |
| `ori_lsp` | `compiler/ori_lsp/` | LSP server (formatting, diagnostics, hover) |

**Playground**: Format-on-Run — code formats automatically when user clicks Run. LSP compiled to WASM provides real-time diagnostics and hover in Monaco.

**Editors**: Same `ori_lsp` binary serves VS Code, Neovim, and other LSP-compatible editors.

See [Tooling Integration](04-implementation/#tooling-integration) for architecture details, or the [LSP Design docs](../lsp/design/) for full LSP documentation.

## Relationship to Spec

The normative formatting rules are defined in [spec/16-formatting.md](/docs/ori_lang/0.1-alpha/spec/16-formatting.md). This design documentation explains *how* to implement those rules, with detailed algorithms and edge cases.

| Document | Purpose |
|----------|---------|
| `spec/16-formatting.md` | *What* the canonical format is (normative) |
| `docs/tooling/formatter/design/` | *How* to implement the formatter (informative) |

## Quick Reference

### Always Inline (unless >100 chars)

- Function signatures
- Type definitions (structs, sum types)
- Generic parameters
- Where clauses (single constraint)
- Collections (lists, maps, tuples)
- Struct literals
- Function calls
- Conditionals
- Chains (≤100 chars)

### Always Stacked

- `run` / `try` blocks
- `match` arms
- `recurse`
- `parallel` / `spawn`
- `nursery`

### Break Triggers

When a line exceeds 100 characters:

| Construct | Breaking Behavior |
|-----------|-------------------|
| Function params | One per line, trailing comma |
| Return type | Own line if `) -> Type` exceeds 100 |
| Generic params | One per line |
| Where clauses | `where` on new line, constraints indented |
| Function calls | Arguments one per line |
| Collections | Simple items wrap multiple per line, complex items one per line |
| Struct literals | Fields one per line |
| Chains | Each `.method()` on own line |
| Conditionals | `if cond then expr` together, `else` on new line |
| Binary expressions | Break before operator |
| Lambdas | Break after `->` only for always-stacked patterns |
