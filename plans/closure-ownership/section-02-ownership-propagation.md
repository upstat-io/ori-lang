---
section: "02"
title: "Ownership Propagation"
status: not-started
reviewed: false
goal: "Populate arg_ownership on ApplyIndirect/InvokeIndirect from closure contracts via the AIMS pipeline"
success_criteria:
  - "annotate_arg_ownership() populates ApplyIndirect and InvokeIndirect"
  - "Ownership is seeded from the PartialApply target's MemoryContract"
  - "Unknown/opaque closures default to all-Borrowed (conservative)"
  - "ARC IR dump shows correct ownership for indirect calls in test programs"
  - "RC trace (ORI_TRACE_RC=1) shows balanced inc/dec for closure captures"
inspired_by:
  - "Existing Apply/Invoke annotation in compiler/ori_arc/src/rc_insert/annotate.rs:131-188"
  - "AIMS arg_ownership emission in compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs"
  - "Lean 4 standard calling convention for closures"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Extend annotate_arg_ownership for indirect calls"
    status: not-started
  - id: "02.2"
    title: "Closure contract resolution"
    status: not-started
  - id: "02.3"
    title: "Handle InvokeIndirect in borrow inference"
    status: not-started
  - id: "02.4"
    title: "Tests and verification"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Ownership Propagation

**Goal:** Teach the AIMS pipeline to populate `arg_ownership` on `ApplyIndirect`/`InvokeIndirect` instructions. The ownership must be seeded from the callee closure's `MemoryContract` — specifically, from the lambda body's parameter ownership for the USER arguments (not captures).

**Context from Section 01:** `ApplyIndirect` and `InvokeIndirect` now carry `arg_ownership` fields, initialized to empty `Vec`. This section populates them.

**Empty `arg_ownership` semantics**: Before annotation (pipeline steps 1-4), `arg_ownership` is empty — this means "not yet annotated." The `is_owned_position()` implementation uses `is_none_or()` which treats missing entries (empty vec) as `Owned` for `Apply`. For `ApplyIndirect`, the pre-annotation behavior should be all-`Borrowed` (conservative — uses `is_some_and`, so empty = all not-owned). After annotation, a non-empty `arg_ownership` vec means "annotated." An empty vec after annotation would be a bug (missed propagation).

- [ ] **Add `debug_assert!`** in `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` at the end of `emit_arg_ownership()` (after line 124, the call to `annotate_arg_ownership`): iterate all blocks in `func`, check every `ArcInstr::ApplyIndirect { args, arg_ownership, .. }` and every `ArcTerminator::InvokeIndirect { args, arg_ownership, .. }` — assert `args.is_empty() || !arg_ownership.is_empty()`. This catches missed propagation in debug builds.

## 02.1 Extend annotate_arg_ownership for indirect calls

The existing `annotate_arg_ownership()` in `compiler/ori_arc/src/rc_insert/annotate.rs` (258 lines, well under 500 limit) loops over `ArcInstr::Apply` (body, lines 148-167) and `ArcTerminator::Invoke` (terminator, lines 170-188) but skips `ApplyIndirect` and `InvokeIndirect`. The AIMS pipeline calls it via `emit_arg_ownership()` in `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` (125 lines). The entry point must be updated at both levels.

**Approach**: Add indirect-call annotation directly in `annotate_arg_ownership()` (not `emit_arg_ownership`) so the logic is co-located with the `Apply`/`Invoke` annotation. Thread the `contracts` map and the function's own body (for `PartialApply` tracing) through as new parameters.

- [ ] **Add `contracts` parameter** to `annotate_arg_ownership()` signature in `annotate.rs:131`:
  ```rust
  pub fn annotate_arg_ownership(
      func: &mut ArcFunction,
      sigs: &FxHashMap<Name, AnnotatedSig>,
      contracts: &FxHashMap<Name, MemoryContract>,  // NEW
      interner: &StringInterner,
      builtins: &BuiltinOwnershipSets,
      pool: &Pool,
  )
  ```
  Update the call site in `emit_arg_ownership()` (`arg_ownership.rs:124`) to pass `contracts` through.
- [ ] **Add `ApplyIndirect` branch** to the body instruction loop (after `Apply` branch, ~line 166):
  ```rust
  if let ArcInstr::ApplyIndirect { closure, args, arg_ownership, .. } = instr {
      *arg_ownership = resolve_indirect_arg_ownership(
          *closure, args.len(), func, contracts, interner,
      );
  }
  ```
  Note: `resolve_indirect_arg_ownership` is a new function (see 02.2). The existing `compute_arg_ownership()` cannot be reused directly because indirect calls have no callee `Name` — they have an `ArcVarId` closure.
- [ ] **Add `InvokeIndirect` branch** to the terminator handling (after `Invoke` branch, ~line 187):
  - Same pattern: resolve ownership from the closure's contract via `resolve_indirect_arg_ownership`
- [ ] **Conservative default**: When the closure's contract cannot be resolved → all args `Borrowed`. This matches the "caller retains cleanup" model that was already implicitly in use.

## 02.2 Closure contract resolution

The key challenge: at an `ApplyIndirect` call site, the closure is a runtime value (`ArcVarId`). We need to determine which lambda function it points to (if statically known) and retrieve that lambda's `MemoryContract`.

- [ ] **Implement `resolve_indirect_arg_ownership()`** — new function in `compiler/ori_arc/src/rc_insert/annotate.rs` (place after `annotate_arg_ownership`, before `apply_consuming_overrides`). Signature:
  ```rust
  fn resolve_indirect_arg_ownership(
      closure_var: ArcVarId,
      user_arg_count: usize,
      func: &ArcFunction,
      contracts: &FxHashMap<Name, MemoryContract>,
      interner: &StringInterner,
  ) -> Vec<ArgOwnership>
  ```
  Implementation steps:
  1. **Trace alias chain**: Walk backward through the function's blocks to find the def of `closure_var`. Follow `Let { dst, value: Var(src), .. }` chains (where `dst == closure_var` → recurse on `src`). Stop at `PartialApply { dst, func: target, args: capture_args, .. }` where `dst` is the resolved var.
  2. **Lookup contract**: If `PartialApply` found, look up `target` in `contracts`. If not found, try monomorphized name resolution (see sub-item below).
  3. **Extract user-arg ownership**: The `PartialApply`'s `capture_args.len()` IS `num_captures`. The contract's params `[num_captures..]` are the user args. Map each to `ArgOwnership::Owned` or `ArgOwnership::Borrowed` based on the contract's `ParamContract.access` field (same logic as `contract_to_params` in `arg_ownership.rs:28-52`).
  4. **Opaque closure fallback**: If the closure is a function parameter, result of another call, or otherwise untraceable → return `vec![ArgOwnership::Borrowed; user_arg_count]`.
  5. **Length validation**: `debug_assert_eq!(result.len(), user_arg_count)` before returning.

- [ ] **Handle capture offset**: The `PartialApply`'s `args` (captures) are the first N params of the target function's contract. The user args start at index N. Example: `PartialApply @lambda_foo(cap1, cap2)` → contract has params `[cap1, cap2, user1, user2]` → user ownership = `contract.params[2..]`.

- [ ] **No separate `num_captures` lookup needed**: The `PartialApply` instruction itself tells us the number of captures — `partial_apply.args.len()`. So `user_arg_offset = partial_apply.args.len()` and we skip that many params in the contract. No need to thread `ArcFunction.num_captures` (line 440 of `ir/mod.rs`).

- [ ] **Handle monomorphized names**: Closure targets may be monomorphized (e.g., `lambda_main_1$m$Lint`). Use the same monomorphized name resolution pattern as `emit_arg_ownership()` in `aims/emit_rc/arg_ownership.rs:89-107` — strip the `$m$` suffix via `ori_ir::MONO_SEPARATOR`, look up the original name in contracts if the mono name is not found.

## 02.3 Handle InvokeIndirect in borrow inference

Currently, `borrow/update.rs:272-274` (309 lines total, under 500 limit) has an empty branch for `InvokeIndirect`:
```rust
// InvokeIndirect: closure call — no named callee to look up
// ownership for, so treat conservatively (no promotion).
| ArcTerminator::InvokeIndirect { .. } => {}
```

- [ ] **Update borrow inference** to use the new `arg_ownership` field on `InvokeIndirect`. Replace the empty branch with logic that mirrors the `Invoke` branch (lines 276-280) but uses `arg_ownership` instead of calling `promote_callee_args()` (which requires a callee `Name`). Specifically:
  ```rust
  ArcTerminator::InvokeIndirect { closure, args, arg_ownership, .. } => {
      // Closure itself is always needed (borrowed).
      changed |= try_mark_param_owned(*closure, ctx.func, ctx.my_sig, ctx.aliases);
      // For args with Owned ownership, mark the corresponding parameter as owned.
      for (i, &arg) in args.iter().enumerate() {
          if arg_ownership.get(i).is_some_and(|o| *o == ArgOwnership::Owned) {
              changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
          }
      }
  }
  ```
  **Note**: Also update the `ApplyIndirect` body instruction branch (line 213-218) to use `arg_ownership` for more precise borrow promotion instead of unconditionally calling `try_mark_param_owned` for ALL args. Currently it promotes all args to owned — once `arg_ownership` is populated, only promote args where `arg_ownership[i] == Owned`.

- [ ] **Cross-reference: `collect_borrowed_call_args()` update**: Section 03.1 handles the `drop_hints.rs` refinement — replacing the conservative `ApplyIndirect` workaround with `arg_ownership`-aware logic, and adding `InvokeIndirect` terminator handling. This item (02.3) handles the borrow inference update only.

## 02.4 Tests and verification

**TDD order**: Write unit tests first, verify they fail (ownership not populated = all Borrowed), then implement, then verify matrix tests pass.

**Rust unit tests** in `compiler/ori_arc/src/rc_insert/tests.rs`:
- [ ] `test_annotate_apply_indirect_from_partial_apply`: construct an `ArcFunction` with a `PartialApply` creating a closure and an `ApplyIndirect` calling it. Provide a matching `MemoryContract`. Verify `arg_ownership` is populated with the correct ownership from the contract's user-arg params.
- [ ] `test_annotate_apply_indirect_opaque_closure`: `ApplyIndirect` where the closure var is a function parameter (not traceable to `PartialApply`). Verify result is all-`Borrowed`.
- [ ] `test_annotate_invoke_indirect`: same pattern for `InvokeIndirect` terminator — verify ownership populated from contract.
- [ ] `test_annotate_apply_indirect_with_captures_offset`: `PartialApply` with 2 captures, contract has 4 params. Verify `arg_ownership` for the `ApplyIndirect` (2 user args) comes from contract params `[2..4]`, NOT `[0..2]`.
- [ ] **Negative pin**: `test_annotate_apply_indirect_opaque_not_owned` — verify opaque closure does NOT produce `Owned` for any arg (pins the conservative default).

**Semantic pin**:
- [ ] ARC IR dump test showing `ApplyIndirect` with `[own, borrow]` annotations for a curried closure — use `ORI_DUMP_AFTER_ARC=1` on a test program and verify the output format.

**Matrix testing** — the fix must be verified across ALL closure patterns (these are existing AOT test files in `compiler/ori_llvm/tests/aot/`):

| Pattern | Test file | What to verify |
|---------|-----------|---------------|
| Curried capture (list) | `arc_curried_closure_capture_list.ori` | Zero leaks, exit 0 |
| Curried capture (str) | `arc_curried_closure_capture_str.ori` | Zero leaks, exit 0 |
| Curried capture (nested) | `arc_curried_closure_capture_nested.ori` | Zero leaks, exit 0 |
| Nested borrowed param (list) | `nested_closure_borrowed_list_param.ori` | Zero leaks, exit 0 |
| Nested borrowed param (str) | `nested_closure_borrowed_str_param.ori` | Zero leaks, exit 0 |
| Triple nested | `triple_nested_closure_capture.ori` | Zero leaks, exit 0 |
| Closure with user arg (borrowed) | `f05_closure_param/fm_closure_param_str_heap.ori` | Zero leaks, exit 0 |
| Scalar capture (negative pin) | `arc_curried_closure_scalar_no_inc.ori` | No RcInc for scalars |

- [ ] **Verify RC trace balance**: `ORI_TRACE_RC=1` on the curried repro shows balanced inc/dec

## 02.R Third Party Review Findings

- None.

## 02.N Completion Checklist

- [ ] `annotate_arg_ownership()` handles `ApplyIndirect` (02.1)
- [ ] `annotate_arg_ownership()` handles `InvokeIndirect` (02.1)
- [ ] `contracts` parameter threaded to `annotate_arg_ownership` (02.1)
- [ ] `resolve_indirect_arg_ownership()` traces closure to PartialApply source (02.2)
- [ ] Capture offset handled correctly — user args from `contract.params[num_captures..]` (02.2)
- [ ] Monomorphized name resolution for closure targets (02.2)
- [ ] Opaque closures default to all-Borrowed (02.2)
- [ ] `debug_assert!` verifies non-empty `arg_ownership` after annotation (02 preamble)
- [ ] Borrow inference updated for InvokeIndirect AND ApplyIndirect precision (02.3)
- [ ] Cross-reference verified: `collect_borrowed_call_args()` refinement tracked in Section 03.1
- [ ] Unit tests including capture offset test and negative pin (02.4)
- [ ] All 8 closure pattern tests pass with zero leaks (02.4)
- [ ] `timeout 150 cargo t` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed
