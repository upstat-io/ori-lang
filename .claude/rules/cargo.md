---
paths:
  - "**.toml"
---

# Cargo Configuration

**Do NOT edit Cargo.toml or clippy.toml without explicit user permission.**

## Aliases (`.cargo/config.toml`)

| Alias | Command | Purpose |
|-------|---------|---------|
| `cargo t` | `test --workspace --exclude ori_llvm` | All Rust unit tests (excl. LLVM) |
| `cargo tv` | `test --workspace --exclude ori_llvm -- --nocapture` | Rust tests with output |
| `cargo tc` | `test -p` | Tests for specific crate (e.g., `cargo tc ori_parse`) |
| `cargo st` | `run -p oric --bin ori -- test tests/` | Ori spec tests |
| `cargo stv` | `run -p oric --bin ori -- test --verbose` | Spec tests verbose |
| `cargo stf` | `run -p oric --bin ori -- test --filter` | Spec tests filtered |
| `cargo c` | `check --workspace --exclude ori_llvm` | Check all crates (excl. LLVM) |
| `cargo b` | `build --workspace --exclude ori_llvm` | Build all crates (excl. LLVM) |
| `cargo cl` | `clippy --workspace --exclude ori_llvm --all-targets` | Clippy all crates (excl. LLVM) |
| `cargo bl` | `build -p oric -p ori_rt --features llvm` | LLVM debug build (compiler + runtime) |
| `cargo blr` | `build -p oric -p ori_rt --features llvm --release` | LLVM release build |
| `cargo rl` | `run -p oric --features llvm --bin ori --` | Run with LLVM |
| `cargo cll` | `clippy -p ori_llvm --all-targets` | Clippy LLVM crate |

## Workspace Structure
- `ori_llvm` and `ori_rt` are workspace **members** but NOT in `default-members` (require LLVM 17)
- Bare `cargo check`/`cargo test` skip them; `--workspace` aliases use `--exclude ori_llvm`
- Use `-p ori_llvm` or `cargo bl` to build LLVM explicitly
- `tools/ori-lsp` is fully excluded (not a workspace member)

## Workspace Lints (deny level)
`unsafe_code` (except `ori_rt`), `dead_code`, `unused`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::todo`, `clippy::unimplemented`, `clippy::dbg_macro`

## Key Files
- `Cargo.toml` — workspace config
- `.cargo/config.toml` — aliases, LLVM path
