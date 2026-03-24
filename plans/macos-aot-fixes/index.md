---
reroute: true
name: "macOS AOT Fixes"
full_name: "macOS AOT Failure Investigation"
status: resolved
reviewed: false
order: 1
---

# macOS AOT Failure Investigation

**Goal:** Fix 2 macOS-only AOT test failures discovered in CI run #23420458239 on PR #88.

## Keyword Index

| Keyword | Section |
|---------|---------|
| merge_edge, scoped_cleanup, struct projection, branch RC | Section 01 |
| trampoline, map, str identity, SIGSEGV, elem_dec | Section 02 |
| CI timeout, Windows, cross-platform | Section 03 |

## Quick Reference

| Section | Title | Status | Severity |
|---------|-------|--------|----------|
| 01 | ARC merge-edge scoped cleanup (exit 1) | complete | Major |
| 02 | Trampoline map str identity SIGSEGV (exit -139) | complete | Critical |
| 03 | CI cross-platform timeout | complete | Minor |
