# Ori Formatter Roadmap

**Zero-config formatting with one canonical style.**

This roadmap tracks implementation of `ori fmt`, the official Ori formatter. The formatter enforces a single, consistent style across all Ori codebases—no configuration, no debates.

## Philosophy

**Width-based, not count-based.** The formatter uses a simple rule: inline if ≤100 characters, break otherwise. No arbitrary thresholds like "break if >3 parameters."

**Two-pass algorithm:**
1. **Measure (bottom-up)**: Calculate inline width of each AST node
2. **Render (top-down)**: Decide inline vs broken format based on measured widths

## Tier Overview

| Tier | Phases | Focus |
|------|--------|-------|
| **Tier 1: Foundation** | 1-2 | Core algorithm and declarations |
| **Tier 2: Expressions** | 3-4 | Expression and pattern formatting |
| **Tier 3: Collections & Comments** | 5-6 | Collections and comment handling |
| **Tier 4: Integration** | 7-8 | Tooling integration and polish |

## Phases

| Phase | Name | Description |
|-------|------|-------------|
| 1 | Core Algorithm | Width calculation, two-pass rendering, line breaking |
| 2 | Declarations | Functions, types, traits, impls, imports, constants |
| 3 | Expressions | Calls, chains, conditionals, lambdas, binary expressions |
| 4 | Patterns | run, try, match, recurse, parallel, spawn, nursery |
| 5 | Collections | Lists, maps, tuples, struct literals, ranges |
| 6 | Comments | Comment handling, doc comment reordering |
| 7 | Tooling Integration | CLI, LSP integration, WASM for playground |
| 8 | Edge Cases & Polish | Comprehensive edge cases, performance optimization |

## Key Design Documents

| Document | Purpose |
|----------|---------|
| `docs/tooling/formatter/design/index.md` | Main overview and philosophy |
| `docs/tooling/formatter/design/01-algorithm/` | Algorithm details |
| `docs/tooling/formatter/design/02-constructs/` | Construct-specific rules |
| `docs/tooling/formatter/design/03-comments/` | Comment formatting |
| `docs/tooling/formatter/design/04-implementation/` | Implementation architecture |
| `docs/tooling/formatter/design/appendices/` | Edge cases |

## Core Rules

| Rule | Specification |
|------|---------------|
| **Indentation** | 4 spaces, no tabs |
| **Line Width** | 100 characters hard limit |
| **Trailing Commas** | Required in multi-line; forbidden in single-line |
| **Blank Lines** | One between top-level items; no consecutive blank lines |
| **Spacing** | Space around binary ops; space after colons/commas |

## Dependencies

```
Phase 1 (Core Algorithm)
    ↓
Phase 2 (Declarations) ← Can start after Phase 1 basics
    ↓
Phase 3 (Expressions) ← Requires Phase 1 complete
    ↓
Phase 4 (Patterns) ← Requires Phase 3 basics
    ↓
Phase 5 (Collections) ← Requires Phase 1 complete
    ↓
Phase 6 (Comments) ← Requires Phase 1-2 complete
    ↓
Phase 7 (Tooling) ← Requires Phases 1-6 substantially complete
    ↓
Phase 8 (Polish) ← Can run in parallel with Phase 7
```

## Implementation Crates

| Crate | Purpose |
|-------|---------|
| `ori_fmt` | Core formatting logic |
| `ori_lsp` | LSP server (depends on ori_fmt) |
| `playground/wasm` | WASM compilation for browser |
