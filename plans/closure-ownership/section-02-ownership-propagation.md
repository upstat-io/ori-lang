---
section: "02"
title: "Ownership Propagation"
status: in-progress
reviewed: true
goal: "Populate arg_ownership on ApplyIndirect/InvokeIndirect from closure contracts via the AIMS pipeline"
success_criteria:
  - "annotate_arg_ownership() populates ApplyIndirect and InvokeIndirect → mission: arg_ownership populated"
  - "Ownership is seeded from the resolved PartialApply target while reusing the same normalized AnnotatedSig + builtin override logic as direct calls → mission: annotate_arg_ownership() populates from closure contracts"
  - "Unknown/opaque closures default to all-Borrowed (conservative) → mission: is_owned_position() respects arg_ownership"
  - "debug_assert! enforces non-empty arg_ownership after annotation for indirect calls with args → mission: correctness invariant"
  - "Legacy borrow inference (borrow/update.rs) uses arg_ownership for ApplyIndirect and InvokeIndirect → mission: collect_borrowed_call_args handles indirect calls"
  - "ARC IR dump shows correct ownership for indirect calls in test programs → mission: observable verification"
  - "RC trace (ORI_TRACE_RC=1) shows balanced inc/dec for closure captures → mission: zero leaks"
inspired_by:
  - "Existing Apply/Invoke annotation in compiler/ori_arc/src/rc_insert/annotate.rs:131-188"
  - "AIMS arg_ownership emission in compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs"
  - "Lean 4 standard calling convention for closures"
depends_on: ["01"]
third_party_review:
  status: findings
  updated: 2026-04-05
sections:
  - id: "02.1"
    title: "Extend annotate_arg_ownership for indirect calls"
    status: complete
  - id: "02.2"
    title: "Closure contract resolution"
    status: complete
  - id: "02.3"
    title: "Handle InvokeIndirect in borrow inference"
    status: complete
  - id: "02.4"
    title: "Tests and verification"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Ownership Propagation

**Goal:** Teach the AIMS pipeline to populate `arg_ownership` on `ApplyIndirect`/`InvokeIndirect` instructions. The ownership must be seeded from the resolved closure target's contract/signature for the USER arguments (not captures) without creating a second ownership-annotation code path that can drift from direct-call semantics.

**Context from Section 01:** `ApplyIndirect` and `InvokeIndirect` now carry `arg_ownership` fields, initialized to empty `Vec`. This section populates them.

**Empty `arg_ownership` semantics**: Before annotation (pipeline steps 1-4), `arg_ownership` is empty — this means "not yet annotated." The `is_owned_position()` implementation uses `is_none_or()` which treats missing entries (empty vec) as `Owned` for `Apply`. For `ApplyIndirect`, the pre-annotation behavior should be all-`Borrowed` (conservative — uses `is_some_and`, so empty = all not-owned). After annotation, a non-empty `arg_ownership` vec means "annotated." An empty vec after annotation would be a bug (missed propagation).

**File size budget for `annotate.rs`**: Currently 258 lines. This section adds ~150 lines (def map builder, `ResolvedDef` enum, `resolve_indirect_arg_ownership`, new branches). Projected total ~410 lines — under the 500-line limit but close. If implementation exceeds ~450 lines, extract `resolve_indirect_arg_ownership` and its supporting types into a sibling `closure_resolve.rs` module and re-export from `rc_insert/mod.rs`.

**TDD execution order**: Although 02.4 is listed last for readability, TDD requires writing the unit test stubs (02.4 Rust tests) FIRST, verifying they fail or do not compile, THEN implementing 02.1-02.3, THEN verifying the tests pass unchanged. The subsection ordering is logical grouping, not execution sequence.

- [x] **Add `debug_assert!`** in `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` at the end of `emit_arg_ownership()` (after line 124, the call to `annotate_arg_ownership`): iterate all blocks in `func`, check every `ArcInstr::ApplyIndirect { args, arg_ownership, .. }` and every `ArcTerminator::InvokeIndirect { args, arg_ownership, .. }` — assert `args.is_empty() || !arg_ownership.is_empty()`. This catches missed propagation in debug builds.
- [x] **Update doc comments**: (a) `annotate_arg_ownership()` in `compiler/ori_arc/src/rc_insert/annotate.rs:107` currently says "Populate `arg_ownership` on all `Apply` and `Invoke` instructions" — update to include `ApplyIndirect`/`InvokeIndirect`. (b) Module doc in `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs:3` currently says "Populates `arg_ownership` on `Apply`/`Invoke` instructions" — update to include `ApplyIndirect`/`InvokeIndirect`.

## 02.1 Extend annotate_arg_ownership for indirect calls

The existing `annotate_arg_ownership()` in `compiler/ori_arc/src/rc_insert/annotate.rs` (258 lines, well under 500 limit) loops over `ArcInstr::Apply` (body, lines 148-167) and `ArcTerminator::Invoke` (terminator, lines 170-188) but skips `ApplyIndirect` and `InvokeIndirect`. The AIMS pipeline calls it via `emit_arg_ownership()` in `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` (125 lines). The indirect path must be added without duplicating the monomorphization merge or builtin/type-qualified override rules that the direct path already centralizes.

**Approach**: Keep indirect-call annotation co-located with `Apply`/`Invoke`, but resolve the closure to a `PartialApply` target/capture list and then reuse the same normalized `AnnotatedSig` + builtin override machinery that direct calls already use. Do NOT create a second path that reads raw `MemoryContract`s directly for indirect calls — that would drift from `emit_arg_ownership()`'s monomorphization merge and from `annotate_arg_ownership()`'s type-qualified builtin overrides.

- [x] **Preserve one normalized ownership source**: keep `emit_arg_ownership()` responsible for building the normalized `AnnotatedSig` map (including monomorphization merge). If `annotate_arg_ownership()` needs extra context for indirect calls, thread only the minimal additional resolver context needed to find the closure target/captures. Do NOT add a parallel raw-`MemoryContract` lookup path for indirect calls.
  ```rust
  // Pseudocode shape: keep `sigs` authoritative, add only resolver context.
  pub fn annotate_arg_ownership(/* ... */, sigs: &FxHashMap<Name, AnnotatedSig>, /* ... */)
  ```
  Rationale: `emit_arg_ownership()` already performs conservative monomorphization merging. Re-implementing that in the indirect path would be drift-prone.
- [x] **Define `ResolvedDef` enum** in `compiler/ori_arc/src/rc_insert/annotate.rs` (place after `ConsumingCtx` struct, before free functions). This is a small enum (~15 lines) that captures only the fields needed for closure resolution — NOT a reference to `&ArcInstr` (which would conflict with the mutable borrow):
  ```rust
  enum ResolvedDef {
      PartialApply { func: Name, capture_args: Vec<ArcVarId> },
      Alias(ArcVarId),
      BlockParam { predecessors: Vec<ArcVarId> },
      Other,
  }
  ```
- [x] **Implement `build_closure_def_map()`** in `compiler/ori_arc/src/rc_insert/annotate.rs` (place after `ResolvedDef`). Signature: `fn build_closure_def_map(func: &ArcFunction) -> FxHashMap<ArcVarId, ResolvedDef>`. Two-pass construction: (1) walk `&func.blocks` to record `PartialApply` targets/capture-args, `Let { value: Var(src) }` aliases, and all other instructions as `Other`; (2) walk all `Jump { target, args }` terminators and for each target block, record each block-param var → `BlockParam { predecessors }` mapping from the jump args. Note: a similar `build_definition_map` exists in `aims/interprocedural/extract.rs:490` (private, returns `&ArcInstr` references) — do NOT reuse it (different return type, would create borrow conflict). Build a specialized version.
- [x] **Insert def map construction before mutable loop**: insert `let def_map = build_closure_def_map(func);` BEFORE the `for block in &mut func.blocks` loop (after `ConsumingCtx` creation at line 144). This precomputes the closure resolution data while `func` is still immutably borrowable. **Borrow compatibility**: `ConsumingCtx` borrows `func.var_types` (a field separate from `func.blocks`), so `build_closure_def_map(&*func)` before the loop and `&mut func.blocks` inside the loop are compatible — Rust's borrow checker allows this because `blocks` and `var_types` are disjoint fields.
- [x] **Add `ApplyIndirect` branch** to the body instruction loop (after `Apply` branch, ~line 166):
  ```rust
  if let ArcInstr::ApplyIndirect { closure, args, arg_ownership, .. } = instr {
      *arg_ownership = resolve_indirect_arg_ownership(
          *closure, args, &def_map, sigs, &consuming_ctx, builtins,
      );
  }
  ```
  Note: `resolve_indirect_arg_ownership` must resolve the indirect callee to `(target, capture_count)` via the precomputed `def_map` and then reuse the same ownership computation as direct calls over the FULL logical arg list `[captures..., user_args...]`, slicing off the capture prefix afterward.
- [x] **Add `InvokeIndirect` branch** to the terminator handling (after `Invoke` branch, ~line 187):
  - Same pattern: resolve ownership from the closure's contract via `resolve_indirect_arg_ownership`
- [x] **Conservative default**: When the closure's contract cannot be resolved → all args `Borrowed`. This matches the "caller retains cleanup" model that was already implicitly in use.

## 02.2 Closure contract resolution

The key challenge: at an `ApplyIndirect` call site, the closure is a runtime value (`ArcVarId`). We need to determine which lambda function it points to (if statically known) and retrieve that lambda's `MemoryContract`.

- [x] **Implement `resolve_indirect_arg_ownership()`** — new function in `compiler/ori_arc/src/rc_insert/annotate.rs` (place after `build_closure_def_map`, before `apply_consuming_overrides` — i.e., between the def-map builder and the override logic). Signature:
  ```rust
  fn resolve_indirect_arg_ownership(
      closure_var: ArcVarId,
      user_args: &[ArcVarId],
      def_map: &FxHashMap<ArcVarId, ResolvedDef>,
      sigs: &FxHashMap<Name, AnnotatedSig>,
      consuming_ctx: &ConsumingCtx<'_>,
      builtins: &BuiltinOwnershipSets,
  ) -> Vec<ArgOwnership>
  ```
  **Borrow-safety note**: `annotate_arg_ownership` iterates `&mut func.blocks`, so you cannot pass `&ArcFunction` into the resolver during that loop. The `def_map` and `ConsumingCtx` are both precomputed before the mutable loop (02.1 items above). `ResolvedDef` is defined in 02.1.

  Implementation steps (each is a verifiable sub-step within `resolve_indirect_arg_ownership`):
  1. **Zero user-arg fast path** (FIRST): if `user_args.is_empty()`, return `Vec::new()` immediately — no resolution needed. This handles thunks (zero-arg closures) and avoids unnecessary def-map lookups.
  2. **Resolve closure var through SSA defs**: implement a local helper `resolve_to_partial_apply(var, def_map, visited) -> Option<(Name, Vec<ArcVarId>)>` that follows `Alias(src)` chains through the def map. Track a `visited: &mut FxHashSet<ArcVarId>` so loop-carried block params / alias cycles terminate — if a var is re-encountered, return `None`. Do NOT rely on "walk blocks backward" ordering — block order is not a semantic dominance order.
  3. **Handle block params / merged closures explicitly**: when the resolver hits a `BlockParam { predecessors }`, recursively resolve each predecessor. Reuse a merged result ONLY when every incoming value resolves to a compatible `PartialApply` for this specific call site: same target `Name`, same capture count, and no disagreement in the post-override user-arg ownership vector after running the full `[captures..., user_args...]` computation. If incoming edges disagree, or a cycle is hit before proving equivalence, return `None` (which triggers the opaque fallback).
  4. **Stop at `PartialApply`**: once a `PartialApply { func: target, capture_args }` is found, use `capture_args` directly — `capture_args.len()` is the capture prefix length.
  5. **Reuse the direct-call ownership path on the FULL arg list**: construct a combined var list `[capture_args..., user_args...]` from the `PartialApply`'s stored capture args and the call site's user args. Call `compute_arg_ownership(target, combined.len(), sigs, consuming_ctx.interner, &builtins.borrowing, &builtins.consuming_receiver_only, &builtins.protocol)` and then `apply_consuming_overrides(target, &combined, &mut ownership, consuming_ctx)`. Only after that, slice away the capture prefix (`ownership[capture_args.len()..].to_vec()`) and return ownership for the user args.
  6. **Opaque closure fallback**: if the closure resolves to `Other`, `None`, or any untraceable origin (function parameter, result of another call, `Project`, `Select`, collection element, or ambiguous merge) → return `vec![ArgOwnership::Borrowed; user_args.len()]`.
  7. **Length validation**: `debug_assert_eq!(result.len(), user_args.len())` before returning.

- [x] **Handle capture offset only after shared ownership computation**: the `PartialApply`'s `args` are the first N logical call arguments, but the slice must happen AFTER builtin/type-qualified overrides run on the full `[captures..., user_args...]` vector. This is required for partially applied builtins like `push`, `add`, `concat`, `remove`, and `union`, whose contracts rely on the existing direct-call override path.

- [x] **No separate `num_captures` lookup needed**: The `PartialApply` instruction itself tells us the number of captures — `partial_apply.args.len()`. So `user_arg_offset = partial_apply.args.len()` and we skip that many params in the contract. No need to thread `ArcFunction.num_captures` (line 391 of `ir/mod.rs`).

- [x] **Do not duplicate monomorphized-name fallback**: indirect resolution must consume the same already-normalized lookup that `emit_arg_ownership()` builds. Do NOT add a second `$m$` stripping path here.

## 02.3 Legacy borrow inference parity

Currently, `borrow/update.rs:272-274` (309 lines total, under 500 limit) has an empty branch for `InvokeIndirect`:
```rust
// InvokeIndirect: closure call — no named callee to look up
// ownership for, so treat conservatively (no promotion).
| ArcTerminator::InvokeIndirect { .. } => {}
```

- [x] **Update `InvokeIndirect` terminator branch** in `compiler/ori_arc/src/borrow/update.rs:268-274`. Replace the empty `InvokeIndirect { .. } => {}` arm (currently grouped with `Branch`/`Switch`/`Unreachable`/`Resume`) with a dedicated arm that uses `arg_ownership`:
  ```rust
  ArcTerminator::InvokeIndirect { args, arg_ownership, .. } => {
      // Do NOT promote the closure operand itself here. In ARC IR the closure
      // position is always borrowed; only user arguments transfer ownership.
      // For args with Owned ownership, mark the corresponding parameter as owned.
      for (i, &arg) in args.iter().enumerate() {
          if arg_ownership.get(i).is_some_and(|o| *o == ArgOwnership::Owned) {
              changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
          }
      }
  }
  ```
  This requires importing `ArgOwnership` in `borrow/update.rs` (currently not imported — add `use crate::ir::ArgOwnership;`).

- [x] **Update `ApplyIndirect` body instruction branch** in `compiler/ori_arc/src/borrow/update.rs:213-218`. Currently it unconditionally promotes the closure operand and ALL args to owned:
  ```rust
  // BEFORE (current):
  ArcInstr::ApplyIndirect { closure, args, .. } => {
      changed |= try_mark_param_owned(*closure, ctx.func, ctx.my_sig, ctx.aliases);
      for &arg in args {
          changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
      }
  }
  ```
  Replace with `arg_ownership`-aware logic — only promote args where `arg_ownership[i] == Owned`:
  ```rust
  // AFTER:
  ArcInstr::ApplyIndirect { closure, args, arg_ownership, .. } => {
      // Closure operand: always borrowed in ARC IR — do NOT promote.
      // User args: promote only where arg_ownership says Owned.
      for (i, &arg) in args.iter().enumerate() {
          if arg_ownership.get(i).is_some_and(|o| *o == ArgOwnership::Owned) {
              changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
          }
      }
  }
  ```
  Note the closure operand (`*closure`) is no longer promoted — it was incorrectly promoted before because the absence of ownership info forced the conservative "promote everything" approach. With `arg_ownership` populated, only user args that transfer ownership need promotion.

- [x] **Remove stale comment** at `borrow/update.rs:272-273` ("InvokeIndirect: closure call — no named callee to look up ownership for, so treat conservatively (no promotion).") — this comment describes the pre-Section-02 behavior and becomes stale after the update.

  **Boundary clarification**: this is a consumer-parity task for the legacy SCC borrow-inference pass, not the source of AIMS `arg_ownership` semantics. The authoritative ownership semantics must still be produced in `annotate_arg_ownership()` / `emit_arg_ownership()`.
  **Sequencing note**: implement this AFTER 02.1 and 02.2 (it consumes the `arg_ownership` they produce), but it MUST be completed within Section 02 — it is not optional. The active leak fix runs through `compute_aims_contracts()` + `realize_rc_reuse()`, not `borrow/update.rs`; this item exists to keep legacy SCC/query tooling semantically aligned. Leaving it unfinished would create a DRIFT between the AIMS path and the legacy path.

- [x] **Cross-reference: `collect_borrowed_call_args()` update**: Section 03.1 handles the `drop_hints.rs` refinement — replacing the conservative `ApplyIndirect` workaround with `arg_ownership`-aware logic, and adding `InvokeIndirect` terminator handling. This item (02.3) handles the borrow inference update only.

- [x] **Hygiene: `borrow/update.rs:241`** — the statement `let _ = value;` in the `ArcInstr::Let` arm is unnecessary (the field is already unused via `..` on the destructure). Remove this dead binding during the 02.3 edit pass.

## 02.4 Tests and verification

**TDD execution sequence** (mandatory — per CLAUDE.md TDD discipline):
1. **Write test stubs first** (all Rust unit tests below) — verify they fail or do not compile before 02.1-02.3 implementation
2. **Implement 02.1** (def map, branches) → verify resolver-level tests still fail (ownership not yet resolved)
3. **Implement 02.2** (resolver) → verify resolver-level tests pass
4. **Implement 02.3** (borrow inference) → verify full test suite passes
5. **Verify matrix tests** (AOT closure tests) pass with zero leaks

**Test module wiring** (prerequisite — do this FIRST before writing any tests):
- [x] **Create `rc_insert/tests.rs`**: add file at `compiler/ori_arc/src/rc_insert/tests.rs`, add `#[cfg(test)] mod tests;` declaration at the bottom of `compiler/ori_arc/src/rc_insert/mod.rs`. Currently `mod.rs` only has `mod annotate;` and `pub use` — the new line goes after the `pub use`.
- [x] **Convert `arg_ownership.rs` to directory**: rename `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` → `compiler/ori_arc/src/aims/emit_rc/arg_ownership/mod.rs`, create `compiler/ori_arc/src/aims/emit_rc/arg_ownership/tests.rs`, add `#[cfg(test)] mod tests;` in the new `mod.rs`. Update the `mod arg_ownership;` in `compiler/ori_arc/src/aims/emit_rc/mod.rs` — no change needed there since Rust resolves both `arg_ownership.rs` and `arg_ownership/mod.rs` the same way.

**Rust unit tests** (in `compiler/ori_arc/src/rc_insert/tests.rs`):

Resolver-level tests — these cover `annotate_arg_ownership()` / `resolve_indirect_arg_ownership()` directly by constructing minimal `ArcFunction` values with the required instruction patterns:
- [x] `test_annotate_apply_indirect_from_partial_apply`: construct an `ArcFunction` with a `PartialApply` creating a closure and an `ApplyIndirect` calling it. Provide a matching `AnnotatedSig` in the `sigs` map (not `MemoryContract` — `annotate_arg_ownership` takes `sigs`, not contracts). Verify `arg_ownership` is populated with the correct ownership from the sig's user-arg params.
- [x] `test_annotate_apply_indirect_opaque_closure`: `ApplyIndirect` where the closure var is a function parameter (not traceable to `PartialApply`). Verify result is all-`Borrowed`.
- [x] `test_annotate_invoke_indirect`: same pattern for `InvokeIndirect` terminator — verify ownership populated from sig.
- [x] `test_annotate_apply_indirect_with_captures_offset`: `PartialApply` with 2 captures, sig has 4 params. Verify `arg_ownership` for the `ApplyIndirect` (2 user args) comes from sig params `[2..4]`, NOT `[0..2]`.
- [x] `test_annotate_apply_indirect_builtin_partial_apply`: partially apply a builtin with receiver-consuming override (`push`, `add`, `concat`, or `remove`) and verify the indirect-call user args inherit the same ownership as a direct call after capture slicing.
- [x] `test_annotate_apply_indirect_zero_capture_function_ref`: zero-capture `PartialApply`/function-ref closure should annotate exactly like a direct call, including external/runtime and builtin behavior.
- [x] `test_annotate_apply_indirect_alias_across_blocks`: closure var reaches the call site through a cross-block alias chain or block param; verify same-target merges annotate correctly.
- [x] `test_annotate_apply_indirect_merge_conflict_defaults_borrowed`: two different closures flow to one block param and are called indirectly; verify the resolver falls back to all-`Borrowed` instead of picking one arbitrarily.
- [x] `test_annotate_apply_indirect_loop_carried_block_param`: loop-carried/block-param closure resolution terminates (no infinite recursion) and either preserves the resolved ownership for equivalent backedges or explicitly falls back to all-`Borrowed` when equivalence cannot be proven.
- [x] `test_annotate_apply_indirect_zero_user_args`: `ApplyIndirect` calling a thunk (zero user args after captures). Verify result is empty `Vec` and no def-map lookup is attempted.
- [x] **Negative pin**: `test_annotate_apply_indirect_opaque_not_owned` — verify opaque closure does NOT produce `Owned` for any arg (pins the conservative default).

**AIMS entry-point tests** (in `compiler/ori_arc/src/aims/emit_rc/arg_ownership/tests.rs`):
- [x] `test_emit_arg_ownership_indirect_reuses_monomorphized_merge`: populate `contracts` with only monomorphized lambda names (or multiple monomorphizations merged conservatively), run `emit_arg_ownership()`, and verify the indirect call site sees the normalized ownership without a duplicate raw-contract fallback path.
- [x] `test_emit_arg_ownership_indirect_debug_assert_invariant`: verify the new post-annotation `debug_assert!` invariant by checking that non-empty indirect arg lists receive non-empty `arg_ownership` after `emit_arg_ownership()`.

**Soundness proof test** (in `compiler/ori_arc/src/rc_insert/tests.rs` or as AOT test):
- [x] **Opaque higher-order wrapper proof**: add one end-to-end test where a closure is passed through a function parameter and called indirectly (`ApplyIndirect` closure source = function param, not resolvable `PartialApply`). Verify the chosen all-`Borrowed` fallback is actually RC-balanced for the wrapper pattern being relied on here (for example, wrapper forwards into a curried closure or returns a closure built from the borrowed arg). If this test fails, Section 02 scope must expand to model higher-order closure contracts rather than relying on the opaque fallback.

**Semantic pin**:
- [x] ARC IR dump test showing `ApplyIndirect` with `[own, borrow]` annotations for a curried closure — use `ORI_DUMP_AFTER_ARC=1` on a test program and verify the output format. Verified: `%8 = ApplyIndirect %7(%6 [own])` and `%9 = ApplyIndirect %8(%2 [borrow])` on curried capture list test.

**Matrix testing** — the fix must be verified across ALL closure patterns (these are existing AOT test files in `compiler/ori_llvm/tests/aot/`):

| Pattern | Test file (under `fixtures/`) | What to verify |
|---------|-----------|---------------|
| Curried capture (list) | `arc/arc_curried_closure_capture_list.ori` | Zero leaks, exit 0 |
| Curried capture (str) | `arc/arc_curried_closure_capture_str.ori` | Zero leaks, exit 0 |
| Curried capture (nested) | `arc/arc_curried_closure_capture_nested.ori` | Zero leaks, exit 0 |
| Nested borrowed param (list) | `higher_order/nested_closure_borrowed_list_param.ori` | Zero leaks, exit 0 |
| Nested borrowed param (str) | `higher_order/nested_closure_borrowed_str_param.ori` | Zero leaks, exit 0 |
| Triple nested | `higher_order/triple_nested_closure_capture.ori` | Zero leaks, exit 0 |
| Closure with user arg (borrowed) | `fat_matrix/f05_closure_param/fm_closure_param_str_heap.ori` | Zero leaks, exit 0 |
| Scalar capture (negative pin) | `arc/arc_curried_closure_scalar_no_inc.ori` | No RcInc for scalars |

- [x] **Verify RC trace balance**: `ORI_TRACE_RC=1` on the curried repro shows balanced inc/dec

**Build mode verification** (per CLAUDE.md Fix Completeness — debug AND release must pass):
- [x] **Debug build**: `timeout 150 cargo t -p ori_arc` — all new unit tests pass in debug
- [x] **Release build**: `timeout 150 cargo test --release -p ori_arc` — all new unit tests pass in release (FastISel behavior differs between modes; ownership annotation is IR-level, but release-mode test coverage catches optimizer-sensitive codegen paths downstream)

## 02.R Third Party Review Findings

These findings were raised during pre-implementation plan review. The plan text (02.1, 02.2) has been rewritten to incorporate all three resolutions. The checkboxes below track whether the **implemented code** satisfies each finding's requirement — check them during `/tpr-review` after implementation.

- [x] `[TPR-02-001][major]` `plans/closure-ownership/section-02-ownership-propagation.md:56-108` — the original draft proposed reading raw `MemoryContract`s directly for indirect calls, which would drift from `emit_arg_ownership()`'s monomorphization merge and from `annotate_arg_ownership()`'s builtin/type-qualified override logic.
  Resolved: Fixed on 2026-04-05. `resolve_indirect_arg_ownership()` calls `compute_arg_ownership()` + `apply_consuming_overrides()` on the full `[captures..., user_args...]` arg list via the same `AnnotatedSig` path as direct calls. Verified by `test_emit_arg_ownership_indirect_reuses_monomorphized_merge`.
- [x] `[TPR-02-002][major]` `plans/closure-ownership/section-02-ownership-propagation.md:97-108` — `capture_args.len()` as an offset is only valid after computing ownership on the full logical arg list. Slicing raw contract params first is wrong for partially applied builtins whose semantics are fixed up by `apply_consuming_overrides()`.
  Resolved: Fixed on 2026-04-05. Ownership computed on full arg list first, then `ownership.drain(..capture_count)` slices off captures. Verified by `test_annotate_apply_indirect_with_captures_offset` and `test_annotate_apply_indirect_builtin_partial_apply`.
- [x] `[TPR-02-003][major]` `plans/closure-ownership/section-02-ownership-propagation.md:97-102` — "walk blocks backward" is not a robust closure resolver in SSA form and does not define behavior for block params / merged closures.
  Resolved: Fixed on 2026-04-05. SSA def-map resolver in `closure_resolve.rs` handles aliases, block params (with per-predecessor visited sets), and loop-carried cycles. Verified by `test_annotate_apply_indirect_alias_across_blocks`, `test_annotate_apply_indirect_merge_conflict_defaults_borrowed`, `test_annotate_apply_indirect_loop_carried_block_param`, and `test_annotate_apply_indirect_diamond_cfg_same_origin`.

- [x] `[TPR-02-004][high]` `compiler/ori_arc/src/rc_insert/closure_resolve.rs:153` — block-param merging currently treats "same callee name + same capture count" as equivalent and keeps the first predecessor's capture list, but the effective indirect-call ownership can still differ after builtin/type-qualified overrides.
  Resolved: Fixed on 2026-04-05. Changed merge comparison from `existing_captures.len() != new_captures.len()` to `*existing_captures != new_captures` (compares actual capture args, not just count). Added regression test `test_annotate_apply_indirect_different_capture_args_defaults_borrowed`.

- [x] `[TPR-02-005][high]` `compiler/ori_arc/src/rc_insert/closure_resolve.rs:122` — the resolver uses one shared `visited` set across all predecessor branches, so compatible merges through distinct aliases of the same closure incorrectly collapse to the opaque all-Borrowed fallback.
  Resolved: Fixed on 2026-04-05. Changed to clone `visited` per-predecessor traversal (`let mut pred_visited = visited.clone()`) so alias paths don't contaminate each other. Added regression test `test_annotate_apply_indirect_diamond_cfg_same_origin`.

- [x] `[TPR-02-006][high]` `compiler/ori_llvm/tests/aot/fixtures/arc/arc_opaque_closure_wrapper.ori:9` — the new "opaque higher-order wrapper proof" does not exercise any RC-managed values, so it cannot validate the all-Borrowed fallback that Section 02 relies on.
  Resolved: Fixed on 2026-04-05. Replaced int-only fixture with str-capturing closure + str user arg. RC trace confirms 1 alloc / 1 dec / 1 free / live=0. `ORI_CHECK_LEAKS=1` passes.

- [x] `[TPR-02-007][medium]` `compiler/ori_arc/src/borrow/update.rs:275` — `InvokeIndirect` borrow inference was changed, but the new behavior has no dedicated unit coverage.
  Resolved: Fixed on 2026-04-05. Added `invoke_indirect_empty_ownership_all_borrowed` and `invoke_indirect_owned_arg_promoted` tests to `borrow/tests.rs`, mirroring the ApplyIndirect coverage.

- [x] `[TPR-02-008][high]` `compiler/ori_arc/src/rc_insert/closure_resolve.rs` — block-param merging now compares raw capture-var identity, which is still too strict for type-qualified builtins.
  Resolved: Fixed on 2026-04-05. Changed merge comparison from var-identity to type-based via `captures_same_types()` helper. Same-type captures merge; different-type captures fall back. Threaded `var_types` through `resolve_to_partial_apply`. Regression: `test_annotate_apply_indirect_same_capture_types_merges`.

- [x] `[TPR-02-009][medium]` `compiler/ori_arc/src/rc_insert/tests.rs` — `test_annotate_apply_indirect_different_capture_args_defaults_borrowed` does not exercise the type-qualified ownership hazard.
  Resolved: Fixed on 2026-04-05. Renamed to `test_annotate_apply_indirect_different_capture_types_defaults_borrowed`, uses `List<int>` vs `str` capture types to exercise the actual type-divergent hazard. Added companion `test_annotate_apply_indirect_same_capture_types_merges` for the same-type merge case.

- [x] `[TPR-02-010][medium]` `compiler/ori_arc/src/rc_insert/tests.rs:693` — the replacement TPR-02-009 regression still does not exercise the type-qualified ownership hazard it claims to cover.
  Resolved: Fixed on 2026-04-05. Updated test to register `concat` in `consuming_receiver` and `consuming_second_arg` builtin sets so `apply_consuming_overrides` actually fires for List captures but not str captures, exercising the real type-qualified divergence path.

## 02.N Completion Checklist

- [x] `ResolvedDef` enum defined (02.1)
- [x] `build_closure_def_map()` implemented with two-pass block-param handling (02.1)
- [x] `annotate_arg_ownership()` handles `ApplyIndirect` via `resolve_indirect_arg_ownership` (02.1)
- [x] `annotate_arg_ownership()` handles `InvokeIndirect` via `resolve_indirect_arg_ownership` (02.1)
- [x] Indirect calls reuse the same normalized ownership lookup as direct calls (02.1)
- [x] `resolve_indirect_arg_ownership()` traces closure to `PartialApply` source through SSA defs (02.2)
- [x] Block-param / merged-closure cases handled conservatively (02.2)
- [x] Resolver terminates on loop-carried/block-param cycles and does not recurse indefinitely (02.2)
- [x] Capture offset handled correctly AFTER full-arg ownership computation (02.2)
- [x] No duplicate monomorphized-name resolution path added for indirect calls (02.2)
- [x] Opaque closures default to all-Borrowed (02.2)
- [x] `debug_assert!` verifies non-empty `arg_ownership` after annotation (02 preamble)
- [x] Doc comments updated: `annotate_arg_ownership()` and `arg_ownership.rs` module doc include ApplyIndirect/InvokeIndirect (02 preamble)
- [x] Legacy borrow inference updated for `InvokeIndirect` (02.3)
- [x] Legacy borrow inference updated for `ApplyIndirect` precision — no longer promotes closure operand (02.3)
- [x] `ArgOwnership` import added to `borrow/update.rs` (02.3)
- [x] Stale comment at `borrow/update.rs:272-273` removed (02.3)
- [x] Dead binding `let _ = value;` at `borrow/update.rs:241` removed (02.3)
- [x] Cross-reference verified: `collect_borrowed_call_args()` refinement tracked in Section 03.1 (02.3)
- [x] Test module wiring: `rc_insert/tests.rs` created with `mod tests;` in `rc_insert/mod.rs` (02.4)
- [x] Test module wiring: `arg_ownership.rs` converted to `arg_ownership/mod.rs` + `arg_ownership/tests.rs` (02.4)
- [x] Unit tests include: partial-apply resolution, opaque closure, InvokeIndirect, capture-offset, builtin partial-apply, zero-capture function-ref, cross-block alias, merge conflict, loop-carried cycle, zero-user-args thunk, negative pin (02.4)
- [x] AIMS entry-point tests: monomorphization merge, debug_assert invariant (02.4)
- [x] Opaque higher-order wrapper proof test passes (02.4)
- [x] All 8 closure pattern tests pass with zero leaks (02.4)
- [x] `annotate.rs` stays under 500 lines (if not, extract `closure_resolve.rs`) (02 preamble)
- [x] Debug AND release `cargo t -p ori_arc` pass (02.4)
- [x] `timeout 150 cargo t` passes
- [x] `timeout 150 ./test-all.sh` passes
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed
- [ ] **Plan sync**: Section 02 frontmatter `status:` updated to `complete`
- [ ] **Plan sync**: `00-overview.md` frontmatter `status:` still `in-progress` (Section 03 remains)
- [ ] **Plan sync**: `index.md` Quick Reference table updated (Section 02 status → Complete)
- [ ] **Plan sync**: Section 03 `depends_on: ["01", "02"]` verified accurate
- [ ] Block-param merge accepts distinct capture vars when they produce the same effective indirect-call ownership (for example, same callee + same receiver type for type-qualified builtins) instead of falling back on raw SSA inequality
- [ ] Replace the current `different_capture_args_defaults_borrowed` regression with typed builtin coverage that distinguishes same-semantics merges from true ownership conflicts
