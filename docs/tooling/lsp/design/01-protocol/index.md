---
title: "Protocol Overview"
description: "Ori LSP Design — LSP Protocol Methods"
order: 1
section: "Protocol"
---

# Protocol Overview

The Ori language server implements a subset of the Language Server Protocol (LSP) 3.17.

## Lifecycle

### Initialization

```
Client                              Server
   │                                   │
   │──── initialize ──────────────────►│
   │                                   │
   │◄─── initialize result ────────────│
   │     (capabilities)                │
   │                                   │
   │──── initialized ─────────────────►│
   │                                   │
```

**Server Capabilities** (Phase 1):

```json
{
  "capabilities": {
    "textDocumentSync": {
      "openClose": true,
      "change": 2,
      "save": { "includeText": false }
    },
    "documentFormattingProvider": true,
    "hoverProvider": true
  }
}
```

### Shutdown

```
Client                              Server
   │                                   │
   │──── shutdown ────────────────────►│
   │◄─── null ─────────────────────────│
   │                                   │
   │──── exit ────────────────────────►│
   │                                   │
```

## Text Document Synchronization

The server uses **incremental sync** (`TextDocumentSyncKind.Incremental = 2`) for efficiency.

### Document Open

```typescript
interface DidOpenTextDocumentParams {
  textDocument: {
    uri: string;
    languageId: "ori";
    version: number;
    text: string;
  }
}
```

On open:
1. Store document in memory
2. Run diagnostics
3. Publish diagnostics to client

### Document Change

```typescript
interface DidChangeTextDocumentParams {
  textDocument: { uri: string; version: number };
  contentChanges: TextDocumentContentChangeEvent[];
}
```

On change:
1. Apply incremental changes to stored document
2. Debounce diagnostic updates (50-100ms)
3. Run diagnostics
4. Publish diagnostics to client

### Document Close

Remove document from memory. Stop publishing diagnostics.

## Request Methods

### `textDocument/formatting`

**Request:**
```typescript
interface DocumentFormattingParams {
  textDocument: { uri: string };
  options: {
    tabSize: number;      // Ignored (Ori uses 4 spaces)
    insertSpaces: boolean; // Ignored (Ori always uses spaces)
  }
}
```

**Response:**
```typescript
type TextEdit[] = {
  range: Range;
  newText: string;
}[];
```

Implementation:
1. Get document text
2. Call `ori_fmt::format()`
3. Return single edit replacing entire document (simpler, fast enough)

### `textDocument/hover`

**Request:**
```typescript
interface HoverParams {
  textDocument: { uri: string };
  position: { line: number; character: number };
}
```

**Response:**
```typescript
interface Hover {
  contents: MarkupContent;
  range?: Range;
}
```

Implementation:
1. Find AST node at position
2. Look up type from `ori_typeck`
3. Format as markdown:
   ```markdown
   ```ori
   x: int
   ```
   ```

### `textDocument/publishDiagnostics`

**Notification** (server → client):
```typescript
interface PublishDiagnosticsParams {
  uri: string;
  version?: number;
  diagnostics: Diagnostic[];
}

interface Diagnostic {
  range: Range;
  severity: DiagnosticSeverity;
  code?: string;
  source: "ori";
  message: string;
  relatedInformation?: DiagnosticRelatedInformation[];
}
```

Severity mapping:
| Ori | LSP |
|-----|-----|
| Error | `DiagnosticSeverity.Error` (1) |
| Warning | `DiagnosticSeverity.Warning` (2) |
| Hint | `DiagnosticSeverity.Hint` (4) |

## Future Methods (Phase 2+)

### `textDocument/definition`

```typescript
interface DefinitionParams {
  textDocument: { uri: string };
  position: Position;
}
// Response: Location | Location[] | LocationLink[]
```

### `textDocument/references`

```typescript
interface ReferenceParams {
  textDocument: { uri: string };
  position: Position;
  context: { includeDeclaration: boolean };
}
// Response: Location[]
```

### `textDocument/completion`

```typescript
interface CompletionParams {
  textDocument: { uri: string };
  position: Position;
  context?: CompletionContext;
}
// Response: CompletionItem[] | CompletionList
```

## Error Handling

LSP errors use standard codes:

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32802 | Request cancelled |

For Ori-specific failures:
- Parse errors → publish as diagnostics (don't fail the request)
- Type errors → publish as diagnostics
- Formatter errors → return empty edit array, publish diagnostic
