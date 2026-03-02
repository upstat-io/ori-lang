---
plan: "design-docs-rewrite"
title: "Design Documentation Rewrite: Compiler Book Treatment"
status: complete
---

# Design Documentation Rewrite

## Mission

Transform `docs/compiler/design/` into a **full-length book on compiler design** that someone can learn compiler design from, using Ori as the case study. A student, a junior engineer, or anyone curious about how compilers work should be able to read these docs front-to-back and come out the other side understanding how to build a real compiler — not a toy one. Each document is a complete chapter that teaches the concept from first principles, builds intuition through worked examples, and grounds everything in a production implementation they can read and study.

This is not reference documentation. It is a textbook that happens to be backed by real code.

The desugaring rewrite (`07-canonicalization/desugaring.md`) is the **minimum bar** for depth and quality. Every document should be at least that thorough.

## Treatment Checklist

Every file receives this treatment where applicable:

### Cleanup
- **Remove file trees** — they go stale on every refactor
- **Remove line counts / statistics** — they change too often to maintain (includes codebase size numbers like "~195,000 lines")
- **Remove file paths from sub-pages** — single-line `Location` blocks
- **Rename "Limitations" to "Design Tradeoffs"**
- **No line number references** — never cite `file.rs:42` style references; line numbers are ephemeral and go stale on every change

### Visual
- **Convert ASCII diagrams to Mermaid** — `flowchart TB`, real line breaks in labels, no subgraphs
- **Mermaid dark theme palette** via `classDef`:
  - **Frontend** — `fill:#1e3a5f,stroke:#60a5fa,color:#dbeafe`
  - **Canon** — `fill:#3b1f6e,stroke:#a78bfa,color:#e9d5ff`
  - **Interpreter** — `fill:#1a4731,stroke:#34d399,color:#d1fae5`
  - **Native** — `fill:#5c3a1e,stroke:#f59e0b,color:#fef3c7`

### Content — Book-Quality Writing
- **Book-length conceptual foundations** — teach the concept from first principles as a textbook chapter would: what is this, why does it exist, what problem does it solve, what are the classical approaches, what are the tradeoffs between them. A reader who has never built a compiler should understand the problem space after reading this section alone.
- **Add "What Makes X Distinctive" section** — lead with unique/special design choices that differentiate Ori from other compilers
- **Explain tradeoffs and alternatives not taken** — not just what Ori does, but what it chose not to do and why. Show the design space.
- **Prior art with substance** — not a table of names, but enough detail about what each reference compiler does differently that the reader learns from the comparison itself
- **Link all source references** — every external reference (paper, framework, compiler, tool) must be hyperlinked to its source (project repo, official docs, or paper publication page). No unlinked citations.
- **Connect theory to implementation** — bridge from the general concept to Ori's specific choices, showing how the theory informed the design
- **Expansive, not terse** — write full chapters, not reference cards. Paragraphs of prose, worked examples, diagrams. The goal is a book someone could learn compiler design from, not a cheat sheet for existing experts.

## Scope

79 files across 16 directories + root index.

## Current Progress

| Section | Status | Notes |
|---------|--------|-------|
| Root `index.md` | **Done** | Full rewrite with book-style intro, dark theme Mermaid |
| 01-architecture | **Done** | Full book treatment: all 4 files rewritten |
| 02-intermediate-representation | **Done** | Full book treatment: all 5 files rewritten |
| 03-lexer | **Done** | Full book treatment: both files rewritten |
| 04-parser | **Done** | Full book treatment: all 5 files rewritten |
| 05-type-system | **Done** | Full book treatment: all 6 files rewritten |
| 06-pattern-system | **Done** | Full book treatment: all 5 files rewritten |
| 07-canonicalization | **Done** | Full book treatment: all 4 files rewritten |
| 08-evaluator | **Done** | Full book treatment: all 5 files rewritten |
| 09-arc-system | **Done** | Full book treatment: all 10 files rewritten |
| 10-llvm-backend | **Done** | Full book treatment: all 7 files rewritten |
| 11-runtime | **Done** | Full book treatment: all 5 files rewritten |
| 12-formatter | **Done** | Full book treatment: all 4 files rewritten |
| 13-diagnostics | **Done** | Full book treatment: all 4 files rewritten |
| 14-testing | **Done** | Full book treatment: all 3 files rewritten |
| 15-platform-targets | **Done** | Full book treatment: all 4 files rewritten |
| appendices | **Done** | Full book treatment: all 5 files rewritten |
