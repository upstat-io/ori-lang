---
section: "04"
title: "Borrow Inference Hardening"
status: complete
goal: "Eliminate silent failures in borrow inference integration with codegen; classify call-site argument ownership"
sections:
  - id: "04.1"
    title: "Warn on borrow signature lookup miss"
    status: complete
  - id: "04.2"
    title: "Eliminate unqualified method dispatch fallback"
    status: complete
  - id: "04.3"
    title: "Add debug_assert coverage"
    status: complete
  - id: "04.4"
    title: "Embed call-site argument ownership in ARC IR"
    status: complete
---

# Section 04: Borrow Inference Hardening

**Status:** Not Started
**Goal:** No silent fallbacks — every borrow signature miss is logged, every method lookup is O(1), and every call site carries per-argument ownership in the IR.

**Context:** `FunctionCompiler` receives `annotated_sigs: &FxHashMap<Name, AnnotatedSig>` and looks up each function by `Name`. A miss means the function compiles with all-Owned parameters (no borrow optimization) — silently. This is correct but wasteful, and misses indicate pipeline bugs. Additionally, `lookup_method_by_unqualified_name` does a linear scan as fallback.

**Correctness context (from 2026-02-22 session):** Beyond the optimization concern, missing borrow info caused **RC leaks**. External C runtime functions (`ori_*`) borrow all args without decrementing — the caller must emit `RcDec` after the call. But the RC inserter was treating all call sites as consuming (ownership transfer), skipping the caller-side `RcDec`. The fix was a 263-line patch to `rc_insert/mod.rs` adding `is_external_callee` detection and `insert_external_invoke_cleanup`. Section 04.4 elevates this from a runtime detection hack into a proper IR contract.

---

## 04.1 Warn on Borrow Signature Lookup Miss

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

- [x] In `define_all()`, when looking up a function's `AnnotatedSig`:
  ```rust
  let sig = match annotated_sigs.get(&func_name) {
      Some(sig) => sig,
      None => {
          tracing::warn!(
              func = %func_name,
              "borrow signature missing — compiling with all-Owned params"
          );
          &default_all_owned_sig
      }
  };
  ```

- [x] Add `debug_assert!` that all functions being compiled have entries:
  ```rust
  #[cfg(debug_assertions)]
  {
      let missing: Vec<_> = functions_to_compile
          .iter()
          .filter(|name| !annotated_sigs.contains_key(name))
          .collect();
      debug_assert!(
          missing.is_empty(),
          "functions missing borrow sigs: {:?}",
          missing
      );
  }
  ```

- [x] Verify this fires correctly by temporarily removing a signature and checking the warning appears

---

## 04.2 Eliminate Unqualified Method Dispatch Fallback

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

**Discovery:** Empirical testing (428 AOT + 3895 spec tests) proved the O(n) `lookup_method_by_unqualified_name` linear scan is **dead code**. The two-tier O(1) lookup chain (`functions.get()` → `lookup_method_by_receiver()`) already resolves 100% of method calls. Reason: `declare_and_bind_derive` and `compile_impl_method_from_sig` both insert into `functions` (unqualified) AND `method_functions` (type-qualified), so tier 1 catches everything tier 3 would catch.

**Resolution:** Instead of adding a secondary index (original plan), the linear scan was replaced with a diagnostic-guarded fallback (`lookup_method_fallback`) that emits `tracing::error!` + `debug_assert!(false)` if ever reached. This is the correct architecture: the existing two-tier lookup IS already O(1), and any future case that reaches the fallback indicates a registration gap that should be fixed at the source.

- [x] Verified `lookup_method_by_unqualified_name` is never reached (0 hits across full test suite)
- [x] Replaced linear scan with `lookup_method_fallback` diagnostic guard
- [x] All 428 AOT + 358 unit + 57 runtime tests pass with `debug_assert!` active

---

## 04.3 Add Debug Assert Coverage

**Files:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `arc_emitter/mod.rs`

**Approach:** Asserts placed at **registration time** (not lookup time), verifying round-trip retrievability of `type_idx_to_name` and `method_functions` entries immediately after insertion. Lookup-time asserts were rejected because `None` is a valid outcome in operator trait dispatch (means "no impl, fall back to built-in"). The fallback diagnostic guard in 04.2 covers the emitter lookup side.

- [x] Assert `type_idx_to_name` and `method_functions` round-trip in `compile_impl_method_from_sig`
- [x] Assert `type_idx_to_name` and `method_functions` round-trip in `declare_and_bind_derive`
- [x] Run `./llvm-test.sh` with debug assertions enabled — 428 AOT + 358 unit + 57 runtime: 0 failures

---

## 04.4 Embed Call-Site Argument Ownership in ARC IR

**Files:** `compiler/ori_arc/src/ir/mod.rs`, `compiler/ori_arc/src/rc_insert/mod.rs`

**Context:** In the Perceus model, a call site either *consumes* an argument (callee takes ownership, caller doesn't Dec) or *borrows* it (callee reads without consuming, caller must Dec after call). Today this distinction is re-derived at RC insertion time by checking:
1. Is the callee external (`ori_*` prefix)? → all args borrowed
2. Does the callee's `AnnotatedSig` mark the param as `Borrowed`? → that arg borrowed
3. Otherwise → owned (consuming)

This logic lives in `is_external_callee()` and `is_borrowing_instr()` in `rc_insert/mod.rs` — it queries the interner and sigs map at insertion time. The result should be embedded in the IR instruction so RC insertion and the emitter can both consume it without re-derivation.

- [x] Define per-arg ownership annotation:
  ```rust
  /// Ownership of a single argument at a call site.
  /// Computed during lowering or RC insertion from AnnotatedSig + callee classification.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum ArgOwnership {
      /// Callee consumes: ownership transfers. Caller emits RcInc if live-after.
      Owned,
      /// Callee borrows: reads without consuming. Caller must RcDec at last use.
      Borrowed,
  }
  ```

- [x] Add `arg_ownership: Vec<ArgOwnership>` to `Invoke` and `Apply` terminators/instructions:
  ```rust
  ArcTerminator::Invoke {
      dst: ArcVarId,
      ty: Idx,
      func: Name,
      args: Vec<ArcVarId>,
      arg_ownership: Vec<ArgOwnership>,  // NEW — parallel to args
      normal: ArcBlockId,
      unwind: ArcBlockId,
  },
  ```

- [x] Compute `arg_ownership` during RC insertion (already has access to `sigs` and `interner`):
  ```rust
  fn compute_arg_ownership(
      callee: Name,
      args: &[ArcVarId],
      sigs: &FxHashMap<Name, AnnotatedSig>,
      interner: &StringInterner,
  ) -> Vec<ArgOwnership> {
      if is_external_callee(callee, sigs, interner) {
          vec![ArgOwnership::Borrowed; args.len()]
      } else if let Some(sig) = sigs.get(&callee) {
          args.iter().enumerate().map(|(i, _)| {
              if sig.params.get(i).is_some_and(|p| p.ownership == Ownership::Borrowed) {
                  ArgOwnership::Borrowed
              } else {
                  ArgOwnership::Owned
              }
          }).collect()
      } else {
          vec![ArgOwnership::Owned; args.len()]
      }
  }
  ```

### Builtin Method Borrowing

**Critical gap discovered 2026-02-22:** Builtin methods (`is_err`, `is_ok`, `unwrap`, `unwrap_err`, `is_some`, `is_none`, `unwrap_or`, `clone` on Option/Result) are emitted inline by `try_emit_builtin_method` in the LLVM emitter. They **all borrow** their receiver — none consume it. But the RC inserter doesn't know this because:

1. `is_external_callee()` checks for `ori_*` prefix — builtins don't have it → returns `false`
2. `sigs.get(callee)` looks up `AnnotatedSig` — builtins have no entry → falls through
3. `insert_external_invoke_cleanup` hits `continue` at the else branch (line 889) → **no cleanup emitted**

Result: when a `Result<int, str>` is passed to `is_err`, the str inside is never decremented. This is the root cause of the 3 ARC leaks in `never_propagation.ori`.

**The pipeline gap in detail:**
1. The canonical IR desugars `r.is_err()` into `is_err(r)` — a direct `Call` with `CanExpr::Ident("is_err")`, **NOT** a `MethodCall`
2. This goes through `lower_call()` → `CanExpr::Ident(name)` path (line 90), NOT `lower_method_call()`
3. `emit_call_or_invoke()` checks `is_nounwind_call()` — `is_err` has no `ori_*` / `__*` prefix → emits `Invoke`
4. RC inserter sees `Invoke` with callee "is_err" — not external, not in sigs → treats as **consuming** (ownership transfer)
5. LLVM emitter's `try_emit_builtin_method` recognizes "is_err" and emits inline IR (tag check) — no actual function call
6. Result: the RC inserter thinks ownership transferred to a function; the emitter inlined it as a read. **Nobody emits the Dec.**

**Critical discovery (during implementation attempt):** Intercepting in `lower_method_call()` does NOT work because the canonical IR has already desugared the method call to a direct function call. The ARC lowering trace confirms: `call: direct (Ident) func="is_err" args=1`. The interception must happen in `emit_call_or_invoke()` which is the shared entry point for both `lower_call` and `lower_method_call`.

**Fix approach — three options:**

**(a) Lower builtins as PrimOp in the ARC lowerer** (recommended — fixes at the correct level):
In `emit_call_or_invoke()`, recognize known tag-check builtins and emit `Project + PrimOp::Eq` instead of `Invoke`. The interception MUST be in `emit_call_or_invoke` (not `lower_method_call`) because the canonical IR desugars `r.is_err()` to `is_err(r)` as a direct call. The RC inserter already treats PrimOps as borrowing (rc_insert/mod.rs:956).

```rust
// In emit_call_or_invoke():
fn emit_call_or_invoke(
    &mut self,
    ty: Idx,
    name: Name,
    args: Vec<ArcVarId>,
    span: Span,
) -> ArcVarId {
    // Try to lower as a tag-check builtin first.
    // Canonical IR desugars r.is_err() to is_err(r), so this is a direct call.
    if args.len() == 1 {
        if let Some(var) = self.try_lower_tag_check(name, args[0], span) {
            return var;
        }
    }
    if self.is_nounwind_call(name) {
        self.builder.emit_apply(ty, name, args, Some(span))
    } else {
        self.builder.emit_invoke(ty, name, args, Some(span))
    }
}

fn try_lower_tag_check(
    &mut self,
    method: Name,
    receiver: ArcVarId,
    span: Span,
) -> Option<ArcVarId> {
    let method_str = self.name_str(method);
    let receiver_ty = self.builder.var_type(receiver);
    let resolved = self.pool.resolve_fully(receiver_ty);
    let tag = self.pool.tag(resolved);

    // Result: Ok=0, Err=1. Option: Some=0, None=1.
    let target_tag = match (method_str, tag) {
        ("is_ok", Tag::Result) => 0i64,
        ("is_err", Tag::Result) => 1,
        ("is_some", Tag::Option) => 0,
        ("is_none", Tag::Option) => 1,
        _ => return None,
    };

    let tag_var = self.builder.emit_project(Idx::INT, receiver, 0, Some(span));
    let tag_const = self.builder.emit_let(
        Idx::INT, ArcValue::Literal(LitValue::Int(target_tag)), None);
    Some(self.builder.emit_let(
        Idx::BOOL,
        ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Eq),
            args: vec![tag_var, tag_const],
        },
        Some(span),
    ))
}
```

**Why this is best:** The ARC IR accurately represents what the operation IS. `is_err` is a tag read, not a function call. The RC inserter already has correct borrowing semantics for PrimOps — zero additional RC insertion logic needed. The LLVM emitter still inlines the codegen, but now the IR and emitter agree on what the operation does. No `annotated_sigs` registration, no `is_external_callee` extension.

**Trade-off:** Must handle this in `emit_call_or_invoke` since it processes already-desugared calls. The receiver type lookup uses `builder.var_type(receiver)` since by this point we have an `ArcVarId`, not a `CanId`. No new PrimOp variants needed — reuses existing `PrimOp::Binary(BinaryOp::Eq)`.

**Key constraint:** The canonical IR desugars method calls to function calls. `try_lower_tag_check` receives an `ArcVarId` (already lowered), not a `CanId`. Type lookup must use `builder.var_type()` to get the receiver's type from the ARC IR, not `expr_type()` which needs a CanId.

**(b) Register builtin methods in `annotated_sigs`** (pragmatic short-term fix):
During `compile_impls` or ARC pipeline setup, insert synthetic `AnnotatedSig` entries for all builtin methods with `Borrowed` receiver:
```rust
// In function_compiler setup or run_arc_pipeline_all:
for (type_name, method_name) in BUILTIN_METHODS_BORROWING_RECEIVER {
    let sig = AnnotatedSig {
        params: vec![AnnotatedParam { ownership: Ownership::Borrowed }],
        return_type: Idx::BOOL, // varies
    };
    annotated_sigs.insert(method_name, sig);
}
```

This feeds into the ArgOwnership pipeline naturally — once builtins are in `annotated_sigs`, `compute_arg_ownership` handles them automatically. **Downside:** The IR still lies — it claims `is_err` is an `Invoke` (function call) when it's really a tag read. The emitter silently reinterprets it.

**(c) Extend `is_external_callee` to recognize builtins** (least recommended):
Add a `is_builtin_callee()` check alongside `is_external_callee()`:
```rust
fn is_builtin_borrowing(callee: Name, sigs: &FxHashMap<Name, AnnotatedSig>) -> bool {
    BUILTIN_BORROWING_METHODS.contains(&callee)
}
```

**Downside:** Another patch on a detection mechanism that's already shown to be fragile. Adds a third path to the "is this callee borrowing?" logic.

**Recommended path:** Option (a) for the final architecture, with option (b) as an acceptable interim fix if (a) requires too much `PrimOp` expansion upfront. Option (c) is not recommended.

### Project Borrowing — The Lean 4 Model

**Discovery (during implementation):** Option (a) fixes `is_err`/`is_ok` calls in user code, but the `?` operator's own `lower_try` code has the same underlying problem. `lower_try` emits `Project v_tag = v_result.0` to extract the tag, then branches on it. The `Project` instruction is NOT classified as borrowing in `is_borrowing_instr` — so Perceus treats it as consuming v_result. But projecting a scalar tag doesn't actually consume the Result — the inner RC fields (e.g., the string in `Err("hello")`) are never Dec'd.

**This is a separate bug from the builtin method issue.** Option (a) fixes user-written `r.is_err()` calls by lowering them as PrimOps. But the `?` operator's desugaring still uses raw Project instructions that consume without cleanup.

**The principled fix: Lean 4's borrowing projection model.**

From `src/Lean/Compiler/IR/RC.lean`: `proj i x` borrows `x`. If the projected field is an object (RC-typed), emit `Inc` on the result. If scalar, no Inc needed. Either way, `x` stays alive and gets Dec'd at its last use. This is the correct semantic model — projecting a field is never consuming.

**Implementation:**
```rust
// In is_borrowing_instr (rc_insert/mod.rs):
ArcInstr::Project { dst, .. } => ctx.classifier.is_scalar(ctx.func.var_type(*dst)),
```

For non-scalar (RC-typed) projections, Perceus already emits `RcInc` on the result — the projection borrows the parent, and the Inc transfers ownership of the extracted field to the new variable. This is exactly the Lean 4 model.

**Known failure: heap corruption with current liveness analysis.**

The initial implementation attempt (blanket scalar-Project-as-borrowing) crashed with `malloc(): unaligned tcache chunk detected`. This is NOT a problem with the borrowing model — it's a bug in the cross-block liveness analysis that must be fixed.

**The `?` operator's `lower_try` ARC IR pattern (exhibits the bug):**
```
B3 (merge block):
  Project v_tag = INT, v_result, field 0   // scalar → now borrowing
  Branch(v_tag == 0, ok_block, err_block)

B4 (ok_block):
  Project v_ok = Idx(224), v_result, field 1   // RC-typed → Inc emitted
  ... uses v_ok ...
  Jump → ...

B5 (err_block):
  Project v_err = STR, v_result, field 1   // RC-typed → Inc emitted
  RcInc { v_err }      // Inc the extracted string
  Construct ...         // wrap in new Result
  Return
```

**Why the crash occurs:** `v_result` is projected in B3 (tag), B4 (ok payload), and B5 (err payload). With borrowing projections, the Perceus backward walk needs to place Dec(v_result) at the latest point where it's dead on ALL control-flow paths. The current liveness analysis fails to do this correctly when:
1. The parent is used across multiple successor blocks (B4 and B5 both project from v_result)
2. Different successors have different borrowing patterns (ok-path extracts non-RC payload, err-path extracts RC payload)
3. The Dec gets placed too early (in B4 before B5 can access v_result), causing the err-path to read freed memory

**Root cause:** The Perceus backward walk processes blocks independently. When it reaches B3's terminator, it needs to merge liveness from B4 and B5. With borrowing projections, v_result is live-in to both B4 and B5 (because each projects from it). The Dec should be emitted in EACH successor AFTER the last projection — not in the predecessor. The bug is in the live-in/live-out propagation at the branch point: the merge doesn't account for v_result being live in both successors independently.

**Fix — cross-block liveness propagation for borrowing projections:**

This requires changes to two interconnected passes in `rc_insert/mod.rs`:

1. **`compute_refined_liveness`** — The liveness analysis must propagate the parent variable's liveness through ALL blocks that project from it, not just the block containing the first use. Specifically:
   - When a `Project` instruction is classified as borrowing, the source variable must be added to the block's `live_out` set (it escapes the block through its continued need in successors)
   - At branch points, if the source variable is live-in to ANY successor, it must be live-out of the predecessor

2. **`process_block_rc` backward walk** — The Dec insertion logic must handle the case where a variable is borrowed in block N and consumed (via non-scalar projection) in successor blocks N+1, N+2:
   - If a variable is live-out (used in successors), do NOT insert Dec in the current block — even if a borrowing use was just processed
   - The Dec must be deferred to each successor independently, placed after the last use of the variable in that path
   - This is the standard Perceus rule (Dec at last use), but the current implementation doesn't correctly propagate "last use" across blocks when borrowing changes the use classification

3. **Cross-block Dec placement** — For the `?` operator pattern specifically, the correct output is:
   ```
   B3 (merge block):
     Project v_tag = INT, v_result, field 0   // borrows v_result
     Branch(v_tag == 0, B4, B5)
     // NO Dec here — v_result is live in both successors

   B4 (ok_block):
     Project v_ok = Idx(224), v_result, field 1   // borrows v_result, Inc on v_ok
     RcDec { v_result }   // last use of v_result on this path — Dec HERE
     ... uses v_ok ...
     Jump → ...

   B5 (err_block):
     Project v_err = STR, v_result, field 1   // borrows v_result, Inc on v_err
     RcDec { v_result }   // last use of v_result on this path — Dec HERE
     ... uses v_err ...
     Return
   ```

   Each path independently places its Dec after the last projection from v_result. The parent's inner RC fields are cleaned up by the Dec's per-variant traversal (using RcStrategy::InlineEnum — cross-reference Section 01.3).

**Implementation steps:**

1. **Add `is_borrowing_projection` to `is_borrowing_instr`:** `Project { dst, .. } => is_scalar(var_type(dst))`. Scalar projections borrow; RC-typed projections also borrow but get Inc on result (handled by existing logic).

2. **Fix `compute_refined_liveness` for borrowing projections:** When processing a `Project` that borrows its source, add the source variable to the block's GEN set (it's used here and must be live). At dataflow fixed-point, this propagates backward through predecessors correctly. The key change: borrowing projections must NOT add the source to the KILL set — the variable is not consumed.

3. **Fix `process_block_rc` Dec placement:** When the backward walk encounters a borrowing use and the variable is in `live_out`, do NOT emit Dec. Instead, defer to the successor blocks. The Dec fires at the LAST borrowing/consuming use of the variable on each control-flow path independently.

4. **Validate with the `?` operator pattern:** The test case is `lower_try` with `Result<int, str>`. Correct behavior: v_result is projected in B3 (borrows), B4 (borrows, Dec), B5 (borrows, Dec). No Dec in B3. Both B4 and B5 emit Dec(v_result) after their last projection. The InlineEnum Dec in each successor cleans up the inner RC fields of the unused variant.

5. **Validate no regressions:** The liveness changes affect ALL functions with Project instructions. Run the full AOT test suite (`./llvm-test.sh`) and spec tests (`cargo st`) to verify no heap corruption, no double-frees, no leaks.

**Reference implementations:**
- **Lean 4** `src/Lean/Compiler/IR/RC.lean`: `visitProj` — unconditionally treats `proj` as borrowing, emits `inc` if result is object-typed
- **Lean 4** `src/Lean/Compiler/IR/LiveVars.lean`: cross-block liveness propagation with fixed-point iteration
- **Swift** `lib/SILOptimizer/ARC/RCStateTransition.cpp`: similar borrowing classification for struct_extract

**Co-implementation requirement with Section 01.3 (RcStrategy):** The Dec emitted for v_result in B4/B5 must use `RcStrategy::InlineEnum` for Result types — this performs a tag-switch with per-variant field traversal. Without InlineEnum Dec, the Dec would attempt a heap pointer free on a stack-allocated Result, causing corruption. Both the borrowing projection fix (this section) and the InlineEnum strategy (Section 01.3) must land together.

---

- [x] Simplify `process_terminator_uses` and `process_instruction_uses` in `rc_insert/mod.rs`:
  - Replace the runtime `invoke_borrowed_args` computation with a read from `arg_ownership`
  - Replace `is_borrowing_instr` with a check on the instruction's embedded ownership
  - Remove `interner` from `RcContext` (no longer needed for external callee detection)

- [x] Simplify `insert_external_invoke_cleanup` post-pass:
  - Instead of re-computing `borrowed_flags`, read from the Invoke's `arg_ownership`
  - The post-pass becomes a simple loop: "for each Borrowed arg not in live_out, emit RcDec"

**Migration steps (explicit):**

1. **Add `ArgOwnership` enum to `ir/mod.rs`** and add `arg_ownership: Vec<ArgOwnership>` field to `Invoke` terminator and `Apply` instruction. Default all to `Owned` so existing code compiles without changes.

2. **Populate `arg_ownership` during RC insertion.** In `process_terminator_uses` (for `Invoke`) and `process_instruction_uses` (for `Apply`), call `compute_arg_ownership()` and store the result on the instruction. This is a new write — it doesn't change behavior yet because no one reads it.

3. **Replace `invoke_borrowed_args` computation** in `process_terminator_uses` (currently lines ~401-437 in `rc_insert/mod.rs`) with a read from the instruction's `arg_ownership`. Delete the inline `let invoke_borrowed_args = match terminator { ... }` block (~35 lines).

4. **Replace `is_borrowing_instr` checks** in `process_instruction_uses` and `process_block_rc` with a read from the instruction's embedded ownership. Delete `is_borrowing_instr()` function.

5. **Simplify `insert_external_invoke_cleanup`** (currently ~80 lines). Replace the `borrowed_flags` recomputation with a read from `Invoke::arg_ownership`. The function becomes ~20 lines: iterate args, check `arg_ownership[i] == Borrowed && !live_out.contains(arg)`, emit `RcDec`.

6. **Delete `is_external_callee()` function** from `rc_insert/mod.rs`. No longer needed — the classification happened in step 2.

7. **Remove `interner` from `RcContext`**, `insert_rc_ops_with_ownership` signature, `run_arc_pipeline` signature, and `run_arc_pipeline_all` signature. Update all call sites in `function_compiler/mod.rs` (3 locations) and `tests.rs` files.

8. **Delete `try_lookup()` from `ori_ir/interner/mod.rs`** (added specifically for external callee detection, no longer needed).

9. **Verify:** `./test-all.sh` passes. `grep -r "interner" compiler/ori_arc/src/` returns zero results (excluding tests that create a dummy interner).

---

## 04.5 Completion Checklist

- [x] `tracing::warn!` on every borrow sig lookup miss
- [x] `debug_assert!` that all compiled functions have sigs (debug builds)
- [x] `lookup_method_by_unqualified_name` eliminated — replaced with diagnostic-guarded fallback
- [x] Method dispatch is O(1) via existing two-tier lookup (no secondary index needed)
- [x] Debug assertions on `type_idx_to_name` and `method_functions` registrations
- [x] `ArgOwnership` enum defined in `ir/mod.rs`
- [x] `Invoke` and `Apply` carry per-arg ownership
- [x] `arg_ownership` computed during RC insertion from sigs + callee classification
- [x] `process_terminator_uses` reads from `arg_ownership` instead of re-deriving
- [x] `insert_external_invoke_cleanup` reads from `arg_ownership` instead of re-computing
- [x] `interner` removed from `RcContext` and `run_arc_pipeline` signature
- [x] Builtin tag-check methods (is_err, is_ok, is_some, is_none) lowered as PrimOp in `emit_call_or_invoke` (option a — correct level of abstraction)
- [x] `Project` with scalar result classified as borrowing in `is_borrowing_instr` (Lean 4 model)
- [x] Cross-block liveness analysis correctly propagates parent variable liveness through borrowing projections
- [x] Dec for borrowed parent emitted at last use on EACH control-flow path independently (not at branch point)
- [x] `never_propagation.ori` — 3 ARC leak tests pass (test_try_chain_first_err, test_try_chain_second_err, test_nested_try_err)
- [x] No heap corruption from Project borrowing change (verify with `tests/spec/control_flow/never_propagation.ori` AND `tests/spec/expressions/`)
- [x] No regression in `./llvm-test.sh`
- [x] No spurious warnings in normal compilation

**Exit Criteria:** `ORI_LOG=ori_llvm=warn ori build examples/hello.ori` produces no borrow-miss warnings. Linear scans eliminated. Call-site ownership is an IR-level contract — no runtime detection of external callees, no interner queries during RC insertion. Builtin methods on Option/Result lowered as PrimOp (option a). ALL `Project` instructions classified as borrowing per the Lean 4 model — scalar projections borrow without Inc, RC-typed projections borrow with Inc on result. Cross-block liveness analysis correctly places Dec at last use on each path independently. `ori test --backend=llvm tests/spec/control_flow/never_propagation.ori` shows 0 ARC leaks and no heap corruption.
