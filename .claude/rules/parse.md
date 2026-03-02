---
paths:
  - "**parse**"
---

# Parser

## Pre-Implementation
- Spec first: add grammar to `docs/ori_lang/v2026/spec/`
- Update `grammar.ebnf` | check disambiguation and context flags
- Create failing tests in `tests/spec/`

## Implementation
- Lexer: new tokens in `ori_lexer/` | AST: add nodes to `ori_ir/`
- Parser: add parsing in `ori_parse/` | type checker + evaluator updates

## Context Flags
- `NO_STRUCT_LIT` -- prevent in `if` conditions
- `IN_PATTERN` -- parsing pattern
- `IN_INDEX` -- inside `[...]` (enables `#`)
- `IN_LOOP` -- enables `break`/`continue`

## Lexer-Parser Boundary
- `>` always single token (never `>>`) | parser combines adjacent `>` in expression context
- Enables: `Result<Result<T, E>, E>`

## Progress Tracking
- `Progress::None` + error -> try alternative
- `Progress::Made` + error -> commit and sync

## Tracing
- Target: `ori_parse` (instrumentation in progress) | `ORI_LOG=oric=debug` (Salsa parse query) | `ORI_LOG=ori_parse=trace` (parser-level)
- Phase dump: `ORI_DUMP_AFTER_PARSE=1` | see compiler.md for full reference

## Key Files
- `ori_lexer/src/lib.rs`: Tokens
- `context/`: ParseContext flags
- `grammar/`: Parsing (expr, decl, pattern, type)
- `error/`: Parse error reporting
- `recovery/`: Error recovery strategies
- `grammar.ebnf`: Unified grammar (in spec)
