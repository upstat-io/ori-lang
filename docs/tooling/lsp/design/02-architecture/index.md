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
│   ├── lib.rs              # Core logic (shared native/WASM)
│   ├── main.rs             # Native: lsp-server + main loop
│   ├── wasm.rs             # WASM: direct function exports
│   │
│   ├── state.rs            # GlobalState + GlobalStateSnapshot
│   ├── dispatch.rs         # Request/notification routing
│   ├── files.rs            # FileSystemProxy (in-memory cache)
│   │
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── diagnostics.rs  # publishDiagnostics + SuggestedFix
│   │   ├── formatting.rs   # textDocument/formatting
│   │   ├── hover.rs        # textDocument/hover
│   │   ├── definition.rs   # textDocument/definition (Phase 2)
│   │   └── completion.rs   # textDocument/completion (Phase 3)
│   │
│   └── feedback.rs         # DiagnosticTracker (incremental updates)
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

### GlobalState (Mutable, Main Thread)

The main thread owns mutable state. Workers receive immutable snapshots.

```rust
/// Mutable state owned by main thread (rust-analyzer pattern)
pub struct GlobalState {
    /// File content with in-memory cache for unsaved edits
    files: FileSystemProxy,

    /// Tracks which files have diagnostics (for incremental updates)
    diagnostic_tracker: DiagnosticTracker,

    /// Connection for sending responses/notifications
    connection: lsp_server::Connection,

    /// Sender for worker thread results
    task_sender: crossbeam_channel::Sender<Task>,
}

impl GlobalState {
    /// Create immutable snapshot for worker threads
    pub fn snapshot(&self) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            files: self.files.clone(),
        }
    }
}
```

### GlobalStateSnapshot (Immutable, Worker Threads)

```rust
/// Immutable snapshot for thread-safe parallel work
#[derive(Clone)]
pub struct GlobalStateSnapshot {
    files: FileSystemProxy,
}

impl GlobalStateSnapshot {
    pub fn read_file(&self, uri: &Url) -> Option<String> {
        self.files.read(uri)
    }
}
```

### FileSystemProxy (Gleam Pattern)

Transparent cache for unsaved editor content:

```rust
/// In-memory cache layered over filesystem (Gleam pattern)
#[derive(Clone)]
pub struct FileSystemProxy {
    /// Unsaved edits from editor
    memory: Arc<RwLock<HashMap<Url, String>>>,
}

impl FileSystemProxy {
    pub fn read(&self, uri: &Url) -> Option<String> {
        // Check in-memory cache first
        if let Some(content) = self.memory.read().unwrap().get(uri) {
            return Some(content.clone());
        }

        // Fall back to disk (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = uri.to_file_path().ok()?;
            std::fs::read_to_string(path).ok()
        }

        #[cfg(target_arch = "wasm32")]
        None
    }

    pub fn write_memory(&self, uri: Url, content: String) {
        self.memory.write().unwrap().insert(uri, content);
    }

    pub fn remove_memory(&self, uri: &Url) {
        self.memory.write().unwrap().remove(uri);
    }
}
```

### DiagnosticTracker (Gleam Pattern)

Track which files have diagnostics for incremental updates:

```rust
/// Tracks diagnostic state for incremental publishing (Gleam pattern)
pub struct DiagnosticTracker {
    files_with_errors: HashSet<Url>,
    files_with_warnings: HashSet<Url>,
}

impl DiagnosticTracker {
    /// Only clear diagnostics for files we're about to update
    pub fn publish_update(
        &mut self,
        connection: &Connection,
        compiled_files: &[Url],
        new_diagnostics: HashMap<Url, Vec<Diagnostic>>,
    ) {
        // Clear diagnostics only for files that were recompiled
        for uri in compiled_files {
            if !new_diagnostics.contains_key(uri) {
                self.publish_empty(connection, uri);
            }
        }

        // Publish new diagnostics
        for (uri, diagnostics) in new_diagnostics {
            self.publish(connection, uri, diagnostics);
        }
    }
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
# Compiler crates
ori_ir = { path = "../ori_ir" }
ori_lexer = { path = "../ori_lexer" }
ori_parse = { path = "../ori_parse" }
ori_typeck = { path = "../ori_typeck" }
ori_fmt = { path = "../ori_fmt" }

# Shared
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Native-only: lsp-server (used by Gleam, rust-analyzer)
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
lsp-server = "0.7"           # Generic LSP transport
lsp-types = "0.95"           # LSP protocol types
crossbeam-channel = "0.5"    # Main loop channels

# WASM-only
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"
```

**Why `lsp-server` over `tower-lsp`**:
- Simpler: no async runtime required
- Battle-tested: used by Gleam and rust-analyzer
- Single-threaded main loop is easier to reason about
- Better fit for snapshot pattern (no async lifetimes)

## Main Loop Architecture (Native)

Single-threaded main loop with worker threads (rust-analyzer pattern):

```rust
// main.rs - Native entry point
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<()> {
    // lsp-server handles stdio transport
    let (connection, io_threads) = lsp_server::Connection::stdio();

    // Initialize
    let (id, params) = connection.initialize_start()?;
    let init_params: InitializeParams = serde_json::from_value(params)?;
    let capabilities = server_capabilities();
    connection.initialize_finish(id, serde_json::to_value(capabilities)?)?;

    // Run main loop
    main_loop(connection, init_params)?;

    io_threads.join()?;
    Ok(())
}
```

### Event Loop with select!

```rust
fn main_loop(connection: Connection, params: InitializeParams) -> Result<()> {
    let (task_sender, task_receiver) = crossbeam_channel::unbounded::<Task>();

    let mut state = GlobalState::new(connection, task_sender);

    loop {
        // Select across multiple event sources (rust-analyzer pattern)
        crossbeam_channel::select! {
            // LSP messages from client (highest priority)
            recv(state.connection.receiver) -> msg => {
                match msg? {
                    Message::Request(req) => {
                        if state.connection.handle_shutdown(&req)? {
                            return Ok(());
                        }
                        handle_request(&mut state, req);
                    }
                    Message::Notification(notif) => {
                        handle_notification(&mut state, notif);
                    }
                    Message::Response(resp) => {
                        // Handle responses to our requests (rare)
                    }
                }
            }

            // Results from worker threads
            recv(task_receiver) -> task => {
                handle_task_result(&mut state, task?);
            }
        }
    }
}
```

### Request Dispatch

```rust
fn handle_request(state: &mut GlobalState, req: Request) {
    // Route by method name (Gleam uses enum, rust-analyzer uses strings)
    let result = match req.method.as_str() {
        "textDocument/hover" => {
            let params: HoverParams = serde_json::from_value(req.params)?;
            // Spawn on thread pool with snapshot
            let snapshot = state.snapshot();
            std::thread::spawn(move || {
                handlers::hover(snapshot, params)
            });
            return; // Response sent via task channel
        }
        "textDocument/formatting" => {
            let params: DocumentFormattingParams = serde_json::from_value(req.params)?;
            // Formatting is fast, run synchronously
            handlers::formatting(&state.files, params)
        }
        _ => Err(LspError::MethodNotFound),
    };

    // Send response
    let response = match result {
        Ok(value) => Response::new_ok(req.id, value),
        Err(e) => Response::new_err(req.id, e.code(), e.message()),
    };
    state.connection.sender.send(Message::Response(response))?;
}
```

## WASM Entry Points

```rust
// wasm.rs - WASM entry point
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmLanguageServer {
    files: FileSystemProxy,
    diagnostic_tracker: DiagnosticTracker,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmLanguageServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            files: FileSystemProxy::new(),
            diagnostic_tracker: DiagnosticTracker::new(),
        }
    }

    // Direct method calls (no message passing in WASM)
    pub fn open_document(&mut self, uri: &str, content: &str) {
        let uri = Url::parse(uri).unwrap();
        self.files.write_memory(uri, content.to_string());
    }

    pub fn format(&self, uri: &str) -> Option<String> {
        let uri = Url::parse(uri).ok()?;
        let content = self.files.read(&uri)?;
        ori_fmt::format(&content).ok()
    }

    pub fn get_diagnostics(&self, uri: &str) -> String {
        let uri = Url::parse(uri).unwrap();
        let diagnostics = handlers::compute_diagnostics(&self.files, &uri);
        serde_json::to_string(&diagnostics).unwrap()
    }
}
```

## Error Handling

Use a unified error type with LSP error codes:

```rust
#[derive(Debug)]
pub enum LspError {
    /// Request parsing failed
    ParseError(String),

    /// Document not found
    DocumentNotFound(Url),

    /// Method not supported
    MethodNotFound,

    /// Compiler error (parse, type check)
    CompilerError(String),

    /// Internal error
    Internal(String),
}

impl LspError {
    pub fn code(&self) -> i32 {
        match self {
            LspError::ParseError(_) => -32700,      // Parse error
            LspError::DocumentNotFound(_) => -32602, // Invalid params
            LspError::MethodNotFound => -32601,      // Method not found
            LspError::CompilerError(_) => -32603,    // Internal error
            LspError::Internal(_) => -32603,         // Internal error
        }
    }

    pub fn message(&self) -> String {
        match self {
            LspError::ParseError(msg) => msg.clone(),
            LspError::DocumentNotFound(uri) => format!("Document not found: {}", uri),
            LspError::MethodNotFound => "Method not found".to_string(),
            LspError::CompilerError(msg) => msg.clone(),
            LspError::Internal(msg) => msg.clone(),
        }
    }
}
```

### Panic Safety (rust-analyzer pattern)

Worker threads catch panics to avoid crashing the server:

```rust
fn spawn_handler<F, R>(snapshot: GlobalStateSnapshot, f: F) -> JoinHandle<Result<R>>
where
    F: FnOnce(GlobalStateSnapshot) -> R + Send + 'static,
    R: Send + 'static,
{
    std::thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(snapshot)))
            .map_err(|_| LspError::Internal("Handler panicked".to_string()))
    })
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
