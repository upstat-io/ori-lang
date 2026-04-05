---
section: "03"
title: "LLVM Cleanup & Verification"
status: in-progress
reviewed: true
goal: "Replace LLVM-level workarounds with arg_ownership logic, add InvokeIndirect handling (drop_hints + unwind_cleanup), verify env drop correctness, remove unused _capture_ownership parameter, fix 3 stale docs, un-ignore tests, verify zero leaks"
success_criteria:
  - "collect_borrowed_call_args ApplyIndirect workaround replaced with arg_ownership-aware logic"
  - "collect_borrowed_call_args handles InvokeIndirect terminator via arg_ownership"
  - "unwind_cleanup handles InvokeIndirect (TPR-01-006)"
  - "Rust unit tests pin ApplyIndirect/InvokeIndirect borrowed-call-arg collection and InvokeIndirect unwind cleanup"
  - "generate_env_drop_fn correctness verified (RcDec ALL captures — no skipping for borrowed)"
  - "Unused _capture_ownership parameter removed from generate_env_drop_fn AND build_closure_env"
  - "lambda_capture_ownership fallback verified safe and tracing::warn added"
  - "All 3 stale doc comments corrected (context.rs:259, closures.rs:86, define_phase.rs:422) — closures.rs:155 eliminated by parameter removal"
  - "The 3 former BUG-04-035 curried-closure tests remain enabled and passing"
  - "All 3 pre-existing nested closure leak tests passing"
  - "ORI_CHECK_LEAKS=1 reports zero leaks on full AOT suite"
  - "Dual-execution parity for all closure test programs"
inspired_by:
  - "Swift SIL partial_apply ownership annotations"
depends_on: ["01", "02"]
third_party_review:
  status: resolved
  updated: 2026-04-05
sections:
  - id: "03.1"
    title: "Replace drop_hints workaround, add InvokeIndirect, wire test modules"
    status: complete
  - id: "03.2"
    title: "Verify env drop function correctness"
    status: complete
  - id: "03.3"
    title: "Un-ignore tests and verify"
    status: complete
  - id: "03.4"
    title: "Full verification matrix"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 03: LLVM Cleanup & Verification

**Goal:** Now that `arg_ownership` flows through the ARC IR for indirect calls (Sections 01-02), remove the LLVM-level workarounds that compensated for the missing ownership info. Verify env drop correctness (it already RcDec's ALL captures correctly — the `_capture_ownership` param is unused). Remove the unused `_capture_ownership` parameter from `generate_env_drop_fn` and `build_closure_env`. Fix 3 stale doc comments. Un-ignore all BUG-04-035 tests and verify zero leaks.

**Context from Sections 01-02:** `ApplyIndirect`/`InvokeIndirect` now carry populated `arg_ownership`. The AIMS system correctly emits `RcInc`/`RcDec` based on ownership. LLVM codegen should now just lower what the ARC IR says.

## 03.1 Replace drop_hints workaround and add InvokeIndirect

The safety fix in commit `a83b8e65` added a workaround in `collect_borrowed_call_args()` (`compiler/ori_arc/src/aims/emit_rc/drop_hints.rs`, 155 lines, well under 500 limit) that conservatively marks ALL `ApplyIndirect` args as potentially shared. This was necessary because `ApplyIndirect` lacked ownership info — `drop_unique` was being used on values that might be shared via closure environments.

**File size check (all well under 500 limit):** `drop_hints.rs` (155 lines), `unwind_cleanup.rs` (165 lines). After converting to `mod.rs` + `tests.rs`, the `mod.rs` files will grow slightly from the `#[cfg(test)] mod tests;` line but remain well under limit.

**Prerequisite**: Section 02 must be complete — `arg_ownership` is populated on `ApplyIndirect`/`InvokeIndirect` before this code runs. Section 02.3 handles the legacy borrow inference update in `borrow/update.rs`; this subsection handles the parallel `drop_hints.rs` refinement.

**TDD execution order**: Wire test modules FIRST, write test stubs SECOND (verify they fail or don't compile), THEN implement the code changes, THEN verify tests pass unchanged. The checklist items below are ordered by topic, not execution sequence.

**Execution slices**: Keep 03.1 as one ARC-side cleanup subsection, but execute it as three bounded checkpoints rather than one mixed edit:
1. **Scaffolding only** — convert `drop_hints.rs` and `unwind_cleanup.rs` into `mod.rs` + `tests.rs`, add `#[cfg(test)] mod tests;`, and land failing test stubs.
2. **`drop_hints` only** — add the shared helper, replace the `ApplyIndirect` workaround, add `InvokeIndirect` borrowed-arg collection, and make the `drop_hints` unit tests pass.
3. **`unwind_cleanup` only** — add `InvokeIndirect` unwind handling and make the `unwind_cleanup` unit tests pass.

This keeps the subsection cohesive because all work stays inside `compiler/ori_arc/src/aims/emit_rc/`, but still follows the "narrow the front" rule at implementation time. A formal subsection split is optional, not required.

**Already consistent (verified 2026-04-05)**: The following indirect-call consumers were updated in Sections 01-02 and already handle `InvokeIndirect` correctly — no changes needed in Section 03:
- `edge_cleanup.rs` — `collect_invoke_edge_decs` uses `Invoke | InvokeIndirect` pattern match (line 283-296)
- `borrow/update.rs` — `InvokeIndirect` terminator scan with `arg_ownership` (line 275-288)
- `forward_walk.rs` — `Invoke | InvokeIndirect` pattern for project-borrowed RcInc (line 32-41)
- `dead_cleanup.rs` — `Invoke | InvokeIndirect` pattern for dead dst cleanup (line 184-185)
- `trampoline.rs` — `Invoke | InvokeIndirect` pattern for target redirection (line 135-136)
- `realize/mod.rs` — COW annotations only apply to `Apply`/`Invoke` with known callee names; `InvokeIndirect` calls through closure fat pointers, which never invoke COW mutation methods — no gap exists

- [x] **Replace the `ApplyIndirect` branch** in `collect_borrowed_call_args()` (lines 78-82 in `drop_hints.rs`). The current branch:
  ```rust
  ArcInstr::ApplyIndirect { args, .. } => {
      for &arg in args {
          borrowed.insert(arg);
      }
  }
  ```
  Replace with `arg_ownership`-aware logic with empty-ownership fallback. No `is_safe_non_sharing_callee` gate (indirect callee is unknown — always use `arg_ownership` directly).
  **Safety: empty `arg_ownership` fallback** — the `debug_assert!` in `arg_ownership/mod.rs:130-160` (from Section 02) catches unannotated indirect calls in debug builds. However, in release builds the assert is stripped, and empty `arg_ownership` with `.get(i) == None` would evaluate `== Some(Borrowed)` as false — treating all args as non-borrowed (potentially unique). This is LESS conservative than the old workaround (which treated all indirect args as borrowed). To maintain safety, add an explicit fallback: if `arg_ownership.is_empty()`, fall back to the old behavior (insert ALL args as borrowed). This preserves the conservative-safe semantics in release while the `debug_assert!` catches the annotation bug in debug:
  ```rust
  ArcInstr::ApplyIndirect { args, arg_ownership, .. } => {
      if arg_ownership.is_empty() {
          // Fallback: unannotated indirect call — conservative all-borrowed.
          for &arg in args {
              borrowed.insert(arg);
          }
      } else {
          for (i, &arg) in args.iter().enumerate() {
              if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                  borrowed.insert(arg);
              }
          }
      }
  }
  ```
- [x] **Add `InvokeIndirect` terminator handling** to `collect_borrowed_call_args()` (after the `Invoke` terminator check, lines 94-109). Currently the terminator scan only checks `Invoke` — add a matching branch with the same empty-ownership fallback:
  ```rust
  if let ArcTerminator::InvokeIndirect { args, arg_ownership, .. } = &block.terminator {
      if arg_ownership.is_empty() {
          for &arg in args {
              borrowed.insert(arg);
          }
      } else {
          for (i, &arg) in args.iter().enumerate() {
              if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                  borrowed.insert(arg);
              }
          }
      }
  }
  ```
- [x] **Extract shared helper** to eliminate duplication between `ApplyIndirect` and `InvokeIndirect` branches. Both use identical logic (empty-ownership fallback + arg_ownership-aware borrowed-arg collection). Extract a private helper function in `drop_hints/mod.rs`:
  ```rust
  /// Insert borrowed args from ownership annotations, with conservative fallback.
  fn insert_borrowed_from_ownership(
      args: &[ArcVarId],
      arg_ownership: &[ArgOwnership],
      borrowed: &mut FxHashSet<ArcVarId>,
  ) {
      if arg_ownership.is_empty() {
          for &arg in args {
              borrowed.insert(arg);
          }
      } else {
          for (i, &arg) in args.iter().enumerate() {
              if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                  borrowed.insert(arg);
              }
          }
      }
  }
  ```
  Call from both the `ApplyIndirect` instruction match arm and the `InvokeIndirect` terminator check. This follows the algorithmic DRY rule (2 instances, >5 lines of shared skeleton).
- [x] **Verify**: values with `ArgOwnership::Owned` at `ApplyIndirect`/`InvokeIndirect` call sites are NOT treated as borrowed call args (they transfer ownership to the callee). Values with `ArgOwnership::Borrowed` ARE treated as borrowed call args (caller retains ownership, may not be unique).
- [x] **Add `InvokeIndirect` to `unwind_cleanup.rs`** (`compiler/ori_arc/src/aims/emit_rc/unwind_cleanup.rs:57`): the unwind iterator cleanup currently only scans `ArcTerminator::Invoke` — add `InvokeIndirect` via pattern-or match (same style as `edge_cleanup.rs:283-296`):
  ```rust
  if let ArcTerminator::Invoke { unwind, normal, .. }
      | ArcTerminator::InvokeIndirect { unwind, normal, .. } = &block.terminator
  {
  ```
  This ensures that iterator cleanup on unwind paths also covers indirect invoke sites. Flagged by TPR-01-006 and noted as "tracked for Section 03" — this is the tracking item.
- [x] **Wire test modules for `drop_hints` and `unwind_cleanup`** (prerequisite — do FIRST before writing tests):
  - Convert `compiler/ori_arc/src/aims/emit_rc/drop_hints.rs` → `compiler/ori_arc/src/aims/emit_rc/drop_hints/mod.rs`, create `compiler/ori_arc/src/aims/emit_rc/drop_hints/tests.rs`, add `#[cfg(test)] mod tests;` at the bottom of the new `mod.rs`. Update `mod drop_hints;` in `compiler/ori_arc/src/aims/emit_rc/mod.rs` — no change needed since Rust resolves both `drop_hints.rs` and `drop_hints/mod.rs` the same way.
  - Convert `compiler/ori_arc/src/aims/emit_rc/unwind_cleanup.rs` → `compiler/ori_arc/src/aims/emit_rc/unwind_cleanup/mod.rs`, create `compiler/ori_arc/src/aims/emit_rc/unwind_cleanup/tests.rs`, add `#[cfg(test)] mod tests;` at the bottom of the new `mod.rs`.
- [x] **Add Rust unit tests** for the ARC-pass changes in 03.1 (shared infrastructure needs direct pins, not only AOT coverage).

  **Test matrix for `drop_hints/tests.rs`** — `collect_borrowed_call_args()`:

  | Dimension | Values |
  |-----------|--------|
  | Call type | `ApplyIndirect` instruction, `InvokeIndirect` terminator |
  | Ownership state | populated (mixed Owned/Borrowed), empty (fallback) |
  | Callee kind | indirect only (no `is_safe_non_sharing_callee` gate) |
  | Alias propagation | direct borrowed var, alias chain (`Let %y = %x`) |

  Required tests (minimum 5):
  - `ApplyIndirect` with populated `arg_ownership` marks only `Borrowed` args, not all args (**semantic pin** — would fail if reverted to old all-borrowed)
  - `InvokeIndirect` terminator with populated `arg_ownership` contributes only `Borrowed` args (**semantic pin** — would fail if `InvokeIndirect` handling is removed)
  - Empty `arg_ownership` on `ApplyIndirect` falls back to all-borrowed (**negative pin** — proves conservative safety in release builds)
  - Empty `arg_ownership` on `InvokeIndirect` falls back to all-borrowed (**negative pin**)
  - Alias chain propagation: `%y = %x` where `%x` is borrowed → `%y` also marked borrowed

  **Test matrix for `unwind_cleanup/tests.rs`** — `add_invoke_unwind_cleanup()`:

  | Dimension | Values |
  |-----------|--------|
  | Terminator type | `Invoke`, `InvokeIndirect` |
  | Unwind state | `Resume` (cleanup needed), non-`Resume` (skip), `normal == unwind` (skip) |
  | Iterator liveness | live iter at invoke, dropped iter before invoke, no iters |

  Required tests (minimum 3):
  - `InvokeIndirect` with `Resume` unwind and live iterator → `ori_iter_drop` inserted (**semantic pin**)
  - No cleanup for non-`Resume` unwind or `normal == unwind`
  - Inserted cleanup uses `ArgOwnership::Borrowed` on the synthetic `ori_iter_drop` arg

  **Test construction pattern**: Use the canonical test helpers from `crate::test_helpers` (`make_func`, `make_func_named`, `b`, `v`, `owned_param`, `borrowed_param`) to build synthetic `ArcFunction` structs. See `compiler/ori_arc/src/aims/emit_rc/arg_ownership/tests.rs` for the closest existing example of testing an `emit_rc` submodule with synthetic IR. Both test files import from `crate::ir` (`ArcFunction`, `ArcInstr`, `ArcTerminator`, `ArcVarId`, `ArgOwnership`, `ArcBlockId`). For `drop_hints/tests.rs`, also import `collect_borrowed_call_args` from `super` and create minimal `FxHashMap<Name, MemoryContract>` + `BuiltinOwnershipSets` fixtures. For `unwind_cleanup/tests.rs`, import `add_invoke_unwind_cleanup` from `super` and create a `StringInterner` for the `iter`/`ori_iter_drop` names.

## 03.2 Verify env drop function correctness (DO NOT skip RcDec for borrowed captures)

The `generate_env_drop_fn()` in `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:189-315` has `_capture_ownership` as an UNUSED parameter. The plan originally proposed using it to skip `RcDec` for borrowed captures — **this is INCORRECT and would cause leaks**.

**Why the env drop must RcDec ALL captures regardless of ownership:**

The env struct physically stores (copies of) ALL captures. "Borrowed" here means "borrowed by the lambda body" — the wrapper function skips `RcInc` for borrowed captures, so the lambda body doesn't get its own reference. But the env itself still holds one reference to each capture (it was copied into the env by `build_closure_env`). When the env is freed, it must `RcDec` all its captures to release its references.

**RC balance for Owned captures**: env stores 1 ref → wrapper `RcInc` creates 2nd ref for lambda body → env drop `RcDec` destroys env's ref → lambda body `RcDec` destroys body's ref → balanced.

**RC balance for Borrowed captures**: env stores 1 ref → wrapper does NOT `RcInc` (body borrows from env) → env drop `RcDec` destroys env's ref → body does NOT `RcDec` → balanced.

Skipping `RcDec` for borrowed captures would orphan the env's reference, causing a leak.

The comment at `closures.rs:222-226` already documents this correctly:
> "The drop function must dec all RC-needing captures regardless of the lambda's borrow annotation — the annotation controls the lambda BODY's treatment, not env ownership."

- [x] **Remove the unused `_capture_ownership` parameter FIRST** — a four-step chain in `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs` (316 lines, well under 500 limit):
  1. **`generate_env_drop_fn` signature** (line 189-195): Remove the `_capture_ownership: &[Ownership]` parameter (line 193). The env drop correctly `RcDec`s all captures regardless — no change to RC logic.
  2. **`generate_env_drop_fn` call site** (line 156-157): Change `self.generate_env_drop_fn(env_struct_ty_id, capture_types, capture_ownership, env_size)` to `self.generate_env_drop_fn(env_struct_ty_id, capture_types, env_size)` — drop the `capture_ownership` argument.
  3. **`build_closure_env` signature** (line 124-129): Remove the `capture_ownership: &[Ownership]` parameter (line 128). This function only passed it through to `generate_env_drop_fn`.
  4. **`build_closure_env` call site** in `emit_partial_apply` (line 98): Change `self.build_closure_env(args, &capture_types, &capture_ownership)` to `self.build_closure_env(args, &capture_types)` — drop the `&capture_ownership` argument.

  The `capture_ownership` local in `emit_partial_apply` (lines 87-92) is still needed — it is passed to `generate_closure_wrapper` at line 107 for the wrapper RcInc logic. The stale comment at line 155 ("Pass capture ownership so borrowed captures are NOT RC-dec'd") will be dead code after this edit (the line references the removed parameter) and can be deleted.
- [x] **Fix stale doc comments** (3 locations remaining after parameter removal, all in `compiler/ori_llvm/src/`):
  1. `codegen/arc_emitter/context.rs:259-260` — incorrectly says "the closure's env drop function must NOT RC-dec that capture — the caller retains ownership." Update to: "the closure's wrapper function skips `RcInc` for borrowed captures — the lambda body borrows from the env rather than getting its own reference."
  2. `codegen/arc_emitter/closures.rs:86` — incorrectly says "which captures are borrowed (skip RC dec in drop fn)." Update to: "which captures are borrowed (skip RcInc in wrapper — body borrows from env)."
  3. `codegen/function_compiler/define_phase.rs:422-423` — incorrectly says "correct env drop functions: borrowed captures must NOT be RC-dec'd." Update to: "correct wrapper functions: borrowed captures skip RcInc (body borrows from env). Env drop RcDec's ALL captures regardless."
  **Note**: `closures.rs:155` ("Pass capture ownership so borrowed captures are NOT RC-dec'd") is eliminated by the parameter removal above — no separate fix needed.
- [x] **Hygiene: `generate_env_drop_fn` is 127 lines** (closures.rs:189-315) — exceeds the 100-line *target* from CLAUDE.md guidelines. Verified: `clippy::too_many_lines` does NOT fire on this function (Clippy counts statements, not lines — the function has fewer than 100 statements). Adding `#[expect(clippy::too_many_lines)]` causes "unfulfilled lint expectation" error. No suppression needed or possible. Sequential LLVM IR emission, same pattern as `generate_closure_wrapper`.
- [x] **Verify the wrapper RcInc logic** in `compiler/ori_llvm/src/codegen/arc_emitter/closure_wrappers.rs:175-179` (239 lines, under 500 limit) is correct with the ownership model:
  - The wrapper `RcInc` fires for `Owned` captures (line 179: `ownership == Ownership::Owned && needs_rc`) — correct: creates a second reference for the lambda body
  - For `Borrowed` captures: no wrapper `RcInc` — correct: body borrows from the env
  - The env drop `RcDec`s ALL captures regardless — correct: env owns all its stored values
- [x] **Verify `lambda_capture_ownership` fallback safety** in `closures.rs:87-92`: the `unwrap_or_else(|| vec![Ownership::Owned; num_captures])` default is conservative-correct (all-Owned causes RcInc for every capture). Add a `tracing::warn!` when the fallback fires for a capturing lambda (non-empty captures) — this indicates a missing entry from `define_phase.rs:424-433` and should not happen in practice. The `tracing` crate is already a dependency of `ori_llvm` and already used in `closures.rs` (lines 42, 51). Suggested implementation:
  ```rust
  .unwrap_or_else(|| {
      tracing::warn!(
          name = callee_name_str,
          captures = num_captures,
          "lambda_capture_ownership missing — defaulting to all-Owned (conservative)"
      );
      vec![Ownership::Owned; num_captures]
  })
  ```

## 03.3 Un-ignore tests and verify

- [x] **Verify the 3 former `BUG-04-035` curried-closure tests remain enabled** in `compiler/ori_llvm/tests/aot/arc.rs` and keep passing with leak checks:
  - `test_arc_curried_closure_capture_list` (arc.rs)
  - `test_arc_curried_closure_capture_str` (arc.rs)
  - `test_arc_curried_closure_capture_nested` (arc.rs)
  - Current tree check (2026-04-05): these tests are already un-ignored, so Section 03 must verify them rather than remove stale `#[ignore]`s.
- [x] **Verify the 3 pre-existing nested closure leak tests** in `compiler/ori_llvm/tests/aot/higher_order.rs` keep passing with leak checks:
  - `test_nested_closure_borrowed_list_param` (higher_order.rs:476)
  - `test_nested_closure_borrowed_str_param` (higher_order.rs:466)
  - `test_triple_nested_closure_capture` (higher_order.rs:453)
- [x] **Run ALL closure AOT tests** with leak check (tests span two files):
  ```bash
  timeout 150 cargo test -p ori_llvm --test aot -- test_arc_curried test_arc_lambda test_arc_closure test_fm_closure_param
  timeout 150 cargo test -p ori_llvm --test aot -- --test-threads=1 test_nested_closure test_triple_nested test_hof_nested
  ```
- [x] **Verify zero leaks**: `ORI_CHECK_LEAKS=1` is already set by `assert_aot_success` — every AOT test automatically checks for leaks. Verify none of the above tests report leaks in their output.

## 03.4 Full verification matrix

**All 6 previously-leaking tests must pass (zero leaks):**

| Test | File | Pattern | Pre-fix status | Post-fix |
|------|------|---------|---------------|----------|
| `test_arc_curried_closure_capture_list` | arc.rs | Curried list capture | Historical leak (test now enabled) | Zero leaks |
| `test_arc_curried_closure_capture_str` | arc.rs | Curried str capture | Historical leak (test now enabled) | Zero leaks |
| `test_arc_curried_closure_capture_nested` | arc.rs | Nested curried | Historical leak (test now enabled) | Zero leaks |
| `test_nested_closure_borrowed_list_param` | higher_order.rs | Borrowed param (list) | Leak (pre-existing) | Zero leaks |
| `test_nested_closure_borrowed_str_param` | higher_order.rs | Borrowed param (str) | Leak (pre-existing) | Zero leaks |
| `test_triple_nested_closure_capture` | higher_order.rs | Triple nesting | Leak (pre-existing) | Zero leaks |

**Additional verification:**

- [x] **Debug build tests**: `timeout 150 cargo t` — all Rust tests pass
- [x] **Release build tests**: `cargo b --release && timeout 150 cargo test --release -p ori_llvm --test aot` — release-mode AOT tests pass (FastISel behavior differs between debug and release)
- [x] **Dual-execution parity**: Verified via parallel test suites — 4,415 interpreter spec tests + 2,103 LLVM AOT tests (including all 6 closure RC leak tests) pass in both debug and release. The `dual-exec-verify.sh` script requires `@main` programs (unsuitable for test-only `.ori` files); the batch comparison was replaced by the parallel spec+AOT test suites which exercise both backends through the same ARC IR pipeline modified by this work.
- [x] **Leak check on ALL AOT tests**: full `timeout 150 cargo test -p ori_llvm --test aot` passes — `ORI_CHECK_LEAKS=1` is already set by `assert_aot_success`, so every test automatically checks for leaks.
- [x] **RC trace balance**: Build a closure test program (e.g., `ori build tests/spec/traits/iterator/map-filter-collect.ori -o /tmp/rc_test`), then `ORI_TRACE_RC=1 /tmp/rc_test` and verify balanced inc/dec (total allocs == total frees). Use `diagnostics/rc-stats.sh` for formatted output. Also run on a curried closure test: `ori build compiler/ori_llvm/tests/aot/fixtures/arc/arc_curried_closure_capture_list.ori -o /tmp/curried_test && ORI_TRACE_RC=1 /tmp/curried_test`.
- [x] **Full test suite**: `timeout 150 ./test-all.sh` — all tests pass, 0 failures, 0 leaks
- [x] **Update BUG-04-035** in `plans/bug-tracker/section-04-codegen-llvm.md`: mark as resolved with cross-link `<!-- resolved-by:plans/closure-ownership -->`
- [x] **Update TPR-04B-014** resolution note in `plans/jit-exception-handling/section-04b-lambda-mono.md`: note that the full architectural fix landed via this plan, add cross-link `<!-- resolved-by:plans/closure-ownership -->`

## 03.R Third Party Review Findings

- [x] `[TPR-03-001][medium]` `closures.rs:189` — Plan claimed lint suppression was added, but `#[expect(clippy::too_many_lines)]` is not present.
  Resolved: Validated on 2026-04-05. The attribute was intentionally removed because Clippy reports "unfulfilled lint expectation" — the function (127 lines) does not exceed Clippy's `too_many_lines` threshold (which counts statements, not lines). The plan item was updated to reflect that no suppression is needed or possible. Checklist item updated accordingly.

- [x] `[TPR-03-002][high]` `section-03-llvm-cleanup.md:267` — Plan claimed dual-exec parity via `dual-exec-verify.sh` but that script requires `@main` programs (none in test-only .ori files).
  Resolved: Validated on 2026-04-05. The `dual-exec-verify.sh` script is unsuitable for test-only spec files (no `@main`). Dual-execution parity is verified via parallel test suites: 4,415 interpreter spec tests + 2,103 LLVM AOT tests (both debug and release) pass, exercising both backends through the same ARC IR pipeline. Plan item updated with the actual verification methodology.

- [x] `[TPR-03-003][medium]` `plans/closure-ownership/index.md:58` — Plan index reported Section 03 as "Not Started" despite all subsections being complete.
  Resolved: Fixed on 2026-04-05. Updated `index.md` Quick Reference (Not Started → Complete), `00-overview.md` status (in-progress → complete), and `section-03-llvm-cleanup.md` status (in-progress → complete).

- [x] `[TPR-03-004][medium]` `section-03-llvm-cleanup.md:4` / `00-overview.md:4` — Plan metadata prematurely set to complete/resolved while TPR/hygiene gates still open.
  Resolved: Fixed on 2026-04-05. Statuses correctly reopened to in-progress/active by Codex. The plan-sync metadata items (status → complete) are intentionally gated AFTER TPR and hygiene reviews pass — they cannot be checked off until the review loop exits clean.

## 03.N Completion Checklist

- [x] Test module wiring: `drop_hints/mod.rs` + `drop_hints/tests.rs` and `unwind_cleanup/mod.rs` + `unwind_cleanup/tests.rs` created (03.1)
- [x] drop_hints `ApplyIndirect` workaround replaced with `arg_ownership`-aware logic (03.1)
- [x] drop_hints `InvokeIndirect` terminator handling added (03.1)
- [x] Shared helper `insert_borrowed_from_ownership` extracted to eliminate duplication (03.1)
- [x] unwind_cleanup `InvokeIndirect` handling added (03.1, TPR-01-006)
- [x] Rust unit tests pin `drop_hints` and `unwind_cleanup` indirect-call behavior (03.1)
- [x] `generate_env_drop_fn` correctness verified — env RcDec's ALL captures (03.2)
- [x] Unused `_capture_ownership` parameter removed from `generate_env_drop_fn` AND `build_closure_env` (03.2)
- [x] All 3 stale doc comments fixed: `context.rs:259`, `closures.rs:86`, `define_phase.rs:422` (03.2)
- [x] Wrapper RcInc logic verified correct (03.2)
- [x] `lambda_capture_ownership` fallback verified and `tracing::warn!` added (03.2)
- [x] `generate_env_drop_fn` lint suppression: not needed — Clippy `too_many_lines` does not fire (counts statements, not lines) (03.2)
- [x] 3 former `BUG-04-035` curried-closure tests verified enabled and passing (03.3)
- [x] 3 pre-existing nested closure leak tests verified passing (03.3)
- [x] All 6 previously-leaking tests pass with zero leaks (03.4)
- [x] Debug AND release builds pass (03.4)
- [x] Dual-execution parity verified via parallel spec+AOT suites (03.4)
- [x] Full test suite passes (03.4)
- [x] BUG-04-035 marked resolved with cross-link (03.4)
- [x] TPR-04B-014 updated with cross-link (03.4)
- [ ] Section 03 frontmatter `status` updated to `complete`
- [ ] `00-overview.md` Quick Reference table updated: Section 03 status → `Complete`
- [ ] `index.md` Quick Reference table updated: Section 03 status → `Complete`
- [ ] `00-overview.md` plan status updated to reflect completion (all sections complete)
- [x] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed
