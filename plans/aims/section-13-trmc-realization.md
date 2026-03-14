---
section: "13"
title: "TRMC Realization & Soundness"
status: in-progress
goal: "Complete the TRMC pipeline from detection through realization, reconcile the soundness gate between may_share and per-variable uniqueness, and wire ContextBehavior into interprocedural contracts"
inspired_by:
  - "Leijen & Lorenzen, JFP 2025 — Tail Recursion Modulo Context"
  - "Lorenzen et al., PLDI 2024 — FIPTree first-class constructor contexts"
  - "Koka src/Core/CTail.hs — TRMC rewrite pass"
depends_on: ["09", "10", "11", "12"]
sections:
  - id: "13.1"
    title: "ContextBehavior Interprocedural Inference"
    status: complete
  - id: "13.2"
    title: "Soundness Gate Reconciliation"
    status: complete
  - id: "13.3"
    title: "Lifting Pre-Pass"
    status: not-started
  - id: "13.4"
    title: "TRMC 4-Equation Rewrite"
    status: not-started
  - id: "13.5"
    title: "Post-Rewrite Verification"
    status: not-started
  - id: "13.6"
    title: "Pipeline Integration & Event Consumption"
    status: not-started
  - id: "13.7"
    title: "Completion Checklist"
    status: in-progress
---

# Section 13: TRMC Realization & Soundness

**Status:** Not Started

**Goal:** Complete the TRMC pipeline from detection (already functional in
`normalize/detect.rs`) through IR rewriting and realization. Reconcile the
soundness gate mismatch between `contract/mod.rs` (function-level
`may_share == false`) and `intraprocedural/mod.rs` (per-variable uniqueness).
Wire `ContextBehavior` into interprocedural contracts (currently hardcoded
`ContextBehavior::default()` at `interprocedural.rs:567`).

**Context:** Detection infrastructure is complete:
- `normalize/detect.rs` identifies TRMC candidates (7 tests pass)
- `ContextRegion` metadata struct is fully specified (`contract/mod.rs:462-495`)
- `detect_trmc_candidates()` marks `ShapeClass::ContextHole` post-convergence
  (`intraprocedural/mod.rs:397-462`)
- `populate_context_events()` records `ContextOpen`/`ContextClose` events
  (`intraprocedural/mod.rs:477-539`)
- Pipeline step 3a (`normalize_function`) is wired in (`aims_pipeline.rs:72-76`)

What is missing:
1. **ContextBehavior is dead metadata.** `interprocedural.rs:567` sets
   `context_behavior: ContextBehavior::default()` -- never computed from
   analysis. No consumer reads it.
2. **Soundness gate mismatch.** `contract/mod.rs:364-368` documents that TRMC
   requires `may_share == false` (effect purity). But
   `intraprocedural/mod.rs:387-390` deliberately skips the `may_share` check
   because the `HeapEscaping -> may_share` accumulation rule makes ANY returned
   Construct trigger `may_share`, which would block all TRMC detection. Instead,
   it relies solely on per-variable `Uniqueness::Unique` (lines 447-450).
   Neither checks both -- the effect gate is documented but not enforced, and the
   uniqueness gate is enforced but not documented in the contract type.
3. **Events are recorded but never consumed.** `ContextOpen`/`ContextClose`
   events exist in `AimsStateMap` but no realization pass reads them.
4. **No IR rewriting.** `NormalizationResult.was_transformed` is always `false`.
   The 4-equation TRMC algorithm (Leijen & Lorenzen JFP 2025, Figure 2) is
   not implemented.

**Reference implementations:**
- **Koka** `src/Core/CTail.hs`: TRMC rewrite pass using defunctionalized
  contexts. The reference for our 4-equation implementation.
- **Leijen & Lorenzen, JFP 2025**: The equational approach with context laws
  `(appctx)` and `(appcomp)`. Lemma 2 (unique linear chain) is the soundness
  condition.
- **FIPTree** (Lorenzen et al., PLDI 2024): First-class constructor contexts
  for O(1) top-down algorithms.

**Depends on:** Section 09 (active shape/effect dimensions), Section 10
(unified realization consuming events), Section 11 (regression guards),
Section 12 (FIP enforcement verifier cross-checking results).

---

## 13.1 ContextBehavior Interprocedural Inference

**File(s):** `compiler/ori_arc/src/aims/contract/mod.rs`,
`compiler/ori_arc/src/aims/interprocedural.rs`

**Prerequisite:** Section 12's file splits (interprocedural.rs ->
interprocedural/mod.rs + extract.rs, intraprocedural/mod.rs ->
mod.rs + post_convergence.rs) must be complete before starting
Section 13. This section modifies `extract_contract()` (which will
be in `interprocedural/extract.rs` after the split) and
`detect_trmc_candidates()` / `populate_context_events()` (which
will be in `intraprocedural/post_convergence.rs` after the split).

`ContextBehavior` has two fields (`preserves_context`, `consumes_hole`) that
are always `false`. This section wires them into `extract_contract()` so they
are computed from analysis state.

- [x] Expand `ContextBehavior` fields per literature review C1:
  ```rust
  pub struct ContextBehavior {
      /// Does this function preserve a constructor context passed to it?
      /// True when the function's return value includes the context variable
      /// (not consumed or dropped).
      pub preserves_context: bool,
      /// Does this function consume the context hole?
      /// True when the function fills the hole field of the context variable.
      pub consumes_hole: bool,
      /// Does in-place TRMC require the context variable to be unique?
      /// Always true for the modulo-cons instantiation. False only if a
      /// CPS fallback is used (not implemented in v1).
      pub requires_unique_context: bool,
      /// Can effect handlers in scope resume more than once?
      /// Derived from EffectSummary. When true, the context variable may be
      /// captured non-linearly, breaking the unique linear chain invariant.
      /// Conservative: set to EffectSummary.may_share.
      pub may_resume_nonlinearly: bool,
  }
  ```

- [x] Update `ContextBehavior::default()`:
  The current `ContextBehavior` derives `Default`, which gives all `bool`
  fields as `false`. With the new `requires_unique_context` field, the
  "safe default" is `true` (require uniqueness unless proven unnecessary).
  Options:
  - **(a)** Remove the `Default` derive and implement manually with
    `requires_unique_context: true`
  - **(b)** Keep `Default` derive and treat `false` as "not yet computed"
    (consumers check `preserves_context` first)
  **Implemented:** Option (a) -- manual Default impl:
  ```rust
  impl Default for ContextBehavior {
      fn default() -> Self {
          Self {
              preserves_context: false,
              consumes_hole: false,
              requires_unique_context: true, // conservative
              may_resume_nonlinearly: false,
          }
      }
  }
  ```
  **Sync points for ContextBehavior::default():**
  - `contract/mod.rs` -- `all_borrowed()` uses `ContextBehavior::default()`
  - `contract/mod.rs` -- `conservative()` uses `ContextBehavior::default()`
  - `interprocedural.rs:567` -- `extract_contract()` uses `ContextBehavior::default()`
    (this is the one being replaced, but others remain as fallbacks)
  - Remove `#[derive(Default)]` from `ContextBehavior` struct

- [x] Update `ContextBehavior::join()` -- AND for `preserves_context` and
  `consumes_hole` (conservative), OR for `requires_unique_context` and
  `may_resume_nonlinearly` (conservative in opposite direction)

- [x] Compute `ContextBehavior` in `extract_contract()` (`interprocedural/extract.rs`):
  - Replace `context_behavior: ContextBehavior::default()` at line 567
  - **Data flow:** `extract_contract()` currently has no access to
    `NormalizationResult` or `context_regions`. The context regions are
    computed in step 3a (`normalize_function()`) and passed to step 4
    (`analyze_function()`). `extract_contract()` is called from within
    `analyze_function()`. Options:
    - **(a)** Thread `context_regions` through `analyze_function()` -- already
      done (it's a parameter). Have `extract_contract()` accept
      `context_regions: &[ContextRegion]` as an additional parameter.
    - **(b)** Query `AimsStateMap` for ContextOpen/ContextClose events
      (already recorded by `populate_context_events()`). But these are
      post-convergence and `extract_contract()` runs during convergence.
    **Recommended:** Option (a) -- pass `context_regions` to `extract_contract()`.
  - `preserves_context`: true if any `ContextRegion` exists where the context
    variable is returned (not consumed by RcDec before return)
  - `consumes_hole`: true if any `ContextRegion` exists where the hole field
    is written
  - `requires_unique_context`: always `true` (modulo-cons instantiation only)
  - `may_resume_nonlinearly`: `effects.may_share` (conservative approximation;
    see literature review section 04.7)

- [x] Add `ContextBehavior` to `MemoryContract` display/debug output
  (Already included via `#[derive(Debug)]` on both types)

- [x] Tests:
  - `context_behavior_default_is_conservative` -- default has safe values
  - `context_behavior_join_is_conservative` -- verify direction for each field
  - `context_behavior_join_is_commutative` -- commutativity check
  - `extract_contract_no_trmc_has_default_context_behavior` -- no TRMC → default
  - `extract_contract_with_trmc_computes_context_behavior` -- non-default ContextBehavior
    when TRMC candidates exist (also validates HeapEscaping → may_share → may_resume_nonlinearly)
  - `context_behavior_conservative_constructor_safe` -- `conservative()` returns
    safe ContextBehavior values

---

## 13.2 Soundness Gate Reconciliation

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/mod.rs`,
`compiler/ori_arc/src/aims/contract/mod.rs`

The documented soundness condition (`contract/mod.rs:364-368`) and the
implemented condition (`intraprocedural/mod.rs:387-390`, `447-450`) are different checks:

- **Documented (contract):** `EffectSummary.may_share == false` --
  function-level effect purity gate
- **Implemented (intraprocedural):** `Uniqueness::Unique` on context variable
  at block exit -- per-variable uniqueness gate

Per-variable uniqueness enforces Lemma 2 (unique linear chain) and is
enforced today. Effect purity would prevent non-linear resumption from
breaking uniqueness between analysis time and runtime -- but its correct
formulation is a **pending design decision, blocked on effect-handler
semantics** (Ori has no effect handlers yet). The second gate is not
merely unimplemented; the right abstraction boundary for it is unknown
until effect handlers exist to validate against.

**Design tension:** The `HeapEscaping -> may_share` accumulation rule (Section
09.1) means any function that returns a Construct has `may_share == true`.
Since TRMC functions build their result in place and return it, they ALWAYS
have `HeapEscaping` returns, which always triggers `may_share`. A naive
`may_share` gate would therefore block ALL TRMC candidates.

**Resolution options:**
1. Gate on `may_share` but exclude `may_share` contributions from returned
   Construct instructions (the "self-sharing" is expected and safe for TRMC)
2. Use a more refined `may_share_non_return` flag that only tracks sharing
   from non-return paths (effect handlers, closures, stored references)
3. Gate on a different effect: check whether the function uses effect handlers
   or capabilities that could capture the context non-linearly, rather than
   the overly broad `may_share`

**Status:** This is a pending design decision, not a solved problem.
The correct resolution depends on effect-handler semantics that do not
exist yet. Until effect handlers land, the per-variable
`Uniqueness::Unique` gate is the sole enforced soundness condition.
This is sound for v1 because without effect handlers there is no
mechanism for non-linear resumption. When effect handlers are
implemented, this section must be revisited and a concrete resolution
chosen before TRMC can be considered sound in the presence of effects.

- [x] **Prerequisite (shared with 12.4a):** Verify that the duplicated
  `collect_recursive_call_defs()` was unified with
  `collect_recursive_call_sites()` in Section 12. Both 13.2 and 13.4
  need recursive call info -- use the shared helper.
  (Verified: only `collect_recursive_call_sites()` exists in normalize/detect.rs)

- [x] Add effect purity gate to `detect_trmc_candidates()`
  (`intraprocedural/post_convergence.rs`):
  - Before the per-Construct loop (after the `recursive_defs.is_empty()` early
    return at line 400-402), add a function-level early return:
  ```rust
  // Soundness gate 2: Effect purity (Leijen & Lorenzen JFP 2025, §5.3)
  // If the function may share (non-linear effect handler resumption),
  // the unique linear chain can be broken at runtime even if analysis
  // proves uniqueness. Gate behind may_share == false.
  //
  // This is conservative: one-shot handlers are safe but we cannot
  // distinguish them yet. See literature review §04.7.
  if contract.effects.may_share {
      trace!("TRMC candidates rejected: function may_share == true");
      return;
  }
  ```
  - Access the contract via `sigs.get(&func.name)` in `analyze_function()`,
    which has `sigs: &FxHashMap<Name, MemoryContract>` as parameter.
    Thread the `EffectSummary` (or a `may_share` bool) through to
    `detect_trmc_candidates()`.
  - Note: `detect_trmc_candidates` currently takes `(state_map, func)` --
    add `may_share: bool` parameter (or `contract: &MemoryContract`)
    and update the call site in `analyze_function()` (line 222).
  - **Fixpoint iteration edge case:** During `analyze_scc_fixpoint()`,
    `sigs` contains contracts from PREVIOUS iterations (or nothing on
    the first iteration). For the first iteration of a recursive SCC,
    `sigs.get(&func.name)` returns `None` (the function's own contract
    hasn't been computed yet). For subsequent iterations, it returns the
    previous iteration's contract.
    - First iteration: `sigs.get(&func.name) == None` -> conservatively
      assume `may_share = true` (skip TRMC). This is safe -- if `may_share`
      converges to `false` in later iterations, TRMC detection will run.
    - Subsequent iterations: use `local_sigs.get(&func.name)` (the SCC-local
      contracts from `analyze_scc_fixpoint()`) instead of the global `sigs`.
      The call site must merge `sigs` and `local_sigs` for the lookup.
    - Alternatively: use the `EffectSummary` accumulated during analysis
      (available via `state_map.effect_summary()` after convergence). Since
      `detect_trmc_candidates()` runs POST-convergence, the effect summary
      is available. This is simpler than threading contracts through.

- [x] Add the same effect gate to `populate_context_events()`
  (`intraprocedural/post_convergence.rs`):
  - Both gates accept `may_share: bool`, logged but not enforced in v1.
  - `state_map.effect_summary().may_share` passed from `analyze_function()`.

- [x] Update doc comments in `contract/mod.rs` ContextBehavior to document
  both gates and their relationship (done in 13.1, expanded here):
  ```rust
  /// TRMC soundness requires TWO gates (Lemma 2, Leijen & Lorenzen JFP 2025):
  ///
  /// 1. **Per-variable uniqueness (enforced):** The context variable must have
  ///    `Uniqueness::Unique` at every point between context creation
  ///    and application. Checked in `detect_trmc_candidates()` and
  ///    `populate_context_events()` (intraprocedural/mod.rs).
  ///
  /// 2. **Effect purity (pending design decision):** In principle,
  ///    `EffectSummary.may_share == false` would guard against non-linear
  ///    resumption capturing the context variable. However, the current
  ///    `HeapEscaping -> may_share` accumulation rule makes ALL TRMC
  ///    candidates trigger `may_share == true`, so a naive gate blocks
  ///    all TRMC. The correct formulation depends on effect-handler
  ///    semantics (not yet implemented). Until then, gate 1 alone is
  ///    sound because no mechanism for non-linear resumption exists.
  ///
  /// Gate 1 is the enforced soundness condition. Gate 2 is required in
  /// principle but its design is blocked on effect-handler semantics.
  ```

- [x] Update doc comments in `intraprocedural/post_convergence.rs` to document
  both gates in `detect_trmc_candidates()` and `populate_context_events()`:
  - Both functions now document the two-gate model (Section 13.2)
  - Effect gate is logged-only, with rationale for why enforcement is deferred

- [x] Tests:
  - `trmc_not_rejected_when_may_share_true` -- confirms gate is logged, not enforced
    (ContextHole still set despite may_share=true from HeapEscaping return)
  - `context_events_recorded_despite_may_share_true` -- events still recorded
    (gate is logged-only; no effect handlers in v1)
  - Uniqueness gate implicitly tested by existing `trmc_candidate_detected_*` tests
    (Construct destinations are always Unique in single-block; non-Unique requires
    multi-block CFG which existing `trmc_not_detected_*` tests cover)

---

## 13.3 Lifting Pre-Pass

**File(s):** `compiler/ori_arc/src/aims/normalize/lift.rs` (NEW)

Before TRMC detection can match constructor contexts, expressions in
constructor field positions must be extracted into let-bindings. Without
lifting, `Construct(f(x), recurse(xs))` has an embedded call in field 0
and the context `Construct([], recurse(xs))` is not exposed.

Lifting transforms:
```
Construct { dst: r, ctor, args: [f(x), recurse(xs)] }
```
Into:
```
let y = f(x)
Construct { dst: r, ctor, args: [y, recurse(xs)] }
```

This is a standard normalization (A-normal form for constructor arguments).

- [ ] Implement `pub fn lift_constructor_args(func: &mut ArcFunction)`:
  - Walk each block's body looking for `Construct` instructions
  - For each `Construct`, check if any argument is defined by an `Apply`,
    `ApplyIndirect`, or `Invoke` in the same block
  - If so, the instruction is already in let-binding form in ARC IR (each
    `Apply { dst, ... }` defines `dst`). Verify this is the case -- ARC IR
    may already be in A-normal form by construction.

- [ ] Determine if lifting is necessary:
  ARC IR is lowered from CanExpr, which may already produce A-normal form
  for constructor arguments (each expression is bound to a variable before
  being passed as a constructor arg). If this is the case, lifting is a no-op
  and this subsection reduces to a verification assertion:
  ```rust
  debug_assert!(
      all_construct_args_are_variables(func),
      "ARC IR should be in A-normal form for constructor arguments"
  );
  ```
  Read `lower/mod.rs` to determine whether the lowerer guarantees this.

- [ ] If lifting IS needed, implement the transformation:
  - For each Construct with an embedded expression argument:
    - Create a fresh `ArcVarId` for the extracted let-binding
    - Extend `func.var_types` with the new variable's type (same as the
      expression's result type). The new `ArcVarId` is the index of the
      pushed entry.
    - Insert the expression as a new instruction before the Construct
    - Replace the expression argument with the fresh variable
  - Update `NormalizationResult.was_transformed = true` when any lifting occurs
  - **Note:** This pass mutates the function (`&mut ArcFunction`). The
    current `normalize_function()` takes `&ArcFunction` (immutable).
    See Section 13.6 for the signature change.

- [ ] Wire into `normalize_function()` (`normalize/mod.rs`):
  - Call `lift_constructor_args()` BEFORE `detect_context_regions()`
  - Lifting must precede detection (invariant I4 from literature review section 04.2)

- [ ] Tests:
  - `lifting_a_normal_form_is_noop` -- already-normalized Construct unchanged
  - `lifting_extracts_embedded_call` -- if lifting is needed
  - `lifting_extends_var_types` -- verify new variables registered in var_types

---

## 13.4 TRMC 4-Equation Rewrite

**File(s):** `compiler/ori_arc/src/aims/normalize/rewrite.rs` (NEW)

**Complexity warning:** This is the most complex subsection in the AIMS
plan. The 4-equation TRMC rewrite transforms the function's IR in-place,
adding parameters, creating blocks, and rewriting terminators. It touches
core IR data structures (`ArcFunction`, `ArcBlock`, `ArcInstr`,
`ArcTerminator`) and must maintain SSA invariants. Estimate: 300-400 lines
of implementation. Read Koka's `CTail.hs` (~400 lines for the core rewrite)
and the Leijen & Lorenzen JFP 2025 paper (Figure 2) before implementing.
Consider implementing 13.5 (verification) first, so the verifier catches
bugs in the rewrite immediately.

Implement the 4-equation TRMC algorithm from Leijen & Lorenzen JFP 2025,
Figure 2. This transforms a recursive function into a tail-recursive version
that builds the result top-down using a context (Minamide tuple: `<res, hole>`).

The 4 equations:
- **(base)** `[[e]]_{f,k} = app k e` -- non-tail: apply context to result
- **(tail)** `[[K[f(args)]]]_{f,k} = f_hat(args, k . ctx(K))` -- recursive
  call under constructor context: compose and recurse
- **(tlet)** `[[let x = e' in e]]_{f,k} = let x = e' in [[e]]_{f,k}` --
  let: recurse into body
- **(tmatch)** `[[match e' { pi -> ei }]]_{f,k} = match e' { pi -> [[ei]]_{f,k} }` --
  match: recurse into each arm

- [ ] Define context representation for ARC IR:
  ```rust
  /// Minamide tuple: pointer to result root + address of hole field.
  /// For AIMS modulo-cons instantiation, the context is represented as:
  /// - `res: ArcVarId` — the root of the partially-built result
  /// - `hole: (ArcVarId, u32)` — (object containing hole, field index)
  struct TrmcContext {
      res: ArcVarId,
      hole_obj: ArcVarId,
      hole_field: u32,
  }
  ```

- [ ] Implement context operations:
  - `ctx(K)`: Create a context from a constructor `K` with a hole at the
    recursive call position. The constructor becomes the result root, and
    the hole is the field that will receive the recursive call's result.
  - `comp(k1, k2)`: Compose two contexts -- fill k1's hole with k2's root.
    This is an in-place `Set` instruction (requires uniqueness).
  - `app(k, e)`: Apply context to expression -- fill the hole with `e`.
    Another in-place `Set` instruction.

- [ ] Implement `pub fn rewrite_trmc(func: &mut ArcFunction, regions: &[ContextRegion]) -> bool`:
  - For each `ContextRegion`:
    1. Verify the region passes both soundness gates (uniqueness + effect purity)
    2. **Function transformation approach (recommended: in-place)**:
       Instead of creating a separate `f_hat`, transform the function in-place:
       - Add an optional context parameter `ctx: Option<TrmcContext>` (or
         a pair of `res: ArcVarId, hole: (ArcVarId, u32)`)
       - At the entry block, branch on whether context is provided:
         - None -> first call (build identity context from initial Construct)
         - Some -> recursive call (use provided context)
       - This avoids creating new `ArcFunction` entries, which would require
         changing the pipeline's function list and re-running interprocedural
         analysis.
       - The original call sites (external callers) pass None/identity context.
       - Recursive call sites pass the composed context.
    3. Apply the 4-equation algorithm to rewrite the function body
    4. Update `func.params` and `func.var_types` for the new context parameter
  - Return `true` if any rewrite was applied
  - **Alternative (auxiliary function):** If in-place rewrite creates too
    much complexity, create a separate `f_hat` function. This requires:
    - A return type from `rewrite_trmc()` that carries new functions
    - The pipeline (`run_aims_pipeline_all()`) collecting new functions and
      re-running interprocedural analysis on them
    - More complex but cleaner separation

- [ ] Handle the fallback case:
  - When soundness gates fail, skip the rewrite (leave function unmodified)
  - Log via `tracing::debug!` why the rewrite was skipped
  - This is the recommended approach from literature review section 04.7

- [ ] Update `NormalizationResult.was_transformed` when rewrite succeeds

- [ ] Set scope bounds for v1:
  - Self-recursive only (no mutual recursion)
  - Single recursive call per region
  - Modulo-cons instantiation only (no CPS fallback)
  - Skip when `may_share == true` (no hybrid path in v1)

- [ ] Tests (ArcIrBuilder test pattern):
  - `rewrite_trmc_simple_list_map` -- basic self-recursive list constructor
  - `rewrite_trmc_enum_variant` -- recursive enum variant construction
  - `rewrite_trmc_skipped_when_not_unique` -- soundness gate 1
  - `rewrite_trmc_skipped_when_may_share` -- soundness gate 2
  - `rewrite_trmc_produces_tail_call` -- rewritten function has tail call
  - `rewrite_trmc_context_operations_emit_set` -- comp/app use Set instructions
  - `rewrite_trmc_multi_arm_match` -- match with recursive calls in multiple
    arms (each arm gets its own context composition)
  - `rewrite_trmc_preserves_non_recursive_arms` -- match arms without
    recursive calls are left unchanged (base case)

---

## 13.5 Post-Rewrite Verification

**File(s):** `compiler/ori_arc/src/aims/normalize/verify.rs` (NEW)

After TRMC rewriting, verify that the context laws hold for each rewrite
site. This catches bugs in the rewrite implementation.

- [ ] Implement `pub fn verify_trmc_rewrite(func: &ArcFunction, regions: &[ContextRegion]) -> Vec<TrmcVerificationError>`:
  Checks per rewrite site:
  1. **Linear context usage:** The context variable is used exactly once per
     control-flow path (no duplication, no dropping)
  2. **Uniqueness preserved:** The context variable has
     `Uniqueness::Unique` at every use point between creation and application
  3. **Effect purity:** `EffectSummary.may_share == false` for the enclosing
     function (redundant with detection gate but catches rewrite-introduced
     violations)
  4. **No polymorphic context:** The constructor used as context has a known
     layout (no type variables in field types)
  5. **Tail position:** All recursive calls in the rewritten function are in
     tail position (the whole point of TRMC)

- [ ] Define `TrmcVerificationError` enum:
  ```rust
  pub enum TrmcVerificationError {
      /// Context variable used more than once on a control-flow path.
      NonLinearContext { function: Name, var: ArcVarId },
      /// Context variable not unique at use point.
      NonUniqueContext { function: Name, var: ArcVarId, block: ArcBlockId },
      /// Function has may_share == true (should not have been rewritten).
      EffectPurityViolation { function: Name },
      /// Recursive call not in tail position after rewrite.
      NonTailRecursiveCall { function: Name, block: ArcBlockId },
  }
  ```

- [ ] Rollback mechanism:
  Before calling `rewrite_trmc()`, save the original function body:
  ```rust
  let original = func.clone();
  let was_rewritten = rewrite_trmc(func, &context_regions);
  if was_rewritten {
      let errors = verify_trmc_rewrite(func, &context_regions);
      if !errors.is_empty() {
          *func = original; // rollback
          tracing::warn!("TRMC verification failed, rolling back");
          // was_rewritten effectively becomes false for downstream
      }
  }
  ```
  `ArcFunction` derives `Clone` (required for this pattern). The clone is
  only taken when TRMC candidates exist (most functions have zero candidates),
  so the cost is bounded.

- [ ] Error handling:
  - In debug builds: `debug_assert!` on verification failures (catches bugs
    in the rewrite implementation during development)
  - In release builds: `tracing::warn!` and skip the rewrite (roll back to
    original function body using the clone above)

- [ ] Wire into `normalize_function()`:
  - Call `verify_trmc_rewrite()` after `rewrite_trmc()`
  - If verification fails, restore the original function body and set
    `was_transformed = false`

- [ ] Tests:
  - `verify_clean_rewrite_passes` -- correctly rewritten function passes
  - `verify_non_linear_context_fails` -- manually constructed bad rewrite
  - `verify_non_tail_call_fails` -- recursive call not in tail position
  - `verify_rollback_restores_original` -- after verification failure,
    function body matches the pre-rewrite state

---

## 13.6 Pipeline Integration & Event Consumption

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs`,
`compiler/ori_arc/src/aims/normalize/mod.rs`

**Implementation order:** This section should be implemented LAST within
Section 13, not first. The pipeline wiring depends on all components
(13.1-13.5) being implemented. Implement in order: 13.1 -> 13.2 -> 13.5
(verification first for safety net) -> 13.3 -> 13.4 -> 13.6.

Wire the complete TRMC pipeline into the AIMS pipeline. Make
`ContextOpen`/`ContextClose` events consumable by realization.

- [ ] Update `normalize_function()` to call the full pipeline:
  ```rust
  pub fn normalize_function(
      func: &mut ArcFunction,
      contract: Option<&MemoryContract>,
  ) -> NormalizationResult {
      // Step 1: Lifting (13.3)
      lift_constructor_args(func);

      // Step 2: Detection (existing)
      let context_regions = detect::detect_context_regions(func);

      // Step 3: Rewrite (13.4) — only when candidates exist and
      // effect purity holds
      let mut was_transformed = false;
      if !context_regions.is_empty() {
          let may_share = contract
              .map_or(true, |c| c.effects.may_share);
          if !may_share {
              let original = func.clone();
              was_transformed = rewrite::rewrite_trmc(func, &context_regions);

              // Step 4: Verification (13.5)
              if was_transformed {
                  let errors = verify::verify_trmc_rewrite(func, &context_regions);
                  if !errors.is_empty() {
                      *func = original;
                      tracing::warn!("TRMC verification failed, rolling back");
                      was_transformed = false;
                  }
              }
          }
      }

      NormalizationResult {
          was_transformed,
          context_regions,
      }
  }
  ```

- [ ] Update `normalize_function()` signature:
  Current: `pub fn normalize_function(func: &ArcFunction) -> NormalizationResult`
  New: `pub fn normalize_function(func: &mut ArcFunction, contract: Option<&MemoryContract>) -> NormalizationResult`
  - `&mut ArcFunction` because both lifting (13.3) and rewrite (13.4)
    mutate the function
  - `Option<&MemoryContract>` because the contract may not exist during
    the first fixpoint iteration (see 13.2 edge case). When `None`,
    conservatively skip TRMC (same as `may_share == true`).
  - The contract is needed for the effect purity gate (`may_share`)
  - Pipeline step 3a already has access to contracts from step 1

- [ ] Update pipeline call site (`aims_pipeline.rs:72-76`):
  ```rust
  // Step 3a: normalize — detect TRMC context regions.
  let norm_result = {
      let _span = tracing::info_span!("normalize_function").entered();
      let contract = config.contracts.get(&func.name);
      crate::aims::normalize::normalize_function(func, contract)
  };
  ```
  - Pass `contract` to `normalize_function()`
  - If `was_transformed`, re-run dependent steps (see below)

- [ ] Consume `ContextOpen`/`ContextClose` events in realization:
  - Events are already stored in `AimsStateMap.events`
  - Add event consumption to `realize_rc_reuse()` (Phase 1):
    - `ContextOpen` events mark where a context is created -> no RcDec on the
      context variable (it is being passed down, not consumed)
    - `ContextClose` events mark where the hole is filled -> the fill
      operation is an in-place `Set`, not a new allocation
  - This connects the existing event recording to actual emission decisions

- [ ] Handle `was_transformed` in pipeline:
  When `was_transformed == true`, the function has been structurally rewritten.
  The following steps MUST re-run:
  1. **Step 3 (compute_var_reprs)** -- new variables from lifting/rewrite need
     `ValueRepr` entries. Without this, RC emission will panic on unknown vars.
  2. **Step 3.5 (detect_immortals)** -- new variables may reference immortal
     values. Without re-running, immortal detection is incomplete.
  3. **Step 4 (analyze_function)** -- the analysis state from before the
     rewrite is invalid. Must converge on the rewritten body.

  Implementation: wrap steps 3, 3.5, 3a, 4 in a loop that re-runs if
  `was_transformed == true`. Since TRMC rewrite is idempotent (running
  detection on an already-rewritten function finds no new candidates),
  the loop terminates in at most 2 iterations.

- [ ] Update `NormalizationResult` if needed:
  - Currently has `was_transformed: bool` and `context_regions: Vec<ContextRegion>`
  - Rollback is handled by cloning the function before rewrite (see 13.5),
    NOT by carrying the original in `NormalizationResult`

- [ ] Tests:
  - `pipeline_normalize_then_analyze` -- full pipeline with TRMC rewrite
    followed by re-analysis
  - `pipeline_normalize_noop_when_no_candidates` -- no rewrite, no re-analysis
  - `pipeline_context_events_consumed_by_realize` -- events affect RC decisions
  - `pipeline_rollback_on_verification_failure` -- verification failure
    restores original function
  - `pipeline_rerun_var_reprs_after_transform` -- new variables have
    `ValueRepr` entries after re-running step 3

---

## 13.6a Codebase Hygiene -- Fix Along the Way

These items should be fixed during implementation of 13.1-13.6:

- [ ] **STYLE (stale doc):** `ContextBehavior` doc at `contract/mod.rs:359-368`
  says "Stage 3" for TRMC -- after Section 13, this is no longer future tense.
  Update to describe the current state when 13.1 is complete.
- [ ] **STYLE (stale doc):** `normalize/mod.rs:14-17` says "Detection only --
  no IR rewriting. The full TRMC 4-equation rewrite is deferred to a future
  stage." After 13.4, this is inaccurate. Update when implementing 13.4.
- [ ] **STYLE (stale doc):** `NormalizationResult::was_transformed` doc at
  `normalize/mod.rs:42` says "always `false` in v1". Update after 13.4.
- [ ] **WASTE (unnecessary clone):** `analyze_scc_fixpoint()` at
  `interprocedural.rs:164` does `external_sigs.clone()` into `combined_sigs`
  every call. For large programs this clones the entire signature map for
  each SCC. Consider using a layered lookup (check `local_sigs` first, then
  `external_sigs`) instead of cloning. Fix when touching this function in
  13.1 (threading `context_regions` to `extract_contract()`).
- [ ] **STYLE (module doc accuracy):** `aims/mod.rs` module doc lists
  `normalize` as "Stage 3: TRMC normalization (detection + context metadata)".
  After Section 13, update to include "detection, lifting, rewriting, and
  verification" instead of just "detection + context metadata".

---

## 13.7 Completion Checklist

- [x] Section 12 file splits are complete before starting (prerequisite)
- [x] Duplicated `collect_recursive_call_defs` already unified (from 12.4a)
- [ ] Stale `ContextBehavior` / `normalize` / `aims/mod.rs` docs updated
  (see 13.6a list)
- [ ] `analyze_scc_fixpoint` clone-on-every-SCC eliminated (see 13.6a)
- [x] `ContextBehavior` fields expanded (`requires_unique_context`,
  `may_resume_nonlinearly`) and computed in `extract_contract()`
  (no more hardcoded default at `interprocedural.rs:567`)
- [x] `ContextBehavior::Default` derive removed; manual impl with
  `requires_unique_context: true`
- [x] `context_regions` threaded to `extract_contract()` as parameter
- [x] Per-variable uniqueness gate enforced in `detect_trmc_candidates()`
  and `populate_context_events()`
- [x] Effect purity gate: design decision documented, placeholder
  infrastructure added (gated on `may_share` with known false-positive
  issue, logged but not enforced in v1). Final resolution blocked on
  effect-handler semantics.
- [ ] Fixpoint edge case handled: first SCC iteration with no contract
  conservatively skips TRMC
- [x] Doc comments in `contract/mod.rs` and `intraprocedural/post_convergence.rs`
  reconciled — both document both gates and their relationship
- [ ] Lifting pre-pass implemented (or verified as unnecessary for ARC IR)
- [ ] `normalize_function()` signature changed to `(&mut ArcFunction, Option<&MemoryContract>)`
- [ ] 4-equation TRMC rewrite implemented for modulo-cons instantiation
  (in-place transform with optional context parameter, or auxiliary function)
- [ ] Post-rewrite verification catches rewrite bugs
- [ ] Rollback mechanism: `func.clone()` before rewrite, restore on
  verification failure
- [ ] Pipeline re-runs steps 3, 3.5, 4 after successful TRMC rewrite
- [ ] `ContextOpen`/`ContextClose` events consumed by realization
- [ ] `NormalizationResult.was_transformed` is `true` when TRMC applied
- [ ] Fallback strategy: skip (leave unmodified) when soundness gates fail
- [ ] Section 12 FIP verifier cross-checks TRMC-rewritten functions
- [ ] `cargo test --workspace` green
- [ ] `./test-all.sh` green
- [ ] Valgrind: 0 memory errors on all test programs

**Exit Criteria:** `normalize_function()` produces structurally rewritten
functions for self-recursive constructor-context patterns. The rewrite
satisfies context laws `(appctx)` and `(appcomp)` verified post-rewrite.
The per-variable uniqueness gate (Lemma 2) is enforced in detection,
rewrite, and verification. The effect purity gate (`may_share`) has
placeholder infrastructure but its final design is a pending decision
blocked on effect-handler semantics -- this is explicitly documented, not
silently deferred. `ContextBehavior` is computed from analysis state, not
hardcoded. `ContextOpen`/`ContextClose` events drive realization decisions.
The TRMC pipeline is end-to-end: detect -> lift -> rewrite -> verify ->
analyze -> realize.
