---
bug: BUG-04-050
severity: low
title: "emit_unified.rs 540 lines (over 500-line limit)"
status: complete
goal: "Split emit_unified.rs so the file stays under the impl-hygiene.md 500-line limit without changing logic."
success_criteria:
  - "`emit_unified.rs` is under 500 lines"
  - "New `project_escape.rs` module extracted for the project-escape cluster"
  - "`cargo c` and `cargo t` still pass; zero logic changes"
subsystem: "compiler/ori_arc/src/aims/realize/"
found: "2026-04-09 (during BUG-04-047 fix — emit_unified.rs grew past 500 lines)"
source: "tpr-review (BUG-04-047 hygiene finding)"
third_party_review:
  status: resolved
  updated: 2026-04-09
---

# Fix: BUG-04-050 — emit_unified.rs over 500-line limit

## § 1. Investigation

**Root cause**: `emit_unified.rs` accumulated the Phase 2.1 "project escape" cluster (4 functions, 213 lines) during the BUG-04-047 fix. The file was already ~520 lines before that fix and grew marginally, pushing past the 500-line impl-hygiene limit.

**Blast radius**: Zero — purely mechanical file split with no logic changes.

**Files affected**:
- `compiler/ori_arc/src/aims/realize/emit_unified.rs` — shrink from 540 to ~327 lines
- `compiler/ori_arc/src/aims/realize/project_escape.rs` — new module (213 lines)
- `compiler/ori_arc/src/aims/realize/mod.rs` — add `mod project_escape;`

## § 1.5 Fix Consensus

**Round 1** (`/tmp/ori-tpr-ixJRCeOc`): Both Codex and Gemini agree the project-escape cluster is the correct extraction boundary. Both agree on `project_escape.rs` as the module name. Both confirm no cross-module callers. Codex suggests nesting under `emit_unified/` directory — rejected (over-engineered for current size). Consensus: proceed with sibling module.

## § 2. Fix Approach

Extract 4 functions from `emit_unified.rs` into new `realize/project_escape.rs`:
1. `emit_project_escape_incs` — Phase 2.1 entry point (`pub(super)`)
2. `build_var_to_parent` — helper (private)
3. `find_edge_decced_project_parents` — helper (private)
4. `follow_jump_chain` — helper (private)

Update `emit_unified.rs` to call `super::project_escape::emit_project_escape_incs`.

## § 3. TDD

N/A — purely mechanical file split. Verification: existing test suite (`./test-all.sh`) passes unchanged.

## § 4. Completion Checklist

- [x] Extract 4 functions to `project_escape.rs`
- [x] Update `emit_unified.rs` caller
- [x] Add `mod project_escape;` to `mod.rs`
- [x] `./test-all.sh` green (16,922 passed, 0 failed)
- [x] Bug entry updated to `[x]`
- [x] TPR + hygiene: skipped (low severity, mechanical file split — documented per scaling rules)
