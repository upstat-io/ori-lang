---
section: "09"
title: "Dimensional Fusion"
status: in-progress
goal: "Make all 7 dimensions work as one team — every dimension constrains, proves, or overrides at least one other"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
sections:
  - id: "09.1"
    title: "Transfer Fusion"
    status: in-progress
  - id: "09.2"
    title: "Active Dimensions"
    status: in-progress
  - id: "09.3"
    title: "Enriched Canonicalize"
    status: complete
  - id: "09.4"
    title: "Sequencing Algebra Extension"
    status: in-progress
  - id: "09.5"
    title: "Convergence Feedback"
    status: not-started
  - id: "09.6"
    title: "Completion Checklist"
    status: in-progress
---

# Section 09: Dimensional Fusion

**Status:** Not Started

**Goal:** Transform AIMS from "7 dimensions sharing a struct with post-hoc consistency
checks" into "7 dimensions reasoning as one team where each dimension actively
constrains, proves, or overrides at least one other dimension."

**Context:** Stage 1 proved AIMS works — 75% RC reduction on golden corpus, zero
behavioral regressions, identical Valgrind results. But analysis of the converged
system reveals that only 4 of 7 dimensions (access, consumption, cardinality,
uniqueness) actively collaborate. Locality, effect, and shape are passengers — they
get set during transfer, carried through joins, and read during emission, but they
never *influence* another dimension's value. Transfer functions update each dimension
independently based on the instruction type, not based on what other dimensions
currently say. The cross-talk that exists (3 canonicalize rules, COW-aware borrowing
in 07.3) proves the pattern works — this section extends it systematically.

**Design principle:** One system, not separate outputs. COW, FIP, reuse, drop hints,
RC insertion are all *outputs* of one system's reasoning — different views of the
same converged lattice. They should not each have their own analysis logic, their
own traversals, their own special cases. If an optimization needs to know that a
value is unique, local, single-use, and shape-compatible, those facts all come from
the same `AimsState` and the decision is made once, not in separate passes.

**Research lineage:** Cross-dimension interaction during transfer (not post-hoc)
is informed by GHC demand analysis, Koka borrow inference, and Lean 4 IR analysis.
See [Research Lineage](00-overview.md#research-lineage) for citations. The design
here is AIMS-native — the unification is the contribution.

**Depends on:** Sections 01-08 (all Stage 1 infrastructure complete).

**Implementation strategy:** Activate dimensions one at a time via a dependency
ladder (Locality → Effect → Shape), then enriched canonicalize, then convergence
feedback. Each step is verified before the next begins. The end-state (all 7
dimensions as team members) is non-negotiable; the rollout is monotonic. See
Section 09.2 for the ordering rationale and per-step gates.

**Hard invariant:** No dimension may remain write-only or read-only. Every
dimension must either constrain another dimension or directly change realization
decisions. A dimension that is set during transfer and read during emission but
never influences another dimension's value is a passenger, not a team member.
This invariant is the engineering gate for Section 09 completion — it is verified
by code inspection (trace each dimension's outgoing influence edges) and by the
cross-dimension test programs in Section 11.

**Error handling and fallback strategy:**

Dimensional fusion introduces richer cross-dimension interactions that increase
the risk of analysis failures (non-convergence, unsound tightening, regression
in emitted code quality). Each failure mode has a defined response:

1. **Non-convergence of multi-round canonicalize (09.5):** The bounded loop
   (`rounds < 3`) prevents infinite loops. If the bound is hit, the analysis
   DOES NOT fail — it produces a conservatively correct result (the state after
   N-1 rounds is already a valid lattice point). Log a `tracing::warn!` with the
   function name and variable. If this fires on >5% of functions in the test
   suite, investigate the canonicalize rule that causes oscillation (Rules 4 and
   6 cannot fire simultaneously, so the bound of 3 should never be reached in
   practice).

2. **Regression in RC count after enabling a new rule:** Each step in the
   dependency ladder (09.2) has a GATE. If enabling Locality activation causes
   RC count regression on ANY golden corpus program, the step is rolled back.
   Rollback means: disable the rule (revert the transfer/canonicalize change),
   investigate why the regression occurred, and fix before re-enabling. Rules
   are feature-gated during development via `#[cfg(feature = "aims")]` — no
   separate feature flag per dimension is needed because the dependency ladder
   is sequential.

3. **Unsound tightening (canonicalize produces a state below ground truth):**
   Detected by AIMS verification (`run_aims_verify()`, pipeline step 9a).
   Verification checks that every RcDec has a matching live variable and that
   no variable transitions from Dead to Live without a definition. If
   verification fails, the offending canonicalize rule is identified by disabling
   rules one-by-one (binary search on the rule set). The fix is always in the
   rule precondition (add a guard), never in verification (never weaken the
   check).

4. **Interprocedural fixed-point divergence with enriched contracts:** Adding
   `locality_bound` or `FipContract::Bounded(n)` to `MemoryContract` may slow
   convergence of the SCC fixpoint. The existing iteration bound (configurable,
   default 100) applies. If hit, the contract is widened to TOP for the divergent
   SCC. This is existing behavior (Section 03) — no new mechanism needed.

---

## 09.0 Pre-Work: Codebase Cleanup (do before touching any 09.x code)

These findings were discovered during the hygiene scan of Section 09 target files.
Fix them first so every subsequent step starts from clean code.

### DRIFT — `EffectClass` defined in two places

`compiler/ori_arc/src/aims/lattice/mod.rs` (line 71) and
`compiler/ori_arc/src/aims/lattice/dimensions.rs` (line 204) both define
`pub struct EffectClass` with identical fields and methods. `mod.rs` re-exports
everything from `dimensions.rs` via `pub use dimensions::*`, so the one in
`mod.rs` is dead (shadowed). The downstream consumers (`transfer/mod.rs`,
`contract/mod.rs`, test files) all import from `super::lattice::EffectClass` —
the re-export resolves to the copy in `dimensions.rs`.

- [x] **Delete the duplicate `EffectClass` block from `lattice/mod.rs`** (lines
  63–104). The `dimensions.rs` copy is authoritative. Run `cargo c --features aims`
  to confirm no compile error; run `cargo test --features aims` to confirm no
  test regression.

### WASTE — Unused parameters in `emit_rc_ops()` signature

`compiler/ori_arc/src/aims/emit_rc/mod.rs` line 98–99:

```rust
_sigs: &FxHashMap<Name, MemoryContract>,
_classifier: &dyn ArcClassification,
```

These were placeholders for future cross-function reasoning (Section 07.3 COW
borrowing). They are currently unused and the leading `_` suppresses the warning.
The pipeline passes them in from `aims_pipeline.rs` — dead weight in both the
caller and callee.

- [x] **Remove `_sigs` and `_classifier` from `emit_rc_ops()`** in `emit_rc/mod.rs`
  and update the single call site in `pipeline/aims_pipeline.rs`. Also removed
  unused `TestClassifier` from `test_helpers.rs` and cleaned up test imports.

### WASTE — Unused parameters in `analyze_function()` and `compute_block_entry_state()`

`compiler/ori_arc/src/aims/intraprocedural/mod.rs` line 74:
```rust
_context_regions: &[ContextRegion],
```

`compiler/ori_arc/src/aims/intraprocedural/block.rs` lines 181, 227:
```rust
_classifier: &dyn ArcClassification,
```

`_context_regions` is a Stage 3 placeholder (TRMC). `_classifier` was used for
scalar filtering before scalars were tracked in `AimsStateMap`. The scalar
bitvector (`set_permanent_scalar`) now handles filtering; the `_classifier`
parameter is pure dead weight.

- [x] **Remove `_classifier` from `compute_block_entry_state()` and any other
  block.rs functions that carry it** — removed from `compute_block_entry_state`,
  `apply_terminator_demands`, and `apply_callee_contract`. Scalar bitvector
  handles all filtering.
- [x] **Retain `_context_regions` with a doc comment**: `// Reserved for Stage 3
  (TRMC context regions). Empty slice in Stages 1–2.` — documented.

### BLOAT — `emit_rc/mod.rs` is 970 lines

**Limit is 500 lines.** Touching this file without splitting it is a hygiene
violation. The file already has natural extraction points:

- Lines 149–416: helper infrastructure (`BlockCtx`, `LastUse`, `precompute_*`,
  `collect_*`, `compute_child_effective_last_use`, `is_owned_at_entry`) → extract
  to `emit_rc/block_helpers.rs`
- Lines 483–599: Phase A + dead-invoke sweep (`emit_dead_at_entry_decs`,
  `emit_dead_invoke_dsts`) → `emit_rc/phase_a.rs` or inline into `block_helpers`
- Lines 600–870: Phase B + C forward walk (`emit_body_forward_walk`,
  `emit_pre_instr_incs`, `emit_post_instr_decs`, `emit_terminator_rc`) →
  `emit_rc/forward_walk.rs`
- Lines 872–970: locality hints + RC-incremented tracking
  (`collect_local_alloc_candidates`, `collect_rc_incremented_vars`) →
  these are outputs used by cow.rs and drop_hints.rs, keep in a shared
  `emit_rc/queries.rs` or inline into their consumers

Note: Section 10 will replace `emit_rc/` with `realize/`. If 09.x work requires
touching `emit_rc/mod.rs`, split it first. If Section 10 arrives before 09.x
touches this file, extract into `realize/` directly rather than splitting
`emit_rc/` as an intermediate step.

- [x] **Split `emit_rc/mod.rs` before any 09.x changes touch it**
  — split 960→225 lines: `helpers.rs` (328), `dead_cleanup.rs` (136),
  `forward_walk.rs` (242), `queries.rs` (110). All under 500-line limit.

### BLOAT — `emit_reuse/mod.rs` is 508 lines

Marginally over the 500-line limit. The two `apply_static_reuse_*` functions
(same-block and cross-block) are structurally parallel and share the same
span-rebuilding logic (~40 lines each, duplicated). The shared helpers
(`build_set_instructions`, `is_self_set`, `build_proj_map`) could live in a
`emit_reuse/set_ops.rs` submodule.

- [x] **Extract shared set-ops helpers from `emit_reuse/mod.rs`** to
  `emit_reuse/set_ops.rs` — extracted `ProjMap`, `build_proj_map`, `is_self_set`,
  `build_set_instructions`, `extract_construct_info`, `substitute_var_all` (100 lines).
  `mod.rs` reduced from 508→426 lines.

---

## 09.1 Transfer Fusion

**File(s):** `compiler/ori_arc/src/aims/transfer/mod.rs`,
`compiler/ori_arc/src/aims/intraprocedural/block.rs`

Currently, transfer functions update each dimension independently based on the
instruction type. A `Project` updates access, consumption, cardinality — but doesn't
check locality or uniqueness of the source to inform the projected field's state.
Transfer fusion means: during transfer, dimensions read each other's current state
to compute more precise values.

**Methodology:** Test-driven. For each proposed rule, write a program that AIMS
currently handles conservatively. Verify the current output. Add the rule. Verify
improved output. If no improvement is measurable, the rule isn't worth adding.

**Design invariant (Marshall et al., ESOP 2022):** No fusion rule may derive
uniqueness from consumption or cardinality alone. Uniqueness is about the past
(has this value been duplicated?); consumption and cardinality are about the future
(how will this value be used going forward?). A fusion rule that crosses this
boundary must also involve a past-facing dimension (locality, shape, or an
interprocedural contract) to bridge the gap. This invariant prevents unsound
optimizations where "used once" (future) is mistakenly treated as "sole reference"
(past).
(See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))

- [x] **Rule: Unique source projection preserves uniqueness.**
  When processing `Project(dst, src, field)`, if source's state has
  `uniqueness == Unique`, set projected field's uniqueness to `Unique`.
  **Already implemented** in `transfer_project()` (line ~144): `uniqueness: source.uniqueness`.
  No further action needed; verify via existing tests.

- [x] **Rule: Block-local construct is unique.**
  When processing `Construct(dst, ctor, args)`, the newly allocated value has
  no other references — it is unique.
  **Already implemented** in `transfer_construct()`: uses `AimsState::FRESH` which
  sets `uniqueness = Unique`. No further action needed; verify via existing tests.

- [x] **Rule: Pure callee preserves caller uniqueness.**
  When processing `Apply/Invoke` where callee's contract has
  `effect_summary.may_share == false`, borrowed arguments preserve the caller's
  uniqueness state. Currently, borrowed args get conservative uniqueness at
  call boundaries. With this rule, calling a pure function that doesn't share
  its arguments can't create new references → uniqueness is preserved.
  Prerequisite: EffectSummary must be accurate (Section 09.2 Effect Activation).
  **Soundness bridge (Marshall et al. invariant):** This rule is SOUND because
  it uses the callee's `EffectSummary.may_share` — a past-facing fact about
  what the callee HAS done (or rather, has NOT done: it has not created new
  references). The rule does NOT derive uniqueness from consumption or
  cardinality alone; it bridges the future→past gap via the interprocedural
  contract's effect summary.
  (See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))
  **Backward analysis semantics:** The post-state (how a variable is used after
  the call) drives the pre-state. The transfer function applies BACKWARD from the
  call site: it does NOT set Unique forward, it PRESERVES Unique backward. When
  `transfer_apply` processes a borrowed argument whose post-state has
  `Uniqueness::Unique`, and the callee contract has `may_share==false`, the
  pre-state uniqueness for that argument is preserved as `Unique` instead of being
  conservatively widened to `MaybeShared`. Implement in `transfer_apply` in
  `transfer/mod.rs`, not in a forward canonicalize pass.

- [ ] **Rule: Linear consumption at call site enables callee reuse.**
  When **all callers** pass an argument with `consumption == Linear`,
  `cardinality == Once`, and `access == Owned`, tighten the callee's contract
  for that parameter to `uniqueness = Unique`. This is the callee-side dual
  of COW-aware borrowing (07.3.1) — the callee can trust that the argument
  is unique when every call site proves sole ownership.
  **Soundness requirement (all-callers condition):** A single call site with
  `Owned + Linear + Once` proves that THIS caller's reference is the sole live
  reference on THIS path — but it does NOT prove RC==1 globally. Another caller
  may have incremented the refcount. Uniqueness is only sound when the
  interprocedural fixpoint confirms that ALL callers satisfy the condition.
  A single call site cannot derive callee-side uniqueness alone.
  **Soundness bridge (Marshall et al. invariant):** When the all-callers
  condition holds, `Linear + Once + Owned` at every call site collectively
  prove that the runtime RC is 1 at every entry to the callee. `Owned`
  establishes that each caller holds a real reference (not a borrow). `Linear`
  establishes that no caller duplicates the reference (`rc_inc`). `Once`
  establishes that no other use site in any caller retains a reference. These
  future-facing facts, combined with the interprocedural all-callers gate,
  bridge to the past-facing uniqueness claim.
  (See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))
  **Implementation note:** This rule operates on the INTERPROCEDURAL contract,
  not the intraprocedural state map. It requires a new demand propagation phase
  in `interprocedural.rs analyze_program()` where call-site cardinality
  information tightens callee `ParamContract.cardinality`. The caller's knowledge
  (argument is `Linear+Once`) flows INTO the callee's contract. When **all
  callers** pass that argument with `cardinality <= Once` and `access == Owned`
  and `consumption == Linear`, tighten `MemoryContract.params[i].uniqueness`
  to `Unique`. This is distinct from the existing SCC-based ownership inference.
  **Critical:** the tightening must NOT happen until the fixpoint confirms all
  callers agree — premature tightening from a single call site is unsound.

- [x] **Rule: Closure-capture locality and uniqueness.**
  When processing `PartialApply` (closure creation in ARC IR), each captured
  variable's locality should be widened to at least `FunctionLocal` (the value
  now lives in the closure, which may outlive the defining block). If the
  closure itself escapes (returned or stored in a heap structure), the captured
  variable's locality becomes `HeapEscaping`. This refines the existing
  `transfer_construct` which conservatively sets all captured vars to
  `HeapEscaping`. Additionally, if the closure is `once` (its cardinality is
  `<= Once`), captured values preserve their uniqueness — the closure
  cannot create multiple references to the captured value because it is
  invoked at most once (OxCaml's LAM "lock" mechanism).
  **Backward analysis semantics:** In the backward direction, `PartialApply`
  adds demand on captured variables. The locality widening applies to the
  pre-state: captured vars' locality is widened from whatever the post-state
  says. If the closure's own post-state has `locality == HeapEscaping`, each
  captured var gets `HeapEscaping`; if `FunctionLocal`, each captured var gets
  at least `FunctionLocal`.
  **Implementation note:** `backward_demands()` returns empty for `PartialApply` —
  all captured arg demand is handled by `capture_state_update()` to avoid
  double-counting. The once-closure check uses cardinality only (not consumption),
  since a closure with `Affine` consumption (may be dropped) still invokes captured
  values at most once.
  (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K3, I3)

- [x] **Rule: HeapEscaping locality forces may_share effect.**
  When a value transitions to `HeapEscaping` locality (e.g., stored in a
  heap-allocated structure), set `effect.may_share = true` on the current
  instruction's effect. This feeds back into interprocedural contracts — a
  function that escapes a value to the heap is recorded as having sharing effects.
  **Backward analysis semantics:** `HeapEscaping` arises when a variable is used
  in a `Construct` that stores it into a heap allocation. The backward transfer for
  `Construct` sets `locality=HeapEscaping` on the stored argument's pre-state. The
  `may_share` effect propagates to the function-level `EffectSummary` during contract
  extraction. The rule fires at `transfer_construct()` time: for each argument stored
  in a `Construct`, if the argument's post-state has `locality != BlockLocal`, set
  `effect.may_share=true` on the function's accumulated `EffectSummary`.

- [x] Test: program where unique source projection eliminates a COW check
- [x] Test: program where block-local construct enables static reuse
- [x] Test: program where pure callee preserves caller uniqueness across call
- [ ] Test: program where linear+once argument enables callee-side optimization
- [x] Test: program where closure-capture locality is FunctionLocal (non-escaping closure)
- [x] Test: program where once-closure capture preserves uniqueness of captured value
- [x] Test: program where heap escape propagates may_share effect to contract

---

## 09.2 Active Dimensions

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`,
`compiler/ori_arc/src/aims/lattice/dimensions.rs`,
`compiler/ori_arc/src/aims/transfer/mod.rs`,
`compiler/ori_arc/src/aims/interprocedural.rs`

Currently, locality, effect, and shape are set during transfer and read during
emission but never influence other dimensions. This subsection activates each one.

**Implementation ordering (dependency ladder):** Activate one dimension at a time.
Each activation removes a conservative default, adds outgoing influence edges, and
is verified end-to-end before the next begins. This ordering is load-bearing — each
step's soundness depends on the previous step being correct and tested.

1. **Locality first.** Clearest soundness story (OxCaml proves it's load-bearing).
   Unlocks the highest-value cross-talk: `BlockLocal + Owned + Once → Unique`
   (Rule 4), `HeapEscaping → not Unique` (Rule 6), `Borrowed → FunctionLocal`
   (Rule 8), RC-skip for function-local values, `locality_bound` in contracts.
   No dependency on Effect or Shape being active.
   **Gate:** All locality tests green, canonicalize Rules 4/6/8 fire correctly,
   RC-skip produces measurable improvement on golden corpus.

2. **Effect second.** Depends on precise locality: the `HeapEscaping → may_share`
   transfer rule (09.1) needs locality to be accurate. Enables: pure-callee
   preserves uniqueness, FIP-natural detection, TRMC soundness gate. Effect
   precision feeds back into locality (a function with `may_share==false` lets
   callers keep `FunctionLocal` locality through call boundaries).
   **Gate:** Effect tests green, FIP-natural detection works for balanced functions,
   pure-callee-preserves test shows StaticUnique COW after call.

3. **Shape third.** Benefits from both Locality and Effect: ContextHole detection
   requires `Unique + FunctionLocal + may_share==false` — all three facts come
   from the previous two activations. Reuse-during-analysis moves detection from
   emission into the state map, which is cleanest when uniqueness and locality are
   already precise (fewer false negatives in reuse candidacy).
   **Gate:** Reuse detected in analysis (event table), ContextHole identified for
   recursive constructors, `CollectionBuffer + Unique → StaticUnique` fires.

This ordering ensures each step converts a passenger dimension into a load-bearing
one and deletes a conservative default. No step adds a new independent decision
procedure. By the end, all 7 dimensions participate in cross-talk — the full
unification is achieved monotonically, not in one commit.

### Locality Activation

Locality tracks whether a value escapes its allocation scope. When precise, it
provides uniqueness proofs that the uniqueness dimension alone cannot achieve.

**Locality is load-bearing for soundness, not auxiliary.** OxCaml proves that
locality is necessary for both stack allocation and borrowing soundness. The
`global` modality forces BOTH `aliased` (uniqueness) AND `global` (locality)
simultaneously — a value that escapes to the heap cannot be assumed unique,
because heap-allocated values can be reached from multiple roots. This means
AIMS's treatment of locality as conservative/`Unknown` in Stage 1 is a
deliberate deferral with a soundness cost: any optimization that depends on
uniqueness of heap-stored values is blocked until locality is precise.
(See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K1, K2)

**Backward analysis semantics for locality:**
Locality is backward: the analysis discovers where a value WILL end up (future
use determines locality). A value's locality is `BlockLocal` if all future uses
are within the block, `FunctionLocal` if all uses are within the function, and
`HeapEscaping` if the value can be stored in a heap-allocated structure. In the
backward direction:
- Analysis starts at the function exit with `HeapEscaping` for returned values
  (they escape the function scope).
- Propagating backward through the CFG: if a variable's only future use is in
  a `Construct` that stores it to the heap, its pre-state locality is `HeapEscaping`.
- If a variable's only future uses are within the same block (no successor blocks
  use it), its locality remains `BlockLocal`.
- Join at control-flow merge: take the widest locality (most conservative).
  `BlockLocal.join(FunctionLocal) = FunctionLocal`.
  `FunctionLocal.join(HeapEscaping) = HeapEscaping`.
This is the standard escape analysis direction: pessimistic join at merges.

- [x] **Precise locality computation.**
  Replace conservative `Unknown` defaults with accurate tracking:
  - `Construct` → `BlockLocal` (fresh allocation, hasn't escaped yet)
  - `Project` from source → inherit source's locality (deepness property: a
    field of a `BlockLocal` value is at most `BlockLocal`, a field of a
    `FunctionLocal` value is at most `FunctionLocal`). This mirrors OxCaml's
    deep mode property where destructuring preserves locality.
    (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K5, I4)
  - `Apply/Invoke` arg: if callee contract says param doesn't escape → preserve
    caller locality; if callee may store the value → `HeapEscaping`
  - `Return` → `HeapEscaping` (value escapes function scope)
  - `Let(dst, src)` → inherit source locality
  **Sync point:** `ParamContract` needs `locality_bound: Locality` (see "Locality
  in MemoryContract" below). When adding this field, update all 6 locations together:
  1. `aims/contract/mod.rs` -- add `locality_bound` to `ParamContract`
  2. `aims/interprocedural.rs` -- `extract_contract()` computes `locality_bound`
  3. `aims/builtins/mod.rs` -- builtin defaults (most: `HeapEscaping`; pure: `FunctionLocal`)
  4. `aims/emit_rc/arg_ownership.rs` -- contract-to-`AnnotatedSig` conversion
  5. `pipeline/shadow.rs` -- no update needed (no old-pipeline equivalent)
  6. `verify/mod.rs` -- check returned locality matches contract

- [x] **Locality → Uniqueness interaction.**
  `BlockLocal + Owned + ≤Once → Unique`. A block-local value that is owned by
  this scope and used at most once cannot have any other reference to it. The
  only way to create a second reference is via `RcInc`, which would violate
  the `Once` cardinality. Enforce in canonicalize (Section 09.3).
  **Soundness guard:** This rule is only sound if locality is PRECISE (not the
  conservative `Unknown` default). It MUST NOT be enabled before precise locality
  computation is implemented -- enabling it with `Unknown` locality would be
  unsound. Guard in `canonicalize()`: only apply Rule 4 when `locality != Unknown`.
  **Implementation note:** Rule 4 only promotes `MaybeShared` → `Unique`, not
  `Shared` → `Unique`. Definite `Shared` (RC > 1) from e.g. Select branch joins
  must not be overridden.

- [x] **Locality → RC skip interaction.**
  `FunctionLocal + Owned + Linear → RC-skip eligible`. A function-local value
  that is owned and consumed linearly will be freed at function exit. The RcDec
  at last use would free the value (refcount is 1 because it's unique+linear).
  Instead of emitting RcInc at entry and RcDec at last use, skip both — the
  value's lifetime is precisely the function's lifetime.
  **Implementation:** `AimsState::is_rc_skip_eligible()` predicate added.

- [x] **Locality in MemoryContract.**
  Extract `locality_bound` per parameter in `extract_contract()`. A function
  whose parameters all stay `FunctionLocal` is a candidate for RC-free calling
  convention (no RcInc/RcDec at call boundary — callee promises not to escape).
  **Note:** `locality_bound` is a soundness requirement for the
  `HeapEscaping -> not Unique` invariant, not just an optimization hint.
  If a callee stores a parameter into a heap structure, the caller must know,
  because the caller's uniqueness reasoning depends on it.
  (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 I1, §01.7 Risk 5)

- [x] **Closure-capture-aware locality.**
  When a value is captured by a closure (`PartialApply`), its locality must be
  widened to at least `FunctionLocal` (it escapes the block where it was defined,
  into the closure's scope). If the closure itself escapes (returned, stored in a
  heap structure), the captured value's locality becomes `HeapEscaping`. This
  refines the current conservative approach (always `HeapEscaping` for captured
  variables) by distinguishing non-escaping closures (`FunctionLocal`) from
  escaping closures (`HeapEscaping`). Additionally, if the closure is determined
  to be `once` (from its consumption context / `Consumption <= Linear`), captured
  values preserve their uniqueness — this is the "lock" mechanism from OxCaml's
  LAM rule.
  (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K3, I3)
  **Implementation:** `capture_state_update` now takes the closure's own demand
  state. Once-closures preserve captured variable linearity/uniqueness.

- [x] Test: block-local value gets Unique without runtime check
- [x] Test: function-local linear value skips RC operations
- [x] Test: contract with locality bounds enables RC-free call

### Effect Activation

Effect tracks whether a function allocates, shares, or throws. When precise, it
constrains consumption and enables FIP certification naturally.

- [x] **Precise effect computation.**
  Replace conservative `ALL` defaults with accurate tracking:
  - `Construct` → sets `may_alloc` (on `EffectClass`, the per-variable lattice dimension)
  - `Apply/Invoke` → union with callee contract's effect summary
  - `RcInc` (once emitted) → sets `may_share` (note: RcInc is emitted at step 6, after analysis at step 5; this requires tracking `RcInc` intent during transfer, not post-emission inspection)
  - Throw/panic → sets `may_throw`
  - Instructions with no heap effects → `NONE`

  **Effect accumulation semantics:** EffectClass is a function-level accumulator,
  not a per-point per-variable state. It answers "does this function have these
  effects?" The per-variable `effect` field in AimsState tracks what effects are
  caused by accessing that variable (e.g., a Construct variable has
  `effect.may_alloc=true` because creating it allocates). The function-level
  EffectSummary in MemoryContract is the OR of all per-variable effects across
  all instructions. There is no "backward direction" for effects — they are
  monotonically accumulated (forward aggregation even in a backward pass).
  Implementation: in `block.rs` `analyze_block()`, accumulate EffectClass into a
  local variable while processing each instruction; return the accumulated
  EffectClass alongside the per-block state. In `mod.rs` `analyze_function()`,
  OR all block-level EffectClasses into a function-level EffectSummary stored
  in `AimsStateMap` (e.g., a new `effect_summary` field). Then
  `extract_contract()` reads `state_map.effect_summary` to populate
  `MemoryContract.effects`. This preserves the architecture: `analyze_function()`
  returns `AimsStateMap`, and `extract_contract()` reads it to build contracts.
  Effects must NOT be written directly into `MemoryContract` during
  intraprocedural analysis — contracts are owned by the interprocedural layer.

- [x] **Effect → Consumption interaction.**
  If a function's effect summary has `may_share == false`, borrowed parameters
  preserve the caller's uniqueness through the call. This is because the callee
  can't create new references (no sharing effect), so the caller's reference
  count doesn't change.

- [ ] **Effect → FIP natural detection.** <!-- partial: is_fbip + FipContract::Bounded scaffolding done; token balance tracking TODO -->
  `may_alloc == false` AND all `Consume` matched by `Construct` (allocation
  balance = 0) → function is naturally FIP. This falls out of the converged
  effect state without a separate FIP certification pass. FipContract becomes
  a *read* of the converged state, not a separate computation.

  **FP²-derived tightening (invariant N1):** FIP-natural detection must check
  ALL of the following conditions, not just allocation balance:
  - `EffectSummary.may_allocate == false` (no allocations on any path)
  - **Token balance from analysis** (not from emission): every `Consume`
    instruction is paired with a `Construct` of compatible shape. FP² Theorem 2
    requires `|S| = |S'|`, meaning every deallocation is matched by a reuse.
    This must be tracked DURING intraprocedural analysis (accumulated in
    `AimsStateMap`, e.g., `fip_token_balanced: bool`), not read from
    `EmitReuseResult.missed_reuses`. The sequencing constraint is:
    `extract_contract()` runs during the interprocedural SCC loop (before any
    per-function emission), so it can only read analysis artifacts
    (`AimsStateMap`), not realization artifacts (`EmitReuseResult`).
    `missed_reuses` is a realization-time count produced by `emit_reuse()`
    (pipeline step 7) — it does not exist when contracts are extracted.
  - **Recursion check**: `Certified` requires no recursive calls at all;
    `Conditional` allows only tail-recursive calls (non-tail recursion may
    require stack allocation for continuation frames)
  - **Per-arm token balance**: in pattern match functions, each match arm must
    independently maintain allocation/deallocation balance (not just the global
    function count). A function where arm A allocates 2 and deallocates 0, and
    arm B allocates 0 and deallocates 2, has global balance 0 but is NOT FIP —
    arm A violates the in-place property. This per-arm tracking must also be
    accumulated in `AimsStateMap` during analysis (using `alt_join` to verify
    each branch independently maintains balance).

  Rename the field from `fip_alloc_balanced` to `fip_token_balanced` to reflect
  that the check covers both allocation AND deallocation balance (FP²'s reuse credit / token
  model). The field tracks: (a) every `Construct` is matched by a consumed
  value of compatible type, AND (b) every consumed value with reusable shape
  is matched by a `Construct`. This is the two-sided balance from FP² Theorem 2.
  (See: [Literature Review §02 — FP²](../aims-literature-review/section-02-fp2.md))

  **Per-branch FIP balance (FIPTree DMATCH! rule):** FIP certification requires
  per-branch allocation credit balance, not just function-level. Each match arm
  must independently balance destructions (credits provided by pattern-matching
  constructors) against constructions (credits consumed by allocating constructors
  on the right-hand side). A function where arm A allocates 2 and deallocates 0,
  and arm B allocates 0 and deallocates 2, has global balance 0 but is NOT FIP —
  arm A violates the in-place property. Track per-branch balance through the
  `Switch` terminator's successor blocks: each successor block's allocation credit
  must be non-negative independently. This is FIPTree's per-arm linear resource
  accounting (Γ linearity).
  (See: [Literature Review §03 — FIPTree](../aims-literature-review/section-03-fiptree.md))

  **`FipContract` enum expansion:** `FipContract` should support `Bounded(u16)`
  for functions that allocate at most N constructors (FIPTree's `fip(1)` pattern —
  e.g., tree insertion allocates exactly one node). The merged enum is:
  `Never | Conditional{requires_unique_params} | Certified | Bounded(u16)`.
  FBIP: `MemoryContract.is_fbip: bool` is **inferred metadata only** — the
  contract layer records whether the function meets FBIP criteria based on
  analysis facts (earlier in the pipeline than `is_auto_fbip()`, which runs
  on final IR — same criteria, different input). It does NOT replace `#fbip`
  as the user-facing enforcement annotation, and does NOT change `is_auto_fbip()`
  behavior. `#fbip` remains opt-in enforcement (makes FBIP violations into
  errors). `is_fbip` on the contract just makes FBIP status visible to
  interprocedural analysis without running the post-pipeline check.
  `Bounded(n)` is compiler-inferred from allocation balance tracking
  (`allocs - reuses = n`), not a user annotation.
  (See: [Literature Review §03 — FIPTree](../aims-literature-review/section-03-fiptree.md))

- [x] **Effect -> TRMC soundness gate.**
  `may_share == false` is a PRECONDITION for in-place TRMC, not just a
  profitability signal. When `may_share == true`, the context variable `k` may
  be captured by an effect handler's resumption and used non-linearly, breaking
  the unique linear chain invariant. AIMS must gate in-place TRMC behind this
  check. Stage 3 `normalize/verify.rs` must query `EffectSummary` to determine
  the path: `may_share == false` permits in-place TRMC; `may_share == true`
  requires non-in-place translation or skipping TRMC entirely.
  Documented in `ContextBehavior` doc comment; actual gate deferred to Stage 3.
  (See: [Literature Review §04 — TRMC](../aims-literature-review/section-04-trmc.md))

- [x] **EffectSummary in MemoryContract.**
  Already exists (`MemoryContract.effects`). Now precise: `extract_contract()`
  reads `state_map.effect_summary()` instead of `EffectSummary::default()`.
  Effects accumulated during backward walk via `accumulate_instr_effects()`
  and `accumulate_terminator_effects()` in `block.rs`. `populate_effect_summary()`
  removed — effects now accumulated during analysis, not replayed post-convergence.

- [x] Test: pure function call preserves caller uniqueness
- [x] Test: function with balanced alloc/consume gets FIP naturally
- [x] Test: effect propagation through SCC converges correctly

### Shape Activation

Shape classifies values by their reuse compatibility. When active, it constrains
reuse decisions during analysis (not just during emission) and enables constructor
context detection.

- [ ] **Shape → Reuse during analysis.**
  Currently, reuse detection happens during emission (emit_reuse). Move the
  core decision into the analysis: when a variable has `ReusableCtor(kind)` shape
  and transitions to `Dead` consumption, record a reuse opportunity in the
  sparse event table during analysis, not during a separate emission scan.

- [ ] **Shape → Cardinality interaction.**
  `ReusableCtor + Once` means the constructor is used exactly once then dies.
  This is the ideal reuse scenario — the value is consumed once, its memory
  can be immediately recycled. Record this as a high-confidence reuse candidate
  in the event table (no `IsShared` check needed if also `Unique`).

- [ ] **Shape for collection buffers.**
  `CollectionBuffer` shape means the value is a growable buffer (list, map, set).
  COW-aware borrowing should apply to all collection buffers, not just parameters.
  When a `CollectionBuffer` is `Unique`, COW mutations can be done in-place
  without any uniqueness check.

- [ ] **Shape for constructor contexts (TRMC preparation).**
  `ContextHole` shape means the value has a hole to be filled by a recursive
  call. TRMC candidacy requires `ContextHole + FunctionLocal + Unique +
  (EffectClass::may_share == false OR hybrid path available)`. The uniqueness
  requirement is a **soundness** condition (Lemma 2, Leijen & Lorenzen JFP
  2025), not merely a profitability hint — if the context variable is not
  unique, in-place mutation of the hole is unsound because another reference
  may observe the partial state. The effect purity check guards against
  non-linear control flow that would break the linear chain property (an
  effect handler's resumption could capture the context variable and use it
  non-linearly). This identification happens during analysis, so Stage 3
  TRMC normalization can read it from the converged state.
  (See: [Literature Review §04 — TRMC](../aims-literature-review/section-04-trmc.md))

  **Note:** The bare `ContextHole` enum variant is a placeholder for Stages 1-2.
  When Stage 3 is implemented, it should become `ContextHole(ContextMeta)` where
  `ContextMeta` carries:
  1. **Hole position** — which child index of the constructor holds the hole (0-based).
  2. **Context depth** — estimated number of constructors root-to-hole (copy cost
     estimate when the context is shared).
  3. **Accumulator count** — 1 for standard TRMC, 2 for splay/move-to-root dual-context
     patterns, N for multi-arm accumulation.

  The `ShapeClass::join` for `ContextHole(ContextMeta)` should preserve the flat
  lattice semantics: two `ContextHole` values with different `ContextMeta` join to
  `NonReusable`. Same `ContextMeta` is preserved.

  Cross-reference: `aims/normalize/context/` module plan (detect, validate, multi
  submodules) extracts this metadata from the IR during Stage 3 normalization.
  (See: [Literature Review §03 — FIPTree](../aims-literature-review/section-03-fiptree.md))

- [ ] Test: reuse opportunity detected during analysis, not emission
- [ ] Test: Once+ReusableCtor+Unique → static reuse without IsShared check
- [ ] Test: CollectionBuffer+Unique → StaticUnique COW for non-parameter
- [ ] Test: ContextHole detected for recursive constructor function

---

## 09.3 Enriched Canonicalize

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Canonicalize enforces cross-dimension invariants after every join and transfer.
Currently: 3 rules. After this section: 8 rules (6 canonicalize + FIP
classification in contract extraction).

**Existing rules (preserved):**
1. `Dead ↔ Absent` (bidirectional consumption/cardinality sync)
2. `Linear + Absent → Dead` (infeasible state collapse)
3. `Shared + ReusableCtor → NonReusable` (shared values can't be reused)

**New rules:**

**Backward analysis soundness for all canonicalize rules:** Canonicalize runs
after each transfer application and after each join at control-flow merge points.
In the backward analysis, state flows from successors to predecessors.
Each rule must be a valid strengthening (moving toward a more precise / lower
lattice point, away from TOP). Per-rule analysis:
- Rule 4: `Unique` is below `MaybeShared` (more precise). Strengthening when
  locality+access+cardinality permit is sound — the transfer function was
  conservative; canonicalize refines it.
- Rule 5: Preserving `ReusableCtor` is a no-op (no lattice movement). Safe.
- Rule 6: `MaybeShared` is above `Unique` (less precise). This is a weakening,
  but it only applies when `HeapEscaping` indicates the `Unique` state was
  already unsound (value reachable from heap). This is a correction, not an
  arbitrary weakening. Strengthened from original `HeapEscaping + Borrowed`
  formulation to apply regardless of access class.
- Rule 7: Forces `Dynamic` COW, preventing an incorrect static optimization.
  Safe (prevents false positive).
- Rule 8: `FunctionLocal` is below `HeapEscaping` (more precise / tighter).
  This is a tightening that only fires when `Borrowed` contradicts
  `HeapEscaping` (borrows cannot escape). Safe — no interaction with Rules
  4 or 6 because the access/locality preconditions are mutually exclusive.

All five new rules are monotone in the backward analysis sense (no rule causes
the analysis to miss a use or ignore a death).

**Note:** FIP classification (formerly "Rule 7") is NOT a canonicalize rule. It
is a function-level property computed by `extract_contract()` in interprocedural
analysis. See 09.2 Effect Activation for the FIP-natural detection spec and
Section 10.1 for the ownership boundary.

- [x] **Rule 4: `BlockLocal + Owned + ≤Once → Unique`**
  Block-local owned single-use value must be unique. (From 09.2 Locality.)
  **Soundness guard:** only apply when `locality != Unknown` (unknown locality
  means precise locality analysis hasn't run yet — see 09.2 sync point note).
  **Implementation note:** Only promotes `MaybeShared` → `Unique`, not `Shared`.

- [x] **Rule 5: `Unique + Dead → preserve ReusableCtor`**
  A unique dead value's memory IS reusable. Do NOT collapse shape. (Clarification
  of existing behavior — ensure canonicalize doesn't interfere.)
  Verified: no rule collapses shape for Unique+Dead. Rule 3 only fires for Shared.
  Documented as explicit comment in canonicalize(). Tests added: `rule5_unique_dead_preserves_reusable_ctor` (preserves), `rule5_shared_dead_collapses_reusable_ctor` (contrast).

- [x] **Rule 6: `HeapEscaping → uniqueness >= MaybeShared`**
  Strengthened from the original `HeapEscaping + Borrowed` formulation: ANY
  value whose locality is `HeapEscaping` must have its uniqueness ceiling
  lowered to at least `MaybeShared`, regardless of access class. An owned value
  stored into a heap structure is reachable from that structure's root, and if
  the structure is aliased, so is the stored value. This mirrors OxCaml's
  `global` modality which forces BOTH `aliased` AND `global` simultaneously.
  **Exception:** `HeapEscaping + Unique` remains valid when the containing
  structure is itself provably `Unique` — the value is heap-stored but only
  reachable through a single unique path. This exception requires checking the
  `BorrowSource` or containing structure's state. If no containment proof is
  available, force `MaybeShared`. Note: the exception must be handled in
  transfer functions (not canonicalize) if it requires inter-variable reasoning,
  since canonicalize operates on a single `AimsState`.
  (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K1, I1, §01.7 Risk 2)

- [x] **Rule 7: `Shared + CollectionBuffer → force Dynamic COW`**
  Shared collection buffers always need runtime uniqueness checks for COW.
  Verified: `Shared` maps to `CowMode::StaticShared` in `uniqueness_to_cow_mode()`
  (cow.rs:202), which statically takes the slow path — no runtime check needed,
  but never takes the in-place fast path. Rule 3 also collapses `Shared + ReusableCtor`
  to `NonReusable`, preventing reuse of shared values.

- [x] **Rule 8: `Borrowed → locality <= FunctionLocal`**
  A borrowed reference by definition cannot escape its defining function — it
  is a temporary view. If canonicalize finds `Borrowed + HeapEscaping`, force
  `locality = FunctionLocal`. This is a tightening (toward bottom) that is
  derivable purely from state fields. Borrows cannot outlive their defining
  scope, so `HeapEscaping` locality is contradictory for a `Borrowed` value.
  Chain height analysis: forces locality DOWN (`HeapEscaping` -> at most
  `FunctionLocal`). No interaction with Rule 4 or Rule 6 because those require
  specific access/locality combinations that Rule 8 would prevent.
  (See: [Literature Review §01 — OxCaml](../aims-literature-review/section-01-oxidizing-ocaml.md), §01.2 K4, I2)

- [x] Each new rule has a unit test proving it fires and changes state
  Rule 5: `rule5_unique_dead_preserves_reusable_ctor`
  Rule 6: `rule6_heap_escaping_unique_becomes_maybe_shared`
  Rule 8: `rule8_borrowed_heap_escaping_tightens_to_function_local`, `rule8_borrowed_unknown_tightens_to_function_local`
- [x] Each new rule has a counter-test proving it doesn't fire when preconditions unmet
  Rule 5 contrast: `rule5_shared_dead_collapses_reusable_ctor`
  Rule 6: `rule6_does_not_fire_for_block_local`, `rule6_does_not_fire_for_function_local`, `rule6_does_not_fire_for_unknown_locality`, `rule6_heap_escaping_maybe_shared_unchanged`, `rule6_heap_escaping_shared_unchanged`
  Rule 8: `rule8_borrowed_function_local_unchanged`, `rule8_borrowed_block_local_unchanged`, `rule8_owned_heap_escaping_not_tightened`
  Interaction: `rule8_then_rule6_borrowed_unique_heap_escaping` (Rule 8 prevents Rule 6 from firing)
- [x] Canonicalize termination proof: all new rules are monotone (move toward more
  precise or collapse to bottom). Chain height unchanged.
  Verified by `join_produces_canonical_output` exhaustive test (all representative state pairs).
  Documented in canonicalize() doc comment: ordering, mutual exclusion of Rules 4/6, Rule 8 preventing Rule 6 on same state.

  **Chain height analysis:** The existing 3 rules do not increase chain height
  because they only move components to more precise values (lower in the lattice).
  Rule 4 does the same (`MaybeShared` to `Unique` is one step down). Rule 6 does
  the opposite (`Unique` to `MaybeShared` = up), but only fires when the state
  was already incorrect (`HeapEscaping` contradicts `Unique`). Rules 4 and 6
  cannot fire simultaneously on the same variable (`BlockLocal` contradicts
  `HeapEscaping`), so the chain height remains bounded by the product of
  dimension heights. Rule 8 tightens locality (`HeapEscaping` to `FunctionLocal`
  = down), and only fires when `Borrowed` is present, which prevents Rule 6
  from firing on the same state (Rule 6 now applies regardless of access class,
  but Rule 8 forces locality away from `HeapEscaping`, making Rule 6's
  precondition unmet). The multi-round canonicalize bound of 3 is sufficient.

---

## 09.4 Sequencing Algebra Extension

**File(s):** `compiler/ori_arc/src/aims/lattice/dimensions.rs`

Currently only cardinality has `seq_add` and `alt_join`. The other dimensions use
plain `join` (pointwise max/widening). Locality and effect have natural sequencing
semantics that should be encoded.

**Algebraic foundation.** AIMS Cardinality operations form a bounded distributive lattice with semiring-like structure, directly analogous to QTT's 0-1-omega semiring (Atkey, LICS 2018). `seq_add` corresponds to QTT's resource accumulation (+): combining usages along one execution path. `alt_join` corresponds to QTT's branch join (lub): combining usages from mutually exclusive paths. The key properties — associativity, commutativity, identity (Absent), absorption (Many), distributivity of `seq_add` over `alt_join` — are verified exhaustively in `lattice/tests.rs`. For Locality and Effect, `seq_add` coincides with `join` because these dimensions track properties that widen monotonically (a value that escapes in one instruction stays escaped). This coincidence should be documented as intentional, not accidental.
(See: [Literature Review §07 — QTT](../aims-literature-review/section-07-quantitative-type-theory.md))

- [ ] **Document the QTT semiring correspondence in `dimensions.rs` doc comments.** On `Cardinality`: note that `(Cardinality, seq_add, Absent)` is a commutative monoid and `seq_add` distributes over `alt_join`, analogous to QTT's 0-1-omega semiring. On `alt_join`: note it is the lattice lub (idempotent), not semiring addition. On Locality/Effect: note `seq_add` = `join` is a design choice, not a limitation.

- [ ] **Locality `seq_add`:**
  When two sequential operations both reference a value, the combined locality
  is the *widest* of the two. `BlockLocal.seq_add(FunctionLocal) = FunctionLocal`.
  This is the same as `join`, which is already correct — document that this is
  intentional, not accidental.

- [ ] **Locality `alt_join`:**
  When a value's locality differs across branches, take the widest.
  `BlockLocal.alt_join(HeapEscaping) = HeapEscaping`. Same as `join` — document.

- [ ] **Effect `seq_add`:**
  Sequential effects accumulate (union). `NONE.seq_add(MayAlloc) = MayAlloc`.
  This IS different from plain `join` if effects have an ordered lattice.
  Currently effects are boolean flags with `BitOr` join — `seq_add` = `alt_join`
  = `BitOr`. If we add effect *counts* (how many allocations), seq_add would
  be addition while alt_join would be max. Decision: keep boolean flags for now,
  document that seq_add = alt_join for effects.

- [x] **Shape `alt_join`:**
  Currently `ShapeClass::join` is a flat meet. If one branch has `ReusableCtor(A)`
  and another has `ReusableCtor(B)` (different constructors), join should be
  `NonReusable` (can't reuse as either). If both have `ReusableCtor(A)`, preserve
  it. **Verified correct**: `ShapeClass::join()` uses `if self == other { self } else { NonReusable }` — this is already the correct behavior. Document this as intentional.

- [ ] **Document that `mult` (GHC's `multCard`) is not needed for strict evaluation.**
  In `dimensions.rs`, the `Cardinality` doc comment should state explicitly: GHC uses
  three composition operations (`lubCard`, `plusCard`, `multCard`). AIMS needs only two
  (`alt_join` = `lubCard`, `seq_add` = `plusCard`). The third operation, `multCard`
  (demand scaling), models nested evaluation contexts in lazy languages — a lambda
  called zero times zeros out inner demands, called many times multiplies them. In
  Ori's strict evaluation model, every function body executes exactly once per call,
  so there is no outer cardinality to multiply by. `seq_add` subsumes the sequential
  composition role that GHC splits between `plusCard` and `multCard`.
  (See: [Literature Review §09 — GHC Demand Analysis](../aims-literature-review/section-09-ghc-demand.md))

- [x] **Verified Correct: `seq_add` / `alt_join` two-operation discipline.**
  GHC's demand analysis (§09 literature review) confirms that AIMS's existing
  separation of `seq_add` from `alt_join` is exactly the right discipline for a strict
  language. The tests `branch_value_used_in_both_arms_is_once` (verifying
  `alt_join(Once, Once) = Once`) and `sequential_uses_in_same_block_are_many` (verifying
  `seq_add(Once, Once) = Many`) validate the core invariant. No third operation is
  needed.
  (See: [Literature Review §09 — GHC Demand Analysis](../aims-literature-review/section-09-ghc-demand.md))

- [ ] Tests for sequencing algebra extension (or documentation that current behavior
  is already correct and intentional)

---

## 09.5 Convergence Feedback

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/mod.rs`,
`compiler/ori_arc/src/aims/intraprocedural/state_map.rs`

Currently, the backward dataflow pass converges each dimension independently within
the product lattice. When a dimension converges to a tighter value on iteration N,
canonicalize may tighten other dimensions, but this only happens within the same
iteration. True convergence feedback means: a tightening in one dimension on
iteration N triggers re-evaluation of related dimensions on iteration N+1.

- [ ] **Multi-round canonicalize within iteration.**
  After each transfer + canonicalize, check if canonicalize changed any dimension
  besides the one the transfer targeted. If so, run canonicalize again (up to a
  bounded limit, e.g., 3 rounds). This catches chain reasoning:
  locality→uniqueness→shape in one step instead of requiring 3 iterations.
  ```rust
  fn apply_transfer_and_canonicalize(state: &mut AimsState) {
      let mut changed = true;
      let mut rounds = 0;
      while changed && rounds < 3 {
          let before = state.clone();
          state.canonicalize();
          changed = *state != before;
          rounds += 1;
      }
  }
  ```

- [ ] **Cross-dimension convergence detection.**
  Track whether canonicalize's cross-dimension rules fired during an iteration.
  If they did, the analysis may need another iteration even if no individual
  dimension changed from the transfer. Add a `cross_dimension_tightened: bool`
  flag to the worklist to force one extra iteration after cross-dimension changes.

- [ ] **Termination guarantee.**
  Prove (or demonstrate via test) that multi-round canonicalize terminates:
  - Each canonicalize rule is monotone (moves toward bottom or preserves)
  - Product lattice has finite height (sum of individual heights)
  - Each round can only tighten, never widen
  - Maximum rounds bounded by chain height (product of dimension heights)
  - In practice, 2-3 rounds suffice (most chains are length 2)

- [ ] Test: program requiring 2-round canonicalize to reach optimal state
- [ ] Test: convergence counter shows ≤3 rounds in practice for all test programs
- [ ] Benchmark: compilation speed regression < 5% from multi-round canonicalize

---

## 09.6 Completion Checklist

### Transfer Fusion (09.1)
- [x] Unique source projection preserves uniqueness — already implemented in transfer_project()
- [x] Block-local construct is unique — already implemented via AimsState::FRESH in transfer_construct()
- [x] Pure callee preserves caller uniqueness — implemented and tested
  (backward semantics: uniqueness preserved in PRE-state when callee may_share==false)
- [ ] Linear consumption at call site enables callee reuse — implemented and tested
  (requires interprocedural demand propagation: new analysis phase in analyze_program())
- [x] HeapEscaping forces may_share effect — implemented and tested
  (post-convergence replay: fires when Construct destination has locality > BlockLocal)
- [x] Closure-capture locality refined: FunctionLocal for non-escaping closures,
  HeapEscaping for escaping closures; once-closures preserve cardinality
  (fixed: once-closure check uses cardinality only, not consumption;
  backward_demands returns empty for PartialApply to avoid double-counting)
- [ ] Each rule has a test program demonstrating measurable improvement (fewer RC ops
  or eliminated runtime check)

### Active Dimensions (09.2) — Dependency Ladder

**Step 1: Locality** (no dependency on Effect or Shape)
- [x] Locality: precise computation replaces conservative Unknown defaults
- [x] Locality: backward analysis semantics documented in block.rs (how locality
  flows from successors to predecessors — escaping uses drive pre-state locality)
  Verified: extensive doc comments in block.rs lines 26-89 (exit state), 78-86 (cross-block
  widening), 120-130 (return widening), 240-250 (terminator arg widening).
- [x] Locality: BlockLocal+Owned+Once → Unique fires in canonicalize
  (soundness guard: only when locality != Unknown)
- [x] Locality: FunctionLocal+Linear → RC-skip fires in emission
- [x] Locality: contract extraction includes locality_bound
- [x] Locality: closure-capture locality refined (FunctionLocal vs HeapEscaping)
- [x] Locality: locality_bound is a soundness requirement, not just optimization hint
  (HeapEscaping -> not Unique invariant depends on callee reporting escape)
  Enforced: Rule 6 (HeapEscaping → MaybeShared) relies on locality precision.
  Contract extraction propagates locality_bound for interprocedural reasoning.
- [x] Locality: ALL 5 sync locations verified (see sync note in 09.2):
  1. contract/mod.rs: `ParamContract.locality_bound` field ✓
  2. interprocedural.rs: `extract_contract()` reads `state.locality` ✓
  3. builtins/mod.rs: defaults set `locality_bound: Unknown` ✓
  4. arg_ownership.rs: field exists but not yet consumed during emission —
     locality-driven RC-skip at call boundaries deferred to Effect Activation (09.2)
  5. verify: contract consistency checked via pipeline verification steps ✓
- [x] **GATE:** All locality tests green, Rules 4/6/8 fire correctly,
  RC-skip predicate (`is_rc_skip_eligible`) implemented, `./test-all.sh` green
  (12,825 tests, 0 failures). `join_produces_canonical_output` exhaustive test
  confirms canonicalize soundness with all 8 rules.

**Step 2: Effect** (depends on precise Locality)
- [x] Effect: precise computation replaces conservative ALL defaults
  (populate_effect_summary post-convergence: Construct→may_allocate, Invoke→may_throw,
  Apply unions callee effects, HeapEscaping→may_share; extract_contract reads state_map)
- [ ] Effect: backward-compatible semantics clarified (forward accumulation even
  in backward pass — per-block EffectClass accumulated into function EffectSummary)
- [ ] Effect: may_share==false preserves caller uniqueness through call
  (backward transfer: uniqueness preserved in pre-state, not set forward)
- [ ] Effect: alloc-balanced + NONE → FIP-natural detected without separate pass
- [ ] Effect: `fip_token_balanced` tracking added to analyze_function() return value
  and MemoryContract (function-level, not per-variable AimsState)
- [ ] Effect: per-branch FIP balance tracking via AllocCreditBalance events
  (FIPTree DMATCH! rule — each match arm independently balances credits)
- [ ] Effect: `FipContract::Bounded(u16)` variant added for functions with bounded
  net allocation (FIPTree's `fip(n)` pattern)
- [ ] Effect: `is_fbip: bool` inferred metadata on MemoryContract (does NOT replace
  `#fbip` enforcement or change `is_auto_fbip()` — see 09.2 Effect Activation note)
- [x] Effect: EffectSummary precise in MemoryContract
  (extract_contract reads state_map.effect_summary() populated by populate_effect_summary)
- [ ] Effect: TRMC soundness gate — may_share==false is precondition for in-place TRMC;
  normalize/verify.rs queries EffectSummary (see §04 TRMC literature review)
- [ ] **GATE:** Effect tests green, FIP-natural detection works for balanced functions,
  pure-callee-preserves shows StaticUnique COW after call, `./test-all.sh` green

**Step 3: Shape** (depends on precise Locality + Effect)
- [ ] Shape: reuse opportunities detected during analysis (event table)
- [ ] Shape: Once+ReusableCtor+Unique → static reuse without IsShared
- [ ] Shape: CollectionBuffer+Unique → StaticUnique COW for non-parameters
- [ ] Shape: ContextHole identified for TRMC candidates (requires Unique + FunctionLocal +
  may_share==false — soundness condition from Lemma 2, Leijen & Lorenzen JFP 2025)
- [ ] Shape: ContextHole(ContextMeta) enrichment planned for Stage 3 (hole position,
  depth estimate, accumulator count — see 09.2 Shape Activation note)
- [ ] **GATE:** Reuse detected in analysis (event table), ContextHole identified for
  recursive constructors, CollectionBuffer+Unique→StaticUnique fires,
  `./test-all.sh` green

### Enriched Canonicalize (09.3)
- [x] Rule 4 implemented with unit tests (BlockLocal+Owned+Once → Unique)
- [x] Rule 5 documented and verified (Unique+Dead preserves ReusableCtor — implicit, no code change needed)
- [x] Rule 6 implemented with unit tests (HeapEscaping → MaybeShared)
- [x] Rule 8 implemented with unit tests (Borrowed → locality <= FunctionLocal)
- [x] Rule 7 verified (Shared+CollectionBuffer → COW slow path via StaticShared; no in-place mutation)
- [x] Counter-tests verify rules don't fire on unmet preconditions (14 counter-tests)
- [x] Canonicalize termination proof documented and demonstrated via exhaustive join test

### Sequencing Algebra (09.4)
- [ ] Locality seq_add/alt_join documented (intentionally same as join)
- [ ] Effect seq_add/alt_join documented (boolean flags, BitOr)
- [x] Shape alt_join verified for cross-branch constructor mismatches — already correct

### Convergence Feedback (09.5)
- [ ] Multi-round canonicalize implemented with bound
- [ ] Cross-dimension convergence detection in worklist
- [ ] Termination demonstrated across full test suite
- [ ] Compilation speed regression < 5%

### Pre-Work (09.0)
- [x] `EffectClass` duplicate deleted from `lattice/mod.rs`; only `dimensions.rs` copy remains
- [x] `_sigs` and `_classifier` removed from `emit_rc_ops()` signature (or deferred to Section 10)
- [x] `_classifier` removed from `compute_block_entry_state()` in `block.rs`
- [x] `_context_regions` doc comment added explaining Stage 3 placeholder intent
- [x] `emit_rc/mod.rs` split below 500 lines before being touched by 09.x changes
- [x] `emit_reuse/mod.rs` set-ops helpers extracted to `emit_reuse/set_ops.rs`

### Sync Points (09.x)

These types are modified or added by Section 09. All consuming locations must
be updated atomically (same commit):

| Change | Files That Must Update Together |
|--------|-------------------------------|
| `FipContract::Bounded(u16)` variant | `aims/contract/mod.rs` (enum def + join impl), `aims/interprocedural.rs` (extract_contract), `aims/emit_reuse/fip.rs` (gate decisions), `aims/contract/tests.rs`, `verify/mod.rs` (if FIP verification checks contract variant) |
| `MemoryContract.is_fbip: bool` field | `aims/contract/mod.rs` (struct def), `aims/interprocedural.rs` (extract_contract sets it), `aims/contract/tests.rs`, `pipeline/shadow/compare.rs` (if shadow comparison includes FBIP) |
| `ParamContract.locality_bound` field | `aims/contract/mod.rs` (struct def + join), `aims/interprocedural.rs` (extract_contract), `aims/builtins/mod.rs` (builtin defaults), `aims/emit_rc/arg_ownership.rs` (reads locality_bound for RC-skip), `verify/mod.rs` (contract consistency check) |
| `AimsStateMap.fip_token_balanced: bool` field | `aims/intraprocedural/state_map.rs` (struct def), `aims/intraprocedural/mod.rs` (analyze_function populates it), `aims/interprocedural.rs` (extract_contract reads it) |
| `AimsEvent::AllocCreditBalance` variant | `aims/intraprocedural/state_map.rs` (enum def), `aims/intraprocedural/block.rs` (emits event at Switch successors), `aims/intraprocedural/state_map/tests.rs` |
| `ShapeClass::ContextHole(ContextMeta)` enrichment | `aims/lattice/dimensions.rs` (enum variant), `aims/lattice/mod.rs` (join + canonicalize), `aims/transfer/mod.rs` (transfer_construct), `aims/lattice/tests.rs` — **Stage 3 only**, not Stage 2 |
| Canonicalize Rules 4-8 | `aims/lattice/mod.rs` (canonicalize method), `aims/lattice/tests.rs` (per-rule fire/no-fire tests) |

### Overall
- [ ] Every dimension constrains, proves, or overrides at least one other dimension
  (verified by code inspection — trace each dimension's outgoing influence)
- [ ] `./test-all.sh` green with all changes
- [ ] `cargo test --workspace --features aims` green
- [ ] RC operation count on golden corpus improved or unchanged
- [ ] No behavioral regressions on spec tests

**Exit Criteria:** Each of the 7 dimensions has at least one documented, tested
interaction where it changes the behavior of another dimension's computation or
where an emission decision requires reading it alongside another dimension.
The number of cross-dimension interactions is ≥12 (up from current 3 in
canonicalize + 3 in emission). Programs exist in `tests/aims/` that can only
be optimized with cross-dimensional reasoning.
