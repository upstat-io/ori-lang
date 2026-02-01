# Phase 1: Foundation

**Goal**: Establish LSP server infrastructure with document synchronization and caching

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Performance Targets, Caching Strategy

---

## 1.1 Server Binary & Transport

- [ ] **Implement**: `ori lsp` CLI command to start server
  - [ ] **Rust Tests**: `oric/src/commands/lsp.rs` — command parsing
  - [ ] **Integration Tests**: `tests/lsp/startup.rs` — server lifecycle

- [ ] **Implement**: stdio transport (stdin/stdout JSON-RPC)
  - [ ] **Rust Tests**: `ori_lsp/src/transport/stdio.rs` — message framing

- [ ] **Implement**: TCP transport for debugging (optional, `--port` flag)
  - [ ] **Rust Tests**: `ori_lsp/src/transport/tcp.rs` — TCP server

- [ ] **Implement**: Graceful shutdown on `exit` notification
  - [ ] **Rust Tests**: `ori_lsp/src/server.rs` — shutdown sequence

---

## 1.2 Protocol Implementation

- [ ] **Implement**: Initialize/Initialized handshake
  - [ ] Parse client capabilities
  - [ ] Advertise server capabilities
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/lifecycle.rs` — initialization

- [ ] **Implement**: Capability negotiation
  - [ ] textDocumentSync (incremental)
  - [ ] hoverProvider
  - [ ] completionProvider
  - [ ] definitionProvider
  - [ ] referencesProvider
  - [ ] documentSymbolProvider
  - [ ] codeActionProvider
  - [ ] diagnosticProvider
  - [ ] semanticTokensProvider
  - [ ] **Rust Tests**: `ori_lsp/src/capabilities.rs` — capability registration

- [ ] **Implement**: Request/Response dispatching
  - [ ] Route requests to handlers
  - [ ] Error handling with proper LSP error codes
  - [ ] **Rust Tests**: `ori_lsp/src/dispatch.rs` — routing logic

- [ ] **Implement**: Notification handling
  - [ ] `textDocument/didOpen`
  - [ ] `textDocument/didChange`
  - [ ] `textDocument/didClose`
  - [ ] `textDocument/didSave`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/text_document.rs` — notifications

---

## 1.3 Document Synchronization

- [ ] **Implement**: Full document sync mode
  - [ ] Store full document text on open/change
  - [ ] **Rust Tests**: `ori_lsp/src/documents.rs` — full sync

- [ ] **Implement**: Incremental sync mode (preferred)
  - [ ] Apply text edits incrementally
  - [ ] Handle contentChanges array
  - [ ] **Rust Tests**: `ori_lsp/src/documents.rs` — incremental sync

- [ ] **Implement**: Document version tracking
  - [ ] Track version numbers for edit ordering
  - [ ] Reject stale diagnostics
  - [ ] **Rust Tests**: `ori_lsp/src/documents.rs` — version tracking

- [ ] **Implement**: File watcher integration
  - [ ] `workspace/didChangeWatchedFiles` notification
  - [ ] Re-parse files changed outside editor
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/workspace.rs` — file watching

---

## 1.4 Caching Infrastructure

> **Performance**: Parse results cached until file changes; type information cached across files

- [ ] **Implement**: Parse result cache
  - [ ] Key by (file_path, content_hash)
  - [ ] LRU eviction for memory management
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/cache.rs` — parse cache

- [ ] **Implement**: Type information cache
  - [ ] Cross-file type resolution caching
  - [ ] Invalidation on dependency change
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/cache.rs` — type cache

- [ ] **Implement**: Reference graph (incremental updates)
  - [ ] Function → callers mapping
  - [ ] Type → usages mapping
  - [ ] Incremental update on file change
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/index.rs` — reference graph

- [ ] **Implement**: Test result cache
  - [ ] Store test pass/fail status
  - [ ] Invalidate on source change
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/cache.rs` — test results

---

## 1.5 Error Recovery

> **DESIGN**: § Error Recovery — Partial analysis when files have errors

- [ ] **Implement**: Parse error recovery
  - [ ] Continue parsing after syntax errors
  - [ ] Produce partial AST
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/recovery.rs` — parse recovery

- [ ] **Implement**: Type checking with errors
  - [ ] Type check valid parts when some code has errors
  - [ ] Mark error regions as `Unknown` type
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/recovery.rs` — type recovery

- [ ] **Implement**: Graceful degradation table
  - [ ] Valid file: all features
  - [ ] Parse errors: highlighting, error diagnostics
  - [ ] Type errors: navigation, completions, diagnostics
  - [ ] Missing imports: within-file navigation, import suggestions
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/recovery.rs` — degradation levels

---

## 1.6 Logging & Debugging

- [ ] **Implement**: Structured logging
  - [ ] Log levels (error, warn, info, debug, trace)
  - [ ] Request/response tracing
  - [ ] **Rust Tests**: `ori_lsp/src/logging.rs` — log configuration

- [ ] **Implement**: Performance metrics
  - [ ] Request latency tracking
  - [ ] Cache hit/miss rates
  - [ ] **Rust Tests**: `ori_lsp/src/metrics.rs` — metric collection

---

## 1.7 Phase Completion Checklist

- [ ] All items in 1.1-1.6 have all checkboxes marked `[x]`
- [ ] Server starts and handles initialize/shutdown
- [ ] Document sync works (open/change/close)
- [ ] Caching infrastructure operational
- [ ] Error recovery produces partial results
- [ ] Performance: server startup < 500ms
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Server runs, syncs documents, caches results, recovers from errors
