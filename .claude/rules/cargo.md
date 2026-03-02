---
paths:
  - "**.toml"
---

# Cargo Configuration

**Do NOT edit Cargo.toml or clippy.toml without explicit user permission.**

## Aliases (`.cargo/config.toml`)

| Alias | Command | Purpose |
|-------|---------|---------|
| `cargo st` | `run -p oric --bin ori -- test tests/` | Ori spec tests |
| `cargo stv` | `run -p oric --bin ori -- test --verbose` | Spec tests verbose |
| `cargo stf` | `run -p oric --bin ori -- test --filter` | Spec tests filtered |
| `cargo t` | `test --workspace` | All Rust unit tests (incl. LLVM) |
| `cargo tv` | `test --workspace -- --nocapture` | Rust tests with output |
| `cargo tc` | `test -p` | Tests for specific crate (e.g., `cargo tc ori_parse`) |
| `cargo c` | `check --workspace` | Check all crates (incl. LLVM) |
| `cargo b` | `build --workspace` | Build all crates (incl. LLVM) |
| `cargo cl` | `clippy --workspace --all-targets` | Clippy all crates (incl. LLVM) |

## Workspace Structure
- All crates including `ori_llvm` are in `default-members`; LLVM 17 is required for all builds
- `llvm` is a default feature of `oric` — bare `cargo build` includes LLVM
- `compile_error!` prevents building `oric` without LLVM
- `tools/ori-lsp` is fully excluded (not a workspace member)

## Workspace Lints (deny level)
`unsafe_code` (except `ori_rt`), `dead_code`, `unused`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::todo`, `clippy::unimplemented`, `clippy::dbg_macro`

## Key Files
- `Cargo.toml` — workspace config
- `.cargo/config.toml` — aliases, LLVM path
