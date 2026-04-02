---
section: "08"
title: "File Size Violations"
status: not-started
reviewed: false
goal: "Split all 58 production files exceeding the 500-line limit into focused submodules"
depends_on: ["01", "02", "04", "05", "06"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "Split ori_types Files (19 files)"
    status: not-started
  - id: "08.2"
    title: "Split ori_llvm/ori_arc Files (17 files)"
    status: not-started
  - id: "08.3"
    title: "Split ori_eval/ori_patterns Files (11 files)"
    status: not-started
  - id: "08.4"
    title: "Split Remaining Crate Files (11 files)"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: File Size Violations

**Status:** Not Started
**Goal:** Split all 58 production source files exceeding the 500-line limit into focused submodules. Each resulting file should be under 500 lines with a single clear responsibility.

**Context:** This section runs after Sections 01-06 because those DRY extractions will change file sizes — some files may drop below the limit naturally. Re-measure all files before splitting.

**Depends on:** Sections 01, 02, 04, 05, 06 (DRY extractions change file sizes).

---

## 08.1 Split ori_types Files (19 files)

**Before splitting, re-measure all files** — DRY work in Section 04 may have reduced some below the limit.

Worst offenders (pre-DRY measurements):
- `infer/expr/operators.rs` — 786 lines. Split: arithmetic, comparison, bitwise, pipe into submodules
- `registry/traits/mod.rs` — 765 lines. Split: trait lookup, impl resolution, coherence checking
- `unify/mod.rs` — 761 lines. Split: union-find engine vs structural unification
- `infer/expr/control_flow.rs` — 746 lines. Split: if/match vs loops (for/while/loop/break/continue)
- `output/mod.rs` — 694 lines
- `infer/mod.rs` — 690 lines. Split: InferEngine definition vs inference entry points
- `pool/mod.rs` — 661 lines
- `pool/format/mod.rs` — 632 lines
- `pool/accessors.rs` — 624 lines
- `check/signatures/mod.rs` — 568 lines
- `check/bodies/mod.rs` — 545 lines
- `type_error/context/mod.rs` — 527 lines
- `infer/expr/calls/method_call.rs` — 511 lines
- `pool/descriptor.rs` — 501 lines
- Plus 5 more borderline files (507-510 lines)

- [ ] Re-measure all files after Section 04 DRY work
- [ ] For each file still >500 lines: identify logical split points (use `scripts/extract_tests.py` for test extraction if needed)
- [ ] Split files, update `mod.rs` declarations, verify imports
- [ ] Use `#[cfg(test)] mod tests;` pattern for test files

---

## 08.2 Split ori_llvm/ori_arc Files (17 files)

Worst offenders (pre-DRY measurements):
- `runtime_functions.rs` — 1606 lines (claims data table exemption — review if exemption is valid)
- `derive_codegen/field_ops/thunks.rs` — 592 lines
- `arc_emitter/instr_dispatch.rs` — 578 lines
- `arc_emitter/builtins/collections/mod.rs` — 558 lines
- `ir_builder/cfg_simplify/mod.rs` — 555 lines
- `arc_emitter/builtins/iterator_consumers.rs` — 547 lines
- `function_compiler/mod.rs` — 544 lines
- `arc_emitter/builtins/debug_helpers.rs` — 534 lines
- `arc_emitter/terminators.rs` — 518 lines
- ori_arc: 8 files (state_map 646, aims_pipeline 590, rewrite 573, verify 559, lattice 552, interprocedural 533, lower/expr 531, extract 517, transfer 516)

- [ ] Re-measure after Section 05 DRY work
- [ ] For `runtime_functions.rs`: validate data table exemption — if truly pure static data, add explicit exemption comment; if it contains logic, split
- [ ] Split remaining files at logical boundaries
- [ ] Verify: `timeout 150 cargo test -p ori_llvm` and `timeout 150 cargo test -p ori_arc` pass

---

## 08.3 Split ori_eval/ori_patterns Files (11 files)

Worst offenders (pre-DRY measurements):
- `ori_patterns/src/errors/mod.rs` — 1018 lines (error types + factory functions)
- `ori_patterns/src/value/composite/mod.rs` — 735 lines
- `ori_patterns/src/lib.rs` — 668 lines
- `ori_eval/src/methods/collections.rs` — 631 lines
- `ori_eval/src/methods/variants.rs` — 586 lines
- `ori_patterns/src/value/mod.rs` — 516 lines
- `ori_eval/src/methods/units.rs` — 511 lines
- `ori_eval/src/interpreter/derived_methods.rs` — 504 lines
- Plus 3 more borderline files

- [ ] Re-measure after Section 02 DRY work
- [ ] `ori_patterns/src/errors/mod.rs` is the worst — split into error_types.rs + error_factories.rs
- [ ] Split remaining files at logical boundaries

---

## 08.4 Split Remaining Crate Files (11 files)

Covering ori_rt, oric, ori_diagnostic, ori_fmt, ori_parse, ori_ir:

- `ori_diagnostic/src/emitter/terminal/mod.rs` — 841 lines
- `ori_rt/src/iterator/consumers.rs` — 678 lines
- `oric/src/ir_dump/expr.rs` — 617 lines
- `ori_fmt/src/declarations/mod.rs` — 614 lines
- `oric/src/ast_dump/expr.rs` — 587 lines
- `oric/src/query/mod.rs` — 582 lines
- `oric/src/imports/mod.rs` — 580 lines
- `ori_fmt/src/width/mod.rs` — 579 lines
- `oric/src/commands/fmt/mod.rs` — 578 lines
- `ori_parse/src/outcome/mod.rs` — 697 lines
- `ori_parse/src/cursor/mod.rs` — 665 lines
- Plus 5 more files from ori_parse (match_patterns, function/mod)

- [ ] Re-measure after Section 01 and 06 DRY work
- [ ] `ori_parse/src/outcome/mod.rs` — extract macros to `outcome/macros.rs`
- [ ] `ori_parse/src/cursor/mod.rs` — extract identifier methods to `cursor/identifiers.rs`
- [ ] Split remaining files at logical boundaries

---

## 08.R Third Party Review Findings

- None.

---

## 08.N Completion Checklist

- [ ] Zero production files >500 lines (excluding validated data table exemptions)
- [ ] All splits use `mod` + submodule pattern (no re-export hacks)
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 08
- [ ] `/impl-hygiene-review last commit`
