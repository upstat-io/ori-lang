---
section: "06"
title: "Verification & Merge Gate"
status: not-started
goal: "Full verification pass confirming all fixes are correct, no regressions exist, and the branch is ready to merge"
third_party_review: false
depends_on:
  - "02"
  - "03"
  - "04"
  - "05"
sections:
  - id: "06.1"
    title: "Test Suite Verification"
    status: not-started
  - id: "06.2"
    title: "Memory Safety Verification"
    status: not-started
  - id: "06.3"
    title: "Behavioral Parity Verification"
    status: not-started
  - id: "06.4"
    title: "Code Quality & Merge Gate"
    status: not-started
---

# Section 06: Verification & Merge Gate

**Status:** Not Started
**Goal:** Run the full verification battery -- test suites, memory safety tools, behavioral parity checks, and code quality gates -- confirming the branch is ready to merge to master.

**Context:** This section runs AFTER all fixes (Sections 02-03) are implemented and all tests (Sections 04-05) are written. No code changes here -- only verification. If any check fails, return to the appropriate section and fix.

---

## 06.1 Test Suite Verification

Run all test suites in both debug and release configurations.

### Commands

```bash
# Full test suite (debug)
timeout 150 ./test-all.sh

# Full test suite (release)
timeout 150 cargo test --release

# Spec tests (Ori language)
timeout 150 cargo st

# LLVM-specific tests
timeout 150 cargo test -p ori_llvm

# ARC-specific tests
timeout 150 cargo test -p ori_arc

# Runtime tests
timeout 150 cargo test -p ori_rt
```

### Expected Results

| Suite | Expected |
|-------|----------|
| `./test-all.sh` | All pass (current baseline + new matrix tests) |
| `cargo test --release` | All pass |
| `cargo st` | All pass |
| `cargo test -p ori_llvm` | All pass |
| `cargo test -p ori_arc` | All pass |
| `cargo test -p ori_rt` | All pass |

- [ ] `timeout 150 ./test-all.sh` -- all pass
- [ ] `timeout 150 cargo test --release` -- all pass
- [ ] `timeout 150 cargo st` -- all pass
- [ ] `timeout 150 cargo test -p ori_llvm` -- all pass
- [ ] `timeout 150 cargo test -p ori_arc` -- all pass
- [ ] `timeout 150 cargo test -p ori_rt` -- all pass
- [ ] No new test failures compared to pre-fix baseline
- [ ] No test timeouts (all complete within 150s)

---

## 06.2 Memory Safety Verification

Run memory safety tools on representative programs from the test matrix.

### Valgrind

Run `valgrind-aot.sh` on fat-pointer iterator programs. Focus on programs that exercise the fixed code paths.

```bash
# Default Valgrind suite
diagnostics/valgrind-aot.sh

# Fat pointer iteration programs
diagnostics/valgrind-aot.sh tests/valgrind/iter_str.ori tests/valgrind/iter_option_str.ori tests/valgrind/iter_nested_list.ori

# For-yield specific programs
diagnostics/valgrind-aot.sh tests/valgrind/for_yield_str.ori tests/valgrind/for_yield_option_str.ori

# Map iteration with str keys (Section 02.3 fix verification)
diagnostics/valgrind-aot.sh tests/valgrind/iter_map_str_keys.ori
```

### Leak Check

Run all iterator test programs with `ORI_CHECK_LEAKS=1`:

```bash
ORI_CHECK_LEAKS=1 ./target/debug/test_program
```

### RC Tracing

Spot-check RC balance on critical programs. `rc-stats.sh` analyzes LLVM IR for static RC balance; `ORI_TRACE_RC=1` captures runtime traces for dynamic verification.

```bash
# Static analysis (LLVM IR)
diagnostics/rc-stats.sh tests/spec/iterators/rc_matrix/for_yield_str_full.ori

# Dynamic trace (runtime)
ORI_TRACE_RC=1 ./target/debug/test_program
```

### Codegen Audit

Run codegen audit on for-yield programs:

```bash
ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ori build tests/spec/iterators/rc_matrix/for_yield_str_full.ori
```

- [ ] `diagnostics/valgrind-aot.sh` -- zero errors on default suite
- [ ] Valgrind on `[str]` for-yield program -- zero errors (no leaks, no invalid reads/writes)
- [ ] Valgrind on `[Option<str>]` for-yield program -- zero errors
- [ ] Valgrind on `[[int]]` for-yield program -- zero errors
- [ ] Valgrind on `[str]` for-do program -- zero errors (regression check)
- [ ] Valgrind on `{str: int}` map for-do program -- zero errors (Section 02.3 fix verification)
- [ ] `ORI_CHECK_LEAKS=1` on all matrix test programs -- zero leak reports
- [ ] `diagnostics/rc-stats.sh` on for-yield `[str]` program -- balanced LLVM IR RC ops (incs+allocs == decs)
- [ ] `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1` on for-yield programs -- zero findings
- [ ] `ORI_RT_DEBUG=1` on all matrix test programs -- zero assertion failures

---

## 06.3 Behavioral Parity Verification

Verify that the interpreter and AOT backend produce identical results for all test programs.

### Dual-Exec Verify

```bash
# Full batch verification
diagnostics/dual-exec-verify.sh tests/spec/iterators/rc_matrix/

# Individual verification
diagnostics/dual-exec-debug.sh tests/spec/iterators/rc_matrix/for_yield_str_full.ori
```

### Code Journey Re-run

Re-run any existing code journeys that exercise iterator paths to confirm no regressions:

```bash
# Check which journeys use for-loops with collections
# Re-run affected journeys with full diagnostics
```

### Release Build Behavioral Tests

The release build may differ from debug due to FastISel behavior. Run behavioral tests specifically with the release binary:

```bash
cargo build --release
# Run each test program with the release binary
./target/release/ori run tests/spec/iterators/rc_matrix/for_yield_str_full.ori
```

- [ ] `diagnostics/dual-exec-verify.sh` on all matrix test programs -- interpreter matches AOT
- [ ] `diagnostics/dual-exec-debug.sh` on for-yield `[str]` -- no mismatch
- [ ] `diagnostics/dual-exec-debug.sh` on for-yield `[Option<str>]` -- no mismatch
- [ ] Re-run existing code journeys exercising iterators -- no regressions
- [ ] Release build produces same output as debug for all test programs
- [ ] No SIGSEGV or SIGABRT in release build (FastISel regression check)

---

## 06.4 Code Quality & Merge Gate

Final quality checks before merge.

### Clippy

```bash
./clippy-all.sh
```

### Formatting

```bash
./fmt-all.sh
```

### Build Variants

```bash
# Debug build (includes LLVM)
cargo build

# Release build
cargo build --release

# Release-LTO build -- verify it compiles
cargo build --profile release-lto
```

### Documentation

- Verify `plans/iter-rc-contract/00-overview.md` status table is updated to `complete`
- Verify `plans/iter-rc-contract/index.md` status entries are updated to `complete`
- Verify any new test files have `//!` module docs

### Merge Criteria

All of the following must be true:

1. `./test-all.sh` green (debug)
2. `cargo test --release` green
3. `./clippy-all.sh` green (zero warnings)
4. `./fmt-all.sh` produces no changes
5. Valgrind clean on fat-pointer iterator programs
6. `ORI_CHECK_LEAKS=1` clean on all matrix tests
7. `dual-exec-verify.sh` shows interpreter-AOT parity
8. No new `#[allow(clippy::...)]` without justification
9. No files over 500 lines (excluding tests) -- `walk.rs` (595), `realize/mod.rs` (505), `transfer/mod.rs` (516) are over limit. If this plan touched any of them, they must be split before merge. If untouched by this plan, document in merge notes.
10. All plan section statuses updated to `complete`

- [ ] `./clippy-all.sh` -- zero warnings
- [ ] `./fmt-all.sh` -- no changes needed
- [ ] `cargo build` -- success (debug)
- [ ] `cargo build --release` -- success
- [ ] `cargo build --profile release-lto` -- success
- [ ] Plan overview status table updated to `complete`
- [ ] Plan index status entries updated to `complete`
- [ ] New test files have `//!` module docs
- [ ] No files over 500 lines (excluding tests) touched by this plan without splitting
- [ ] No new `#[allow(clippy::...)]` without `#[expect]` and reason
- [ ] All 10 merge criteria satisfied

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] All test suites pass in debug and release (Section 06.1)
- [ ] Valgrind clean on all fat-pointer iterator programs (Section 06.2)
- [ ] Zero leaks reported by `ORI_CHECK_LEAKS=1` (Section 06.2)
- [ ] RC balance verified by `rc-stats.sh` (LLVM IR static analysis) and `ORI_TRACE_RC=1` (runtime dynamic traces) (Section 06.2)
- [ ] Interpreter-AOT parity confirmed by `dual-exec-verify.sh` (Section 06.3)
- [ ] Release build behavioral parity confirmed (Section 06.3)
- [ ] Clippy and formatting clean (Section 06.4)
- [ ] All merge criteria satisfied (Section 06.4)
- [ ] Plan section statuses updated to `complete`

---

## Section 06 Exit Criteria

All verification checks pass. The branch is ready to merge: tests green in debug+release, Valgrind clean, zero leaks, interpreter-AOT parity confirmed, clippy clean, formatting clean. All plan sections updated to complete status.
