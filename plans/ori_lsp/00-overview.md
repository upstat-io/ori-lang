# Ori LSP Implementation Plan

> **ROADMAP**: Section 22.2 in `plans/roadmap/section-22-tooling.md`
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

## Section Overview

### Section 1: Foundation

Server infrastructure and protocol basics.

| Subsection | Focus |
|------------|-------|
| 1.1 | Server Binary & Transport |
| 1.2 | Protocol Implementation |
| 1.3 | Document Synchronization |
| 1.4 | Caching Infrastructure |

### Section 2: Navigation

Code navigation features.

| Subsection | Focus |
|------------|-------|
| 2.1 | Go-to-Definition |
| 2.2 | Find References |
| 2.3 | Document Symbols |
| 2.4 | Workspace Symbols |

### Section 3: Information Display

Hover and inlay hints.

| Subsection | Focus |
|------------|-------|
| 3.1 | Basic Hover |
| 3.2 | Expanded Hover |
| 3.3 | Type & Capture Hints |
| 3.4 | Inlay Hint Configuration |

### Section 4: Editing Support

Real-time feedback and completions.

| Subsection | Focus |
|------------|-------|
| 4.1 | Diagnostics |
| 4.2 | Quick Fixes |
| 4.3 | Completions |
| 4.4 | Signature Help |

### Section 5: Code Actions

Refactoring and code transformations.

| Subsection | Focus |
|------------|-------|
| 5.1 | Function-Level Actions |
| 5.2 | Expression-Level Actions |
| 5.3 | Error-Level Actions |
| 5.4 | Test-Centric Actions |

### Section 6: Semantic Features

Advanced highlighting and outline.

| Subsection | Focus |
|------------|-------|
| 6.1 | Semantic Highlighting |
| 6.2 | Document Outline |
| 6.3 | Folding Ranges |
| 6.4 | Selection Ranges |

### Section 7: Test Integration

Test-first visibility features.

| Subsection | Focus |
|------------|-------|
| 7.1 | Inline Test Status |
| 7.2 | Code Lens |
| 7.3 | Test Explorer |
| 7.4 | Coverage Display |

### Section 8: Workspace & Integration

Multi-project support and tool integration.

| Subsection | Focus |
|------------|-------|
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
Section 1 (Foundation) → Section 2 (Navigation) → Section 3 (Information)
                      → Section 4 (Editing) → Section 5 (Code Actions)
                      → Section 6 (Semantic) → Section 7 (Test Integration)
                      → Section 8 (Workspace)
```

**Key Dependencies**:
- Section 1 must complete before any other section (server infrastructure)
- Section 2 provides navigation infrastructure used by Sections 3-7
- Section 4 (Diagnostics) enables Section 5 (Code Actions)
- Section 6 and 7 can proceed in parallel after Section 4
- Section 8 requires Sections 1-4 complete

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

## Integration with Roadmap

This LSP implementation plan provides detailed breakdowns for **Section 22.2 (LSP Server)** in the main roadmap. The roadmap's Section 22.2 references this plan:

```markdown
## 22.2 LSP Server

> **DETAILED PLAN**: `plans/ori_lsp/` — Phased implementation with tracking
> **CRATE**: `compiler/ori_lsp/` — LSP server implementation
```

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file - section overview |
| `index.md` | Keyword index for quick finding |
| `priority-and-tracking.md` | Current status and tracking |
| `section-01-foundation.md` | Server infrastructure |
| `section-02-navigation.md` | Navigation features |
| `section-03-information.md` | Hover and hints |
| `section-04-editing.md` | Diagnostics and completions |
| `section-05-code-actions.md` | Refactoring actions |
| `section-06-semantic.md` | Highlighting and outline |
| `section-07-test-integration.md` | Test visibility |
| `section-08-workspace.md` | Workspace features |

### Source References

| Reference | Location |
|-----------|----------|
| LSP Design | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` |
| Semantic Addressing | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/01-semantic-addressing.md` |
| Edit Operations | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/02-edit-operations.md` |
| Structured Errors | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/03-structured-errors.md` |
| Refactoring API | `docs/ori_lang/0.1-alpha/archived-design/12-tooling/07-refactoring-api.md` |
