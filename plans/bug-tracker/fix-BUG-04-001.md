---
bug: "BUG-04-001"
title: "Cross-compilation to Windows fails: host linker used instead of cross-linker"
severity: "high"
status: complete
goal: "Cross-compilation attempts that lack a suitable cross-linker fail early with a clear, actionable error instead of silently falling back to the host linker"
success_criteria:
  - "ori build --target=x86_64-pc-windows-msvc on Linux without lld-link returns LinkerError with clear message listing required tools"
  - "ori build --target=x86_64-pc-windows-gnu on Linux without mingw-w64 returns LinkerError with clear message"
  - "Cross-compilation detection correctly identifies host vs target OS mismatch"
  - "GCC cross-compiler name resolution returns correct target-prefixed names"
  - "Native compilation behavior is completely unchanged"
subsystem: "compiler/ori_llvm/src/aot/linker/"
found: "2026-03-28"
source: "manual"
third_party_review:
  status: findings
  updated: 2026-04-06
---

# Fix: BUG-04-001 — Cross-compilation to Windows fails: host linker used instead of cross-linker

**Status:** In Progress
**Severity:** High
**Goal:** When cross-compiling to a different OS and no suitable cross-linker is available, fail immediately with a clear error message explaining what tools are needed — never silently fall back to the host linker.

**Success Criteria:**
- [ ] Cross-compilation without cross-linker returns clear `LinkerError::LinkerNotFound` with actionable help
- [ ] Cross-compiler name resolution produces correct target-prefixed names
- [ ] Native compilation completely unchanged
- [ ] All existing tests pass

**Context:** When building for `x86_64-pc-windows-msvc` from Linux, `LinkerFlavor::for_target()` correctly selects `Msvc`, but `LinkerDetection::is_available()` fails (no MSVC tools on Linux). The fallback cascades to host `cc`, which receives a Windows COFF object and fails with a cryptic `R_AMD64_IMAGEBASE` error. Three sub-issues: (1) no validation that cross-linker exists, (2) no cross-compiled runtime for target, (3) system library selection ignores target OS. This fix addresses (1) directly; (3) is largely resolved by (1) since the correct cross-linker brings its own system libraries.

---

## 1. Root Cause Analysis

- **Symptom**: `ori build hello.ori --target=x86_64-pc-windows-msvc` on Linux produces `R_AMD64_IMAGEBASE with __ImageBase undefined` — GNU ld receiving Windows COFF object.
- **Proximate cause**: `LinkerDriver::link()` falls back to host `cc` when no Windows-specific linker is found.
- **Root cause**: `LinkerDetection::is_available()` is not cross-compilation-aware. It checks if generic `cc` responds to `--version`, which always succeeds on Linux even though the host `cc` cannot link Windows objects. The detection doesn't distinguish "this linker exists" from "this linker is suitable for the target."
- **Blast radius**: All cross-compilation to a different OS family (Linux→Windows, Linux→macOS, etc.) will silently use the host linker and produce confusing errors. Same-OS cross-arch (x86_64→aarch64 Linux) would also fail but with somewhat clearer errors.
- **Affected files**:
  - `compiler/ori_llvm/src/aot/linker/mod.rs` — add cross-compilation detection, target-aware linker availability check, cross-compiler name resolution, and cross-compilation error formatting
  - `compiler/ori_llvm/src/aot/linker/driver.rs` — use target-aware detection, fail early when no suitable cross-linker exists
  - `compiler/ori_llvm/src/aot/linker/gcc.rs` — use cross-compiler program name when cross-compiling

**Reference implementations:**
- **Rust** `rustc_codegen_ssa/back/link.rs`: Uses `LinkerInfo` with target-specific linker selection. Cross-compilation explicitly checked via `sess.target.linker_flavor`.
- **Go** `cmd/link/internal/ld/config.go`: Uses `buildcfg.GOOS`/`GOARCH` to detect cross-compilation and select appropriate tools.

---

## 2. TDD — Test Matrix

### Exact failing case
- [ ] Cross-compile to windows-msvc from Linux with no cross-linker → `LinkerNotFound` error with helpful message

### Cross-target coverage
- [ ] Cross-compile to windows-gnu from Linux → suggests mingw-w64
- [ ] Cross-compile to aarch64-linux-gnu from x86_64-linux → suggests aarch64-linux-gnu-gcc
- [ ] Cross-compile to darwin from Linux → suggests osxcross or lld

### Native compilation (regression guard)
- [ ] Native Linux compilation → uses host `cc`, unchanged behavior
- [ ] LinkerFlavor::for_target selects correct flavor for each target family

### Cross-compiler name resolution
- [ ] windows-gnu arch=x86_64 → x86_64-w64-mingw32-gcc
- [ ] linux-gnu arch=aarch64 → aarch64-linux-gnu-gcc
- [ ] linux-musl arch=aarch64 → aarch64-linux-musl-gcc
- [ ] windows-msvc → None (no GCC cross-compiler for MSVC targets)
- [ ] darwin → None (no standard GCC cross-compiler name)
- [ ] wasm → None (uses wasm-ld, not GCC)

### Cross-compilation detection
- [ ] Same OS = not cross-compiling
- [ ] Different OS = cross-compiling
- [ ] WASM always considered cross-compilation (but handled by wasm-ld path)

### Semantic pin
- [ ] Test that verifies host `cc` is NOT returned as available for a cross-OS target

### Negative pin
- [ ] Test that cross-compilation error message contains the target triple and lists specific tools

### Verify tests fail before fix
- [ ] All new tests fail against current code (confirming they test the right thing)

---

## 3. Implementation

- [ ] Add `is_cross_compiling()` to `LinkerDetection` — compile-time `cfg!` comparison of host OS vs target OS
- [ ] Add `gcc_cross_compiler_name()` — target-prefixed GCC program name for cross-compilation
- [ ] Add `is_available_for_target()` — target-aware linker availability that checks cross-compilers when cross-compiling
- [ ] Add `cross_compilation_error()` to `LinkerDriver` — clear error message with per-target tool suggestions
- [ ] Update `LinkerDriver::link()` — use target-aware detection, fail early with clear error
- [ ] Update `LinkerDriver::create_linker()` — use cross-compiler name for GCC when cross-compiling
- [ ] Add tests in `compiler/ori_llvm/src/aot/linker/tests.rs`

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-006][high]` `compiler/ori_llvm/src/aot/linker/mod.rs:457` / `compiler/ori_llvm/src/aot/linker/driver.rs:116` — same-OS cross-arch targets still fall back to the host `cc`, so BUG-04-001 remains open for part of its documented blast radius.
  Resolved: Fixed on 2026-04-06. Extended `is_cross_compiling()` to compare both host OS and host architecture against target. Added `host_arch` cfg detection. `aarch64-unknown-linux-gnu` on x86_64 Linux now correctly detected as cross-compilation. Tests: `test_linux_x86_to_linux_aarch64_is_cross_compiling` (semantic pin), `test_linux_x86_to_linux_x86_is_not_cross_compiling` (regression guard), `test_host_cc_not_available_for_cross_arch_linux_target` (semantic pin for detection). 25 tests passing.

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `LinkerDetection::is_available_for_target(LinkerFlavor::Gcc, &windows_target)` returns `false` on Linux (does not find host `cc` as suitable for Windows). `LinkerDriver::link()` returns `LinkerError::LinkerNotFound` with message containing "cross-compilation" and the target triple when no suitable cross-linker exists. All existing native compilation tests and build flows pass unchanged.
