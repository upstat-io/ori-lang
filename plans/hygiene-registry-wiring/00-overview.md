---
plan: "hygiene-registry-wiring"
title: "Hygiene Fixes for Registry Wiring: Implementation Plan"
status: done
references:
  - "plans/type_strategy_registry/"
---

# Hygiene Fixes for Registry Wiring: Implementation Plan

## Mission

Address implementation hygiene findings from the last 4 commits that implemented `ori_registry` and wired it into `ori_types`. Three crates were reviewed: `ori_registry`, `ori_types`, and `ori_llvm`. Findings range from correctness bugs (Result trait methods ignoring err_ty, WASM string param leaks) to code health (file size limits, import consistency, function bloat). One original finding (generate_js_wrapper line count) was corrected during review.

## Status: COMPLETE

All 6 sections (19 items) completed. 12,458 tests pass, 0 failures. `./clippy-all.sh` green. Release build succeeds.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Critical Fixes (ori_llvm) | `section-01-critical-fixes.md` | Done |
| 02 | Drift Fixes (ori_types) | `section-02-drift-fixes.md` | Done |
| 03 | Registry Cleanup (ori_registry) | `section-03-registry-cleanup.md` | Done |
| 04 | Type Checker Polish (ori_types) | `section-04-typeck-polish.md` | Done |
| 05 | LLVM Bloat Reduction (ori_llvm) | `section-05-llvm-bloat.md` | Done |
| 06 | Verification | `section-06-verification.md` | Done |
