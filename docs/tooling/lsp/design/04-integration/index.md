---
title: "Integration Overview"
description: "Ori LSP Design — Client Integration"
order: 1
---

# Integration Overview

How the Ori LSP server integrates with various clients: Playground, VS Code, Neovim, and others.

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │           ori_lsp crate             │
                    │    (shared formatting, analysis)    │
                    └─────────────────────────────────────┘
                              │               │
            ┌─────────────────┴───────────────┴─────────────────┐
            │                                                   │
            ▼                                                   ▼
    ┌───────────────┐                               ┌───────────────────┐
    │  WASM Build   │                               │   Native Build    │
    │  (browser)    │                               │   (desktop)       │
    └───────────────┘                               └───────────────────┘
            │                                                   │
            ▼                                                   ▼
    ┌───────────────┐                       ┌───────────────────────────┐
    │   Playground  │                       │      Desktop Editors      │
    │   (Monaco)    │                       │  ┌─────────┬─────────┐    │
    └───────────────┘                       │  │ VS Code │ Neovim  │    │
                                            │  └─────────┴─────────┘    │
                                            └───────────────────────────┘
```

## Client Comparison

| Feature | Playground | VS Code | Neovim |
|---------|------------|---------|--------|
| Transport | In-memory | stdio | stdio |
| Build | WASM | Native | Native |
| Diagnostics | Immediate | Push | Push |
| Formatting | Format-on-Run | Format-on-Save/Cmd | Format-on-Save/Cmd |
| Hover | Yes | Yes | Yes |
| Multi-file | No (single file) | Yes | Yes |

## Integration Points

### Playground

See [Playground Integration](playground.md) for details.

Key characteristics:
- Single file only
- WASM module loaded in browser
- Format-on-Run (not explicit format command)
- Immediate feedback (no debounce needed for single file)

### VS Code

See [Editor Integration](editors.md) for details.

Key characteristics:
- Multi-file workspace support
- Extension spawns `ori_lsp` binary
- Full LSP protocol over stdio
- Rich extension ecosystem (syntax highlighting, snippets, etc.)

### Neovim

See [Editor Integration](editors.md) for details.

Key characteristics:
- Native LSP client (built-in since 0.5)
- Minimal configuration needed
- Works with existing Neovim LSP plugins

## Shared Code

The `ori_lsp` crate is designed for maximum code reuse:

```rust
// lib.rs - shared across all targets

pub struct OriLanguageServer {
    documents: DocumentManager,
    // No transport-specific state
}

impl OriLanguageServer {
    pub fn new() -> Self { ... }

    // Core operations - used by all clients
    pub fn open_document(&mut self, uri: &str, text: &str) { ... }
    pub fn update_document(&mut self, uri: &str, text: &str) { ... }
    pub fn close_document(&mut self, uri: &str) { ... }

    pub fn format(&self, uri: &str) -> Option<String> { ... }
    pub fn hover(&self, uri: &str, line: u32, col: u32) -> Option<Hover> { ... }
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> { ... }
}
```

Target-specific entry points:

```rust
// main.rs - native binary
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let server = OriLanguageServer::new();
    run_stdio_server(server);
}

// wasm.rs - WASM module
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmLanguageServer(OriLanguageServer);

#[wasm_bindgen]
impl WasmLanguageServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(OriLanguageServer::new())
    }

    pub fn format(&self, uri: &str) -> Option<String> {
        self.0.format(uri)
    }
    // ... wrap other methods
}
```

## Feature Availability

Not all features make sense for all clients:

| Feature | Playground | Desktop Editors |
|---------|------------|-----------------|
| Format document | Yes | Yes |
| Diagnostics | Yes | Yes |
| Hover | Yes | Yes |
| Go to definition | No* | Yes |
| Find references | No* | Yes |
| Workspace symbols | No | Yes |
| Multi-file diagnostics | No | Yes |
| Code actions | Limited | Yes |

*Single-file playground doesn't have cross-file navigation

## Rollout Strategy

### Phase 1: Playground Sandbox

1. Build `ori_lsp` WASM module
2. Integrate with existing Playground
3. Test diagnostics, hover, formatting
4. Gather feedback

Benefits:
- Low-stakes environment
- Fast iteration
- Immediate user feedback

### Phase 2: VS Code Extension

1. Create minimal VS Code extension
2. Bundle native `ori_lsp` binary
3. Configure language association (`.ori` files)
4. Publish to marketplace

### Phase 3: Neovim Support

1. Document LSP configuration
2. Create `ftplugin/ori.lua` for easy setup
3. Test with common Neovim LSP plugins

### Phase 4: Enhanced Features

1. Go to definition
2. Find references
3. Completions
4. Code actions (quick fixes)

## Testing Strategy

### Unit Tests

Test core logic in isolation:

```rust
#[test]
fn test_hover_returns_type() {
    let server = OriLanguageServer::new();
    server.open_document("file:///test.ori", "let x: int = 42");

    let hover = server.hover("file:///test.ori", 0, 4);

    assert!(hover.is_some());
    assert!(hover.unwrap().contents.value.contains("int"));
}
```

### Integration Tests

Test full protocol flow:

```rust
#[tokio::test]
async fn test_initialize_shutdown() {
    let (client, server) = create_test_pair();

    let init_result = client.initialize().await;
    assert!(init_result.capabilities.hover_provider.is_some());

    client.shutdown().await;
    client.exit().await;
}
```

### E2E Tests

Test in real environments:

```bash
# Playground: Cypress/Playwright tests
npm run test:e2e

# VS Code: VS Code extension tests
npm run test:vscode

# Neovim: Neovim headless tests
nvim --headless -c "lua require('test').run()"
```
