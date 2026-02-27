---
paths:
  - "**/Cargo.toml"
  - "**/clippy.toml"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# Cargo Configuration

**Do NOT edit Cargo.toml or clippy.toml without explicit user permission.**

## Aliases (`.cargo/config.toml`)

| Alias | Command | Purpose |
|-------|---------|---------|
| `cargo t` | `test --workspace` | All Rust unit tests |
| `cargo tv` | `test --workspace -- --nocapture` | Rust tests with output |
| `cargo tc` | `test -p` | Tests for specific crate (e.g., `cargo tc ori_parse`) |
| `cargo st` | `run -p oric --bin ori -- test tests/` | Ori spec tests |
| `cargo stv` | `run -p oric --bin ori -- test --verbose` | Spec tests verbose |
| `cargo stf` | `run -p oric --bin ori -- test --filter` | Spec tests filtered |
| `cargo c` | `check --workspace` | Check all crates |
| `cargo b` | `build --workspace` | Build all crates |
| `cargo cl` | `clippy --workspace --all-targets` | Clippy all crates |
| `cargo bl` | `build -p oric -p ori_rt --features llvm` | LLVM debug build (compiler + runtime for AOT) |
| `cargo blr` | `build -p oric -p ori_rt --features llvm --release` | LLVM release build (compiler + runtime for AOT) |
| `cargo rl` | `run -p oric --features llvm --bin ori --` | Run with LLVM |
| `cargo cll` | `clippy --manifest-path compiler/ori_llvm/Cargo.toml --all-targets` | Clippy LLVM crate |

## Workspace Lints (deny level)
`unsafe_code` (except `ori_rt`), `dead_code`, `unused`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::todo`, `clippy::unimplemented`, `clippy::dbg_macro`

## Key Files
- `Cargo.toml`: Workspace config
- `.cargo/config.toml`: Aliases, LLVM path
