---
title: "WASM Compilation"
description: "Ori LSP Design — WebAssembly Deployment"
order: 2
section: "Architecture"
---

# WASM Compilation

Compiling the LSP server to WebAssembly for browser-based Playground integration.

## Build Process

### Prerequisites

```bash
# Install wasm-pack
cargo install wasm-pack

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### Build Command

```bash
# From compiler/ori_lsp/
wasm-pack build --target web --out-dir ../../playground/wasm-lsp/pkg
```

Output:
```
playground/wasm-lsp/pkg/
├── ori_lsp.js           # JS glue code
├── ori_lsp.d.ts         # TypeScript types
├── ori_lsp_bg.wasm      # WASM binary
└── package.json
```

## JavaScript API

### WASM Exports

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmLanguageServer {
    inner: OriLanguageServer,
}

#[wasm_bindgen]
impl WasmLanguageServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: OriLanguageServer::new_wasm(),
        }
    }

    /// Handle an incoming JSON-RPC message from the client
    #[wasm_bindgen]
    pub fn handle_message(&mut self, message: &str) -> Option<String> {
        let request: Message = serde_json::from_str(message).ok()?;
        let response = self.inner.handle(request);
        response.map(|r| serde_json::to_string(&r).unwrap())
    }

    /// Get pending notifications (diagnostics, etc.)
    #[wasm_bindgen]
    pub fn get_notifications(&mut self) -> Vec<JsValue> {
        self.inner
            .drain_notifications()
            .into_iter()
            .map(|n| JsValue::from_str(&serde_json::to_string(&n).unwrap()))
            .collect()
    }

    /// Open a document
    #[wasm_bindgen]
    pub fn open_document(&mut self, uri: &str, text: &str) {
        self.inner.open_document(uri, text);
    }

    /// Update document content
    #[wasm_bindgen]
    pub fn update_document(&mut self, uri: &str, text: &str) {
        self.inner.update_document(uri, text);
    }

    /// Close a document
    #[wasm_bindgen]
    pub fn close_document(&mut self, uri: &str) {
        self.inner.close_document(uri);
    }

    /// Format a document (returns formatted text)
    #[wasm_bindgen]
    pub fn format(&self, uri: &str) -> Option<String> {
        self.inner.format(uri)
    }

    /// Get hover info at position
    #[wasm_bindgen]
    pub fn hover(&self, uri: &str, line: u32, character: u32) -> Option<String> {
        let result = self.inner.hover(uri, line, character)?;
        Some(serde_json::to_string(&result).unwrap())
    }

    /// Get diagnostics for a document
    #[wasm_bindgen]
    pub fn get_diagnostics(&self, uri: &str) -> String {
        let diagnostics = self.inner.get_diagnostics(uri);
        serde_json::to_string(&diagnostics).unwrap()
    }
}
```

### TypeScript Usage

```typescript
import init, { WasmLanguageServer } from './pkg/ori_lsp.js';

async function createServer(): Promise<WasmLanguageServer> {
    await init();
    return new WasmLanguageServer();
}

// Usage
const server = await createServer();

// Open document
server.open_document('file:///main.ori', 'let x = 42');

// Get diagnostics
const diagnosticsJson = server.get_diagnostics('file:///main.ori');
const diagnostics = JSON.parse(diagnosticsJson);

// Format
const formatted = server.format('file:///main.ori');

// Hover
const hoverJson = server.hover('file:///main.ori', 0, 4);
if (hoverJson) {
    const hover = JSON.parse(hoverJson);
    console.log(hover.contents);
}

// Update on edit
server.update_document('file:///main.ori', 'let x: int = 42');
```

## Monaco Integration

### Language Client

Create a lightweight LSP-like client for Monaco:

```typescript
import * as monaco from 'monaco-editor';
import { WasmLanguageServer } from './pkg/ori_lsp.js';

class OriLanguageClient {
    private server: WasmLanguageServer;
    private diagnosticsCallback: (uri: string, diagnostics: any[]) => void;

    constructor(server: WasmLanguageServer) {
        this.server = server;
    }

    onDiagnostics(callback: (uri: string, diagnostics: any[]) => void) {
        this.diagnosticsCallback = callback;
    }

    openDocument(model: monaco.editor.ITextModel) {
        const uri = model.uri.toString();
        this.server.open_document(uri, model.getValue());
        this.updateDiagnostics(uri);
    }

    updateDocument(model: monaco.editor.ITextModel) {
        const uri = model.uri.toString();
        this.server.update_document(uri, model.getValue());
        this.updateDiagnostics(uri);
    }

    closeDocument(model: monaco.editor.ITextModel) {
        const uri = model.uri.toString();
        this.server.close_document(uri);
    }

    format(model: monaco.editor.ITextModel): monaco.editor.ISingleEditOperation[] | null {
        const uri = model.uri.toString();
        const formatted = this.server.format(uri);
        if (!formatted) return null;

        return [{
            range: model.getFullModelRange(),
            text: formatted,
        }];
    }

    hover(model: monaco.editor.ITextModel, position: monaco.Position): monaco.languages.Hover | null {
        const uri = model.uri.toString();
        const hoverJson = this.server.hover(uri, position.lineNumber - 1, position.column - 1);
        if (!hoverJson) return null;

        const hover = JSON.parse(hoverJson);
        return {
            contents: [{ value: hover.contents.value }],
        };
    }

    private updateDiagnostics(uri: string) {
        const diagnosticsJson = this.server.get_diagnostics(uri);
        const diagnostics = JSON.parse(diagnosticsJson);
        if (this.diagnosticsCallback) {
            this.diagnosticsCallback(uri, diagnostics);
        }
    }
}
```

### Monaco Provider Registration

```typescript
// Register formatting provider
monaco.languages.registerDocumentFormattingEditProvider('ori', {
    provideDocumentFormattingEdits(model) {
        return client.format(model);
    }
});

// Register hover provider
monaco.languages.registerHoverProvider('ori', {
    provideHover(model, position) {
        return client.hover(model, position);
    }
});

// Wire up diagnostics to Monaco markers
client.onDiagnostics((uri, diagnostics) => {
    const model = monaco.editor.getModel(monaco.Uri.parse(uri));
    if (!model) return;

    const markers = diagnostics.map(d => ({
        severity: d.severity === 1 ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
        startLineNumber: d.range.start.line + 1,
        startColumn: d.range.start.character + 1,
        endLineNumber: d.range.end.line + 1,
        endColumn: d.range.end.character + 1,
        message: d.message,
        source: 'ori',
    }));

    monaco.editor.setModelMarkers(model, 'ori', markers);
});

// Wire up model events
editor.onDidChangeModelContent(() => {
    client.updateDocument(editor.getModel());
});
```

## Size Optimization

### Release Build

```toml
# Cargo.toml
[profile.release]
lto = true
opt-level = 's'  # Optimize for size
codegen-units = 1
```

### wasm-opt

Post-process with `wasm-opt` for further reduction:

```bash
wasm-opt -Os -o ori_lsp_opt.wasm ori_lsp_bg.wasm
```

### Code Splitting

If the full LSP is too large, consider splitting:

```
ori_lsp_core.wasm     # Diagnostics only (smallest)
ori_lsp_format.wasm   # + Formatting
ori_lsp_full.wasm     # + Hover, completion, etc.
```

## Async Considerations

WASM is single-threaded. Avoid blocking operations:

```rust
// BAD: Blocks the main thread
pub fn compute_expensive(&self) -> String {
    // Long computation...
}

// GOOD: Return immediately, poll for results
#[wasm_bindgen]
pub fn start_computation(&mut self, id: u32) {
    self.pending.insert(id, Computation::new());
}

#[wasm_bindgen]
pub fn poll_computation(&mut self, id: u32) -> Option<String> {
    self.pending.get_mut(&id)?.poll()
}
```

For diagnostic debouncing, use JavaScript timers:

```typescript
let debounceTimer: number | null = null;

function onDocumentChange() {
    if (debounceTimer) {
        clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(() => {
        client.updateDocument(editor.getModel());
    }, 100);
}
```

## Testing

### WASM-specific tests

```rust
#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_format() {
        let server = WasmLanguageServer::new();
        server.open_document("file:///test.ori", "let x=1");

        let formatted = server.format("file:///test.ori").unwrap();
        assert_eq!(formatted, "let x = 1\n");
    }
}
```

Run with:
```bash
wasm-pack test --headless --firefox
```
