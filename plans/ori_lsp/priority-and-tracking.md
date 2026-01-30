# Priority Order & Tracking

## Current Status

### Phase 1: Foundation

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 1.1 | Server Binary & Transport | ⏳ Not started | CLI entry point, stdio/TCP |
| 1.2 | Protocol Implementation | ⏳ Not started | Initialize, capabilities, dispatch |
| 1.3 | Document Synchronization | ⏳ Not started | Full/incremental sync |
| 1.4 | Caching Infrastructure | ⏳ Not started | Parse, type, reference caches |
| 1.5 | Error Recovery | ⏳ Not started | Partial analysis, degradation |
| 1.6 | Logging & Debugging | ⏳ Not started | Structured logs, metrics |

### Phase 2: Navigation

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 2.1 | Go-to-Definition | ⏳ Not started | Single/multi-target navigation |
| 2.2 | Find References | ⏳ Not started | Categorized results |
| 2.3 | Document Symbols | ⏳ Not started | Hierarchical outline |
| 2.4 | Workspace Symbols | ⏳ Not started | Cross-file search |
| 2.5 | Implementation Navigation | ⏳ Not started | Trait implementations |

### Phase 3: Information Display

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 3.1 | Basic Hover | ⏳ Not started | Type, signature, location |
| 3.2 | Expanded Hover | ⏳ Not started | Body, tests, captures |
| 3.3 | Type & Capture Hints | ⏳ Not started | Inlay hints |
| 3.4 | Inlay Hint Configuration | ⏳ Not started | Toggle settings |

### Phase 4: Editing Support

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 4.1 | Diagnostics | ⏳ Not started | < 50ms, precise spans |
| 4.2 | Quick Fixes | ⏳ Not started | Type conversion, imports |
| 4.3 | Completions | ⏳ Not started | 10-15 quality items |
| 4.4 | Signature Help | ⏳ Not started | Parameter highlighting |

### Phase 5: Code Actions

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 5.1 | Function-Level Actions | ⏳ Not started | Test, rename, extract |
| 5.2 | Expression-Level Actions | ⏳ Not started | Extract, inline, transform |
| 5.3 | Error-Level Actions | ⏳ Not started | Quick fix integration |
| 5.4 | Test-Centric Actions | ⏳ Not started | Coverage, test creation |
| 5.5 | Code Action Handler | ⏳ Not started | Request handling |
| 5.6 | Edit Operations Integration | ⏳ Not started | Multi-file edits |

### Phase 6: Semantic Features

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 6.1 | Semantic Highlighting | ⏳ Not started | Ori-specific tokens |
| 6.2 | Document Outline | ⏳ Not started | Coverage indicators |
| 6.3 | Folding Ranges | ⏳ Not started | Functions, blocks, imports |
| 6.4 | Selection Ranges | ⏳ Not started | Hierarchical selection |

### Phase 7: Test Integration

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 7.1 | Inline Test Status | ⏳ Not started | Pass/fail next to functions |
| 7.2 | Code Lens | ⏳ Not started | Run/Debug/Coverage |
| 7.3 | Test Explorer | ⏳ Not started | Test tree view |
| 7.4 | Coverage Display | ⏳ Not started | Line decorations |
| 7.5 | Test Commands | ⏳ Not started | Command palette |

### Phase 8: Workspace & Integration

| Section | Name | Status | Notes |
|---------|------|--------|-------|
| 8.1 | Multi-Root Workspace | ⏳ Not started | Multiple folders |
| 8.2 | Project Detection | ⏳ Not started | ori.toml, structure |
| 8.3 | Formatter Integration | ⏳ Not started | Format on save |
| 8.4 | Edit Operations Integration | ⏳ Not started | Refactoring API |
| 8.5 | Test Runner Integration | ⏳ Not started | `ori test` execution |
| 8.6 | Configuration | ⏳ Not started | Settings management |

---

## Immediate Priority

**Current Focus**: Phase 1 (Foundation)

### What's Next (Priority Order)

1. **Phase 1.1 (Server Binary)** — Create `ori lsp` command
   - Entry point for language server
   - stdio transport for editor communication

2. **Phase 1.2 (Protocol)** — Implement LSP handshake
   - Initialize/Initialized sequence
   - Capability negotiation

3. **Phase 1.3 (Document Sync)** — Track open documents
   - Full and incremental sync
   - Version tracking

### Prerequisites

Before starting LSP implementation:

- [ ] Phase 22.1 (Formatter) should be substantially complete
- [ ] Phase 22.7 (Structured Diagnostics) provides error infrastructure
- [ ] Core compiler (Phases 1-5) must be stable

---

## Milestones

### M1: Basic Server (Phases 1-2)

- [ ] Server starts and handles lifecycle
- [ ] Document sync working
- [ ] Go-to-definition works
- [ ] Find references works
- [ ] Document symbols works

**Exit criteria**: Can navigate code in editor

### M2: Information Display (Phase 3)

- [ ] Hover shows types and signatures
- [ ] Expanded hover shows body and tests
- [ ] Inlay hints show inferred types

**Exit criteria**: Can understand code without navigating

### M3: Editing Support (Phase 4)

- [ ] Diagnostics publish within 50ms
- [ ] Quick fixes available
- [ ] Completions ranked and limited
- [ ] Signature help shows parameters

**Exit criteria**: Productive editing experience

### M4: Code Actions (Phase 5)

- [ ] Refactoring actions work
- [ ] Test actions (run, generate, go to)
- [ ] Error fixes via code actions

**Exit criteria**: Full refactoring support

### M5: Advanced Features (Phase 6)

- [ ] Semantic highlighting for Ori constructs
- [ ] Document outline with coverage
- [ ] Folding and selection ranges

**Exit criteria**: Rich visual feedback

### M6: Test Integration (Phase 7)

- [ ] Inline test status
- [ ] Code lens for run/debug
- [ ] Test explorer integration
- [ ] Coverage visualization

**Exit criteria**: Test-first development workflow

### M7: Full Featured (Phase 8)

- [ ] Multi-root workspace
- [ ] Formatter integration
- [ ] All settings configurable

**Exit criteria**: Production-ready LSP

---

## Performance Targets

| Operation | Target | Current | Status |
|-----------|--------|---------|--------|
| Diagnostics | < 50ms | — | ⏳ |
| Hover | < 20ms | — | ⏳ |
| Completions | < 100ms | — | ⏳ |
| Go-to-definition | < 50ms | — | ⏳ |
| Find references | < 200ms | — | ⏳ |
| Document symbols | < 100ms | — | ⏳ |
| Formatting | < 100ms | — | ⏳ |

---

## Dependencies

### External Dependencies

| Dependency | Purpose | Crate |
|------------|---------|-------|
| tower-lsp | LSP protocol implementation | `tower-lsp` |
| lsp-types | LSP type definitions | `lsp-types` |
| tokio | Async runtime | `tokio` |

### Internal Dependencies

| Dependency | Purpose | Location |
|------------|---------|----------|
| ori_parse | Parsing for analysis | `compiler/ori_parse/` |
| ori_typeck | Type information | `compiler/oric/src/typeck/` |
| ori_diagnostic | Error formatting | `compiler/ori_diagnostic/` |
| ori_fmt | Formatting | `compiler/ori_fmt/` |

---

## Related Plans

| Plan | Location | Relationship |
|------|----------|--------------|
| Main Roadmap | `plans/roadmap/` | Phase 22.2 references this plan |
| Formatter | `plans/ori_fmt/` | Integration in Phase 8.3 |
| Testing Framework | Phase 14 | Integration in Phase 7 |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| — | Use tower-lsp | Standard Rust LSP framework |
| — | Test-first visibility | Core design principle from spec |
| — | 10-15 completion items | Quality over quantity |
| — | < 50ms diagnostic target | "Instant response" principle |
