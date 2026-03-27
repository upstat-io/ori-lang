---
section: "02"
title: "Trampoline map str identity SIGSEGV"
status: complete
reviewed: false
goal: "Keep indirect sret closure calls ABI-correct across wrappers, trampolines, and SEH funclets for large return values such as `str`."
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-03-23
sections:
  - id: "02.1"
    title: "Indirect sret closure ABI"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Trampoline map str identity SIGSEGV

**Status:** Complete
**Goal:** Closure trampolines, wrappers, and `ApplyIndirect` must all agree on indirect-sret ABI details for fat returns, and that agreement must hold on ARM64 and Windows/MSVC funclet paths.

**Context:** The original SIGSEGV came from an ARM64 sret ABI mismatch in trampoline-mediated closure calls returning `str`. The fix corrected the trampoline, wrapper, and `ApplyIndirect` paths to use explicit sret for large returns. Third-party review then identified and drove resolution of six follow-up issues: wrapper `noundef` on hidden sret params, SEH funclet operand bundles for indirect-sret calls, missing direct regressions for the SEH+sret helper, file-size violations, weak semantic pins, and environment-dependent release coverage. All findings are resolved and pinned by regressions.

**Depends on:** None.

---

## 02.1 Indirect sret closure ABI

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs`, `compiler/ori_llvm/src/codegen/ir_builder/calls.rs`, `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs`

All ABI work is resolved. Wrapper declarations now preserve the repo's `sret` attribute contract (no `noundef` on hidden sret params), and indirect-sret closure calls inside SEH funclets use the required `"funclet"` operand bundle via `call_indirect_with_sret_and_funclet()`.

- [x] Skip `noundef` on hidden wrapper sret parameters while keeping the intended attributes on the remaining wrapper arguments.
  Done: Added `sret_offset` in `closures.rs` to skip param 0 when has_sret, matching `function_compiler.rs` pattern.
- [x] Route `ret_is_indirect && current_funclet_pad.is_some()` through a funclet-aware indirect-sret call helper.
  Done: Added `call_indirect_with_sret_and_funclet()` in `seh.rs`. Updated `apply.rs` to nest funclet check inside `ret_is_indirect` branch.
- [x] Add targeted regression coverage for a capturing closure wrapper returning `str` and for Windows/MSVC SEH closure calls returning a fat value.
  Done: Added `test_closure_wrapper_sret_no_noundef` in `ir_quality_attributes.rs` (semantic pin — fails if wrapper absent or sret has `noundef`). Added `seh_indirect_call_with_sret_and_funclet` in `ir_builder/tests.rs` (pins the SEH+sret helper with LLVM verification + attribute assertions).
- [x] Re-run the trampoline and IR-quality tests that cover the `str` indirect-sret path.
  Done: All 13,650 tests pass (0 failures), including 4 trampoline tests and 82 IR-quality tests.

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:407` — Closure wrappers incorrectly mark the hidden sret pointer `noundef`.
  Evidence: The wrapper declaration loop adds `noundef` to every parameter, including the explicit sret pointer introduced for fat returns. The existing IR-quality rule in `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs` says the sret pointer must not get `noundef`, and a fresh `ORI_DUMP_AFTER_LLVM=1` build of a capturing closure returning `str` emitted `_ori_partial_1(ptr noalias noundef sret(...), ...)`.
  Impact: The wrapper ABI now disagrees with the repository's own attribute contract for sret parameters, which risks misoptimization and leaves wrapper declarations outside current regression coverage.
  Required plan update: Skip `noundef` on wrapper sret params, then add an IR-quality regression that inspects a capturing closure wrapper returning `str` or another >16-byte aggregate.
  Resolved: Validated against codebase on 2026-03-23 — confirmed loop at line 407-415 adds `noundef` to all params including sret (unlike `function_compiler.rs` which correctly uses `sret_offset`). Implementation tasks integrated into 02.1.

- [x] `[TPR-02-002][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs:314` — Indirect sret closure calls inside SEH funclets bypass the required `"funclet"` operand bundle.
  Evidence: `emit_apply_indirect()` takes the `ret_is_indirect` path before checking `current_funclet_pad`, so the code always emits a plain `call_indirect_with_sret()` for fat returns. The emitter contract says all calls inside SEH pads must carry a funclet bundle, and the dedicated `call_indirect_with_funclet()` helper is documented as required for `ApplyIndirect` in that context.
  Impact: Windows MSVC `catch(expr:)` or cleanup paths that invoke closures returning fat values can emit invalid LLVM IR or incorrect unwind metadata, even though the non-SEH macOS tests pass.
  Required plan update: Add a funclet-aware indirect-sret call helper, route the `ret_is_indirect && current_funclet_pad.is_some()` path through it, and add Windows-targeted coverage for closure calls returning `str` inside `catch(expr:)`.
  Resolved: Validated against codebase on 2026-03-23 — confirmed `if ret_is_indirect` branch takes priority over funclet check, and no `call_indirect_with_sret_and_funclet` helper exists. Implementation tasks integrated into 02.1.

- [x] `[TPR-02-003][medium]` `compiler/ori_llvm/src/codegen/ir_builder/seh.rs:365` — The new SEH indirect-sret helper is not pinned by a direct regression, but the section is marked complete as if it were.
  Evidence: The fix adds `call_indirect_with_sret_and_funclet()`, and `emit_apply_indirect()` now routes the `ret_is_indirect && current_funclet_pad.is_some()` path through it. Fresh grep over `compiler/ori_llvm/src/codegen/ir_builder/tests.rs` and `compiler/ori_llvm/tests/aot/` found no test mentioning `call_indirect_with_sret_and_funclet`, `sret.*funclet`, or any SEH+sret combination; the only nearby low-level check is `seh_indirect_call_with_funclet`, which exercises funclet bundles without an sret parameter. The section text at `section-02-trampoline-map-str-identity.md:44-45` still claims the funclet path is covered by "Windows-targeted coverage or equivalent IR verification," but the current tree contains only the wrapper-attribute regression plus review-by-inference.
  Impact: A correctness-critical Windows/MSVC ABI path can regress without any test failure, and the plan overstates completion for `TPR-02-002`.
  Required plan update: Add a direct regression for the SEH + indirect-sret combination, preferably an `ir_builder` test that emits an indirect call inside a funclet with both the `"funclet"` bundle and param-0 `sret`/`noalias` attributes, or equivalent platform-independent structural verification.
  Resolved: Validated against codebase on 2026-03-23 — confirmed no test exercises the sret+funclet combination. Implementation task integrated into 02.N.

- [x] `[TPR-02-004][low]` `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:1` — The touched wrapper-emission file remains over the repository's 500-line limit.
  Evidence: Fresh `wc -l` reports `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs` at 540 lines after this change. `CLAUDE.md` and `.claude/rules/impl-hygiene.md` require splitting files before they exceed the limit, and this commit adds more logic to the already-oversized file.
  Impact: The ABI-sensitive closure path stays harder to review and maintain, which raises regression risk in a subsystem already carrying cross-platform calling-convention complexity.
  Required plan update: Split wrapper or env-drop generation into a sibling submodule before treating Section 02 as fully complete.
  Resolved: Validated against codebase on 2026-03-23 — confirmed 540 lines, over the 500-line limit. Implementation task integrated into 02.N.

- [x] `[TPR-02-005][medium]` `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs:1020` — The wrapper sret regression still passes without proving a wrapper was emitted.
  Evidence: `test_closure_wrapper_sret_no_noundef()` searches for a matching `_ori_partial_*` definition, but every assertion is guarded by `if let Some(decl) = wrapper_decl` and the file-level comment at lines 1044-1045 explicitly says the test becomes "a no-op" if no wrapper is found. That violates `CLAUDE.md` and `.claude/rules/tests.md`, which require bug fixes to land with a semantic pin rather than an optional check.
  Impact: Section 02 currently claims `TPR-02-001` is pinned by an IR-quality regression, but a future change that inlines away or renames the wrapper can make this test pass without checking the hidden-sret `noundef` contract at all.
  Required plan update: Strengthen the regression so it fails when the expected wrapper is absent, ideally by asserting that the chosen program shape emits at least one matching `_ori_partial_*` wrapper before checking the sret parameter attributes.
  Resolved: Fixed on 2026-03-23 — replaced `if let Some(decl)` guard with `.expect()` that panics when no wrapper is found. The test is now a true semantic pin: it fails if the wrapper is absent or if the sret param has `noundef`.

- [x] `[TPR-02-006][medium]` `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs:1014` — The wrapper sret regression still becomes a no-op in `--release`, so Section 02 overstates debug+release coverage.
  Evidence: Fresh `cargo test -p ori_llvm --release test_closure_wrapper_sret_no_noundef -- --nocapture` on 2026-03-23 passed only because the test hit `if !ir.contains("define ") { eprintln!("skipping: release binary does not emit IR"); return; }`, printing `skipping: release binary does not emit IR` and performing no wrapper assertions. `CLAUDE.md` and `.claude/rules/tests.md` require backend coverage across debug and release for LLVM fixes, but the current regression only pins the hidden-sret `noundef` contract in debug builds.
  Impact: A release-only regression in closure-wrapper sret attributes can slip through while the plan and checklist claim the fix is covered in both backend modes.
  Required plan update: Add a release-capable semantic pin for the wrapper attribute contract, either by making the release test path emit inspectable IR or by asserting the same property through a lower-level builder/codegen test that runs in `--release`.
  Resolved: Fixed on 2026-03-23 — added `ir_capture_binary()` to `util/aot.rs` that always prefers the debug `ori` binary for IR capture (release binary compiles out phase dumps via `dbg_set!`). `compile_and_capture_ir` now uses this function. All 82 IR-quality tests run real assertions in both `cargo test` and `cargo test --release` — zero skips. 13,651 tests pass.

- [x] `[TPR-02-007][medium]` `compiler/ori_llvm/tests/aot/util/aot.rs:335` — The release-path semantic pin still degrades to a skip when no debug `ori` binary exists, but Section 02 is marked complete as if release coverage were unconditional.
  Evidence: `ir_capture_binary()` prefers `target/debug/ori`, but it explicitly falls back to `target/release/ori` when no debug binary exists. In that fallback case `compile_and_capture_ir()` runs the release compiler with `ORI_DEBUG_LLVM=1`, and `test_closure_wrapper_sret_no_noundef()` still exits early on `if !ir.contains("define ") { eprintln!(\"skipping: release binary does not emit IR\"); return; }`. Fresh local verification on 2026-03-23 showed the test passes in `cargo test -p ori_llvm --release ...` only because a debug `ori` binary is already present in this workspace; the test itself does not enforce that precondition.
  Impact: The claimed debug+release semantic pin is environment-dependent. On a clean release-only workspace or CI shard without `target/debug/ori`, the regression silently skips again and the hidden-sret attribute contract is no longer pinned in the release lane.
  Required plan update: Make IR-capture tests fail fast when the required debug compiler is unavailable, or build/provision that binary as part of the release test path, so release coverage does not depend on leftover artifacts from a prior debug build.
  Resolved: Fixed on 2026-03-23 — removed the release binary fallback from `ir_capture_binary()`. The function now panics if no debug binary exists, making the requirement explicit and non-bypassable. Also removed all 72 `if !ir.contains("define ")` silent-skip guards across 8 test files (ir_quality_attributes, ir_quality_codegen, ir_quality_loops, ir_quality_block_merge, ir_quality_cfg_simplify, cli, generics, arc). All IR-quality tests now fail fast in any environment lacking the debug binary. 13,651 tests pass (debug + release).

- [x] `[TPR-02-008][low]` `plans/macos-aot-fixes/section-02-trampoline-map-str-identity.md:20` — Section 02’s narrative still describes the old “gaps remain” state even though the current tree and checklist claim those gaps are resolved.
  Evidence: The section frontmatter and checklist now claim `complete`/resolved status for the wrapper-sret and funclet work, and fresh verification in the current tree passed `test_closure_wrapper_sret_no_noundef` in both debug and release plus `seh_indirect_call_with_sret_and_funclet`. But the prose in `**Context:**`, `02.1`, and the `Done:` note for regression coverage still says the ABI work “is not yet closed” and that the funclet path is covered only by code review rather than by a direct regression.
  Impact: The plan disagrees with the repository state and with itself. Readers cannot tell whether Section 02 is actually still open for a code defect or merely carrying stale plan prose.
  Required plan update: Reconcile Section 02 and the overview narrative with the current implementation and verification evidence, or explicitly restate any remaining non-documentation gap if one still exists.
  Resolved: Reconciled on 2026-03-23 — updated Context, 02.1 description, and regression coverage Done note to reflect the resolved state with all regressions pinned.

---

## 02.N Completion Checklist

- [x] `TPR-02-001` remains resolved in code and is pinned by an IR-quality regression that fails when the wrapper is absent.
  Done: `test_closure_wrapper_sret_no_noundef` now uses `.expect()` instead of `if let Some` — the test fails if no `_ori_partial_*` wrapper with sret is emitted.
- [x] `TPR-02-002` remains resolved in code and is pinned by Windows-targeted coverage or equivalent IR verification.
  Done: `seh_indirect_call_with_sret_and_funclet` in `ir_builder/tests.rs` verifies LLVM IR has both `"funclet"` bundle and `sret`/`noalias` on param 0.
- [x] `TPR-02-003` is resolved by a direct regression for the SEH + indirect-sret helper path.
  Done: Same test (`seh_indirect_call_with_sret_and_funclet`) pins the `call_indirect_with_sret_and_funclet` helper with LLVM verification + attribute assertions.
- [x] `TPR-02-004` is resolved by splitting `closures.rs` back under the file-size limit.
  Done: Extracted `generate_closure_wrapper` to `closure_wrappers.rs` (239 lines). `closures.rs` now 315 lines.
- [x] `TPR-02-005` is resolved by making the wrapper-attribute regression fail if no wrapper is emitted.
  Done: Same change — `.expect()` replaces the `if let Some` guard. All 13,651 tests pass (debug + release).
- [x] `TPR-02-007` is resolved by making the release-path wrapper regression independent of pre-existing debug build artifacts.
  Done: Removed release fallback from `ir_capture_binary()` — panics if no debug binary. Removed all 72 silent-skip guards from IR tests. 13,651 tests pass (debug + release).
- [x] `TPR-02-008` is resolved by reconciling Section 02 and overview prose with the current implementation and verification state.
  Done: Updated Context, 02.1 description, and regression coverage note to reflect resolved state on 2026-03-23.
- [x] `cargo test -p ori_llvm test_trampoline_map_str_identity -- --nocapture` passes.
- [x] `cargo test -p ori_llvm test_trampoline_filter_str -- --nocapture` passes.
- [x] `cargo test -p ori_llvm test_trampoline_fold_str -- --nocapture` passes.
- [x] `cargo test -p ori_llvm test_trampoline_for_each_str -- --nocapture` passes.

**Exit Criteria:** All indirect-sret closure entry points agree on the ABI for large returns, wrapper IR no longer marks hidden sret pointers `noundef`, Windows/MSVC funclet calls carry the required bundle, and the relevant trampoline and IR-quality tests pass without platform-specific drift.
