# Parser v2: Architecture Overview

## Executive Summary

This document outlines the design for Ori's next-generation parser, synthesizing the best patterns from production compilers (Rust, Go, TypeScript, Zig, Gleam, Elm, Roc) to create a parser that is:

- **Fast**: SoA memory layout, capacity pre-estimation, branch hints
- **Modular**: Clear separation of concerns, formal grammar, composable combinators
- **Extensible**: Adding new syntax requires minimal, localized changes
- **Robust**: Multi-error recovery, progress tracking, snapshot speculation
- **User-friendly**: 50+ specific error types, contextual hints, precise positions

## Design Principles

### 1. Separation of Concerns

```
Lexer → Tokens → Parser → Untyped AST → Type Checker → Typed AST
         ↓
   (position tracking)
```

The parser produces an **untyped AST**. Semantic validation (type checking, name resolution) happens in subsequent phases. This follows Go's philosophy: "accept a larger language than spec permits" during parsing.

### 2. Memory-First Design

Following Zig's approach, memory layout drives architecture:

- **Structure of Arrays (SoA)**: Node tags, spans, and data stored separately
- **Capacity pre-estimation**: Allocate based on source length ratios
- **Small value optimization**: Common cases (1-2 children) stored inline
- **Arena allocation**: Single lifetime, linear cleanup

### 3. Progress-Aware Error Recovery

Combining Rust's snapshots with Roc's progress tracking:

- **Progress enum**: Track whether input was consumed
- **Snapshot speculation**: Save/restore state for backtracking
- **Sync sets**: Known recovery points (keywords, delimiters)
- **Error accumulation**: Collect all errors, don't stop at first

### 4. Explicit Context

Following TypeScript and Rust:

- **Context bitflags**: IN_PATTERN, NO_STRUCT_LIT, CONST_EXPR
- **Indentation tracking**: min_indent flows through parse tree
- **Restriction-based control**: Single function with behavior flags

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Parser v2                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   Lexer      │───>│   Cursor     │───>│   Grammar    │       │
│  │  (tokens)    │    │ (lookahead)  │    │  (rules)     │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│         │                   │                   │                │
│         │                   │                   │                │
│         ▼                   ▼                   ▼                │
│  ┌──────────────────────────────────────────────────────┐       │
│  │                    Parser State                       │       │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │       │
│  │  │ tokens  │ │ position│ │ context │ │ indent  │     │       │
│  │  │ stream  │ │ tracking│ │ flags   │ │ stack   │     │       │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │       │
│  └──────────────────────────────────────────────────────┘       │
│         │                                                        │
│         ▼                                                        │
│  ┌──────────────────────────────────────────────────────┐       │
│  │                   Node Storage (SoA)                  │       │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │       │
│  │  │  tags   │ │  spans  │ │  data   │ │  extra  │     │       │
│  │  │ [u8;N]  │ │[Span;N] │ │[Data;N] │ │ [u32;M] │     │       │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │       │
│  └──────────────────────────────────────────────────────┘       │
│         │                                                        │
│         ▼                                                        │
│  ┌──────────────────────────────────────────────────────┐       │
│  │                   Error Collection                    │       │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐     │       │
│  │  │ ParseError  │ │ Expected    │ │ Recovery    │     │       │
│  │  │ variants    │ │ tokens      │ │ sync sets   │     │       │
│  │  └─────────────┘ └─────────────┘ └─────────────┘     │       │
│  └──────────────────────────────────────────────────────┘       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Parser State (~64 bytes target)

```rust
struct ParserState<'a> {
    // Token stream (from lexer)
    tokens: &'a [Token],
    token_index: u32,

    // Position tracking
    current_pos: Position,
    line_start: u32,

    // Context
    context: ParseContext,  // bitflags
    min_indent: u16,

    // Error accumulation
    errors: Vec<ParseError>,
    expected: ExpectedTokens,
}
```

### 2. Node Storage (SoA)

```rust
struct NodeStorage {
    tags: Vec<NodeTag>,       // 1 byte each
    spans: Vec<Span>,         // 8 bytes each
    data: Vec<NodeData>,      // 8 bytes each (union)
    extra: Vec<u32>,          // Variable-length data
}

// Total: 17 bytes per node (vs 24+ in current parser)
```

### 3. Parse Result

```rust
enum Progress { Made, None }

struct ParseResult<T, E> {
    progress: Progress,
    result: Result<T, E>,
}
```

### 4. Error Hierarchy

```rust
enum ParseError {
    Expr(ExprError),
    Pattern(PatternError),
    Type(TypeParseError),
    Item(ItemError),
    Module(ModuleError),
}

enum ExprError {
    Start(Position),
    LetBindingName(Position),
    LetBindingEquals(Position),
    IfCondition(Position),
    IfThen(Position),
    IfElse(Position),
    MatchScrutinee(Position),
    MatchArrow(Position),
    LambdaArrow(Position),
    CallArgument(Position),
    // ... 30+ variants
}
```

## Performance Targets

| Metric | Current | Target | Technique |
|--------|---------|--------|-----------|
| Parser state size | ~200 bytes | ≤64 bytes | Minimize fields, use indices |
| Node size | 24+ bytes | 17 bytes | SoA layout |
| Memory per 1K source | ~2KB | ~1.2KB | Pre-estimation, inline storage |
| Errors collected | 1 (first) | All | Progress tracking |
| Cache misses | High | Low | SoA, contiguous arrays |

## Comparison with Current Parser

| Aspect | Current (v1) | Proposed (v2) |
|--------|-------------|---------------|
| Memory layout | AoS (ExprArena) | SoA (NodeStorage) |
| Error recovery | Sync sets only | Progress + Snapshots + Sync |
| Disambiguation | Ad-hoc lookahead | Tristate + Speculation |
| Context passing | Implicit | Explicit bitflags |
| Indentation | Basic | First-class combinators |
| Grammar | Implicit in code | Formal BNF + code |
| Error types | Generic | 50+ specific variants |

## Document Index

| Document | Purpose |
|----------|---------|
| [01-implementation-plan.md](01-implementation-plan.md) | Phases, timeline, file structure |
| [02-formal-grammar.md](02-formal-grammar.md) | Complete BNF grammar |
| [03-soa-storage.md](03-soa-storage.md) | Memory layout, node storage |
| [04-pratt-parser.md](04-pratt-parser.md) | Expression parsing algorithm |
| [05-error-recovery.md](05-error-recovery.md) | Recovery strategies |
| [06-indentation.md](06-indentation.md) | Whitespace-sensitive parsing |
| [07-disambiguation.md](07-disambiguation.md) | Tristate lookahead patterns |

## Key Insights from Research

### From Rust
- Snapshot-based speculation for complex disambiguation
- Restriction bitflags for context-dependent parsing
- Expected token accumulation for error messages

### From Zig
- SoA layout for cache efficiency
- Capacity pre-estimation (8:1 bytes:tokens, 2:1 tokens:nodes)
- Small value optimization (`_one`, `_two`, `_multi` variants)
- Branch hints (`#[cold]`) on error paths

### From TypeScript
- Tristate lookahead: `False/True/Unknown`
- Context flags for yield/await/strict mode
- Incremental parsing with syntax cursor (future)

### From Elm
- Four-continuation CPS for precise error positions
- Indentation as first-class parser concern
- Extremely specific error types (50+ variants)
- No backtracking past consumed input

### From Go
- Accept larger language, validate later
- Simple one-token lookahead
- Bitset-based sync sets

### From Roc
- Progress tracking in result type
- Per-context error enums
- Indentation combinators

### From Gleam
- Simple precedence parser (proven, not over-engineered)
- Naming conventions: `parse_x`, `expect_x`, `maybe_x`
- Metadata tracking for formatting

## Success Criteria

1. **All existing tests pass** with new parser
2. **Error messages improve** for common mistakes
3. **Memory usage decreases** by ≥30%
4. **Adding new syntax** requires ≤3 file changes
5. **Formal grammar** matches implementation
6. **Parser state** fits in one cache line (≤64 bytes)
