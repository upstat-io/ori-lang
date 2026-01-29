---
title: "Overview"
description: "Ori Language Server Design — Implementation Guide"
order: 0
---

# Overview

This documentation describes the design and implementation of the Ori Language Server (`ori_lsp`). The language server provides IDE features via the Language Server Protocol (LSP).

## Goals

1. **Single implementation, multiple clients** — One LSP server serves VS Code, Neovim, Playground, and any LSP-compatible editor
2. **WASM-first** — Compiles to WebAssembly for in-browser Playground use
3. **Incremental** — Start with essential features, expand over time
4. **Integrated** — Leverages existing compiler infrastructure (`ori_fmt`, `ori_typeck`, etc.)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ori_lsp crate                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Protocol   │  │  Features   │  │  Document Manager   │  │
│  │  Handler    │  │  (handlers) │  │  (open files, sync) │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│         │               │                    │              │
│         ▼               ▼                    ▼              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Compiler Components                     │   │
│  │  ori_fmt │ ori_typeck │ ori_parse │ ori_lexer        │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         │                              │
         ▼                              ▼
   Native binary                  WASM module
   (VS Code, Neovim)              (Playground)
```

## Feature Roadmap

### Phase 1: Foundation

| Feature | LSP Method | Priority |
|---------|------------|----------|
| Formatting | `textDocument/formatting` | P0 |
| Diagnostics | `textDocument/publishDiagnostics` | P0 |
| Hover | `textDocument/hover` | P0 |

### Phase 2: Navigation

| Feature | LSP Method | Priority |
|---------|------------|----------|
| Go to Definition | `textDocument/definition` | P1 |
| Find References | `textDocument/references` | P1 |
| Document Symbols | `textDocument/documentSymbol` | P1 |

### Phase 3: Editing

| Feature | LSP Method | Priority |
|---------|------------|----------|
| Completion | `textDocument/completion` | P2 |
| Signature Help | `textDocument/signatureHelp` | P2 |
| Rename | `textDocument/rename` | P2 |

### Phase 4: Advanced

| Feature | LSP Method | Priority |
|---------|------------|----------|
| Code Actions | `textDocument/codeAction` | P3 |
| Inlay Hints | `textDocument/inlayHint` | P3 |
| Semantic Tokens | `textDocument/semanticTokens` | P3 |

## Documentation Sections

### Protocol

- [Protocol Overview](01-protocol/index.md) — LSP methods and lifecycle
- [Document Sync](01-protocol/document-sync.md) — Text synchronization strategy

### Architecture

- [Architecture Overview](02-architecture/index.md) — Crate structure and dependencies
- [WASM Compilation](02-architecture/wasm.md) — Browser deployment

### Features

- [Features Overview](03-features/index.md) — Feature implementations
- [Diagnostics](03-features/diagnostics.md) — Error and warning reporting
- [Hover](03-features/hover.md) — Type information display
- [Formatting](03-features/formatting.md) — Code formatting integration

### Integration

- [Integration Overview](04-integration/index.md) — Client integration
- [Playground](04-integration/playground.md) — Browser-based Monaco integration
- [Editors](04-integration/editors.md) — VS Code, Neovim configuration

## Crate Location

```
compiler/ori_lsp/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Core LSP logic (shared native/WASM)
│   ├── main.rs          # Native binary entry point
│   ├── protocol/        # LSP message handling
│   ├── features/        # Feature implementations
│   └── document.rs      # Document state management
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ori_fmt` | Code formatting |
| `ori_typeck` | Type checking, type info for hover |
| `ori_parse` | Parsing for diagnostics |
| `ori_lexer` | Tokenization |
| `ori_ir` | AST and spans |
| `tower-lsp` | LSP protocol implementation (native) |
| `wasm-bindgen` | WASM bindings (browser) |

## Design Principles

1. **Leverage existing infrastructure** — Use `ori_fmt` for formatting, `ori_typeck` for type info
2. **Stateless where possible** — Minimize server state for simplicity
3. **Fast feedback** — Prioritize responsiveness over completeness
4. **Graceful degradation** — Partial results better than failure
