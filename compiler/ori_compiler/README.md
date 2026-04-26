# ori_compiler

> **`ori_compiler` exists to be a pure, IO-free orchestration facade** — one entry point a WASM-hosted or embedded consumer can drive without `oric`'s Salsa runtime or filesystem access.

## Role in the pipeline

`ori_compiler` is the pure, embedding-friendly compilation facade. It orchestrates the core pipeline (lex → parse → typecheck → canonicalize → eval) without performing any IO itself — no file reading, no Salsa initialization, no LLVM dependency. The same compilation logic runs natively via `oric` (which adds Salsa + IO + LLVM) and in the browser via `ori_compiler`, with identical observable behavior.

Primary use case: the Ori language website (`ori-lang-website`) and any future embeddings (VS Code extension in-process compilation, REPL hosts, WASM playgrounds).

## Architecture

- Single compilation entry point that takes source + module metadata and produces typed/canonical IR + diagnostics.
- No Salsa — incremental caching is the native driver's concern.
- No LLVM — codegen lives in `oric`'s `llvm` feature.
- No filesystem — callers pass source as strings.

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_diagnostic`, `ori_lexer`, `ori_parse`, `ori_types`, `ori_canon`, `ori_eval`, `ori_patterns`, `ori_fmt` |
| Downstream | WASM hosts, embedding consumers, `ori-lang-website` |

## Invariants

- **Purity is the contract**: any IO, Salsa, or LLVM dependency creeping in fractures the embedding story.
- **Behavioral parity with `oric`**: the facade must produce identical compilation output for identical input. Divergence is a WASM-parity bug.
- **No convenience APIs that require IO**: if a proposed helper would need to read a file, the helper goes in `oric` instead.

## Testing

```bash
cargo test -p ori_compiler
```
