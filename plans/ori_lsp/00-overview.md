# Ori LSP Implementation Plan

> **Language Server Protocol** — Developer tooling for code intelligence

## Design Philosophy

From the LSP design document (`docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md`):

1. **Show what matters, hide what does not** — No information overload
2. **Instant response** — No waiting, no spinners
3. **Accurate or nothing** — Wrong info is worse than no info
4. **Semantic, not syntactic** — Show meaning, not just structure
5. **Test-first visibility** — Every view shows test status

The Ori LSP is designed for **reviewing AI-generated code**, prioritizing verification, test visibility, and quick understanding over authoring assistance.

---

## Phase Overview

### Phase 1: Foundation

Server infrastructure and protocol basics.

| Section | Focus |
|---------|-------|
| 1.1 | Server Binary & Transport |
| 1.2 | Protocol Implementation |
| 1.3 | Document Synchronization |
| 1.4 | Caching Infrastructure |

### Phase 2: Navigation

Code navigation features.

| Section | Focus |
|---------|-------|
| 2.1 | Go-to-Definition |
| 2.2 | Find References |
| 2.3 | Document Symbols |
| 2.4 | Workspace Symbols |

### Phase 3: Information Display

Hover and inlay hints.

| Section | Focus |
|---------|-------|
| 3.1 | Basic Hover |
| 3.2 | Expanded Hover |
| 3.3 | Type & Capture Hints |
| 3.4 | Inlay Hint Configuration |

### Phase 4: Editing Support

Real-time feedback and completions.

| Section | Focus |
|---------|-------|
| 4.1 | Diagnostics |
| 4.2 | Quick Fixes |
| 4.3 | Completions |
| 4.4 | Signature Help |

### Phase 5: Code Actions

Refactoring and code transformations.

| Section | Focus |
|---------|-------|
| 5.1 | Function-Level Actions |
| 5.2 | Expression-Level Actions |
| 5.3 | Error-Level Actions |
| 5.4 | Test-Centric Actions |

### Phase 6: Semantic Features

Advanced highlighting and outline.

| Section | Focus |
|---------|-------|
| 6.1 | Semantic Highlighting |
| 6.2 | Document Outline |
| 6.3 | Folding Ranges |
| 6.4 | Selection Ranges |

### Phase 7: Test Integration

Test-first visibility features.

| Section | Focus |
|---------|-------|
| 7.1 | Inline Test Status |
| 7.2 | Code Lens |
| 7.3 | Test Explorer |
| 7.4 | Coverage Display |

### Phase 8: Workspace & Integration

Multi-project support and tool integration.

| Section | Focus |
|---------|-------|
| 8.1 | Multi-Root Workspace |
| 8.2 | Project Detection |
| 8.3 | Formatter Integration |
| 8.4 | Edit Operations Integration |

---

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Diagnostics | < 50ms | After keystroke |
| Hover | < 20ms | Immediate feel |
| Completions | < 100ms | Before user notices |
| Go-to-definition | < 50ms | Instant navigation |
| Find references | < 200ms | Can show progress for large codebases |
| Document symbols | < 100ms | For outline view |
| Formatting | < 100ms | Per file |

---

## Dependency Graph

```
Phase 1 (Foundation) → Phase 2 (Navigation) → Phase 3 (Information)
                    → Phase 4 (Editing) → Phase 5 (Code Actions)
                    → Phase 6 (Semantic) → Phase 7 (Test Integration)
                    → Phase 8 (Workspace)
```

**Key Dependencies**:
- Phase 1 must complete before any other phase (server infrastructure)
- Phase 2 provides navigation infrastructure used by Phases 3-7
- Phase 4 (Diagnostics) enables Phase 5 (Code Actions)
- Phase 6 and 7 can proceed in parallel after Phase 4
- Phase 8 requires Phases 1-4 complete

---

## Crate Structure

```
compiler/
├── ori_lsp/                    # LSP server crate
│   ├── src/
│   │   ├── lib.rs             # Crate root
│   │   ├── server.rs          # Server implementation
│   │   ├── capabilities.rs    # LSP capability negotiation
│   │   ├── handlers/          # Request handlers
│   │   │   ├── mod.rs
│   │   │   ├── hover.rs
│   │   │   ├── completion.rs
│   │   │   ├── definition.rs
│   │   │   ├── references.rs
│   │   │   ├── diagnostics.rs
│   │   │   ├── code_action.rs
│   │   │   ├── semantic_tokens.rs
│   │   │   └── document_symbol.rs
│   │   ├── analysis/          # Analysis infrastructure
│   │   │   ├── mod.rs
│   │   │   ├── cache.rs
│   │   │   └── index.rs
│   │   └── test_integration/  # Test-specific features
│   │       ├── mod.rs
│   │       ├── status.rs
│   │       └── explorer.rs
│   ├── Cargo.toml
│   └── tests/
└── oric/
    └── src/
        └── commands/
            └── lsp.rs         # CLI entry point
```

---

## Integration with Phase 22

This LSP implementation plan provides detailed breakdowns for **Phase 22.2 (LSP Server)** in the main roadmap. The main roadmap's Phase 22.2 section should reference this plan:

```markdown
## 22.2 LSP Server

> **DETAILED PLAN**: `plans/ori_lsp/` — Phased implementation with tracking
> **CRATE**: `compiler/ori_lsp/` — LSP server implementation
```

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file - phase overview |
| `priority-and-tracking.md` | Current status and tracking |
| `phase-01-foundation.md` | Server infrastructure |
| `phase-02-navigation.md` | Navigation features |
| `phase-03-information.md` | Hover and hints |
| `phase-04-editing.md` | Diagnostics and completions |
| `phase-05-code-actions.md` | Refactoring actions |
| `phase-06-semantic.md` | Highlighting and outline |
| `phase-07-test-integration.md` | Test visibility |
| `phase-08-workspace.md` | Workspace features |

### Source References

| Reference | Location |
|-----------|----------|
| LSP Design | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` |
| Semantic Addressing | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/01-semantic-addressing.md` |
| Edit Operations | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/02-edit-operations.md` |
| Structured Errors | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/03-structured-errors.md` |
| Refactoring API | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/07-refactoring-api.md` |
