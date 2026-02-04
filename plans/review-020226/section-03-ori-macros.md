---
section: "03"
title: Diagnostic System Migration
status: completed
priority: critical
goal: Add into_diagnostic() methods to Problem types, preparing for derive-based diagnostics
files:
  - compiler/oric/Cargo.toml
  - compiler/oric/src/problem/semantic.rs
  - compiler/oric/src/problem/typecheck.rs
  - compiler/oric/src/problem/parse.rs
  - compiler/oric/src/problem/mod.rs
  - compiler/oric/src/reporting/semantic.rs
  - compiler/oric/src/reporting/type_errors.rs
  - compiler/oric/src/reporting/parse.rs
---

# Section 03: Diagnostic System Migration

**Status:** ✅ Completed
**Priority:** CRITICAL — Consolidates diagnostic rendering with Problem types
**Goal:** Add `into_diagnostic()` methods to all Problem types, preparing for derive-based diagnostics

---

## Implementation Approach

Instead of creating 69+ separate structs with `#[derive(Diagnostic)]` (which would add significant complexity for marginal benefit), we:

1. **Added `into_diagnostic()` directly to Problem enums** — Same functionality as the derive would generate
2. **Updated Render trait to delegate** — `render()` now calls `into_diagnostic()`
3. **Added ori_macros dependency** — Ready for future derive-based migration if needed

This approach:
- Keeps diagnostic logic co-located with Problem definitions
- Maintains Salsa compatibility (enums unchanged)
- Reduces reporting module to thin wrappers
- Prepares for future derive-based migration

---

## 03.1 Enable ori_macros in oric

- [x] Add `ori_macros.workspace = true` to `compiler/oric/Cargo.toml`
- [x] Available for future derive-based diagnostics

---

## 03.2 Migrate SemanticProblem (22 variants)

Location: `compiler/oric/src/problem/semantic.rs`

- [x] Added `into_diagnostic()` method handling all 22 variants
- [x] Handles warnings vs errors (InfiniteRecursion, UnusedVariable, etc.)
- [x] Handles conditional suggestions (similar name suggestions)
- [x] Updated `reporting/semantic.rs` to delegate to `into_diagnostic()`

---

## 03.3 Migrate TypeProblem (25 variants)

Location: `compiler/oric/src/problem/typecheck.rs`

- [x] Added `into_diagnostic()` method handling all 25 variants
- [x] Imports `suggest_similar` for field/method suggestions
- [x] Updated `reporting/type_errors.rs` to delegate to `into_diagnostic()`

---

## 03.4 Migrate ParseProblem (19 variants)

Location: `compiler/oric/src/problem/parse.rs`

- [x] Added `into_diagnostic()` method handling all 19 variants
- [x] Imports `suggest_similar` for pattern arg suggestions
- [x] Updated `reporting/parse.rs` to delegate to `into_diagnostic()`

---

## 03.5 Update Unified Problem Enum

Location: `compiler/oric/src/problem/mod.rs`

- [x] Added `into_diagnostic()` method that delegates to variant-specific methods

---

## 03.6 Verification

- [x] `./clippy-all.sh` passes
- [x] `./test-all.sh` passes (6,367 tests, 0 failures)
- [x] Error output unchanged (same rendering logic)
- [x] Render trait now thin wrappers delegating to `into_diagnostic()`

---

## Architecture After Migration

```
problem/semantic.rs     → into_diagnostic() (22 variants)
problem/typecheck.rs    → into_diagnostic() (25 variants)
problem/parse.rs        → into_diagnostic() (19 variants)
problem/mod.rs          → into_diagnostic() (delegates)

reporting/semantic.rs   → render() { self.into_diagnostic() }
reporting/type_errors.rs → render() { self.into_diagnostic() }
reporting/parse.rs      → render() { self.into_diagnostic() }
```

Benefits:
- Diagnostic logic lives with Problem types (data + presentation co-located)
- Reporting module is now thin delegation layer
- Ready for future removal of Render trait
- ori_macros available for future derive-based approach

---

## 03.N Completion Checklist

- [x] ori_macros integrated in oric
- [x] All 66 Problem variants have `into_diagnostic()` methods
- [x] Rendering logic moved from reporting/ to problem/ files
- [x] Render impls reduced to one-line delegations
- [x] Error output unchanged
- [x] `./test-all.sh` passes

**Exit Criteria:** ✅ All Problem types have `into_diagnostic()`, Render delegates to it
