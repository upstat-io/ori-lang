# Proposal: MCP Semantic Tooling Server

**Status:** Draft (Loose Concept)
**Author:** Eric (with Claude)
**Created:** 2026-03-05

> **THIS IS A LOOSE CONCEPT, NOT A CONCRETE DESIGN.** The ideas below are
> early-stage thinking about a long-term direction. Nothing here is ready for
> implementation. The purpose of this document is to capture the vision so
> future design decisions don't accidentally close doors.

---

## Summary

An MCP (Model Context Protocol) server that exposes the Ori compiler's semantic knowledge to AI agents, providing instant answers to queries that currently require file scanning, and verified code transformations that currently require manual multi-file editing.

The server sits in front of the LSP / Salsa database and translates AI-oriented queries into compiler lookups against a warm cache.

---

## Motivation

### The Current AI-Codebase Interaction Is Wasteful

When AI tools work on an Ori codebase, they grep for function names, read files, mentally parse code, grep for callers, and reconstruct dependency graphs — all information the compiler has already computed. Every `Grep` call is an O(n) text search to answer a question the type checker answered in milliseconds.

### Ori's Design Makes This Solvable

Ori's combination of design choices creates a compiler where the hardest queries are fast:

- **Expression-based**: every AST node has a type — `type_at(line, col)` is a direct lookup
- **HM inference**: all types fully resolved after type check — no partial types to re-resolve
- **No overloading + named params**: symbol resolution is unambiguous — go-to-definition is a hash lookup
- **Capability effects**: effect information in signatures — no interprocedural effect analysis needed
- **ARC precomputation**: ownership, uniqueness, COW mode all cached — performance queries are lookups
- **No macros**: source text = compiler input — no expansion layer between AI and semantics

---

## Architecture (Conceptual)

```
AI Agent (Claude Code, etc.)
    | MCP protocol (JSON-RPC)
Ori MCP Server (query translation + write operations)
    | LSP protocol or direct Salsa queries
ori-lsp (long-running, warm Salsa DB)
    | Salsa queries
Compiler internals (type checker, ARC pipeline, etc.)
```

The LSP server would need to exist anyway for human IDE support. The MCP server is a thin adapter layer.

---

## Query Categories (Brainstorm)

### Read Queries — Semantic Lookups

Things AI currently gets via grep/read that the compiler already knows:

- `type_at(file, line, col)` — type of expression at cursor
- `signature(function)` — full function signature with types
- `callers(function)` — all call sites across the project
- `methods(type)` — all methods available on a type
- `impls(trait)` — all types implementing a trait
- `deps(module)` — module dependency graph
- `diagnostics(file)` — all errors/warnings
- `arc_class(type)` — ARC classification (Scalar/DefiniteRef/PossibleRef)

### Deep Queries — Compiler Analysis

Information no amount of text searching can reveal:

- `type_chain(file, line, col)` — declared type, inferred type, narrowed range, ARC class, uniqueness
- `impact_analysis(function)` — callers, trait impls, tests, everything affected by a change
- `arc_ir(function)` — ARC IR with RC op count, COW modes, FBIP status
- `rename_check(from, to, scope)` — preview conflicts and affected sites before renaming

### Write Operations — Verified Transformations

Compiler-verified multi-file edits:

- `rename(symbol, to, update_callers)` — semantic rename across all files
- `change_signature(function, add_param, ...)` — update declaration + all call sites
- `extract_function(file, range, name)` — extract code into function with correct params/types
- `move_symbol(symbol, from, to, update_imports)` — move between modules
- `implement_trait(type, trait)` — generate correct method stubs
- `add_field(type, field, update_constructors, update_derives)` — add field + update all construction sites

Key property: write operations can **verify before returning**. The MCP server applies edits to Salsa, re-typechecks, and returns either a verified diff or precise errors.

---

## Why Ori Is Uniquely Suited

### Named Parameters Make Refactoring Safe

`change_signature` adding a parameter with a default value is safe in Ori because every call site uses named arguments. No positional ambiguity. No silent breakage from argument reordering.

### No Macros = Complete Visibility

`rename` and `move_symbol` see every reference. There are no macro-generated references hiding in unexpanded code.

### Value Semantics = No Borrow Checker Rejection

Generated code won't be rejected by a borrow checker. If the data flow is correct, it compiles. Automated refactoring tools in Rust are limited because generated code might violate borrow rules in unpredictable ways.

### Salsa Warm Cache = Instant Verification

After generating edits, verification is an incremental Salsa recomputation — not a full recompile. Sub-second verification of multi-file refactorings.

---

## Relationship to Self-Hosting

The MCP server itself is a candidate for partial self-hosting:

- MCP protocol handling (JSON parsing, routing) — could be Ori
- Query translation — could be Ori
- Salsa database — stays in Rust (proven, complex)
- Compiler phases — gradually Ori (starting with lexer)

---

## Open Questions

- What MCP protocol version / transport to target?
- Should the MCP server be a separate process or embedded in the LSP?
- How to handle stale cache when files change outside the editor?
- What's the minimum viable set of queries for a useful v1?
- How does this interact with multi-file compilation and module boundaries?
- Performance budget: what's the acceptable latency for write operations?

---

## Prior Art

- **rust-analyzer**: Fast LSP for Rust, Salsa-based. Read-only queries are excellent. Write operations (assists) are limited by borrow checker unpredictability.
- **TypeScript Language Server**: Good refactoring support, but fights conditional types and `any` propagation.
- **gopls**: Simple and fast, but limited semantic depth (no effect system, limited generics).
- **Facebook Codemod**: AST-pattern code transformations for JavaScript. No type information.
- **Sourcegraph**: Code intelligence via indexing. Read-only. No verified transformations.

None of these combine semantic queries + verified write operations + warm compiler cache in a single tool.

---

## Non-Goals (For Now)

- This is NOT a concrete implementation plan
- No timeline, no roadmap section, no dependency ordering
- No protocol specification or API design
- No commitment to any particular architecture
- The purpose is solely to capture the concept and ensure future design decisions remain compatible
