---
section: "05"
title: "Developer Tooling"
status: complete
goal: "cargo run cannot silently strip LLVM feature from the ori binary"
inspired_by: []
depends_on: []
sections:
  - id: "05.1"
    title: "LLVM Feature Guard"
    status: complete
  - id: "05.2"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Developer Tooling

**Status:** Complete
**Goal:** Developers cannot accidentally lose LLVM support by running `cargo run`. The binary at `~/.local/bin/ori` always reflects the intended feature set.

**Context:** Journey 1 discovered that `cargo run -- run file.ori` rebuilds `oric` WITHOUT `--features llvm`, overwriting the LLVM-enabled binary at `target/debug/ori`. The symlink at `~/.local/bin/ori` then points to a non-LLVM binary. Any `cargo run` invocation silently breaks AOT until next `cargo bl`. This is a developer experience trap — easy to trigger, hard to diagnose.

**Depends on:** None (fully independent).

---

## 05.1 LLVM Feature Guard

**File(s):** `compiler/oric/Cargo.toml`, `compiler/oric/src/main.rs`

**Finding #10** (LOW): `cargo run` rebuilds without `--features llvm`, overwriting the LLVM binary.

**Fix approach — 3 options:**

**(a) Cargo alias with default features** (recommended — simplest):
Add `llvm` to `default` features in `oric/Cargo.toml` so `cargo run` always includes LLVM:
```toml
[features]
default = ["llvm"]
llvm = ["dep:ori_llvm"]
```
Builders who explicitly want non-LLVM can use `cargo run --no-default-features`.

**Trade-off:** CI or minimal builds must explicitly opt out with `--no-default-features`.

**(b) Runtime detection + warning**:
At startup, check if LLVM feature is compiled in. If not, print a warning:
```rust
#[cfg(not(feature = "llvm"))]
eprintln!("warning: LLVM support not compiled in. Use `cargo bl` to rebuild with LLVM.");
```

**Downside:** Doesn't prevent the overwrite — just warns after the fact.

**(c) Separate binary names**:
Use `ori` for LLVM-enabled and `ori-lite` for non-LLVM. Different paths, no overwrites.

**Downside:** Adds complexity to tooling and documentation.

**Recommended path:** Option (a) + Option (b) together. Default features include LLVM (prevents accidental stripping) AND a runtime warning if LLVM is missing (catches edge cases).

- [x] Add `llvm` to default features in `compiler/oric/Cargo.toml`
- [x] Add runtime warning when LLVM feature is not compiled in
  ```rust
  // In main.rs early startup:
  #[cfg(not(feature = "llvm"))]
  {
      eprintln!("warning: ori compiled without LLVM support. AOT compilation unavailable.");
      eprintln!("         Rebuild with: cargo bl");
  }
  ```
- [x] Update `build-all.sh` if needed (may need `--no-default-features` for non-LLVM builds)
- [x] Update CLAUDE.md if build commands change
- [x] Test: `cargo run -- run tests/spec/declarations/constants.ori` — produces correct output with LLVM available
- [x] Test: `cargo run --no-default-features -- run tests/spec/declarations/constants.ori` — shows warning, still works for eval

---

## 05.2 Completion Checklist

- [x] `cargo run` includes LLVM feature by default
- [x] `cargo run --no-default-features` explicitly opts out (with warning)
- [x] `~/.local/bin/ori` symlink always points to LLVM-enabled binary after `cargo run`
- [x] `build-all.sh` still works correctly
- [x] `cargo bl` / `cargo blr` still work correctly

**Exit Criteria:** Running `cargo run -- run file.ori` does NOT overwrite the LLVM-enabled binary. `ori build file.ori` works immediately after `cargo run` without needing `cargo bl`.
