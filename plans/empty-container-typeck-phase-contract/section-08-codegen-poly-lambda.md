---
section: "08"
title: "Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042)"
status: not-started
reviewed: false
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
  - "§08.1.5 verifies whether §03's `default_unbound_vars_from_empty_literals` end-of-body defaulting pass and `validate_body_types` validator cover polymorphic-lambda return positions. The function name `default_unbound_vars_from_empty_literals` is scope-by-empty-literal, NOT scope-by-lambda-return — coverage of poly-lambda returns is unverified at plan time. If `Idx(241)` is `Tag::Var` (per blind-spots.json), §03 has a poly-lambda gap that §08.1.5 confirms or refutes; if confirmed, §08.1.5 produces the typeck-side fix that §03 inherits."
  - "Matrix: poly-lambda × import context × generic callsite × `Tag::Scheme PROPAGATE_MASK` regression — the §08.2 grid covers all cells AND adds a §07/BUG-04-085 regression pin (`types.md §TF-3` propagation interaction with §02.0's flag-propagation fix)."
inspired_by:
  - "Rust rustc_codegen_ssa — handles poly-fn types and mono separately with careful Pool scoping (uses `MonoItem::Fn` with full `Instance(def-id, substs)` to isolate each mono copy's type environment)"
  - "Swift SIL Mono — monomorphizes polymorphic closures via dedicated substitution passes that isolate BoundVar from the mono context"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
review_pipeline:
  stage: editor-done
  next_step: 6
  updated: 2026-04-18
sections:
  - id: "08.1"
    title: "Investigation and root cause analysis"
    status: not-started
  - id: "08.1.5"
    title: "Verify typeck PC-2 invariant on poly-lambda code paths (must precede §08.3)"
    status: not-started
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

**Reviewer-surfaced reconnaissance** (per `/tmp/review-plan-ori_lang-SbDhu0MS/blind-spots.json` — codex HIGH trust + gemini LOWER trust convergence; verified manually):

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

- [ ] **Reproduce the failure cleanly**: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` → capture the `Idx(241)` unresolved error and the 17 LCFail list.
- [ ] **Reduce the repro WITHOUT relying on `#skip`**: produce a 5–10 line `.ori` source that contains (a) one polymorphic lambda definition and (b) one call to `assert_eq` (or an inlined imported generic), and fails the same way. Per `/tmp/review-plan-ori_lang-SbDhu0MS/blind-spots.json`: **`#skip("BUG-04-042")` does NOT prevent body compilation** — `tests/spec/expressions/lambda_mono.ori:137-141` documents that skipped tests still compile their function bodies. Use ONE of these isolation strategies instead:
  - **Preferred**: place the repro under a non-auto-discovered path (e.g., `compiler/ori_llvm/tests/aot/repros/poly_lambda_mono.ori` or a temp `/tmp/` file referenced explicitly in a Rust unit test) so `ori test tests/` does not pick it up.
  - **Alternative**: gate the repro file with `#cfg(feature: "bug_04_042_repro")` so it only enters the corpus when the feature is explicitly enabled — `cargo run --bin ori -- test --backend=llvm --feature bug_04_042_repro <file>`.
  - **Most surgical**: write the repro as a Rust unit test in `compiler/ori_llvm/tests/aot/poly_lambda_mono.rs` that drives the compiler programmatically with the failing source string inline — bypasses `ori test` discovery entirely and gives the cleanest failure mode for bisection.
- [ ] **Trace the failing mono site**: enable `ORI_LOG=ori_llvm=trace,ori_types=debug,ori_arc=debug` on the repro; find the point where `Idx(241)` is looked up and fails; log the `Idx` at every monomorphization request AND at every `body_type_map` build site (both local at `monomorphization.rs:94-107` and imported at `llvm_backend.rs:317-355`).
- [ ] **Bisect the origin**: classify `Idx(241)` as ONE of:
  - (a) a poly-lambda's `Tag::BoundVar` that leaked into mono scope (Hypothesis 1)
  - (b) a `Tag::Scheme` body var that should have been substituted before mono compile (Hypothesis 1+3 interaction)
  - (c) a fresh `Tag::Var(Unbound)` instantiation var that was never linked (Hypothesis 3)
  - (d) a `Tag::Var(Generalized)` from typeck that bypassed `validate_body_types` because the validator exempts `VarState::Generalized` (per `types.md §SC-1` shipped divergence) — implies §03 PC-2 enforcement has a poly-lambda gap (Hypothesis 4 — feeds §08.1.5)
  - (e) a poisoned cache entry in `TypeInfoStore` produced by a *prior* function's mono context that shares the dedup'd `Idx` (Hypothesis 4 — pure codegen-side cache-poisoning)
- [ ] **Inspect `TypeInfoStore` cache state at the failure point**: dump `TypeInfoStore` contents (consider adding `ORI_LOG=ori_llvm::codegen::type_info=trace` if not already wired) just before the `Idx(241)` lookup; identify whether the cache contains `TypeInfo::Error` from a prior context vs. a genuinely missing entry. If the entry was poisoned by a prior context, the fix is context-scoped caching (architectural risk #3 in blind-spots.json) — `TypeInfoStore` becomes a per-mono cache or invalidates on mono context exit.
- [ ] **Inspect the nounwind analyze pass**: `compiler/ori_llvm/src/codegen/function_compiler/nounwind/analyze.rs` is the primary `is_callee_intercepted → TypeInfoStore::get()` trigger per blind-spots cross-cutting concern #5 — name it as in-scope investigation territory and verify whether its `Idx` lookup pattern is the cache-poisoning trigger.
- [ ] **Document the root cause** in a new §08.1.R subsection (analogous to §03.R) with the exact file:line where the bleed occurs, AND classify which of (a)–(e) above is the actual mechanism. If the answer is (d), §08.1.5 work expands to absorb §03's poly-lambda gap; if (e), §08.3 owns the `TypeInfoStore` context-scoping fix.

## 08.1.5 Verify typeck PC-2 invariant on poly-lambda code paths (must precede §08.3)

**Goal:** Before assuming the bleed is purely codegen-side, verify that typeck's PC-2 contract (`typeck.md §PC-2` — no `Tag::Var` in body `expr_types` after `validate_body_types`) actually holds on polymorphic-lambda code paths. If `Idx(241)` is `Tag::Var(Unbound)` rather than `Tag::BoundVar`, **typeck is leaking** and §03's clean-producer assumption (per overview §Design Principles 1) is violated for poly-lambda bodies.

**Why this gate is mandatory**: per `/tmp/review-plan-ori_lang-SbDhu0MS/blind-spots.json`, both reviewers independently flagged that `Idx(241)` is most likely `Tag::Var` (inference-time), not `Tag::BoundVar` (post-generalization). `is_polymorphic_lambda()` at `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` only checks `BoundVar`/`Scheme` on return types — lambdas whose return stays `Tag::Var(Generalized)` bypass mono handling entirely. The §03 end-of-body defaulting pre-pass (`InferEngine::default_unbound_vars_from_empty_literals`, `compiler/ori_types/src/infer/mod.rs:733`) only walks empty-literal expression roots; polymorphic-lambda return positions are NOT in its scope. If the producer is leaking, fixing only the consumer (codegen) is INVERTED-TDD per CLAUDE.md (the deliverable is the producer-side enforcement).

- [ ] **Audit `default_unbound_vars_from_empty_literals` scope** (`compiler/ori_types/src/infer/mod.rs:733`): confirm whether it walks polymorphic-lambda return-type positions or only empty-collection-literal allocation sites. If only the latter, document the gap — poly-lambda `Tag::Var(Unbound)` return types fall through to `validate_body_types`, which then exempts `VarState::Generalized` per `types.md §SC-1` shipped divergence.
- [ ] **Audit `validate_body_types` exemption** (`compiler/ori_types/src/check/validators/mod.rs::collect_first_unbound_var`): confirm the `VarState::Generalized` exemption arm. The validator's gate order is `resolve_fully → HAS_ERROR → HAS_VAR`; a `Tag::Var(Generalized)` survives the HAS_VAR check (because generalized vars carry `Tag::Var` per shipped pool, not `Tag::BoundVar`) and is exempted before `E2005` fires. Confirm whether this exemption is too broad for poly-lambda returns.
- [ ] **Decide ownership** — based on §08.1's classification (a)/(b)/(c)/(d)/(e):
  - If (d) — typeck IS leaking poly-lambda return types: this section absorbs the typeck fix. Add a §08.1.5.fix subitem extending `default_unbound_vars_from_empty_literals` (or adding a sibling pass `default_unbound_vars_from_polylambda_returns`) to walk polymorphic-lambda body-exit positions and either default unconstrained vars to `Idx::NEVER` or emit `E2005` per `typeck.md §PC-2`. Coordinate with §03 by adding a `<!-- cross-section:08.1.5 → 03 -->` note to §03's frontmatter.
  - If NOT (d) — typeck is clean: document the verification (`Tag::BoundVar`/`Tag::Var(Generalized)` correctly produced; codegen mishandles) and proceed to §08.2 with the codegen-only scope.
- [ ] **Add a regression test** in `compiler/ori_types/src/check/validators/tests.rs` that compiles a polymorphic lambda definition and asserts: (a) the lambda's return-type position carries either `Tag::BoundVar` or `Tag::Var(Generalized)`, (b) NO `Tag::Var(Unbound)` survives in the body's `expr_types` map. This is the producer-side semantic pin per `tests.md §Matrix Clamping`.

**Decision gate**: §08.1.5 MUST close before §08.3 starts. The §08.3 fix shape depends on which producer is leaking — fixing the wrong producer leaves the symptom intact and burns the fix budget.

## 08.2 TDD matrix: poly-lambda + imported generics + Scheme PROPAGATE_MASK pin

**Goal:** Write failing tests BEFORE implementing the fix.

- [ ] **Spec test (TDD)**: `tests/spec/expressions/poly_lambda_with_imported_generic.ori` — a file that defines a polymorphic lambda AND calls `assert_eq<int>` at least three times with different monomorphic types. (NOTE: this file is referenced by the audit as DEAD_PATH because it doesn't exist yet; it will be CREATED here as a TDD forward-reference. The audit's flag is correct in the literal sense but does not represent a defect — file creation IS this checklist item.)
- [ ] **Rust unit test in `ori_llvm`**: a direct LLVM codegen test that monomorphizes `assert_eq<T>` in the presence of a pre-existing poly-lambda registration in the type_info store.
- [ ] **AOT integration test in `compiler/ori_llvm/tests/aot/`** (per parent plan overview line 35 — required mission deliverable): a Rust integration test that drives the full AOT pipeline (`cargo run --bin ori -- build`) on a `.ori` source containing both a polymorphic lambda definition AND `assert_eq<T>` calls from `std.testing`, then runs the resulting binary and verifies exit code 0. This complements the spec test (which exercises the JIT path via `ori test`) by exercising the linked-binary path — both must pass for §08 to satisfy the parent plan's pillar-5 verification contract.
- [ ] **Matrix cells**:
  - **Type dimension**: `int`, `str`, `bool`, `float` — four mono instantiations of `assert_eq<T>` in the same file
  - **Lambda dimension**:
    - (a) poly-lambda defined but unused
    - (b) poly-lambda defined and called monomorphically
    - (c) poly-lambda defined and called with different types at different sites
    - (d) **NEW (per blind-spots.json)** — poly-lambda whose return type stays `Tag::Var(Generalized)` (e.g., `let f = x -> x` used in a context that doesn't constrain the return), exercising the `is_polymorphic_lambda()` bypass at `lambda_mono/type_resolve.rs:55-73`
  - **Import dimension**: (a) `std.testing.assert_eq` (the actual failure case), (b) locally-defined generic that mimics the same shape
  - **Tag::Scheme `PROPAGATE_MASK` regression pin (NEW — coordinates with §07/BUG-04-085)**: a cell that exercises a polymorphic lambda whose body contains nested generics (`List<T>` where `T` is the lambda's bound var), so that §02.0's `Tag::Scheme HAS_VAR` propagation fix (`compiler/ori_types/src/pool/mod.rs:651-660`, per overview §Implementation Sequence Phase 7 / BUG-04-085) is exercised in the poly-lambda + imported-generic context. This pin guards against §08 silently undoing §07's fix or vice versa — landing one without the other risks reopening BUG-04-085's "ArcIrEmitter: variable not yet defined" symptom in a different shape per blind-spots.json architectural-risk #4.
  - **`prepare_mono_cached` cache-miss fallback negative pin (NEW)**: a cell that exercises the cache-miss path at `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:119-139` where `prepare_mono_cached()` falls back to `canon.root_for(mono_fn.original_name).unwrap_or(canon.root)` — verifies the host-module fallback continues to produce correct output when the mono cache misses. Without this pin, §08.3 changes to the mono pipeline could break the fallback path silently. (NOTE: the cache-miss path is reached via metadata stripping at `compiler/oric/src/test/runner/llvm_backend.rs:448-450`, so the test must construct a scenario where the imported metadata is unavailable.)
  - **Nested-container substitution pin (NEW)**: a cell exercising `apply_bound_var_map` at `lambda_mono/type_resolve.rs:142` with a polymorphic lambda whose parameter type is `List<T>` (nested generic). Per blind-spots.json, the function only fixes top-level vars, so nested-container substitution is a known gap that may contribute to the 17 LCFails figure.
- [ ] **Negative pin**: confirm that reverting the §08.3 fix causes the tests to fail again (prevents silent regression).
- [ ] **Verify all tests fail** before starting §08.3 implementation (TDD discipline per `tests.md §TDD for Bugs`).

## 08.3 Implementation: fix BoundVar bleed at identified call sites

**Goal:** Fix the root cause identified in §08.1. Scope depends on §08.1 findings AND §08.1.5 ownership decision — the TDD matrix in §08.2 pins the correct behavior; the fix must make those tests pass without breaking any existing test.

- [ ] **Fix the identified call site(s)** per §08.1 root cause + §08.1.5 ownership. Candidate fix shapes (pick one or more based on investigation):
  - If Pool contamination (Hypothesis 1): scope the poly-lambda registration so its `BoundVar`s don't leak across monomorphization boundaries.
  - If type_info store leak (Hypothesis 2): tag the store entries with their originating monomorphization context so the mono pipeline doesn't read poly-lambda entries when resolving imported generics.
  - If sequencing (Hypothesis 3): reorder the mono pipeline so imported generics are fully resolved before poly-lambda body compilation proceeds.
  - If `body_type_map` / `TypeInfoStore` cache poisoning (Hypothesis 4 — added 2026-04-18): make `TypeInfoStore` context-scoped (per-mono cache) or add per-mono invalidation hooks; alternatively, change `is_callee_intercepted → TypeInfoStore::get()` to use a context-tagged lookup that doesn't share `TypeInfo::Error` entries across mono contexts. **Refactoring the underlying file (`compiler/ori_llvm/src/codegen/type_info/store.rs`) for size or organization is OUT OF SCOPE for this section** — see note below.
  - If typeck producer leak (Hypothesis (d) per §08.1.5): the fix lives in §08.1.5's fix subitem (typeck-side) AND a debug_assert at codegen entry per §08.6.
- [ ] **Remove `fallback_bound_vars_to_int` or replace with a hard codegen error** (`lambda_mono/type_resolve.rs:392-408`): per blind-spots.json, this fallback "silently converts unresolved-type bugs into ABI/RC bugs". Per CLAUDE.md §The One Rule and §INVERTED-TDD-IS-BANNED, the ONLY acceptable resolutions are: (a) delete the fallback entirely so unresolved `BoundVar`s surface as a codegen failure rather than silent ABI drift, OR (b) replace it with an `E5xxx` codegen-range diagnostic emitted via `self.builder.record_codegen_error()` that fires in BOTH debug AND release. A `#[cfg(debug_assertions)]`-gated hard panic is explicitly forbidden — release builds would still silently convert unresolved types, which is the exact failure mode the fallback was introducing in the first place. Pick (b) when the codegen path has a downstream recovery route; pick (a) when the recovery would never complete soundly anyway.
- [ ] **Verify `apply_bound_var_map` covers nested-container substitution** (`lambda_mono/type_resolve.rs:142-175`): per blind-spots.json, the function only handles top-level vars. If §08.2's nested-container pin fails, extend the function to recurse into container types via `Pool::visit_children` (per `types.md §TF-3`), or document that the caller is responsible for pre-substituting nested generics.
- [ ] **Run `timeout 150 cargo test -p ori_llvm`** — no regressions.
- [ ] **Run `timeout 150 cargo st`** — interpreter parity preserved.
- [ ] **Run `timeout 150 ./target/release/ori test --backend=llvm tests/`** — LLVM backend passes on the §08.2 test corpus.
- [ ] **Remove the §08.1 isolated repro** from its non-auto-discovered location (or remove the `#cfg(feature: "bug_04_042_repro")` gate) and confirm the repro passes via both backends as part of the standard corpus.

**Note on `TypeInfoStore` size (audit BLOAT_RISK)**: `compiler/ori_llvm/src/codegen/type_info/store.rs` is 388 lines (audit minor finding). Per blind-spots.json, this file is the SURFACE point where `Idx(241)` shows up, NOT the root cause. The BLOAT_RISK is owned by a separate bug-tracker artifact whose lifecycle is independent from §08: file `/add-bug` titled `"BLOAT: ori_llvm::codegen::type_info::store.rs at 388 lines (approaching 500-line limit) — split into submodules"` with subsystem `ori-codegen`, severity `low`. Filing IS the concrete ownership transfer per CLAUDE.md §"Future Improvement" MUST Be Concretely Tracked — the bug entry is the tracking artifact, and `/review-bugs` sweeps it on its own cadence. The §08.3 fix MAY add lines to this file; if the file crosses 500 lines as a result of §08.3, the bug is escalated to `medium` and the split happens inline before §08.5 closes (owned by §08.3 at that point, not a separate cleanup).

## 08.4 Coordination with roadmap Section 21A — claim 21.7/21.11/21.12 corrections

**Goal:** Make §08's territorial overlap with roadmap §21A explicit so a future §21A resumption does not silently overwrite this section's fix.

**Why explicit claim is required**: per `/tmp/review-plan-ori_lang-SbDhu0MS/blind-spots.json` architectural-risk #5 — `plans/roadmap/section-21A-llvm.md:104-107` claims "Generic monomorphization: IMPLEMENTED (verified 2026-03-29). 33 generics tests pass including cross-module `assert_eq` instantiation." This is **contradicted by the current commit-wall**: the LLVM backend spec run CRASHES on `assert_eq<T>` monomorphization for poly-lambda hosts. §08 is effectively reopening roadmap subsections 21.7 (Function Sequences & Expressions), 21.11 (Lambda & Closure Support), and 21.12 (the next adjacent codegen subsection) without claiming that scope. A later §21A resumption could see §21A's "verified 2026-03-29" markers and overwrite §08's fix as "already done".

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
- [ ] **Dual-execution parity audit on poly-lambda paths beyond §08.2** (per blind-spots.json cross-cutting concern #3): explicitly run dual-exec-verify on existing `tests/spec/expressions/lambda_mono.ori` and any `tests/spec/traits/` poly-lambda sites to claim parity responsibility for the broader poly-lambda surface, not just the new corpus. Without this audit, §08 leaves an "orphaned parity claim" — fixed in the new tests, untested in pre-existing tests.

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
- [ ] `diagnostics/dual-exec-verify.sh` clean on §08.2 corpus AND on pre-existing poly-lambda tests (per §08.5 broadened parity audit)
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
