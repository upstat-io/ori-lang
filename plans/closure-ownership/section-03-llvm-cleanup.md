---
section: "03"
title: "LLVM Cleanup & Verification"
status: not-started
reviewed: false
goal: "Replace LLVM-level workarounds with arg_ownership logic, add InvokeIndirect handling (drop_hints + unwind_cleanup), verify env drop correctness, fix stale docs, un-ignore tests, verify zero leaks"
success_criteria:
  - "collect_borrowed_call_args ApplyIndirect workaround replaced with arg_ownership-aware logic"
  - "collect_borrowed_call_args handles InvokeIndirect terminator via arg_ownership"
  - "unwind_cleanup handles InvokeIndirect (TPR-01-006)"
  - "generate_env_drop_fn correctness verified (RcDec ALL captures — no skipping for borrowed)"
  - "All 4 stale doc comments corrected (context.rs:259, closures.rs:86, closures.rs:155, define_phase.rs:422)"
  - "All 3 #[ignore = BUG-04-035] tests un-ignored and passing"
  - "All 3 pre-existing nested closure leak tests passing"
  - "ORI_CHECK_LEAKS=1 reports zero leaks on full AOT suite"
  - "Dual-execution parity for all closure test programs"
inspired_by:
  - "Swift SIL partial_apply ownership annotations"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Replace drop_hints workaround and add InvokeIndirect"
    status: not-started
  - id: "03.2"
    title: "Verify env drop function correctness"
    status: not-started
  - id: "03.3"
    title: "Un-ignore tests and verify"
    status: not-started
  - id: "03.4"
    title: "Full verification matrix"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: LLVM Cleanup & Verification

**Goal:** Now that `arg_ownership` flows through the ARC IR for indirect calls (Sections 01-02), remove the LLVM-level workarounds that compensated for the missing ownership info. Verify env drop correctness (it already RcDec's ALL captures correctly — the `_capture_ownership` param is unused). Fix stale doc comments. Un-ignore all BUG-04-035 tests and verify zero leaks.

**Context from Sections 01-02:** `ApplyIndirect`/`InvokeIndirect` now carry populated `arg_ownership`. The AIMS system correctly emits `RcInc`/`RcDec` based on ownership. LLVM codegen should now just lower what the ARC IR says.

## 03.1 Replace drop_hints workaround and add InvokeIndirect

The safety fix in commit `a83b8e65` added a workaround in `collect_borrowed_call_args()` (`compiler/ori_arc/src/aims/emit_rc/drop_hints.rs`, 155 lines, well under 500 limit) that conservatively marks ALL `ApplyIndirect` args as potentially shared. This was necessary because `ApplyIndirect` lacked ownership info — `drop_unique` was being used on values that might be shared via closure environments.

**Prerequisite**: Section 02 must be complete — `arg_ownership` is populated on `ApplyIndirect`/`InvokeIndirect` before this code runs. Section 02.3 handles the legacy borrow inference update in `borrow/update.rs`; this subsection handles the parallel `drop_hints.rs` refinement.

- [ ] **Replace the `ApplyIndirect` branch** in `collect_borrowed_call_args()` (lines 78-82 in `drop_hints.rs`). The current branch:
  ```rust
  ArcInstr::ApplyIndirect { args, .. } => {
      for &arg in args {
          borrowed.insert(arg);
      }
  }
  ```
  Replace with `arg_ownership`-aware logic:
  ```rust
  ArcInstr::ApplyIndirect { args, arg_ownership, .. } => {
      for (i, &arg) in args.iter().enumerate() {
          if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
              borrowed.insert(arg);
          }
      }
  }
  ```
  No `is_safe_non_sharing_callee` gate (indirect callee is unknown — always use `arg_ownership` directly).
- [ ] **Add `InvokeIndirect` terminator handling** to `collect_borrowed_call_args()` (after the `Invoke` terminator check, lines 94-109). Currently the terminator scan only checks `Invoke` — add a matching branch:
  ```rust
  if let ArcTerminator::InvokeIndirect { args, arg_ownership, .. } = &block.terminator {
      for (i, &arg) in args.iter().enumerate() {
          if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
              borrowed.insert(arg);
          }
      }
  }
  ```
- [ ] **Verify**: values with `ArgOwnership::Owned` at `ApplyIndirect`/`InvokeIndirect` call sites are NOT treated as borrowed call args (they transfer ownership to the callee). Values with `ArgOwnership::Borrowed` ARE treated as borrowed call args (caller retains ownership, may not be unique).
- [ ] **Add `InvokeIndirect` to `unwind_cleanup.rs`** (`compiler/ori_arc/src/aims/emit_rc/unwind_cleanup.rs:57`): the unwind iterator cleanup currently only scans `ArcTerminator::Invoke` — add a matching check for `InvokeIndirect` so that iterator cleanup on unwind paths also covers indirect invoke sites. This was flagged by TPR-01-006 and noted as "tracked for Section 03" — this is the tracking item.

## 03.2 Verify env drop function correctness (DO NOT skip RcDec for borrowed captures)

The `generate_env_drop_fn()` in `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:189-315` has `_capture_ownership` as an UNUSED parameter. The plan originally proposed using it to skip `RcDec` for borrowed captures — **this is INCORRECT and would cause leaks**.

**Why the env drop must RcDec ALL captures regardless of ownership:**

The env struct physically stores (copies of) ALL captures. "Borrowed" here means "borrowed by the lambda body" — the wrapper function skips `RcInc` for borrowed captures, so the lambda body doesn't get its own reference. But the env itself still holds one reference to each capture (it was copied into the env by `build_closure_env`). When the env is freed, it must `RcDec` all its captures to release its references.

**RC balance for Owned captures**: env stores 1 ref → wrapper `RcInc` creates 2nd ref for lambda body → env drop `RcDec` destroys env's ref → lambda body `RcDec` destroys body's ref → balanced.

**RC balance for Borrowed captures**: env stores 1 ref → wrapper does NOT `RcInc` (body borrows from env) → env drop `RcDec` destroys env's ref → body does NOT `RcDec` → balanced.

Skipping `RcDec` for borrowed captures would orphan the env's reference, causing a leak.

The comment at `closures.rs:222-226` already documents this correctly:
> "The drop function must dec all RC-needing captures regardless of the lambda's borrow annotation — the annotation controls the lambda BODY's treatment, not env ownership."

- [ ] **Fix stale doc comments** (4 locations, all in `compiler/ori_llvm/src/`):
  1. `codegen/arc_emitter/context.rs:259-260` — incorrectly says "the closure's env drop function must NOT RC-dec that capture — the caller retains ownership." Update to: "the closure's wrapper function skips `RcInc` for borrowed captures — the lambda body borrows from the env rather than getting its own reference."
  2. `codegen/arc_emitter/closures.rs:86` — incorrectly says "which captures are borrowed (skip RC dec in drop fn)." Update to: "which captures are borrowed (skip RcInc in wrapper — body borrows from env)."
  3. `codegen/arc_emitter/closures.rs:155` — incorrectly says "Pass capture ownership so borrowed captures are NOT RC-dec'd." Update to: "Pass capture ownership so borrowed captures skip RcInc in the wrapper (body borrows from env). Env drop RcDec's ALL captures regardless."
  4. `codegen/function_compiler/define_phase.rs:422-423` — incorrectly says "correct env drop functions: borrowed captures must NOT be RC-dec'd." Update to: "correct wrapper functions: borrowed captures skip RcInc (body borrows from env). Env drop RcDec's ALL captures regardless."
- [ ] **Remove the unused `_capture_ownership` parameter** from `generate_env_drop_fn` in `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:189`. The parameter is unused (prefixed with `_`), and the env drop correctly `RcDec`s all captures — no change to the RC logic. Also update the call site at `closures.rs:156-157` to stop passing `capture_ownership`. The `capture_ownership` local is still needed for `build_closure_env` (line 98) and the wrapper function (stored in context at line 87-92).
- [ ] **Verify the wrapper RcInc logic** in `compiler/ori_llvm/src/codegen/arc_emitter/closure_wrappers.rs:175-179` (239 lines, under 500 limit) is correct with the ownership model:
  - The wrapper `RcInc` fires for `Owned` captures (line 179: `ownership == Ownership::Owned && needs_rc`) — correct: creates a second reference for the lambda body
  - For `Borrowed` captures: no wrapper `RcInc` — correct: body borrows from the env
  - The env drop `RcDec`s ALL captures regardless — correct: env owns all its stored values

## 03.3 Un-ignore tests and verify

- [ ] **Remove `#[ignore]`** from the 3 curried closure tests in `compiler/ori_llvm/tests/aot/arc.rs` (764 lines — test file, no 500-line limit). Search for `BUG-04-035` to find the exact lines:
  - `test_arc_curried_closure_capture_list` — remove `#[ignore = "BUG-04-035: ..."]`
  - `test_arc_curried_closure_capture_str` — remove `#[ignore = "BUG-04-035: ..."]`
  - `test_arc_curried_closure_capture_nested` — remove `#[ignore = "BUG-04-035: ..."]`
- [ ] **Run ALL closure AOT tests** with leak check:
  ```bash
  timeout 150 cargo test -p ori_llvm --test aot -- test_arc_curried test_nested_closure test_triple_nested test_fm_closure_param test_arc_lambda test_arc_closure
  ```
- [ ] **Verify zero leaks**: `ORI_CHECK_LEAKS=1` is already set by `assert_aot_success` — every AOT test automatically checks for leaks. Verify none of the above tests report leaks in their output.

## 03.4 Full verification matrix

**All 6 previously-leaking tests must pass (zero leaks):**

| Test | Pattern | Pre-fix status | Post-fix |
|------|---------|---------------|----------|
| `test_arc_curried_closure_capture_list` | Curried list capture | Leak (#ignore) | Zero leaks |
| `test_arc_curried_closure_capture_str` | Curried str capture | Leak (#ignore) | Zero leaks |
| `test_arc_curried_closure_capture_nested` | Nested curried | Leak (#ignore) | Zero leaks |
| `test_nested_closure_borrowed_list_param` | Borrowed param (list) | Leak (pre-existing) | Zero leaks |
| `test_nested_closure_borrowed_str_param` | Borrowed param (str) | Leak (pre-existing) | Zero leaks |
| `test_triple_nested_closure_capture` | Triple nesting | Leak (pre-existing) | Zero leaks |

**Additional verification:**

- [ ] **Debug build tests**: `timeout 150 cargo t` — all Rust tests pass
- [ ] **Release build tests**: `cargo b --release && timeout 150 cargo test --release -p ori_llvm --test aot` — release-mode AOT tests pass (FastISel behavior differs between debug and release)
- [ ] **Dual-execution parity**: `bash diagnostics/dual-exec-debug.sh --no-color tests/spec/traits/iterator/` — interpreter and AOT produce identical results for closure-heavy tests. Also verify with a closure-specific test file.
- [ ] **Leak check on ALL AOT tests**: full `timeout 150 cargo test -p ori_llvm --test aot` passes — `ORI_CHECK_LEAKS=1` is already set by `assert_aot_success`, so every test automatically checks for leaks.
- [ ] **RC trace balance**: Build a closure test program, then `ORI_TRACE_RC=1 ./target/debug/test_binary` and verify balanced inc/dec (total allocs == total frees). Use `diagnostics/rc-stats.sh` for formatted output.
- [ ] **Full test suite**: `timeout 150 ./test-all.sh` — all tests pass, 0 failures, 0 leaks
- [ ] **Update BUG-04-035** in `plans/bug-tracker/section-04-codegen-llvm.md`: mark as resolved with cross-link `<!-- resolved-by:plans/closure-ownership -->`
- [ ] **Update TPR-04B-014** resolution note in `plans/jit-exception-handling/section-04b-lambda-mono.md`: note that the full architectural fix landed via this plan, add cross-link `<!-- resolved-by:plans/closure-ownership -->`

## 03.R Third Party Review Findings

- None.

## 03.N Completion Checklist

- [ ] drop_hints `ApplyIndirect` workaround replaced with `arg_ownership`-aware logic (03.1)
- [ ] drop_hints `InvokeIndirect` terminator handling added (03.1)
- [ ] unwind_cleanup `InvokeIndirect` handling added (03.1, TPR-01-006)
- [ ] `generate_env_drop_fn` correctness verified — env RcDec's ALL captures (03.2)
- [ ] Unused `_capture_ownership` parameter removed from `generate_env_drop_fn` (03.2)
- [ ] All 4 stale doc comments fixed: `context.rs:259`, `closures.rs:86`, `closures.rs:155`, `define_phase.rs:422` (03.2)
- [ ] Wrapper RcInc logic verified correct (03.2)
- [ ] 3 `#[ignore = "BUG-04-035"]` tests un-ignored (03.3)
- [ ] All 6 previously-leaking tests pass with zero leaks (03.4)
- [ ] Debug AND release builds pass (03.4)
- [ ] Dual-execution parity verified (03.4)
- [ ] Full test suite passes (03.4)
- [ ] BUG-04-035 marked resolved with cross-link (03.4)
- [ ] TPR-04B-014 updated with cross-link (03.4)
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed
