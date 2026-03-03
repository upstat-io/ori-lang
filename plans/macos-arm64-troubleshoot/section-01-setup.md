---
section: "01"
title: "Environment Setup"
status: not-started
goal: "Get the Ori compiler building on macOS ARM64"
sections:
  - id: "01.1"
    title: "Prerequisites"
    status: not-started
  - id: "01.2"
    title: "Build & Verify"
    status: not-started
---

# Section 01: Environment Setup

**Status:** Not Started
**Goal:** Ori compiler builds and the interpreter runs on macOS ARM64.

---

## 01.1 Prerequisites

Install LLVM 21 and set up the build environment.

```bash
# 1. Install LLVM 21 via Homebrew
brew install llvm@21

# 2. Set the LLVM prefix for llvm-sys (add to ~/.zshrc or ~/.bashrc)
export LLVM_SYS_211_PREFIX="$(brew --prefix llvm@21)"

# 3. Verify LLVM is found
ls "$LLVM_SYS_211_PREFIX/bin/llc"
# Should print a path — if not, LLVM install is wrong

# 4. Ensure Rust stable toolchain
rustup default stable
rustc --version
# Should be 1.8x+
```

---

## 01.2 Build & Verify

```bash
# 5. Clone (if not already) and build
cd ~/projects/ori_lang   # or wherever you cloned it
cargo build 2>&1 | tee build.log

# 6. Verify the binary exists
ls -la target/debug/ori

# 7. Quick interpreter smoke test (should work even if AOT is broken)
echo '@main () -> void = print(msg: "hello");' > /tmp/smoke.ori
cargo run -p oric --bin ori -- run /tmp/smoke.ori
# Expected output: hello

# 8. Quick check (type checker only, no codegen)
cargo run -p oric --bin ori -- check /tmp/smoke.ori
# Expected: no errors
```

**If step 5 fails:** Paste the full `build.log` — likely a missing LLVM lib or version mismatch.
**If step 7 fails:** Paste the error — interpreter issues are separate from the AOT stack overflow.
**If step 7 succeeds but step 8 fails:** Unlikely but paste the error.

---

## Completion Checklist

- [ ] `cargo build` succeeds
- [ ] `ori run /tmp/smoke.ori` prints "hello"
- [ ] `ori check /tmp/smoke.ori` reports no errors
