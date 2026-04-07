---
section: "01"
title: "ARC IR Shape"
status: complete
reviewed: true
goal: "Add arg_ownership fields to ApplyIndirect and InvokeIndirect, update all IR queries and utilities"
success_criteria:
  - "ApplyIndirect and InvokeIndirect carry arg_ownership: Vec<ArgOwnership>"
  - "is_owned_position() respects arg_ownership for indirect calls (instruction and terminator)"
  - "emit_terminator_rc() handles InvokeIndirect project-borrowed RcInc via arg_ownership"
  - "is_var_defined_in_block() handles InvokeIndirect (defines dst in normal successor)"
  - "ARC IR dump shows arg_ownership for indirect calls"
  - "ARC IR verifier checks arg_ownership length matches args length"
  - "All existing tests pass unchanged"
inspired_by:
  - "Existing Apply/Invoke arg_ownership pattern in compiler/ori_arc/src/ir/instr.rs"
  - "Lean 4 standard vs borrowed calling conventions"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-05
sections:
  - id: "01.1"
    title: "Add arg_ownership to ApplyIndirect"
    status: complete
  - id: "01.2"
    title: "Add arg_ownership to InvokeIndirect"
    status: complete
  - id: "01.3"
    title: "Update IR queries and utilities"
    status: complete
  - id: "01.4"
    title: "Tests and verification"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: ARC IR Shape

**Goal:** Add `arg_ownership` fields to `ApplyIndirect` and `InvokeIndirect` in the ARC IR, following the exact pattern used by `Apply` and `Invoke`.

## 01.1 Add arg_ownership to ApplyIndirect

- [x] **Add field** to `ArcInstr::ApplyIndirect` in `compiler/ori_arc/src/ir/instr.rs:41-46`:
  ```rust
  ApplyIndirect {
      dst: ArcVarId,
      ty: Idx,
      closure: ArcVarId,
      args: Vec<ArcVarId>,
      arg_ownership: Vec<ArgOwnership>,  // NEW
  },
  ```
- [x] **Update match arms that destructure `ApplyIndirect`** across `ori_arc`. Use `cargo build -p ori_arc` — exhaustive match will flag every site. Sites that **must change**:
  - `ir/instr.rs`: `is_owned_position()` (line 313) — **major change**, see 01.3
  - `lower/builder/emission.rs`: `emit_apply_indirect()` (line 49) — add `arg_ownership: Vec::new()` to construction (empty vec = pre-annotation state, populated in Section 02)
  - `oric/src/arc_dump/instr.rs` (line 57-75): dump format — **must update** to show ownership annotations (see 01.3)
- [x] **Sites that use `..` binding** (verify no change needed — `cargo build` confirms):
  - `ir/instr.rs`: `used_vars()` (line 212), `defined_var()` (line 170), `substitute_var()` (line 369), `uses_var()` (line 271) — all use `..` binding
  - `aims/transfer/mod.rs`: `transfer_arg_demand()` (line 266), `transfer_def()` (line 80) — `..` binding
  - `aims/normalize/verify.rs` (line 332), `borrow/update.rs` (line 213), `borrow/derived.rs` (line 122), `graph/call_graph/mod.rs` (line 84 wildcard arm)
  - `aims/realize/walk_dec.rs` (line 145), `aims/realize/walk.rs` (line 223): consume via `is_owned_position()` — no direct match on `ApplyIndirect`
  - **Note**: `forward_walk.rs`, `unwind_cleanup.rs`, `dead_cleanup.rs`, `realize/mod.rs` do NOT reference `ApplyIndirect` — no changes needed there
- [x] **Default initialization**: New `arg_ownership` defaults to empty `Vec` at construction (same as `Apply`'s pre-annotation state). The AIMS pipeline populates it in Section 02.

## 01.2 Add arg_ownership to InvokeIndirect

- [x] **Add field** to `ArcTerminator::InvokeIndirect` in `compiler/ori_arc/src/ir/mod.rs:276-283`:
  ```rust
  InvokeIndirect {
      dst: ArcVarId,
      ty: Idx,
      closure: ArcVarId,
      args: Vec<ArcVarId>,
      arg_ownership: Vec<ArgOwnership>,  // NEW
      normal: ArcBlockId,
      unwind: ArcBlockId,
  },
  ```
- [x] **Update match arms that destructure `InvokeIndirect`** — sites that **must change**:
  - `lower/builder/mod.rs`: `terminate_invoke_indirect()` (line 322) — add `arg_ownership: Vec::new()` to `ArcTerminator::InvokeIndirect` construction (line 337-344)
  - `oric/src/arc_dump/instr.rs` (lines 312-329): dump format — replace `fmt_args_simple(out, args, func)` with `fmt_args_with_ownership(out, args, arg_ownership, func)` (see 01.3)
  - `aims/realize/emit_unified.rs` (484 lines — at limit, do NOT add unrelated code): `is_var_defined_in_block()` (line 478) — add `InvokeIndirect` check after the existing `Invoke` check:
    ```rust
    if let ArcTerminator::InvokeIndirect { dst, .. } = &block.terminator {
        if *dst == var {
            return true;
        }
    }
    ```
    **Rationale**: `InvokeIndirect`, like `Invoke`, defines `dst` in its normal successor block. The current code only checks `Invoke`, causing the pipeline to miss `InvokeIndirect` defs — this is a latent bug that becomes visible once `arg_ownership` enables proper RC emission for indirect invoke sites.
  - `aims/emit_rc/forward_walk.rs`: `emit_terminator_rc()` (lines 31-54) — add a matching `InvokeIndirect` branch after the existing `Invoke` branch:
    ```rust
    if let ArcTerminator::InvokeIndirect { args, arg_ownership, .. } = terminator {
        for (pos, &var) in args.iter().enumerate() {
            let is_owned = arg_ownership
                .get(pos)
                .is_some_and(|o| *o == ArgOwnership::Owned);
            if is_owned
                && ctx.project_borrowed_defs.contains(&var)
                && ctx.func.var_reprs[var.index()] != ValueRepr::Scalar
            {
                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    new_body.push(ArcInstr::RcInc { var, count: 1, strategy });
                }
            }
        }
    }
    ```
    **Note**: `InvokeIndirect`'s `used_vars()` returns `[...args, closure]` (closure LAST). But `arg_ownership` parallels `args` directly (no offset), so we iterate `args` with direct index. Also update the comment at line 82 to include InvokeIndirect in the list of ownership-transferring terminators.
- [x] **Sites that use `..` binding** (verify no change needed — `cargo build` confirms):
  - `ir/mod.rs`: `used_vars()` (line 311), `uses_var()` (line 330), `substitute_var()` (line 357) — all use `..` binding
  - `borrow/update.rs`: InvokeIndirect branch (lines 272-274) — currently empty `{}`. Section 02.3 handles updating this for borrow inference.
  - `aims/transfer/mod.rs`: `transfer_def_terminator()` (line 367-372) — `..` binding
  - `aims/emit_rc/trampoline.rs`: lines 89 and 136 — `..` binding
  - `aims/interprocedural/mod.rs`: line 513 — `..` binding
  - `block_merge/compact.rs`: line 136 — `..` binding
  - `graph/mod.rs`: lines 73 and 88 — `..` binding
  - **Note**: `unwind_cleanup.rs`, `dead_cleanup.rs`, `realize/mod.rs` do NOT reference `InvokeIndirect` — no changes needed there

## 01.3 Update IR queries and utilities

- [x] **Fix `is_owned_position()`** in `compiler/ori_arc/src/ir/instr.rs:313-342`:
  - Replace the hardcoded `false` for `ApplyIndirect` with `arg_ownership`-based lookup:
    ```rust
    ArcInstr::ApplyIndirect { args, arg_ownership, .. } => {
        // pos indexes into used_vars(): pos=0 is closure (always borrowed),
        // pos 1..=args.len() are user args. arg_ownership parallels args,
        // so arg_ownership[i] corresponds to used_vars position i+1.
        if pos == 0 {
            return false; // closure is always borrowed
        }
        let arg_idx = pos - 1;
        // Empty arg_ownership (pre-annotation) → is_some_and returns false →
        // all positions NOT owned. This is safe (conservative: caller retains
        // cleanup). Differs from Apply's `is_none_or` which defaults to Owned.
        arg_idx < args.len()
            && arg_ownership
                .get(arg_idx)
                .is_some_and(|o| *o == ArgOwnership::Owned)
    },
    ```
  - **WARNING**: This does NOT match the `Apply` pattern directly. `Apply`'s `used_vars()` returns only `args` (no closure field), so `pos` maps directly to `args[pos]`. For `ApplyIndirect`, `used_vars()` prepends the closure at position 0, creating a +1 offset. Existing tests (`is_owned_position_apply_indirect_closure_is_borrowed` in `ir/tests.rs:1147`) validate this.
- [x] **Add `is_owned_position()` for terminators**: `ArcTerminator` currently has no `is_owned_position()` method. Add one to handle `InvokeIndirect` (and `Invoke`, which has `arg_ownership` already). **CAUTION**: `InvokeIndirect`'s `used_vars()` returns `[...args, closure]` (closure at END, not beginning — opposite of `ApplyIndirect`'s `used_vars()` which puts closure first). The position-to-arg mapping must account for this asymmetry. **Note**: The realization pass (`walk.rs`, `walk_dec.rs`) only calls `is_owned_position` on body instructions — terminator RC is handled separately via `terminator_deferred` and edge cleanup. The new terminator `is_owned_position()` may need to be called from the terminator RC emission path (check `emit_unified.rs` and the edge cleanup logic).
- [x] **Update ARC IR dump** (`ORI_DUMP_AFTER_ARC=1`): display `arg_ownership` annotations for indirect calls, matching the format used for `Apply`/`Invoke`. Two files to update in `compiler/oric/src/arc_dump/instr.rs` (459 lines, under 500 limit):
  - `ApplyIndirect` dump (lines 57-75): change destructure to include `arg_ownership`, replace `fmt_args_simple(out, args, func)` (line 90) with `fmt_args_with_ownership(out, args, arg_ownership, func)`
  - `InvokeIndirect` dump (lines 312-329): change destructure to include `arg_ownership`, replace `fmt_args_simple(out, args, func)` (line 327) with `fmt_args_with_ownership(out, args, arg_ownership, func)`
- [x] **Update ARC IR verifier** (`compiler/ori_arc/src/verify/mod.rs`, 369 lines): add a new `VerifyError` variant (e.g., `ArgOwnershipLenMismatch { block, expected, actual }`) and add a check that `arg_ownership.len() == args.len()` when `arg_ownership` is non-empty, for both `ApplyIndirect` (instruction loop) and `InvokeIndirect` (terminator check). The verifier currently has no `arg_ownership` checks at all — this is a new addition. Note: `aims/normalize/verify.rs` is the TRMC post-rewrite verifier, NOT the general ARC IR verifier.
- [x] **Update serialization** (if `cache` feature is enabled): `arg_ownership` needs serde derives — already handled by the `#[cfg_attr(feature = "cache", derive(...))]` on `ArcInstr`. Verify this also covers `ArcTerminator` for `InvokeIndirect`.

## 01.4 Tests and verification

**TDD order**: Write tests first, verify they fail (compilation errors from missing field are acceptable "failures"), then implement.

- [x] **Update existing tests** in `compiler/ori_arc/src/ir/tests.rs` (1215 lines — test file, no 500-line limit):
  - `is_owned_position_apply_indirect_closure_is_borrowed` (line 1147): add `arg_ownership: vec![]` to the `ApplyIndirect` construction (line 1151-1156). Test should STILL pass (empty vec = all Borrowed via `is_some_and`).
  - `is_owned_position_apply_indirect_no_args` (line 1175): add `arg_ownership: vec![]` to construction (line 1176-1181). Test should STILL pass.
- [x] **Write new Rust unit tests** in `compiler/ori_arc/src/ir/tests.rs`:
  - `test_apply_indirect_is_owned_position_empty`: empty `arg_ownership` → all positions return `false` (conservative default — uses `is_some_and` not `is_none_or`, so empty vec = all Borrowed)
  - `test_apply_indirect_is_owned_position_owned`: `arg_ownership = [Owned, Borrowed]` → pos 0 (`closure`) is NOT owned, pos 1 (`args[0]`) IS owned, pos 2 (`args[1]`) is NOT owned. Note the +1 offset: `pos` indexes into `used_vars()` where closure is at position 0.
  - `test_apply_indirect_is_owned_position_out_of_bounds`: pos beyond `args.len()` returns `false`
  - `test_invoke_indirect_is_owned_position`: same pattern for `InvokeIndirect` — but closure is at the END of `used_vars()`, so the offset mapping differs. Test: `arg_ownership = [Owned, Borrowed]` → pos 0 (`args[0]`) IS owned, pos 1 (`args[1]`) is NOT owned, pos 2 (`closure`) is NOT owned.
  - **Negative pin**: `test_apply_indirect_is_owned_position_not_default_owned` — verify that `ApplyIndirect` with empty `arg_ownership` does NOT default to Owned (unlike `Apply` which uses `is_none_or`). This pins the conservative-for-indirect semantic.
- [x] **Write verifier test** in `compiler/ori_arc/src/verify/tests.rs`:
  - `test_verify_apply_indirect_arg_ownership_len_mismatch`: construct `ApplyIndirect` with `args.len() = 2` but `arg_ownership.len() = 1` (non-empty mismatch) → verifier returns `ArgOwnershipLenMismatch` error
  - `test_verify_invoke_indirect_arg_ownership_len_mismatch`: same for `InvokeIndirect` terminator
  - `test_verify_apply_indirect_arg_ownership_empty_ok`: empty `arg_ownership` with non-empty `args` → NO error (empty = pre-annotation, valid)
- [x] **Verify all existing tests pass**: `timeout 150 cargo t -p ori_arc` — exhaustive match ensures no missed sites
- [x] **Verify ARC IR dump** shows `arg_ownership` annotations: `ORI_DUMP_AFTER_ARC=1 cargo run -- build /tmp/test.ori`

## 01.R Third Party Review Findings

- [x] `[TPR-01-001][major]` `edge_cleanup.rs:73,278,312,428` — InvokeIndirect missing from invoke-specific RC edge cleanup.
  Resolved: Fixed on 2026-04-05. Added InvokeIndirect to collect_invoke_edge_decs with correct is_some_and default (Borrowed for indirect, Owned for direct). Added is_indirect flag to invoke_transfers_ownership helper.
- [x] `[TPR-01-002][major]` `dead_cleanup.rs:174,185,248` — InvokeIndirect missing from dead-dst fallback sweep.
  Resolved: Fixed on 2026-04-05. Added InvokeIndirect to emit_dead_invoke_dsts and is_var_defined_in_block.
- [x] `[TPR-01-003][minor]` `ir/tests.rs:1223,1283,1326` — Decorative `// --- ... ---` banner comments.
  Resolved: Fixed on 2026-04-05. Replaced with plain section comments.
- [x] `[TPR-01-004][major]` `edge_cleanup.rs:312,398,428` — is_none_or ownership default wrong for InvokeIndirect (defaults empty to Owned instead of Borrowed).
  Resolved: Fixed on 2026-04-05 (2 iterations). Added is_indirect detection: Category 3 uses is_some_and, Category 2 uses is_none_or for borrowed detection, invoke_transfers_ownership parameterized.
- [x] `[TPR-01-005][medium]` `drop_hints.rs:95` — InvokeIndirect missing from collect_borrowed_call_args terminator scan.
  Resolved: Pre-existing gap, already tracked in Section 03.1. No regression from Section 01 changes.
- [x] `[TPR-01-006][medium]` `unwind_cleanup.rs:57` — InvokeIndirect missing from unwind iterator cleanup.
  Resolved: Pre-existing gap, tracked for Section 03. No regression from Section 01 changes.

## 01.N Completion Checklist

- [x] `ApplyIndirect` has `arg_ownership` field (01.1)
- [x] `InvokeIndirect` has `arg_ownership` field (01.2)
- [x] `is_owned_position()` uses `arg_ownership` for `ApplyIndirect` with +1 offset (01.3)
- [x] `is_owned_position()` added for `ArcTerminator` to handle `InvokeIndirect` (01.3)
- [x] `emit_terminator_rc()` handles `InvokeIndirect` project-borrowed RcInc (01.2)
- [x] `is_var_defined_in_block()` handles `InvokeIndirect` (01.2)
- [x] ARC IR dump shows ownership annotations for both `ApplyIndirect` and `InvokeIndirect` (01.3)
- [x] Verifier checks `arg_ownership` length with new error variant (01.3)
- [x] Existing `is_owned_position` tests updated with `arg_ownership` field (01.4)
- [x] New unit tests for `is_owned_position` with indirect calls including negative pin (01.4)
- [x] Verifier tests for `arg_ownership` length mismatch (01.4)
- [x] `timeout 150 cargo t` passes — no regressions
- [x] `timeout 150 ./test-all.sh` passes
- [x] `/tpr-review` passed — clean on iteration 4 (6 findings fixed across 3 iterations)
- [x] `/impl-hygiene-review` passed — 3 findings fixed (algorithmic duplication, BLOAT, import ordering)
