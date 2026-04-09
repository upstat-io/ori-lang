---
plan: "diagnostic-tooling-improvements"
title: "Diagnostic Tooling Improvements: Exhaustive Implementation Plan"
status: not-started
references:
  - ".claude/rules/arc.md"
  - ".claude/rules/aot.md"
  - ".claude/rules/diagnostic.md"
  - ".claude/rules/compiler.md"
  - "diagnostics/README.md"
---

# Diagnostic Tooling Improvements: Exhaustive Implementation Plan

## Mission

Reduce debugging churn when tracking down hard AOT/LLVM/ARC/AIMS issues by fixing broken tools, enhancing existing scripts with missing diagnostic layers, creating new bisection tooling, expanding fixture coverage, and integrating diagnostic hints into the test harness. Sourced from a dual-source TPR review (Codex + Gemini, 15 findings, 6 thematic clusters) with architectural consensus from a subsequent /tp-help consultation.

## Mission Success Criteria

- [ ] `aims-compare.sh` removed; new `debug-release-compare.sh` functional and passing self-test
- [ ] `diagnose-aot.sh` runs codegen-audit, dumps ARC IR, supports `--release` and `--both-builds`, enables `ORI_VERIFY_ARC`
- [ ] `dual-exec-debug.sh` auto-dumps ARC IR on mismatch alongside LLVM IR
- [ ] `ORI_AUDIT_CODEGEN=1` emits per-block structured JSON; `rc-stats.sh --block-level` consumes it
- [ ] `bisect-passes.sh` can identify which AIMS pipeline phase introduced a failure for a given `.ori` file
- [ ] 7+ new diagnostic fixtures covering closures, iterators, nested structures, generics, trait dispatch, and failure modes
- [ ] `self-test.sh` exercises all new fixtures and scripts
- [ ] `test-all.sh` prints diagnostic command suggestions on LLVM/AOT test failures
- [ ] `check-debug-flags.sh` runs as part of `./test-all.sh` or `./clippy-all.sh`
- [ ] `ir-dump.sh` uses canonical `ORI_DUMP_AFTER_LLVM` instead of legacy `ORI_DEBUG_LLVM`
- [ ] `diagnostics/README.md` and `CLAUDE.md` updated to reflect all changes
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │        Diagnostic Toolkit            │
                    │  diagnostics/*.sh + fixtures/*.ori   │
                    └──────────┬──────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                     │
    ┌─────▼─────┐    ┌────────▼────────┐   ┌───────▼──────┐
    │  Single    │    │  Comparison     │   │  Bisection   │
    │  File      │    │  Tools          │   │  Tools       │
    │  Battery   │    │                 │   │              │
    ├────────────┤    ├─────────────────┤   ├──────────────┤
    │diagnose-aot│    │dual-exec-debug  │   │bisect-passes │
    │codegen-aud │    │dual-exec-verify │   │              │
    │rc-stats    │    │debug-release-cmp│   │              │
    │ir-dump     │    │ir-diff          │   │              │
    │arc-dump    │    │                 │   │              │
    └─────┬──────┘    └────────┬────────┘   └──────┬───────┘
          │                    │                    │
          └────────────────────┼────────────────────┘
                               │
              ┌────────────────▼────────────────────┐
              │     Compiler Diagnostic Surface      │
              │  ORI_AUDIT_CODEGEN (JSON per-block)  │
              │  ORI_DUMP_AFTER_ARC / _LLVM          │
              │  ORI_VERIFY_ARC / ORI_CHECK_LEAKS    │
              │  AIMS phase trace checkpoints         │
              └─────────────────────────────────────┘
```

## Design Principles

1. **Canonical diagnostic surfaces** — shell scripts consume structured compiler output, never re-derive facts the compiler already knows. `rc-stats.sh` regex-matching LLVM IR for RC ops is LEAK:scattered-knowledge; the compiler's `ORI_AUDIT_CODEGEN` pass is the SSOT for RC analysis. New features extend the compiler surface, then shell scripts render/query it.

2. **One tool, one question** — each diagnostic script answers ONE debugging question. `diagnose-aot.sh` = "is this AOT program healthy?" `dual-exec-debug.sh` = "do interpreter and AOT agree?" `bisect-passes.sh` = "which AIMS pass broke it?" No tool tries to do everything; the tools compose.

3. **Fixtures match failure surfaces** — diagnostic fixtures must exercise the code patterns that actually cause debugging churn: closures (capture RC), iterators (early-exit cleanup), nested structures (elem_dec_fn), and failure modes (leak, double-free). Three basic fixtures are insufficient.

## Section Dependency Graph

```
  01 (Remove aims-compare)  ──────┬──────────────────┐
                                  │                   │
  02 (Enhance diagnose-aot) ──────┘────────┐          │
  03 (Enhance dual-exec-debug) ────────────┤          │
  04 (Block-level RC stats — Rust+shell) ──┤          │
  05 (bisect-passes — Rust+shell) ─────────┤          │
  06 (Expand fixtures + self-test) ────────┤          │
                                           ▼          ▼
                                07 (Integration + polish + docs)
```

- Sections 01, 03-06 are **independent** — each can be implemented without the others.
- **Section 02 depends on Section 01** — 01.2 adds `find_ori_bin_profile()` and `require_both_builds()` to `_common.sh`, which 02.2 consumes for `--release` and `--both-builds` flags.
- Section 07 depends on ALL prior sections (updates docs, integrates into test-all.sh).
- Within sections 04 and 05, the Rust compiler change comes before the shell script update.

## Implementation Sequence

```
Phase 1 - Quick Wins (shell-only, no Rust changes)
  └─ 01: Remove aims-compare, create debug-release-compare
  └─ 02: Enhance diagnose-aot.sh
  └─ 03: Enhance dual-exec-debug.sh
  └─ 06: Expand fixtures + self-test

Phase 2 - Compiler Surface Extensions (targeted Rust changes)
  └─ 04: Block-level RC stats (ORI_AUDIT_CODEGEN JSON + rc-stats.sh)
  └─ 05: AIMS phase bisection (trace checkpoints + shell driver)

Phase 3 - Integration
  └─ 07: test-all.sh hints, check-debug-flags in CI, ir-dump DRIFT fix, docs
```

**Why this order:**
- Phase 1 delivers immediate value with zero Rust changes — these are the most common pain points.
- Phase 2 extends the compiler's diagnostic surface for block-level stats and phase bisection.
- Phase 3 ties everything together and updates documentation.

## Metrics (Current State)

| Component | Files | Lines |
|-----------|-------|-------|
| `diagnostics/*.sh` | 14 | ~4100 |
| `diagnostics/fixtures/*.ori` | 3 | ~30 |
| `diagnostics/README.md` | 1 | ~230 |
| `diagnostics/self-test.sh` | 1 | ~283 |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Remove aims-compare + create debug-release-compare | ~250 new, ~350 deleted | Medium | — |
| 02 Enhance diagnose-aot.sh | ~80 modified | Low | — |
| 03 Enhance dual-exec-debug.sh | ~30 modified | Low | — |
| 04 Block-level RC stats | ~150 Rust + ~60 shell | Medium | — |
| 05 bisect-passes.sh | ~50 Rust + ~200 shell | High | — |
| 06 Expand fixtures + self-test | ~200 new (.ori + self-test) | Low | — |
| 07 Integration + polish | ~100 modified | Low | 01-06 |
| **Total new** | **~1120** | | |
| **Total deleted** | **~350** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| `aims-compare.sh` dead — `--features aims` no longer exists | AIMS became default, feature flag removed | Section 01 | Not Started |
| `ir-dump.sh` uses legacy `ORI_DEBUG_LLVM` instead of `ORI_DUMP_AFTER_LLVM` | DRIFT from flag rename | Section 07 | Not Started |
| `rc-stats.sh` regex-matches RC ops (LEAK:scattered-knowledge) | Shell duplicates compiler knowledge | Section 04 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Remove aims-compare + create debug-release-compare | `section-01-aims-compare.md` | Not Started |
| 02 | Enhance diagnose-aot.sh | `section-02-diagnose-aot.md` | Not Started |
| 03 | Enhance dual-exec-debug.sh | `section-03-dual-exec-debug.md` | Not Started |
| 04 | Block-level RC Stats | `section-04-block-rc-stats.md` | Not Started |
| 05 | AIMS Pass Bisection | `section-05-bisect-passes.md` | Not Started |
| 06 | Expand Fixtures + Self-Test | `section-06-fixtures.md` | Not Started |
| 07 | Integration + Polish | `section-07-integration.md` | Not Started |
