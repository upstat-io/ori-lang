---
plan: "design-docs-rewrite"
title: "Design Documentation Rewrite: Compiler Book Treatment"
status: in-progress
---

# Design Documentation Rewrite

## Mission

Rewrite every file in `docs/compiler/design/` so the documentation reads like a compiler design textbook — each section opens with conceptual foundations, explains the general compiler concept, then shows how Ori applies and adapts those ideas. Remove stale artifacts, apply consistent visual treatment, and ensure every page serves both as reference documentation and educational material.

## Treatment Checklist

Every file receives this treatment where applicable:

- **Remove file trees** — they go stale on every refactor
- **Remove line counts / statistics** — they change too often to maintain
- **Convert ASCII diagrams to Mermaid** — `flowchart TB`, real line breaks in labels, no subgraphs
- **Mermaid dark theme palette** via `classDef`:
  - **Frontend** — `fill:#1e3a5f,stroke:#60a5fa,color:#dbeafe`
  - **Canon** — `fill:#3b1f6e,stroke:#a78bfa,color:#e9d5ff`
  - **Interpreter** — `fill:#1a4731,stroke:#34d399,color:#d1fae5`
  - **Native** — `fill:#5c3a1e,stroke:#f59e0b,color:#fef3c7`
- **Add "What Makes X Distinctive" section** — lead with unique/special design choices
- **Rename "Limitations" to "Design Tradeoffs"**
- **Remove file paths from sub-pages** — single-line `Location` blocks
- **Open with conceptual foundations** — what is this concept, what problem does it solve, what are the classical approaches
- **Explain tradeoffs and alternatives not taken** — not just what Ori does, but why
- **Prior art with substance** — what each reference compiler does differently
- **Connect theory to implementation** — bridge from general concept to Ori's specific choices

## Scope

76 files across 16 directories + root index.

## Current Progress

| Section | Status | Notes |
|---------|--------|-------|
| Root `index.md` | **Done** | Full rewrite with book-style intro, dark theme Mermaid |
| 01-architecture | Needs upgrade | Old treatment (file tree removal only) |
| 02-intermediate-representation | Needs upgrade | Old treatment |
| 03-lexer | Needs upgrade | Old treatment |
| 04-parser | Needs upgrade | Old treatment |
| 05-type-system | Needs upgrade | Old treatment |
| 06-pattern-system | Needs upgrade | Recent treatment, missing conceptual foundations |
| 07-canonicalization | Partial | Index done, `desugaring.md` has full book treatment, others need upgrade |
| 08-evaluator | Not started | |
| 09-arc-system | Not started | |
| 10-llvm-backend | Not started | |
| 11-runtime | Not started | |
| 12-formatter | Not started | |
| 13-diagnostics | Not started | |
| 14-testing | Not started | |
| 15-platform-targets | Not started | |
| appendices | Not started | |
