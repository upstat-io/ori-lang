# oric

> **`oric` is the native compiler driver.** The only impure crate in the compiler — absorbs IO, Salsa, and LLVM so the rest of the pipeline stays pure.
>
> Full mission: [`.claude/rules/missions.md §oric`](../../.claude/rules/missions.md)

## Role in the system

`oric` is the compiler binary — the thing that gets built as `target/debug/ori` (and `~/.local/bin/ori` when installed). It owns:

- **CLI argument parsing**: `ori check`, `ori run`, `ori build`, `ori fmt`, etc.
- **Salsa orchestration**: incremental compilation database + query execution
- **Filesystem IO**: reading source files, writing artifacts
- **LLVM integration**: via the default `llvm` feature (always on in default builds)
- **Env var consumption**: every `ORI_*` env var is consumed here (or at the LLVM boundary in `ori_llvm`)

IO lives **ONLY** in this crate — core pipeline crates are pure.

## Architecture

- `src/main.rs` — CLI entry, command dispatch
- `src/tracing_setup.rs` — tracing/logging initialization (`ORI_LOG`, `ORI_LOG_TREE`)
- `src/cli/` — command implementations
- `src/diagnostic/` — user-facing diagnostic rendering
- `tests/aims-snapshots/` — AIMS per-pass snapshot corpus
- `tests/phases/` — phase-level integration tests

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `salsa`, `ori_ir`, `ori_registry`, `ori_diagnostic`, `ori_lexer`, `ori_types`, `ori_parse`, `ori_patterns`, `ori_eval`, `ori_canon`, `ori_fmt`; plus `ori_llvm`, `ori_arc`, `ori_repr` via default `llvm` feature |
| Dev-only | `ori_rt` |
| Downstream | nothing — this is the binary crate |

## Invariants

- **IO lives only here**: if a core crate wants to read a file, the IO is refactored through `oric` instead.
- **Env var consumption is centralized**: env vars consumed outside `oric` (or `ori_llvm`'s LLVM layer) are phase-purity violations.
- **Salsa queries must be pure**: no side effects, no non-determinism, no `Arc<Mutex<T>>` / fn pointers / `dyn Trait` in query inputs or values.
- **Default build includes LLVM**: `llvm` is a default feature; bare `cargo build` produces a full-featured compiler. Disabling requires `--no-default-features` and is unsupported for normal builds.

## Feature flags

- `default` = `["llvm"]`
- `llvm` — enables LLVM backend (bringing in `ori_llvm`, `ori_arc`, `ori_repr`)

## Testing

```bash
cargo test -p oric
# AIMS snapshots
cargo test -p oric --test aims_snapshots
# Phase-level tests
cargo test -p oric --test phases
# Full project
./test-all.sh
```

## Running

```bash
# Check (no codegen)
cargo run -- check path/to/file.ori
# Run (evaluator)
cargo run -- run path/to/file.ori
# Build (AOT binary)
cargo run -- build path/to/file.ori
```

## Key env vars

See `CLAUDE.md §Tracing` / `.claude/rules/compiler.md §Tracing` for the full set. Commonly used:

- `ORI_LOG=debug` — tracing filter (like `RUST_LOG`)
- `ORI_LOG_TREE=1` — hierarchical tree output
- `ORI_DUMP_AFTER_PARSE=1`, `ORI_DUMP_AFTER_TYPECK=1`, `ORI_DUMP_AFTER_ARC=1`, `ORI_DUMP_AFTER_LLVM=1` — per-phase IR dumps
- `ORI_BLESS=1` — update snapshot baselines

## References

- [`.claude/rules/compiler.md`](../../.claude/rules/compiler.md) — compiler architecture + IO rules + tracing
- [`.claude/rules/aot.md`](../../.claude/rules/aot.md) — AOT build flow
- `CLAUDE.md §Commands` — primary harnesses
