---
section: "08"
title: "Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042)"
status: in-progress
reviewed: true
goal: >
  Fix the cross-module pool-merge var_id collision (corrected diagnosis 2026-04-19;
  see §08.1.R) at `compiler/oric/src/test/runner/llvm_backend.rs:320-360` by lifting
  a remap-aware re-intern abstraction into `compiler/ori_types/src/pool/re_intern/`
  so that imported-module types entering the test runner's merged pool get fresh,
  non-aliasing var_ids, allowing monomorphized imported generics (assert_eq<T: Eq +
  Debug>) to compile regardless of whether the host module contains polymorphic
  lambda definitions. Currently blocks `test-all.sh` on the Ori spec (LLVM backend)
  run, which transitively blocks atomic commits for every other section of this
  plan. The prior "poly-lambda BoundVar bleed" framing was a surface-symptom
  description; the real defect is var-id aliasing in the test-runner's pool merge.
success_criteria:
  - "`timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` passes with zero LCFails (previously: `Idx(241)` unresolved type variable, 17 LCFails)."
  - "`assert_eq<int>` / `assert_eq<str>` / `assert_eq<bool>` monomorphize cleanly in any spec test file that also defines polymorphic lambdas. Verified by the existing `tests/spec/types/integer_safety.ori` + `tests/spec/expressions/lambda_mono.ori` pair continuing to compile, plus new coverage adding a file with BOTH features interleaved."
  - "`timeout 150 ./test-all.sh` reports no `Ori spec (LLVM backend) CRASHED` line; the LLVM backend spec run passes at parity with the interpreter (or carries concrete `#skip` annotations for any remaining skips, each pointing to a separate non-blocker bug)."
  - "LLVM IR verification (`ORI_VERIFY_ARC=1`) passes for every monomorphized `assert_eq` site."
  - "No regression in `tests/spec/expressions/lambda_mono.ori` (currently passes via interpreter — must continue to pass via LLVM)."
  - "Cross-module pool-merge preserves var_id disjointness between the host test-file pool and every imported module's pool: after §08.3's remap-aware re-intern, every imported `Tag::Var` / `Tag::BoundVar` / `Tag::RigidVar` that lands in the merged pool has a freshly-allocated var_id (via `merged_pool.next_var_id`) distinct from every var_id the host test file already holds, so no imported type can read a host-file-originated `VarState::Generalized` slot out of `merged_pool.var_states`. Verified by the §08.2 matrix cells for leaf var remap / scheme binder remap / `scheme_var_ids` remap / VarState clone-vs-blank-init (each with positive + negative pins)."
  - "`FunctionSig.scheme_var_ids` and `Tag::Scheme` binder lists on re-interned imported signatures are coherent with the remapped leaf `Tag::Var` ids — specifically, the `var_subst` map built at `compiler/oric/src/test/runner/llvm_backend.rs:321-328` from `generic_sig.scheme_var_ids` resolves every leaf `Tag::Var` encountered by `substitute_in_pool` at `compiler/ori_types/src/pool/substitute/mod.rs` (no stranded leaves left unmapped because the sig side's ids drifted from the re-interned type side's ids)."
  - "Matrix: poly-lambda × import context × generic callsite × remap-aware re-intern correctness — the §08.2 grid covers all cells (leaf var remap, scheme binder remap, scheme_var_ids coherence, VarState clone-vs-blank-init) AND retains the pre-existing `Tag::Scheme PROPAGATE_MASK` regression pin for BUG-04-085 cross-coverage (per `types.md §TF-3` propagation)."
inspired_by:
  - "Rust rustc_codegen_ssa — handles poly-fn types and mono separately with careful Pool scoping (uses `MonoItem::Fn` with full `Instance(def-id, substs)` to isolate each mono copy's type environment)"
  - "Swift SIL Mono — monomorphizes polymorphic closures via dedicated substitution passes that isolate BoundVar from the mono context"
depends_on: ["03"]
third_party_review:
  status: resolved
  updated: 2026-04-18
  notes: "user-accepted at iter_cap_reached after 3 rounds; 10 substantive findings fixed inline across commits bbc8e15d, 77af4126, e11972b0; 2 remaining findings are meta (duplicates of TPR-04-R0-001 and TPR-04-R0-002 already filed at §04.R.TPR); option key: accept-with-findings"
review_pipeline:
  stage: editor-done
  next_step: 6
  updated: 2026-04-19
sections:
  - id: "08.1"
    title: "Investigation and root cause analysis"
    status: complete
  - id: "08.1.5"
    title: "Decide fix shape for the cross-module pool-merge var_id collision (must precede §08.3)"
    status: not-started
  - id: "08.2"
    title: "TDD matrix: poly-lambda + imported generics + pool-merge remap pins"
    status: in-progress

  - id: "08.3"
    title: "Implementation: remap-aware re-intern for cross-module pool merge"
    status: not-started
  - id: "08.4"
    title: "Coordination with roadmap Section 21A — claim 21.7/21.11/21.12 corrections"
    status: not-started
  - id: "08.5"
    title: "Verification: LLVM backend spec run green"
    status: not-started
  - id: "08.6"
    title: "§04 ↔ §08 seam coordination — confirm no seam-order change under corrected fix"
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

## Root-Cause Hypothesis (historical; superseded 2026-04-19 — see §08.1.R)

"Polymorphic lambda `BoundVar` types in the shared Pool interfere with `MonoInstance` body compilation for imported generics. Fix spans Pool scoping, type_info store, function compiler, and lambda_mono." — `plans/bug-tracker/section-04-codegen-llvm.md:459`

**Status**: this hypothesis list and the 2026-04-18 classification that flowed from it are SUPERSEDED. See §08.1.R HISTORY block for the corrected diagnosis (cross-module pool-merge var_id collision at `compiler/oric/src/test/runner/llvm_backend.rs:320-360`). The candidates below are retained as historical context for readers tracing the investigation arc; they are NOT the active root cause.

Historical candidate root causes that §08.1 investigated:

1. **Pool contamination** (hypothesis 1): polymorphic lambda registrations leave `BoundVar` residue in the shared Pool. **Status**: CLOSE, but not quite — the collision is in the TEST-RUNNER's `merged_pool`, not the module-level pool produced by typeck. The real bug is adjacent: imported types re-interned into the merged pool carry unchanged source var_ids that alias host var_states slots (§08.1.R evidence item 1).
2. **type_info store leak** (hypothesis 2): `Idx(241)` observed at `TypeInfoStore`. **Status**: SURFACE SYMPTOM — `TypeInfoStore` is where the collision manifests as an observable error, not where it originates. Fixing the store was ruled out during 2026-04-18 investigation.
3. **function_compiler/lambda_mono sequencing** (hypothesis 3): order of poly-lambda body vs imported-generic mono compilation. **Status**: REFUTED — codex Step 4 architectural_risks confirmed production codegen uses a single pool (no merge step); sequencing is not the issue.
4. **`body_type_map` / `arc_cache` cache poisoning** (hypothesis 4): shared cache state across mono contexts. **Status**: REFUTED — `TypeInfoStore` is per-codegen-context (documented at `compiler/ori_llvm/src/codegen/type_info/store.rs:37-65`); cross-context poisoning via this cache is not a candidate mechanism.
5. **Typeck producer leak** (2026-04-18 classification, Hypothesis (d)): `Tag::Var(VarState::Generalized)` escaping typeck's PC-2 boundary. **Status**: REFUTED (2026-04-19) — the vars ARE correctly `VarState::Generalized` post-typeck; the leak is not at typeck's boundary but at the test-runner's pool-merge boundary downstream. Both broad and narrowed sibling-pass implementations were tried and reverted at HEAD=3dd4ded6 because typeck.md §GN-3 + `build_exempt_var_ids` exempt every Generalized var from defaulting. See §08.1.R HISTORY block.

**Active root cause (2026-04-19)**: cross-module pool-merge var_id collision. See §08.1.R single-sentence root cause and evidence chain.

**Architectural risk note (retained, still accurate under corrected diagnosis)**: codegen has its own type-instantiation phase that runs AFTER typeck PC-2 — `ori_arc::lower::calls::lambda` applies `type_subst` during ARC lowering, and `lambda_mono/mod.rs` mutates ARC IR. These passes are CONSUMERS of the pool's type facts, not producers; they cannot be the root cause of the collision. Under §08.3's fix, they receive clean post-remap types and operate correctly without modification. The "parallel emission path" concern in the prior plan-text resolves to "no additional producer exists — the pool-merge is the single upstream producer of the corrupted state".

## 08.1 Investigation and root cause analysis

**Goal:** Produce a single sentence naming the root cause and the file + line(s) where it originates. Investigation MUST consider all hypothesis candidates (per the expanded list above).

**Classification outcome (2026-04-19, SUPERSEDES the 2026-04-18 classification):** The original 2026-04-18 classification (Hypothesis **(d)** — `Tag::Var(VarState::Generalized)` leaking from typeck) was **WRONG**. Both broad and narrowed sibling-pass implementations of "default unbound vars from poly-lambda returns" were tried and reverted at HEAD=3dd4ded6 because typeck.md §GN-3 Value Restriction converts unconstrained vars to `VarState::Generalized` during body inference, and the end-of-body defaulting pass's exemption set (built by `build_exempt_var_ids` at `compiler/ori_types/src/check/validators/mod.rs`, cited by `typeck.md §PC-2` "End-of-body defaulting pre-pass") is scope-by-var and includes every `VarState::Generalized` var — the sibling pass can never substitute them. The actual root cause is upstream of typeck's hand-off entirely: a **cross-module pool-merge var_id collision** in the test runner at `compiler/oric/src/test/runner/llvm_backend.rs:320-360` that corrupts the merged pool's `var_states` indexing, so a downstream `Tag::Var` read that LOOKS like "typeck leaked" is actually reading a host-file-originated `VarState::Generalized` slot through an imported `var_id` that was never shifted. The formal decision between remap-aware re-intern vs alternative fix shapes is §08.1.5's responsibility (§08.1.5 is the decision gate). See §08.1.R HISTORY block for the full diagnosis correction.

- [x] **Reproduce the failure cleanly**: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` → captured `Idx(241)` unresolved at `ori_llvm::codegen::type_info::store` + 17 LCFails (run 2026-04-18, 95.54ms). Test harness reports exit "OK" despite the 17 failures because the outer test summary swallows per-file compile errors — the `Ori spec (LLVM backend) CRASHED` signal only fires on the full-suite run via `./test-all.sh`.
- [~] **Reduce the repro WITHOUT relying on `#skip`**: DEFERRED to §08.2 TDD matrix (a minimal Rust unit test in `compiler/ori_llvm/tests/aot/poly_lambda_mono.rs` is the cleanest option per the original checkbox). Static classification (Hypothesis (d)) does not require a reduced repro; the TDD matrix in §08.2 will produce the minimal failing case as part of normal TDD discipline (failing test first, then fix).
- [~] **Trace the failing mono site**: NOT RUN — runtime trace attempt was denied. Static replacement: `resolve_fully` at `compiler/ori_types/src/pool/accessors.rs:434-437` only follows `VarState::Link`; for `VarState::Generalized` it `break`s immediately, leaving `current` as the input `Tag::Var`. The comment at `accessors.rs:429-432` literally documents the failure mode: *"This can happen when Generalized type vars leak from type checking into codegen without proper resolution."* The `Tag::Var` arm at `ori_llvm/src/codegen/type_info/store.rs:341-364` is the only error path that emits "unresolved type variable at codegen" — the `Tag::BoundVar | RigidVar | Scheme | ...` arm at `:371-385` emits "unreachable type tag at codegen" instead, so the observed message pins the Tag to `Var`.
- [x] **Bisect the origin**: classified as **(d)** — `Tag::Var(VarState::Generalized)` from typeck that bypassed `validate_body_types` because `collect_first_unbound_var` exempts `VarState::Generalized` (per `types.md §SC-1` shipped divergence). Evidence chain documented in §08.1.R below. This activates §08.1.5 as the producer-side fix gate.
- [~] **Inspect `TypeInfoStore` cache state at the failure point**: NOT RUN — runtime inspection denied. Static replacement: the store's `Tag::Var` arm calls `self.pool.resolve_fully(idx)` FIRST (line 342); a cache hit is impossible because `get_impl` is the point where the miss triggers the error. Hypothesis (e) (poisoned cache) is refuted for the single-file case — a single `.ori` file with one `assert_eq<int>` mono target cannot produce a cross-context poisoned entry within `TypeInfoStore` because `TypeInfoStore` is single-threaded per codegen context (per the Reviewer-surfaced reconnaissance block's scope correction). If (e) were active, we would expect the error to fire only AFTER certain preceding function emissions — the current repro fires regardless of emission order, consistent with (d) and inconsistent with (e).
- [~] **Inspect the nounwind analyze pass**: NOT RUN — static review suffices. `nounwind/analyze.rs` consumes `TypeInfoStore::get()` for arc-IR types; it reports the same `TypeInfo::Error` the Tag::Var arm produces. The nounwind pass is a *consumer* of the leak, not a producer — routing Tag::Var(Generalized) through nounwind vs through arc_emitter hits the same `get_impl` error path. No cache-poisoning signal found at nounwind layer.
- [x] **Document the root cause** in §08.1.R below. Hypothesis (d) confirmed by static analysis; §08.1.5 will formally pin the producer-side fix via a regression test in `compiler/ori_types/src/check/validators/tests.rs`.

### 08.1.R Root-cause documentation (2026-04-19, corrected)

**Classification:** **Cross-module pool-merge var_id collision at the test-runner boundary.** The bug lives upstream of every hypothesis the original 2026-04-18 investigation considered — it is injected in the test-runner's pool-merge step, BEFORE the resulting merged pool is handed to codegen.

**Single-sentence root cause:** In `compiler/oric/src/test/runner/llvm_backend.rs:320-360` the test runner merges imported-module types into a per-test-file `merged_pool` whose `var_states` vector was cloned from the test-file pool (NOT from the imported pools); the re-interning path at `compiler/ori_types/src/pool/re_intern/mod.rs:192-193` (`Tag::Var | Tag::BoundVar | Tag::RigidVar => target.intern(tag, source.data(idx))`) preserves imported var_ids unchanged into the target, and the widening call `merged_pool.ensure_var_capacity(max_id + 1)` at `llvm_backend.rs:341-343` only APPENDS fresh `Unbound` slots — it never SHIFTS the imported var_ids — so an imported `Tag::Var(var_id = N)` reads slot `N` of `merged_pool.var_states`, which may hold a test-file poly-lambda's `VarState::Generalized` state from an unrelated local binder; `substitute_in_pool` (`compiler/ori_types/src/pool/substitute/mod.rs:82-88`) then branches on that corrupted `VarState::Generalized` and emits a substitution that looks like "a generalized var leaked from typeck" but is in fact a var-id aliasing artifact of the pool merge.

**Evidence chain** (every link verified against source at HEAD=3dd4ded6):

1. **Collision site:** `compiler/oric/src/test/runner/llvm_backend.rs:334-343` — `merged_pool.ensure_var_capacity(max_id + 1)` after copying imported types' var_ids unchanged. The inline comment at `:329-333` documents: *"Re-interned Vars carry source var_ids, but the merged pool's var_states array was cloned from the test file's pool and may not cover imported var_ids. substitute_in_pool follows links via var_state(), which panics on out-of-bounds var_ids."* — the widening call handles the crash, but NOT the semantic collision.

2. **Re-intern preserves source var_ids unchanged:** `compiler/ori_types/src/pool/re_intern/mod.rs:192-193` — `Tag::Var | Tag::BoundVar | Tag::RigidVar => target.intern(tag, source.data(idx))`. The comment above reads "Type variables: data = var_id (pool-independent)" — that comment is WRONG for merged pools, because `var_states` is indexed by `var_id` and is pool-specific.

3. **Re-intern hash fast path is invalid for var-bearing subtrees:** `compiler/ori_types/src/pool/re_intern/mod.rs:56-60` — if `source.hash(idx)` collides with an existing target entry, it returns the target entry as-is. Leaf `Tag::Var` hashes include the raw `var_id` (`pool/mod.rs:395-413`), so two imported vars that coincidentally share a Merkle hash with an unrelated local var deduplicate into the local var's slot — another channel for the same collision.

4. **var_subst build reads the pre-remap scheme_var_ids:** `llvm_backend.rs:321-327` — `var_subst` is keyed by `generic_sig.scheme_var_ids`, which `re_intern_sig` at `pool/re_intern/mod.rs:83-97` CLONES unchanged (never re-numbers). If §08.3 remaps imported leaf var_ids in the type tree but does NOT remap `scheme_var_ids` in the sig, the substitution map stops matching and the fix silently regresses.

5. **Tag::Scheme extra payload stores raw binder var_ids:** `compiler/ori_types/src/pool/construct/mod.rs:161-174` writes the binder var_id list into `extra` at intern time; `compiler/ori_types/src/pool/re_intern/mod.rs:185-193` preserves them unchanged via `source.scheme_vars(idx).to_vec()` + `target.scheme(&vars, body)`. Binders must remap together with leaves, not independently.

6. **VarState::Generalized branch reads the collided slot:** `compiler/ori_types/src/unify/substitute.rs:78-83` and `compiler/ori_types/src/pool/substitute/mod.rs:82-88` both branch on `VarState::Generalized`. A remap that allocates fresh var_ids but blanks them to `Unbound` (instead of cloning the source's VarState) destroys the semantic state and distorts downstream generic behavior — the source's `VarState::Generalized` must be cloned into the new destination id.

7. **`resolutions` map is NOT in the blast radius:** `compiler/ori_types/src/pool/accessors.rs:358-359` — `resolutions: FxHashMap<Idx, Idx>`. Keys are `Idx`, NOT `var_id`. The fix does not touch this map; reviewers and implementers MUST NOT list it as a touch point or they will chase phantom work.

8. **Production codegen does NOT cross-pool-merge:** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:115-165` and `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:95-149` operate on a single pool. The collision surface is specifically the test-runner's merge step — the production AOT path does not share this defect. §08.3's fix site (test runner + `pool/re_intern`) is thus correct even though the USE site is test-only; the abstraction (§08.3 lifts remap logic into `pool/re_intern/`) is what keeps production codegen free of the defect class if the merge pattern ever ships there.

**Fix ownership:** Upstream of the typeck-output boundary — specifically, at the test-runner's pool merge. Fixing codegen to tolerate the corrupted `VarState::Generalized` read would be inverted TDD (the pool's append-only + var_id-pool-local invariants are the deliverable; weakening them on the failing path is banned per `CLAUDE.md §INVERTED-TDD`). Fixing typeck to re-default poly-lambda returns would fix a symptom the re-intern step is actually creating out of thin air. The correct fix is remap-aware re-intern — §08.3 owns it; §08.1.5 is the decision gate between remap-aware re-intern and any alternative (e.g., a per-module-scoped merged_pool that never combines var_id spaces at all — rejected below).

**Why the sibling-pass fix was tried and reverted at HEAD=3dd4ded6** (historical note, load-bearing for future readers):

- A broad `default_unbound_vars_from_polylambda_returns` pass walking every `ExprKind::Lambda` at end-of-body was implemented AND narrowed to just unbound-without-constraint returns. Both variants were reverted because `typeck.md §GN-3` generalizes any unconstrained var in a lambda position during body inference BEFORE the end-of-body defaulting pass runs; by the time defaulting fires, the vars are already `VarState::Generalized`, and `build_exempt_var_ids` puts every `VarState::Generalized` var in the exempt set via its `FunctionSig.scheme_var_ids` path. The sibling pass therefore had nothing to substitute. No amount of narrowing or broadening recovered — the time order (§GN-3 runs first, defaulting runs second, validator runs third) is the architectural reason, not a tuning knob. Codex's Step 4 blind-spots round (2026-04-19) independently flagged the same order-of-operations problem in its review.

**HISTORY block — 2026-04-19 diagnosis correction:**

- **2026-04-18** §08.1 classified Hypothesis (d) ("typeck leaks `Tag::Var(VarState::Generalized)`"); §08.1.5 was marked `complete` with option (ii) selected (sibling pass); §08.3 was framed around the typeck fix.
- **2026-04-19** `/tp-help` consensus (Gemini HIGH trust + codex Step 4 blind-spots) identified the cross-module pool-merge var_id collision at `llvm_backend.rs:320-360` as the real root cause. The sibling-pass approach was tried and reverted at HEAD=3dd4ded6; the revert is evidence that §08.1.5's prior "complete" state pointed at an approach that CANNOT work given typeck.md §GN-3 Value Restriction. Per `CLAUDE.md §Plan Corrections Go IN the Plan`, the prior diagnosis is rewritten here (not amended via memory), and §08.1.5's status is reset to `not-started` to reflect that the fix shape is now different and the decision has not yet been implemented.
- **Cross-reference:** memory pointer `/home/eric/.claude/projects/-home-eric-projects-ori-lang/memory/project_bug_04_042_pool_merge_diagnosis.md` (to be shrunk to a one-line pointer to this §08.1.R after the plan lands — per `CLAUDE.md §Plan Corrections Go IN the Plan`, the plan file is the authoritative record; the memory entry is a breadcrumb only).

## 08.1.5 Decide fix shape for the cross-module pool-merge var_id collision (must precede §08.3)

**Goal:** Under the corrected §08.1.R diagnosis (cross-module pool-merge var_id collision, not a typeck leak), confirm the fix shape for §08.3 BEFORE implementation starts. The decision is not "which producer is leaking" — §08.1.R pins the collision site at `compiler/oric/src/test/runner/llvm_backend.rs:320-360` and the re-intern path at `compiler/ori_types/src/pool/re_intern/mod.rs:185-193`. The decision IS where the remap abstraction lives (test-runner call site only, or lifted into `pool/re_intern/`) and exactly which of the pool's internal structures must be rewritten coherently so every Merkle-hash and substitution invariant still holds.

**Why §08.1.5 reset from complete to not-started (2026-04-19):** Per the §08.1.R HISTORY block, the prior 2026-04-18 decision ("option (ii) — sibling pass `default_unbound_vars_from_polylambda_returns`") was based on the stale Hypothesis (d) diagnosis. Both broad and narrowed variants of that sibling pass were implemented and then reverted at HEAD=3dd4ded6 because typeck.md §GN-3 Value Restriction + `build_exempt_var_ids` exempt every `VarState::Generalized` var — the sibling pass has nothing to substitute. The "complete" state that §08.1.5 carried before this editor round pointed at a tried-and-reverted approach; resetting to `not-started` is not a regression of prior work, it's a correction per `CLAUDE.md §Plan Corrections Go IN the Plan`. T17–T19 regression pins the prior round added to `compiler/ori_types/src/check/validators/tests.rs` remain legitimate typeck-boundary clamps (they pin that the validator is NOT the enforcement point for the pool-merge bug) but they are NOT the §08.3 fix itself — the fix is upstream of the typeck-output boundary.

**Why options (i), (ii), (iii) are all rejected under the corrected diagnosis:**

- **(i) extend `default_unbound_vars_from_empty_literals` to poly-lambda returns** — rejected. The vars are already `VarState::Generalized` by the time the defaulting pass runs (typeck.md §GN-3 runs first); the defaulting pass's exempt set covers them. Adds a hook with no effect on the failing path.
- **(ii) sibling pass `default_unbound_vars_from_polylambda_returns`** — rejected. Tried and reverted at HEAD=3dd4ded6 for the same reason as (i). Evidence is in the revert itself.
- **(iii) remove the `VarState::Generalized` exemption from `validate_body_types`** — rejected. Fires `E2005` on every polymorphic let-binding, breaks let-polymorphism entirely; `CLAUDE.md §INVERTED-TDD` flags this as the canonical widened-exemption anti-pattern. Also: the vars aren't actually unresolved — they're correctly generalized; the real bug is var_id collision downstream.

**The correct fix shape — remap-aware re-intern** (per codex's Step 4 blind-spots advice, 2026-04-19; confirmed as the sole approach compatible with pool invariants `types.md §TY-6` append-only and `types.md §TF-3` Merkle-hash propagation):

1. **Lift the remap logic into `compiler/ori_types/src/pool/re_intern/` as a reusable abstraction.** Do NOT bury it in `llvm_backend.rs:320-360` even though the call site is there — the abstraction belongs in the pool's re-intern module so any future cross-pool-merge call site (production codegen, WASM `ori_compiler` facade, other test harnesses) gets correct semantics by default. The `llvm_backend.rs:320-360` call site becomes the first consumer, not the owner.
2. Build a `src_var_id → dst_var_id` remap map during re-interning of imported types. For every imported `Tag::Var` / `Tag::BoundVar` / `Tag::RigidVar` / `Tag::Scheme` binder encountered, allocate a fresh `var_id` via `merged_pool.next_var_id` and record the mapping.
3. Rebuild the imported type tree with the remapped var_ids via full re-intern (NOT `Item.data` rewrite-in-place — the pool is append-only per `types.md §TY-6`; rewriting payloads in place would corrupt the intern map and the `hashes` column).
4. Rewrite `FunctionSig.scheme_var_ids` (`compiler/ori_types/src/output/mod.rs:423-428`, consumed by `llvm_backend.rs:321-327` to build `var_subst`) from the same remap map. `re_intern_sig` at `pool/re_intern/mod.rs:83-97` currently clones these unchanged — that is the hidden coherence bug that would silently regress §08.3 if missed.
5. Rewrite the `Tag::Scheme` binder list in `extra` (stored as raw var_ids at `pool/construct/mod.rs:161-174`, preserved unchanged at `pool/re_intern/mod.rs:185-193`) from the same remap map.
6. For each remapped var_id, CLONE the source `VarState` into the new destination id — do NOT blank-init to `Unbound`. `unify/substitute.rs:78-83` and `pool/substitute/mod.rs:82-88` branch on `VarState::Generalized`; wiping it distorts generic behavior. Cloning preserves semantic state while fixing the aliasing.
7. The `re_intern_type` hash fast path at `pool/re_intern/mod.rs:56-60` is INVALID for var-bearing subtrees once var_ids are remapped (leaf `Tag::Var` hashes include the raw var_id per `pool/mod.rs:395-413`). A remap-aware path is required for var-bearing types; the fast path may only be used when the source type has no var-bearing descendants (`TypeFlags::HAS_VAR` + `HAS_BOUND_VAR` + `HAS_RIGID_VAR` all clear).
8. **Out-of-scope (explicitly not part of §08.3's blast radius):** the `resolutions` map at `pool/accessors.rs:358-359` is keyed by `Idx`, NOT by `var_id`. DO NOT list it as a touch point; reviewers chasing phantom work there is a known failure mode flagged in codex's Step 4 blind-spots.

**Decision gate tasks:**

- [ ] **Confirm the remap-aware re-intern shape against `types.md §TY-6` append-only invariant**: re-read `types.md §TY-6` and `§TF-3` Merkle-hash propagation; confirm that fresh-var-id allocation + full re-intern (not `Item.data` rewrite-in-place) is the only pool-invariant-preserving fix shape. Record the confirmation as a comment on the §08.3 implementation checkbox.
- [ ] **Confirm production codegen is unaffected** by re-reading `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:115-165` and `nounwind/prepare.rs:95-149` — both operate on a single pool and never invoke `re_intern_*`. Document that the fix site (test-runner pool merge) is the ONLY current caller; lifting the remap abstraction into `pool/re_intern/` is defensive, not reactive.
- [ ] **Audit the T17–T19 regression pins added 2026-04-18** to `compiler/ori_types/src/check/validators/tests.rs`: they pin typeck-boundary behavior that is correct and unchanged by §08.3. They STAY as legitimate typeck clamps. Add a doc comment on each test noting `// Not the enforcement point for BUG-04-042 — see plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md §08.1.R` so future readers don't mistake them for the fix.
- [ ] **Confirm memory pointer handling**: `/home/eric/.claude/projects/-home-eric-projects-ori-lang/memory/project_bug_04_042_pool_merge_diagnosis.md` MUST be reduced to a one-line pointer to §08.1.R after §08.3 lands (per `CLAUDE.md §Plan Corrections Go IN the Plan` — plan is authoritative, memory is breadcrumb).

**Decision gate**: §08.1.5 MUST close before §08.3 starts. The §08.3 fix shape depends on the decision recorded here.

## 08.2 TDD matrix: poly-lambda + imported generics + Scheme PROPAGATE_MASK pin

**Goal:** Write failing tests BEFORE implementing the fix.

- [x] **Spec test (TDD)**: `tests/spec/expressions/poly_lambda_with_imported_generic.ori` — created 2026-04-18. Defines polymorphic identity lambdas paired with imported `assert_eq<T>` calls across four type instantiations (int/str/bool/float) plus lambda-flavor (a)/(b)/(c), locally-defined generic counterpart (import dim b), `Tag::Scheme PROPAGATE_MASK` regression pin (nested `[T]` lambda body), and nested-container parameter pin (`xs -> xs[0]` with `[T]` param). 10 attached tests total. **Verified TDD signal**: interpreter 10/10 pass, LLVM backend 10/10 compile-fail with `unresolved type variable at codegen — type inference bug idx=Idx(238)` + `Idx(241)` — matching §08.1's documented `Idx(241)` symptom exactly.
- [x] **Rust unit test in `ori_llvm`**: satisfied by the AOT integration test below (`compiler/ori_llvm/tests/aot/poly_lambda_mono.rs`), which IS a Rust test in the `ori_llvm` crate that exercises LLVM codegen end-to-end via the AOT pipeline. A separate lower-level test that constructs a `Pool` manually and calls `TypeInfoStore::get()` directly would duplicate the coverage with less realistic fixture data; the plan's intent (reproduce the bug via LLVM codegen with poly-lambda + imported-mono interaction) is met by the AOT test file.
- [x] **AOT integration test in `compiler/ori_llvm/tests/aot/`** (per parent plan overview line 35 — required mission deliverable): added `compiler/ori_llvm/tests/aot/poly_lambda_mono.rs` with two `assert_aot_success`-based tests (`test_poly_lambda_with_imported_assert_eq_int` and `..._str`) driving `ori build` → linked binary → runtime exit-0 on fixtures `fixtures/poly_lambda_mono/poly_lambda_with_imported_assert_eq_{int,str}.ori`. Registered in `compiler/ori_llvm/tests/aot/main.rs`. **Verified TDD signal**: both tests FAIL today at compile step with E5001 `unresolved function 'assert_eq' in apply/invoke — missing mono instance?` — the AOT-path surface of the same §08.1.R Hypothesis (d) leak. Pairs with the spec test (JIT path) to close the parent plan's pillar-5 verification contract.
- [x] **Matrix cells** (10 total — coverage decisions documented below):
  - **Type dimension**: `int`, `str`, `bool`, `float` — four mono instantiations of `assert_eq<T>` in the same file ✓
  - **Lambda dimension**:
    - (a) poly-lambda defined but unused ✓ (`poly_unused_int`, `poly_unused_str`)
    - (b) poly-lambda defined and called monomorphically ✓ (`poly_mono_{int,str,bool,float}`)
    - (c) poly-lambda defined and called with different types at different sites ✓ (`poly_multi_type_same_fn`)
    - (d) **DEFERRED to §08.5 broadened parity audit** — the `.map(transform: s -> s)` iterator-callback cell was found at §08.2 authoring time (2026-04-18) to be blocked on the interpreter by pre-existing **BUG-04-030 interference** (`tests/spec/patterns/data.ori` already fails the same way today). Running cell (d) inside §08.2 would make the test fail for the wrong reason (BUG-04-030, not §08). §08.5 is the correct home — it runs dual-exec-verify on `tests/spec/expressions/lambda_mono.ori` and `tests/spec/traits/iterator/` AFTER §08.3 closes the producer-side leak, which is when cell (d)'s intent (verify `is_polymorphic_lambda`'s `contains_bound_var` gate continues to route generalized-return callbacks around the mono pipeline) becomes testable. **Concrete anchor**: §08.5 checklist's "Dual-execution parity audit on poly-lambda paths beyond §08.2" item.
  - **Import dimension**: (a) `std.testing.assert_eq` ✓ + (b) locally-defined generic that mimics the same shape ✓ (`check_eq_local<T: Eq + Debug>`)
  - **Tag::Scheme `PROPAGATE_MASK` regression pin** ✓ (`propagate_mask_nested_list` — `let $wrap = x -> [x, x, x]` exercises `Tag::Scheme HAS_VAR` propagation through `[T]` lambda body)
  - **`prepare_mono_cached` cache-miss fallback negative pin**: covered implicitly — every test runs from a fresh compilation context, so every `assert_eq<T>` mono call lowers through the cache-miss path at `nounwind/prepare.rs:119-139`. The plan's original framing required a scenario "where the imported metadata is unavailable" via metadata stripping at `llvm_backend.rs:448-450`; that framing applied when the cache-miss path was an optimization fallback. Current behavior is that cache-miss IS the primary path on first mono emission — no special scenario needed. If §08.3 introduces a persistent cache, this item escalates to an explicit scenario.
  - **Nested-container substitution pin** ✓ (`nested_container_param` — `let $first_of = xs -> xs[0]` with `xs: [T]` nested generic parameter)
- [~] **Negative pin**: DEFERRED to §08.3 close-out — after §08.3's remap-aware re-intern lands and tests go green, `git stash` the fix, re-run the tests, confirm they fail again, then `git stash pop` to re-apply the fix. This is an EXECUTION-TIME verification that cannot run until §08.3 lands. **Concrete anchor**: §08.3 checklist item "Run §08.2 negative pin" is the single canonical execution entry for this verification; this `[~]` marker tracks the §08.2-owned contract, the `[ ]` in §08.3 is the actual execution checkbox.
- [x] **Verify all tests fail** before starting §08.3 implementation — verified 2026-04-18. Spec test: interpreter 10/10 pass + LLVM backend 10/10 compile-fail with `Idx(238)`/`Idx(241)` unresolved. AOT tests: 2/2 FAIL with E5001 `missing mono instance`. TDD discipline per `tests.md §TDD for Bugs` satisfied: failing tests are the exact shape §08.3 must make pass.

### 08.2 Matrix extension (2026-04-19, codex Step 4 blind-spots)

Under the corrected §08.1.R diagnosis (cross-module pool-merge var_id collision), four additional matrix cells pin the remap-aware re-intern semantics at the pool-crate boundary, where the fix actually lives. Each cell carries positive + negative pins per `tests.md §Matrix Clamping`. These cells extend (do NOT replace) the 10 cells above, which remain valid integration coverage for the end-to-end poly-lambda × imported-generic interaction.

- [ ] **(e1) Leaf var remap across pools** — add test cells to `compiler/ori_types/src/pool/re_intern/tests.rs` re-interning a standalone `Tag::Var(id=N)` from a source pool into a target pool whose `var_states[N]` is a different pool's Generalized slot.
  - Positive pin: `re_intern_type_with_var_remap` allocates a fresh `dst_id` via `target.next_var_id` AND `var_remap.get(&N) == Some(dst_id)` AND `target.var_states[dst_id]` is a clone of `source.var_states[N]`.
  - Negative pin: the legacy `re_intern_type` (without var_remap) reproduces the collision — asserts that `target` now contains a `Tag::Var(N)` whose `var_states` slot was cloned from the target pool (demonstrating why the remap-aware variant is load-bearing).
- [ ] **(e2) Scheme binder remap together with body leaves** — add test cells for `Tag::Scheme([7, 9], Tag::Function([Tag::Var(7), Tag::Var(9)], Tag::Var(7)))` re-interned across pools.
  - Positive pin: the re-interned scheme's binder list matches the remapped leaves (`scheme.binders == [remap[7], remap[9]]` AND every `Tag::Var` leaf in the body uses the same mapped ids).
  - Negative pin: a variant that re-interns leaves but clones binders unchanged; asserts the resulting scheme is internally inconsistent (binder list references a var_id absent from the body) — proves `pool/re_intern/mod.rs:185-193`'s current `source.scheme_vars(idx).to_vec()` pattern IS the bug when the enclosing pool merges var-id spaces.
- [ ] **(e3) FunctionSig.scheme_var_ids coherence with remapped type tree** — add test cells in `pool/re_intern/tests.rs` (or a new `sig_remap_tests.rs` sibling if `tests.rs` grows past the §BLOAT threshold) exercising `re_intern_sig` on a sig whose `scheme_var_ids = [7]` and whose `param_types` / `return_type` reference `Tag::Var(7)`.
  - Positive pin: after `re_intern_sig`, `sig.scheme_var_ids == [remap[7]]` AND every leaf `Tag::Var` in `param_types` / `return_type` uses `remap[7]`; a test `var_subst = HashMap::from([(sig.scheme_var_ids[0], concrete)])` + `substitute_in_pool(target, leaf, &var_subst)` resolves correctly.
  - Negative pin: run `re_intern_sig` in its pre-fix form (cloning `scheme_var_ids` unchanged); assert that `var_subst` built from the cloned ids does NOT substitute any leaves (because leaf and sig ids drifted) — proves the hidden coherence bug is exercised.
- [ ] **(e4) VarState clone-vs-blank-init semantic preservation** — add test cells verifying the `VarState` cloning path for `Tag::Var(VarState::Generalized { rank: 3 })` (or equivalent representative state per `types.md §SC-1`) across pools.
  - Positive pin: after `re_intern_type_with_var_remap`, `target.var_states[dst_id]` matches `source.var_states[src_id]` via structural equality (state variant + rank).
  - Negative pin: a variant that blanks the destination to `VarState::Unbound`; assert that `substitute_in_pool(target, leaf, &var_subst)` now takes the `Unbound` branch at `pool/substitute/mod.rs:82-88` (different from the `Generalized` branch at `unify/substitute.rs:78-83`), distorting dispatch — proves why cloning is load-bearing rather than cosmetic.

These cells live at the pool crate's re-intern boundary (where the fix actually lives), NOT at the validator boundary (where the 2026-04-18 T17–T19 pins live). The T17–T19 typeck-boundary pins stay as legitimate validator-behavior clamps; they are NOT the enforcement point for BUG-04-042 (per the §08.1.5 audit item that adds pointer comments to them).

### 08.2 HISTORY

- **2026-04-18 (original)** 10 integration matrix cells authored covering type × lambda × import × `Tag::Scheme PROPAGATE_MASK` × cache-miss × nested-container dimensions. Spec tests 10/10 failing on LLVM backend; AOT tests 2/2 failing with E5001. TDD discipline confirmed.
- **2026-04-19 (matrix extension)** Four additional cells (e1–e4) added to pin the remap-aware re-intern semantics at `pool/re_intern/` under the corrected §08.1.R diagnosis. Existing 10 cells remain valid — they pin end-to-end behavior; e1–e4 pin the implementation-boundary semantics that make the end-to-end behavior reachable. Driven by codex Step 4 blind-spots advice (`blind-spots.json` blind_spots + cross_cutting items): pool/re_intern/tests.rs:350-360 previously pinned only scheme hash parity, NOT var-id remap semantics or scheme_var_ids coherence.

**Tooling retrospective (2026-04-18, still valid):** no gaps. §08.2 was pure test-authoring backed by existing infrastructure — `cargo st` / `ori test --backend=llvm` for spec TDD signal verification, `assert_aot_success` in `compiler/ori_llvm/tests/aot/common.rs` for AOT integration tests, `cargo test -p ori_types validators` for typeck-side T17–T19 regression pins. The 2026-04-19 matrix extension similarly leans on existing infrastructure — `cargo test -p ori_types pool::re_intern` is the execution entry for e1–e4 — so no new tool is required. The static-classification workflow that originally pinned Hypothesis (d) without runtime `ORI_LOG` / `ORI_DUMP_AFTER_TYPECK` traces was constrained by denied runtime access; the 2026-04-19 re-diagnosis via `/tp-help` dual-source review and codex blind-spots confirms that static evidence (pool/re_intern source + llvm_backend merge site) was sufficient to identify the real root cause without runtime tooling. No new diagnostic scripts or test helpers created.

## 08.3 Implementation: remap-aware re-intern for cross-module pool merge

**Goal:** Fix the cross-module pool-merge var_id collision identified in §08.1.R by lifting remap logic into `compiler/ori_types/src/pool/re_intern/` as a reusable abstraction, with `compiler/oric/src/test/runner/llvm_backend.rs:320-360` as the first consumer. The TDD matrix in §08.2 pins the correct behavior; the fix must make those tests pass without breaking any existing test.

**Primary surface:** `compiler/ori_types/src/pool/re_intern/` (owner of the remap abstraction) + `compiler/oric/src/test/runner/llvm_backend.rs:320-360` (first caller). Do NOT scatter the remap logic into codegen or ARC passes; the pool crate owns type structure, so the re-intern module is where remap policy belongs.

**Implementation checklist (7 steps, each mapping 1:1 to §08.1.5's remap-aware re-intern shape):**

- [ ] **1. Design `re_intern_with_remap` API in `pool/re_intern/mod.rs`**: introduce a new public entry point (e.g., `pub fn re_intern_type_with_var_remap(source: &Pool, idx: Idx, target: &mut Pool, cache: &mut FxHashMap<Idx, Idx>, var_remap: &mut FxHashMap<u32, u32>) -> Idx`) that takes a `src_var_id → dst_var_id` map alongside the existing type cache. The existing `re_intern_type` stays for call sites that do NOT cross pool boundaries with variable-carrying types (structurally var-free imports); it delegates to the new entry point with an empty `var_remap` for backward compatibility. Document the distinction: the var-remap variant is mandatory for cross-pool-merge contexts where the target pool's `var_states` was cloned from a different source than the imported types. Keep the abstraction in the pool crate — do NOT export var_id surgery to consumers.
- [ ] **2. Build `src_var_id → dst_var_id` during re-intern**: for every imported `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, and `Tag::Scheme` binder encountered during re-intern, allocate a fresh `var_id` via `target.next_var_id` (extending the existing `var_states` vector) and record `var_remap.insert(src_var_id, dst_var_id)`. Replace the current `Tag::Var | Tag::BoundVar | Tag::RigidVar => target.intern(tag, source.data(idx))` arm at `pool/re_intern/mod.rs:192-193` with a remap-aware arm that reads `var_remap.entry(src_var_id).or_insert_with(|| target.next_var_id())` before the intern.
- [ ] **3. Rebuild the imported type tree via full re-intern (append-only per `types.md §TY-6`)**: the existing `re_intern_type` traversal already appends new entries to `target` rather than rewriting in place — preserve this. Do NOT introduce any code path that mutates `target.items[i].data` after interning; append-only is load-bearing for `intern_map` coherence and for the parallel `hashes` column (`types.md §TY-2`).
- [ ] **4. Rewrite `FunctionSig.scheme_var_ids` in `re_intern_sig`**: extend `re_intern_sig` at `pool/re_intern/mod.rs:83-97` to take the shared `var_remap` map and rewrite every entry of `result.scheme_var_ids` through it (panic via `expect` if a scheme_var_id is not in the remap — that's a soundness violation, not a recoverable case). This matches the `var_subst` build loop at `llvm_backend.rs:321-327` so every `generic_sig.scheme_var_ids[i]` still resolves to the intended `instance.generic_args[i]` after remap.
- [ ] **5. Rewrite the `Tag::Scheme` binder list in extra**: modify the `Tag::Scheme` arm at `pool/re_intern/mod.rs:185-193` — instead of `let vars = source.scheme_vars(idx).to_vec();` followed by `target.scheme(&vars, body)`, walk each `src_var_id` through `var_remap` to produce the destination binder list, then call `target.scheme(&remapped_vars, body)`. The binders MUST be allocated BEFORE the body is re-interned (so the body's leaf `Tag::BoundVar` / `Tag::Var` references to these binders can find them in the remap during the recursive descent).
- [ ] **6. Clone source `VarState` into destination id (do NOT blank-init to `Unbound`)**: after allocating the new var_id via `target.next_var_id`, explicitly set `target.var_states[new_id]` to a clone of `source.var_states[src_id]`. `unify/substitute.rs:78-83` and `pool/substitute/mod.rs:82-88` branch on `VarState::Generalized`; blanking the clone to `Unbound` would distort generic behavior at every downstream substitution site. Add a unit test in `pool/re_intern/tests.rs` that re-interns a `Tag::Var(Generalized)` across pools and asserts the destination slot carries `VarState::Generalized`, not `VarState::Unbound`.
- [ ] **7. Guard the Merkle-hash fast path against var-bearing subtrees**: modify the fast path at `pool/re_intern/mod.rs:56-60`. Before `target.lookup_by_hash(source.hash(idx))` returns a target `Idx`, check `source.flags(idx).intersects(HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR)` — if any var-bearing flag is set, SKIP the fast path and fall through to the remap-aware recursive traversal. Leaf `Tag::Var` hashes include the raw `var_id` (`pool/mod.rs:395-413`), so a var-bearing subtree's hash is pool-local; treating it as pool-independent is the quiet channel by which the collision reappears even after the explicit remap lands.

**Failure modes to guard against** (codex Step 4 blind-spots; each gets a §08.2 matrix cell):

- [ ] **(a) Merkle hash staleness — rewriting `Item.data` in place leaves intern_map + hashes column stale**: test that `re_intern_with_remap` NEVER mutates an existing `target` entry's payload. Positive pin: after re-intern, every `target.items[i]` that was already present before the call SHALL be `==` to its pre-call value. Negative pin: a test that deliberately mutates `target.items[i].data` and asserts `target.lookup_by_hash(target.hash(i))` now returns `None` (proves the invariant is load-bearing).
- [ ] **(b) Leaf var_id in Merkle hash — hash fast path invalid for var-bearing subtrees**: test `re_intern_with_remap` on a `[Tag::Var(id=7)]` list-type when `target` already contains a structurally-identical `[Tag::Var(id=7)]` from an unrelated source. Positive pin: the remap allocates a fresh `dst_id` (e.g., `id=42`) and interns a DIFFERENT target entry — the fast path MUST NOT dedup these. Negative pin: if step 7's guard is removed, the test demonstrates the collision (pre-remap dedup) returning the wrong target entry.
- [ ] **(c) Tag::Scheme binder remap — binders in extra payload must remap together with leaves**: test re-intern of `Tag::Scheme([7], Tag::Function([Tag::Var(7)], Tag::Var(7)))` across pools. Positive pin: after remap, the scheme's binder list AND every leaf `Tag::Var` reference resolve to the SAME fresh dst_var_id. Negative pin: a test that remaps leaves but not binders (removing step 5) and asserts the resulting scheme is internally inconsistent (binder list references a var_id absent from the body).
- [ ] **(d) FunctionSig.scheme_var_ids coherence — sig side must remap with type side**: test the full `re_intern_sig` path on a `FunctionSig` whose `scheme_var_ids` contains `[7]` and whose `param_types` / `return_type` reference `Tag::Var(7)`. Positive pin: after remap, `scheme_var_ids` and the leaf var references resolve to the SAME fresh dst_var_id, so `var_subst = HashMap::from([(scheme_var_ids[0], concrete)])` at `llvm_backend.rs:321-328` correctly substitutes every leaf. Negative pin: a test that remaps leaves but clones `scheme_var_ids` unchanged (step 4 omitted) and asserts `substitute_in_pool` leaves the leaves untouched (proving the silent regression path).
- [ ] **(e) VarState clone-vs-blank-init — cloning preserves Generalized semantics**: test re-intern of a `Tag::Var(VarState::Generalized)` across pools. Positive pin: the destination slot carries `VarState::Generalized` with the same `rank` as the source. Negative pin: a test that blank-inits the destination to `VarState::Unbound` and asserts `substitute_in_pool` now takes the `Unbound` branch instead of the `Generalized` branch, distorting generic dispatch.

**Downstream verification:**

- [ ] **Run `timeout 150 cargo test -p ori_types pool::re_intern`** — the new unit tests in `pool/re_intern/tests.rs` pass (including the 5 failure-mode guard pins above).
- [ ] **Run `timeout 150 cargo test -p ori_llvm`** — no regressions at the LLVM integration layer.
- [ ] **Run `timeout 150 cargo st`** — interpreter parity preserved (interpreter does NOT use the test-runner pool merge; this is a smoke test that the pool-crate changes didn't break the common path).
- [ ] **Run `timeout 150 ./target/release/ori test --backend=llvm tests/spec/expressions/poly_lambda_with_imported_generic.ori`** — the §08.2 spec corpus compiles cleanly via the LLVM backend.
- [ ] **Run `timeout 150 cargo test -p ori_llvm --test aot poly_lambda_mono`** — the §08.2 AOT integration tests pass (E5001 `missing mono instance` no longer fires).
- [ ] **Run §08.2 negative pin** (anchor for §08.2 deferral): after §08.3 tests are green, `git stash` the §08.3 implementation, re-run `cargo st tests/spec/expressions/poly_lambda_with_imported_generic.ori` + `cargo test -p ori_llvm --test aot poly_lambda_mono` + `cargo test -p ori_types pool::re_intern`, confirm ALL three test suites fail again with the §08.1.R symptoms. Then `git stash pop` to restore the fix. This pins §08.2's semantic contract: the tests pass ONLY because of §08.3's remap-aware re-intern, not because of unrelated changes.

**Note on `lambda_mono/type_resolve.rs` reconnaissance items** (`apply_bound_var_map`, `fallback_bound_vars_to_int`, `is_polymorphic_lambda`): the Reviewer-surfaced reconnaissance block flagged these as places where the surface symptom (`Idx(241)` unresolved) LOOKS like a lambda_mono bug. Under the corrected §08.1.R diagnosis, the lambda_mono path is a consumer, not a producer — once §08.3's remap-aware re-intern is in place, the substitution map built at `llvm_backend.rs:321-328` correctly resolves every imported leaf `Tag::Var`, so `resolve_all_lambda_bound_vars` (`define_phase.rs:134`, `nounwind/prepare.rs:173`) sees only well-scoped `BoundVar`s and completes without fallback. If lambda_mono code changes are still required after §08.3 lands, §08.3 re-opens; if not, those items are documented as unneeded and the `fallback_bound_vars_to_int` audit becomes a separate BLOAT concern (silent fallback in a code path that should never fire), filed via `/add-bug` with subsystem `ori-codegen`, severity `low`, after §08.5 closes.

**Note on `TypeInfoStore` size (audit BLOAT_RISK)**: `compiler/ori_llvm/src/codegen/type_info/store.rs` is 388 lines (audit minor finding). Under the corrected diagnosis this file is the OBSERVATION point where `Idx(241)` surfaces, not the root cause. Size concern is owned by a separate bug-tracker artifact whose lifecycle is independent from §08: file `/add-bug` titled `"BLOAT: ori_llvm::codegen::type_info::store.rs at 388 lines (approaching 500-line limit) — split into submodules"` with subsystem `ori-codegen`, severity `low`. Filing IS the concrete ownership transfer per CLAUDE.md §Ownership & Deferral. §08.3's pool-crate changes SHALL NOT add lines to `store.rs`; if they do (unexpected), the bug escalates to `medium` and the split happens inline before §08.5 closes.

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

## 08.6 §04 ↔ §08 seam coordination — confirm no seam-order change under the corrected fix

**Goal:** Under the corrected §08.1.R diagnosis (cross-module pool-merge var_id collision, fix in `pool/re_intern/` + test-runner call site), confirm that §04's `assert_no_unresolved_type_vars` debug_assert seam placement remains correct without modification. This section is now a confirmation-under-the-new-fix step, NOT a rework request — the §04.2 seam at `define_phase.rs:315` + `:375` already sits correctly relative to the new fix site.

**Why the seam order is correct under the corrected fix** (per codex Step 4 architectural_risks #4 and cross_cutting items):

- **The bug is injected BEFORE the seam, not by the seam.** §08.3's fix lands in `pool/re_intern/` — upstream of ARC lowering, upstream of lambda_mono substitution, upstream of `process_arc_function`. By the time §04.2's `assert_no_unresolved_type_vars` seam fires at `define_phase.rs:315` (`process_arc_function`) and `:375` (`declare_and_process_lambda`), the remap-aware re-intern has already produced clean, var-id-disjoint types in the merged pool. No mid-substitution `Tag::Var(Generalized)` or `Tag::BoundVar` state is observable at §04.2's seam that wouldn't have been observable under the prior (incorrect) diagnosis.
- **`resolve_all_lambda_bound_vars` still runs at its two callsites** (`define_phase.rs:134` inside `emit_arc_function`; `nounwind/prepare.rs:173` inside `prepare_arc_function`). These are codegen-side substitution passes for in-module lambda monomorphization; they are orthogonal to the cross-module re-intern path. §08.3's fix does not change when or how `resolve_all_lambda_bound_vars` runs.
- **§04.2's seam at `:315` + `:375` sees POST-lambda-mono-substitution state** (because `emit_arc_function` → … → `process_arc_function` puts line 134's substitution before line 315's assertion). Under the corrected diagnosis, the seam ALSO sees post-remap state (because `pool/re_intern/` runs during the test-runner's pool-merge step, long before any ARC pipeline call). Both invariants hold simultaneously.

**Distinguish the two call surfaces** (unchanged from the prior §08.6 content, still load-bearing documentation):

- **§04's debug_assert seam — `assert_no_unresolved_type_vars`** — inserted by §04 at the SINGLE upstream choke point per the parent plan's overview Architecture diagram:
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315` (`process_arc_function`) — pre-`run_arc_pipeline` per-function hook.
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:375` (`declare_and_process_lambda`) — pre-`run_arc_pipeline` per-lambda hook.
- **`lambda_mono` helper — `resolve_all_lambda_bound_vars`** — already exists; runs at two callsites:
  - `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:134` (inside `emit_arc_function`) — runs BEFORE the function body's lambdas are compiled.
  - `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:173` (`prepare_arc_function`) — runs in the two-pass nounwind path, also before per-function lambda compilation.

These remain distinct mechanisms; §08.3's fix touches neither.

**§08.6 tasks are now confirmation-only (reduced scope under the corrected diagnosis):**

- [ ] **Confirm no §04.2 seam order change is needed after §08.3 lands**: after §08.3's remap-aware re-intern is implemented and the §08.2 matrix (including the e1–e4 pool-crate cells) is green, re-read `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315` and `:375`. Verify that the assertion still fires POST-substitution for both the intra-module lambda_mono path AND the newly-fixed cross-module re-intern path. Record the confirmation in §08.6.R as "§04.2 seam order is correct under corrected §08.1.R diagnosis; no change required."
- [ ] **Confirm §04.2's assertion is neither too aggressive nor too permissive under the corrected fix**: the assertion should reject only `Tag::Var(Unbound)` (PC-2 violations); `Tag::BoundVar` and `Tag::Var(Generalized)` mid-substitution remain legitimate. Under §08.3's fix, the merged pool's imported types arrive at `process_arc_function` with `Tag::Var(Generalized)` vars bound to correctly-remapped var_ids — the assertion should see them as legitimate polymorphic state, not PC-2 violations. If §04.2's assertion rejects `Tag::Var(Generalized)` wholesale (too aggressive), it fires false positives on the §08.2 e4 cell's output; if it accepts `Tag::Var(Unbound)` (too permissive), it misses legitimate PC-2 violations. Record the verification in §08.6.R.
- [ ] **No cross-link edit to `section-04-codegen-assertions.md` is required** under the corrected diagnosis — the §04.2 seam already carries the correct placement rationale per the parent plan's overview Architecture diagram. The prior §08.6 plan to add a cross-link item to §04 was premised on a seam-choice risk that doesn't exist under the corrected fix shape. If §04.2 adds new pre-`process_arc_function` validation seams during its own implementation, this confirmation-only task picks them up at execution time; no pre-emptive §04.2 edit is needed now.

**Note on §08.1.5's investigation scope change:** the prior §08.6 text noted "§08.1.5's investigation may add a third surface (a typeck-side fix or a new pre-`process_arc_function` validation seam)". Under the corrected diagnosis, §08.1.5 does NOT add any typeck-side fix (the fix is pool-crate-side, upstream of typeck's output boundary) and does NOT add a new pre-`process_arc_function` validation seam (§04.2's seam remains sufficient because the fix runs pool-merge-side). §08.6 absorbs no additional coordination items.

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
