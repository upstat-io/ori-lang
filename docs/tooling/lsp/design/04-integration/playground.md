---
title: "Playground Integration"
description: "Ori LSP Design — Browser-based Playground"
order: 2
section: "Integration"
---

# Playground Integration

Integrating the LSP server with the browser-based Ori Playground.

## Existing Infrastructure

The Playground already has WASM infrastructure at `playground/wasm/`:

```
playground/wasm/
├── Cargo.toml          # WASM crate config
├── src/
│   └── lib.rs          # run_ori() function
└── pkg/                # wasm-pack output
```

The existing `run_ori()` function:
- Lexes, parses, type-checks, and evaluates Ori code
- Returns JSON with output/errors
- Used for the "Run" button

## LSP Addition

Add LSP functionality alongside existing runtime:

```
playground/
├── wasm/               # Existing runtime WASM
│   └── ...
├── wasm-lsp/           # NEW: LSP WASM module
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── web/                # Web frontend
    └── ...
```

Alternative: Single WASM module with both:

```
playground/wasm/
├── Cargo.toml
└── src/
    ├── lib.rs          # Re-exports
    ├── runtime.rs      # run_ori() - existing
    └── lsp.rs          # LSP server - NEW
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Browser                                  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Monaco Editor                         │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────┐  │   │
│  │  │ Squigglies │  │   Hover    │  │   Format-on-Run    │  │   │
│  │  │ (markers)  │  │ (tooltip)  │  │   (auto-format)    │  │   │
│  │  └────────────┘  └────────────┘  └────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│           │                │                │                   │
│           ▼                ▼                ▼                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  OriLanguageClient                       │   │
│  │            (JavaScript bridge layer)                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                            │                                    │
│                            ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              ori_lsp (WASM module)                       │   │
│  │                                                          │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │   │
│  │  │ Diagnostics │  │    Hover    │  │   Formatting    │   │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                            │                                    │
│                            ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                 ori_eval (WASM module)                   │   │
│  │               (existing - runs code)                     │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## JavaScript Bridge

### Initialization

```typescript
// playground.ts
import init as initLsp, { WasmLanguageServer } from './wasm-lsp/pkg/ori_lsp.js';
import init as initRuntime, { run_ori } from './wasm/pkg/ori_wasm.js';

let lspServer: WasmLanguageServer | null = null;

async function initialize() {
    // Load both WASM modules
    await Promise.all([initLsp(), initRuntime()]);

    // Create LSP server
    lspServer = new WasmLanguageServer();

    // Set up Monaco
    setupMonaco();
}
```

### Language Client

```typescript
class OriLanguageClient {
    private server: WasmLanguageServer;
    private currentUri = 'file:///playground.ori';

    constructor(server: WasmLanguageServer) {
        this.server = server;
    }

    // Called when editor content changes
    onContentChange(text: string) {
        this.server.update_document(this.currentUri, text);
        this.updateDiagnostics();
    }

    // Update Monaco markers from diagnostics
    private updateDiagnostics() {
        const diagnosticsJson = this.server.get_diagnostics(this.currentUri);
        const diagnostics: Diagnostic[] = JSON.parse(diagnosticsJson);

        const markers = diagnostics.map(d => ({
            severity: this.mapSeverity(d.severity),
            startLineNumber: d.range.start.line + 1,
            startColumn: d.range.start.character + 1,
            endLineNumber: d.range.end.line + 1,
            endColumn: d.range.end.character + 1,
            message: d.message,
            source: 'ori',
        }));

        monaco.editor.setModelMarkers(
            editor.getModel()!,
            'ori',
            markers
        );
    }

    private mapSeverity(lspSeverity: number): monaco.MarkerSeverity {
        switch (lspSeverity) {
            case 1: return monaco.MarkerSeverity.Error;
            case 2: return monaco.MarkerSeverity.Warning;
            case 3: return monaco.MarkerSeverity.Info;
            case 4: return monaco.MarkerSeverity.Hint;
            default: return monaco.MarkerSeverity.Info;
        }
    }

    // Get formatted code
    format(): string | null {
        return this.server.format(this.currentUri);
    }

    // Get hover info
    hover(line: number, column: number): monaco.languages.Hover | null {
        const hoverJson = this.server.hover(this.currentUri, line, column);
        if (!hoverJson) return null;

        const hover = JSON.parse(hoverJson);
        return {
            contents: [{ value: hover.contents.value }],
            range: hover.range ? {
                startLineNumber: hover.range.start.line + 1,
                startColumn: hover.range.start.character + 1,
                endLineNumber: hover.range.end.line + 1,
                endColumn: hover.range.end.character + 1,
            } : undefined,
        };
    }
}
```

## Monaco Setup

### Language Registration

```typescript
function setupMonaco() {
    // Register Ori language
    monaco.languages.register({ id: 'ori', extensions: ['.ori'] });

    // Basic syntax highlighting (separate from LSP)
    monaco.languages.setMonarchTokensProvider('ori', oriMonarchConfig);

    // LSP-powered features
    setupLspFeatures();
}

function setupLspFeatures() {
    const client = new OriLanguageClient(lspServer!);

    // Diagnostics: update on content change
    editor.onDidChangeModelContent(() => {
        client.onContentChange(editor.getValue());
    });

    // Hover provider
    monaco.languages.registerHoverProvider('ori', {
        provideHover(model, position) {
            return client.hover(
                position.lineNumber - 1,
                position.column - 1
            );
        }
    });

    // Formatting provider
    monaco.languages.registerDocumentFormattingEditProvider('ori', {
        provideDocumentFormattingEdits(model) {
            const formatted = client.format();
            if (!formatted) return [];

            return [{
                range: model.getFullModelRange(),
                text: formatted,
            }];
        }
    });

    // Initialize with current content
    client.onContentChange(editor.getValue());
}
```

## Format-on-Run

Following Go/Gleam playground conventions:

```typescript
async function runCode() {
    const code = editor.getValue();

    // 1. Format first
    const formatted = client.format();
    if (formatted && formatted !== code) {
        editor.setValue(formatted);
        // Briefly pause to show formatting
        await sleep(100);
    }

    // 2. Then run
    const result = run_ori(formatted ?? code);
    displayOutput(result);
}

// Run button handler
document.getElementById('run-btn')!.addEventListener('click', runCode);

// Keyboard shortcut: Ctrl/Cmd + Enter
editor.addCommand(
    monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter,
    runCode
);
```

## Debouncing

For diagnostics, debounce to avoid excessive computation:

```typescript
class OriLanguageClient {
    private debounceTimer: number | null = null;
    private readonly DEBOUNCE_MS = 150;

    onContentChange(text: string) {
        this.server.update_document(this.currentUri, text);

        // Debounce diagnostics
        if (this.debounceTimer) {
            clearTimeout(this.debounceTimer);
        }
        this.debounceTimer = setTimeout(() => {
            this.updateDiagnostics();
        }, this.DEBOUNCE_MS);
    }
}
```

For hover, respond immediately (no debounce needed).

## Error Display

### Inline Markers (Squigglies)

Monaco markers show errors inline:

```typescript
// Red squiggle for errors
{
    severity: monaco.MarkerSeverity.Error,
    startLineNumber: 1,
    startColumn: 5,
    endLineNumber: 1,
    endColumn: 10,
    message: "type mismatch: expected `int`, found `str`",
}
```

### Problems Panel (Optional)

A dedicated panel listing all errors:

```html
<div id="problems-panel">
    <h3>Problems</h3>
    <ul id="problems-list"></ul>
</div>
```

```typescript
function updateProblemsPanel(diagnostics: Diagnostic[]) {
    const list = document.getElementById('problems-list')!;
    list.innerHTML = '';

    for (const d of diagnostics) {
        const li = document.createElement('li');
        li.className = d.severity === 1 ? 'error' : 'warning';
        li.textContent = `Line ${d.range.start.line + 1}: ${d.message}`;
        li.onclick = () => {
            // Jump to error location
            editor.setPosition({
                lineNumber: d.range.start.line + 1,
                column: d.range.start.character + 1,
            });
            editor.focus();
        };
        list.appendChild(li);
    }
}
```

## Loading States

Show loading indicator while WASM loads:

```typescript
async function initialize() {
    showLoading('Loading Ori...');

    try {
        await Promise.all([initLsp(), initRuntime()]);
        lspServer = new WasmLanguageServer();
        setupMonaco();
        hideLoading();
    } catch (e) {
        showError('Failed to load Ori');
        console.error(e);
    }
}

function showLoading(message: string) {
    document.getElementById('loading')!.textContent = message;
    document.getElementById('loading')!.style.display = 'block';
}

function hideLoading() {
    document.getElementById('loading')!.style.display = 'none';
}
```

## Bundle Size

Target WASM sizes:

| Module | Target | Notes |
|--------|--------|-------|
| `ori_lsp.wasm` | < 2 MB | Diagnostics, hover, formatting |
| `ori_eval.wasm` | < 3 MB | Full interpreter |
| Combined | < 4 MB | With deduplication |

Optimization techniques:
- `wasm-opt -Os`
- LTO (link-time optimization)
- Remove debug info in release
- Lazy loading (load LSP first, eval on demand)

## Testing

### Manual Testing

Checklist:
- [ ] Diagnostics appear for syntax errors
- [ ] Diagnostics appear for type errors
- [ ] Diagnostics clear when errors are fixed
- [ ] Hover shows type information
- [ ] Hover works on variables, functions, types
- [ ] Format-on-Run formats the code
- [ ] Formatted code is semantically equivalent

### Automated Testing

```typescript
// Playwright tests
test('diagnostics appear for type error', async ({ page }) => {
    await page.goto('/playground');
    await page.waitForSelector('.monaco-editor');

    // Type code with error
    await page.type('.monaco-editor', 'let x: int = "hello"');

    // Wait for diagnostics
    await page.waitForSelector('.squiggly-error');

    // Verify marker exists
    const markers = await page.evaluate(() => {
        return monaco.editor.getModelMarkers({});
    });
    expect(markers).toHaveLength(1);
    expect(markers[0].message).toContain('type mismatch');
});
```
