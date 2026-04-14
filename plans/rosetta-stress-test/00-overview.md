---
plan: "rosetta-stress-test"
title: "Rosetta Code Compiler Stress Test: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "plans/llvm-verification-tooling/"
  - "plans/test-suite-health/"
  - "plans/perf-engineering/"
  - "plans/bug-tracker/"
---

# Rosetta Code Compiler Stress Test: Exhaustive Implementation Plan

## Mission

Methodically implement Rosetta Code tasks in Ori, treating each program as a **deep language evaluation**: push Ori's full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, everything) to write the most elegant version possible, then run the complete diagnostic/verification/benchmark battery to surface compiler bugs, codegen issues, memory leaks, performance problems, and language design gaps. **The bugs found, language insights recorded, and compiler improvements made are the primary deliverable — the Rosetta tasks are fuel.**

Each program is its own subsection with careful analysis. The plan grows organically: 15 programs per section, and after completing each batch of 15, run `/create-plan` to add the next 15 to this plan.

This plan WILL modify the compiler. Every failure spawns `/fix-bug` with TDD matrix rigor (per CLAUDE.md §Zero Deferral). Missing language features that block elegant implementation are recorded as blockers referencing the roadmap or bug-tracker.

## Mission Success Criteria

- [ ] Every implemented program pushes Ori's feature set to its full extent — no workarounds for "missing features" without recording the gap
- [ ] Every program passes the complete per-program pipeline (see §Per-Program Pipeline below)
- [ ] Every compiler bug discovered is filed via `/add-bug` or fixed via `/fix-bug`
- [ ] Every language/syntax gap is documented with a roadmap/bug-tracker cross-reference
- [ ] Performance baselines captured for every program (interpreter, AOT debug, AOT release)
- [ ] Zero dual-exec mismatches between interpreter and LLVM for AOT-eligible programs
- [ ] Zero memory leaks (`ORI_CHECK_LEAKS=1`) on all AOT programs
- [ ] Clean codegen audit (`ORI_AUDIT_CODEGEN=1`) on all AOT programs
- [ ] Task manifest tracks per-program status and findings
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

## Per-Program Pipeline

**Every single Rosetta program undergoes ALL of these steps.** This is the heart of the plan — no shortcuts, no partial runs.

### Phase A: Language Design Analysis
| Step | Action | Finding Type |
|------|--------|-------------|
| A1 | Read Rosetta task definition, understand the problem | — |
| A2 | Design the most idiomatic Ori solution — use FULL feature set (generics, closures, traits, pattern matching, iterators, sum types, `as`/`as?`, everything) | Language elegance |
| A3 | Write implementation with `@main` | — |
| A4 | Write comprehensive tests in `_test/` (`use std.testing { assert_eq }`) | — |
| A5 | Copy task definition: `_tasks/NNN_Name.md` → `<task>/task.md` | — |
| A6 | Document language findings: where Ori shines, where it forces workarounds, missing features → blocker referencing roadmap/bug-tracker | Language gaps |

### Phase B: Compiler Correctness (Lexer → Parser → Typeck → Eval)
| Step | Command | What It Catches |
|------|---------|----------------|
| B1 | `ori check <task>.ori` | Type errors, inference failures, bad error messages |
| B2 | `ORI_LOG=ori_types=debug ori check <task>.ori` | Type inference decision tracing |
| B3 | `ORI_DUMP_AFTER_PARSE=1 ori check <task>.ori` | Parser AST structure problems |
| B4 | `ORI_DUMP_AFTER_TYPECK=1 ori check <task>.ori` | Type resolution problems |
| B5 | `ori test <task>/_test/<task>.test.ori` | Interpreter correctness |
| B6 | `ori run <task>.ori` | `@main` interpreter output |

### Phase C: LLVM Codegen & AOT
| Step | Command | What It Catches |
|------|---------|----------------|
| C1 | `ori build <task>.ori -o /tmp/<task>_debug` | AOT compilation errors |
| C2 | `ori build --release <task>.ori -o /tmp/<task>_release` | Release build issues |
| C3 | `ORI_DUMP_AFTER_LLVM=1 ori build <task>.ori` | Generated LLVM IR quality inspection |
| C4 | `ORI_DUMP_AFTER_ARC=1 ori build <task>.ori` | ARC IR / RC strategy decisions |
| C5 | Run debug binary, capture stdout | Debug AOT correctness |
| C6 | Run release binary, capture stdout | Release AOT correctness |
| C7 | `diagnostics/dual-exec-debug.sh <task>.ori` | Interpreter vs AOT output mismatch |
| C8 | `diagnostics/debug-release-compare.sh <task>.ori` | Debug vs release behavioral divergence |

### Phase D: Memory & ARC Verification
| Step | Command | What It Catches |
|------|---------|----------------|
| D1 | `ORI_CHECK_LEAKS=1 /tmp/<task>_debug` | Memory leaks (live RC objects at exit) |
| D2 | `ORI_TRACE_RC=1 /tmp/<task>_debug` | RC event trace (alloc/inc/dec/free log) |
| D3 | `ORI_RT_DEBUG=1 /tmp/<task>_debug` | Runtime assertions (header validation) |
| D4 | `ORI_VERIFY_ARC=1 ori build <task>.ori` | ARC IR correctness + per-function LLVM IR verify |
| D5 | `ORI_VERIFY_EACH=1 ori build <task>.ori` | Which LLVM optimization pass breaks IR |
| D6 | `ORI_LLVM_LINT=1 ori build <task>.ori` | Likely-UB patterns (div-by-zero, alignment, unreachable) |
| D7 | `diagnostics/rc-stats.sh <task>.ori` | Per-function RC balance (alloc+inc vs dec+free) |
| D8 | `diagnostics/rc-stats.sh --block-level <task>.ori` | Per-basic-block RC breakdown |
| D9 | `diagnostics/codegen-audit.sh <task>.ori` | Static RC/COW/ABI analysis |
| D10 | `diagnostics/codegen-audit.sh --strict <task>.ori` | Pessimistic codegen analysis |
| D11 | `diagnostics/bisect-passes.sh <task>.ori` (if RC issue found) | Which AIMS pipeline phase caused imbalance |

### Phase E: Debug Symbols & Binary Quality
| Step | Command | What It Catches |
|------|---------|----------------|
| E1 | `readelf --debug-dump=info /tmp/<task>_debug \| grep DW_TAG_subprogram` | Missing function debug symbols |
| E2 | `readelf --debug-dump=line /tmp/<task>_debug` | Missing line number tables |
| E3 | Record binary sizes: debug, release, stripped | Binary bloat |

### Phase F: Performance Benchmarking
| Step | Measurement | Purpose |
|------|------------|---------|
| F1 | `time ori run <task>.ori` (3 runs, median) | Interpreter speed |
| F2 | `time /tmp/<task>_debug` (3 runs, median) | AOT debug speed |
| F3 | `time /tmp/<task>_release` (3 runs, median) | AOT release speed |
| F4 | AOT compile time (debug + release) | Compilation speed |
| F5 | AOT-vs-interpreter speedup ratio | Codegen quality metric |
| F6 | Debug-vs-release speedup ratio | Optimizer effectiveness |

### Phase G: Bug Filing & Findings
| Step | Action | Artifact |
|------|--------|---------|
| G1 | Any compiler crash/ICE → `/add-bug` | Bug tracker entry |
| G2 | Any wrong output → `/add-bug` | Bug tracker entry |
| G3 | Any memory leak → `/add-bug` | Bug tracker entry |
| G4 | Missing language feature blocking elegant impl → record as blocker with roadmap xref | Blocker in manifest + subsection notes |
| G5 | Syntax gap (Ori should support X but doesn't) → record in subsection analysis | Language finding |
| G6 | Bad error message → `/add-bug` | Bug tracker entry |
| G7 | Performance anomaly (debug > release, 100x slower than expected) → investigate → `/add-bug` if codegen issue | Bug tracker entry |
| G8 | Update `rosetta-manifest.json` with program status and findings | Manifest entry |

## Architecture

```
  Per-Program Lifecycle (each program = 1 subsection)
  ===================================================

  1. Read task definition from _tasks/NNN_Name.md
  2. Design most idiomatic Ori solution (push full feature set)
  3. Write implementation + @main + tests
  4. Run Phase A-G pipeline (see tables above)
  5. File/fix every bug discovered
  6. Record language findings in subsection notes
  7. Update manifest

  Plan Growth (organic, 15 at a time)
  ====================================

  Section 01: Infrastructure + First 15 Programs
       ↓ (after completion)
  /create-plan → Section 02: Next 15 Programs
       ↓ (after completion)
  /create-plan → Section 03: Next 15 Programs
       ↓ (repeat until corpus exhausted or value diminishes)

  rosetta-manifest.json (SSOT for all program metadata)
  ├── status, tier, features, has_main, has_tests
  ├── aot_eligible, skip_reason (→ roadmap/bug-tracker xref)
  ├── perf: { interp_ms, aot_debug_ms, aot_release_ms, ... }
  ├── bugs_filed: [BUG-XX-NNN, ...]
  └── language_findings: ["no generic constraints yet", ...]
```

## Design Principles

1. **Elegance first, workarounds never.** Each program must use Ori's full feature set. If a feature doesn't work, that's a finding — not a reason to write ugly code. Record the gap, reference the roadmap blocker, and implement the best possible version given current limitations.

2. **Each program is a deep evaluation.** Not "implement and move on" — carefully analyze what worked, what didn't, what Ori does better than other languages, and what it lacks. The analysis is as valuable as the code.

3. **Organic growth.** 15 programs per section. After each batch, assess what was learned and use `/create-plan` to add the next 15. Task selection for subsequent batches is informed by gaps discovered in prior batches.

4. **Full pipeline, no shortcuts.** Every program runs Phases A through G. Skipping steps hides bugs. The verification tools from `plans/llvm-verification-tooling/` (`ORI_VERIFY_ARC`, `ORI_VERIFY_EACH`, `ORI_LLVM_LINT`, `bisect-passes.sh`, `codegen-audit.sh --strict`) exist precisely for this purpose.

5. **Bugs are the deliverable.** The Rosetta tasks are fuel for finding compiler issues. Every Phase B-G failure is a discovery that improves the compiler.

## Section Dependency Graph

```
  01 (Infrastructure + First 15 Programs)
       │
       └──→ /create-plan adds Section 02 (Next 15)
                 │
                 └──→ /create-plan adds Section 03 (Next 15)
                           │
                           └──→ ... (organic growth)
```

Each section is self-contained: infrastructure is in §01, subsequent sections add programs only.

**Cross-plan dependencies:**
- **llvm-verification-tooling** (active reroute, 78% complete): Built the verification test suites and env flags used in the per-program pipeline. The tools are delivered; this plan consumes them.
- **test-suite-health** (active reroute, order 7): Tracks 3,956 LCFail tests. Programs using features blocked in LLVM mode get `skip_reason` referencing the specific LCFail category.
- **perf-engineering** (queued, order 10): Uses rosetta ackermann as a benchmark (BUG-03-004). Performance data from this plan feeds perf-engineering.
- **bug-tracker**: All discovered bugs filed there.

## Implementation Sequence

```
Phase 0 - Infrastructure (§01.PRE)
  └─ 01.PRE.1: Task manifest schema + rosetta-manifest.json
  └─ 01.PRE.2: Task file reorganization (move _tasks/*.md into task folders)
  └─ 01.PRE.3: Update README, docs

Phase 1 - First 15 Programs (§01.1 through §01.15)
  └─ Each program = 1 subsection running full Phase A-G pipeline
  └─ Programs selected from existing 20 implementations + new tasks
  Gate: 15 programs pass full pipeline, all bugs filed/fixed

Phase 2+ - Organic Growth
  └─ /create-plan adds §02 with next 15 programs
  └─ Task selection informed by Phase 1 findings
```

## Metrics (Current State)

| Category | Count |
|----------|-------|
| Task definition files in `_tasks/` | 599 (all populated) |
| Implemented task directories | 20 |
| Tasks passing interpreter | ~18 (stack/queue/reverse_string skipped) |
| Tasks building through AOT | 7 (those with `@main`) |

## Estimated Effort

| Section | Programs | Est. Lines | Complexity |
|---------|----------|-----------|------------|
| 01 Infrastructure | — | ~200 | Medium |
| 01.1-01.15 Programs | 15 | ~50-100 each | Variable |
| Bug fixes discovered | — | ~500-1500 | High |
| **Total Section 01** | **15** | **~2000-3500** | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Status |
|-----|-----------|--------|
| BUG-03-004: Interpreter 63µs/call | Environment cloning | Escalated to perf-engineering |
| BUG-04-033: Multi-clause LLVM codegen | PHI node mismatch | Fixed |
| stack/queue `#skip` | Methods not implemented or wrong API | Not Started |
| reverse_string `#skip` | String slice not implemented | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Infrastructure + First 15 Programs | `section-01-first-15.md` | Not Started |
