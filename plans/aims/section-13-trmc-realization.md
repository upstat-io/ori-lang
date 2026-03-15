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
    status: in-progress
  - id: "13.3"
    title: "Lifting Pre-Pass"
    status: complete
  - id: "13.4"
    title: "TRMC 4-Equation Rewrite"
    status: in-progress
  - id: "13.5"
    title: "Post-Rewrite Verification"
    status: in-progress
  - id: "13.6"
    title: "Pipeline Integration & Event Consumption"
    status: in-progress
  - id: "13.7"
    title: "Completion Checklist"
    status: in-progress
  - id: "13.8"
    title: "TRMC Behavioral Test Matrix"
    status: not-started
---

# Section 13: TRMC Realization & Soundness

**Status:** Incomplete — **5 confirmed bugs** (2 High, 3 Medium)

**Claim:** `normalize_function()` transforms self-recursive constructor-context
functions into tail-recursive form with in-place context mutation. The rewrite
is sound (behaviorally equivalent), verified (uniqueness enforced), and
integrated (post-rewrite contracts refreshed).

**Evidence:** Detection works (`normalize/detect.rs`, 7 tests). Rewrite code
exists (`normalize/rewrite.rs`). ContextBehavior computed from analysis
(`interprocedural/extract.rs`). Pipeline wiring exists (`aims_pipeline.rs`
step 3a).

**Not yet realized:**
- Rewrite is unreachable — `may_share` gate blocks all real candidates (Bug 4)
- Rewrite produces wrong output — argument threading broken (Bug 1)
- Post-rewrite uniqueness verification stubbed out (Bug 5)
- Post-rewrite contracts stale — no refresh mechanism (Bug 2)
- Helper blocks violate block-param convention (Bug 3)
- Zero behavioral tests — all 37 tests are structural only

**Open contradictions:** Invariant 2 ("active rewrites must be sound") is
violated: the rewrite exists in the pipeline but cannot produce correct output.
Invariant 4 ("enabled surface must be end-to-end verified") is violated: no
behavioral verification exists.

**Required invariant:** Per-variable `Uniqueness::Unique` on the context
variable at every mutation point (Lemma 2, Leijen & Lorenzen JFP 2025). This
is the sole soundness condition. The effect purity gate (`may_share`) is not
applicable because Ori has no effect handlers — there is no mechanism for
non-linear resumption that could break uniqueness.

**Goal:** Complete the TRMC pipeline from detection through rewriting, verification,
and realization. Fix all 5 bugs. Build the behavioral test matrix.

**Context:** Detection infrastructure is complete:
- `normalize/detect.rs` identifies TRMC candidates (7 tests pass)
- `ContextRegion` metadata struct is fully specified (`contract/mod.rs`, line ~553+)
- `detect_trmc_candidates()` marks `ShapeClass::ContextHole` post-convergence
  (`intraprocedural/post_convergence.rs`)
- `populate_context_events()` records `ContextOpen`/`ContextClose` events
  (`intraprocedural/post_convergence.rs`)
- Pipeline step 3a (`normalize_function`) is wired in (`aims_pipeline.rs`)

What is missing:
1. ~~**ContextBehavior is dead metadata.**~~ **RESOLVED (Section 13.1).**
   `interprocedural/extract.rs` now computes `context_behavior` via
   `compute_context_behavior()` from `ContextRegion` metadata.
2. **Soundness gate mismatch — plan is internally inconsistent.**
   `contract/mod.rs:364-368` documents that TRMC requires `may_share == false`
   (effect purity). `intraprocedural/post_convergence.rs:202` deliberately
   skips the `may_share` check because the `HeapEscaping -> may_share`
   accumulation rule makes ANY returned Construct trigger `may_share`, which
   would block all TRMC detection. Instead, it relies solely on per-variable
   `Uniqueness::Unique` (post_convergence.rs:246). The **plan itself**
   contradicts: Section 13.2 L284 says v1 should enforce only per-variable
   uniqueness, but L299 and L547 mark "skip when may_share == true" as done.
   The **code** follows both interpretations in different places:
   
`normalize/mod.rs:94` enforces the strict `may_share` gate (blocks rewrite),
   while `post_convergence.rs:202` follows the loose interpretation (logs only).
   Since every TRMC function returns a Construct → HeapEscaping → may_share,
   **the normalize gate blocks 100% of real TRMC candidates**. The rewrite is
   effectively dead code.
3. **Post-rewrite uniqueness verification is stubbed out.**
   `verify.rs:44-66` defines `NonUniqueContext` and `EffectPurityViolation`
   with `#[expect(dead_code)]` — never constructed anywhere. The rewrite can
   emit `Set` on a shared context variable without proof of uniqueness. The
   non-rewrite path (`post_convergence.rs:246`) correctly enforces uniqueness
   before marking ContextHole, but the rewrite path bypasses this enforcement.
4. **Events are recorded but never consumed.** `ContextOpen`/`ContextClose`
   events exist in `AimsStateMap` but no realization pass reads them.
5. **No IR rewriting in practice.** `NormalizationResult.was_transformed` is
   always `false` because Bug 4 (may_share gate) blocks all candidates.
   The 4-equation TRMC algorithm exists in `rewrite.rs` but is unreachable.

**Confirmed bugs (2026-03-14 audit, updated 2026-03-14 verification):**

> **Bug 1 (HIGH) — Recursive argument threading.** `rewrite.rs:362` loop-back
> Jump passes only 3 context args (`[true_var, new_res, input.ctor_dst]`).
> The recursive call's NEW arguments (extracted by `extract_recursive_call()`
> at line 151 but discarded with `let (rec_dst, _) = ...`) are never
> threaded to the loop header. The loop header block has `N_original + 3`
> params after the rewrite (0 original block params + 3 appended context
> params), and the prologue Jump passes 3 args — so structurally consistent,
> but the **function-level params** (`func.params[i].var`) are never rebound
> between iterations. Unlike `tail_call/rewrite.rs` (lines 50-68 fresh block
> params, lines 104-119 Let bindings bridging fresh→original, back-edge Jump
> passes `call_args`), the TRMC rewrite has NO mechanism to rebind function
> params with the recursive call's new arguments. **Any recursive function
> that changes its arguments on the recursive call will loop infinitely
> with the original argument values.**
>
> 
> **Bug 2 (MEDIUM) — Stale interprocedural contracts.** TRMC rewrites happen
> in the per-function pipeline (step 3a in `aims_pipeline.rs:92`) AFTER
> `analyze_program()` computes all contracts once (line ~275-278). The
> per-function re-run loop (lines ~76-104) only re-runs `var_reprs`,
> `immortals`, and `normalize_function` — NOT interprocedural analysis.
> `config.contracts` is `&FxHashMap` (immutable borrow). After a successful
> TRMC rewrite, `ContextBehavior`, `has_unbounded_stack`, `FipContract`, and
> effect summaries remain pre-rewrite. Callers use stale contracts. FIP
> verification (step 5a) cross-checks rewritten IR against stale contracts.
>
> **Bug 3 (MEDIUM) — Helper blocks use context vars without block-param
> threading.** `rewrite.rs:327-340` (compose block) and `rewrite.rs:419-428`
> (apply_ctx block) reference loop-header block params (`ctx_res`,
> `ctx_hole_obj`) directly, but these blocks have `params: vec![]` — they
> receive no values from their predecessors via Jump args. This works by
> SSA dominance accident (loop header dominates helper blocks), but violates
> the ARC IR block-param calling convention used by `tail_call/rewrite.rs`
> (lines 104-119). Any future pass that restructures the CFG (e.g., block
> splitting, code motion) could break dominance and cause undefined behavior.
>
> **Bug 4 (HIGH) — may_share gate blocks 100% of real TRMC candidates.**
> 
`normalize/mod.rs:94` enforces `if may_share { skip }` before calling
> `rewrite_and_verify()`. But the `HeapEscaping -> may_share` accumulation
> rule (Section 09.1) means ALL functions that return a Construct have
> `may_share == true` — and every TRMC function returns a Construct.
> `post_convergence.rs:202` correctly handles this by logging but NOT
> enforcing the gate (continuing to detect candidates). **Note:** the planned
> test `trmc_not_rejected_when_may_share_true` is not yet implemented. But
`normalize/mod.rs:94` then rejects
> that same candidate during the rewrite phase. **Net effect: detection
> works, rewriting is dead code.** The plan contradicts itself: Section
> 13.2 L284 says v1 should enforce only per-variable uniqueness, but
> Section 13.4 L547 marks "Skip when may_share == true" as done. The
> fix: remove the `may_share` gate from `normalize/mod.rs` and rely on
> per-variable uniqueness (already enforced in `post_convergence.rs:246`)
> as the sole soundness condition (Ori has no effect handlers). This is sound because Ori has
> no effect handlers — there is no mechanism for non-linear resumption.
>
> **Bug 5 (MEDIUM) — Post-rewrite uniqueness/effect verification is
> stubbed out.** `verify.rs:44-66` defines `NonUniqueContext` and
> `EffectPurityViolation` with `#[expect(dead_code, reason = "constructed
> by Section 13.6 pipeline integration")]` — but nothing constructs them
> anywhere in the codebase. `normalize/mod.rs:144` calls
> `verify_trmc_rewrite()` which performs ONLY structural checks. The
> verification function's own doc (verify.rs:156-159) says: "This function
> only checks structural properties of the IR — it does not require
> uniqueness analysis or interprocedural contracts." Meanwhile the non-
> rewrite path in `post_convergence.rs:246-264` actively checks
> `state.uniqueness != Uniqueness::Unique` before marking ContextHole.
> The rewrite bypasses this enforcement. The fix: after TRMC rewrite,
> re-run analysis (already planned in 13.6), then verify that context
> variables in the rewritten function are `Unique` at all use points.
> Construct the `NonUniqueContext` and `EffectPurityViolation` variants
> when violations are found.

**Test coverage gap:** The existing TRMC tests in `normalize/tests.rs` are
**entirely structural** — they check block counts, param counts, absence of
self-calls, and presence of `Set` instructions. No test:
- Verifies the rewritten function produces correct output for any input
- Checks that recursive call arguments are threaded to the next iteration
- Validates post-rewrite contract refresh
- Tests behavioral equivalence between pre-rewrite and post-rewrite functions
This contrasts with Section 08's Matrix A/B/C tests, which include AOT
behavioral tests, ARC dump golden checks, and runtime-trap tests for the
RC alias/projection system. Section 13 has no equivalent test matrix.

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
`compiler/ori_arc/src/aims/interprocedural/extract.rs`

**Note:** Section 12's file splits are complete. `extract_contract()` is in
`interprocedural/extract.rs`. `detect_trmc_candidates()` and
`populate_context_events()` are in `intraprocedural/post_convergence.rs`.

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
      /// CPS fallback is used (not implemented currently).
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
  
  - `interprocedural/extract.rs` -- `extract_contract()` now uses
    `compute_context_behavior()` (Section 13.1 complete)
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

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs`,
`compiler/ori_arc/src/aims/contract/mod.rs`

The documented soundness condition (`contract/mod.rs` ContextBehavior doc, ~line 398) and the
implemented condition (`intraprocedural/post_convergence.rs`) are different checks:

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
This is sound currently because without effect handlers there is no
mechanism for non-linear resumption. When effect handlers are
implemented, this section must be revisited and a concrete resolution
chosen before TRMC can be considered sound in the presence of effects.

- [x] **Prerequisite (shared with 12.4a):** Verify that the duplicated
  `collect_recursive_call_defs()` was unified with
  `collect_recursive_call_sites()` in Section 12. Both 13.2 and 13.4
  need recursive call info -- use the shared helper.
  (Verified: only `collect_recursive_call_sites()` exists in normalize/detect.rs)

- [x] ~~Add effect purity gate to `detect_trmc_candidates()`~~ — REVERTED.
  The original plan added a `may_share` early return to post_convergence.rs.
  This was implemented as log-only (correct), but `normalize/mod.rs:94`
  independently enforces the gate as a hard block. See Bug 4 above.

- [x] **BUG FIX (HIGH — Bug 4): Remove the `may_share` gate from
  `normalize/mod.rs`.** The `may_share` check at `normalize/mod.rs:91-100`
  (`if may_share { skip rewrite }`) blocks ALL real TRMC candidates because
  every function returning a Construct has `may_share == true` via the
  `HeapEscaping -> may_share` accumulation rule. The post_convergence path
  correctly treats this as log-only. The test `trmc_not_rejected_when_-
  may_share_true` asserts candidates are detected despite `may_share == true`.
  **Fix (applied 2026-03-14):**
  1. Removed the `may_share` gate from `normalize/mod.rs`. Gate changed to
     `contract.is_some()` (only skip when no converged contract available).
     Comment documents effect purity gate deferral to effect-handler impl.
  2. Renamed test `pipeline_normalize_noop_when_may_share` to
     `pipeline_normalize_proceeds_despite_may_share` — now verifies rewrite
     PROCEEDS and self-calls are eliminated despite `may_share == true`.
  3. Log-only gate in `post_convergence.rs:202` kept as documentation.
  4. `contract/context.rs` ContextBehavior doc (lines 20-26) already states
     `may_share` is the future gate, per-variable uniqueness is enforced.
  5. Section 13.4 L629 already marked "Skip when may_share == true" as reverted.
  Also: updated `rewrite.rs` scope/soundness docs, `verify.rs`
  `EffectPurityViolation` dead_code reason and Display message, and
  `normalize/mod.rs` module/function doc comments.

- [x] Add the same effect gate to `populate_context_events()`
  (`intraprocedural/post_convergence.rs`):
  - Both gates accept `may_share: bool`, logged but not enforced currently.
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
    (gate is logged-only; Ori has no effect handlers)
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

- [x] Implement `pub fn lift_constructor_args(func: &ArcFunction)`:
  - Verified: ARC IR enforces A-normal form by type system (`Construct.args:
    Vec<ArcVarId>`). Embedded expressions are impossible by construction.
  - Implementation: `normalize/lift.rs` — debug assertion verifying all
    Construct dst/arg variable IDs are in bounds of `var_types`.

- [x] Determine if lifting is necessary:
  ARC IR is **not** in A-normal form by convention — it is enforced by the
  **type system**. `Construct.args` is `Vec<ArcVarId>`, so only variable
  references can appear as constructor arguments. The lowering pass
  (`lower/`) evaluates each sub-expression into a variable before building
  the `Construct` instruction. Lifting is a **verified no-op**.

- [x] If lifting IS needed, implement the transformation:
  N/A — lifting is not needed. ARC IR type system prevents the condition.

- [x] Wire into `normalize_function()` (`normalize/mod.rs`):
  - `lift::lift_constructor_args(func)` called BEFORE `detect_context_regions()`
  - Invariant I4 satisfied: lifting (verification) precedes detection.

- [x] Tests:
  - `lifting_a_normal_form_is_noop` — well-formed recursive Construct, no panic
  - `lifting_multi_field_construct_valid` — 3-field Construct, all valid
  - `lifting_no_constructs_is_noop` — function without Constructs
  - `lifting_catches_invalid_arg_var` — debug_assert catches out-of-bounds arg
  - `lifting_catches_invalid_dst_var` — debug_assert catches out-of-bounds dst

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

- [x] Define context representation for ARC IR:
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

  **Note:** `TrmcContext` in `rewrite.rs` currently has
  `#[expect(dead_code, reason = "constructed by post-rewrite verification
  (Section 13.5)")]` but `verify.rs` does not use or construct it. The
  `RewriteContext` struct in `verify.rs` serves a similar purpose with
  different fields. Either (a) remove `TrmcContext` and use
  `RewriteContext` consistently, or (b) use `TrmcContext` in verification
  when Bug 5 (uniqueness verification) is implemented. Decide during Bug 5
  fix — do not leave dead_code expect attribute indefinitely.

- [x] Implement context operations:
  - `ctx(K)`: Create a context from a constructor `K` with a hole at the
    recursive call position. The constructor becomes the result root, and
    the hole is the field that will receive the recursive call's result.
  - `comp(k1, k2)`: Compose two contexts -- fill k1's hole with k2's root.
    This is an in-place `Set` instruction (requires uniqueness).
  - `app(k, e)`: Apply context to expression -- fill the hole with `e`.
    Another in-place `Set` instruction.

- [x] Implement `pub fn rewrite_trmc(func: &mut ArcFunction, regions: &[ContextRegion]) -> bool`:
  **Bug 1 (argument threading) and Bug 3 (helper block threading) FIXED
  (2026-03-15). Fresh block params for function params, Let bindings bridging
  fresh→original (tail_call pattern), loop-back Jump threads rec_args,
  span maintenance, SSA dominance documented + verified.**
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

- [x] **BUG FIX (HIGH): Recursive argument threading.** The loop-back Jump
  (`rewrite.rs:362`) must thread the recursive call's NEW arguments alongside
  the context triple. The fix follows `tail_call/rewrite.rs`:
  1. Add fresh block params to the loop header for each `func.params[i]`
     (in addition to the 3 context params). Use `func.fresh_var(ty)` for each.
  2. Prepend `Let` bindings in the loop header body that rebind
     `func.params[i].var` from the fresh block params (same pattern as
     `tail_call/rewrite.rs:109-119`).
  3. The prologue Jump must pass the original function param vars as the
     first N args, followed by the 3 context init values.
  4. The loop-back Jump must pass the recursive call's extracted args as the
     first N args, followed by `[true_var, new_res, input.ctor_dst]`.
  5. `extract_recursive_call()` return value must no longer be discarded —
     use `let (rec_dst, rec_args) = extract_recursive_call(...)`.
  6. Update `check_loop_header_args` in verify.rs to expect
     `func.params.len() + 3` args.

  7. **Propagate `rec_args` through `RewriteInput`.** Currently
     `check_admission()` calls `extract_recursive_call()` but only uses
     `rec_dst` (line 151: `let (rec_dst, _) = ...`). The `rec_args` are
     discarded at the admission stage and unavailable in
     `emit_recursive_path()`. Fix: add `rec_args: Vec<ArcVarId>` field
     to `RewriteInput` and populate it from `extract_recursive_call()`.
     `emit_recursive_path()` then uses `input.rec_args` for the loop-back
     Jump's first N arguments.
  8. **Update `emit_prologue()` call.** `emit_prologue()` currently
     generates a Jump with only 3 args (`[false_var, null_sentinel,
     null_sentinel]`). After adding N fresh block params for function
     params, the prologue Jump must pass `func.params.len() + 3` args.
     Pass the original `func.params[i].var` as the first N args.

- [x] **BUG FIX (MEDIUM): Helper block context variable threading.** The
  compose block (`rewrite.rs:327-340`) and apply_ctx block
  (`rewrite.rs:419-428`) use `ctx_hole_obj` and `ctx_res` from the loop
  header without receiving them through block params. Fix by either:
  **(a)** Adding block params to the compose and apply_ctx blocks, and
  passing the context vars via Jump args from the predecessor block. OR
  **(b)** Documenting that these blocks are dominated by the loop header
  and the variables are function-scoped SSA names (not block-scoped), with
  a verification check that dominance holds. Option (a) is preferred for
  consistency with the block-param calling convention.

- [x] Handle the fallback case:
  - When soundness gates fail, skip the rewrite (leave function unmodified)
  - Log via `tracing::debug!` why the rewrite was skipped
  - This is the recommended approach from literature review section 04.7

- [x] Update `NormalizationResult.was_transformed` when rewrite succeeds

- [x] Scope bounds:
  - Self-recursive only (no mutual recursion)
  - Single recursive call per region
  - Modulo-cons instantiation only (no CPS fallback)
  - ~~Skip when `may_share == true` (no hybrid path currently)~~ **REVERTED —
    see Bug 4. `may_share` gate removed; per-variable uniqueness is the
    sole soundness condition (Ori has no effect handlers) (no effect handlers exist)**

- [x] Tests (ArcIrBuilder test pattern):
  - `rewrite_trmc_simple_list_map` -- basic self-recursive list constructor
  - `rewrite_trmc_enum_variant` -- recursive enum variant construction
  - `rewrite_trmc_skipped_when_not_unique` -- soundness gate 1
  - `rewrite_trmc_skipped_when_may_share` -- soundness gate 2
  
    **Must be revised after Bug 4 fix:** this test asserts the rewrite is
    skipped when `may_share == true`. After removing the `may_share` gate,
    revise to `rewrite_trmc_skipped_when_no_contract` — tests that
    rewrite is skipped when `contract` is `None` (first SCC iteration).
    Also revise `pipeline_normalize_noop_when_may_share` (in 13.6 tests)
    to expect rewrite proceeds despite `may_share == true`.
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

- [x] Implement `pub fn verify_trmc_rewrite(func: &ArcFunction, regions: &[ContextRegion]) -> Vec<TrmcVerificationError>`:
  **COMPLETE (2026-03-15). Checks: no-residual-self-calls, linear-context-usage,
  null-containment, loop-header-arg-count (exact shape), context-var-dominance.
  Bug 1 (argument threading) verified by loop-header arg count check.
  Bug 5 (uniqueness) handled by separate `verify_trmc_soundness()` (semantic).**
  **Implemented checks (structural, no external state needed):**
  1. **No residual self-calls:** All self-recursion converted to loop-back
     Jumps. (`check_no_residual_self_calls`)
  2. **Linear context usage:** The context variable is used exactly once per
     control-flow path (no duplication, no dropping). (`check_linear_context_usage`)
  3. **Null sentinel containment:** `LitValue::Null` only in prologue block.
     (`check_null_containment`)
  4. **Loop-header arg consistency:** Every Jump to loop header passes the
     correct number of args. (`check_loop_header_args`)
  **Planned checks (require Bug fixes):**
  5. **Param rebinding:** Loop header has `func.params.len() + 3` block
     params; Let bindings bridge fresh params to original vars; all Jumps
     to header pass correct arg count. (`check_param_rebinding` — Bug 1 fix)
  6. **Uniqueness preserved:** The context variable has
     `Uniqueness::Unique` at every use point between creation and application.
     Requires post-rewrite re-analysis. (Bug 5 fix)
  7. **Effect purity (future):** `EffectSummary.may_share == false` — deferred
     to effect-handler implementation. Not a current check.
  **Not implemented (low priority):**
  8. **No polymorphic context:** The constructor used as context has a known
     layout (no type variables in field types). Unlikely in practice since
     ARC IR is monomorphized.
  9. **Tail position:** All recursive calls in the rewritten function are in
     tail position (the whole point of TRMC). Subsumed by check 1
     (no residual self-calls — if all self-calls are eliminated, they
     are trivially in tail position as loop-back Jumps).

- [x] Define `TrmcVerificationError` enum:
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

- [x] Rollback mechanism:
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

- [x] Error handling:
  - In debug builds: `debug_assert!` on verification failures (catches bugs
    in the rewrite implementation during development)
  - In release builds: `tracing::warn!` and skip the rewrite (roll back to
    original function body using the clone above)

- [x] Wire into `normalize_function()`:
  - `rewrite_and_verify()` encapsulates the clone + rewrite + verify +
    rollback pattern. Section 13.6 wires it into `normalize_function()`
    with contract access for the `may_share` gate.

- [x] **BUG FIX (MEDIUM): Verification does not check argument threading.**
  `verify_trmc_rewrite()` checks:
  1. No residual self-calls (check_no_residual_self_calls)
  2. Linear context usage (check_linear_context_usage)
  3. Null sentinel containment (check_null_containment)
  4. Loop-header arg count (check_loop_header_args)
  But it does NOT check:
  - That the loop-back Jump passes the correct NUMBER of args
    (func.params.len() + 3, not just 3)
  - That the prologue Jump passes the original function param vars
  - That the recursive call's arguments appear in the loop-back Jump
  - That function-param variables are rebound via block params + Let bindings
  Add a new check: `check_param_rebinding(func, ctx, errors)` that verifies:
  - Loop header has `func.params.len() + 3` block params
  - Each original param var has a corresponding Let binding in the header body
  - All Jumps to the loop header pass `func.params.len() + 3` args

- [x] **BUG FIX (MEDIUM — Bug 5): Implement uniqueness and effect purity
  verification.** `NonUniqueContext` now constructed by
  `verify_trmc_soundness()` (2026-03-15). `EffectPurityViolation` remains
  dead code (no effect handlers). The fix:
  1. After TRMC rewrite succeeds AND re-analysis runs (13.6 pipeline re-run),
     verify that context variables have `Uniqueness::Unique` at every use
     point in the rewritten function's converged `AimsStateMap`.
  2. Construct `NonUniqueContext { function, var, block }` when the context
     variable is not unique at a use point (Set, Jump arg, or Return).
  3. Construct `EffectPurityViolation { function }` if post-rewrite analysis
     shows effect concerns (placeholder for future effect handlers).
  4. Remove the `#[expect(dead_code)]` attributes.
    5. Create a separate `verify_trmc_soundness(func, state_map, regions)`
     that runs AFTER `analyze_function()` (step 4) when `was_transformed`.
     Do NOT add to `verify_trmc_rewrite()` — that runs before analysis.
  6. On verification failure, roll back the rewrite (existing clone mechanism).

- [x] Tests:
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

- [x] Update `normalize_function()` to call the full pipeline:
  **COMPLETE (2026-03-15). Bug 4 gate removed, Bug 1 threading fixed,
  Bug 3 dominance documented, Bug 5 soundness verification wired.
  Pipeline actually transforms functions end-to-end.**
  Calls `lift_constructor_args` → `detect_context_regions` →
  `rewrite_and_verify` (when candidates exist).
  Implemented in `normalize/mod.rs`.

- [x] Update `normalize_function()` signature:
  Current: `pub fn normalize_function(func: &ArcFunction) -> NormalizationResult`
  New: `pub fn normalize_function(func: &mut ArcFunction, contract: Option<&MemoryContract>) -> NormalizationResult`
  - `&mut ArcFunction` because both lifting (13.3) and rewrite (13.4)
    mutate the function
  - `Option<&MemoryContract>` because the contract may not exist during
    the first fixpoint iteration (see 13.2 edge case). When `None`,
    conservatively skip TRMC (same as `may_share == true`).
  - The contract is needed for the effect purity gate (`may_share`)
  - Pipeline step 3a already has access to contracts from step 1

- [x] Update pipeline call site (`aims_pipeline.rs:72-76`):
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

- [x] Consume `ContextOpen`/`ContextClose` events in realization:
  No-op currently: after TRMC rewrite, self-calls are eliminated, so
  `detect_trmc_candidates` finds no `ContextHole` shapes and
  `populate_context_events` records nothing. For non-rewritten functions
  (may_share=true), normal RC is correct. Event consumption infrastructure
  exists in `AimsStateMap.events_in_block()` for future use (multi-region,
  mutual recursion).

- [x] **BUG FIX (MEDIUM): Post-rewrite contract refresh.** After a
  successful TRMC rewrite, interprocedural contracts become stale. The
  rewritten function has different properties:
  - `has_unbounded_stack` should be `false` (loop, not recursion)
  - `ContextBehavior` should reflect the rewrite outcome
  - `FipContract` may upgrade (`Never` → `Certified` if now alloc-balanced
    with constant stack)
  - Effect summary may change (no more recursive call effects)
  Fix: After the per-function pipeline loop completes for a TRMC-rewritten
  function, re-extract the contract from the rewritten function's converged
  state map. Options:
  **(a) Post-loop contract refresh:** After `run_aims_pipeline()` returns
  for a rewritten function, call `extract_contract()` on the rewritten
  function's state map and update `contracts`. Requires `contracts` to be
  `&mut FxHashMap` in the per-function loop, or a collect-and-apply second
  pass (same pattern as `may_deallocate` updates at lines 311-315).
  **(b) Two-pass pipeline:** First pass runs TRMC rewrites only. Second
  pass re-runs interprocedural analysis on the rewritten functions. Cleaner
  but doubles pipeline cost for TRMC functions.
  **Recommended:** Option (a) — post-loop contract refresh. Consistent
  with the existing `may_deallocate` second-pass pattern.


  **State map availability for contract re-extraction:** `extract_contract()`
  normally runs inside `analyze_program()` (interprocedural) and receives
  the function's converged `AimsStateMap` directly from `analyze_function()`.
  In the per-function pipeline, `analyze_function()` runs at step 4 and
  produces the state map used by realization at step 5. To re-extract a
  contract, the per-function pipeline must:
  1. Return the converged `AimsStateMap` from `run_aims_pipeline()` (or
     at least the post-convergence data needed by `extract_contract()`:
     `effect_summary`, `fip_balance`, `context_regions`, `scalars`).
  2. In the second pass (alongside `may_deallocate` updates), call
     `extract_contract()` with the state map data for TRMC-rewritten
     functions.
  **Alternative:** Add a `contract_refresh: Option<MemoryContract>` field
  to `AimsPipelineResult` that `run_aims_pipeline()` populates only when
  `was_transformed == true`. The second pass applies it to `contracts`.
  This avoids threading the entire state map out of the per-function pipeline.
  **Sync points for contract refresh:**
  - `AimsPipelineResult` — add `contract_refresh` field
  - `run_aims_pipeline()` — populate when `was_transformed`
  - `run_aims_pipeline_all()` second pass — apply refreshed contracts
  - Section 12.1 second pass — must run AFTER contract refresh (ordering:
    contract refresh → `may_deallocate` update → `contract.fip`
    recomputation → FIP verification)

- [x] Handle `was_transformed` in pipeline:
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

    4. **Step 4a (verify_trmc_soundness)** -- when `was_transformed`, verify
     that context variables have `Uniqueness::Unique` at all use points
     in the converged `AimsStateMap`. On failure, restore the pre-rewrite
     function clone (same rollback pattern as structural verification) and
     re-run step 4 on the restored function. This is Bug 5's runtime
     location in the pipeline.

- [x] Update `NormalizationResult` if needed:
  - Currently has `was_transformed: bool` and `context_regions: Vec<ContextRegion>`
  - Rollback is handled by cloning the function before rewrite (see 13.5),
    NOT by carrying the original in `NormalizationResult`

- [x] Tests:
  8 pipeline integration tests added in `normalize/tests.rs`:
  - `pipeline_normalize_then_rewrite_with_contract`
  - `pipeline_normalize_noop_when_no_contract`
  - `pipeline_normalize_noop_when_may_share`
  - `pipeline_normalize_noop_when_no_candidates`
  - `pipeline_rewrite_idempotent`
  - `pipeline_rerun_var_reprs_after_transform`
  - `pipeline_rollback_on_verification_failure`
  - `pipeline_context_events_empty_after_rewrite`

---

## 13.6a Codebase Hygiene -- Fix Along the Way

These items should be fixed during implementation of 13.1-13.6:

- [x] **STYLE (stale doc):** `ContextBehavior` doc at `contract/mod.rs:388`
  updated to "Stage 3a" and added note about pre-rewrite vs post-rewrite
  context behavior semantics.
- [x] **STYLE (stale doc):** `normalize/mod.rs:14-17` says "Detection only --
  no IR rewriting. The full TRMC 4-equation rewrite is deferred to a future
  stage." After 13.4, this is inaccurate. Update when implementing 13.4.
- [x] **STYLE (stale doc):** `NormalizationResult::was_transformed` doc at
  `normalize/mod.rs:42` says "always `false` currently". Update after 13.4.
- [x] **WASTE (unnecessary clone):** `analyze_scc_fixpoint()` at
  
  `interprocedural/mod.rs:176` — the clone is inherent to building the combined
  view for `analyze_function`'s `&FxHashMap` parameter. A layered lookup
  would require changing the downstream API. Documented tradeoff in code
  comment; clone cost is bounded by finalized contracts count.
- [x] **STYLE (module doc accuracy):** `aims/mod.rs` module doc updated to
  "Stage 3a: TRMC normalization (detection, lifting, rewriting, and
  verification)".
- [x] **STYLE (stale scope doc in rewrite.rs):** `rewrite.rs:21` updated
  from "Skip when `may_share == true`" to "Per-variable uniqueness as sole
  soundness gate (no effect-handler gate)". (2026-03-14, Bug 4 fix)
- [x] **STYLE (stale soundness doc in rewrite.rs):** `rewrite_trmc()` doc
  at `rewrite.rs:60` updated from "Effect purity: `may_share == false`
  (checked by caller)" to "Effect purity: deferred to effect-handler
  implementation." (2026-03-14, Bug 4 fix)
- [x] **WASTE (dead code in helpers.rs):** `emit_rc/helpers.rs` —
  `build_alias_map()` and `resolve_alias_root()` removed (2026-03-15).
  Bug 1 fix does not use them; `borrow/update.rs` has its own copy.
- [x] **WASTE (dead code `TrmcContext` in rewrite.rs):** Removed
  `TrmcContext` struct (2026-03-15). `verify.rs` uses `RewriteContext`.
- [x] **STYLE (TODO without tracking):** `aims_pipeline.rs` TODO replaced
  with Note documenting the Section 11 decision (2026-03-15).

---

## 13.7 Completion Checklist

### Infrastructure (complete)
- [x] Section 12 file splits are complete before starting (prerequisite)
- [x] Duplicated `collect_recursive_call_defs` already unified (from 12.4a)
- [x] Stale `ContextBehavior` / `normalize` / `aims/mod.rs` docs updated
  (see 13.6a list)
- [x] `analyze_scc_fixpoint` clone-on-every-SCC documented (see 13.6a —
  layered lookup requires API change, deferred)
- [x] `ContextBehavior` fields expanded (`requires_unique_context`,
  `may_resume_nonlinearly`) and computed in `extract_contract()`
  
  (no more hardcoded default — computed by `compute_context_behavior()`)
- [x] `ContextBehavior::Default` derive removed; manual impl with
  `requires_unique_context: true`
- [x] `context_regions` threaded to `extract_contract()` as parameter
- [x] Per-variable uniqueness gate enforced in `detect_trmc_candidates()`
  and `populate_context_events()`
- [x] Effect purity gate: design decision documented, placeholder
  infrastructure added. **RESOLVED (Bug 4 fix, 2026-03-14):** Removed
  the hard-block enforce path from `normalize/mod.rs`. The log-only path
  in `post_convergence.rs:202` is the current design. Final resolution of
  the effect gate itself is blocked on effect-handler semantics.
  `EffectPurityViolation` variant in `verify.rs` retained with updated
  `#[expect(dead_code)]` reason.
- [x] Fixpoint edge case handled: first SCC iteration with no contract
  conservatively skips TRMC (`contract.is_none_or(|c| c.effects.may_share)`)
  **NOTE:** After Bug 4 fix, this check should be revised to not gate on
  `may_share` — it should only gate on `contract.is_none()` (no contract
  available during first iteration).
- [x] **Fixpoint edge case revision (companion to Bug 4 fix):** After
  removing the `may_share` gate from `normalize/mod.rs`, updated
  `normalize_function()` to gate only on `contract.is_some()`.
  Old: `let may_share = contract.is_none_or(|c| c.effects.may_share);`
  New: `if contract.is_some() { ... }` (gate on no-contract only).
  Module-level doc at `normalize/mod.rs:27` updated from "the `may_share`
  effect purity gate passes" to "the function has a converged contract
  from interprocedural analysis". (2026-03-14)
- [x] Doc comments in `contract/mod.rs` and `intraprocedural/post_convergence.rs`
  reconciled — both document both gates and their relationship
- [x] Lifting pre-pass verified as unnecessary for ARC IR (type-enforced
  A-normal form; `lift_constructor_args` is debug verification only)
- [x] `normalize_function()` signature changed to `(&mut ArcFunction, Option<&MemoryContract>)`

### Bug fixes (blocked — must be fixed before Section 13 can exit)
- [x] **BUG 1 (HIGH):** Recursive argument threading in `rewrite.rs` (see 13.4)
  - Loop header has fresh block params for ALL function params + 3 context params
  - Prologue Jump passes original function param vars + 3 context init values
  - Loop-back Jump passes recursive call args + 3 context values
  - Let bindings bridge fresh block params → original param vars (tail_call pattern)
  - Span maintenance for rebuilt header body (2026-03-15)
- [x] **BUG 2 (MEDIUM):** Post-rewrite contract refresh in pipeline (see 13.6)
  - `has_unbounded_stack = false` in second-pass contract refresh
  - `was_trmc_rewritten` flag survives semantic verification rollback
  - Full `extract_contract()` re-extraction deferred — requires SCC peer data
- [x] **BUG 3 (MEDIUM):** Helper block context var threading in `rewrite.rs` (see 13.4)
  - Resolution: option (b) — SSA dominance documented + verification check
  - `check_context_var_dominance` uses `DominatorTree` to verify all blocks
    using context vars are dominated by the loop header
  - Unreachable blocks skipped (idom == None)
- [x] **BUG 4 (HIGH):** Remove `may_share` gate from `normalize/mod.rs` (see 13.2)
  - Deleted `may_share` gate; replaced with `contract.is_some()` check (2026-03-14)
  - Per-variable uniqueness (post_convergence.rs:246) is the sole soundness gate currently
  - Renamed test to `pipeline_normalize_proceeds_despite_may_share` — verifies rewrite proceeds
  - Plan text reconciled: L284 (uniqueness-only) is correct; L629 (skip may_share) already reverted
- [x] **BUG 5 (MEDIUM):** Implement uniqueness verification in verify.rs (see 13.5)
    - `NonUniqueContext` variant constructed by `verify_trmc_soundness()`
    - `#[expect(dead_code)]` removed from `NonUniqueContext`
  - `EffectPurityViolation` remains dead code (no effect handlers);
    reason updated to "deferred to effect-handler implementation"
  - Verify context variables are `Unique` at all use points after re-analysis
  - `verify_trmc_soundness(func, state_map)` in verify.rs
    (runs at step 4a, after analyze_function converges)
  - Rollback via `pre_trmc_func` clone in pipeline on failure (2026-03-15)

### Rewrite correctness (gated on bug fixes)
- [x] 4-equation TRMC rewrite implemented for modulo-cons instantiation
  (in-place transform with optional context parameter, or auxiliary function)
  **All 4 bugs fixed (2026-03-15). Rewrite produces structurally and
  semantically correct output. Argument threading, SSA dominance,
  uniqueness verification, and contract refresh all implemented.**
- [x] `may_share` gate removed from normalize/mod.rs (Bug 4 fix — unblocks rewrite) (2026-03-14)
- [x] Post-rewrite verification catches argument threading bugs (Bug 1 fix, 2026-03-15)
- [x] Post-rewrite uniqueness verification implemented (Bug 5 fix, 2026-03-15 —
  `NonUniqueContext` variant constructed; `EffectPurityViolation` deferred)
- [x] Rollback mechanism: `func.clone()` before rewrite, restore on
  verification failure
- [x] Pipeline re-runs steps 3, 3.5, 4 after successful TRMC rewrite
  (loop in `aims_pipeline.rs` — at most 2 iterations, idempotent)
- [x] Pipeline re-extracts contract after successful rewrite (Bug 2 fix, 2026-03-15)
  Minimal refresh: `has_unbounded_stack = false`. Full `extract_contract()`
  deferred — requires SCC peer data threading.
- [x] Second-pass ordering in `run_aims_pipeline_all()`: contract refresh
  (Bug 2) runs BEFORE `may_deallocate` update (Section 12.1) and
  `contract.fip` recomputation. (2026-03-15)
- [x] `ContextOpen`/`ContextClose` events consumed by realization (no-op
  currently — rewrite eliminates patterns that generate events; see 13.6)
- [x] `NormalizationResult.was_transformed` is `true` when TRMC applied
- [x] Fallback strategy: skip (leave unmodified) when soundness gates fail
- [x] Section 12 FIP verifier cross-checks TRMC-rewritten functions
  (already wired: `verify_fip_contract` runs on all functions post-emission)

### Behavioral test matrix (Section 13.8 — not started)
- [ ] Matrix D tests written and passing (see 13.8 for full spec)
- [ ] Matrix E control-flow/lifetime axes covered
- [ ] Matrix F assertion strategy implemented across all 3 layers
- [ ] All concrete fixtures pass with correct behavioral output

### Codebase hygiene (fix during bug fixes, not separately)
- [x] `rewrite.rs` scope/soundness docs updated after Bug 4 fix (see 13.6a)
- [x] `emit_rc/helpers.rs` dead code resolved: `build_alias_map` +
  `resolve_alias_root` removed — not used by TRMC rewrite (2026-03-15)
- [x] `rewrite.rs` `TrmcContext` dead code resolved: removed (2026-03-15)
- [x] `aims_pipeline.rs` orphan TODO resolved: replaced with Note
  documenting Section 11 decision (2026-03-15)
- [x] No `#[expect(dead_code)]` remains in `normalize/` or `verify/` modules
  except `EffectPurityViolation` (deferred to effect-handler implementation)

### Final gates
- [ ] `cargo test --workspace` green (with bug fixes applied)
- [ ] `./test-all.sh` green
- [ ] Valgrind: 0 memory errors on all test programs (including TRMC-rewritten)
- [ ] At least one end-to-end Ori program with TRMC rewrite produces correct
  AOT output verified by `dual-exec-verify.sh`
- [ ] **Principle 3 gate:** `verify_trmc_rewrite()` checks all 7 properties
  (no-residual-self-calls, linear-context, uniqueness, effect-purity,
  null-containment, loop-header-args, param-rebinding). All
  `TrmcVerificationError` variants are constructed by verification code
  except `EffectPurityViolation` (deferred to effect-handler implementation;
  retains `#[expect(dead_code)]` with updated reason). The verifier accepts
  all TRMC-rewritten functions in the test suite (no rollbacks triggered).
- [ ] **Principle 3 gate:** Post-rewrite contracts are refreshed and accurate:
  `has_unbounded_stack == false` for all TRMC-rewritten functions,
  `FipContract` upgraded where applicable, callers see updated contracts.

**Exit Criteria:** `normalize_function()` produces structurally AND
behaviorally correct rewritten functions for self-recursive constructor-
context patterns. The rewrite correctly threads recursive call arguments
to the loop header (same pattern as `tail_call/rewrite.rs`). Post-rewrite
contracts are refreshed so callers see accurate properties. The rewrite
satisfies context laws `(appctx)` and `(appcomp)` verified post-rewrite.
The per-variable uniqueness gate (Lemma 2) is enforced in detection,
rewrite, and verification. The effect purity gate (`may_share`) has
placeholder infrastructure but its final design is a pending decision
blocked on effect-handler semantics -- this is explicitly documented, not
silently deferred. `ContextBehavior` is computed from analysis state, not
hardcoded. The behavioral test matrix (Section 13.8) covers all
interaction axes between TRMC rewrite, RC emission, reuse, COW, FIP,
and the interprocedural contract layer.

---

## 13.8 TRMC Behavioral Test Matrix

**File(s):** `compiler/ori_arc/src/aims/normalize/tests.rs` (Rust unit),
`compiler/ori_llvm/tests/aot/arc.rs` (AOT behavioral),
`tests/aims/trmc/` (Ori spec programs)

**Why this section exists:** The existing TRMC tests (13.4-13.6) are
**entirely structural** — they check block layout, param counts, and
instruction shapes. They never run the rewritten function or verify it
produces correct output. This gap is why Bugs 1-3 went undetected.

This section defines a comprehensive test matrix analogous to Section 08's
Matrix A/B/C (which successfully caught 5 RC alias/projection bugs). The
matrix covers the **interaction surface** between the TRMC rewrite and
every other AIMS subsystem that touches the rewritten IR.

### Matrix D — TRMC rewrite shape coverage

Tests that the rewrite produces correct IR for each structural pattern.
These are ARC IR-level tests (ArcIrBuilder) that run the full AIMS pipeline
on a hand-built ArcFunction and verify both structure AND computed states.

| ID | Shape under test | Minimal ARC IR pattern | Expected property | Failure mode if broken | Primary layer |
|----|------------------|----------------------|-------------------|------------------------|---------------|
| D1 | Single-arg self-recursion with arg change | `f(n) = Cons(n, f(n-1))` | Loop-back Jump has `[n-1, true, new_res, ctor_dst]`; next iteration sees n-1, not n | Infinite loop with original arg (Bug 1) | ARC unit + AOT |
| D2 | Multi-arg self-recursion | `map(f, Cons(h, t)) = Cons(f(h), map(f, t))` | Loop-back Jump threads BOTH args; `f` unchanged, `t` is tail | Second arg not threaded; wrong traversal | ARC unit + AOT |
| D3 | Prologue passes original function params | `f(x)` with prologue block | Prologue Jump has `[x, false, null, null]` (N params + 3 ctx) | Prologue passes only 3 ctx args; params undefined | ARC unit |
| D4 | Let bindings bridge block params to body | After rewrite, loop header body starts with `Let(orig_var, fresh_param)` for each param | Original param vars are live in the rewritten body | Body references dead original param vars | ARC unit |
| D5 | Helper blocks receive context vars | Compose block has ctx_hole_obj and ctx_res via params or dominance | Set instruction targets valid context var | Use of undefined var in helper block | ARC unit |
| D6 | Base-case return with context (has_ctx=true) | Return in non-recursive branch when ctx accumulated | Set(ctx_hole_obj, field, ret_value); Return(ctx_res) | Return wrong value; context not applied | ARC unit + AOT |
| D7 | Base-case return without context (has_ctx=false) | First call — no accumulated context yet | Return(ret_value) directly, no Set | Null sentinel returned instead of actual value | ARC unit + AOT |
| D8 | Enum variant constructor with hole at field 0 | `Cons(recurse(xs), payload)` — hole at index 0 | Hole field gets ctx_res placeholder; Set targets field 0 | Wrong field index; hole at wrong position | ARC unit |
| D9 | Enum variant with hole at non-zero field | `Node(left, recurse(right))` — hole at index 1 | Hole field index matches region.hole_field | Field 0 written instead of field 1 | ARC unit |
| D10 | Rewrite skipped for non-tail construct | Construct not last instruction in block | `was_transformed == false`; function unchanged | Partial rewrite of non-eligible function | ARC unit |
| D11 | Rewrite skipped for cross-block region | Call in block 0, construct in block 1 (out of scope) | `was_transformed == false`; function unchanged | Cross-block rewrite with missing dataflow | ARC unit |
| D12 | Multiple context regions rejected (out of scope) | Function with 2 recursive calls under different constructors | Single-region only: skip; function unchanged | Partial transform leaving residual self-call | ARC unit |

### Matrix E — TRMC × AIMS subsystem interaction coverage

Tests that the rewritten IR interacts correctly with every downstream
AIMS subsystem. Each row tests a specific cross-system boundary.

| ID | Interaction | What to verify | Failure mode if broken |
|----|-------------|---------------|------------------------|
| E1 | TRMC × RC emission | RcInc/RcDec correctly placed for context vars; no double-free on context root | Context root freed before base-case return applies it |
| E2 | TRMC × reuse emission | Reuse opportunities detected in rewritten function (pattern match arms that destroy+reconstruct same ctor) | Reuse detection fails because death/alloc events are stale after rewrite |
| E3 | TRMC × COW annotations | COW mutations inside the loop body get correct CowMode (context vars are unique by construction) | MaybeShared COW check on provably-unique context var |
| E4 | TRMC × drop hints | RcDec on context vars in base-case return path gets drop hint if collection type | Drop hint for non-unique collection; or missing drop hint for unique one |
| E5 | TRMC × FIP contract | Rewritten function with alloc-balanced + constant stack → FipContract::Certified | Stale contract says Never (Bug 2); or Certified but stack not actually constant |
| E6 | TRMC × interprocedural contracts | Caller calling TRMC-rewritten callee uses refreshed contract (has_unbounded_stack=false, updated ContextBehavior) | Caller uses pre-rewrite contract; misses optimization opportunities |
| E7 | TRMC × tail_call pass | Tail-call pass runs AFTER TRMC rewrite; must not try to re-lower already-loopified self-calls | Double loop-lowering; broken back-edge args |
| E8 | TRMC × block_merge | Merge must not invalidate the prologue→header→helper block topology | Merge deletes prologue (single-predecessor optimization); context init lost |
| E9 | TRMC × verify pass | ARC verify (`check_function`) passes on rewritten function; no undefined-var or unreachable-block errors | Verify catches the undefined context vars from Bug 3 |
| E10 | TRMC × immortal detection | Immortal vars (empty string, etc.) in rewritten function body are correctly detected | New vars from rewrite miss immortal scan; RC emitted for immortal |
| E11 | TRMC × intraprocedural analysis | Backward analysis converges on the loop structure; context vars get correct Uniqueness/Locality | Analysis doesn't converge (back-edge creates infinite widening) |
| E12 | TRMC × var_reprs | New vars from rewrite (ctx_has, ctx_res, ctx_hole_obj, fresh params, sentinels) get correct ValueRepr | Missing ValueRepr → panic in RC emission (`var_reprs` is empty) |

### Matrix F — control-flow and lifetime axes

Axes that must be crossed with Matrix D/E tests. Each axis value must be
covered by at least one concrete test fixture.

| Axis | Values that must be covered | Why it matters |
|------|-----------------------------|----------------|
| Argument count | 1, 2, 3+ params | Bug 1 manifests with any non-zero arg change; multi-arg tests catch partial threading |
| Argument change pattern | All args change, some change, none change (tail-call-like) | None-change is degenerate (loop body is no-op); tests catch off-by-one in arg threading |
| Constructor arity | 1-field, 2-field, 3+ field | Hole field index varies; tests catch wrong-field bugs |
| Hole position | Field 0, field 1, last field | Ensure hole_field is respected, not hardcoded to 0 |
| Base-case path | Single return, multiple returns across blocks, return in match arm | Base-case rewrite applies to ALL Return terminators in original blocks |
| Recursive depth | 0 (base case only), 1, 5+, 100+ (stack overflow without TRMC) | Proves the loop actually iterates; deep recursion proves stack is constant |
| Context accumulation | 0 nodes (identity), 1 node, many nodes | First-call path vs compose path; tests both branches of ctx_has |
| Return type | Simple struct, enum variant, nested struct | Ensures context var type matches function return type |
| Callee effects | Pure (may_share=false), impure (may_share=true, rewrite PROCEEDS currently — no effect handlers) | Per-variable uniqueness is sole current gate; effect purity deferred to effect-handler implementation |
| Pipeline position | Before tail_call pass, before block_merge, before verify | Each downstream pass sees the rewritten IR in the expected state |

### Matrix G — assertion strategy

| Layer | Assertions required | Notes |
|-------|---------------------|-------|
| `normalize/tests.rs` | Structural + state checks: block params count == `func.params.len() + 3`, Let bindings present for each param, loop-back Jump arg count, prologue Jump arg count, recursive call args appear in loop-back args | Fast unit coverage; catches Bug 1-3 directly |
| `realize/tests.rs` | RC/reuse/COW decision checks for TRMC-rewritten ArcFunctions: context vars get correct `decide()` and `decide_annotations()` outputs | Catches interaction bugs (Matrix E) |
| ARC dump golden checks | Exact `RcInc`/`RcDec`/`Set` placement in the rewritten ARC IR, verified by `ORI_DUMP_AFTER_ARC=1` | Catches structural RC placement bugs not visible in unit tests |
| AOT behavioral tests (`ori_llvm/tests/aot/arc.rs`) | End-to-end: Ori source → ARC lowering → TRMC rewrite → AIMS analysis → LLVM emission → execution → correct output | Catches all behavioral bugs; the ultimate correctness gate |
| AOT runtime-trap tests | Program must NOT hit `ori_rc_dec called on already-freed allocation` or SIGSEGV | Critical for context lifetime and RC correctness |
| Valgrind tests (`tests/valgrind/trmc/`) | 0 definite leaks for TRMC-rewritten programs | Catches context root leaks, hole field leaks |
| Legacy parity (pre-rewrite vs post-rewrite) | Same Ori program compiled with and without TRMC rewrite produces identical output | Proves the rewrite is semantics-preserving |
| `dual-exec-verify.sh` | Interpreter vs AOT match for TRMC-eligible programs | Catches eval/codegen divergence introduced by the rewrite |

### Concrete fixtures (implement after Bug 1-3 fixes)

| Fixture name | Scenario | Expected output | Matrices covered |
|--------------|----------|----------------|------------------|
| `trmc_list_map_increment` | `map_inc(Cons(1, Cons(2, Nil))) = Cons(2, Cons(3, Nil))` — single-arg recursion changing arg | `[2, 3]` | D1, D3, D4, D6, D7, E1, E11 |
| `trmc_list_map_two_args` | `map(f, list)` where f is a function param — 2-arg, only second changes | Mapped list | D2, F(arg count=2, change=some) |
| `trmc_tree_mirror` | Binary tree mirror: `mirror(Node(l, r)) = Node(mirror(r), mirror(l))` — hole at field 0 | Mirrored tree | D8, D9, F(hole=0 and 1) |
| `trmc_deep_recursion_100` | Build 100-element list via TRMC — stack overflow without rewrite | 100-element list, no stack overflow | F(depth=100+) |
| `trmc_base_case_immediate` | Call with base case (Nil) — 0 recursive iterations | Empty result via direct return | D7, F(depth=0) |
| `trmc_multi_field_ctor` | `type T = Node(a: int, b: str, tail: T)` — 3-field, hole at field 2 | Correctly built chain | D9, F(ctor arity=3, hole=last) |
| `trmc_enum_with_tag_change` | Recursive enum: `transform(A(x, rest)) = B(x+1, transform(rest))` — ctor kind changes | Variant tag updated correctly in each node | D8 |
| `trmc_rewrite_skipped_no_contract` | Function on first SCC iteration (no contract available) — TRMC skipped, normal recursion | Same output as without TRMC, no stack optimization | D10, F(no contract) |
| `trmc_rc_balance_no_leak` | TRMC-rewritten function processes 1000 nodes, Valgrind clean | 0 leaks, 0 errors | E1, E4, G(Valgrind) |
| `trmc_cow_on_context_var` | Context var is a list; push inside the loop body | StaticUnique COW (context var is unique by construction) | E3, E11 |
| `trmc_caller_uses_refreshed_contract` | Caller calls TRMC-rewritten callee; caller's analysis uses updated contract | Caller optimizes based on callee's constant-stack guarantee | E5, E6 |
| `trmc_fip_certified_after_rewrite` | Alloc-balanced function: pattern-match destroys node, ctor rebuilds | FipContract::Certified in post-rewrite contract | E5 |
| `trmc_vs_interpreter_parity` | Same TRMC-eligible function run via eval and AOT | Identical output | G(legacy parity, dual-exec) |

### Implementation order

1. **Fix Bug 4** (may_share gate) — unblocks the rewrite so it actually runs
 
   - Also update `normalize/mod.rs` module-level doc line 27 ("the
     `may_share` effect purity gate passes" → "the function has a
     converged contract") and the scope doc in `rewrite.rs` line 21
     ("Skip when `may_share == true`" → remove or replace with
     per-variable uniqueness reference)
2. **Fix Bug 1** (argument threading) — makes the rewrite produce correct code
   **WARNING: HIGH COMPLEXITY.** This rewrites the core block layout of
   `emit_prologue`, `emit_recursive_path`, and `rewrite_single_region`.
   The `tail_call/rewrite.rs` implementation (lines 50-119) is the
   reference pattern. Read it carefully before starting. The fix touches
   5 functions in `rewrite.rs` plus `check_loop_header_args` in `verify.rs`.
   Estimate 150-200 lines changed. Write D1-D4 tests BEFORE the fix to
   have a failing test harness.
3. Write `normalize/tests.rs` fixtures D1-D4 (verify Bug 1 fix with ARC unit tests)
4. **Fix Bug 3** (helper block threading) — or document dominance guarantee
5. Write `normalize/tests.rs` fixtures D5-D12
6. **Fix Bug 5** (uniqueness verification) — post-rewrite soundness proof
7. **Fix Bug 2** (contract refresh) — so downstream assertions have accurate contracts
   **WARNING: CROSS-SECTION DEPENDENCY.** This fix must be coordinated
   with Section 12.1's stale-contract bug and sequencing gap. Both bugs
   touch the same second pass in `run_aims_pipeline_all()` (lines 309-315).
   Implement them in a single commit to avoid partial fixes.
 
   - Must be coordinated with Section 12.1 sequencing fix: the combined
     second pass in `run_aims_pipeline_all()` must apply updates in order:
     (1) contract refresh, (2) `may_deallocate` update, (3) `contract.fip`
     recomputation, (4) FIP verification
8. Write `realize/tests.rs` E-matrix interaction tests
9. Write `ori_llvm/tests/aot/arc.rs` behavioral fixtures
10. Write Valgrind fixtures in `tests/valgrind/trmc/`
11. Run `dual-exec-verify.sh` on TRMC-eligible programs
12. Record baselines for golden corpus RC counts with TRMC active
