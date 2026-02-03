# Code Review Remediation Plan — February 2, 2026

> **Comprehensive Technical Debt Remediation** — Addresses all 36 issues found in the compiler code review, organized by dependency order for systematic resolution.

## Source

This plan derives from the comprehensive code review conducted on 2026-02-02, which analyzed:
- Automated tooling: clippy, cargo audit, cargo outdated, cargo machete, cargo geiger, tokei
- Manual analysis: 10 categories across architecture, memory, performance, testing, style

---

## Design Philosophy

From `.claude/rules/compiler.md`:

1. **Fix issues encountered** — No "pre-existing" exceptions
2. **Do it properly** — Correct architecture over quick hacks
3. **No shortcuts** — Long-term maintainability over expedience
4. **Arena allocation** — ExprArena + ExprId, not Box<Expr>
5. **Interning** — Name for identifiers, not String
6. **Newtypes** — Type-safe IDs prevent mixing

---

## Section Overview

### Tier 0: CI Blockers (Section 01)

Must be fixed immediately — blocks all other work.

| Section | Focus | Priority |
|---------|-------|----------|
| 01 | Clippy Errors & CI | CRITICAL |

### Tier 1: Foundation Cleanup (Sections 02-03)

Removes dead code and establishes proper diagnostic infrastructure.

| Section | Focus | Priority |
|---------|-------|----------|
| 02 | Dependency Cleanup | CRITICAL |
| 03 | Diagnostic System (ori_macros) | CRITICAL |

### Tier 2: Memory & Safety (Sections 04-05)

Fixes memory issues and replaces panic! with proper error handling.

| Section | Focus | Priority |
|---------|-------|----------|
| 04 | Memory & Interning | CRITICAL |
| 05 | Panic Elimination | CRITICAL |

### Tier 3: Performance (Section 06)

Algorithmic improvements and hot path optimization.

| Section | Focus | Priority |
|---------|-------|----------|
| 06 | Performance Optimization | HIGH |

### Tier 4: Code Quality (Sections 07-08)

Large function extraction and pattern consolidation.

| Section | Focus | Priority |
|---------|-------|----------|
| 07 | Large Function Extraction | HIGH |
| 08 | Extractable Patterns | HIGH |

### Tier 5: Diagnostics Quality (Section 09)

User-facing error message improvements.

| Section | Focus | Priority |
|---------|-------|----------|
| 09 | Diagnostic Quality | HIGH |

### Tier 6: Testing & API (Sections 10-11)

Test coverage and API design improvements.

| Section | Focus | Priority |
|---------|-------|----------|
| 10 | Testing Improvements | MEDIUM |
| 11 | API Design | MEDIUM |

---

## Dependency Graph

```
Section 01 (Clippy) ──→ ALL OTHER SECTIONS
                         │
Section 02 (Deps) ───────┤
                         │
Section 03 (ori_macros) ─┼──→ Section 04 (Memory)
                         │         │
                         │         ▼
                         │    Section 05 (Panic)
                         │         │
                         ▼         ▼
                    Section 06 (Performance)
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
        Section 07   Section 08   Section 09
        (Functions)  (Patterns)   (Diagnostics)
              │          │          │
              └──────────┼──────────┘
                         ▼
                    Section 10 (Testing)
                         │
                         ▼
                    Section 11 (API)
```

**Key Dependencies:**
- Section 01 blocks everything (CI must pass)
- Section 03 (ori_macros) must complete before Section 04 (Memory) — the diagnostic migration changes Problem types
- Section 04 and 05 can run in parallel after 03
- Sections 07-09 can run in parallel after 06
- Section 10-11 are final cleanup

---

## Issue Summary

| Severity | Count | Sections |
|----------|-------|----------|
| **CRITICAL** | 15 | 01, 02, 03, 04, 05 |
| **HIGH** | 13 | 06, 07, 08, 09 |
| **MEDIUM** | 8 | 10, 11 |
| **Total** | 36 | |

---

## Success Criteria

A section is complete when:

1. **Fixed** — All tasks checked off
2. **Tested** — `./test-all` passes
3. **Linted** — `./clippy-all` passes
4. **Verified** — Manual verification of fix effectiveness

---

## Milestones

| Milestone | Tier | Sections | Exit Criteria |
|-----------|------|----------|---------------|
| **M0: CI Green** | 0 | 01 | Clippy passes, CI unblocked |
| **M1: Clean Foundation** | 1 | 02-03 | No unused deps, ori_macros adopted |
| **M2: Memory Safe** | 2 | 04-05 | No String in queries, no panic! on user input |
| **M3: Fast** | 3 | 06 | No O(n²), FxHashMap in hot paths |
| **M4: Clean Code** | 4 | 07-08 | Functions <50 lines, patterns extracted |
| **M5: Good Errors** | 5 | 09 | All errors have spans, suggestions |
| **M6: Well Tested** | 6 | 10-11 | Full coverage, clean API |

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `index.md` | Keyword search index |
| `00-overview.md` | This file — goals, tiers, dependencies |
| `section-01-clippy.md` | Immediate clippy fixes |
| `section-02-dependencies.md` | Unused dependency cleanup |
| `section-03-ori-macros.md` | Diagnostic system migration |
| `section-04-memory.md` | Memory and interning fixes |
| `section-05-panic.md` | Panic elimination |
| `section-06-performance.md` | Performance optimization |
| `section-07-functions.md` | Large function extraction |
| `section-08-patterns.md` | Extractable pattern consolidation |
| `section-09-diagnostics.md` | Diagnostic quality improvements |
| `section-10-testing.md` | Testing improvements |
| `section-11-api.md` | API design cleanup |

---

## Estimated Effort

| Section | Effort | Files Affected |
|---------|--------|----------------|
| 01 | 1 hour | 1 file |
| 02 | 30 min | 6 Cargo.toml |
| 03 | 2-3 days | ~15 files |
| 04 | 1 day | ~10 files |
| 05 | 4 hours | ~6 files |
| 06 | 1 day | ~15 files |
| 07 | 2 days | ~8 files |
| 08 | 2-3 days | ~10 files |
| 09 | 1 day | ~5 files |
| 10 | 1 day | ~15 files |
| 11 | 2 hours | ~3 files |
| **Total** | ~10-12 days | ~90 files |
