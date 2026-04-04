---
plan: "llvm-worker-isolation"
title: "LLVM Worker Subprocess Isolation"
status: not-started
supersedes: []
references:
  - "compiler/oric/src/test/runner/llvm_backend.rs"
  - "compiler/oric/src/test/runner/mod.rs"
  - "compiler/oric/src/test/result/mod.rs"
  - "compiler/oric/src/commands/test.rs"
  - "compiler/oric/src/main.rs"
  - "test-all.sh"
---

# LLVM Worker Subprocess Isolation

## Mission

Isolate the LLVM backend spec test runner from C++ SIGSEGV crashes by running each test file's LLVM compilation and execution in a separate subprocess. The parent process (orchestrator) survives worker crashes, reports them as real failures, and continues processing remaining files. This unblocks the pre-commit hook for any `.rs` changes that expand LLVM test coverage.

## Mission Success Criteria

- [ ] `./test-all.sh` passes cleanly — LLVM backend no longer crashes the parent process
- [ ] All 2098+ AOT integration tests pass (zero regressions from this change)
- [ ] All 4415+ interpreter spec tests pass (unchanged)
- [ ] All 7379+ Rust unit tests pass (including new tests for this plan)
- [ ] Worker crashes (SIGSEGV, SIGABRT) are detected and reported as `BackendCrash` — a distinct failure category that blocks the test gate (not `LlvmCompileFail`, not hidden)
- [ ] `ori test --backend=llvm --json <file>` emits sentinel-framed JSON to stdout for any input (success, failure, compile error, crash) — framing is robust against Ori `print()` output on stdout
- [ ] Performance: LLVM backend spec test run completes within 2x of current wall-clock time (subprocess overhead bounded)
- [ ] **Weakened test gate reverted**: The `ORI_LLVM_CRASHED` exit-0 escape hatch in `test-all.sh` (lines 541-558) is removed — LLVM backend crashes now produce exit code 1 via `BackendCrash` outcomes, making the workaround unnecessary. The gate is restored to full strength: crashes are real failures that block the pre-commit hook, not "known issues" that get a pass.

## Architecture

```
CURRENT (in-process, crashes entire runner):

  ori test --backend=llvm tests/spec/
    ↓
  for each file:
    run_file_llvm()  ← catch_unwind wraps Rust panics
      ↓                 but NOT C++ SIGSEGV
    compile_module_with_tests()  ← LLVM C++ can crash here
      ↓
    run tests from compiled module
    ↓
  collect FileSummary → TestSummary → exit code


NEW (subprocess isolation):

  ori test --backend=llvm tests/spec/
    ↓
  for each file (bounded pool, ~CPU count):
    spawn: ori test --backend=llvm --json <file>
      ↓ (separate process, separate address space)
      ↓ worker: parse → typecheck → LLVM compile → run tests
      ↓ stdout: sentinel-framed JSON FileSummary
      ↓ exit code: 0=pass, 1=fail, 2=no-tests; signal=crash
    ↓
  parent: wait for child
    exit 0     → extract sentinel-framed JSON → aggregate results
    exit 1     → extract sentinel-framed JSON → aggregate failures
    signal     → BackendCrash outcome (detected via ExitStatus::signal())
    no frame   → BackendCrash (crash before JSON emission)
    timeout    → kill child → BackendCrash with timeout message
    ↓
  collect FileSummary → TestSummary → exit code
```

## Design Principles

1. **Process boundary is the only sound isolation for C++ crashes.** Rust's `catch_unwind` catches Rust panics but not SIGSEGV/SIGABRT from LLVM C++ code. Thread isolation doesn't help — a SIGSEGV kills the entire process, not just the offending thread. Subprocess isolation provides OS-level fault containment. Evidence: Zig uses subprocesses for clang codegen isolation (`Compilation.zig:6304-6334`).

2. **Self-invocation, not a new binary.** The `ori` binary already handles `ori test --backend=llvm <file>` for single files. The orchestrator spawns itself with `std::env::current_exe()`. No new binary, no IR serialization, no serde for IR types — just a `--json` flag for structured output. Evidence: the existing CLI dispatch in `main.rs` already routes single-file test execution through the full pipeline.

3. **Crashes are failures, not special cases.** A worker crash produces `BackendCrash` — a distinct `TestOutcome` variant that counts as a real failure in `has_failures()` and blocks the test gate. Worker crashes are never downgraded to `LlvmCompileFail` or hidden. This preserves test gate integrity.

## Section Dependency Graph

```
§01 JSON Output Protocol
  ↓
§02 Subprocess Orchestrator
  ↓
§03 Verification
```

All sections are sequential — each depends on the prior.

## Implementation Sequence

```
Phase 1 - Protocol
  └─ §01: Add --json flag, serde derives, JSON emission

Phase 2 - Orchestrator
  └─ §02: Subprocess spawning, result parsing, crash detection,
           bounded parallel pool, timeout handling

Phase 3 - Verification
  └─ §03: Test matrix, performance measurement, integration
```

**Why this order:**
- Phase 1 establishes the wire protocol. Without JSON output, the orchestrator can't parse results.
- Phase 2 uses the protocol. The orchestrator depends on structured output.
- Phase 3 verifies the whole system works end-to-end.

## Metrics (Current State)

| File | Production LOC | Role | Hygiene |
|------|---------------|------|---------|
| `compiler/oric/src/test/runner/llvm_backend.rs` | 545 | LLVM test runner (in-process JIT pipeline — stays as worker path) | **[BLOAT]** 545 lines, over 500-line limit. Has 3 `#[expect(clippy::...)]` suppressions. |
| `compiler/oric/src/test/runner/mod.rs` | 572 | Test routing, Backend enum, config | **[BLOAT]** 572 lines, over 500-line limit. `run_file_with_interner` has `#[expect(clippy::too_many_lines)]`. |
| `compiler/oric/src/test/result/mod.rs` | 327 | TestOutcome, FileSummary, TestSummary, CoverageReport | Clean. Under limit. |
| `compiler/oric/src/commands/test.rs` | 233 | Test command entry point, output formatting | Clean. Under limit. |
| `compiler/oric/src/main.rs` | 405 | CLI dispatch | `real_main` has `#[expect(clippy::too_many_lines, clippy::cognitive_complexity)]`. |
| `test-all.sh` | 562 | Test orchestration script | **[BLOAT]** 562 lines. Contains `ORI_LLVM_CRASHED` weakened gate (lines 556-558). |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 JSON Output Protocol | ~150 new, ~30 modified | Low | — |
| 02 Subprocess Orchestrator | ~250 new, ~100 modified | Medium | 01 |
| 03 Verification | ~100 new (tests) | Low | 02 |
| **Total new** | **~500** | | |
| **Total modified** | **~130** | | |

## Codebase Hygiene Findings

These issues exist in files this plan touches. They should be fixed along the way (per CLAUDE.md "continuous improvement everywhere" rule).

| Tag | File | Line(s) | Finding |
|-----|------|---------|---------|
| **[BLOAT]** | `runner/mod.rs` | - | 572 lines, over 500-line limit. `run_file_with_interner` has `#[expect(clippy::too_many_lines)]`. Not addressed in this plan (extraction would be a separate effort), but no net lines should be added. |
| **[BLOAT]** | `llvm_backend.rs` | - | 545 lines, over 500-line limit. Has 3 `#[expect(clippy::...)]` suppressions. This file stays as-is (worker path). |
| **[BLOAT]** | `test-all.sh` | - | 562 lines. Weakened gate logic adds ~20 lines of dead-code-to-be. Plan actively removes these (02.4). |
| **[WASTE]** | `runner/mod.rs` | 116-120 | Stale LLVM sequential execution comment block. Describes old in-process approach. Plan removes this in 02.4. |
| **[WASTE]** | `test-all.sh` | 458, 526, 556 | `ORI_LLVM_CRASHED` references — dead code after subprocess isolation. Plan removes all in 02.4. |
| **[WASTE]** | `test-all.sh` | 546 | `ANY_CORE_FAILED` variable — unnecessary with subprocess isolation. Plan removes in 02.4. |
| **[NOTE]** | `result/mod.rs` | 327 | Tests in sibling `tests.rs` — correct pattern. 148 lines of tests. |
| **[NOTE]** | `commands/test.rs` | 233 | Clean, well-structured. Under limit. |
| **[NOTE]** | `main.rs` | 405 | `real_main` has 2 clippy suppressions but is a CLI router — acceptable per pattern. |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| LLVM backend spec tests crash with SIGSEGV | Malformed IR from unresolved type variables (Root Causes A-C) passed to LLVM C++ | JIT EH §06.2-06.8 | Not Started — this plan provides containment, not fix |
| `var_emitted()` returns `ValueId::NONE` on undefined vars | Missing ARC variable definitions from unresolved monomorphization | JIT EH §06.2 | Hardened with poison value (committed fd37c5c1) |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | JSON Output Protocol | `section-01-json-protocol.md` | Not Started |
| 02 | Subprocess Orchestrator | `section-02-orchestrator.md` | Not Started |
| 03 | Verification | `section-03-verification.md` | Not Started |
