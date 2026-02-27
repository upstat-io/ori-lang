---
paths:
  - "**/parse/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

**Expression-based — NO `return`**: Last expression IS the value. `return` token exists only to emit helpful error.

# Parser

## Pre-Implementation
- Spec first: add grammar to `docs/ori_lang/0.1-alpha/spec/`
- Update `grammar.ebnf`
- Check disambiguation and context flags
- Create failing tests in `tests/spec/`

## Implementation
- Lexer: new tokens in `ori_lexer/`
- AST: add nodes to `ori_ir/`
- Parser: add parsing in `ori_parse/`
- Type checker + Evaluator updates

## Context Flags
- `NO_STRUCT_LIT` — prevent in `if` conditions
- `IN_PATTERN` — parsing pattern
- `IN_INDEX` — inside `[...]` (enables `#`)
- `IN_LOOP` — enables `break`/`continue`

## Lexer-Parser Boundary
- `>` always single token (never `>>`)
- Parser combines adjacent `>` in expression context
- Enables: `Result<Result<T, E>, E>`

## Progress Tracking
- `Progress::None` + error → try alternative
- `Progress::Made` + error → commit and sync

## Debugging / Tracing

**Always use `ORI_LOG` first when debugging parse issues.** Tracing target: `ori_parse` (dependency declared, instrumentation in progress).

```bash
ORI_LOG=debug ori check file.ori                    # See Salsa parse query execution
ORI_LOG=oric=debug ori check file.ori               # See when parsed() query runs
ORI_LOG=ori_parse=trace ori check file.ori          # Parser-level tracing (as instrumented)
```

### Phase Dump
```bash
ORI_DUMP_AFTER_PARSE=1 ori check file.ori           # Dump AST after parsing
```

**Tips**:
- Parse error on valid syntax? Check `grammar.ebnf` first, then context flags
- Salsa returning stale parse? Use `ORI_LOG=oric=debug` to check `WillExecute` for `parsed()` query
- Parser currently has tracing dependency prepared but limited instrumentation — add `#[tracing::instrument]` to functions you're debugging

## Key Files
- `ori_lexer/src/lib.rs`: Tokens
- `context/`: ParseContext flags
- `grammar/`: Parsing (expr, decl, pattern, type)
- `error/`: Parse error reporting
- `recovery/`: Error recovery strategies
- `grammar.ebnf`: Unified grammar (in spec)
