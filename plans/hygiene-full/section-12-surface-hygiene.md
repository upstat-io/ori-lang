---
section: "12"
title: "Surface Hygiene"
status: in-progress
reviewed: true
goal: "Address surface hygiene: oversized files, missing SAFETY comments, missing module docs, dead code, large match arms, leaked pool IDs, cold-path allocations"
inspired_by:
  - "CLAUDE.md -- 500 line limit, unsafe SAFETY comments, module docs"
  - "Rust compiler -- data-driven dispatch for large match arms"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "12.1"
    title: "Oversized Files"
    status: not-started
  - id: "12.2"
    title: "Unsafe SAFETY Comments"
    status: not-started
  - id: "12.3"
    title: "Missing Module Docs"
    status: not-started
  - id: "12.4"
    title: "Dead Code Cleanup"
    status: not-started
  - id: "12.5"
    title: "Large Match Arms"
    status: not-started
  - id: "12.6"
    title: "Pool Var ID Leakage"
    status: not-started
  - id: "12.7"
    title: "Cold Path String Allocation"
    status: not-started
  - id: "12.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "12.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 12: Surface Hygiene

**Status:** Not Started
**Goal:** Address surface-level hygiene findings: split oversized files, add SAFETY comments to unsafe blocks, add module docs, remove dead code, convert large match arms to data-driven dispatch, fix pool var ID leakage through FunctionSig, and eliminate unnecessary cold-path string allocations.

**Context:** These are BLOAT findings that individually are minor but collectively degrade code quality. Per CLAUDE.md: 500 line limit for non-test files, `unsafe` blocks require `// SAFETY:` comments, module-level `//!` docs required.

**Depends on:** All prior sections (surface cleanup after architectural changes land -- file splits must happen after architectural changes to avoid merge conflicts).

**Test strategy:** Pure refactoring (file splits, comment additions, dead code removal). No behavioral changes.
- `timeout 150 ./test-all.sh` must pass unchanged after each subsection
- `./clippy-all.sh` must pass -- splitting files can surface new clippy warnings from changed visibility

---

## 12.1 Oversized Files

**File(s):** 69 non-test `.rs` files exceed 500 lines. Top offenders:
- `compiler/ori_types/src/type_error/check_error/mod.rs` -- 2210 lines
- `compiler/ori_parse/src/incremental/copier.rs` -- 1595 lines
- `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` -- 1562 lines
- `compiler/ori_parse/src/lib.rs` -- 1326 lines
- `compiler/ori_types/src/check/mod.rs` -- 1291 lines

Some of these are legitimately large (error variant definitions, runtime function declarations), but most should be split into submodules.

- [ ] **BLOAT** -- 69 non-test `.rs` files exceed the 500-line limit
- [ ] Prioritize splitting: files >1000 lines first, then >750, then >500
- [ ] For each oversized file, determine natural split boundaries (e.g., `check_error/mod.rs` can split by error category, `runtime_functions.rs` can split by function category)
- [ ] Use `scripts/extract_tests.py` for any that have inline tests

---

## 12.2 Unsafe SAFETY Comments

**File(s):** Multiple files in `compiler/ori_rt/src/`

The runtime crate (`ori_rt`) contains 20+ `unsafe` blocks, many without `// SAFETY:` comments explaining the safety invariants. While `ori_rt` is inherently unsafe (C-ABI functions, raw pointer manipulation), each `unsafe` block should document what invariants make the operation safe.

- [ ] **BLOAT** -- 20+ unsafe blocks in `ori_rt` without `// SAFETY:` comments
- [ ] Add `// SAFETY:` comments to each unsafe block explaining the invariant that makes the operation sound
- [ ] Focus on `rc/mod.rs`, `rc/allocate.rs`, `rc/list_rc.rs`, `rc/map_rc.rs`, `rc/elem_header.rs`, `io/mod.rs`, `io/jit_recovery.rs`

---

## 12.3 Missing Module Docs

**File(s):** Various crate roots and important modules

At least 4 important modules lack `//!` module-level documentation:
- Files without module docs that should have them (entry points, key abstractions)

- [ ] **BLOAT** -- Missing `//!` module docs on important modules
- [ ] Add module docs to undocumented crate roots and key module entry points

---

## 12.4 Dead Code Cleanup

**File(s):** Various files with `#[allow(dead_code)]` annotations

Dead code guarded by `#[allow(dead_code)]` without justification is technical debt. Each instance should either be used or removed.

- [x] **Verified: zero unjustified** — `grep '#[allow(dead_code)]'` on production code (excluding tests) returns 0 results. All dead_code annotations either have `reason` or are in `#[cfg(test)]` blocks. (2026-04-01)

---

## 12.5 Large Match Arms

**File(s):** `compiler/ori_types/src/type_error/check_error/mod.rs`

The type error module contains match arms with 100+ arms for error variant dispatch. Per CLAUDE.md: "105-arm match that should be data-driven. No 20+ arm match in single file; group related arms; 3+ similar arms -> extract helper."

- [ ] **BLOAT** -- 100+ arm match in type error dispatch that should be data-driven
- [ ] Convert to data-driven dispatch (e.g., lookup table or grouped helper functions) or split into category-based submodules

---

## 12.6 Pool Var ID Leakage

**File(s):** `compiler/ori_types/src/output/mod.rs`

Pool-internal variable IDs (`var_ids`) leak through `FunctionSig` (line 425 comment: "Pool var_ids for the scheme's quantified type variables"). These are implementation details of the type pool that should not be exposed in the public output API.

- [ ] **EXPOSURE** `output/mod.rs:425` -- Pool var_ids leaking through `FunctionSig`, exposing type pool internals to consumers
- [ ] Determine if var_ids are genuinely needed by consumers or if they can be hidden behind an opaque interface

---

## 12.7 Cold Path String Allocation

**File(s):** Various error/diagnostic paths

String allocations (`format!()`, `String::from()`) on cold error paths are generally acceptable, but any hot-path string allocation should use `write!()` to a buffer instead.

- [x] **Verified** — `format!()` calls in lexer/parser are in error factories and comment handling (cold paths). No hot-path string allocations found. (2026-04-01)

---

## 12.R Third Party Review Findings

- None.

---

## 12.N Completion Checklist

- [ ] Top 5 oversized files (>1000 lines) split into submodules
- [ ] All `unsafe` blocks in `ori_rt` have `// SAFETY:` comments
- [ ] Key modules have `//!` module docs
- [ ] Dead code removed or justified
- [ ] 100+ arm match converted to data-driven dispatch
- [ ] Pool var ID leakage assessed and resolved or documented
- [ ] No hot-path string allocations
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 12` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** Top 5 oversized files are under 1000 lines each. All `unsafe` blocks documented. `./test-all.sh` green.
