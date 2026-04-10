---
reroute: true
name: "Diag Tooling"
full_name: "Diagnostic Tooling Improvements"
status: active
order: 1
---

# Diagnostic Tooling Improvements Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Remove aims-compare + create debug-release-compare
**File:** `section-01-aims-compare.md` | **Status:** Complete

```
aims-compare.sh, aims-compare, aims-baseline.sh, aims-measure.sh
debug-release-compare, FastISel, release build, debug build
ORI_DUMP_AFTER_ARC, --features aims, DRIFT, dead script
```

---

### Section 02: Enhance diagnose-aot.sh
**File:** `section-02-diagnose-aot.md` | **Status:** Complete

```
diagnose-aot.sh, codegen-audit.sh, arc-dump.sh
ORI_AUDIT_CODEGEN, ORI_VERIFY_ARC, --release, --both-builds
ARC IR dump, codegen audit, all-in-one diagnostic
```

---

### Section 03: Enhance dual-exec-debug.sh
**File:** `section-03-dual-exec-debug.md` | **Status:** Complete

```
dual-exec-debug.sh, arc-dump.sh, mismatch
interpreter vs AOT, auto-diagnostics, ARC IR
backend comparison, codegen divergence
```

---

### Section 04: Block-level RC Stats
**File:** `section-04-block-rc-stats.md` | **Status:** Not Started

```
rc-stats.sh, --block-level, per-block, basic block
ORI_AUDIT_CODEGEN, structured JSON, rc_balance
ori_rc_alloc, ori_rc_inc, ori_rc_dec, ori_rc_free
LEAK:scattered-knowledge, SSOT, canonical surface
```

---

### Section 05: AIMS Pass Bisection
**File:** `section-05-bisect-passes.md` | **Status:** Not Started

```
bisect-passes.sh, AIMS pipeline, phase bisection
trace_phase_snapshot, realize_rc_reuse, merge_blocks
realize_annotations, compute_var_reprs, analyze_function
Swift sil-opt-pass-count, sequential checkpoint
```

---

### Section 06: Expand Fixtures + Self-Test
**File:** `section-06-fixtures.md` | **Status:** Not Started

```
fixtures, closure.ori, iterator_break.ori, nested_list.ori
generic.ori, trait_dispatch.ori, leak.ori, double_free.ori
self-test.sh, diagnostic regression, coverage
```

---

### Section 07: Integration + Polish
**File:** `section-07-integration.md` | **Status:** Not Started

```
test-all.sh, diagnostic hints, check-debug-flags.sh
ir-dump.sh, ORI_DEBUG_LLVM, ORI_DUMP_AFTER_LLVM, DRIFT
README.md, CLAUDE.md, documentation, CI integration
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Remove aims-compare + create debug-release-compare | `section-01-aims-compare.md` |
| 02 | Enhance diagnose-aot.sh | `section-02-diagnose-aot.md` |
| 03 | Enhance dual-exec-debug.sh | `section-03-dual-exec-debug.md` |
| 04 | Block-level RC Stats | `section-04-block-rc-stats.md` |
| 05 | AIMS Pass Bisection | `section-05-bisect-passes.md` |
| 06 | Expand Fixtures + Self-Test | `section-06-fixtures.md` |
| 07 | Integration + Polish | `section-07-integration.md` |
