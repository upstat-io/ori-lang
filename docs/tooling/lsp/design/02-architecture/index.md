---
title: "Architecture Overview"
description: "Ori LSP Design — Crate Structure and Dependencies"
order: 1
---

# Architecture Overview

The LSP server is structured for code reuse between native and WASM targets.

## Crate Structure

```
compiler/ori_lsp/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Core logic (shared)
│   ├── main.rs             # Native entry point (stdio transport)
│   ├── wasm.rs             # WASM entry point (JS bindings)
│   │
│   ├── protocol/
│   │   ├── mod.rs          # Protocol types
│   │   ├── handler.rs      # Request/notification dispatch
│   │   └── transport.rs    # Transport abstraction
│   │
│   ├── features/
│   │   ├── mod.rs
│   │   ├── diagnostics.rs  # publishDiagnostics
│   │   ├── formatting.rs   # textDocument/formatting
│   │   ├── hover.rs        # textDocument/hover
│   │   ├── definition.rs   # textDocument/definition (Phase 2)
│   │   └── completion.rs   # textDocument/completion (Phase 3)
│   │
│   └── document.rs         # DocumentManager
```

## Dependency Graph

```
┌─────────────────────────────────────────────────────────┐
│                      ori_lsp                            │
│                                                         │
│   ┌─────────────┐    ┌─────────────┐    ┌───────────┐   │
│   │  features/  │    │  protocol/  │    │ document  │   │
│   │             │    │             │    │           │   │
│   └──────┬──────┘    └──────┬──────┘    └─────┬─────┘   │
│          │                  │                 │         │
└──────────┼──────────────────┼─────────────────┼─────────┘
           │                  │                 │
           ▼                  ▼                 ▼
    ┌──────────────────────────────────────────────────┐
    │              Compiler Crates                     │
    │                                                  │
    │  ori_fmt ◄─┐                                     │
    │            │                                     │
    │  ori_typeck ◄─┬─────── ori_ir                    │
    │               │           ▲                      │
    │  ori_parse ◄──┘           │                      │
    │      ▲                    │                      │
    │      │                    │                      │
    │  ori_lexer ───────────────┘                      │
    └──────────────────────────────────────────────────┘
```

## Core Types

### Language Server

```rust
pub struct OriLanguageServer {
    /// Document state management
    documents: DocumentManager,

    /// Client for sending notifications
    client: Client,

    /// Server configuration
    config: ServerConfig,
}

impl OriLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            documents: DocumentManager::new(),
            client,
            config: ServerConfig::default(),
        }
    }
}
```

### Feature Trait

Each feature implements a common interface:

```rust
pub trait Feature {
    /// Handle the LSP request/notification
    async fn handle(&self, server: &OriLanguageServer, params: Value) -> Result<Value>;
}
```

### Transport Abstraction

Abstract over stdio (native) and message passing (WASM):

```rust
pub trait Transport {
    async fn read_message(&mut self) -> Result<Message>;
    async fn write_message(&mut self, msg: &Message) -> Result<()>;
}

// Native: stdio
pub struct StdioTransport {
    stdin: BufReader<Stdin>,
    stdout: Stdout,
}

// WASM: JavaScript callbacks
#[cfg(target_arch = "wasm32")]
pub struct WasmTransport {
    pending_messages: VecDeque<Message>,
    on_message: js_sys::Function,
}
```

## Cargo Configuration

```toml
[package]
name = "ori_lsp"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[[bin]]
name = "ori_lsp"
path = "src/main.rs"

[dependencies]
ori_ir = { path = "../ori_ir" }
ori_lexer = { path = "../ori_lexer" }
ori_parse = { path = "../ori_parse" }
ori_typeck = { path = "../ori_typeck" }
ori_fmt = { path = "../ori_fmt" }

serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Native-only dependencies
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }

# WASM-only dependencies
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
```

## Conditional Compilation

Use `cfg` attributes for target-specific code:

```rust
// Entry points
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Native: tower-lsp with tokio
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run_server());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn create_server() -> WasmLanguageServer {
    // WASM: Return handle for JS
    WasmLanguageServer::new()
}
```

```rust
// Transport
impl OriLanguageServer {
    #[cfg(not(target_arch = "wasm32"))]
    async fn send_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    #[cfg(target_arch = "wasm32")]
    fn send_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        let msg = serde_json::to_string(&PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        }).unwrap();
        self.on_notification.call1(&JsValue::NULL, &msg.into()).unwrap();
    }
}
```

## Error Handling

Use a unified error type:

```rust
#[derive(Debug)]
pub enum LspError {
    /// Request parsing failed
    ParseError(String),

    /// Document not found
    DocumentNotFound(Url),

    /// Compiler error (parse, type check)
    CompilerError(String),

    /// Internal error
    Internal(String),
}

impl From<LspError> for tower_lsp::jsonrpc::Error {
    fn from(e: LspError) -> Self {
        match e {
            LspError::ParseError(msg) => Error {
                code: ErrorCode::ParseError,
                message: msg,
                data: None,
            },
            LspError::DocumentNotFound(uri) => Error {
                code: ErrorCode::InvalidParams,
                message: format!("Document not found: {}", uri),
                data: None,
            },
            LspError::CompilerError(msg) => Error {
                code: ErrorCode::InternalError,
                message: msg,
                data: None,
            },
            LspError::Internal(msg) => Error {
                code: ErrorCode::InternalError,
                message: msg,
                data: None,
            },
        }
    }
}
```

## Testing Strategy

### Unit Tests

Test features in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_on_variable() {
        let code = "let x: int = 42";
        let server = test_server_with_doc("test.ori", code);

        let result = hover(&server, position(0, 4)); // cursor on 'x'

        assert_eq!(result.contents, "```ori\nx: int\n```");
    }

    #[test]
    fn test_diagnostics_parse_error() {
        let code = "let x = ";
        let diagnostics = compute_diagnostics(code);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    }
}
```

### Integration Tests

Test full request/response cycle:

```rust
#[tokio::test]
async fn test_format_request() {
    let (client, server) = test_client_server();

    client.open_document("file:///test.ori", "let x=1").await;

    let edits = client.format("file:///test.ori").await;

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "let x = 1\n");
}
```
