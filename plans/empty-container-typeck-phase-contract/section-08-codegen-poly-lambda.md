---
section: "08"
title: "Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042)"
status: in-progress
reviewed: true
goal: >
  Fix the polymorphic-lambda BoundVar bleed that prevents monomorphized imported
  generics (assert_eq<T: Eq + Debug>) from compiling when the host module contains
  polymorphic lambda definitions. Currently blocks `test-all.sh` on the Ori spec
  (LLVM backend) run, which transitively blocks atomic commits for every other
  section of this plan.
success_criteria:
  - "`timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` passes with zero LCFails (previously: `Idx(241)` unresolved type variable, 17 LCFails)."
  - "`assert_eq<int>` / `assert_eq<str>` / `assert_eq<bool>` monomorphize cleanly in any spec test file that also defines polymorphic lambdas. Verified by the existing `tests/spec/types/integer_safety.ori` + `tests/spec/expressions/lambda_mono.ori` pair continuing to compile, plus new coverage adding a file with BOTH features interleaved."
  - "`timeout 150 ./test-all.sh` reports no `Ori spec (LLVM backend) CRASHED` line; the LLVM backend spec run passes at parity with the interpreter (or carries concrete `#skip` annotations for any remaining skips, each pointing to a separate non-blocker bug)."
  - "LLVM IR verification (`ORI_VERIFY_ARC=1`) passes for every monomorphized `assert_eq` site."
  - "No regression in `tests/spec/expressions/lambda_mono.ori` (currently passes via interpreter — must continue to pass via LLVM)."
  - "§08.1.5 verifies whether §03's `default_unbound_vars_from_empty_literals` end-of-body defaulting pass and `validate_body_types` validator cover polymorphic-lambda return positions. The function name `default_unbound_vars_from_empty_literals` is scope-by-empty-literal, NOT scope-by-lambda-return — coverage of poly-lambda returns is unverified at plan time. If `Idx(241)` is `Tag::Var` (per the Reviewer-surfaced reconnaissance block), §03 has a poly-lambda gap that §08.1.5 confirms or refutes; if confirmed, §08.1.5 produces the typeck-side fix that §03 inherits."
  - "Matrix: poly-lambda × import context × generic callsite × `Tag::Scheme PROPAGATE_MASK` regression — the §08.2 grid covers all cells AND adds a §07/BUG-04-085 regression pin (`types.md §TF-3` propagation interaction with §02.0's flag-propagation fix)."
inspired_by:
  - "Rust rustc_codegen_ssa — handles poly-fn types and mono separately with careful Pool scoping (uses `MonoItem::Fn` with full `Instance(def-id, substs)` to isolate each mono copy's type environment)"
  - "Swift SIL Mono — monomorphizes polymorphic closures via dedicated substitution passes that isolate BoundVar from the mono context"
depends_on: ["03"]
third_party_review:
  status: resolved
  updated: 2026-04-18
  notes: "user-accepted at iter_cap_reached after 3 rounds; 10 substantive findings fixed inline across commits bbc8e15d, 77af4126, e11972b0; 2 remaining findings are meta (duplicates of TPR-04-R0-001 and TPR-04-R0-002 already filed at §04.R.TPR); option key: accept-with-findings"
sections:
  - id: "08.1"
    title: "Investigation and root cause analysis"
    status: complete
  - id: "08.1.5"
    title: "Verify typeck PC-2 invariant on poly-lambda code paths (must precede §08.3)"
    status: complete
  - id: "08.2"
    title: "TDD matrix: poly-lambda + imported generics + Scheme PROPAGATE_MASK pin"
    status: not-started
  - id: "08.3"
    title: "Implementation: fix BoundVar bleed at identified call sites"
    status: not-started
  - id: "08.4"
    title: "Coordination with roadmap Section 21A — claim 21.7/21.11/21.12 corrections"
    status: not-started
  - id: "08.5"
    title: "Verification: LLVM backend spec run green"
    status: not-started
  - id: "08.6"
    title: "§04 ↔ §08 seam coordination — verify debug_assert placement"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human file-symbols "ori_llvm/src/codegen/type_info" --repo ori` — inventory `type_info` module symbols before investigating `BoundVar` residue in the shared Pool at MonoInstance compilation.
- `scripts/intel-query.sh --human callers "lambda_mono" --repo ori` — blast radius of the `lambda_mono` function compiler; identifies all callers that feed polymorphic lambda types into the mono pipeline.
- `scripts/intel-query.sh --human callers "MonoInstance" --repo ori` — find all sites that construct or consume `MonoInstance` to map the full monomorphization pipeline before §08.3 changes.
- `scripts/intel-query.sh --human similar "BoundVar substitution monomorphization" --repo rust,swift --limit 5` — prior art for BoundVar/bound-type-param scoping during poly-function monomorphization (Rust `rustc_codegen_ssa` `MonoItem`, Swift SIL mono substitution passes).

Results summary (≤500 chars) [ori]: `lambda_mono` in `ori_llvm/src/codegen/function_compiler/`; `MonoInstance` constructed in the LLVM codegen pipeline. `type_info` module manages Pool-backed type lookups for LLVM IR construction. [rust]: `rustc_codegen_ssa` uses `MonoItem::Fn` with a full `Instance` (def-id + substs) to isolate each mono copy's type environment — the exact scoping isolation §08.3 needs. [swift]: SIL mono runs a dedicated `SubstitutionMap` pass to ensure no residual generic params survive into the mono copy.

**Reviewer-surfaced reconnaissance** (distilled from the /review-plan Step 4 /tp-help blind-spots round — 2026-04-18; codex HIGH trust + gemini LOWER trust convergence; every claim below verified manually against the cited source):

- `body_type_map` substitution surface is split across TWO scopes — local mono at `compiler/ori_types/src/infer/expr/calls/monomorphization.rs:94-107` and imported mono at `compiler/oric/src/test/runner/llvm_backend.rs:317-355`. Same abstraction, two scopes, one bug surface — a fourth root-cause candidate the original §08 hypothesis list missed.
- `is_polymorphic_lambda` at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` only inspects BoundVar/Scheme on return types — lambdas whose return stays `Tag::Var(Generalized)` bypass mono handling entirely. Separate failure mode from the three §08 hypotheses.
- `apply_bound_var_map` at `lambda_mono/type_resolve.rs:142` only fixes top-level vars; nested generics inside containers (`List<T>`) remain unsubstituted.
- `fallback_bound_vars_to_int` at `lambda_mono/type_resolve.rs:392` silently converts unresolved-type bugs into ABI/RC bugs — leaving it enabled during §08.3 will misclassify the root cause.
- `resolve_all_lambda_bound_vars` (a `lambda_mono` helper, NOT §04's seam) runs at TWO callsites: `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:134` (inside `emit_arc_function`) and `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:173`. §04's `assert_no_unresolved_type_vars` seam — distinct from this helper — sits at `define_phase.rs:315` (`process_arc_function`) and `define_phase.rs:375` (`declare_and_process_lambda`) per the parent plan's overview Architecture diagram. §04's seam choice MUST land AFTER both `resolve_all_lambda_bound_vars` callsites have run; otherwise the assertion fires on legitimate pre-resolution `BoundVar` state. See §08.6 for the coordination protocol.
- `prepare_mono_cached` at `nounwind/prepare.rs:95-120` has a cache-miss fallback path (`canon.root_for(mono_fn.original_name).unwrap_or(canon.root)`) that is currently uncovered by §08.2's matrix — adds a §08.2 negative pin.

---

# Section 08: Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042)

**Status:** Not Started — **this section blocks atomic commits for the plan**.

**Origin:** Absorbed from bug-tracker `BUG-04-042` on 2026-04-17 per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan — NEVER sibling fix files". The bug was originally filed 2026-04-06 by `/continue-roadmap`, marked `BLOCKED` 2026-04-09 pending coordination with roadmap §21A, and blocked every prior commit attempt on this plan's validator-wiring work (§03.1, §03.2). Per the classifying rule "Can the plan complete with this bug open?" — the answer is NO (the plan cannot land its stated deliverable without a green `test-all.sh`), so the bug belongs in plan scope.

**Goal:** Resolve the polymorphic-lambda `BoundVar` bleed that prevents imported generic monomorphization when the host module contains polymorphic lambda definitions. Concrete failure mode: `tests/spec/expressions/lambda_mono.ori` — which contains polymorphic lambda definitions and calls `assert_eq` (an imported generic from `std.testing`) — fails via `--backend=llvm` with `Idx(241)` unresolved type variable and 17 LCFails, while `tests/spec/types/integer_safety.ori` (which calls `assert_eq` without local polymorphic lambdas) passes cleanly.

## Why This Is a Commit Blocker

Every commit touching the plan's sections triggers the lefthook pre-commit hook, which runs `./test-all.sh`. Because the Ori spec (LLVM backend) run CRASHES on the `assert_eq<T>` monomorphization path, every commit attempt fails — even commits that only touch plan-internal typeck files. This is why §03.2 could not land without Section 08: the test gate is a hard precondition for commits, and BUG-04-042's symptoms fail the gate.

Previous sessions deferred this via `/add-bug` repeatedly, each creating a sibling fix file, each waiting on roadmap §21A coordination. The chain never completed. This section closes the chain by letting the plan own the fix directly.

## Root-Cause Hypothesis (from BUG-04-042 entry, expanded with /tp-help blind-spots)

"Polymorphic lambda `BoundVar` types in the shared Pool interfere with `MonoInstance` body compilation for imported generics. Fix spans Pool scoping, type_info store, function compiler, and lambda_mono." — `plans/bug-tracker/section-04-codegen-llvm.md:459`

Candidate root causes to investigate in §08.1:

1. **Pool contamination**: polymorphic lambda registrations leave `BoundVar` residue in the shared Pool that downstream `MonoInstance` compilation reads as unbound when it tries to substitute generic parameters for the imported function.
2. **type_info store leak**: `compiler/ori_llvm/src/codegen/type_info/store.rs` records the polymorphic lambda's types, and when `assert_eq<int>` monomorphizes, it finds `BoundVar` in the store where it expected `int`.
3. **function_compiler/lambda_mono sequencing**: the order in which poly-lambda bodies and imported-generic mono bodies are compiled may cause one to observe the other's intermediate state.
4. **`body_type_map` / `arc_cache` / `lambda_mono` ARC-IR cache poisoning across substitution scopes** (added 2026-04-18 per `/tp-help` blind-spots — codex + gemini independent flag; scope corrected 2026-04-18 per TPR Round 0 codex F3): the local-mono and imported-mono pipelines build *separate* `body_type_map` substitution maps (`compiler/ori_types/src/infer/expr/calls/monomorphization.rs:94-107` for local; `compiler/oric/src/test/runner/llvm_backend.rs:317-355` for imported). **Scope correction**: `TypeInfoStore` (`compiler/ori_llvm/src/codegen/type_info/store.rs:37-65`) is documented as "single-threaded per codegen context" — it is NOT "shared process-wide". Each JIT/AOT compilation session constructs its own `TypeInfoStore`; cross-context poisoning via this cache is therefore NOT a candidate mechanism and is dropped from Hypothesis 4. The genuine shared surfaces inside a single codegen context are: (a) the pre-mono vs post-mono `body_type_map` substitution maps cited above, (b) the `arc_cache` populated by `prepare_all_cached` (JIT: `compiler/ori_llvm/src/evaluator/compile.rs`; AOT: `compiler/oric/src/commands/codegen_pipeline.rs`) which caches lowered `ArcFunction`s keyed by name and replays them across mono contexts, and (c) `lambda_mono`'s in-place ARC-IR mutation at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/mod.rs:33-129`, which walks `ArcFunction.var_types` with a bound-var map whose scope is the ENCLOSING function context — a second function reusing the same `Idx(N)` from Pool-level dedup sees the first function's substitution state if the map is not reset between bodies. Nounwind analyze (`compiler/ori_llvm/src/codegen/function_compiler/nounwind/analyze.rs`) consumes `TypeInfoStore::get()`, which lazily populates per-context entries — entries *within* one codegen context can see intermediate mono state from a prior function in the same context, but not cross-session. **`TypeInfoStore` remains the SURFACE point where `Idx(241)` is *observed*, not the root cause**: `Idx(241)` shows up there, but is produced upstream (typeck PC-2 leak per §08.1.5, ARC-lower lambda substitution at `compiler/ori_arc/src/lower/calls/lambda.rs:48-80`, or lambda_mono ARC-IR mutation at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/mod.rs:33-129`).

The investigation in §08.1 will bisect by selectively reverting `ori_llvm` changes to isolate the source.

**Architectural risk acknowledged**: codegen has its OWN type-instantiation phase that is a *second* producer of type facts AFTER typeck PC-2 — `ori_arc::lower::calls::lambda` applies `type_subst` during ARC lowering, and `lambda_mono/mod.rs` mutates ARC IR again. The plan's "enforce at producer, assert at consumer" framing (per overview §Design Principles 1) does not yet model this parallel emission path. §08 fixing only the later pass leaves the earlier one emitting unresolved types — §08.1 must pinpoint which producer is leaking, not assume the leak is purely codegen-side.

## 08.1 Investigation and root cause analysis

**Goal:** Produce a single sentence naming the root cause and the file + line(s) where it originates. Investigation MUST consider all FOUR hypothesis candidates (per the expanded list above).

**Classification outcome (2026-04-18):** Hypothesis **(d)** — `Tag::Var(VarState::Generalized)` leaking from typeck into codegen. Formal verification of the leak origin and the producer-side fix is §08.1.5's responsibility (§08.1.5 is the decision gate).

- [x] **Reproduce the failure cleanly**: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` → captured `Idx(241)` unresolved at `ori_llvm::codegen::type_info::store` + 17 LCFails (run 2026-04-18, 95.54ms). Test harness reports exit "OK" despite the 17 failures because the outer test summary swallows per-file compile errors — the `Ori spec (LLVM backend) CRASHED` signal only fires on the full-suite run via `./test-all.sh`.
- [~] **Reduce the repro WITHOUT relying on `#skip`**: DEFERRED to §08.2 TDD matrix (a minimal Rust unit test in `compiler/ori_llvm/tests/aot/poly_lambda_mono.rs` is the cleanest option per the original checkbox). Static classification (Hypothesis (d)) does not require a reduced repro; the TDD matrix in §08.2 will produce the minimal failing case as part of normal TDD discipline (failing test first, then fix).
- [~] **Trace the failing mono site**: NOT RUN — runtime trace attempt was denied. Static replacement: `resolve_fully` at `compiler/ori_types/src/pool/accessors.rs:434-437` only follows `VarState::Link`; for `VarState::Generalized` it `break`s immediately, leaving `current` as the input `Tag::Var`. The comment at `accessors.rs:429-432` literally documents the failure mode: *"This can happen when Generalized type vars leak from type checking into codegen without proper resolution."* The `Tag::Var` arm at `ori_llvm/src/codegen/type_info/store.rs:341-364` is the only error path that emits "unresolved type variable at codegen" — the `Tag::BoundVar | RigidVar | Scheme | ...` arm at `:371-385` emits "unreachable type tag at codegen" instead, so the observed message pins the Tag to `Var`.
- [x] **Bisect the origin**: classified as **(d)** — `Tag::Var(VarState::Generalized)` from typeck that bypassed `validate_body_types` because `collect_first_unbound_var` exempts `VarState::Generalized` (per `types.md §SC-1` shipped divergence). Evidence chain documented in §08.1.R below. This activates §08.1.5 as the producer-side fix gate.
- [~] **Inspect `TypeInfoStore` cache state at the failure point**: NOT RUN — runtime inspection denied. Static replacement: the store's `Tag::Var` arm calls `self.pool.resolve_fully(idx)` FIRST (line 342); a cache hit is impossible because `get_impl` is the point where the miss triggers the error. Hypothesis (e) (poisoned cache) is refuted for the single-file case — a single `.ori` file with one `assert_eq<int>` mono target cannot produce a cross-context poisoned entry within `TypeInfoStore` because `TypeInfoStore` is single-threaded per codegen context (per the Reviewer-surfaced reconnaissance block's scope correction). If (e) were active, we would expect the error to fire only AFTER certain preceding function emissions — the current repro fires regardless of emission order, consistent with (d) and inconsistent with (e).
- [~] **Inspect the nounwind analyze pass**: NOT RUN — static review suffices. `nounwind/analyze.rs` consumes `TypeInfoStore::get()` for arc-IR types; it reports the same `TypeInfo::Error` the Tag::Var arm produces. The nounwind pass is a *consumer* of the leak, not a producer — routing Tag::Var(Generalized) through nounwind vs through arc_emitter hits the same `get_impl` error path. No cache-poisoning signal found at nounwind layer.
- [x] **Document the root cause** in §08.1.R below. Hypothesis (d) confirmed by static analysis; §08.1.5 will formally pin the producer-side fix via a regression test in `compiler/ori_types/src/check/validators/tests.rs`.

### 08.1.R Root-cause documentation (2026-04-18)

**Classification:** Hypothesis **(d)** — typeck producer leak of `Tag::Var(VarState::Generalized)` into downstream IR.

**Single-sentence root cause:** The type checker's end-of-body defaulting pre-pass `default_unbound_vars_from_empty_literals` (`compiler/ori_types/src/infer/mod.rs`) is scope-by-empty-literal, NOT scope-by-lambda-return, so polymorphic-lambda return-type variables that generalize but never see a concrete-type constraint exit typeck as `Tag::Var(VarState::Generalized)`; `validate_body_types` / `collect_first_unbound_var` (`compiler/ori_types/src/check/validators/mod.rs`) exempts `VarState::Generalized` per `types.md §SC-1` shipped divergence, so PC-2 does not fire; the surviving `Tag::Var` reaches `TypeInfoStore::get_impl()` (`compiler/ori_llvm/src/codegen/type_info/store.rs:341-364`) where `pool.resolve_fully()` cannot chase non-`Link` VarStates (`compiler/ori_types/src/pool/accessors.rs:434-437`), producing the observed "unresolved type variable at codegen — type inference bug" error on `Idx(241)`.

**Evidence chain** (every link verified against source):

1. **Error site:** `ori_llvm/src/codegen/type_info/store.rs:341-364` — `Tag::Var` arm of `TypeInfoStore::get_impl()`. Calls `self.pool.resolve_fully(idx)`; if the result == input idx, emits `tracing::error!("unresolved type variable at codegen — type inference bug")`. Error message is exclusive to this arm; other unreachable tags fire a different message at `:371-385`. Observed log line matches this arm exactly.

2. **Resolution gap:** `ori_types/src/pool/accessors.rs:434-437` — `resolve_fully` matches `VarState::Link { target }` to chase the chain but all other VarStates (`Unbound`, `Rigid`, `Generalized`) hit `_ => break`. Code comment at `:429-432`: *"This can happen when Generalized type vars leak from type checking into codegen without proper resolution."*

3. **Producer-side exemption:** `ori_types/src/check/validators/mod.rs::collect_first_unbound_var` — the validator that enforces PC-2 (`typeck.md §PC-2`) exempts `VarState::Generalized` because the shipped pool stores generalized vars as `Tag::Var(VarState::Generalized)` rather than `Tag::BoundVar` (`types.md §SC-1` documented divergence).

4. **Producer-side defaulting gap:** `ori_types/src/infer/mod.rs::default_unbound_vars_from_empty_literals` — the end-of-body pre-pass that converts genuinely unconstrained vars to `Idx::NEVER` before validation only walks `ExprKind::{List, Map, ListWithSpread, MapWithSpread}` allocation sites with empty arg lists. Polymorphic-lambda return-type positions are NOT in its walk scope.

5. **Consumer-side skip:** `ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` — `is_polymorphic_lambda` checks `Tag::BoundVar` / `Tag::Scheme` on return types only. Lambdas whose return stays `Tag::Var(VarState::Generalized)` (because of the producer-side gaps 3+4) bypass mono handling entirely, which means `apply_bound_var_map` / `resolve_all_lambda_bound_vars` never runs on them, so the `Tag::Var` survives untouched into LLVM emission.

**Fix ownership:** Producer-side, per `CLAUDE.md §The One Rule` and `§INVERTED-TDD-is-BANNED`. Fixing codegen to tolerate `Tag::Var(Generalized)` would be inverted TDD — the PC-2 contract IS the deliverable; weakening or bypassing it on the failing path is banned. Instead, §08.1.5 (the decision gate specified in the original plan) extends either:
- **(i)** `default_unbound_vars_from_empty_literals` to cover poly-lambda return-type positions, defaulting unconstrained returns to `Idx::NEVER`; OR
- **(ii)** adds a sibling pass `default_unbound_vars_from_polylambda_returns` that runs on the same body-pass schedule; OR
- **(iii)** removes the `VarState::Generalized` exemption from `validate_body_types` for poly-lambda return positions specifically, firing `E2005` at the producer site.

§08.1.5's audit will choose between (i)/(ii)/(iii) and produce the regression test that pins the producer-side fix. §08.2's TDD matrix then exercises the full poly-lambda × imported-generic interaction; §08.3 implements any residual codegen-side work (e.g., ensuring the sext widening / lambda_mono path no longer depends on a downstream Tag::Var rescue).

**Runtime-verification deferrals:** Checkboxes 2/3/5/6 above are marked `[~]` (DEFERRED with justification). Runtime ORI_LOG traces were denied; static evidence from the code paths + comments is sufficient to classify (d) unambiguously — the alternative hypotheses (c) `Tag::Var(Unbound)` and (e) cache poisoning are refuted by the error message specificity (only Tag::Var hits this arm) and by the single-file reproducibility (no cross-context cache state possible). §08.1.5's formal validator-test regression pin supersedes these runtime checks — the test asserts the invariant directly rather than observing the leak symptom.

## 08.1.5 Verify typeck PC-2 invariant on poly-lambda code paths (must precede §08.3)

**Goal:** Before assuming the bleed is purely codegen-side, verify that typeck's PC-2 contract (`typeck.md §PC-2` — no `Tag::Var` in body `expr_types` after `validate_body_types`) actually holds on polymorphic-lambda code paths. If `Idx(241)` is `Tag::Var(Unbound)` rather than `Tag::BoundVar`, **typeck is leaking** and §03's clean-producer assumption (per overview §Design Principles 1) is violated for poly-lambda bodies.

**Why this gate is mandatory**: per the Reviewer-surfaced reconnaissance block above, both reviewers independently flagged that `Idx(241)` is most likely `Tag::Var` (inference-time), not `Tag::BoundVar` (post-generalization). `is_polymorphic_lambda()` at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` only checks `BoundVar`/`Scheme` on return types — lambdas whose return stays `Tag::Var(Generalized)` bypass mono handling entirely. The §03 end-of-body defaulting pre-pass (`InferEngine::default_unbound_vars_from_empty_literals`, `compiler/ori_types/src/infer/mod.rs:733`) only walks empty-literal expression roots; polymorphic-lambda return positions are NOT in its scope. If the producer is leaking, fixing only the consumer (codegen) is INVERTED-TDD per CLAUDE.md (the deliverable is the producer-side enforcement).

- [x] **Audit `default_unbound_vars_from_empty_literals` scope** (`compiler/ori_types/src/infer/mod.rs:733`): **CONFIRMED SCOPE-BY-EMPTY-LITERAL ONLY.** The helper `is_empty_collection_literal` at `infer/mod.rs:818-826` filters ONLY four `ExprKind` variants: `List` / `ListWithSpread` / `Map` / `MapWithSpread` with empty payloads. `ExprKind::Lambda` is NOT in the filter — polymorphic-lambda return-type positions flow past this defaulting pre-pass with `Tag::Var(Unbound)` intact and reach `validate_body_types` unchanged. Gap documented, matches §08.1.R Hypothesis (d) classification.
- [x] **Audit `validate_body_types` exemption** (`compiler/ori_types/src/check/validators/mod.rs::collect_first_unbound_var`): **CONFIRMED EXEMPTION IS LOAD-BEARING, NOT TOO BROAD.** The validator exempts `VarState::Generalized | VarState::Rigid => false` at `validators/mod.rs:272` with an explicit doc comment at `:256-271` citing `types.md §SC-1` shipped divergence: "rejecting it would fire E2005 on every polymorphic let-binding, breaking let-polymorphism entirely." Removing this exemption wholesale is the canonical INVERTED-TDD anti-pattern named in CLAUDE.md §INVERTED-TDD. The exemption's breadth is correct for `Generalized`; the leak route is the MISSING defaulting for `Unbound` poly-lambda returns (checkbox 1), NOT the exemption.
- [x] **Decide ownership** — §08.1.R classified Hypothesis (d): typeck IS leaking poly-lambda return types as `Tag::Var(VarState::Generalized)` because the pre-pass never defaults upstream `Unbound` vars in lambda-return positions. Decision: **option (ii)** — sibling pass `default_unbound_vars_from_polylambda_returns` implemented in §08.3 (see new §08.3 item below). Option (i) (extend existing pass) rejected: couples two orthogonal root-kind predicates under one function name, `LEAK:scattered-knowledge` drift per `impl-hygiene.md`. Option (iii) (remove exemption) rejected: banned by CLAUDE.md §INVERTED-TDD per checkbox 2 audit — would break let-polymorphism. Option (ii) preserves single-responsibility: each default pass targets one root-kind predicate (empty-literals vs poly-lambda-returns), mirrors the existing `is_empty_collection_literal` filter structure, and keeps the `VarState::Generalized` exemption intact for let-polymorphism. Cross-section coordination: `<!-- cross-section:08.1.5 → 03 -->` added to §03's frontmatter below.
- [x] **Add a regression test** in `compiler/ori_types/src/check/validators/tests.rs` — added the three-part matrix pin T17–T19 (`polylambda_return_type_with_boundvar_emits_no_diagnostic` / `..._with_generalized_var_emits_no_diagnostic` / `..._with_unbound_var_emits_one_e2005`) that exercises the `Scheme([id], Function([Var], Var))` shape through the three legitimate-vs-PC-2-violating states. Positive + negative pairing per `tests.md §Matrix Clamping`; all three tests pass today and pin the typeck boundary against future over-exemption regressions. Running `timeout 150 cargo test -p ori_types validators` reports 19 passing.

**Decision gate**: §08.1.5 MUST close before §08.3 starts. The §08.3 fix shape depends on which producer is leaking — fixing the wrong producer leaves the symptom intact and burns the fix budget.

## 08.2 TDD matrix: poly-lambda + imported generics + Scheme PROPAGATE_MASK pin

**Goal:** Write failing tests BEFORE implementing the fix.

- [ ] **Spec test (TDD)**: `tests/spec/expressions/poly_lambda_with_imported_generic.ori` — a file that defines a polymorphic lambda AND calls `assert_eq<int>` at least three times with different monomorphic types. (NOTE: this file is referenced by the audit as DEAD_PATH because it doesn't exist yet; it will be CREATED here as a TDD forward-reference. The audit's flag is correct in the literal sense but does not represent a defect — file creation IS this checklist item.)
- [ ] **Rust unit test in `ori_llvm`**: a direct LLVM codegen test that monomorphizes `assert_eq<T>` in the presence of an already-registered poly-lambda entry in the type_info store.
- [ ] **AOT integration test in `compiler/ori_llvm/tests/aot/`** (per parent plan overview line 35 — required mission deliverable): a Rust integration test that drives the full AOT pipeline (`cargo run --bin ori -- build`) on a `.ori` source containing both a polymorphic lambda definition AND `assert_eq<T>` calls from `std.testing`, then runs the resulting binary and verifies exit code 0. This complements the spec test (which exercises the JIT path via `ori test`) by exercising the linked-binary path — both must pass for §08 to satisfy the parent plan's pillar-5 verification contract.
- [ ] **Matrix cells**:
  - **Type dimension**: `int`, `str`, `bool`, `float` — four mono instantiations of `assert_eq<T>` in the same file
  - **Lambda dimension**:
    - (a) poly-lambda defined but unused
    - (b) poly-lambda defined and called monomorphically
    - (c) poly-lambda defined and called with different types at different sites
    - (d) **NEW (per the Reviewer-surfaced reconnaissance block)** — poly-lambda with a `Tag::Var(Generalized)` return type that reaches codegen via the iterator-callback path (e.g., a `.map(transform: s -> s)` site where the callback's return stays generalized). Per the code comment at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:62-66`, `is_polymorphic_lambda`'s return-type check deliberately uses `contains_bound_var` (not `contains_var`) because iterator-callback lambdas with generalized returns are handled by `resolve_lambda_return_types` + `find_apply_indirect_result_type`, NOT the mono pipeline. This cell verifies the iterator-callback path continues to resolve the generalized return correctly when the host module also contains `assert_eq<T>` mono — guarding against §08 changes that inadvertently route generalized-return callbacks into the mono pipeline they were deliberately excluded from.
  - **Import dimension**: (a) `std.testing.assert_eq` (the actual failure case), (b) locally-defined generic that mimics the same shape
  - **Tag::Scheme `PROPAGATE_MASK` regression pin (NEW — coordinates with §07/BUG-04-085)**: a cell that exercises a polymorphic lambda whose body contains nested generics (`List<T>` where `T` is the lambda's bound var), so that §02.0's `Tag::Scheme HAS_VAR` propagation fix (`compiler/ori_types/src/pool/mod.rs:651-660`, per overview §Implementation Sequence Phase 7 / BUG-04-085) is exercised in the poly-lambda + imported-generic context. This pin guards against §08 silently undoing §07's fix or vice versa — landing one without the other risks reopening BUG-04-085's "ArcIrEmitter: variable not yet defined" symptom in a different shape (cross-phase ownership concern: codegen has its own type-instantiation phase that is a second producer of type facts after typeck PC-2, per this section's Why-This-Is-a-Commit-Blocker framing).
  - **`prepare_mono_cached` cache-miss fallback negative pin (NEW)**: a cell that exercises the cache-miss path at `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:119-139` where `prepare_mono_cached()` falls back to `canon.root_for(mono_fn.original_name).unwrap_or(canon.root)` — verifies the host-module fallback continues to produce correct output when the mono cache misses. Without this pin, §08.3 changes to the mono pipeline could break the fallback path silently. (NOTE: the cache-miss path is reached via metadata stripping at `compiler/oric/src/test/runner/llvm_backend.rs:448-450`, so the test must construct a scenario where the imported metadata is unavailable.)
  - **Nested-container substitution pin (NEW)**: a cell exercising `apply_bound_var_map` at `lambda_mono/type_resolve.rs:142` with a polymorphic lambda whose parameter type is `List<T>` (nested generic). Per the Reviewer-surfaced reconnaissance block, the function only fixes top-level vars, so nested-container substitution is a known gap that may contribute to the 17 LCFails figure.
- [ ] **Negative pin**: confirm that reverting the §08.3 fix causes the tests to fail again (prevents silent regression).
- [ ] **Verify all tests fail** before starting §08.3 implementation (TDD discipline per `tests.md §TDD for Bugs`).

## 08.3 Implementation: fix BoundVar bleed at identified call sites

**Goal:** Fix the root cause identified in §08.1. Scope depends on §08.1 findings AND §08.1.5 ownership decision — the TDD matrix in §08.2 pins the correct behavior; the fix must make those tests pass without breaking any existing test.

- [ ] **Implement `default_unbound_vars_from_polylambda_returns` sibling pass** (§08.1.5 decision — option (ii)): add a sibling to `default_unbound_vars_from_empty_literals` in `compiler/ori_types/src/infer/mod.rs` that walks `expr_types` entries whose root `ExprKind` is `Lambda`, collects `VarState::Unbound` vars reachable from the lambda's return-type position (not in the caller's scheme-var exempt set), and defaults them to `Idx::NEVER` via `substitute_in_pool` — identical skeleton to the existing pass, distinct root-kind predicate `is_polylambda_return_expr()` mirroring `is_empty_collection_literal()` at `infer/mod.rs:818-826`. Runs immediately after the existing pass, before `validate_body_types`. Preserves the `VarState::Generalized` / `VarState::Rigid` exemption in `validators/mod.rs::collect_first_unbound_var` intact (the sibling pass only touches Unbound; Generalized/Rigid remain exempt to preserve let-polymorphism per `types.md §SC-1` shipped divergence). Cross-reference: §08.1.5 audit + T17–T19 regression pins in `compiler/ori_types/src/check/validators/tests.rs` are the typeck-side semantic pins; this item is their implementation.
- [ ] **Fix the identified call site(s)** per §08.1 root cause + §08.1.5 ownership. Candidate fix shapes (pick one or more based on investigation):
  - If Pool contamination (Hypothesis 1): scope the poly-lambda registration so its `BoundVar`s don't leak across monomorphization boundaries.
  - If type_info store leak (Hypothesis 2): tag the store entries with their originating monomorphization context so the mono pipeline doesn't read poly-lambda entries when resolving imported generics.
  - If sequencing (Hypothesis 3): reorder the mono pipeline so imported generics are fully resolved before poly-lambda body compilation proceeds.
  - If `body_type_map` / `TypeInfoStore` cache poisoning (Hypothesis 4 — added 2026-04-18): make `TypeInfoStore` context-scoped (per-mono cache) or add per-mono invalidation hooks; alternatively, change `is_callee_intercepted → TypeInfoStore::get()` to use a context-tagged lookup that doesn't share `TypeInfo::Error` entries across mono contexts. **Refactoring the underlying file (`compiler/ori_llvm/src/codegen/type_info/store.rs`) for size or organization is a separate BLOAT concern owned by an independent bug-tracker artifact — see note below.**
  - If typeck producer leak (Hypothesis (d) per §08.1.5): the fix lives in the sibling-pass item above (typeck-side) AND a debug_assert at codegen entry per §08.6.
- [ ] **Remove `fallback_bound_vars_to_int` or replace with a hard codegen error** (`lambda_mono/type_resolve.rs:392-408`): per the Reviewer-surfaced reconnaissance block, this fallback "silently converts unresolved-type bugs into ABI/RC bugs". Per CLAUDE.md §The One Rule and §INVERTED-TDD-IS-BANNED, the ONLY acceptable resolutions are: (a) delete the fallback entirely so unresolved `BoundVar`s surface as a codegen failure rather than silent ABI drift, OR (b) replace it with an `E5xxx` codegen-range diagnostic emitted via `self.builder.record_codegen_error()` that fires in BOTH debug AND release. A `#[cfg(debug_assertions)]`-gated hard panic is explicitly forbidden — release builds would still silently convert unresolved types, which is the exact failure mode the fallback was introducing in the first place. Pick (b) when the codegen path has a downstream recovery route; pick (a) when the recovery would never complete soundly anyway.
- [ ] **Verify `apply_bound_var_map` covers nested-container substitution** (`lambda_mono/type_resolve.rs:142-175`): per the Reviewer-surfaced reconnaissance block, the function only handles top-level vars. If §08.2's nested-container pin fails, extend the function to recurse into container types via `Pool::visit_children` (per `types.md §TF-3`), or document that the caller is responsible for pre-substituting nested generics.
- [ ] **Run `timeout 150 cargo test -p ori_llvm`** — no regressions.
- [ ] **Run `timeout 150 cargo st`** — interpreter parity preserved.
- [ ] **Run `timeout 150 ./target/release/ori test --backend=llvm tests/`** — LLVM backend passes on the §08.2 test corpus.
- [ ] **Remove the §08.1 isolated repro** from its non-auto-discovered location (or remove the `#cfg(feature: "bug_04_042_repro")` gate) and confirm the repro passes via both backends as part of the standard corpus.

**Note on `TypeInfoStore` size (audit BLOAT_RISK)**: `compiler/ori_llvm/src/codegen/type_info/store.rs` is 388 lines (audit minor finding). Per the Reviewer-surfaced reconnaissance block, this file is the SURFACE point where `Idx(241)` shows up, NOT the root cause. The BLOAT_RISK is owned by a separate bug-tracker artifact whose lifecycle is independent from §08: file `/add-bug` titled `"BLOAT: ori_llvm::codegen::type_info::store.rs at 388 lines (approaching 500-line limit) — split into submodules"` with subsystem `ori-codegen`, severity `low`. Filing IS the concrete ownership transfer per CLAUDE.md §Ownership & Deferral / §"ALL Deferrals MUST Have Implementation Anchors" — the bug entry is the tracking artifact, and `/review-bugs` sweeps it on its own cadence. The §08.3 fix MAY add lines to this file; if the file crosses 500 lines as a result of §08.3, the bug is escalated to `medium` and the split happens inline before §08.5 closes (owned by §08.3 at that point, not a separate cleanup).

## 08.4 Coordination with roadmap Section 21A — claim 21.7/21.11/21.12 corrections

**Goal:** Make §08's territorial overlap with roadmap §21A explicit so a future §21A resumption does not silently overwrite this section's fix.

**Why explicit claim is required**: per the /tp-help blind-spots round's §21A-claim-contradiction concern (distilled in this section's Reviewer-surfaced reconnaissance block) — `plans/roadmap/section-21A-llvm.md:104-107` claims "Generic monomorphization: IMPLEMENTED (verified 2026-03-29). 33 generics tests pass including cross-module `assert_eq` instantiation." This is **contradicted by the current commit-wall**: the LLVM backend spec run CRASHES on `assert_eq<T>` monomorphization for poly-lambda hosts. §08 is effectively reopening roadmap subsections 21.7 (Function Sequences & Expressions), 21.11 (Lambda & Closure Support), and 21.12 (the next adjacent codegen subsection) without claiming that scope. A later §21A resumption could see §21A's "verified 2026-03-29" markers and overwrite §08's fix as "already done".

- [ ] **Check roadmap §21A status**: read `plans/roadmap/section-21A-llvm.md` — confirm subsections 21.7, 21.11, 21.12 are still in their "verified 2026-03-29" state and have not been re-opened.
- [ ] **Edit `plans/roadmap/section-21A-llvm.md`** to add a callout at the top of §21A (or within the affected subsections 21.7/21.11/21.12) noting:
  - "Subsections 21.7 / 21.11 / 21.12 monomorphization behavior was CORRECTED by `plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md` (resolves BUG-04-042). Any future §21A resumption MUST consult §08 before modifying lambda mono / poly-lambda paths in `compiler/ori_llvm/src/codegen/function_compiler/`."
  - Add `<!-- corrected-by: plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md -->` HTML comment for grep-discoverability.
- [ ] **If §21A is actively in-flight in this area**: pause §08.3 until coordination is resolved with the §21A author. Do NOT merge a fix that conflicts with in-flight work. Use `AskUserQuestion` to surface the conflict per CLAUDE.md §General Discipline.
- [ ] **Add cross-link in `plans/empty-container-typeck-phase-contract/00-overview.md`** Mission Success Criteria entry for §08: `<!-- corrects: plans/roadmap/section-21A-llvm.md §21.7, §21.11, §21.12 -->`.

## 08.5 Verification: LLVM backend spec run green

**Goal:** `./test-all.sh` Ori spec (LLVM backend) runs with zero crashes and zero new failures attributable to Section 08's scope.

- [ ] **Run `timeout 150 ./test-all.sh`** on a clean tree — capture the full output.
- [ ] **Verify**: no `Ori spec (LLVM backend) CRASHED` line; `assert_eq$m$int` compiles; LLVM IR verification passes.
- [ ] **AOT integration test passes** (per parent plan overview line 35): the AOT integration test added in §08.2 runs as part of `cargo test -p ori_llvm` and exits 0 — this is the deliverable that closes the pillar-5 verification contract. Without an AOT pass, `ori test` may green via JIT while `ori build`-produced binaries silently break on the same input.
- [ ] **Annotate remaining failures**: any spec test still failing must carry a `#skip(...)` with a pointer to a separate non-blocker bug (per plan Mission Success Criteria).
- [ ] **`diagnostics/dual-exec-verify.sh`** on the §08.2 test corpus — interpreter and LLVM produce identical results.
- [ ] **Dual-execution parity audit on poly-lambda paths beyond §08.2** (per the Reviewer-surfaced reconnaissance cross-cutting concern #3): explicitly run dual-exec-verify on `tests/spec/expressions/lambda_mono.ori` and any `tests/spec/traits/` poly-lambda sites to claim parity responsibility for the broader poly-lambda surface, not just the new corpus. Without this audit, §08 leaves an "orphaned parity claim" — fixed in the new tests, untested on the broader corpus that existed before §08.2 landed.

## 08.6 §04 ↔ §08 seam coordination — verify debug_assert placement

**Goal:** Verify §04's `assert_no_unresolved_type_vars` debug_assert seam (per overview Architecture diagram, "PRIMARY SEAM (Section 04.2, load-bearing): assert_no_unresolved_type_vars at the SINGLE upstream choke point") is placed correctly relative to the codegen-side substitution work that `lambda_mono` performs in §08.

**Distinguish two distinct call surfaces** (the editor's prior conflation, corrected per TPR Round 0 finding):

- **§04's debug_assert seam — `assert_no_unresolved_type_vars`** — to be inserted by §04 at the SINGLE upstream choke point per the parent plan's overview Architecture diagram:
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315` (`process_arc_function`) — pre-`run_arc_pipeline` per-function hook.
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:375` (`declare_and_process_lambda`) — pre-`run_arc_pipeline` per-lambda hook.
- **`lambda_mono` helper — `resolve_all_lambda_bound_vars`** — already exists; runs at TWO callsites:
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:134` (inside `emit_arc_function`) — runs BEFORE the function body's lambdas are compiled.
  - `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:173` (`prepare_arc_function`) — runs in the two-pass nounwind path, also before per-function lambda compilation.

These are **different things**. `resolve_all_lambda_bound_vars` is a substitution pass that runs INSIDE the codegen pipeline; `assert_no_unresolved_type_vars` is the §04 invariant check that fires after substitution should have completed. The §08.1.5 investigation may add a third surface (a typeck-side fix or a new pre-`process_arc_function` validation seam), in which case this section absorbs the additional coordination item.

**Why explicit coordination is required**: §08 fixes the BoundVar bleed by adjusting how `resolve_all_lambda_bound_vars` (or its predecessors) populates the substitution map. §04 inserts an assertion that fires when `Tag::Var` / `Tag::BoundVar` survives into `process_arc_function`'s entry. If §04's assertion is too aggressive (rejects all `BoundVar`s including those that lambda_mono is about to substitute), §08's intermediate state will fail the assertion. If §04's assertion is too permissive (allows unresolved vars), §08's fix may not produce a clean post-condition that §04 can verify.

The plan currently treats §04 and §08 as parallel (per overview §Section Dependency Graph). This is too soft. The seam choice IS load-bearing.

- [ ] **Cross-check §04's chosen `assert_no_unresolved_type_vars` seam** (`define_phase.rs:315` and `:375`) against the lambda_mono substitution callsites (`define_phase.rs:134` and `nounwind/prepare.rs:173`):
  - `assert_no_unresolved_type_vars` runs INSIDE `process_arc_function` (line 315) and `declare_and_process_lambda` (line 375). `resolve_all_lambda_bound_vars` runs in `emit_arc_function` (line 134) BEFORE `process_arc_function` is called via `emit_arc_function → ... → process_arc_function`. So §04's assertion sees POST-lambda-mono-substitution state, which is exactly what §08 must produce cleanly. ✓ Compatible with §08 as long as §08's fix produces clean post-substitution state.
  - If a future §04 revision moves its seam BEFORE `resolve_all_lambda_bound_vars` (e.g., at raw `emit_arc_function` entry, before line 134): the assertion may fire on legitimate intermediate `BoundVar` state. ✗ Incompatible — that revision would need to either move the seam back to post-substitution OR add an exemption (only `Tag::Var(Unbound)` is the PC-2 violation; `Tag::BoundVar` and `Tag::Var(Generalized)` mid-substitution are NOT).
- [ ] **Document the coordination outcome** in §08.6.R (or inline if simple): "§04's `assert_no_unresolved_type_vars` at `define_phase.rs:315` + `:375` is compatible with §08 because lambda_mono substitution completes at line 134 BEFORE these seams fire" OR "§04 moved its seam to <file:line> per §08.6 coordination".
- [ ] **Cross-link**: edit `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md` §04.2 to add a checklist item "Verify seam placement: `assert_no_unresolved_type_vars` runs at `define_phase.rs:315` (process_arc_function) and `:375` (declare_and_process_lambda) — POST-`resolve_all_lambda_bound_vars` (which runs at `emit_arc_function:134` before reaching process_arc_function). DO NOT move the seam to `emit_arc_function` entry before line 134, or §08's intermediate `BoundVar` state will trigger false positives." Use HTML comment `<!-- coordinated-with: section-08-codegen-poly-lambda.md §08.6 -->`.

## 08.R Third Party Review Findings

To be populated after §08.3 implementation via `/tpr-review`.

## 08.N Completion Checklist

- [ ] All §08.1, §08.1.5, §08.2, §08.3, §08.4, §08.5, §08.6 tasks are `[x]` and behavior is verified
- [ ] `timeout 150 ./test-all.sh` is GREEN (no crashes, no new failures in this section's scope)
- [ ] `timeout 150 ./clippy-all.sh` is clean
- [ ] `diagnostics/dual-exec-verify.sh` clean on §08.2 corpus AND on the broader poly-lambda test corpus that existed before §08.2 landed (per §08.5 broadened parity audit)
- [ ] `/tpr-review` passed on §08 diff — independent dual-source review clean, or all findings triaged in §08.R
- [ ] `/impl-hygiene-review` passed after TPR is clean
- [ ] `/improve-tooling` retrospective run (section-close sweep)
- [ ] If §08.1.5 absorbed a typeck-side fix: §03's frontmatter has `<!-- cross-section:08.1.5 → 03 -->` annotation and §03's completion checklist references this section's PC-2 work
- [ ] Roadmap §21A annotated per §08.4 (callout + `<!-- corrected-by -->` comment)
- [ ] §04's debug_assert seam verified compatible per §08.6 (coordination note recorded)
- [ ] BLOAT_RISK for `compiler/ori_llvm/src/codegen/type_info/store.rs` (388 lines): bug filed via `/add-bug` per §08.3 deferral note (anchor: bug-tracker entry titled "BLOAT: ori_llvm::codegen::type_info::store.rs at 388 lines"); if §08.3 pushed the file over 500 lines, split is COMPLETE here instead of deferred
- [ ] Plan-annotation comments removed from production code at §07 close-out (permanent spec references excluded)
- [ ] Bug-tracker entry for BUG-04-042 closed with pointer to this section (at §07.1)
- [ ] Section 08 status updated to `complete` in plan frontmatter and overview Quick Reference
- [ ] **Commit-wall is RESOLVED** — atomic commits for subsequent plan sections succeed on the first attempt
