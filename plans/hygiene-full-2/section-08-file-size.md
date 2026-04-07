---
section: "08"
title: "File and Function Size Violations"
status: in-progress
reviewed: false
goal: "Split all 69 production files exceeding the 500-line limit into focused submodules, and decompose 31+ functions exceeding the 100-line limit"
depends_on: ["01", "02", "03", "04", "05", "06", "07"]
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
    title: "Split Remaining Crate Files (27 files)"
    status: in-progress
  - id: "08.5"
    title: "Function Size Violations"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: File and Function Size Violations

**Status:** Not Started
**Goal:** Split all 69 production source files exceeding the 500-line limit into focused submodules, and decompose 31+ functions exceeding the 100-line limit into helper functions. Each resulting file should be under 500 lines with a single clear responsibility; each function under 100 lines (target <50).

**Context:** This section runs after Sections 01-07 because those DRY extractions and annotation removals will change file sizes -- some files may drop below the limit naturally. Re-measure all files before splitting.

**Depends on:** Sections 01-07 (DRY extractions and annotation removal change file sizes).

---

## 08.1 Split ori_types Files (19 files)

**Before splitting, re-measure all files** -- DRY work in Section 04 may have reduced some below the limit.

Worst offenders (pre-DRY measurements):
- `infer/expr/operators.rs` -- 799 lines. Split: arithmetic, comparison, bitwise, pipe into submodules
- `registry/traits/mod.rs` -- 765 lines. Split: trait lookup, impl resolution, coherence checking
- `unify/mod.rs` -- 761 lines. Split: union-find engine vs structural unification
- `infer/expr/control_flow.rs` -- 746 lines. Split: if/match vs loops (for/while/loop/break/continue)
- `output/mod.rs` -- 694 lines
- `infer/mod.rs` -- 690 lines. Split: InferEngine definition vs inference entry points
- `pool/mod.rs` -- 661 lines
- `pool/format/mod.rs` -- 632 lines
- `pool/accessors.rs` -- 624 lines
- `check/signatures/mod.rs` -- 568 lines
- `check/bodies/mod.rs` -- 545 lines
- `type_error/context/mod.rs` -- 527 lines
- `infer/expr/calls/method_call.rs` -- 511 lines
- `pool/descriptor.rs` -- 501 lines
- Plus 5 more borderline files (507-510 lines)

- [ ] Re-measure all files: `find compiler/ori_types -name "*.rs" -not -path "*/test*" | while read f; do lines=$(wc -l < "$f"); if [ "$lines" -gt 500 ]; then echo "$lines $f"; fi; done | sort -rn`
- [ ] For each file still >500 lines: identify logical split points -- look for section comments, distinct impl blocks, or groups of related functions
- [ ] Split files, update `mod.rs` declarations, verify imports
- [ ] Use `#[cfg(test)] mod tests;` pattern for test files
- [ ] `timeout 150 cargo test -p ori_types` after each split to catch broken imports immediately
- [ ] **Priority order:** Start with the worst offenders (operators.rs 799, registry/traits/mod.rs 765, unify/mod.rs 761) -- these have the clearest logical split points

---

## 08.2 Split ori_llvm/ori_arc Files (17 files)

Worst offenders (pre-DRY measurements):
- `runtime_functions.rs` -- 1606 lines (claims data table exemption -- review if exemption is valid)
- `arc_emitter/terminators.rs` -- 745 lines
- `function_compiler/define_phase.rs` -- 675 lines
- `derive_codegen/field_ops/thunks.rs` -- 592 lines
- `arc_emitter/instr_dispatch.rs` -- 587 lines
- `arc_emitter/builtins/collections/mod.rs` -- 566 lines
- `ir_builder/cfg_simplify/mod.rs` -- 555 lines
- `arc_emitter/builtins/iterator_consumers.rs` -- 547 lines
- `function_compiler/mod.rs` -- 544 lines
- `ir_builder/checked_ops.rs` -- 543 lines
- `arc_emitter/builtins/debug_helpers.rs` -- 534 lines
- ori_arc: 8 files (state_map 646, aims_pipeline 590, rewrite 573, verify 559, lattice 552, interprocedural 536, transfer 524, extract 517)

- [ ] Re-measure after Section 05 DRY work
- [ ] For `runtime_functions.rs` (1606 lines): validate data table exemption. Exemption criteria: file contains ONLY `const`/`static` declarations and their type definitions, with no function bodies containing logic (trivial constructors OK). If it contains runtime logic, dispatch tables, or non-trivial functions, split. Add explicit `// FILE SIZE EXEMPTION: pure static data table` comment at file top.
- [ ] Split remaining files at logical boundaries
- [ ] Verify: `timeout 150 cargo test -p ori_llvm` and `timeout 150 cargo test -p ori_arc` pass

---

## 08.3 Split ori_eval/ori_patterns Files (11 files)

Worst offenders (pre-DRY measurements):
- `ori_patterns/src/errors/mod.rs` -- 1018 lines (error types + factory functions)
- `ori_patterns/src/value/composite/mod.rs` -- 735 lines
- `ori_patterns/src/lib.rs` -- 668 lines
- `ori_eval/src/methods/collections.rs` -- 631 lines
- `ori_eval/src/methods/variants.rs` -- 586 lines
- `ori_patterns/src/value/mod.rs` -- 516 lines
- `ori_eval/src/methods/units.rs` -- 511 lines
- `ori_eval/src/interpreter/derived_methods.rs` -- 504 lines
- Plus 3 more borderline files

- [ ] Re-measure after Section 02 DRY work
- [ ] `ori_patterns/src/errors/mod.rs` (1018 lines) -- worst offender. Contains: type definitions (`ControlAction`, `EvalError`, `EvalErrorKind` enum with ~30 variants), factory functions (~40 `pub fn`), and `Display`/`From` impls. Split into:
  - `errors/types.rs` -- `ControlAction`, `EvalError`, `EvalErrorKind` enum + Display/From impls
  - `errors/factories.rs` -- all `pub fn` error factory functions
  - `errors/mod.rs` -- re-exports only (dispatch hub)
- [ ] `ori_patterns/src/value/composite/mod.rs` (735 lines) -- split composite value types by logical grouping (struct vs enum vs closure)
- [ ] Split remaining files at logical boundaries

---

## 08.4 Split Remaining Crate Files (27 files)

Covering ori_rt, oric, ori_diagnostic, ori_fmt, ori_parse, ori_ir:

- `ori_diagnostic/src/emitter/terminal/mod.rs` -- 841 lines
- `ori_parse/src/outcome/mod.rs` -- 697 lines
- `ori_rt/src/iterator/consumers.rs` -- 678 lines
- `ori_parse/src/cursor/mod.rs` -- 665 lines
- `oric/src/ir_dump/expr.rs` -- 617 lines
- `ori_fmt/src/declarations/mod.rs` -- 614 lines
- `ori_ir/src/arena/range_builders.rs` -- 608 lines
- `ori_parse/src/grammar/expr/patterns/match_patterns.rs` -- 595 lines
- `oric/src/ast_dump/expr.rs` -- 587 lines
- `oric/src/query/mod.rs` -- 582 lines
- `oric/src/imports/mod.rs` -- 580 lines
- `ori_fmt/src/width/mod.rs` -- 579 lines
- `oric/src/commands/fmt/mod.rs` -- 578 lines
- `oric/src/test/runner/mod.rs` -- 572 lines
- `ori_rt/src/rc/mod.rs` -- 554 lines
- `oric/src/problem/codegen/mod.rs` -- 551 lines
- `ori_diagnostic/src/diagnostic/mod.rs` -- 547 lines
- `oric/src/test/runner/llvm_backend.rs` -- 545 lines
- `ori_parse/src/grammar/item/function/mod.rs` -- 532 lines
- `ori_ir/src/arena/mod.rs` -- 530 lines
- `ori_fmt/src/formatter/inline.rs` -- 520 lines
- `ori_rt/src/string/methods/mod.rs` -- 517 lines
- `ori_fmt/src/spacing/category.rs` -- 516 lines
- `ori_rt/src/lib.rs` -- 512 lines
- `ori_fmt/src/formatter/broken.rs` -- 511 lines
- `ori_ir/src/ast/expr.rs` -- 510 lines
- `ori_ir/src/canon/expr.rs` -- 507 lines
- `ori_ir/src/canon/arena.rs` -- 507 lines

- [ ] Re-measure after Section 01 and 06 DRY work
- [ ] `ori_parse/src/outcome/mod.rs` -- extract macros to `outcome/macros.rs`
- [x] `ori_parse/src/cursor/mod.rs` -- extract identifier methods to `cursor/identifiers.rs` (done in Section 06, TPR-06-004)
- [ ] Split remaining files at logical boundaries

---

## 08.5 Function Size Violations (31+ functions >100 lines)

**Context:** The overview tracks "Functions >100 lines: 31+" as a metric, and CLAUDE.md mandates "functions < 100 lines (target < 50)." These functions violate the coding guidelines and indicate functions that mix concerns or implement complex algorithms without helper extraction.

- [ ] Audit all production files for functions >100 lines. Use a scope-aware tool rather than naive brace matching (which breaks on nested blocks). Approach: use `cargo clippy` with `too_many_lines` lint configured, or manually identify functions in the worst-offender files listed below.
- [ ] For each function >100 lines: extract helper functions, split into logical stages, or decompose into a struct with methods
- [ ] Priority targets (likely worst offenders based on file sizes): functions in `operators.rs`, `control_flow.rs`, `terminators.rs`, `runtime_functions.rs`, `consumers.rs`, `collections.rs`
- [ ] Verify: no production function exceeds 100 lines (excluding data tables and generated match arms with explicit exemption comments)
- [ ] Valid exemptions: pure data tables, exhaustive match arms over large enums where each arm is 1-3 lines. All exemptions require an explicit `// SIZE EXEMPTION:` comment.

---

## 08.R Third Party Review Findings

- None.

---

## 08.T Test Strategy

This section is pure file/function splitting with zero behavioral change. High volume of moves means high risk of broken imports.

- [ ] **Intermediate test gates are mandatory.** Run `timeout 150 cargo test -p <crate>` after EVERY file split within that crate. Do not batch multiple splits before testing.
- [ ] After completing each sub-section (08.1-08.5), run `timeout 150 ./test-all.sh`
- [ ] After all splits, verify debug AND release: `timeout 150 cargo b --release && timeout 150 ./test-all.sh`
- [ ] Structural verification after all splits:
  - `find compiler/ -name "*.rs" -not -path "*/test*" -not -path "*/bench*" -not -path "*/target/*" | while read f; do lines=$(wc -l < "$f"); if [ "$lines" -gt 500 ]; then echo "$lines $f"; fi; done | sort -rn` shows only exempted files

---

## 08.N Completion Checklist

- [ ] Zero production files >500 lines (excluding validated data table exemptions)
- [ ] Zero production functions >100 lines (excluding validated exemptions with `// SIZE EXEMPTION:` comments)
- [ ] All splits use `mod` + submodule pattern (no re-export hacks)
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] Debug AND release builds pass
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 08
- [ ] `/impl-hygiene-review`
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.
