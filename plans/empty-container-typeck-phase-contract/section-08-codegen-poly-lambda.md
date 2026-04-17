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
  - "`assert_eq<int>` / `assert_eq<str>` / `assert_eq<bool>` monomorphize cleanly in any spec test file that also defines polymorphic lambdas. Verified by the existing `integer_safety.ori` + `lambda_mono.ori` pair continuing to compile, plus new coverage adding a file with BOTH features interleaved."
  - "`timeout 150 ./test-all.sh` reports no `Ori spec (LLVM backend) CRASHED` line; the LLVM backend spec run passes at parity with the interpreter (or carries concrete `#skip` annotations for any remaining skips, each pointing to a separate non-blocker bug)."
  - "LLVM IR verification (`ORI_VERIFY_ARC=1`) passes for every monomorphized `assert_eq` site."
  - "No regression in `tests/spec/expressions/lambda_mono.ori` (currently passes via interpreter — must continue to pass via LLVM)."
  - "Matrix: poly-lambda × import context × generic callsite — 3×3×N grid covered."
inspired_by:
  - "Rust rustc_codegen_ssa — handles poly-fn types and mono separately with careful Pool scoping"
  - "Swift SIL Mono — monomorphizes polymorphic closures via dedicated substitution passes that isolate BoundVar from the mono context"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "Investigation and root cause analysis"
    status: not-started
  - id: "08.2"
    title: "TDD matrix: poly-lambda + imported generics"
    status: not-started
  - id: "08.3"
    title: "Implementation: fix BoundVar bleed at identified call sites"
    status: not-started
  - id: "08.4"
    title: "Coordination with roadmap Section 21A (if active)"
    status: not-started
  - id: "08.5"
    title: "Verification: LLVM backend spec run green"
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

---

# Section 08: Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042)

**Status:** Not Started — **this section blocks atomic commits for the plan**.

**Origin:** Absorbed from bug-tracker `BUG-04-042` on 2026-04-17 per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan — NEVER sibling fix files". The bug was originally filed 2026-04-06 by `/continue-roadmap`, marked `BLOCKED` 2026-04-09 pending coordination with roadmap §21A, and blocked every prior commit attempt on this plan's validator-wiring work (§03.1, §03.2). Per the classifying rule "Can the plan complete with this bug open?" — the answer is NO (the plan cannot land its stated deliverable without a green `test-all.sh`), so the bug belongs in plan scope.

**Goal:** Resolve the polymorphic-lambda `BoundVar` bleed that prevents imported generic monomorphization when the host module contains polymorphic lambda definitions. Concrete failure mode: `tests/spec/expressions/lambda_mono.ori` — which contains polymorphic lambda definitions and calls `assert_eq` (an imported generic from `std.testing`) — fails via `--backend=llvm` with `Idx(241)` unresolved type variable and 17 LCFails, while `tests/spec/safety/integer_safety.ori` (which calls `assert_eq` without local polymorphic lambdas) passes cleanly.

## Why This Is a Commit Blocker

Every commit touching the plan's sections triggers the lefthook pre-commit hook, which runs `./test-all.sh`. Because the Ori spec (LLVM backend) run CRASHES on the `assert_eq<T>` monomorphization path, every commit attempt fails — even commits that only touch plan-internal typeck files. This is why §03.2 could not land without Section 08: the test gate is a hard precondition for commits, and BUG-04-042's symptoms fail the gate.

Previous sessions deferred this via `/add-bug` repeatedly, each creating a sibling fix file, each waiting on roadmap §21A coordination. The chain never completed. This section closes the chain by letting the plan own the fix directly.

## Root-Cause Hypothesis (from BUG-04-042 entry)

"Polymorphic lambda `BoundVar` types in the shared Pool interfere with `MonoInstance` body compilation for imported generics. Fix spans Pool scoping, type_info store, function compiler, and lambda_mono." — `plans/bug-tracker/section-04-codegen-llvm.md:459`

Candidate root causes to investigate in §08.1:

1. **Pool contamination**: polymorphic lambda registrations leave `BoundVar` residue in the shared Pool that downstream `MonoInstance` compilation reads as unbound when it tries to substitute generic parameters for the imported function.
2. **type_info store leak**: `compiler/ori_llvm/src/codegen/type_info/store.rs` records the polymorphic lambda's types, and when `assert_eq<int>` monomorphizes, it finds `BoundVar` in the store where it expected `int`.
3. **function_compiler/lambda_mono sequencing**: the order in which poly-lambda bodies and imported-generic mono bodies are compiled may cause one to observe the other's intermediate state.

The investigation in §08.1 will bisect by selectively reverting `ori_llvm` changes to isolate the source.

## 08.1 Investigation and root cause analysis

**Goal:** Produce a single sentence naming the root cause and the file + line(s) where it originates.

- [ ] **Reproduce the failure cleanly**: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` → capture the `Idx(241)` unresolved error and the 17 LCFail list.
- [ ] **Reduce the repro**: produce a 5-10 line `.ori` source that contains (a) one polymorphic lambda definition and (b) one call to `assert_eq` (or an inlined imported generic), and fails the same way. Save as `tests/spec/expressions/poly_lambda_mono_repro.ori` with `#skip("BUG-04-042")` temporarily.
- [ ] **Trace the failing mono site**: enable `ORI_LOG=ori_llvm=trace,ori_types=debug` on the repro; find the point where `Idx(241)` is looked up and fails; log the `Idx` at every monomorphization request.
- [ ] **Bisect the origin**: is `Idx(241)` (a) a poly-lambda's BoundVar that leaked into mono scope, (b) a scheme body var that should have been substituted before mono compile, or (c) a fresh instantiation var that was never linked?
- [ ] **Document the root cause** in a new §08.1.R subsection (analogous to §03.R) with the exact file:line where the bleed occurs.

## 08.2 TDD matrix: poly-lambda + imported generics

**Goal:** Write failing tests BEFORE implementing the fix.

- [ ] **Spec test (TDD)**: `tests/spec/expressions/poly_lambda_with_imported_generic.ori` — a file that defines a polymorphic lambda AND calls `assert_eq<int>` at least three times with different monomorphic types.
- [ ] **Rust unit test in `ori_llvm`**: a direct LLVM codegen test that monomorphizes `assert_eq<T>` in the presence of a pre-existing poly-lambda registration in the type_info store.
- [ ] **Matrix cells**:
  - Type dimension: `int`, `str`, `bool`, `float` — four mono instantiations of `assert_eq<T>` in the same file
  - Lambda dimension: (a) poly-lambda defined but unused, (b) poly-lambda defined and called monomorphically, (c) poly-lambda defined and called with different types at different sites
  - Import dimension: (a) `std.testing::assert_eq` (the actual failure case), (b) locally-defined generic that mimics the same shape
- [ ] **Negative pin**: confirm that reverting the §08.3 fix causes the tests to fail again (prevents silent regression).
- [ ] **Verify all tests fail** before starting §08.3 implementation (TDD discipline per `tests.md §TDD for Bugs`).

## 08.3 Implementation: fix BoundVar bleed at identified call sites

**Goal:** Fix the root cause identified in §08.1. Scope depends on §08.1 findings — the TDD matrix in §08.2 pins the correct behavior; the fix must make those tests pass without breaking any existing test.

- [ ] **Fix the identified call site(s)** per §08.1 root cause. Candidate fix shapes (pick one based on investigation):
  - If Pool contamination: scope the poly-lambda registration so its `BoundVar`s don't leak across monomorphization boundaries.
  - If type_info store leak: tag the store entries with their originating monomorphization context so the mono pipeline doesn't read poly-lambda entries when resolving imported generics.
  - If sequencing: reorder the mono pipeline so imported generics are fully resolved before poly-lambda body compilation proceeds.
- [ ] **Run `timeout 150 cargo test -p ori_llvm`** — no regressions.
- [ ] **Run `timeout 150 cargo st`** — interpreter parity preserved.
- [ ] **Run `timeout 150 ./target/release/ori test --backend=llvm tests/`** — LLVM backend passes on the §08.2 test corpus.
- [ ] **Remove `#skip("BUG-04-042")`** from the repro file and confirm it passes via both backends.

## 08.4 Coordination with roadmap Section 21A (if active)

**Goal:** Ensure the fix doesn't interfere with active roadmap §21A work.

- [ ] **Check roadmap §21A status**: is it still active in the `compiler/ori_llvm/src/codegen/` area?
- [ ] **If active**: coordinate merge order; the §21A author needs visibility into §08.3's scope.
- [ ] **If complete / blocked**: proceed independently; note §21A's status in the section retrospective.
- [ ] **If conflicts arise**: pause §08.3 until coordination is resolved. Do NOT merge a fix that conflicts with in-flight work.

## 08.5 Verification: LLVM backend spec run green

**Goal:** `./test-all.sh` Ori spec (LLVM backend) runs with zero crashes and zero new failures attributable to Section 08's scope.

- [ ] **Run `timeout 150 ./test-all.sh`** on a clean tree — capture the full output.
- [ ] **Verify**: no `Ori spec (LLVM backend) CRASHED` line; `assert_eq$m$int` compiles; LLVM IR verification passes.
- [ ] **Annotate remaining failures**: any spec test still failing must carry a `#skip(...)` with a pointer to a separate non-blocker bug (per plan Mission Success Criteria).
- [ ] **`diagnostics/dual-exec-verify.sh`** on the §08.2 test corpus — interpreter and LLVM produce identical results.

## 08.R Third Party Review Findings

To be populated after §08.3 implementation via `/tpr-review`.

## 08.N Completion Checklist

- [ ] All §08.1-§08.5 tasks are `[x]` and behavior is verified
- [ ] `timeout 150 ./test-all.sh` is GREEN (no crashes, no new failures in this section's scope)
- [ ] `timeout 150 ./clippy-all.sh` is clean
- [ ] `/tpr-review` passed on §08 diff — independent dual-source review clean, or all findings triaged in §08.R
- [ ] `/impl-hygiene-review` passed after TPR is clean
- [ ] `/improve-tooling` retrospective run (section-close sweep)
- [ ] Plan-annotation comments removed from production code at §07 close-out (permanent spec references excluded)
- [ ] Bug-tracker entry for BUG-04-042 closed with pointer to this section (at §07.1)
- [ ] Section 08 status updated to `complete` in plan frontmatter and overview Quick Reference
- [ ] **Commit-wall is RESOLVED** — atomic commits for subsequent plan sections succeed on the first attempt
