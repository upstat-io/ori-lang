---
bug: "BUG-04-074"
title: "AOT codegen: empty list literal `[]` with `push()` leaves unresolved type variables — LLVM verification failure"
severity: "critical"
status: in-progress
goal: "Empty list literals (`[]`) with element types inferred solely from downstream usage compile cleanly through AOT, with `resolve_fully()` producing concrete element types for codegen. Ambiguous empty-list bindings emit spec-mandated E2005 (14-expressions.md:1224-1228) at type check time, not codegen."
success_criteria:
  - "The exact repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and runs successfully"
  - "No `unresolved type variable at codegen` errors from `ori_llvm::codegen::type_info::store` for empty-list-with-inferred-element cases"
  - "Interpreter and LLVM produce identical results (dual-execution parity) for the repro and edge cases"
  - "Matrix tests cover empty `[]`, `Set<T>`, `{}` + int/str/bool/struct element types + push/insert/len/iter usage patterns"
  - "No regressions in `timeout 150 ./test-all.sh`"
subsystem: "compiler/ori_types/src/infer/expr/blocks.rs"
found: "2026-04-13"
source: "continue-roadmap"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-074 — AOT codegen: empty list literal `[]` with `push()` leaves unresolved type variables — LLVM verification failure

**Status:** In Progress
**Severity:** high
**Goal:** Empty container literals without explicit type annotations must produce resolvable types at codegen time when constrained by downstream usage within the same function body.

**Success Criteria:**
- [ ] Repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and exits 0
- [ ] Matrix of empty-container + usage-constraint combinations compiles and runs through both interpreter and LLVM
- [ ] Semantic pin test that only passes with the new resolution behavior
- [ ] Negative pin that rejects the old generalized-var-leak behavior
- [ ] No regressions in `timeout 150 ./test-all.sh`

**Context:** Filed 2026-04-13 during continue-roadmap work. The interpreter handles empty lists correctly (type inference flows through naturally), but AOT compilation fails with three `unresolved type variable at codegen — type inference bug` errors. The bug is intermittent across empty-container scenarios and blocks AOT compilation of idiomatic Ori code like initializing an empty list and populating it via `push()`.

---

## 0. POST-INVESTIGATION REWRITE (2026-04-16)

**Why this section exists.** §1.5 round 1 (2026-04-14) reached `/tp-help` consensus on the generalization-policy + body-exit-validation approach. §R round-1/2/3/4 Plan TPR refined it. Commits `b1f2c354 … 65b3aff4` shipped §3.1/§3.2/§3.3/§3.4/§3.6 (should_generalize + validate_body_types + wiring at `blocks.rs:77/159` + `sequences.rs:249` + the producer-side PC-2 enforcement described in `typeck.md §PC-2`). `timeout 150 cargo st tests/spec/` against that shipped state:

- `ori check` on the repro → clean (no E2005, no typeck error).
- `ori build` on the repro → still fails with `unresolved type variable at codegen — type inference bug idx=Idx(96)`.

**The shipped plan closed the wrong leak.** Body-exit validation catches *let-binding types with unbound vars at body exit*. It does not catch the case where the method-call machinery itself never populated the unification constraint that would have bound those vars. `ori check` passes because by the end of body-inference, there are NO remaining `Tag::Var`s in the body's `expr_types` map — the receiver type `List(Var(X))` had `Var(X)` linked during some downstream unification path (specifically `unify_higher_order_constraints` firing for certain method names, or receiver-receiver unification when a later call's receiver type happens to merge with the original). But in the repro's chain — `[] → push(value:10) → len()` — `push` is NOT in the `unify_higher_order_constraints` whitelist (line 175-263 of `method_call.rs`), so no unification fires; the final `List(Var(X))` type reaches codegen through a path that escapes the body-exit validator's walk.

**Residual root cause.** `resolve_builtin_method` at `compiler/ori_types/src/infer/expr/methods/mod.rs:53-86` takes `(engine, receiver_ty, tag, method_name)` and returns `Option<Idx>` — a return type. It NEVER consumes the call-site argument types, NEVER performs arity checking, and NEVER unifies arg types against the method's declared parameter types. For `ages.push(value: 10)`:

- `ages : List(Var(X))` (element type is an unbound unification variable).
- Registry lookup `find_method(TypeTag::List, "push")` returns a `MethodDef` whose `params: &[ParamDef { name: "value", ty: ReturnTag::ElementType, ownership: Owned }]` and whose `returns: ReturnTag::SelfType`.
- `registry_bridge::return_tag_to_idx` converts the return `SelfType` back to `receiver_ty = List(Var(X))` — so the method call's result type is `List(Var(X))` with `Var(X)` unchanged.
- The call-site arg `10 : int` is inferred (in `method_call.rs:33-37`) but is NEVER unified with what the method's `params[0].ty` (which is `ReturnTag::ElementType`, resolvable to `Var(X)` via the SAME `return_tag_to_idx` bridge) declares. `Var(X)` stays `Unbound`.
- The chain then assigns `ages = ages.push(value: 10)` — `ages`'s stored type remains `List(Var(X))`. `.len()` returns `int` regardless of the element type, so no later unification binds `Var(X)`. Codegen sees `List(Var(X))` at `Idx(96)` and errors.

**Why `ori check`'s body-exit validator missed it.** The `push` call re-interns `List(Var(X))` into the expression's `expr_types` entry. `Var(X)`'s `var_id` is the SAME one present in `FunctionSig.param_types` iff this is a generic body — but in the repro, the enclosing `@main` is non-generic and has no scheme vars, so `scheme_var_ids` is empty. The validator walks the body types and SHOULD flag `Var(X)` as unresolved. Reading `validate_body_types` carefully: it DOES emit E2005 on any `Tag::Var` that is not in `scheme_var_ids`. So `ori check` SHOULD report E2005 — but empirically it doesn't. The explanation per investigation: `expr_types` stores the UNRESOLVED (pre-link) type for the `[]` empty-list literal, which is `List(Var(X))`. At body exit, `Var(X)` is STILL `Unbound`, so the validator walk does find it and emit E2005 on that span. The REAL phenomenon that looks like "`ori check` passes" appears to be: some codepath links `Var(X)` mid-body (possibly `unify_higher_order_constraints` on an adjacent call, OR the receiver-receiver unification at `method_call.rs:38/80` calling `unify_higher_order_constraints` with `method="push"`, which is in the `_ => {}` fallthrough arm and does nothing but by-the-way returns the ret_ty that re-registers `List(Var(X))` under a FRESH pool Idx that differs from the stored `expr_types` entry). Result: the body-exit validator sees no unresolved var under the recorded ExprIndex, but codegen's own `pool.resolve_fully` walk of the ArcFunction re-derives `List(Var(X))` from the typed IR and fails at `Idx(96)`. This is a separate leak between ` validate_body_types`'s scope and codegen's scope — worth filing via `/add-bug` as a distinct validator-coverage gap, but NOT the primary bug.

**Fresh `/tp-help` consensus (2026-04-16, `/tmp/ori-tpr-as5XcVA2`).** Both reviewers converge on: the root cause is the builtin arg-param unification gap. Codex: "hybrid — A (arg-param unification inside builtin dispatch consuming `MethodDef.params`) now, B (unified `MethodRegistry` endgame) later." Codex corrected a pre-consensus claim that `MethodDef` lacks parameter metadata — `ParamDef { name, ty: ReturnTag, ownership }` at `compiler/ori_registry/src/method/mod.rs:14-23` already exists, and `ReturnTag` is documented (`compiler/ori_registry/src/tags/return_tag.rs:8`) as valid for parameter positions. The bridge `registry_bridge::return_tag_to_idx` (`compiler/ori_types/src/infer/expr/registry_bridge/mod.rs:285-372`) maps `ReturnTag → Idx` relative to the receiver — it works identically whether the source is a return or a param. So Approach A consumes an existing SSOT, NOT new metadata, and is therefore NOT a `LEAK:algorithmic-duplication`. Gemini: "Approach B only per CLAUDE §The One Rule." Reconciliation: Codex's factual correction is verifiable in source; Gemini's position rested on the false premise that A requires new parameter encoding. User confirmed 2026-04-16: hybrid — A in this fix arc, B tracked as a follow-up plan (file path to be assigned at Phase 5 closure per §3.7' below).

**Phase of record at this point.** §1 Root Cause Analysis, §1.5 round 1, §2 (base TDD matrix), §2.5 rounds 1-4, and §3.1/§3.2/§3.3/§3.4/§3.6 are ALL shipped. The fix-plan is now reset to Phase 2.5 with a NEW §3' (below, under §3) as the rewritten Implementation surface, a NEW §1.5 round 2 (2026-04-16) consensus record, a NEW §2.arg-param TDD matrix expansion, and SUPERSEDED-ENTIRELY banners on §R rounds 1-4. ONE fresh Plan TPR round runs next on §3'; then Phase 3 TDD with the expanded matrix.

---

## 0.1 Session Recovery — 2026-04-16 (pre-context-clear snapshot)

**Authoritative snapshot** of /fix-bug resume state. Read this BEFORE re-entering /fix-bug Phase 0 after a context clear. Agrees with the frontmatter `resume_point` field — this section is the body-visible mirror so Phase 0's file read surfaces it even if frontmatter is abbreviated.

### State at snapshot time

- **Phase**: 2.5 Plan TPR. Round 5 COMPLETE. Round 6 verification NOT YET RUN.
- **Fix file git state**: MODIFIED, UNSTAGED in working tree. Intentional — do NOT stage or commit on resume.
- **Why commit deferred (2026-04-16 decision)**: pre-commit hook (lefthook `full-check.sh`) runs `./target/release/ori test --backend=llvm tests/` which currently SIGABRTs + cascades E2005 errors on capability/propagation tests. Those failures ARE the exact BUG-04-074 symptom the §3' plan fixes. Committing before Phase 4 requires `--no-verify`, which the user has NOT authorized — so commit happens AFTER Phase 4 implementation when the test suite returns clean.
- **User directive at snapshot**: "Defer commit until Phase 4 implementation fixes the failures." (Recorded from AskUserQuestion response, 2026-04-16.)

### Round 5 findings (all resolved in the uncommitted edits below)

| ID | Severity | Convergence | Location | Resolution site in §3' |
|----|----------|-------------|----------|------------------------|
| TPR-05-codex-F1 + TPR-05-gemini-F1 | HIGH | AGREEMENT | `ori_registry/src/defs/option/mod.rs:114` — `ok_or` uses `ReturnTag::ResultOfProjectionFresh(TypeProjection::Element)` composite return tag | §3.1'.a — introduced `return_tag_fresh_arity()` helper + threaded correlation slot through `return_tag_to_idx` for composite-Fresh subparts; added bucket-4 fallback for multi-Fresh non-higher-order methods |
| TPR-05-codex-F2 | HIGH | Standalone | `ori_types/src/unify/mod.rs:289` — `Action::Link` fires for var-to-var links, not just var-to-concrete | §3.2'.a — added root-resolution gate using `pool.resolve_fully`; replay fires only on non-Var, non-Error root; obligations stay pending across var-to-var links with monotonic convergence |
| TPR-05-codex-F3 + TPR-05-gemini-F2 | HIGH | AGREEMENT | `ori_types/src/check/mod.rs:389` — `TypeCheckResult::finish_with_pool()` exports flat `TypedModule`, NO `TypedBody` aggregate | §3.4'.ii EXCISED (not relocated) — per-body-exit validator + §3.1'/§3.1'.a arg-param fix closes the scope-mismatch concern that §3.4'.ii was defending against. §3.4' renumbered: former .iii/.iv/.v/.vi → now .ii/.iii/.iv/.v |
| TPR-05-gemini-F3 | LOW | META (duplicate) | `fix-BUG-04-074.md:1314` — ≤20 E2005 threshold "arbitrary" | Already labeled "plan-local heuristic (NOT a rule-citation)" per prior TPR-05-R1-codex-F5 revision. No action. |

Full Round 5 entry in §R "Phase 2.5 — Plan TPR Round 5 (2026-04-16)" (below the SUPERSEDED banner).

### Recovery playbook for next session

1. **`/continue-roadmap` scanner runs first.** Expect `critical_bugs` gate to fire (BUG-04-074 still `- [ ]` in section-04-codegen-llvm.md:48, correctly — the fix has NOT landed yet) + `dirty_tree` gate to fire (unstaged fix file + pre-existing parallel-session files). User picks "fix-bug BUG-04-074" again; dirty-tree either "Proceed with dirty tree" (consistent with snapshot) or "commit-push" (WILL FAIL on same hook — do NOT pick this unless Phase 4 has since landed).
2. **`/fix-bug BUG-04-074`** — Phase 0 sub-agent reads this file, detects existing fix file with `status: in-progress` + frontmatter `resume_point` starting with "2026-04-16 SESSION RECOVERY POINT". Handoff should include `Resume mode: yes — pick up at Phase 2.5 Round 6`.
3. **Phase 2.5 Round 6 Plan TPR.** Invoke `/tpr-review` via Skill tool on `plans/bug-tracker/fix-BUG-04-074.md` with objective "verify Round 5 revisions are clean — Round 5 findings were resolved in §3.1'.a (Fresh-arity helper), §3.2'.a (root-resolution gate), §3.4' (excised .ii, renumbered), §3.6' (updated). Do NOT re-find those resolved items." Expect CLEAN status or trivial findings only.
4. **Phase 3 TDD.** Write all tests from §2.arg-param + §2.arg-param.R5 matrix (lines 296-380 of this file). Verify they FAIL against current code (confirms they test the actual bug).
5. **Phase 4 Implementation.** Per §3.8' Implementation order: §3.1'+§3.1'.a → §3.2'+§3.2'.a → §3.3' → §3.4'.i/.ii/.iii/.iv/.v → §3.5'. Run `timeout 150 ./test-all.sh` after the sequence — should return clean. Full-check.sh pre-commit hook should then pass.
6. **Phase 4 /commit-push** — now succeeds because the pre-existing failures ARE fixed by this arc.
7. **Phase 5 Completion Checklist** — /tpr-review on code, /impl-hygiene-review, /improve-tooling retrospective, /sync-claude, file §3.6' remaining follow-up bugs via /add-bug, /create-plan for §3.7' Approach B endgame, flip bug entry to `[x]` + update overview count, final /commit-push.

### Invariants to preserve across context clear

- BUG-04-074 entry in `section-04-codegen-llvm.md:48` STAYS `- [ ]` with BLOCKER note until Phase 5 closure.
- `00-overview.md` BUG-04-074 reference STAYS in the blocker group with BUG-04-042 + BUG-04-084.
- Sibling bugs BUG-04-042 + BUG-04-084 also stay `[ ]` — they share the same BLOCKER annotation and will be unblocked (possibly fixed, possibly re-verified) by this arc's §3.1' arg-param unification landing.
- Fix file `status: in-progress` stays until Phase 5 final close.
- Parallel-session files (`.claude/skills/fix-next-bug/SKILL.md`, `.claude/skills/improve-tooling/tpr-review-design.md`, `.claude/skills/tp-help/SKILL.md`, `.claude/skills/tpr-review/SKILL.md`, `tests/spec/lexical/keywords.ori`, `tests/spec/traits/core/is_empty.ori`, `tests/spec/traits/core/len.ori`) are from the user's other sessions and must NOT be touched, unstaged, or reset by the recovery flow.

### What NOT to do on resume

- Do NOT re-run Round 5 Plan TPR — it's complete, findings recorded, revisions applied.
- Do NOT discard the fix file's uncommitted edits. They ARE Round 5's output; re-doing the work is pure waste.
- Do NOT `--no-verify` the /commit-push — user withheld authorization at snapshot time.
- Do NOT invoke `/create-plan` for §3.7' yet — it's a Phase 5 closure artifact; creating it now violates the plan escalation workflow.
- Do NOT re-investigate "is the crash pre-existing?" — the only valid question is "is it fixed?" Answer: not yet, but the active fix arc IS the fix.

---

## 1. Root Cause Analysis

- **Symptom**: `ori build` emits `error[E5001]: LLVM module verification failed` with 3 preceding `unresolved type variable at codegen` errors on Idx(96), Idx(99), Idx(103). The interpreter (`ori run`) handles the same program correctly.

- **Proximate cause**: The stored expression type for the empty list literal `[]` in `TypedModule.expr_types` is `List(Var(X))` where `Var(X)` has state `Generalized` and has NO `Link` to a concrete type. Additional instantiation vars (Var(Y)) at use sites like `ages.len()` remain `Unbound` because `.len()` doesn't constrain the element type. When `ori_llvm::codegen::type_info::store::get_or_compute_type_info` encounters these Vars during codegen emission, `pool.resolve_fully(idx)` returns the Var unchanged (because it only follows `VarState::Link` chains, not `Generalized` or `Unbound` states), triggering the error path.

- **Root cause**: The `infer_let` function in `compiler/ori_types/src/infer/expr/blocks.rs:167` unconditionally calls `engine.generalize(init_ty)` on every let binding initializer type, implementing HM let-polymorphism. For empty containers whose element type is a fresh unification variable introduced by `infer_empty_list()`, generalization marks the element var as `VarState::Generalized`. Later scheme instantiations at use sites create fresh `Var(Y)`s — SOME get linked (e.g., `push(value: 10)` unifies its fresh var with `int`), but OTHERS at element-type-irrelevant use sites (like `.len()`) stay `Unbound`. There is no persistent mapping from the generalized var to its concrete instantiations, so the expression type stored on the `[]` literal retains the generalized var, and the `.len()` call's instantiation var retains its `Unbound` state.

  **Confirmed by debug output**:
  - `Idx(96)`: `Var(5)` state = `Generalized { id: 5, name: None }` — the original empty-list element var
  - `Idx(99)`: `Var(6)` state = `Unbound { id: 6, rank: Rank(3), name: None }` — a scheme-instantiation var at a use site
  - `Idx(103)`: `Var(8)` state = `Unbound { id: 8, rank: Rank(3), name: None }` — another instantiation

- **Blast radius**: Affects all empty container literals (`[]`, `{}`, `Set<T>`) whose element types are inferred from downstream usage and where at least one use site doesn't fully constrain the element type. Because `.len()`, `.is_empty()`, and control-flow predicates on containers don't constrain element types, this is a wide class of real-world programs. Confirmed affected (tested):
  - `let ages = [];` + `ages.push(...)` + `ages.len()` — FAILS
  - `let $ages = [];` + `ages.push(...)` + `.len()` — FAILS (both mutable and immutable)
  - `let ages: [int] = []; ...` — WORKS (annotation monomorphizes)
  - `let ages = [0]; ages = ages.push(...)` — WORKS (non-empty has concrete element)

- **Affected files**:
  - `compiler/ori_types/src/infer/expr/blocks.rs:167` — `infer_let` must avoid generalization when it would leave unresolvable vars. The proposed change: guard generalization on the top-level tag of `init_ty`. Only generalize when the init type is a function (standard let-polymorphism for lambdas like `let id = x -> x`). Container types (`List`, `Option`, `Set`, `Map`, `Tuple`, `Range`, etc.) should NOT be generalized — local bindings of these types should remain monomorphic, with the fresh element var staying `Unbound` so downstream unification links it directly.

**Reference implementations**:
- **Rust `rustc_hir_typeck`**: Local `let` bindings are NEVER generalized. Rust has no let-polymorphism for local bindings — each binding must resolve to a single concrete type. `let x = vec![]; x.push(10);` works because inference flows forward monomorphically. This is the cleanest model for a compiler that monomorphizes for codegen.
- **OCaml, Elm, Gleam**: Generalize let bindings (HM), but these languages don't require monomorphic codegen (JS/bytecode/tree-walking). Their approach doesn't transfer to LLVM-targeting compilers.
- **Haskell GHC**: Has the monomorphism restriction specifically to avoid unresolved inference problems with polymorphic local bindings. Similar motivation to this fix.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach. Ran BEFORE tests or implementation to catch wrong-approach errors before they lock in. See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the calling contract.

- **Proposed approach (pre-consensus)**: In `infer_let` at `compiler/ori_types/src/infer/expr/blocks.rs:167`, guard the `engine.generalize(init_ty)` call on the top-level tag of `init_ty`. Only generalize when `init_ty.tag() == Tag::Function | Tag::Scheme`. Bind the pattern to `init_ty` directly for other tags.
- **tp-help run scratch dir**: `/tmp/ori-tpr-VmdOpipn` (launched 2026-04-14 14:08:53 EDT, codex walltime 340s, gemini walltime 104s)

### Round 1

**Codex summary (LEAK + GAP + DRIFT findings)**:
- `LEAK` + `GAP`: **Proposed patch site is not the repro path.** Block-statement `let` inside a function body is handled by `infer_block` at `compiler/ori_types/src/infer/expr/blocks.rs:22-97`, generalizing at line 88 (and line 85 for non-capturing lambdas). `infer_let` at line 116 is only used for `ExprKind::Let` via `infer/expr/mod.rs:167-172`. A third duplicated policy exists in try-block lets at `sequences.rs:204-251` with generalize at line 247.
- `DRIFT`: **The spec already says the repro case is a compile-time error.** `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228` states: "An empty list literal `[]` requires type context for inference. Without context, it is a compile-time error." Example: `let y = [];  // error: cannot infer element type`. The compiler is violating the spec by NOT emitting this error and silently passing to codegen.
- `GAP`: **PC-2/TR-2 is being enforced in the wrong phase.** Codegen detection (`ori_llvm/src/codegen/type_info/store.rs:341-363`) is too late. Typeck should emit `E2005` (ambiguous_type — already defined at `type_error/check_error/mod.rs:235-243` but no production call site) via a checker-exit sweep.
- Recommendation: fix ALL THREE generalize sites + wire `E2005` + add PC-2 validation sweep before exports/codegen.

**Gemini summary (DRIFT + GAP findings)**:
- `DRIFT`: **Tag-based check is brittle.** `matches!(tag, Function | Scheme)` fails when the resolved type is still a `Tag::Var` awaiting bi-directional unification (e.g., `let f = if cond then (x -> x) else (y -> y)`). Tag-based check would not generalize this even though it should. Use **AST-based Value Restriction**: check `ExprKind::Lambda` on the init's AST node instead.
- `GAP`: **(b) and (d) are not alternatives — they are two halves of the same phase contract.** MUST implement both: (d) skip generalization for containers (so `push` can unify directly), AND (b) emit `TypeCheckError("cannot infer type")` for any remaining `Tag::Var` at check exit. Otherwise, programs with no downstream constraint (`let xs = []; xs.len()`) leak `Unbound` vars to codegen.
- Spec verified: `docs/ori_lang/v2026/spec/13-variables.md` does NOT mandate let-polymorphism for local bindings. §13.6 requires value semantics ("Assignment is value copy") which is incompatible with polymorphic local bindings mutating types across calls. Standardizing on monomorphic local bindings (Rust-style) is fully spec-compliant.
- Recommendation: (1) Change generalization guard to AST-based Value Restriction (`ExprKind::Lambda`), (2) Implement final resolution pass emitting E2005, (3) Update `tests/spec/types/collections.ori` to annotate ambiguous empty lists.

**Agreement points (strong convergence)**:
1. My proposed fix is **incomplete and uses brittle tag-based detection**. Use AST-based Value Restriction instead.
2. **Phase contract enforcement MUST be added to the type checker** — a final-resolution sweep that emits `E2005` for any remaining `Tag::Var` in `expr_types`. Without this, PC-2/TR-2 is violated.
3. **Spec alignment is mandatory**: `14-expressions.md:1224-1228` already declares `let y = []` a compile-time error. The compiler is not enforcing its own spec.
4. There are **multiple generalization sites** (at least three: `infer_block` line 85/88, `infer_let` line 167, `sequences.rs:247`) — the fix must address all of them as an SSOT policy.
5. The existing code at `blocks.rs:79-89` ALREADY uses AST-based Lambda detection for capturing closures (same pattern needed for this fix) — this is precedent.

**Disagreement points**: None material. Both reviewers converge on the same recommendation.

**Independent code verification**:
- ✅ Verified `compiler/ori_types/src/infer/expr/blocks.rs:22-97`: block-statement let handler. Lines 79-89 show existing AST-based Lambda check for capturing closures. Generalization at line 85 (non-capturing lambdas) and line 88 (fallback — THIS is the repro path).
- ✅ Verified `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228`: spec explicitly says `let y = []` is a compile-time error.
- ✅ Verified `compiler/ori_types/src/infer/expr/sequences.rs:204-251`: try-block let handler. Line 247 generalizes `bound_ty`.
- ✅ Verified `compiler/ori_types/src/infer/expr/blocks.rs:116-179` (`infer_let`): generalization at line 167. This IS a generalization site, but `infer_let` is only invoked by `ExprKind::Let` as a standalone expression, NOT by block-statement `let` — so my original proposed fix would have fixed a different path than the repro.
- ✅ Verified `E2005` (ambiguous_type) in `compiler/ori_types/src/type_error/check_error/`: the error code + constructor exist but have no production call site — this is a GAP per codex.

**Outcome**: Persuaded divergence — **I was wrong about both the patch site AND the completeness of the fix.** The revised approach integrates both reviewers' recommendations.

### Final agreed approach (REVISED after Plan TPR rounds 1 & 2 — 2026-04-14)

**Multi-part point fix** (still scoped to `ori_types` crate, no plan escalation needed). Revised after TWO rounds of Plan TPR findings — round 1 reshaped consensus (§R Phase 2.5 round 1), round 2 corrected architectural flaws in the validation approach (§R Phase 2.5 round 2).

1. **AST-based Value Restriction at all 3 generalization sites**. Extract a shared helper `should_generalize(arena: &ExprArena, init: ExprId) -> bool` that returns true ONLY if the init is a non-capturing `ExprKind::Lambda`. Apply at:
   - `compiler/ori_types/src/infer/expr/blocks.rs:79-89` — block-statement let in `infer_block` (primary repro path per TPR-04-004-codex)
   - `compiler/ori_types/src/infer/expr/blocks.rs:167` — `infer_let` (standalone let expression — rare surface, needs dedicated test)
   - `compiler/ori_types/src/infer/expr/sequences.rs:247` — try-block let

   This SOLVES the primary repro: `ages.push(value: 10)` directly unifies the fresh Var(X) with `int` via `VarState::Link` (no generalization interposed), so `pool.resolve_fully()` resolves `List(Var(X))` → `List(int)` downstream.

2. **Per-body-exit validation pass emitting E2005** (REVISED round 2 per TPR-04-001-codex-r2 and cascading changes):

   **Do NOT validate at `bind_pattern` time.** At that point, later statements in the same block have not yet run, so downstream constraints (`push(10)` linking the element var) have not fired. Validating then emits E2005 for the success-case repro.

   **Correct timing**: validation runs as a sweep at the END of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) — after the full function body / method body / test body has been inferred. By body exit, all in-scope unification has fired; a `Tag::Var` still unresolved at that point is genuinely ambiguous.

   **Implementation shape**:
   - The checker records every let-binding's `(span, final_ty)` pair into a per-body side table during body inference (one vector per body).
   - At body-exit, walk the side table and invoke `validate_binding_type_resolved(engine, span, final_ty)` for each entry.
   - `validate_binding_type_resolved`:
     a. Call `pool.resolve_fully(final_ty)` first to follow `VarState::Link` chains (per `pool/accessors.rs:412-491`). Without this, resolved-but-unsubstituted vars false-positive as ambiguous.
     b. Cascade-suppress: skip if `pool.flags(resolved).contains(TypeFlags::HAS_ERROR)` — the binding is already poisoned and emitting a second diagnostic violates `typeck.md UN-4` monotonicity. Do NOT use `engine.has_errors()` — that is module-wide and silently swallows E2005 when an unrelated prior error exists.
     c. Invoke `has_unbound_var(pool, resolved, bound_vars: &FxHashSet<u32>)` — returns `Option<OffendingVar { var_id, context_desc }>`.
     d. On `Some(offender)`: `engine.push_error(TypeCheckError::ambiguous_type(span, offender.var_id, offender.context_desc))` — matches the real constructor signature at `check_error/mod.rs:236`.

   The `has_unbound_var` recursion walks the resolved type using the REAL tag-specific Pool accessors:
   - Simple containers (Tag 16..32 — List, Option, Set, Range, Iterator, DoubleEndedIterator): child via `Idx::from_raw(pool.data(idx))` per `types.md TK-1`.
   - Two-child (Map, Result): `pool.map_key(idx)` + `pool.map_value(idx)` (or `pool.result_ok(idx)` + `pool.result_err(idx)`).
   - Complex (Function, Tuple, Struct, Enum): `pool.function_params(idx)` + `pool.function_return(idx)`; `pool.tuple_elems(idx)`; `pool.struct_fields(idx)`; `pool.enum_variants(idx)`.
   - Named / Applied: `pool.applied_args(idx)` for generic args; Named unwraps to its underlying via `pool.resolve(idx)` before recursion.
   - Scheme: `pool.scheme_vars(idx)` are pushed into `bound_vars`, then recurse into `pool.scheme_body(idx)`, then vars are popped. A `Tag::Var(v)` encountered during the scheme body walk is RESOLVED iff `bound_vars.contains(&v)`; unresolved iff NOT.

3. **Scope broadened honestly** (REVISED round 2 per TPR-04-005-codex-r2):

   The validator is intentionally type-agnostic — it walks any `Tag::Var` in the resolved binding type and flags it when not scheme-bound. The spec at `14-expressions.md:1224-1228` mandates `[]` ambiguity rejection; `{}` and `Set<T>` ambiguity being rejected by the SAME mechanism is incidental (the mechanism does not special-case lists). This is architecturally correct per `impl-hygiene.md §SSOT` — special-casing by constructor tag would be a LEAK:scattered-knowledge hack.

   The TDD matrix (§2) focuses on `[]` as the spec-sanctioned case with ONE documentation-regression test for `{}` showing that the validator catches map ambiguity too (with a note that any future spec expansion is a SEPARATE proposal, not scope creep here). Sets are not tested because `Set<T>()` is not valid Ori surface syntax (verified: `infer_empty_set()` does not exist).

4. **Recursive negative pin** (per TPR-04-002-codex round 1): use `contains_var` (modeled after `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`) to walk the repro's typed IR recursively and assert no unresolved Vars at any depth.

5. **Test updates broadened** (per TPR-04-005-codex round 1): audit includes `[].iter()`, `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)` in `double_ended_methods.ori` lines 35, 84, 133, 174 — not just `let name = []` bindings.

6. **Algorithmic DRY follow-up**: file separately as BUG-04-{next} per TPR-04-003-gemini round 1. Consolidation of the three let-binding sites into `bind_local_let` is out of scope for this fix.

7. **Positive tests** (REVISED round 2 per TPR-04-006-codex-r2):
   - Repro: `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via AOT.
   - Lambda let-polymorphism (TYPE-SAFE pin per round 2): `let id = x -> x; let a = id(1); let b = id("hello")` — the scheme `∀α. (α) -> α` is instantiated twice with different concrete types; both bindings type-check and neither is ambiguous. (Round-2 rejected the prior pin `let b = id([])` because `[]` alone provides no element-type context, so E2005 correctly fires on `b` — that pin would FAIL post-fix.)
   - Lambda polymorphism with annotated empty list: `let id = x -> x; let b: [int] = id([])` — `b`'s annotation resolves the ambiguity; `id` stays polymorphic. Added per TPR-04-006-codex-r2 to specifically test the "can I still pass `[]` through a polymorphic lambda" case with the annotation resolving ambiguity.
   - Capturing lambda: unchanged existing behavior.

8. **Negative pins**:
   - `let xs = []; xs.len()` — E2005 at check time (not codegen).
   - `let x = []; unknown_fn(x)` — only `UnknownIdent` error fires, NOT spurious E2005 (cascade-suppression pin per TPR-04-003-codex round 1 — round-2 tightened to rely ONLY on `TypeFlags::HAS_ERROR`, not module-wide `engine.has_errors()`).
   - Regression pin: recursive `contains_var` walk of repro's typed IR shows no unresolved Vars.
   - Generic-function interaction (NEW round 2 per TPR-04-005-gemini-r2): `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — E2005 on `xs` because no downstream constraint on the element type even after the generic call.

### Round 2 — Post-Investigation (2026-04-16)

**Why a second consensus round ran.** After §3.1/§3.2/§3.3/§3.4/§3.6 shipped (four commits between `b1f2c354` and `65b3aff4`), `ori check` on the repro cleared cleanly, but `ori build` still failed with `unresolved type variable at codegen — type inference bug idx=Idx(96)`. Investigation (see `§0 POST-INVESTIGATION REWRITE`) traced the residual leak to `resolve_builtin_method` at `compiler/ori_types/src/infer/expr/methods/mod.rs:53-86`, which never unifies call-site argument types with method param types. The round-1 approach addressed generalization policy + body-exit validation — both correct and shipped — but the root cause lives one layer deeper, inside the builtin method-call path itself.

**tp-help run scratch dir**: `/tmp/ori-tpr-as5XcVA2` (launched 2026-04-16 13:30 EDT, codex walltime ~370s, gemini walltime ~900s — codex summary at `output.md` lines 2-22, gemini summary at lines 24-48).

**Pre-consensus question posed to reviewers**: four candidate approaches — (A) add arg-param unification inside `resolve_builtin_method` consuming `MethodDef.params` metadata; (B) rewire builtin dispatch through the unified `MethodRegistry` that `typeck.md §RG-3` names as the endgame; (C) delete `unify_higher_order_constraints` and replace with generic signature instantiation; (D) wire `validate_body_types` first and accept ~800 concurrent spec-test failures while converging on the root cause.

**Codex summary (hybrid: A now, B later)**:
- `A` is the correct immediate fix IF done in the canonical home. The real `GAP` is that `resolve_builtin_method` returns only a type — it never checks arity AND never does arg-param unification. `method_call.rs:28-42` / `method_call.rs:70-84` infer args into `arg_types: Vec<Idx>` but then only call `unify_higher_order_constraints` (a method-name switch) and return without any signature check.
- `A` is sufficient for BUG-04-074 IF executed against existing `MethodDef.params` data. It is NOT "just symptom treatment" — consuming the registry's signature data at builtin dispatch IS using the current SSOT. `ReturnTag` is explicitly documented as valid for parameter positions (`compiler/ori_registry/src/tags/return_tag.rs:8`); `MethodDef.params: &'static [ParamDef]` with `ParamDef { name, ty: ReturnTag, ownership }` already exists at `compiler/ori_registry/src/method/mod.rs:14-23,47`. NO new metadata type is required; NO `ParamTag` invention; NO algorithmic duplication.
- `A` leaves the `RG-3` two-path split in place and does not eliminate closure-param special-casing (builtin higher-order params are `ReturnTag::Fresh` in the registry — too weak to express `(T) -> U` shape). Higher-order shape encoding graduates to `B`.
- `B` is the endgame per `typeck.md §RG-3`, but not as "register builtins in the current `TraitRegistry`" — that registry is keyed by exact `self_type: Idx`, so it cannot host generic receiver families (`List<T>`, `Map<K,V>`) without more matching infrastructure. The target is a real unified `MethodRegistry` with builtin + inherent + trait lookup behind one API.
- `C` is the wrong abstraction today (richer `MethodDef.params` encoding needed first). `D` is the wrong sequencing (turns one root `GAP` into an 800-failure regression front, violates `CLAUDE §Stabilization Discipline`'s narrow-front principle).
- Additional hazards surfaced: (1) `GAP` — builtin arity checking is entirely absent (normal calls check at `call_inference.rs:117`, impl calls check at `impl_lookup.rs:82`, builtin calls don't). (2) `GAP` — `validate_body_types` is unwired for tests, impl methods, and def-impl methods, not just functions. (3) `LEAK` — unresolved-receiver deferral at `method_call.rs:323` returns a fresh var without recording any method obligation. (4) `LEAK` — `resolve_named_type_method` at `methods/mod.rs:88` hardcodes `unwrap`/`inner`/`value`/`debug`/`to_str` outside the registries. (5) `LEAK`/`DRIFT` — downstream `ori_arc/src/rc_insert/annotate.rs:83,361` re-encodes builtin method semantics by method name and receiver type.

**Gemini summary (Approach B only, architecturally)**:
- `B` (unified dispatch via `TraitRegistry`) is the ONLY architecturally correct fix per `CLAUDE §The One Rule`. Current two-path method resolution is `LEAK:duplicated-dispatch`. Per `CLAUDE`: "effort and scope are irrelevant."
- `A` is "symptom-patching workaround that actively worsens architectural decay." Claims A requires adding `ParamTag` encoding to `MethodDef` and writing new unification logic inside `resolve_builtin_method`, creating `LEAK:algorithmic-duplication`.
- `B` cannot wait — CLAUDE forbids temporary fixes/deferrals.
- `unify_higher_order_constraints` is `LEAK:scattered-knowledge` + `DRIFT`; "must be eliminated entirely" under unified dispatch which handles higher-order params via standard generic signature instantiation.
- `validate_body_types` must be wired AFTER root-cause fix, in the SAME atomic arc (agrees with Codex on NOT D).
- `PC-2` output contract is catastrophically failing because `validate_body_types` is disconnected from codegen's actual view.

**Divergence reconciliation**:
- Gemini's architectural objection to A rests on the factual claim "`MethodDef` entirely lacks parameter signature encoding, whereas `TraitRegistry` already possesses full signature modeling." This is FALSE in the shipped code. `MethodDef.params: &[ParamDef]` exists (line 47). `ParamDef.ty: ReturnTag` (line 19) reuses the SAME `ReturnTag` vocabulary as returns. `registry_bridge::return_tag_to_idx` (line 285) already maps `ReturnTag → Idx` relative to receiver — the bridge is shape-agnostic as to whether the source was a return or a param. Consuming `MethodDef.params` at the builtin dispatch path is NOT algorithmic duplication; it is using the existing SSOT for its documented purpose.
- Codex verified the factual claim in source; Claude independently confirmed (this conversation) at `ori_registry/src/method/mod.rs:47` and `registry_bridge/mod.rs:285-372`.
- Once the factual premise is corrected, Gemini's architectural objection dissolves — A *is* the canonical home for builtin dispatch logic, and consuming the existing signature SSOT is the correct SSOT discipline, not a violation.
- Both reviewers agree on: (1) NOT wiring `validate_body_types` first + eating 800 failures (D rejected), (2) `unify_higher_order_constraints` is a LEAK needing consolidation, (3) `PC-2` producer-side enforcement is real and must land in this arc.

**Independent code verification (Claude, 2026-04-16)**:
- ✅ `compiler/ori_types/src/infer/expr/methods/mod.rs:53-86` — `resolve_builtin_method` signature confirmed: `(engine, receiver_ty, tag, method_name) -> Option<Idx>`. No args. No arity check. No param unification.
- ✅ `compiler/ori_registry/src/method/mod.rs:14-23,47` — `ParamDef` struct confirmed (`name`, `ty: ReturnTag`, `ownership`); `MethodDef.params: &'static [ParamDef]` confirmed.
- ✅ `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs:285-372` — `return_tag_to_idx(engine, receiver_ty, return_tag) -> Idx` confirmed receiver-relative and ReturnTag-exhaustive.
- ✅ `compiler/ori_types/src/infer/expr/calls/method_call.rs:28-42,70-84` — caller already has `arg_types: Vec<Idx>` before the `Return` dispatch. Plumbing into builtin is straightforward.
- ✅ `compiler/ori_types/src/infer/expr/calls/method_call.rs:164-265` — `unify_higher_order_constraints` confirmed as method-name switch over `map|flat_map|filter|any|all|find|for_each|fold|rfold`. `_ => {}` fallthrough confirms `push`/`insert`/etc. are never touched.
- ✅ `compiler/ori_types/src/infer/expr/calls/method_call.rs:321-328` — `Tag::Var` receiver deferral confirmed as fresh-var return without obligation recording.
- ✅ `compiler/ori_types/src/infer/expr/calls/call_inference.rs:117-137` — normal-call arity pattern (`call_args.len() < required_params || call_args.len() > params.len()` → `arity_mismatch_named` / `arity_mismatch`). This IS the sibling template the builtin path must mirror.

**Outcome**: Convergence via persuaded divergence — user (2026-04-16) approved **hybrid: A now, B later**. A lands in this fix arc (§3' below). B is tracked as a standalone `/create-plan` follow-up with title "Unified `MethodRegistry` — eliminate RG-3 two-path dispatch" (plan path assigned at Phase 5 closure).

---

## 2. TDD — Test Matrix (revised 2026-04-14 after Plan TPR)

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] `test_aot_empty_list_with_push_inferred_element_compiles` — the exact repro from the bug entry: `@main () -> int = { let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1 }` compiles via `ori build` and runs exit 0.

### Edge cases — multiple container types via block-statement let (the repro path per codex finding)
- [ ] `test_empty_list_push_multiple_times_resolves_to_int_via_block_let` — `let xs = []; xs = xs.push(value: 1); xs = xs.push(value: 2); xs.len() == 2`
- [ ] `test_empty_list_with_annotation_compiles_unchanged` — `let xs: [int] = []; xs = xs.push(value: 10)` — regression guard
- [ ] `test_empty_list_inferred_from_first_push_resolves_element` — `let xs = []; xs = xs.push(value: 42)` — push constrains element

### Cross-type coverage (block-statement let — repro path)
- [ ] `test_empty_list_block_let_element_int_via_push` — `let xs = []; xs = xs.push(value: 10)`
- [ ] `test_empty_list_block_let_element_str_via_push` — `let xs = []; xs = xs.push(value: "hello")`
- [ ] `test_empty_list_block_let_element_bool_via_push` — `let xs = []; xs = xs.push(value: true)`

### Cross-generalization-site coverage (CRITICAL per TPR-04-004-codex — 3 distinct paths, revised)
- [ ] `test_empty_list_standalone_let_expr_routes_through_infer_let` — Rust unit test in `ori_types/src/infer/expr/tests.rs` that constructs a standalone `ExprKind::Let` AST node (via parser surface `@test () -> void = let x = 1;` confirmed in `ori_parse/src/tests/parser.rs:130-167, 1703-1730`) and asserts the dispatch goes through `infer_let`, NOT `infer_block`. The prior plan's wording "let xs = [] in xs.push(value: 10)" does NOT route through `infer_let` — block-statement let routes through `infer_block`. This test MUST force the actual `infer_let` dispatch.
- [ ] `test_empty_list_try_block_let_compiles` — covers `sequences.rs:247` path — inside a `try { let xs = ...; ... }` block

### Lambda let-polymorphism preservation (must still work after fix)
- [ ] `test_lambda_let_polymorphism_identity_used_at_multiple_types` — `let id = x -> x; let a = id(1); let b = id("hello"); ...` — must continue to work
- [ ] `test_lambda_let_polymorphism_pair` — `let pair = x -> y -> (x, y); pair(1)(true); pair("a")(1.0)` — generalize should fire for lambda
- [ ] `test_capturing_lambda_no_generalize_regression` — non-capturing lambda generalizes, capturing lambda does not (existing behavior at `blocks.rs:79-89` preserved)

### Ambiguous type error emission (revised per TPR-04-006-codex: scope narrowed to lists only)
- [ ] `test_empty_list_ambiguous_no_constraint_emits_E2005` — `let xs = []; xs.len()` (no element constraint) MUST emit `E2005` at type check time (NOT codegen). Negative pin: `#compile_fail("cannot infer")` matches E2005 message.
- [ ] `test_truly_polymorphic_untyped_rejected_at_check_not_codegen` — key semantic pin: the error surfaces in `ori check` (typeck phase), not only in `ori build` (codegen phase).

**Removed** (per TPR-04-006-codex — spec doesn't mandate, implementation doesn't exist):
- ~~`test_empty_map_ambiguous_emits_E2005`~~ — spec at 14-expressions.md:1238-1240 is neutral on `let m = {}`; out of scope for this fix.
- ~~`test_empty_set_ambiguous_emits_E2005`~~ — `Set<int>()` is NOT valid Ori syntax; `infer_empty_set()` does not exist.

### Cascade suppression (NEW — per TPR-04-003-codex + TPR-04-001-gemini)
- [ ] `test_empty_list_inside_already_error_typed_expression_suppresses_E2005` — program: `let x = []; fop(x)` where `fop` is undefined. Should emit ONLY `UnknownIdent` for `fop`, NOT a cascading E2005 for the empty list `x`. Verifies typeck.md UN-4 recovery monotonicity is preserved.
- [ ] `test_empty_list_in_malformed_call_suppresses_E2005` — program with parse-level or type-level error surrounding the empty list, where the primary error is sufficient — validates cascade suppression at multiple error shapes.

### Semantic pins (only pass with the correct new behavior)
- [ ] `test_generalize_skipped_for_list_literal_init` — Rust unit test in `ori_types`: construct an `infer_block` scenario with `StmtKind::Let { init: List(empty) }`, assert that the stored `expr_types[init_id]` resolves via `pool.resolve_fully()` to `List(int)` after a subsequent `push(value: 10)` unifies the element. If generalization were still firing, the var would be `Generalized` and resolution would fail.
- [ ] `test_generalize_fires_for_lambda_literal_init` — Rust unit test: same scenario but with `StmtKind::Let { init: Lambda { ... } }`, assert that generalization DOES fire (scheme is produced).

### Negative pins (reject the old broken behavior — revised per TPR-04-002-codex)
- [ ] `test_no_unresolved_var_in_repro_expr_types_recursive` — Rust integration test using a `contains_var` helper (modeled after `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`) that walks `expr_types` RECURSIVELY. For the repro program, assert NO `expr_type` Idx contains `Tag::Var` at any depth (top-level OR nested inside container types). Shallow check is insufficient — the bug shape is `List(Var(X))` where top is `List`. This test MUST fail against current code.
- [ ] `test_ambiguous_empty_list_rejected_at_check_not_codegen` — see above; pins that the spec-mandated error surfaces in typeck, not codegen.

### Lambda polymorphism regression pins (REVISED round 2)
- [ ] `test_polymorphic_lambda_instantiated_at_multiple_concrete_types_passes` — corrected pin per TPR-04-006-codex-r2: `let id = x -> x; let a = id(1); let b = id("hello")` — both instantiations concrete; neither ambiguous. If generalization still fires for lambdas after the fix, this test passes; if the fix over-reaches and disables lambda generalization, this test fails.
- [ ] `test_polymorphic_lambda_with_annotated_empty_list_resolves_cleanly` — NEW round 2: `let id = x -> x; let b: [int] = id([])` — verifies polymorphism survives AND the annotation on `b` resolves the ambiguity. Without the annotation, E2005 correctly fires — that's the negative case tested below.
- [ ] `test_polymorphic_lambda_captures_outer_ambiguous_empty_rejected` — important edge case: `{ let empty = []; let f = () -> empty; f() }` — the outer `empty` is ambiguous (len never constrains). After fix, E2005 on `empty`, because `empty` is not a lambda binding. The closure `f` capturing `empty` doesn't change `empty`'s ambiguity.

### Generic function interaction (NEW round 2 — per TPR-04-005-gemini-r2)
- [ ] `test_empty_list_passed_to_generic_function_emits_E2005` — `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — the generic call instantiates `id` with `List(β)` where `β` is fresh and remains unlinked (there is no downstream use that constrains the element type). At body exit, `xs : List(β)` with `β` still `Unbound` → E2005 fires with the `β` var_id in the diagnostic. Explicitly covers `tests.md §Interaction Testing`'s mandated `[] passed to generic function` cell.

### Incidental map ambiguity coverage (NEW round 2 — documentation regression test)
- [ ] `test_empty_map_ambiguous_no_constraint_emits_E2005_documentation` — `let m = {}; m.len()` — the validator is type-agnostic and catches this the same way it catches `[]`. This test DOCUMENTS the incidental coverage (spec currently neutral on `{}`); the test file's `/// doc comment` explains: "Spec at `14-expressions.md:1224-1228` mandates `[]` ambiguity rejection. `{}` ambiguity rejection rides for free because the validator walks any `Tag::Var`. Spec expansion to cover `{}` explicitly is a separate proposal."

### Cross-phase parity
- [ ] `test_empty_list_push_interpreter_and_llvm_parity` — the repro runs identically under `ori run` (interpreter) and `ori build + exec` (AOT). Dual-execution check per `canon.md §4.5`.

### Verify tests fail before fix
- [ ] All new AOT tests fail against current code (confirming they test the bug)
- [ ] All new typeck tests fail against current code OR pass trivially (document which)

### 2.arg-param — TDD Matrix Expansion (2026-04-16 post-investigation)

Added AFTER the post-investigation rewrite (§0) identified the residual root cause as missing arg-param unification inside `resolve_builtin_method`. These tests live alongside the §2 tests above; they are NOT replacements. The §2 tests exercise generalization-policy + body-exit validation (shipped); these exercise arg-param unification at the builtin dispatch path (new, §3').

#### Primary semantic pin — unifies Var(X) from arg
- [ ] `test_push_value_unifies_element_var_with_arg_type` — Rust unit in `methods/tests.rs`. Construct `List(Var(X))` receiver; call `resolve_builtin_method_with_args(engine, receiver, List, "push", &[Idx::INT])`; assert `pool.resolve_fully(Var(X)) == Idx::INT` via `engine.resolve`. This test MUST fail against current code (arg-param unification doesn't exist yet).

#### Cross-type coverage (all 4 primitive arg types)
- [ ] `test_push_int_element_resolves_list_of_int` — `let xs = []; xs = xs.push(value: 10); xs.len()` compiles via AOT (the ORIGINAL repro).
- [ ] `test_push_str_element_resolves_list_of_str` — `let xs = []; xs = xs.push(value: "hello"); xs.len()` compiles.
- [ ] `test_push_bool_element_resolves_list_of_bool` — `let xs = []; xs = xs.push(value: true); xs.len()` compiles.
- [ ] `test_push_float_element_resolves_list_of_float` — `let xs = []; xs = xs.push(value: 3.14); xs.len()` compiles.

#### Cross-method coverage (List mutators beyond push)
- [ ] `test_insert_unifies_element_var` — `let xs = []; xs = xs.insert(index: 0, value: 42); xs.len()` compiles.
- [ ] `test_remove_does_not_over-unify` — `let xs: [int] = [1, 2, 3]; let y = xs.remove(index: 0)` — `y: int` (arg `index: int` unifies with param `index: int`; no spurious constraint on List's element).

#### Cross-container coverage (other containers with mutators)
- [ ] `test_set_add_unifies_element_var` — if applicable per registry (check `ori_registry::find_method(Set, "add")` exists); otherwise skip with `#skip("add not in Set registry")`.
- [ ] `test_map_insert_unifies_key_and_value_vars` — `let m = {}; m = m.insert(key: "k", value: 1); m.len()` compiles — unifies BOTH `Var(K)` with `Idx::STR` AND `Var(V)` with `Idx::INT` in a single call.

#### Arity checking (NEW gap — Codex-cited)
- [ ] `test_push_with_zero_args_rejected_with_arity_error` — `let xs: [int] = []; xs.push()` — must emit `E2004` (arity mismatch) at typeck, NOT silently return `List<int>`.
- [ ] `test_push_with_two_args_rejected_with_arity_error` — `let xs: [int] = []; xs.push(1, 2)` — must emit `E2004`.
- [ ] `test_len_with_extra_arg_rejected_with_arity_error` — `let xs: [int] = []; xs.len(5)` — must emit `E2004` (`len` is a zero-arg method).

#### Higher-order preservation (unify_higher_order_constraints still works)
- [ ] `test_map_closure_param_type_unifies_with_iterator_element` — `let xs = [1, 2, 3]; xs.iter().map(r -> r + 1).collect()` still type-checks and produces `List(int)`. This MUST continue to pass after `unify_higher_order_constraints` is integrated into the builtin path; regression indicator if the integration breaks it.
- [ ] `test_filter_closure_param_type_unifies_with_iterator_element` — `let xs = [1, 2, 3]; xs.iter().filter(r -> r > 1).collect()` still type-checks.
- [ ] `test_fold_accumulator_and_return_unify` — `xs.iter().fold(0, (acc, x) -> acc + x)` still returns `int`.

#### Integrated with shipped validator
- [ ] `test_empty_list_no_constraint_still_emits_E2005` — `let xs = []; xs.len()` (no element-binding call) — the §3.3-shipped body-exit validator catches this. Must continue to emit E2005. This guards against the arg-param unification fix accidentally masking the ambiguous case by spuriously linking `Var(X)` via some other path.
- [ ] `test_empty_list_with_push_no_longer_emits_E2005` — `let xs = []; xs = xs.push(value: 10); xs.len()` — this case PREVIOUSLY would emit E2005 under strict shipped validator behavior, but AFTER arg-param unification lands, `Var(X)` resolves to `int` at body exit and the validator's walk finds no unresolved vars. Semantic pin for the new behavior.

#### Cross-phase parity (AOT parity for arg-param cases)
- [ ] `test_push_int_interpreter_and_llvm_parity` — `ori run` and `ori build + exec` produce identical output for the repro and all 4 primitive element types.

#### Negative pin — reject old broken behavior
- [ ] `test_resolve_builtin_method_without_args_deprecated_or_returns_without_unification` — if the new function is `resolve_builtin_method_with_args`, the old `resolve_builtin_method(engine, recv, tag, name)` either (a) is removed entirely, or (b) is preserved but documented as "does NOT unify args — callers MUST also call `unify_builtin_args` separately". If (a), grep confirms no remaining callers outside `method_call.rs`; if (b), a doc-comment integration test asserts the warning is present.

#### Cross-body-pass validator wiring (Codex gap #2)
- [ ] `test_validate_body_types_runs_in_test_body_pass` — a `@t tests @foo () -> void = { let xs = []; xs.len() }` test body emits E2005 via `check_test_bodies` (pass 3 in `CK-1`). Currently the validator is unwired for test bodies; this MUST fail until §3.4' lands.
- [ ] `test_validate_body_types_runs_in_impl_method_body_pass` — an `impl Foo { @m (self) -> void = { let xs = []; xs.len() } }` method body emits E2005 via `check_impl_bodies` (pass 4). MUST fail pre-fix.
- [ ] `test_validate_body_types_runs_in_def_impl_method_body_pass` — a `def impl Foo { @m (self) -> void = { let xs = []; xs.len() } }` method body emits E2005 via `check_def_impl_bodies` (pass 5). MUST fail pre-fix.

#### Verify tests fail before fix (expansion)
- [ ] All new `test_push_*`/`test_insert_*`/`test_map_insert_*` tests fail against current code (confirming they test the arg-param unification gap).
- [ ] All arity-check tests fail against current code (confirming the builtin arity gap).
- [ ] All body-pass-wiring tests fail against current code (confirming the validator is unwired for tests/impl/def-impl).

### 2.arg-param.R5 — Round 5 TPR revision test cells (2026-04-16)

Added to cover the Round 5 plan revisions (§3.1'.a, §3.2'.a, §3.4'.ii) that promoted follow-ups into shipped prerequisites. Each test corresponds to a specific reviewer finding.

#### Correlated-Fresh coverage (§3.1'.a — per TPR-05-codex-F1)
- [ ] `test_option_ok_or_unifies_err_param_with_result_err_type` — `let opt: Option<int> = Some(1); let r = opt.ok_or(err: "boom")` — `r: Result<int, str>` post-fix. Rust unit in `methods/tests.rs`: construct `Option<int>` receiver, call resolver with method "ok_or" and `arg_types: [Idx::STR]`, assert returned `Idx` is `Result(int, str)` (not `Result(int, Var(Unbound))`).
- [ ] `test_option_ok_or_with_unbound_err_arg_still_resolves_if_later_constrained` — `let opt: Option<int> = Some(1); let err = something_typed_as_str; let r = opt.ok_or(err: err)` — if `something_typed_as_str` has type `str`, `r: Result<int, str>`. Exercises correlation when arg is itself inferred.
- [ ] `test_list_zip_preserves_element_var_correlation` — if `zip` is in the registry with correlated Fresh (check registry), similar pattern.

#### Tag::Var receiver obligation-table coverage (§3.2'.a — per TPR-05-codex-F4, TPR-05-gemini-F3)
- [ ] `test_tag_var_receiver_method_call_replays_after_link` — Rust unit: build inference scenario where `let x = some_expr; x.push(value: 10)` has `some_expr: Var(X)`. Call the new `resolve_receiver_for_dispatch`, confirm an obligation was pushed, then unify `Var(X) := [int]`, confirm replay fires and the method call's stored ret_ty is `[int]`, not the original fresh var.
- [ ] `test_tag_var_receiver_unlinked_at_body_exit_emits_E2005` — `@main () -> int = { let x = something_never_linked; x.push(10); 0 }` — body-exit validator finds the obligation's `ret_ty` is still unbound, emits E2005.
- [ ] `test_tag_var_receiver_poisoned_cascade_suppressed` — if the receiver var acquires `HAS_ERROR` via some downstream unification failure, the obligation's ret_ty should NOT emit a second E2005 (cascade suppression per `ER-4`).

#### Validator scope mismatch — ArcFunction entry pass (§3.4'.ii — per TPR-05-gemini-F1)
- [ ] `test_arc_function_entry_validator_catches_bug_04_074_repro` — run `ori build` on the exact BUG-04-074 repro with ONLY §3.4'.ii active (no §3.1'). Assert the new `validate_arc_function_types` catches the unresolved `Var(X)` in `arc_fn.var_types` at ArcFunction lowering entry and emits E2005 BEFORE codegen tries and fails. This test documents that §3.4'.ii provides defense-in-depth: if §3.1' arg-param unification ever regresses, the second validator pass catches it as a clear typeck-phase error instead of a cryptic codegen crash.
- [ ] `test_arc_function_entry_validator_exempts_generic_body_scheme_vars` — a generic body `@id<T> (x: T) -> T = x` has `arc_fn.var_types` containing the scheme-bound `Var(T)`; validator exempts per `scheme_var_ids`. Confirms no false E2005 on legitimate generic parametricity.

#### §3.4'.iii pass-wiring coverage (Codex F2 expansion)
- [ ] `test_validate_body_types_runs_in_pass_2_function_body` — moved from §2.arg-param above to this revision: explicitly verify pass 2 has the hook (audit confirmation test). If the `3.4'.i` audit finds pass 2 already has the hook, this test documents that. If not, it's the regression pin for the fix.

#### Test-attribute interaction coverage (§3.4'.iv — per TPR-05-gemini-F2)
- [ ] `test_skip_attribute_does_not_bypass_validator` — `@t tests @foo #skip("reason") @target () -> void = { let xs = []; xs.len() }` — E2005 fires, skip IS blocked per CLAUDE.md `#skip` rule. Positive semantic pin for 3.4'.iv case 1.
- [ ] `test_compile_fail_keyed_form_ignores_incidental_E2005` — `@t tests @foo #compile_fail(code: "E1234") @target () -> void = { /* body that hits E1234 AND has let xs = []; xs.len() as noise */ }` — test passes because `code: "E1234"` matches one of the emitted codes. Covers case 2.
- [ ] `test_ambiguous_empty_container_in_previously_passing_test_now_emits_E2005` — a regression pin for case 3: the validator catches body-level ambiguity that previously was silenced because the ambiguous var was never used downstream.

#### Pre-rollout audit coverage (§3.4'.v — per TPR-05-codex-F3)
- [ ] `test_double_ended_iterator_ori_line_24_annotated` — manual audit of `tests/spec/traits/iterator/double_ended.ori:24` found `let result = []` without element constraint. After 3.4'.v audit, the file has `let result: [int] = []` (or equivalent) and continues to pass. Regression pin against future un-annotation.

#### Verify Round 5 tests fail before fix
- [ ] `test_option_ok_or_*` tests fail against current code (no correlated-Fresh handling exists).
- [ ] `test_tag_var_receiver_*` tests fail against current code (no obligation table exists).
- [ ] `test_arc_function_entry_validator_*` tests fail against current code (no second validator pass exists).
- [ ] `test_skip_attribute_does_not_bypass_validator` PASSES against current code but for the WRONG reason — the validator isn't wired into pass 3, so no E2005 fires. Post-3.4'.iii it continues passing but for the RIGHT reason — this is a semantic-pin inversion, document accordingly.

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — severity `high` AND complexity-elevated subsystems (`ori_types` — type inference, `ori_llvm` — codegen).

*To be filled after Plan TPR runs in Phase 2.5.*

---

## 3. Implementation

> **HISTORICAL (SHIPPED) — see §3' below for the active plan.** §3.1 through §3.7 describe the 2026-04-14 plan (should_generalize + validate_body_types + wiring) that shipped in commits `b1f2c354` → `3069f3e6` → `a9a0de43` → `65b3aff4`. These components are all in the main branch and work correctly on their own scope. They DID NOT resolve the codegen failure because the residual root cause is at a different layer (see §0 POST-INVESTIGATION REWRITE + §1.5 Round 2). The active implementation plan is §3' immediately after §3.7.

Four coordinated components per consensus — all in `ori_types` crate:

### 3.1 Extract `should_generalize` helper (SSOT policy)

Add a shared policy function in `compiler/ori_types/src/infer/expr/blocks.rs` (or a new `generalization_policy.rs` module if cleaner):

```rust
/// Determines whether a let-binding initializer should be generalized
/// for let-polymorphism. Uses AST-based Value Restriction: only lambdas
/// are generalized; all other initializers are bound monomorphically.
///
/// This is a critical phase-contract enforcement: generalizing non-lambda
/// initializers (e.g., empty collection literals) leaves `Tag::Var` with
/// `VarState::Generalized` in `expr_types`, which violates typeck PC-2
/// (no Tag::Var in typed IR output) and causes codegen failures per
/// codegen-rules.md TR-2.
///
/// Non-capturing lambdas are generalized (preserves `let id = x -> x`
/// polymorphism). Capturing lambdas are NOT generalized (existing
/// behavior at `blocks.rs:79-89` — capturing closures cannot be
/// monomorphized by AOT codegen).
///
/// Fix: BUG-04-074 (plans/bug-tracker/fix-BUG-04-074.md)
/// Spec: docs/ori_lang/v2026/spec/14-expressions.md §14.17.1 (empty lists require type context)
pub(super) fn should_generalize(arena: &ExprArena, init: ExprId) -> bool {
    match &arena.get_expr(init).kind {
        ExprKind::Lambda { params, body, .. } => {
            let param_names: Vec<Name> =
                arena.get_params(*params).iter().map(|p| p.name).collect();
            !body_captures_outer(arena, *body, &param_names)
        }
        _ => false,
    }
}
```

### 3.2 Apply the helper at all 3 generalization sites

**Site A — `infer_block` block-statement let** at `compiler/ori_types/src/infer/expr/blocks.rs:79-89`:

```rust
// BEFORE (current code):
if let ExprKind::Lambda { params, body, .. } = &arena.get_expr(*init).kind {
    let param_names: Vec<Name> =
        arena.get_params(*params).iter().map(|p| p.name).collect();
    if body_captures_outer(arena, *body, &param_names) {
        init_ty
    } else {
        engine.generalize(init_ty)
    }
} else {
    engine.generalize(init_ty)  // ← BUG: generalizes non-lambda initializers
}

// AFTER:
if should_generalize(arena, *init) {
    engine.generalize(init_ty)
} else {
    init_ty
}
```

**Site B — `infer_let` standalone let expression** at `compiler/ori_types/src/infer/expr/blocks.rs:116-168`:

```rust
// BEFORE:
engine.generalize(init_ty)

// AFTER:
if should_generalize(arena, init) {
    engine.generalize(init_ty)
} else {
    init_ty
}
```

Note: `infer_let` already has capture-detection at lines 159-163; the new helper subsumes that check.

**Site C — `sequences.rs` try-block let** at `compiler/ori_types/src/infer/expr/sequences.rs:243-247`:

```rust
// BEFORE:
let bound_ty = unwrap_result_or_option(engine, init_ty);
engine.generalize(bound_ty)

// AFTER:
let bound_ty = unwrap_result_or_option(engine, init_ty);
if should_generalize(arena, *init) {
    engine.generalize(bound_ty)
} else {
    bound_ty
}
```

### 3.3 Per-body-exit validation pass (E2005 emission — REVISED 2026-04-14 ROUND 3)

**CRITICAL round-2 revision** (preserved): do NOT validate at `bind_pattern` time. That is too early — in `infer_block` (`blocks.rs:22-97`), later statements in the same block have not yet been inferred at the moment `bind_pattern` runs for a let binding. For the success-case repro `let ages = []; ages = ages.push(value: 10); ages.len()`, the `push(10)` unification that links `ages`'s element var runs AFTER `bind_pattern` for the let has completed. Validating at `bind_pattern` time would emit E2005 for the success case.

**CRITICAL round-3 redesign** (new): the round-2 design used a `LetBindingRecord` side-table populated only at 3 generalization sites + an `FxHashSet<u32>` push/pop to track scheme-bound vars. Both mechanisms are unnecessary AND incomplete:
- The side-table missed non-let ambiguous expressions like `[].len()` standalone (per [TPR-04-005-codex-r3] / [TPR-04-002-gemini-r3]).
- The FxHashSet duplicates information already present in `VarState`. Per [TPR-04-001-gemini-r3], `generalize()` mutates VarState to `Generalized` in-place (`generalization.rs:47-54`); inspecting `pool.var_state(var_id)` directly is the canonical SSOT.

**Correct round-3 approach**: validate at the END of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) by sweeping ALL expression types in the body's `expr_types` map. Use `pool.var_state(var_id)` to classify each `Tag::Var` encountered. By body exit, all in-scope unification AND generalization have fired — VarState correctly reflects which vars are bound.

**Implementation shape (ROUND 4 — corrected against real InferEngine API)**:

No side-table is needed. At the end of each Bodies-group pass (in `check/bodies/mod.rs` functions `check_function_bodies`, `check_test_bodies`, `check_impl_bodies`, `check_def_impl_bodies`), after the body's `infer_*` call returns but BEFORE releasing the per-body context, invoke `validate_body_types`. The helper lives in a PUBLIC `ori_types` validator module (not the private `infer/expr/`) so that `ori_llvm`'s pre-codegen pass (§3.6) can share the `has_unbound_var` helper.

**Module location** (REVISED round 4 per TPR-04-002-codex-r4): place the validator helpers in `compiler/ori_types/src/check/validators/mod.rs` (new module) with a `pub` re-export through `ori_types/src/lib.rs`. The shipped `lib.rs` already re-exports selected top-level items (per the crate layout); add `pub use check::validators::{has_unbound_var, OffendingVar};` so `ori_llvm` can import the helper directly.

```rust
// In compiler/ori_types/src/check/validators/mod.rs (NEW public module):

use rustc_hash::{FxHashMap, FxHashSet};
use ori_ir::{ExprArena, ExprId, Span};
use crate::{Idx, Pool, Tag, TypeFlags, VarState};
use crate::infer::ExprIndex;  // re-exported; currently pub type ExprIndex = usize
use crate::type_error::check_error::TypeCheckError;

/// Sweep ALL expr_types for the body and emit E2005 for any expression
/// whose final type carries an unresolved Tag::Var (VarState::Unbound)
/// after all in-body unification AND generalization have fired.
///
/// Timing: MUST run at body exit (after the body's infer_* completes).
/// At that point, generalize() has run for every let-binding, so vars
/// that ARE bound carry VarState::Generalized; vars that AREN'T bound
/// remain VarState::Unbound — the unambiguous signal of "couldn't infer."
///
/// Scope: ALL expr_types, not just let bindings. This catches non-let
/// ambiguous expressions like `[].len()` standalone, which the round-2
/// LetBindingRecord side-table missed (TPR-04-005-codex-r3 +
/// TPR-04-002-gemini-r3).
///
/// Determinism: iterate sorted entries to ensure a stable diagnostic
/// selection when multiple expressions share an ambiguous type Idx.
/// FxHashMap iteration is non-deterministic (impl-hygiene.md §Pass
/// determinism); sorting by ExprIndex fixes which expression's span
/// gets the diagnostic (TPR-04-004-codex-r4 + TPR-04-002-gemini-r4).
///
/// Cascade suppression: skip when the expression's resolved type carries
/// TypeFlags::HAS_ERROR. Per typeck.md UN-4, Tag::Error unifies with
/// anything silently; emitting a second diagnostic violates recovery
/// monotonicity. Per-type local gate, NOT module-wide engine.has_errors().
///
/// Fix: BUG-04-074
pub fn validate_body_types(
    pool: &Pool,
    arena: &ExprArena,
    body_expr_types: &FxHashMap<ExprIndex, Idx>,
    record_error: &mut dyn FnMut(TypeCheckError),
) {
    // Deduplicate by resolved Idx: many expressions share the same type
    // (e.g., every int literal hits Idx::INT). Dedupe avoids emitting N
    // identical diagnostics.
    let mut seen: FxHashSet<Idx> = FxHashSet::default();

    // Collect sorted entries for DETERMINISTIC diagnostic selection.
    // FxHashMap iteration order is non-deterministic — sorting by the
    // source-stable ExprIndex key ensures the FIRST expression (lowest
    // ExprIndex) receives the diagnostic every run.
    let mut entries: Vec<(ExprIndex, Idx)> =
        body_expr_types.iter().map(|(&k, &v)| (k, v)).collect();
    entries.sort_by_key(|&(idx, _)| idx);

    for (expr_index, raw_ty) in entries {
        let resolved = pool.resolve_fully(raw_ty);

        if !seen.insert(resolved) {
            continue;
        }

        // Cascade suppression: skip if this expression's type is already poisoned.
        if pool.flags(resolved).contains(TypeFlags::HAS_ERROR) {
            continue;
        }

        if let Some(offender) = has_unbound_var(pool, resolved) {
            // ExprIndex (usize) → ExprId (u32 newtype) for arena lookup.
            let expr_id = ExprId::from_raw(expr_index as u32);
            let span = arena.get_expr(expr_id).span;
            record_error(TypeCheckError::ambiguous_type(
                span,
                offender.var_id,
                "expression type".to_string(),
            ));
        }
    }
}

pub struct OffendingVar {
    pub var_id: u32,
}

/// Recursively find the first unresolved Tag::Var in `ty` whose VarState is
/// VarState::Unbound (not Linked, Rigid, or Generalized).
///
/// Round-3 design preserved: NO bound-vars side table. VarState IS the SSOT
/// for binding status — generalize() mutates VarState in-place from Unbound
/// to Generalized when constructing a scheme (generalization.rs:47-54).
/// Just inspect var_state directly.
///
/// Round-4 corrections:
/// - Each recursive call resolves its child via pool.resolve_fully() —
///   children may have their own VarState::Link chains.
/// - Real Pool accessor names (struct_fields, enum_variants) with Vec<Idx>
///   loop binders (no `&` prefix — accessors return owned Vec, not slices).
pub fn has_unbound_var(pool: &Pool, ty: Idx) -> Option<OffendingVar> {
    // Schemes don't propagate HAS_VAR (Pool::compute_flags maps Tag::Scheme to
    // IS_SCHEME only — pool/mod.rs:651-652). So we must NOT trust the HAS_VAR
    // fast-path for schemes; recurse into the body unconditionally.
    if pool.tag(ty) == Tag::Scheme {
        let body = pool.resolve_fully(pool.scheme_body(ty));
        return has_unbound_var(pool, body);
    }

    // Fast path for non-scheme types: HAS_VAR propagation is correct.
    if !pool.flags(ty).contains(TypeFlags::HAS_VAR) {
        return None;
    }

    match pool.tag(ty) {
        Tag::Var => {
            let var_id = pool.data(ty);
            match pool.var_state(var_id) {
                VarState::Unbound { .. } => Some(OffendingVar { var_id }),
                // Link/Rigid/Generalized are all "resolved" in the sense
                // that they are not user-visible ambiguity. Link should
                // have been short-circuited by the caller's resolve_fully,
                // but defending here is cheap.
                VarState::Link { .. } | VarState::Rigid { .. } | VarState::Generalized { .. } => None,
            }
        }
        // BoundVar (inside a scheme body) refers to a scheme binder; resolved.
        Tag::BoundVar => None,
        // RigidVar is parametric (TK-6). Resolved at this layer.
        Tag::RigidVar => None,
        // Simple containers — child in `data` per types.md TK-1.
        // Per-child resolve_fully BEFORE recursing.
        Tag::List | Tag::Option | Tag::Set | Tag::Range
        | Tag::Iterator | Tag::DoubleEndedIterator | Tag::Channel => {
            let child = pool.resolve_fully(Idx::from_raw(pool.data(ty)));
            has_unbound_var(pool, child)
        }
        // Two-child containers.
        Tag::Map => {
            has_unbound_var(pool, pool.resolve_fully(pool.map_key(ty)))
                .or_else(|| has_unbound_var(pool, pool.resolve_fully(pool.map_value(ty))))
        }
        Tag::Result => {
            has_unbound_var(pool, pool.resolve_fully(pool.result_ok(ty)))
                .or_else(|| has_unbound_var(pool, pool.resolve_fully(pool.result_err(ty))))
        }
        // Complex types — real Pool accessor names.
        // function_params returns Vec<Idx> (not &[Idx]); drop the `&` in the
        // loop binder.
        Tag::Function => {
            for param in pool.function_params(ty) {
                if let Some(off) = has_unbound_var(pool, pool.resolve_fully(param)) {
                    return Some(off);
                }
            }
            has_unbound_var(pool, pool.resolve_fully(pool.function_return(ty)))
        }
        Tag::Tuple => {
            for elem in pool.tuple_elems(ty) {
                if let Some(off) = has_unbound_var(pool, pool.resolve_fully(elem)) {
                    return Some(off);
                }
            }
            None
        }
        // struct_fields returns Vec<(Name, Idx)> — destructure the tuple.
        Tag::Struct => {
            for (_name, field_ty) in pool.struct_fields(ty) {
                if let Some(off) = has_unbound_var(pool, pool.resolve_fully(field_ty)) {
                    return Some(off);
                }
            }
            None
        }
        // enum_variants returns Vec<(Name, Vec<Idx>)> — nested iteration.
        Tag::Enum => {
            for (_name, payloads) in pool.enum_variants(ty) {
                for payload_ty in payloads {
                    if let Some(off) = has_unbound_var(pool, pool.resolve_fully(payload_ty)) {
                        return Some(off);
                    }
                }
            }
            None
        }
        Tag::Applied => {
            for arg in pool.applied_args(ty) {
                if let Some(off) = has_unbound_var(pool, pool.resolve_fully(arg)) {
                    return Some(off);
                }
            }
            None
        }
        // Named resolves via registry; unresolved here means an earlier
        // diagnostic already fired. Skip silently (cascade per UN-4).
        Tag::Named | Tag::Alias => None,
        // Projection should have been normalized by unify (UN-8) or flagged
        // earlier. Skip silently.
        Tag::Projection => None,
        // Primitives, Error, Unit, Never, SelfType (substituted), Infer
        // (replaced at CK-3), and Scheme (handled above) — none reach here
        // with unresolved vars.
        _ => None,
    }
}
```

**Call-site shape** (in `check/bodies/mod.rs`):

```rust
// In check_function_bodies / check_test_bodies / check_impl_bodies /
// check_def_impl_bodies, after the body's infer_* call returns:

crate::check::validators::validate_body_types(
    engine.pool(),
    arena,
    engine.expr_types(),
    &mut |err| engine.push_error(err),
);
```

The `InferEngine` exposes `pool()`, `expr_types()` (returning `&FxHashMap<ExprIndex, Idx>`), and `push_error(TypeCheckError)`. The closure form `&mut dyn FnMut(TypeCheckError)` is used because `validate_body_types` must not hold a mutable borrow of the full engine while walking the pool (which is inside the engine). The closure captures `engine` and calls `push_error` only when an error is produced.

**Key differences across all rounds**:

*From round-2 pseudocode (removed in round 3)*:
1. `LetBindingRecord` side-table — GONE. Sweeps ALL `expr_types`; catches non-let ambiguities.
2. `FxHashSet<u32>` push/pop for scheme vars — GONE. `pool.var_state(var_id)` IS the SSOT.
3. Top-level-only `resolve_fully` — REPLACED by per-child `resolve_fully` at every recursive step.
4. Fictional `pool.struct_field_types` / `pool.enum_variant_payloads` — REPLACED with real accessors.

*New in round 4 (corrections to round-3 design)*:
5. **Signature: `FxHashMap<ExprIndex, Idx>`, NOT `FxHashMap<ExprId, Idx>`.** `ExprIndex = usize` per `infer/mod.rs:56`. Per [TPR-04-003-codex-r4] + [TPR-04-001-gemini-r4].
6. **Span retrieval via `arena.get_expr(ExprId::from_raw(idx as u32)).span`**, NOT via a non-existent `engine.expr_span()`. The validator takes `arena: &ExprArena` as a parameter. Per [TPR-04-003-codex-r4] + [TPR-04-001-gemini-r4].
7. **Sort entries by `ExprIndex` before iteration** for DETERMINISM. `FxHashMap` iteration order is non-deterministic per `impl-hygiene.md §Pass determinism`; sorting ensures the lowest-ExprIndex (earliest-in-source) expression gets the diagnostic every run. Per [TPR-04-004-codex-r4] + [TPR-04-002-gemini-r4].
8. **Loop binder `for param in ...`**, NOT `for &param in ...`. `pool.function_params(ty)`, `pool.tuple_elems(ty)`, `pool.applied_args(ty)` return owned `Vec<Idx>`, not slices. `&param` would be a compilation error. Per [TPR-04-004-gemini-r4].
9. **Public validator module, NOT private `infer/expr/generalization_policy.rs`.** The helper moves to `compiler/ori_types/src/check/validators/mod.rs` with `pub` re-export through `lib.rs` so `ori_llvm` can consume it for pre-codegen validation (§3.6). Per [TPR-04-002-codex-r4].
10. **Call-site via closure `&mut dyn FnMut(TypeCheckError)`**, NOT a direct `engine.push_error()` inside the validator. The validator must not hold a mutable borrow on the full engine while walking the pool; closure-based error recording sidesteps the borrow constraint.

*Preserved architectural properties*:
- Lambda polymorphism is preserved by construction (VarState::Generalized after `generalize()` runs).
- Cascade gate: per-resolved-type `TypeFlags::HAS_ERROR` only; no module-wide gate.
- Deduplication via `seen: FxHashSet<Idx>` to avoid N diagnostics for the same ambiguous type.

**Helper module location** (REVISED round 4):
- Place `validate_body_types` + `has_unbound_var` + `OffendingVar` in `compiler/ori_types/src/check/validators/mod.rs` (new module).
- `should_generalize` (§3.1) stays in `compiler/ori_types/src/infer/expr/generalization_policy.rs` — it's AST-based and internal to the inference layer; no downstream consumer outside `ori_types`.
- Add `pub use check::validators::{has_unbound_var, OffendingVar};` to `compiler/ori_types/src/lib.rs` so `ori_llvm` can import for §3.6.

**Pool accessor verification** — VERIFIED IN ROUND 3 or EARLIER:
- `pool.resolve_fully(idx) -> Idx` — `pool/accessors.rs:412-491`
- `pool.struct_fields(idx) -> Vec<(Name, Idx)>` — `pool/accessors.rs:538-551`
- `pool.enum_variants(idx) -> Vec<(Name, Vec<Idx>)>` — `pool/accessors.rs:606-629`
- `pool.var_state(var_id) -> &VarState` — referenced consistently across `unify/`, `pool/`, `infer/`

VERIFICATION PENDING at implementation time (confirm via `scripts/intel-query.sh symbols "<name>" --repo ori` in Phase 3):
- `pool.scheme_body(idx) -> Idx`, `pool.scheme_vars(idx) -> &[u32]`
- `pool.function_params(idx) -> Vec<Idx>`, `pool.function_return(idx) -> Idx`
- `pool.tuple_elems(idx) -> Vec<Idx>`
- `pool.applied_args(idx) -> Vec<Idx>`
- `pool.map_key(idx) -> Idx`, `pool.map_value(idx) -> Idx`
- `pool.result_ok(idx) -> Idx`, `pool.result_err(idx) -> Idx`

Round 3 caught `struct_field_types` / `enum_variant_payloads` as fictional; round 4 must verify the remaining accessors before implementation. If any is missing OR has a different signature than assumed, either correct §3.3 or add the missing accessor as a thin wrapper with a `#[cfg(test)]` unit test in `ori_types/src/pool/tests.rs`. Missing accessors are in-scope for this fix — the natural completion of the `types.md TL-2` contract.

### 3.4 Test updates (broadened per TPR-04-005-codex)

- [ ] Audit `tests/spec/types/collections.ori` — add type annotations to any active `let empty = []` bindings. Most are already commented out per codex's investigation.
- [ ] Audit `tests/spec/traits/iterator/double_ended.ori:25-34, 66-82` — dead-local `let result = []` sites; annotate if they fail E2005.
- [ ] Audit `tests/spec/traits/iterator/double_ended.ori:167` — direct-receiver `[].iter()` form.
- [ ] Audit `tests/spec/traits/iterator/double_ended_methods.ori:35, 84, 133, 174` — direct-receiver forms: `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)`.
- [ ] Full repo sweep: `rg -n 'let \w+ = \[\];|let \$?\w+ = \[\];|\[\]\.iter\(\)|\[\]\.len\(\)|\[\]\.is_empty\(\)' tests/ library/` to discover any remaining direct-receiver or binding-site forms NOT yet listed.
- [ ] For each affected test file, decide: annotate (`let xs: [int] = []`) OR remove dead local OR mark `#compile_fail` if the test's purpose is to verify error emission.

### 3.5 Algorithmic DRY follow-up (out of scope — per TPR-04-003-gemini)

The three let-binding sites (`infer_block`, `infer_let`, `sequences.rs`) have duplicated surrounding logic (capture detection, branch on generalize, bind pattern). Consolidating into a single `bind_local_let` abstraction is a valid `LEAK:algorithmic-duplication` concern per `impl-hygiene.md` §Algorithmic DRY but is OUT OF SCOPE for this bug fix.

**Concrete tracking artifact** (required per CLAUDE.md "future improvement" rule — must not be a nebulous deferral):
- [ ] Action: file `BUG-04-{next}` via `/add-bug` at close-out of BUG-04-074 with title "Consolidate let-binding inference across infer_block/infer_let/sequences.rs into shared helper" and subsystem `ori_types`. Severity: `low` (code hygiene, not correctness).

### 3.6 Pre-codegen invariant validation (REVISED round 4 — per TPR-04-003-gemini-r4 + TPR-04-001-codex-r4)

Round-2 placed the check in `TypeInfoStore::get_or_compute_type_info`. Round-3 broadened to a pre-codegen pass using `body_expr_types`. **Round-4 caught both as architecturally wrong**:

1. **`ori_llvm` does NOT have access to typeck AST structures** ([TPR-04-003-gemini-r4]). The ARC pipeline consumes `ori_arc::ArcFunction`, which stores per-variable types in `var_types: Vec<Idx>` (see `ori_arc/src/ir/mod.rs:375-387`). The caller-typecheck `expr_types` map is gone by the time codegen runs — it was in `InferEngine`, not threaded into `ArcFunction`. The round-3 validator signature (`body_expr_types: &FxHashMap<ExprId, Idx>`) is unimplementable as written.

2. **Placement at `FunctionCompiler::compile` is TOO LATE** ([TPR-04-001-codex-r4]). The JIT pipeline calls `collect_mono_functions()` at `compiler/ori_llvm/src/evaluator/compile.rs:230-243` BEFORE any `FunctionCompiler` is constructed. `collect_mono_functions` walks mono instances and their type arguments directly. A Tag::Var in a mono instance's type args would reach name mangling and LLVM type construction before any per-function validator runs.

**Round-4 design**: split into two validation points, both using the same `has_unbound_var` helper from §3.3 (now publicly exported from `ori_types::check::validators`):

**3.6a — Pre-collect-mono validation (in `compile_all_functions`, BEFORE `collect_mono_functions`)**:

```rust
// In compiler/ori_llvm/src/evaluator/compile.rs, immediately before the
// collect_mono_functions call at line 230:

#[cfg(debug_assertions)]
validate_mono_inputs_resolved(self.pool, mono_instances, function_sigs);

let mut mono_functions = crate::monomorphize::collect_mono_functions(
    mono_instances, function_sigs, interner, self.pool,
);

// ...

#[cfg(debug_assertions)]
fn validate_mono_inputs_resolved(
    pool: &Pool,
    mono_instances: &[MonoInstance],
    function_sigs: &FunctionSigMap,
) {
    use ori_types::check::validators::has_unbound_var;

    // Every monomorphic instance's type arguments must be fully resolved —
    // they drive name mangling and LLVM type construction. A Tag::Var here
    // produces poisoned mangled names that silently alias unrelated types.
    for inst in mono_instances {
        for &ty_arg in &inst.type_args {
            let resolved = pool.resolve_fully(ty_arg);
            if let Some(off) = has_unbound_var(pool, resolved) {
                panic!(
                    "Tag::Var reached LLVM monomorphization — typeck.md PC-2 violation. \
                     mono_instance={:?}, ty_arg={:?}, var_id={}",
                    inst, resolved, off.var_id,
                );
            }
        }
    }

    // Every function signature type must be fully resolved — signatures are
    // consumed by collect_mono_functions → mangle_mono_name and by ABI
    // classification, both of which require concrete types.
    for (name, sig) in function_sigs.iter() {
        for &param_ty in &sig.param_types {
            let resolved = pool.resolve_fully(param_ty);
            if let Some(off) = has_unbound_var(pool, resolved) {
                panic!(
                    "Tag::Var in function signature reached monomorphization — typeck.md PC-2 violation. \
                     fn={:?}, param_ty={:?}, var_id={}",
                    name, resolved, off.var_id,
                );
            }
        }
        let return_resolved = pool.resolve_fully(sig.return_type);
        if let Some(off) = has_unbound_var(pool, return_resolved) {
            panic!(
                "Tag::Var in function return type reached monomorphization — typeck.md PC-2 violation. \
                 fn={:?}, return_ty={:?}, var_id={}",
                name, return_resolved, off.var_id,
            );
        }
    }
}
```

**3.6b — Per-function ArcFunction validation (BEFORE `FunctionCompiler::compile` runs for each function)**:

```rust
// In the per-function compilation loop — either at the top of
// FunctionCompiler::compile(&arc_func) or just before it's invoked.
// Uses arc_func.var_types (Vec<Idx>) — the ARC IR's SSOT for per-variable
// types, NOT the typeck body_expr_types map which is not accessible here.

#[cfg(debug_assertions)]
fn validate_arc_function_types_resolved(pool: &Pool, arc_func: &ArcFunction) {
    use ori_types::check::validators::has_unbound_var;

    // arc_func.var_types is indexed by ArcVarId::index() and contains the
    // resolved Idx for every variable in the function body. ArcFunction is
    // produced AFTER typeck, so if the typeck→ARC pipeline is sound these
    // are all fully resolved. The debug_assert catches typeck bugs that
    // let a Var slip through.
    for (var_id, &raw_ty) in arc_func.var_types.iter().enumerate() {
        let resolved = pool.resolve_fully(raw_ty);
        if let Some(off) = has_unbound_var(pool, resolved) {
            panic!(
                "Tag::Var reached LLVM codegen in ArcFunction body — typeck.md PC-2 violation. \
                 fn={:?}, var_id={}, ty={:?}, ambiguous_var_id={}",
                arc_func.name, var_id, resolved, off.var_id,
            );
        }
    }

    // Validate signature params and return as well — defense-in-depth
    // overlap with 3.6a catches any regression in the collect_mono path.
    for param in &arc_func.params {
        let resolved = pool.resolve_fully(param.ty);
        if let Some(off) = has_unbound_var(pool, resolved) {
            panic!(
                "Tag::Var in ArcFunction param — typeck.md PC-2 violation. \
                 fn={:?}, param_ty={:?}, var_id={}",
                arc_func.name, resolved, off.var_id,
            );
        }
    }
    let return_resolved = pool.resolve_fully(arc_func.return_type);
    if let Some(off) = has_unbound_var(pool, return_resolved) {
        panic!(
            "Tag::Var in ArcFunction return type — typeck.md PC-2 violation. \
             fn={:?}, return_ty={:?}, var_id={}",
            arc_func.name, return_resolved, off.var_id,
        );
    }
}
```

Cost is bounded: O(function-size) per function, in debug builds only. Release builds skip both validators entirely (`#[cfg(debug_assertions)]`).

**Defense-in-depth retained**: the per-call-site `debug_assert!` at `TypeInfoStore::get_or_compute_type_info` (round-2 location) ALSO stays. Three layers now catch different failure modes:
- **3.6a** catches Tag::Var leaks at the monomorphization input boundary (earliest possible failure point).
- **3.6b** catches Tag::Var leaks in ArcFunction body variable types (per-function failure, pinpoints offending function).
- **TypeInfoStore-local check** catches any future codegen consumer that synthesizes a Tag::Var DURING codegen (future-bug defense).

The existing release-mode `TypeInfo::Error` return path in TypeInfoStore is also retained as a final-resort production safety net. In debug builds, leaks crash loudly at the earliest of the three points they can be detected; in release builds, leaks fail gracefully with `TypeInfo::Error` rather than generating wrong LLVM IR.

### 3.7 Implementation order

1. Write all tests from §2 (including negative pins that REQUIRE E2005 emission)
2. Verify tests fail appropriately (some fail on codegen error, some pass trivially — document which)
3. Verify the Pool accessors enumerated in §3.3 exist (grep / intel-query.sh); add any missing ones in the same commit with dedicated unit tests
4. Implement §3.1 helper + §3.2 three-site replacement → verifies the main repro compiles end-to-end (typeck passes, codegen succeeds)
5. Implement §3.3 body-exit validation sweep → verifies ambiguous cases produce E2005 at check
6. Implement §3.6 consumer-side debug_assert → catches any future leak of Tag::Var to codegen
7. Apply §3.4 test updates as needed
8. Run `timeout 150 ./test-all.sh` — full suite green
9. Run `timeout 150 ./clippy-all.sh` — no warnings
10. Run `/commit-push`, then Phase 5 (TPR + hygiene)

---

## 3'. Implementation (ACTIVE — 2026-04-16 Post-Investigation Rewrite)

Five coordinated components. All land in a single atomic fix arc per the reviewer consensus on narrow-front discipline. §3.1'–§3.4' are the direct fix for BUG-04-074's residual root cause. §3.5' integrates existing scattered logic. §3.6'–§3.7' handle follow-ups discovered during investigation.

### 3.1' Extend `resolve_builtin_method` to check arity and unify arg types with method params (SSOT)

**Change the signature** of `resolve_builtin_method` in `compiler/ori_types/src/infer/expr/methods/mod.rs:53-86` from its current 4-parameter shape to an 8-parameter shape that accepts the call-site arg types and arg spans, plus an error-emission callback:

```rust
/// Resolve a built-in method call: look up in registry, check arity,
/// unify each arg type with the method's declared param type, and
/// return the method's declared return type.
///
/// `arg_types` and `arg_spans` are parallel slices for all non-named
/// arguments at the call site (or the named-arg value expressions in the
/// named-call variant). The caller MUST infer each arg's type before
/// invoking this function.
///
/// On arity mismatch: emits `E2004` and returns `Some(Idx::ERROR)` —
/// distinguishes "method exists but wrong arity" from "method not found".
/// On arg-type mismatch: emits `E2001` per mismatched slot via `engine.check_type`
/// with `ExpectedOrigin::Context { kind: ContextKind::FunctionArgument }`
/// (mirrors the impl-path pattern at `method_call.rs:89-103`).
/// Returns `None` ONLY when the method is not recognized for this type tag.
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
    named_params: Option<&[Name]>, // Some for named-arg call; None for positional
) -> Option<Idx>
```

**Body** (augments the current 5-step logic with arity + arg unification, sitting between steps 3 and 4 of the existing body):

1. (unchanged) Named/Applied routing → `resolve_named_type_method`.
2. (unchanged) Convert `tag → TypeTag` via `registry_bridge::tag_to_type_tag`.
3. (unchanged) `find_method(type_tag, method_name)` → `method_def: &MethodDef`.
4. **NEW** — arity check. Mirrors `call_inference.rs:117-137`:
   ```rust
   if arg_types.len() != method_def.params.len() {
       engine.push_error(TypeCheckError::arity_mismatch_named(
           call_span,
           format!("{}.{}", tag_display(tag), method_name),
           method_def.params.len(),
           arg_types.len(),
       ));
       return Some(Idx::ERROR);
   }
   ```
5. **NEW** — arg-param unification. For each `(i, (arg_ty, param_def))` in `arg_types.iter().zip(method_def.params.iter()).enumerate()`:
   ```rust
   // Convert the param's ReturnTag to an Idx using the SAME bridge that
   // resolves return types. ParamDef.ty: ReturnTag reuses the same vocabulary,
   // per compiler/ori_registry/src/method/mod.rs:19 and
   // compiler/ori_registry/src/tags/return_tag.rs:8 ("valid for param positions").
   let param_ty = registry_bridge::return_tag_to_idx(engine, receiver_ty, param_def.ty);
   let expected = Expected {
       ty: param_ty,
       origin: ExpectedOrigin::Context {
           span: call_span,
           kind: ContextKind::FunctionArgument {
               func_name: None,
               arg_index: i,
               param_name: named_params.and_then(|np| np.get(i).copied()),
           },
       },
   };
   let _ = engine.check_type(*arg_ty, &expected, arg_spans[i]);
   ```
6. (unchanged) `Range<float>` rejection.
7. (unchanged) Convert `return_tag` to `Idx` via `registry_bridge::return_tag_to_idx` and return.

**Critical correctness property**: Step 5 uses `registry_bridge::return_tag_to_idx` with `receiver_ty` as the context — so `ReturnTag::SelfType` resolves to `receiver_ty` (the whole `List(Var(X))`), `ReturnTag::ElementType` resolves to `Var(X)`, `ReturnTag::KeyType` / `ValueType` resolve to `Map<K,V>`'s `Var(K)`/`Var(V)` respectively. For `push(value: 10)` this makes `param_ty = Var(X)`, and `engine.check_type(Idx::INT, Expected { ty: Var(X), ... }, span)` performs `unify(Idx::INT, Var(X))`, binding `Var(X) := Idx::INT`. `List(Var(X))` now resolves to `List<int>` everywhere downstream — in the method's return (`SelfType`), in any subsequent `.push`/`.insert`/`.len` call, and in codegen.

### 3.1'.a — Correlated-Fresh + closure-Fresh carve-out (Round 5 revision per TPR-05-codex-F1, TPR-05-gemini-F4)

**Problem.** Blanket routing of `ParamDef.ty` through `return_tag_to_idx` is semantically wrong for two classes of methods where `ReturnTag::Fresh` appears at a param position:

1. **Correlated-Fresh (e.g., `Option<T>.ok_or(err: E) -> Result<T, E>`)** — `ok_or`'s `err` param has `ty: ReturnTag::Fresh` AND the method's return is `Result<T, E>` where the SAME `E` must be the `Err` ok-type. Running `return_tag_to_idx` independently on the param and on the return calls `engine.fresh_var()` TWICE, producing two distinct unification variables `Var(α)` and `Var(β)`. The call site `opt.ok_or("error")` unifies `Var(α) := Idx::STR`, but the return type is `Result<T, Var(β)>` with `Var(β)` still `Unbound`. This reproduces the original BUG-04-074 failure class under a different method. Verified: `tests/spec/types/option/ok_or.ori` exercises exactly this case per `ok_or` method semantic contract.

2. **Closure-Fresh (e.g., `List<T>.map(f: (T) -> U) -> List<U>`)** — `CLOSURE_PARAM` at `compiler/ori_registry/src/defs/params.rs:11` encodes the closure param as `ParamDef { name: "f", ty: ReturnTag::Fresh, ... }`. `return_tag_to_idx` produces a fresh `Var(α)`. The call-site arg `arg_ty` is `Function(Var, U)` — unifying `Function(Var, U) := Var(α)` binds `Var(α)` to the closure type itself, NOT to the closure's parameter type. The closure's input var stays unlinked; `unify_higher_order_constraints_impl` must still fire (as §3.3' already specifies) to propagate the source iterator's element type into the closure's input. This is not wrong per se — it's just that the arg-param unification is **vacuous** for closure-Fresh, contributing no information. `Fresh` is too weak an encoding to capture `(T) -> U` shape.

**Carve-out (shipped in this arc).** The arg-param loop in §3.1' step 5 treats `ReturnTag::Fresh` as a signal "this param needs correlation, not unification" rather than unifying blindly:

```rust
for (i, (arg_ty, param_def)) in arg_types.iter().zip(method_def.params.iter()).enumerate() {
    // Correlated/closure Fresh: skip arg-param unification here.
    // The correlation flows through return_tag_to_idx(receiver_ty, return_tag)
    // for correlated-Fresh (where Return uses the SAME ReturnTag::Fresh slot),
    // OR through unify_higher_order_constraints_impl for closure-Fresh.
    if matches!(param_def.ty, ReturnTag::Fresh) {
        continue;  // handled elsewhere
    }
    let param_ty = registry_bridge::return_tag_to_idx(engine, receiver_ty, param_def.ty);
    let expected = Expected { /* ... as in step 5 above ... */ };
    let _ = engine.check_type(*arg_ty, &expected, arg_spans[i]);
}
```

**Correlated-Fresh correctness via shared var (shipped in this arc) — Round 1 revision per TPR-05-R1-codex-F1 + TPR-05-R1-gemini-F1; Round 5 revised per TPR-05-codex-F1 + TPR-05-gemini-F1 to handle composite-Fresh return tags (`ResultOfProjectionFresh`).** `return_tag_to_idx` today allocates a new fresh_var for each `ReturnTag::Fresh` encounter (line 296 of `registry_bridge/mod.rs`: `ReturnTag::Fresh => engine.fresh_var()`). For correlated-Fresh methods, the param AND the return both need the SAME fresh_var. The correlation must also apply to **composite return tags containing a `Fresh` subpart** — notably `ReturnTag::ResultOfProjectionFresh(TypeProjection)` (`compiler/ori_registry/src/tags/return_tag.rs:75` — "Result<P, E> where P is a projection and E is fresh") used by `ok_or` (`compiler/ori_registry/src/defs/option/mod.rs:114`).

**Fresh-arity helper (single SSOT for Fresh-counting).** Introduce a helper that counts `Fresh` positions within any return tag, including composite forms:

```rust
// compiler/ori_types/src/infer/expr/methods/mod.rs (or registry_bridge module)
fn return_tag_fresh_arity(tag: ReturnTag) -> u8 {
    match tag {
        ReturnTag::Fresh => 1,
        ReturnTag::ResultOfProjectionFresh(_) => 1, // Err slot is Fresh
        // Any future composite tag with a Fresh subpart adds its arity here.
        _ => 0,
    }
}
```

`return_tag_fresh_arity(method_def.returns)` replaces the direct `matches!(returns, ReturnTag::Fresh)` check. Adding a new composite tag with `Fresh` subparts requires updating this helper — it's the single SSOT for "does this return tag consume the correlation slot?"

**Arity-of-Fresh partition determines the dispatch path.** Scan the `MethodDef` signature's `Fresh` positions (params + return) and classify the method into one of three buckets:

1. **Single-Fresh correlation (one `Fresh` in params + exactly one `Fresh` position in return, counted by `return_tag_fresh_arity` to include composites).** Examples verified against `ori_registry/src/defs/`: `Option.ok_or(err: Fresh) -> ResultOfProjectionFresh(Element)` (composite return — the `Err` slot is `Fresh`, corresponds to `err` param), `Option.unwrap_or(default: Fresh) -> Fresh` (direct return — legacy shape), and similar "carry arg into return" methods. Dispatch: use the correlation-slot sidecar. `return_tag_to_idx` accepts an `Option<&mut Option<Idx>>` parameter; when it encounters `ReturnTag::Fresh` (or the `Fresh` subpart of a composite like `ResultOfProjectionFresh`), it reads from OR allocates into the slot. Param-site and return-site share the same fresh_var.

2. **Multi-Fresh higher-order (two or more `Fresh` positions across params+return, at least one param is a closure or accumulator the existing higher-order handler knows about).** Verified against `compiler/ori_registry/src/defs/iterator/mod.rs:58-76` where `FOLD_PARAMS: [ParamDef; 2] = [ParamDef { name: "initial", ty: ReturnTag::Fresh, ... }, ParamDef { name: "op", ty: ReturnTag::Fresh, ... }]` plus `fold`'s return is also `Fresh`. This class is ALREADY correctly handled by `unify_higher_order_constraints_impl` for `fold`/`rfold` (existing match arms at `method_call.rs:228-262` unify `ret_ty := init_ty` AND `ret_ty := closure_ret` AND `closure_param[0] := ret_ty` AND `closure_param[1] := source_elem`). Dispatch: skip the correlation slot entirely for these methods; §3.3' higher-order handler does the work. Detection: a method is in this bucket iff `method_def.params` contains ≥2 `Fresh` OR the method name is in the higher-order whitelist (`map`, `filter`, `flat_map`, `fold`, `rfold`, `any`, `all`, `find`, `for_each`).

3. **Single-Fresh on param with no return correlation (one `Fresh` in params, `return_tag_fresh_arity(return) == 0`).** Example: `CLOSURE_PARAM` at `ori_registry/src/defs/params.rs:11` used by `any`/`all`/`filter`/`for_each` where the closure's return is `bool`, not `Fresh`. Dispatch: skip arg-param unification (vacuous), let §3.3' higher-order handler unify the closure's param with the source element. Same effective behavior as bucket 2 — the correlation slot is NOT involved.

4. **Multi-Fresh non-higher-order fallback (per TPR-05-gemini-F1 — theoretical category, no shipped method matches today but the bucket logic must not silently leak).** If a method has 2+ Fresh params, is NOT higher-order, and `return_tag_fresh_arity(return) != fresh_param_count`, the correlation cannot be expressed with a single slot. Dispatch: allocate an INDEPENDENT `engine.fresh_var()` per `Fresh` param, unify each arg against its own fresh var (so arg types flow into their respective params), and document the case in a `tracing::warn!` so any such registration surfaces a design-review ask. No registered method today hits this case; if one ever does, the warn fires and a follow-up plan-item tracks a richer encoding.

**Dispatch table in `resolve_builtin_method` step 5 (post-Round-5 revision)**:

```rust
let fresh_param_count: u8 = method_def.params.iter()
    .map(|p| return_tag_fresh_arity(p.ty))
    .sum();
let return_fresh_arity = return_tag_fresh_arity(method_def.returns);
let higher_order_method = HIGHER_ORDER_METHOD_NAMES.contains(&method_name);

// Correlation slot scope: one method-call resolution. Consumed by both the
// param-Fresh positions (bucket 1 & 4) and by return_tag_to_idx when it
// encounters a Fresh subpart of a composite return tag (e.g. ResultOfProjectionFresh).
let mut correlation_slot: Option<Idx> = None;

let bucket_1 = !higher_order_method
    && fresh_param_count == 1
    && return_fresh_arity == 1;

for (i, (arg_ty, param_def)) in arg_types.iter().zip(method_def.params.iter()).enumerate() {
    match param_def.ty {
        ReturnTag::Fresh if bucket_1 => {
            // Bucket 1: single-Fresh correlation. Allocate the slot and unify arg.
            let slot = *correlation_slot.get_or_insert_with(|| engine.fresh_var());
            let _ = engine.check_type(*arg_ty, &Expected { ty: slot, .. }, arg_spans[i]);
        }
        ReturnTag::Fresh if higher_order_method || fresh_param_count >= 2 && return_fresh_arity >= 1 => {
            // Bucket 2: higher-order or matched multi-Fresh — §3.3' handles.
            continue;
        }
        ReturnTag::Fresh if return_fresh_arity == 0 => {
            // Bucket 3: bare closure param, non-Fresh return — §3.3' handles.
            continue;
        }
        ReturnTag::Fresh => {
            // Bucket 4: multi-Fresh non-higher-order fallback (theoretical today).
            tracing::warn!(
                method = method_name,
                fresh_params = fresh_param_count,
                return_fresh = return_fresh_arity,
                "bucket-4 multi-Fresh non-higher-order — allocating independent fresh_var per param",
            );
            let per_param_fresh = engine.fresh_var();
            let _ = engine.check_type(*arg_ty, &Expected { ty: per_param_fresh, .. }, arg_spans[i]);
        }
        _ => {
            // Non-Fresh: blanket arg-param unification via return_tag_to_idx.
            let param_ty = registry_bridge::return_tag_to_idx(
                engine,
                receiver_ty,
                param_def.ty,
                None, // non-Fresh params do not consume the correlation slot
            );
            let _ = engine.check_type(*arg_ty, &Expected { ty: param_ty, .. }, arg_spans[i]);
        }
    }
}

// At step 7 return-tag conversion: pass the correlation slot so that
// Fresh (direct) AND Fresh subparts of composites (ResultOfProjectionFresh,
// future multi-Fresh composites) consume it.
let return_ty = match method_def.returns {
    ReturnTag::Fresh if correlation_slot.is_some() => correlation_slot.unwrap(),
    ReturnTag::Fresh => computed_returns::resolve_computed_return(engine, receiver_ty, tag, method_name),
    other => registry_bridge::return_tag_to_idx(
        engine,
        receiver_ty,
        other,
        correlation_slot.as_mut(),  // threaded in — the bridge uses it for composite-Fresh subparts
    ),
};
```

**`return_tag_to_idx` signature change** (Round 5 per TPR-05-codex-F1):

```rust
// compiler/ori_types/src/infer/expr/registry_bridge/mod.rs
pub(super) fn return_tag_to_idx(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: ReturnTag,
    correlation_slot: Option<&mut Option<Idx>>,  // NEW — consumed for Fresh subparts
) -> Idx {
    match tag {
        ReturnTag::Fresh => {
            // Direct Fresh consumes the correlation slot if provided.
            if let Some(slot) = correlation_slot {
                *slot.get_or_insert_with(|| engine.fresh_var())
            } else {
                engine.fresh_var()
            }
        }
        ReturnTag::ResultOfProjectionFresh(proj) => {
            // Composite: Ok = projection, Err = Fresh (shared slot).
            let ok_idx = project_from_receiver(engine, receiver_ty, proj);
            let err_idx = if let Some(slot) = correlation_slot {
                *slot.get_or_insert_with(|| engine.fresh_var())
            } else {
                engine.fresh_var()
            };
            engine.pool_mut().result_of(ok_idx, err_idx)
        }
        // All other non-Fresh-bearing tags: ignore the slot.
        other => /* existing tag-specific handling (projections, fixed wrappers, etc.) */,
    }
}
```

All existing callers of `return_tag_to_idx` that do NOT need correlation pass `None`. The change is additive; the sites in `method_call.rs` that produce the `Expected { ty: param_ty, .. }` for non-Fresh params (bucket `_` arm above) pass `None`.

`HIGHER_ORDER_METHOD_NAMES` is a `&[&str]` shared with §3.3's helper — single SSOT for "is this method handled by `unify_higher_order_constraints_impl`". Adding a method name there routes its Fresh params into bucket 2/3 automatically.

**Closure-Fresh via §3.3' integration (shipped in this arc).** Closure-Fresh params continue to rely on `unify_higher_order_constraints_impl` (§3.3') for the actual constraint propagation. The `continue` in the loop above prevents vacuous unification; the helper does the real work. This preserves the same behavior `map`/`filter`/`fold`/etc. have today, while making the skip explicit instead of accidental.

**Approach-B linkage.** The proper long-term fix is richer `MethodDef.params` encoding — `ReturnTag::Closure { param: Box<ReturnTag>, ret: Box<ReturnTag> }` for closure shape, and a `ReturnTag::CorrelatedFresh(slot_id)` variant so the registry data itself names the correlation. This graduates to the Approach B endgame (§3.7') and eliminates BOTH the correlation-map sidecar AND the `unify_higher_order_constraints_impl` special-case in one move.

**Ownership semantics note**: `ParamDef.ownership` is currently ignored by the type checker (it only affects ARC's borrow inference). We do NOT consume it here; ARC will read it downstream per `ori_registry/src/method/mod.rs:22` doc comment. If future work needs ownership-aware type checking (e.g., for `Never`-param edge cases), it slots in here.

### 3.2' Update `method_call.rs` call sites to thread args into builtin resolution

`resolve_receiver_and_builtin` at `method_call.rs:296` currently calls `resolve_builtin_method` BEFORE args are inferred. We cannot move arg inference up into this function without refactoring `ReceiverDispatch` substantially (it would force positional-and-named to converge into one vector shape). Simpler + architecturally correct: keep the current two-phase structure, but DEFER the builtin resolution until args are inferred.

**Refactor plan**:
- Split `resolve_receiver_and_builtin` into `resolve_receiver_for_dispatch` (returns resolved receiver Idx + tag, handles Error/Scheme/Tag::Var/Range<float>/DEI rejection) and a new helper `check_builtin_and_dispatch` called AFTER arg inference.
- `infer_method_call` (positional) at `method_call.rs:20`:
  1. Call `resolve_receiver_for_dispatch(engine, arena, receiver, method, span)` → returns `ReceiverOutcome::{Error(Idx::ERROR), Deferred(fresh_var), Resolved { receiver_ty, tag }}`.
  2. On Error/Deferred: still infer all args for side-effect-only typing (preserves current diagnostic discovery), then return.
  3. On Resolved: infer args into `arg_types: Vec<Idx>` and `arg_spans: Vec<Span>`.
  4. Look up method name string (`engine.lookup_name(method)`).
  5. Call `resolve_builtin_method(engine, receiver_ty, tag, name_str, &arg_types, &arg_spans, span, None)`.
  6. On `Some(ret_ty)` where `ret_ty != Idx::ERROR`: that IS the method return. Call `unify_higher_order_constraints(engine, method, ret_ty, receiver_ty, &arg_types)` (still needed for higher-order — see §3.5' integration note) and return.
  7. On `Some(Idx::ERROR)`: arity/arg-type already emitted diagnostic; return Idx::ERROR.
  8. On `None`: method is not builtin; fall through to impl lookup (`lookup_impl_method` + `resolve_impl_signature` + `check_positional_args` as today).
- `infer_method_call_named` at `method_call.rs:62`: same pattern with `arg_types` built from `call_args` and `named_params: Some(&call_args.iter().map(|a| a.name).collect())`. Note: `ParamDef.name: &'static str` vs `Name` (interned) — we intern each `ParamDef.name` at call time via `engine.intern_name(p.name)` to build the param-name slice for `ContextKind::FunctionArgument { param_name }`. Named-arg-order validation remains `(target-only)` per `typeck.md §EX-3` — we do NOT enforce named-arg reordering in this fix arc (that is a separate feature).

**Impact on `ReceiverDispatch::Return.receiver_ty` wiring**: The current `ReceiverDispatch::Return { ret_ty, receiver_ty }` variant is the ONLY caller of `unify_higher_order_constraints`. After the refactor, `unify_higher_order_constraints` invocation moves to step 6 above. The refactor lines up cleanly: the existing "Return" branch's semantic was "builtin hit, return this type after higher-order adjustment" — the new structure expresses exactly that, with arity + arg-param unification layered in.

### 3.2'.a — Tag::Var receiver deferred method-call obligation (Round 5 revision per TPR-05-codex-F4, TPR-05-gemini-F3)

**Problem.** `method_call.rs:321-328` currently handles a `Tag::Var` receiver by allocating a fresh ret var and returning, with NO record that a method `m` is pending resolution against that receiver. If the receiver var later links to a concrete type, the method call is already typed (with the fresh ret var) — the downstream pool sees the ret var, not the concrete method's return type. This is a real constraint-loss LEAK that pre-exists BUG-04-074 but survives the §3.2' refactor unchanged. Per CLAUDE §The One Rule, "effort and scope are irrelevant" — we MUST fix it here, not defer to §3.6'.

**Shipped fix — pending-method-obligation table.** In `InferEngine`, add a side table `pending_method_obligations: Vec<PendingMethodObligation>`:

```rust
#[derive(Debug, Clone)]
struct PendingMethodObligation {
    receiver_var_id: u32,               // the Var(X) waiting to link
    method: Name,                       // the called method name
    arg_types: Vec<Idx>,                // pre-inferred arg types
    arg_spans: Vec<Span>,
    ret_ty: Idx,                        // the fresh var we returned to the caller
    call_span: Span,
    named_params: Option<Vec<Name>>,    // named-arg case
}
```

`resolve_receiver_for_dispatch` on `Tag::Var`:
1. Infer args into `arg_types`/`arg_spans` (ALREADY the current code path).
2. Allocate `ret_ty = engine.fresh_var()`.
3. Push `PendingMethodObligation { receiver_var_id: <var_id of the Tag::Var>, ... }` into the side table.
4. Return `ReceiverOutcome::Deferred { ret_ty }`.

**Replay dispatch (Round 1 revision per TPR-05-R1-codex-F2 + TPR-05-R1-codex-F3).** After unification completes, the replay fires `replay_method_call` which re-runs the FULL `infer_method_call` dispatch chain (NOT just `resolve_builtin_method`):

```rust
fn replay_method_call(engine: &mut InferEngine<'_>, ob: &PendingMethodObligation, new_recv_ty: Idx) -> Idx {
    let tag = engine.pool().tag(new_recv_ty);
    // Step 1: try builtin resolution with the now-concrete receiver.
    if let Some(builtin_ret) = resolve_builtin_method(
        engine, new_recv_ty, tag, &engine.lookup_name(ob.method).unwrap_or(""),
        &ob.arg_types, &ob.arg_spans, ob.call_span, ob.named_params.as_deref()
    ) {
        let _ = engine.unify().unify(ob.ret_ty, builtin_ret);
        return builtin_ret;
    }
    // Step 2: fall through to impl lookup (inherent → trait).
    let outcome = lookup_impl_method(engine, new_recv_ty, ob.method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, ob.method, ob.arg_types.len(), ob.call_span) {
        // Use check_positional_args / named-arg equivalent logic to verify args against sig.params.
        check_positional_args_from_types(engine, &ob.arg_types, &ob.arg_spans, &sig, ob.call_span);
        let _ = engine.unify().unify(ob.ret_ty, sig.ret);
        return sig.ret;
    }
    // Step 3: method genuinely not found on the concrete type.
    engine.push_error(TypeCheckError::unknown_method(ob.call_span, ob.method, new_recv_ty));
    let _ = engine.unify().unify(ob.ret_ty, Idx::ERROR);
    Idx::ERROR
}
```

This matches the three-tier resolution order in `typeck.md §EX-4` (builtin → inherent → trait) that `infer_method_call` already follows. The replay is an exact re-invocation with the deferred receiver now concrete — NOT a subset of it.

**Replay hook keyed to the full unify call, not one var — Round 5 revision per TPR-05-codex-F2 adds root-resolution gate.** `compiler/ori_types/src/unify/mod.rs`'s `unify()` is recursive: unifying `List(Var(X)) := List(int)` descends and links `Var(X) := int` via a child `unify(Var(X), int)` call. During one top-level `unify()` invocation, ANY number of vars may acquire `VarState::Link`. The replay hook:

1. Extend `UnifyEngine` with a `newly_linked_vars: Vec<u32>` field.
2. Every `bind_var(var_id, target_idx)` call (the single SSOT where `VarState::Unbound → VarState::Link` transitions happen — verify via `grep -n "VarState::Link" compiler/ori_types/src/unify/` that there is exactly one such site; if multiple, consolidate them first) appends `var_id` to `newly_linked_vars`. **Critical:** `bind_var` fires for BOTH var-to-concrete links (`Var(X) := int`) AND var-to-var links (`Var(X) := Var(Y)`) — the append is unconditional; filtering happens in the drain loop below.
3. `InferEngine::unify(a, b)` at the public API boundary drains `newly_linked_vars` AFTER the recursive `unify_engine.unify(a, b)` returns. For each drained `var_id`:

   **Root-resolution gate (Round 5).** Before firing replay, resolve the var's current root via `pool.resolve_fully(Var(var_id))`:
   - If the resolved root's tag is still `Tag::Var` (the var linked to ANOTHER var that is itself still unbound, i.e., var-to-var link chain not terminated at a concrete type), **leave the obligation pending** — do NOT fire replay. The drain loop does NOT remove the obligation from `pending_method_obligations`; a subsequent linking event that concretizes the chain will re-surface this var_id in `newly_linked_vars` and re-visit the gate.
   - If the resolved root has `TypeFlags::HAS_ERROR` (the receiver's var became poisoned via some downstream unification failure), **remove the obligation without firing replay**. This suppresses cascading diagnostics per `typeck.md ER-4` (follow-on suppression on poisoned subexpressions). The fresh `ret_ty` the caller stored remains unbound, which the body-exit validator handles per its cascade-suppression gate (`typeck.md ER-4 → HAS_ERROR check`).
   - If the resolved root is concrete (non-Var, non-Error tag), **fire replay** via `replay_method_call(engine, ob, resolved_receiver)`. Remove the obligation after successful replay.

   The gate uses `pool.resolve_fully` rather than a single-hop `var_state(var_id) == Link { target }` check because a chain `Var(X) := Var(Y) := int` requires transitive resolution — a single-hop check would report `Var(Y)` and misclassify the chain as still-Var when in fact it terminates at `int`.

4. If the replay itself triggers further unifications (inner `unify()` calls), those may link more vars, which append to `newly_linked_vars` again. The drain loop continues until the vector is empty — converges because:
   - Concrete-root obligations are removed after replay (one-shot).
   - Error-root obligations are removed without replay (one-shot).
   - Var-root obligations are NOT removed but also do NOT retry until a subsequent linking event surfaces them (linking is monotonic: a var cannot un-link once bound).
   - The total number of obligations is finite (bounded by the number of Tag::Var receiver sites in the body).

**Replay safety.** Replay runs within the same body's inference scope (the validator walks body `expr_types`, the replay pre-processes before that walk). Replay is idempotent: concrete-root and error-root obligations cannot fire twice (removed on first visit); var-root obligations are gated and converge only when their root concretizes. Obligations that never link to a concrete root are either (a) legitimately ambiguous (user wrote `let x = something; x.method()` with no constraint on `x`) — E2005 fires via validator on the fresh `ret_ty`, or (b) poisoned (the receiver's var became `HAS_ERROR`) — cascade-suppression in `validate_body_types` silences them. No new diagnostic surface is added. No cascading `unknown_method` errors fire on var-to-var intermediate states because the root-resolution gate suppresses replay until the chain terminates.

**Convergence proof sketch.** Let `O` be the pending-obligations set and `R` be the set of var_ids that have been resolved to a concrete or error root. At each drain-loop iteration: for each `var_id` in the drain queue, if `var_id`'s root is in `R` (concrete or error), the obligation is removed; otherwise the obligation stays. `R` is monotonically growing (links never unbind per `typeck.md UN-7` union-find discipline). Each linking event moves at most `O(|O|)` obligations from "pending" to "resolved" in one drain pass. Since `R` grows monotonically and `|O|` is finite, the drain loop terminates in `O(|O| × max_link_chain_depth)` operations — polynomially bounded by the program's var count.

**Removes §3.6' item 1 from the follow-up list.** The "Tag::Var receiver deferral loses method obligation" LEAK is now shipped inside §3.2'.a above. Update §3.6' to delete item 1 at Phase 5 close-out (see §3.6' revision below).

### 3.3' Integrate `unify_higher_order_constraints` as a post-step of arg-param unification (NOT delete)

Codex-cited `LEAK:scattered-knowledge`: `unify_higher_order_constraints` at `method_call.rs:164-265` hardcodes method names (`map`, `flat_map`, `filter`, `any`, `all`, `find`, `for_each`, `fold`, `rfold`). It CANNOT be deleted in this fix arc because `MethodDef.params` today encodes closure params as `ReturnTag::Fresh` — too weak to express `(T) -> U` shape. Richer closure-param encoding is part of Approach B endgame.

**What `3.3'` changes**: relocate the call site of `unify_higher_order_constraints` from the outer `infer_method_call` / `infer_method_call_named` functions INTO `resolve_builtin_method`, AS AN OPTIONAL POST-STEP after step 5 (arg-param unification) above.

**API-accurate pseudocode (Round 5 revision per TPR-05-codex-F5).** The §3.1' signature binds these exact variables inside `resolve_builtin_method`: `method_name: &str` (the formal parameter), `return_ty: Idx` (the result of step 7's `return_tag_to_idx` call — bound immediately after step 7 computes it), `receiver_ty: Idx` (formal parameter), `arg_types: &[Idx]` (formal parameter). Step 6 (higher-order propagation) inserts AFTER step 7 (return_tag_to_idx) and BEFORE the final `Some(return_ty)` return:

```rust
pub(crate) fn resolve_builtin_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,              // &str — NOT Name — per step 3's find_method signature
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
    named_params: Option<&[Name]>,
) -> Option<Idx> {
    // Steps 1-3: Named/Applied routing, tag → TypeTag, find_method. (unchanged)
    // Step 4: arity check. (§3.1' step 4)
    // Step 5: arg-param unification with Fresh-carve-out. (§3.1' step 5 + §3.1'.a)
    // Step 6: Range<float> rejection. (unchanged)

    // Step 7: convert return tag to Idx via the bridge.
    let return_ty = if method_def.returns == ReturnTag::Fresh {
        computed_returns::resolve_computed_return(engine, receiver_ty, tag, method_name)
    } else {
        registry_bridge::return_tag_to_idx(engine, receiver_ty, method_def.returns)
    };

    // Step 8 (NEW): higher-order closure-param propagation.
    // Uses the already-bound method_name (&str) and return_ty (Idx) — no
    // new identifiers. The helper's signature becomes:
    //   pub(super) fn unify_higher_order_constraints_impl(
    //       engine: &mut InferEngine<'_>,
    //       method_name: &str,
    //       return_ty: Idx,
    //       receiver_ty: Idx,
    //       arg_types: &[Idx],
    //   )
    // (replacing the current outer signature that takes `method: Name`
    //  and re-looks-up the string via engine.lookup_name(method)).
    // TODO(methodregistry-endgame): this block is the shipped workaround
    // for closure-param shape; remove when MethodDef.params grows
    // ReturnTag::Closure encoding (§3.7' Approach B endgame).
    higher_order::unify_higher_order_constraints_impl(
        engine, method_name, return_ty, receiver_ty, arg_types,
    );

    Some(return_ty)
}
```

This consolidates the dispatch into the builtin-method canonical home (eliminating the `LEAK:scattered-knowledge` flag from Codex) while preserving the existing behavioral semantics (all map/filter/fold tests continue to pass). The `unify_higher_order_constraints` function itself is renamed `unify_higher_order_constraints_impl`, moved to `compiler/ori_types/src/infer/expr/methods/higher_order.rs` (new submodule), and made `pub(super)` so `resolve_builtin_method` can call it. Critical API change: the new helper takes `method_name: &str` directly instead of `method: Name` + `engine.lookup_name(method)` — saves one indirection and removes the `Option<&str>` failure mode on unknown names.

**Why NOT fold into Approach B now**: richer `MethodDef.params` encoding means extending `ReturnTag` with a `Closure(param_proj, return_proj)` variant (where both projections are themselves `ReturnTag`s), adding construction sites for every closure-taking method's `params` in `ori_registry/src/defs/*`, and updating `return_tag_to_idx` to construct `Function(...)` types from the new variant. That's a 15+ file refactor across `ori_registry` + `ori_types` + tests, appropriate for a standalone plan (Approach B endgame) but not for this fix arc.

### 3.4' Wire `validate_body_types` across ALL four Bodies-group passes + handle test attributes

Reviewer-cited gaps collated here (Codex F2 + Gemini F2 + Codex F3 + Round 1 prior; Round 5 excised the typeck-exit second-validator subsection per TPR-05-codex-F3 + TPR-05-gemini-F2 AGREEMENT that `TypeCheckResult::finish_with_pool()` does not export a `TypedBody` aggregate): `validate_body_types` is currently wired into pass 2 (function bodies) per the 2026-04-14 shipped plan. Passes 3, 4, 5 have NO wiring. Test attributes `#skip` and `#compile_fail` interact with E2005 emission in ways that can break existing tests. Existing test bodies in `tests/spec/traits/iterator/double_ended.ori` (lines 24, 66) use ambiguous empty-container patterns that will trip the validator once passes 3/4/5 are wired.

**Round 5 architectural decision (per TPR-05-codex-F3 + TPR-05-gemini-F2 AGREEMENT).** The Round 1 plan included a §3.4'.ii "second validator at `finish_with_pool()`" as defense-in-depth against a "scope mismatch" between the body-exit validator's view and codegen's view. Both reviewers independently verified that `TypeCheckResult::finish_with_pool()` at `compiler/ori_types/src/check/mod.rs:389` produces a flat `TypedModule { expr_types, functions, ... }` with NO `TypedBody` aggregate and no per-expr→function-owner mapping — the data contract the §3.4'.ii validator sketch assumed doesn't exist. Round 5 excised §3.4'.ii entirely: the underlying "scope mismatch" concern was a symptom of the arg-param unification leak (BUG-04-074's root cause), and §3.1'/§3.1'.a close that leak directly. Once the leak is closed, the per-body-exit validator sees a clean typed IR and codegen never re-derives stale types. The second validator pass is redundant defense-in-depth against a leak that no longer exists. §3.4' is renumbered: the former §3.4'.iii/iv/v/vi become §3.4'.ii/iii/iv/v.

**3.4'.i — Pass-2 wiring audit (Codex F2).** Verify via `grep -n validate_body_types compiler/ori_types/src/check/bodies/functions.rs` that pass 2 calls the validator. The shipped commit `65b3aff4` claims this but Codex reviewer flagged `functions.rs:115` as lacking the hook. Before implementation: open `check/bodies/functions.rs`, trace every `check_function_body` exit point, confirm `validate_body_types(...)` is invoked after `engine.pop_context()`. If the hook is actually missing (reviewer finding correct), add it as `3.4'.i`. If shipped (reviewer mis-cite), mark `3.4'.i` as "audit-only — already shipped" and proceed to `3.4'.ii`.

**3.4'.ii — Passes 3/4/5 wiring (Codex F2 + existing plan; renumbered from former 3.4'.iii in Round 5).** After 3.4'.i lands, add `validate_body_types` invocations at the exit of `check_test_body` (pass 3), `check_impl_method_body` (pass 4), `check_def_impl_method_body` (pass 5) — each at the `engine.pop_context()` call site matching pass 2's pattern. Each invocation passes `(pool, arena, expr_types, sig, sig_span, scheme_var_ids, record_error)` — same signature as pass 2. Per-body context (per-function `FunctionSig.scheme_vars`, per-body `expr_types` slice scoped to the body being checked, per-body `arena`) is available at the `engine.pop_context()` site in each Bodies-group pass — the API mismatch that doomed the Round 1 typeck-exit validator does NOT exist at the per-body hook site.

**3.4'.iii — Test-attribute interaction (Gemini F2; renumbered from former 3.4'.iv in Round 5).** Per CLAUDE.md, `#skip("reason")` only works when the test body type-checks cleanly; type errors block the skip. `#compile_fail("expected")` expects a compile error. After 3.4'.ii wires the validator into pass 3, existing test bodies that used ambiguous empty-list patterns without element-type constraints will suddenly emit E2005. Three cases:

1. **`#skip`'d test body with empty-list pattern** — `@t tests @foo #skip("reason") @target () -> void = { let xs = []; xs.len() }` — E2005 fires, blocking the skip. This is a TRUE bug: the test body IS genuinely ambiguous, and the skip reason should not hide that. Resolution: audit all `#skip`'d bodies in `tests/spec/` during 3.4'.v (below), annotate ambiguous containers with element types (`let xs: [int] = []`) OR promote the skip to `#compile_fail("E2005")` if the bug IS about ambiguity.
2. **`#compile_fail`'d test body with empty-list pattern used as secondary noise** — `#compile_fail("E1234")` where the body has `let xs = []; xs.len()` as incidental. Pre-3.4'.ii: compiler emits E1234 from the actual bug, E2005 suppressed at codegen (never gets there). Post-3.4'.ii: compiler emits E2005 AND E1234. The `#compile_fail` matcher checks substring "E1234" — passes. But `E2005` is an extra error; if the test asserts "exactly one error", it breaks. Resolution: audit `#compile_fail` bodies with ambiguous containers; add explicit element types OR switch to `#compile_fail` keyed form with `code: "E1234"` (ignores extras). The `_template.md` harness supports both shorthand and keyed forms per `.claude/rules/tests.md §Attributes`.
3. **Non-`#skip`, non-`#compile_fail` test body that happens to work today via accident** — `tests/spec/traits/iterator/double_ended.ori:24` is exactly this: `let iter = [1, 2, 3].iter(); let result = [];` — the `result` is ambiguous but the body never uses it, so no codegen error surfaces today. Post-3.4'.ii: E2005 fires on `result`. Resolution: annotate these sites in the audit pass (3.4'.iv).

**Critical — order matters.** The ordering of `#skip` evaluation vs validator firing must be: validator runs FIRST (it's part of body-checking), `#skip`/`#compile_fail` matching runs LATER (harness level). The harness reads the emitted diagnostics and decides whether to pass/skip/fail the test. Per `.claude/rules/tests.md`, the validator is a type-check-phase deliverable; the harness observes its outputs. There is no "evaluate #skip before validation" — that would be INVERTED-TDD per `impl-hygiene.md §Finding Categories` (skipping the enforcement the subsection deliverable is designed to catch). The correct fix is audit-and-annotate, not skip-the-validator.

**3.4'.iv — Pre-rollout test audit (Codex F3; renumbered from former 3.4'.v in Round 5).** Before 3.4'.ii lands, grep `tests/spec/` for ambiguous-container patterns that would newly fail:

```bash
# Patterns that commonly trigger post-3.4'.ii E2005:
grep -rn "let [a-zA-Z_]* = \[\];" tests/spec/     # `let xs = []` without annotation
grep -rn "let [a-zA-Z_]* = \{\};" tests/spec/     # `let xs = {}` without annotation
```

For each hit: (a) if a subsequent use-site constrains the element type (e.g., `xs.push(10)` after arg-param unification lands per §3.1'/§3.1'.a), no change needed; (b) if NO downstream constraint, annotate explicitly (`let xs: [int] = []`). Fix these BEFORE the 3.4'.ii commit so the `test-all.sh` run stays green. Codex flagged `tests/spec/traits/iterator/double_ended.ori:24,66` as known hits — audit starts there.

**3.4'.v — Narrow-front discipline check (renumbered from former 3.4'.vi in Round 5).** After 3.1'-3.3' land (arg-param unification closes the bulk of the cases), 3.4'.ii + 3.4'.iv together should NOT produce a large concurrent-test-failure front. CLAUDE.md §Stabilization Discipline mandates the narrow-front principle qualitatively ("complete one fix/section fully before starting another") but does not quantify a threshold; the plan-local heuristic (NOT a rule-citation) is ≤20 concurrent E2005 emissions during the intermediate validator-wiring steps, chosen as an interpretive cap that triggers "STOP and triage" rather than "accept and push through" (per TPR-05-R1-codex-F5 factual correction — the quantitative threshold is plan-local, not CLAUDE-cited). Run `timeout 150 cargo st tests/spec/ 2>&1 | grep -c 'E2005'` after the sequence: (1) land 3.1'/3.1'.a/3.2'/3.2'.a/3.3', (2) run tests, (3) land 3.4'.i, (4) run tests again, (5) audit per 3.4'.iv, (6) land 3.4'.ii. If at any intermediate step the failure count spikes beyond the plan-local threshold, STOP and triage: either the arg-param unification is missing cases or the audit missed annotations. Adjusting the threshold downward (e.g., to 10) is permitted if §3.4'.iv audit shows finer-grained control is warranted; upward drift is a red flag.

### 3.5' Update `tag_display` for builtin method arity error messages (minor)

The arity error at §3.1' step 4 uses `tag_display(tag)` to produce the method's type display name (e.g., "`List`", "`Map`", "`Str`"). If `compiler/ori_types/src/infer/expr/methods/mod.rs` already has this helper, reuse. Otherwise add a small `fn tag_display(tag: Tag) -> &'static str` with arms for every container/primitive that can have a `TypeTag` registry entry. Mirror the existing `ori_registry::TypeTag::name()` implementation at `compiler/ori_registry/src/tags/mod.rs`. Zero-cost, zero-complexity — prevents ugly `Debug`-formatted tags in user diagnostics.

### 3.6' Follow-up bugs to file via `/add-bug` (track, don't fix here)

Per CLAUDE §Proactive Bug Filing. These hazards surfaced during 2026-04-16 investigation; each is out of scope for BUG-04-074 but must be tracked.

**Round 5 revision (2026-04-16)**: items previously listed as 1 and 4 were reviewer-flagged as BLOCKING rather than follow-ups (TPR-05-codex-F4, TPR-05-gemini-F3, TPR-05-gemini-F1). Both have been promoted into shipped prerequisites in §3.2'.a and §3.4'.ii respectively. The remaining follow-ups:

- **BUG (LEAK — methods/mod.rs:88) — `resolve_named_type_method` hardcodes method names.** `unwrap`/`inner`/`value`/`debug`/`to_str` are hardcoded outside the registries for Named/Applied (user-defined) types. Any `/query-intel` or future extension-dispatch work has to mirror this. File with severity `medium`; subsystem `ori_types`.
- **BUG (LEAK/DRIFT — ori_arc/src/rc_insert/annotate.rs:83,361) — ARC re-encodes builtin method semantics by method name + receiver type.** Downstream ARC annotation duplicates knowledge already in `ori_registry::MethodDef.receiver`/`params[].ownership`. File with severity `medium`; subsystem `ori_arc`.

Items PROMOTED to shipped prerequisites (NOT follow-ups anymore):
- ~~BUG (LEAK — method_call.rs:321-328) — Tag::Var receiver deferral loses method obligation.~~ → Shipped in §3.2'.a via pending-method-obligation table.
- ~~BUG (GAP — validate_body_types scope mismatch vs codegen view).~~ → **Resolved by §3.1'/§3.1'.a arg-param unification** (Round 5 per TPR-05-codex-F3 + TPR-05-gemini-F2 AGREEMENT). The "scope mismatch" was a symptom of the `resolve_builtin_method` arg-param leak — with that leak closed, the per-body-exit validator sees a clean typed IR and codegen never re-derives stale types. Round 1 proposed a second validator at `finish_with_pool()`, but `TypeCheckResult` doesn't export the `TypedBody` aggregate that placement required; Round 5 excised the second-validator subsection and the original concern is fully addressed by arg-param unification alone.

Each remaining filed entry points at this fix section (`fix-BUG-04-074.md`) for context. Filing happens in Phase 5 step 7 (before bug-entry closure for BUG-04-074).

### 3.7' Approach B endgame plan (track, don't create here)

At Phase 5 closure, run `/create-plan` with title **"Unified MethodRegistry — eliminate RG-3 two-path dispatch"**. Plan path TBD (suggest `plans/method-registry-unification/`). Scope:
- Extend `MethodDef.params` to encode closure-shape params (`ReturnTag::Closure { param: Box<ReturnTag>, ret: Box<ReturnTag> }` or equivalent) so builtin higher-order methods don't need the `unify_higher_order_constraints_impl` special case.
- Migrate builtin-method lookup into the unified `MethodRegistry` with receiver-family matching (not exact `self_type: Idx`) — covers `List<T>`, `Map<K, V>`, etc.
- Delete `unify_higher_order_constraints_impl` once the registry covers closure shapes.
- Migrate `resolve_named_type_method`'s hardcoded method names into the registry (closes one of the §3.6' LEAKs).

The plan is created AT Phase 5 closure (not now) because A must land + validate cleanly first. The link between this fix and the future plan lives in the Phase 5 report.

### 3.8' Implementation order (active)

1. Write all tests from §2 (existing) AND §2.arg-param (new) — verify failures against current code.
2. Implement §3.1' arity + arg-param unification in `resolve_builtin_method`.
3. Implement §3.2' `method_call.rs` refactor threading args into builtin dispatch.
4. Implement §3.3' relocation of `unify_higher_order_constraints` into the builtin path.
5. Implement §3.4' wiring of `validate_body_types` across all four Bodies passes.
6. Apply §3.5' `tag_display` helper if needed.
7. Run `timeout 150 ./test-all.sh` — full suite green. Fix any regressions immediately per CLAUDE §Stabilization Discipline.
8. Run `timeout 150 ./clippy-all.sh` — no warnings.
9. Run `/commit-push` per `.claude/skills/commit-push/workflow.md`.
10. Phase 5: /tpr-review → /impl-hygiene-review → /improve-tooling retro → /sync-claude → file §3.6' follow-up bugs → /create-plan for §3.7' endgame → /commit-push closure artifacts.

---

## R. Third Party Review Findings

> **SUPERSEDED-ENTIRELY-2026-04-16 — Rounds 1-4 (2026-04-14).** All four 2026-04-14 Plan TPR rounds targeted the shipped-but-wrong approach (should_generalize + validate_body_types). That approach landed in commits `b1f2c354` → `3069f3e6` → `a9a0de43` → `65b3aff4` and is correct on its own scope, but did NOT resolve BUG-04-074's codegen failure (see §0 POST-INVESTIGATION REWRITE). The residual root cause is in `resolve_builtin_method` arg-param unification (see §3.1'), which none of the 2026-04-14 rounds identified because their scope was the generalization-and-validation layer, not the builtin-method-dispatch layer. **All round-1, round-2, round-3, round-4 findings below are preserved VERBATIM as audit trail only — none of them translate to the active §3' plan.** A single fresh Plan TPR round 5 runs at Phase 2.5 on §3' (active) and will record its findings in `### Phase 2.5 — Plan TPR Round 5 (2026-04-16)` below this supersession banner when complete.

### Phase 2.5 — Plan TPR (2026-04-14)

Plan TPR run `/tmp/ori-tpr-DZNWHvXU`. codex walltime 436s, gemini walltime 232s, ASYMMETRY LOW (comparable depth). 12 findings (10 actionable + 2 informational). Adversarial review flagged multiple critical flaws in the plan's validation-pass design.

- [x] `[TPR-04-001-codex][high]` `plans/bug-tracker/fix-BUG-04-074.md:283` — Narrow the E2005 sweep so valid lambda polymorphism survives.
  Resolved: Revised §3.3 on 2026-04-14. Validation pass is now NARROWLY SCOPED to let-binding declared types (final_ty after generalization), NOT every expr_types entry. Added `test_polymorphic_lambda_with_container_still_type_checks` positive pin. Sub-expression Vars from pre-generalization storage are no longer touched, preserving lambda_mono/type_resolve.rs:55-73's legitimate Var usage.
  Evidence: `infer_expr` stores every subexpression type before the caller generalizes (`ori_types/src/infer/expr/mod.rs:272`), so `let id = x -> x` leaves the lambda typed as `Function(Var, Var)` in expr_types. Downstream `is_polymorphic_lambda` at `ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` EXPLICITLY treats generalized `Tag::Var` inside polymorphic lambdas as legitimate machinery, using `contains_var` for deep checks. My blanket sweep would turn BUG-04-074 into a let-polymorphism regression.
  Required plan update: Validate only ambiguity surfaces that must be concrete at body exit, not every `expr_types` entry. Either (a) rewrite generalized let-initializer `expr_types` to `Scheme` before storage so the walk sees bound-var-correct shapes, or (b) make the sweep AST/context-aware so legitimate lambda-polymorphism surfaces are exempt. Add an explicit positive pin proving `let id = x -> x; id(1); id("hello")` still passes.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `type_resolve.rs:47-73` confirmed.)

- [x] `[TPR-04-002-gemini][high]` `plans/bug-tracker/fix-BUG-04-074.md:237` — Track bound variable IDs in `has_unresolved_var` instead of a boolean.
  Resolved: Revised §3.3 on 2026-04-14. `has_unbound_var` now uses `FxHashSet<u32>` for bound var IDs. When entering a Scheme, scheme_vars are pushed onto the set; popped on exit. A `Tag::Var` is "unbound" only if its var_id is NOT in the current set. This correctly handles captured vars inside closure lambdas — they aren't in the scheme's bound set, so they're flagged as ambiguous.
  Evidence: A scheme only binds SPECIFIC `var_ids` (those returned by `pool.scheme_vars()` — verified at `ori_types/src/unify/generalization.rs:47-58` where `pool.scheme(&vars, ty)` binds exactly those). If a closure captures an outer empty collection, the outer `Tag::Var` inside the scheme's body is NOT bound by that scheme. The proposed `walk_type(pool, ty, /*under_scheme=*/ false)` with a boolean flag would wrongly exempt such captured Vars.
  Required plan update: Update `has_unresolved_var` to track the exact set of bound variable IDs (e.g., passing down a `&FxHashSet<u32>`) and only exempt `Var`s whose IDs are present in the bound set, not all Vars under any Scheme.
  Basis: fresh_verification. Confidence: high. (Verified independently: `generalization.rs:47-58` confirmed.)

- [x] `[TPR-04-003-codex][high]` `plans/bug-tracker/fix-BUG-04-074.md:283` — Suppress E2005 on error-poisoned subexpressions.
  Resolved: Revised §3.3 on 2026-04-14. Added cascade-suppression guard at top of `validate_binding_type_resolved`: skip when `engine.has_errors()` is true OR when `final_ty` has `TypeFlags::HAS_ERROR` set. Added `test_empty_list_inside_already_error_typed_expression_suppresses_E2005` negative pin.
  Evidence: GAP against typeck.md UN-4 and impl-hygiene.md §Error Recovery Monotonicity. `infer_expr` stores every subexpression type eagerly (`ori_types/src/infer/expr/mod.rs:272`), while failing paths return `Idx::ERROR` only at outer expressions. An empty literal nested inside a broken expression still sits in `expr_types` as `List(Var)` and would pick up a second E2005 even though the primary diagnostic explains the failure.
  Required plan update: Specify suppression rules in §3.3: skip any `expr_type` whose tree contains `Tag::Error` / `HAS_ERROR`. Add a negative recovery test where an empty literal inside an already-error-typed expression does NOT emit E2005.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-001-gemini][high]` `plans/bug-tracker/fix-BUG-04-074.md:214` — Suppress E2005 emission if engine already has errors.
  Resolved: Same fix as [TPR-04-003-codex] — see above. `engine.has_errors()` guard + `TypeFlags::HAS_ERROR` check at top of validation helper.
  Evidence: Same class as TPR-04-003-codex. If `let x = []` with a downstream typo (`fop(x)` instead of `foo(x)`), the typechecker emits `UnknownIdent` and `x` remains unresolved. Indiscriminate E2005 emission causes cascading errors, violating UN-4 recovery monotonicity.
  Required plan update: Add `engine.has_errors()` guard or `Tag::Error` unification check before emitting E2005.
  Basis: fresh_verification. Confidence: high. (Cross-reference: TPR-04-003-codex — agreement on semantics even though merger didn't auto-detect.)

- [x] `[TPR-04-002-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:164` — Make the no-Var regression pin recursive.
  Resolved: Revised §2 TDD Matrix on 2026-04-14. Negative pin `test_no_unresolved_var_in_repro_expr_types_recursive` now uses recursive `contains_var` helper modeled after `lambda_mono/type_predicates.rs:10-25` to check Var at any depth.
  Evidence: The proposed negative pin iterates `TypedModule.expr_types` and checks top-level `Tag::Var`. The bug shape is `List(Var(X))` where the TOP is `List` but the CHILD is `Var`. Plan's shallow check would pass while the bug remains. Repo already has `contains_var` at `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`.
  Required plan update: Change negative pin to walk each expr_type recursively with `contains_var`-style helper. Assert the specific repro's empty-list expr_type has no unresolved Vars at any depth.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-004-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:146` — Hit the actual standalone let-expression path.
  Resolved: Revised §2 TDD Matrix on 2026-04-14. Replaced test case with Rust unit test `test_empty_list_standalone_let_expr_routes_through_infer_let` that constructs `ExprKind::Let` directly and asserts dispatch through `infer_let`, not `infer_block`. Test uses the parser surface `@test () -> void = let x = 1;` confirmed at `ori_parse/src/tests/parser.rs:130-167`.
  Evidence: `infer/expr/mod.rs:159-173` routes `ExprKind::Block` through `infer_block` and `ExprKind::Let` through `infer_let`. Block-statement `let` inside `@main () -> int = { let x = ...; x }` dispatches to `infer_block`, NOT `infer_let`. Standalone let expression is `@test () -> void = let x = 1;` (parser surface at `ori_parse/src/tests/parser.rs:130-167, 1703-1730`). Plan's infer_let coverage cell doesn't force the intended dispatch.
  Required plan update: Replace the `infer_let` coverage cell with a real standalone `ExprKind::Let` program OR a focused Rust unit test that necessarily dispatches through `infer_let`.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `infer/expr/mod.rs:159-173` confirmed.)

- [x] `[TPR-04-005-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:330` — Expand the test audit beyond two dead locals.
  Resolved: Revised §3.4 on 2026-04-14. Test audit now includes direct-receiver `[].iter()` forms: `double_ended.ori:167` and `double_ended_methods.ori:35, 84, 133, 174`. Added full repo sweep step: `rg -n 'let \w+ = \[\];|let \$?\w+ = \[\];|\[\]\.iter\(\)|\[\]\.len\(\)|\[\]\.is_empty\(\)' tests/ library/`.
  Evidence: §3.4 only cites `double_ended.ori:25-34, 66-82`, but active suite coverage also uses uncontextualized empty-list receivers: `[].iter()` at `double_ended.ori:167`; `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)` at `double_ended_methods.ori:35, 84, 133, 174`. Plan misses direct-receiver forms entirely.
  Required plan update: Broaden §3.4 to audit ALL uncontextualized empty-literal forms — direct receivers (`[].iter()`) not just `let name = []` bindings. Name all currently-discovered active files and update them.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-006-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:155` — Narrow or re-spec the map and set portion of the fix.
  Resolved: Revised §1.5, §2, §3 on 2026-04-14. Scope NARROWED to empty lists only — dropped `{}` and `Set<int>()` test cases. Rationale: spec at `14-expressions.md:1224-1228` only declares `[]` a compile-time error; `{}` parsing is spec-neutral (1238-1240); `Set<int>()` is NOT valid Ori syntax and `infer_empty_set()` does NOT exist (verified). The validation pass WILL catch ambiguous maps `let m = {}; m.len()` if they arise (it's type-agnostic — walks any Tag::Var), but tests focus on the spec-sanctioned list case.
  Evidence: Spec at `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228` only declares `[]` without context a compile-time error. For `{}`, spec at 1238-1240 only says it parses as empty map literal — no normative rejection of `let m = {}`. For sets, documented empty construction is `Set.new()` or `[].iter().collect()` (not `Set<int>()` which is NOT valid Ori syntax). `infer_empty_set()` does not exist as a function (verified).
  Required plan update: Either (a) narrow the fix scope and success criteria to empty-list ambiguity only, or (b) add an explicit spec investigation step for empty maps + real empty-set construction path BEFORE including them in goals, diagnostics, or tests. If (b), may require a spec proposal.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `infer_empty_map` DOES exist at `infer/mod.rs:630`; `infer_empty_set` does NOT exist.)

- [x] `[TPR-04-003-gemini][medium]` `plans/bug-tracker/fix-BUG-04-074.md:154` — Consolidate `infer_let` control flow to fix Algorithmic Duplication.
  Resolved: Added §3.5 with concrete tracking artifact on 2026-04-14. The consolidation is out-of-scope for BUG-04-074 per CLAUDE.md narrow-the-front discipline, but tracked for close-out: file `BUG-04-{next}` via `/add-bug` at close with title "Consolidate let-binding inference across infer_block/infer_let/sequences.rs into shared helper", subsystem `ori_types`, severity `low`.
  Evidence: Per `impl-hygiene.md` §Algorithmic DRY ("same fix at 3+ callsites = missing abstraction"), my plan extracts `should_generalize` but leaves the surrounding multi-step let-binding algorithm (detect capture, branch on generalize, bind pattern) duplicated across `infer_block`, `infer_let`, and `sequences.rs`.
  Required plan update: Add a step to consolidate the let-binding logic into a single `bind_local_let` abstraction — OR explicitly mark it as out-of-scope for this fix with a concrete tracking artifact (bug or roadmap item).
  Basis: direct_file_inspection. Confidence: medium.

- [x] `[TPR-04-004-gemini][medium]` `plans/bug-tracker/fix-BUG-04-074.md:228` — Tailor E2005 suggestion message dynamically.
  Resolved: Moot after scope narrowing per [TPR-04-006-codex]. Since tests only cover empty lists, the existing E2005 message "cannot infer type" is adequate. If future work extends to maps/sets (via a spec proposal), message tailoring should be added then. No action needed for this fix.
  Evidence: Plan proposes E2005 message "add a type annotation like `let x: [int] = []`". But if the sweep covers `{}` and sets (per TPR-04-006-codex this may be dropped), a hardcoded list suggestion is misleading for maps. Violates impl-hygiene.md §Diagnostic Message Quality ("show the fix").
  Required plan update: If empty map/set stay in scope, validation pass should inspect `arena.get_expr(expr_idx).kind` to tailor: `let x: {str: int} = {}` for maps, `let x: [int] = []` for lists. If scope narrows to lists only (per TPR-04-006-codex), this is moot.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-007-codex][informational]` `plans/bug-tracker/fix-BUG-04-074.md:189` — Helper extraction and per-body hook are structurally sound.
  Evidence: Confirmed 3 current let-generalization sites (`infer_block`, `infer_let`, try-block `let`). CK-1 body-pass structure supports body-local validation. Centralizing the AST-based value-restriction predicate is the right direction.
  Resolved: Confirmed — keeping the helper centralized and the per-body hook as planned.

- [x] `[TPR-04-005-gemini][informational]` `plans/bug-tracker/fix-BUG-04-074.md:243` — Downstream `ori fmt` and `ori run` are safe.
  Evidence: `ori fmt` uses parser only — no typechecking dependency. `ori run` blocks evaluation when `has_errors()` is true. Plan's downstream assessments are accurate.
  Resolved: Confirmed — no plan updates needed for fmt/run paths.

### Plan revisions applied

Based on the verified findings, §1.5 Fix Consensus, §2 TDD Matrix, and §3 Implementation will be revised before Phase 3 (TDD writing). Key revisions:

1. **Narrow the validation pass target** (addresses TPR-04-001-codex + TPR-04-002-gemini): instead of walking ALL `expr_types`, validate only at specific exit points OR rewrite generalized let-init expr_types to Scheme shape before storage.
2. **Scoped bound-var tracking** (addresses TPR-04-002-gemini): replace `under_scheme: bool` with `bound_vars: &FxHashSet<u32>` passed through the walk.
3. **Cascade suppression** (addresses TPR-04-003-codex + TPR-04-001-gemini): skip E2005 emission when `HAS_ERROR` flag is set on the type or when `engine.has_errors()`.
4. **Narrow scope to lists only** (addresses TPR-04-006-codex): drop `{}` and `Set<int>()` from the test matrix; spec only mandates `[]` rejection. Remove `infer_empty_set()` references (no such function exists).
5. **Recursive negative pin** (addresses TPR-04-002-codex): use `contains_var` helper for deep checking.
6. **Real `infer_let` dispatch test** (addresses TPR-04-004-codex): add a program that actually routes through `infer_let` via standalone `let x = 1;` syntax.
7. **Broader test audit** (addresses TPR-04-005-codex): audit `[].iter()` and similar direct-receiver forms, not just `let name = []`.
8. **Algorithmic DRY follow-up** (addresses TPR-04-003-gemini): file as follow-up bug (BUG-04-{next}) rather than in-scope refactor — keep this fix narrowly focused.

### Phase 2.5 — Plan TPR Round 2 (2026-04-14)

Plan TPR round 2 run `/tmp/ori-tpr-G6VzNAHW`. Codex walltime 424s (131 events, 14096-byte envelope). Gemini walltime 223s (97 events, 5721-byte envelope). ASYMMETRY MODERATE (bytes 11.7x RED; walltime 1.9x and events 1.4x LOW). 12 actionable findings, 0 agreements (all complementary) — round-1 revisions introduced new architectural flaws in §3.3 that round 2 correctly surfaced. Every finding independently verified against the code; the critical claims (validator timing, Link-chain following, Pool API surface, `ambiguous_type` signature) all hold.

- [x] `[TPR-04-001-codex-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:320` — Validator runs too early; would emit E2005 on the success-case repro.
  Resolved: Revised §3.3 on 2026-04-14 (round-2). Validation moved from "immediately after `bind_pattern` at each let site" to a **per-body-exit sweep** invoked by the Bodies-group pass (CK-1 passes 2–5) after the full body has been inferred. By that point, downstream constraints (e.g., `push(10)` linking the element var) have had a chance to fire.
  Evidence: `compiler/ori_types/src/infer/expr/blocks.rs:22-97` processes block statements in order. For the repro `let ages = []; ages = ages.push(value: 10); ages.len()`, at the moment `bind_pattern` is called for `let ages = []`, the later `ages.push(value: 10)` has NOT yet unified the element var with `int`. Emitting E2005 at that point would fail the stated success criterion "the exact repro compiles via `ori build` and runs successfully."
  Required plan update: Change §3.3 to run validation at body exit (once per function / method / test body in Bodies-group passes 2–5), not at `bind_pattern` time.
  Basis: fresh_verification. Confidence: high. (Verified: `blocks.rs:22-97` block statement ordering confirmed.)

- [x] `[TPR-04-002-codex-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:384` — `has_unbound_var` must follow `VarState::Link` chains before classifying ambiguity.
  Resolved: Revised §3.3 on 2026-04-14. `has_unbound_var` now calls `pool.resolve_fully(ty)` as its first step (which walks `VarState::Link` chains up to 16 hops per `pool/accessors.rs:412-491`), then inspects the resolved type. A `Tag::Var` still present after `resolve_fully` is genuinely `Unbound` or `Generalized`-not-in-scope.
  Evidence: `compiler/ori_types/src/unify/substitute.rs:73-76` confirms: `if let VarState::Link { target } = self.pool.var_state(var_id) { return self.substitute(*target, subst); }` — a `Tag::Var` node with `VarState::Link` targets is a *resolved* inference variable. `pool/accessors.rs:412-491` `resolve_fully` walks Link chains explicitly. A walker that only checks `pool.tag(ty) == Tag::Var` would flag every resolved-but-unsubstituted var as ambiguous, producing false-positive E2005 for fully-inferred programs.
  Required plan update: Specify that `has_unbound_var` must invoke `pool.resolve_fully(ty)` before any structural dispatch.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-001-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:386` — Resolve types before checking for variables to prevent false positives.
  Resolved: Same fix as [TPR-04-002-codex-r2] — `resolve_fully()` invoked first in `has_unbound_var`. Gemini's finding and codex's round-2-002 surfaced the same root cause via independent paths.
  Evidence: Same as [TPR-04-002-codex-r2].
  Required plan update: Same as [TPR-04-002-codex-r2].
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-002-codex-r2] — agreement-by-root-cause, different specific wording.)

- [x] `[TPR-04-002-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:387` — `ambiguous_type` constructor signature mismatch; code will fail to compile.
  Resolved: Revised §3.3 on 2026-04-14. Changed `has_unbound_var` return type from `bool` to `Option<OffendingVar>` where `OffendingVar { var_id: u32, context_desc: String }`. At the emission site: `engine.push_error(TypeCheckError::ambiguous_type(span, offender.var_id, offender.context_desc))`.
  Evidence: `compiler/ori_types/src/type_error/check_error/mod.rs:236`: `pub fn ambiguous_type(span: Span, var_id: u32, context_desc: String) -> Self`. The plan's `TypeCheckError::ambiguous_type(span)` does not match this signature. Additionally, `compiler/ori_types/src/infer/context.rs:68` defines `pub fn push_error(&mut self, error: TypeCheckError)` — the plan's `engine.record_error(...)` is not a real method.
  Required plan update: Fix the call-site to use the real 3-argument constructor signature and `push_error` method name; have `has_unbound_var` return the offending var_id so the caller can construct the diagnostic.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-003-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:125` — Drop module-wide `engine.has_errors()` suppression.
  Resolved: Revised §3.3 on 2026-04-14. Removed the global `engine.has_errors()` branch. Cascade suppression now relies SOLELY on the type-local `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` check per `TypeFlags::HAS_ERROR` propagation (`types.md TF-3`).
  Evidence: `compiler/ori_types/src/infer/context.rs:51-55`: `pub fn has_errors(&self) -> bool { !self.errors.is_empty() }`. This is module-wide — returns true if ANY prior error exists anywhere in the current module check. Under the proposed global gate, a pre-existing unrelated error ANYWHERE in the module silently swallows E2005 emissions for all subsequent ambiguous empty-list bindings, violating the success criterion "error surfaces at typeck, not codegen." The correct cascade-suppression gate is `TypeFlags::HAS_ERROR` on the binding's own `final_ty` — it propagates from children upward per `types.md TF-3`.
  Required plan update: Delete the `if engine.has_errors() { return; }` branch from `validate_binding_type_resolved`. Keep ONLY the `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` check.
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-004-gemini-r2] — agreement on the over-suppression semantics.)

- [x] `[TPR-04-004-gemini-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:374` — Remove global error suppression to avoid masking independent errors.
  Resolved: Same fix as [TPR-04-003-codex-r2] — drop `engine.has_errors()` global gate; retain only `TypeFlags::HAS_ERROR` local check.
  Evidence: Same as [TPR-04-003-codex-r2].
  Required plan update: Same as [TPR-04-003-codex-r2].
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-004-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:415` — Pseudocode references APIs that do not exist; must rewrite against real Pool accessors.
  Resolved: Revised §3.3 on 2026-04-14. Replaced `pool.children(ty)` + `pool.extra(extra_off)` with the real tag-specific accessors:
  - `Tag::Function` → `pool.function_params(idx)` + `pool.function_return(idx)`
  - `Tag::Tuple` → `pool.tuple_elems(idx)`
  - `Tag::Applied` → `pool.applied_args(idx)` (generic args for `Option<[]>`, `Result<[], E>`, etc.)
  - `Tag::Struct` → `pool.struct_fields(idx)` (field tys)
  - `Tag::Enum` → `pool.enum_variants(idx)` (variant payloads)
  - `Tag::Map` / `Tag::Result` → `pool.map_key(idx)` + `pool.map_value(idx)` (or `pool.result_ok(idx)` + `pool.result_err(idx)`)
  - `Tag::List` / `Tag::Option` / `Tag::Set` / `Tag::Range` / `Tag::Iterator` / `Tag::DoubleEndedIterator` → `Idx::from_raw(pool.data(idx))` (child in DATA per `types.md TK-1` simple-container row)
  - `Tag::Scheme` → `pool.scheme_vars(idx)` + `pool.scheme_body(idx)`
  Evidence: `pool.children()` does NOT exist as a public method on `Pool`; `compiler/ori_types/src/pool/descriptor.rs:299-360` has only private `visit_children()`. `pool.extra()` is private (the `extra` field is private to `Pool`). `types.md TK-1` confirms the simple-container vs extra-backed split; `types.md TL-2` enumerates the complex-type layouts. The plan's pseudocode as written won't compile.
  Required plan update: Rewrite §3.3 recursion using the concrete tag-specific accessors enumerated above.
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-003-gemini-r2] — gemini independently flagged the same missing APIs.)

- [x] `[TPR-04-003-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:417` — Use correct accessors for complex type children instead of non-existent `children` method.
  Resolved: Same fix as [TPR-04-004-codex-r2] — replaced placeholder API references with real tag-specific accessors.
  Evidence: Same as [TPR-04-004-codex-r2].
  Required plan update: Same as [TPR-04-004-codex-r2].
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-005-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:129` — Align implementation with the claimed list-only scope (or broaden scope honestly).
  Resolved: Revised §1.5 on 2026-04-14. Chose the BROADEN-HONESTLY path: the fix's mechanics are naturally type-agnostic (the generalization guard disables for any non-lambda init; the validator walks any unresolved `Tag::Var`). Rather than special-casing by constructor (which would be a LEAK:scattered-knowledge hack per `impl-hygiene.md §SSOT`), §1.5 now documents that the validator catches ALL ambiguous let-binding types as a uniform safety net. The spec currently (`14-expressions.md:1224-1228`) only mandates `[]` ambiguity rejection, but `{}` and `Set<T>` ambiguity being rejected by the same mechanism is a happy coincidence — not a special case. If the spec should be expanded to explicitly cover `{}` and `Set<T>`, that's a spec proposal tracked separately. The TDD matrix tests focus on `[]` (spec-sanctioned) plus one `{}` regression test to document the incidental coverage.
  Evidence: `plans/bug-tracker/fix-BUG-04-074.md:129-133` claimed "scope NARROWED to empty lists only" — but §3.1/§3.2 disable generalization for EVERY non-lambda init, and §3.3's recursive walker checks ANY unresolved `Tag::Var`. The stated narrowing conflicts with the implemented mechanics.
  Required plan update: Either (a) honestly broaden the stated scope to match the mechanics, or (b) actually special-case by tag. Option (a) is more correct architecturally because (b) would introduce ad-hoc per-tag logic that violates SSOT.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-006-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:200` — Replace the invalid `id([])` lambda pin.
  Resolved: Revised §2 on 2026-04-14. The previous positive pin `let id = x -> x; let a = id([1, 2]); let b = id([])` had a logical flaw: `let b = id([])` provides NO element-type context for `[]`, so under the new semantics E2005 correctly fires for `b` — making the "pin" a false positive generator. Replaced with `let id = x -> x; let a = id([1, 2]); let b: [int] = id([])` where the annotation on `b` resolves the ambiguity, so the lambda remains polymorphic AND both applications type-check cleanly.
  Evidence: `compiler/ori_types/src/infer/expr/collections.rs:18-20` shows `[]` infers as `List(fresh_var)`. `let b = id([])` unifies `id`'s instantiation `(α) -> α` with `(List(β)) -> List(β)` — β is still unconstrained after the call, so `b : List(β)` with β unlinked. Under the fix, E2005 fires at body-exit on `b`. The plan's pin would FAIL post-fix, defeating its purpose as a polymorphism-preservation test.
  Required plan update: Change the pin to annotate `b` (`let b: [int] = id([])`) or otherwise constrain the element type downstream (e.g., `let b = id([]).push(value: 1)`).
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-005-gemini-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:195` — Include generic function parameter interaction test in the matrix.
  Resolved: Added new test to §2 on 2026-04-14: `test_empty_list_passed_to_generic_function_emits_E2005`. Test case: `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — no downstream constraint on `xs`'s element type, so E2005 must fire at body-exit. This verifies the `Tag::Var` survives generic unification (the generic `T` unifies with `List<β>` but β remains free) and is correctly flagged.
  Evidence: `tests.md §Interaction Testing` mandates 3 cross-feature tests, with "[] passed to generic function" explicitly listed. The round-1 matrix omitted this interaction.
  Required plan update: Added matrix row in §2 under "Cross-generalization-site coverage" + "Lambda let-polymorphism preservation" combined coverage.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-007-codex-r2][low]` `compiler/ori_llvm/src/codegen/type_info/store.rs:351` — Add an explicit consumer-side invariant check.
  Resolved: Added §3.6 on 2026-04-14 (new subsection). The fix adds a `debug_assert!` at the LLVM codegen entry for `get_or_compute_type_info` that panics if a `Tag::Var` survives to codegen. Per `impl-hygiene.md §Cross-Phase Invariant Contracts`, every typed-IR-to-codegen contract should be `debug_assert!`'d at the consumer's entry. Production behavior unchanged (the existing `TypeInfo::Error` path is retained as a release-build safety net).
  Evidence: `compiler/ori_llvm/src/codegen/type_info/store.rs:341-363` currently logs `unresolved type variable at codegen — type inference bug` but does not `debug_assert!`. Per `CLAUDE.md §Stabilization Discipline`, implicit invariants become invisible regressions — every cross-phase invariant should be either a test or a `debug_assert!`. This fix makes the typeck→codegen "no Tag::Var" contract explicit per `types.md PC-2` and `typeck.md PC-2`.
  Required plan update: Added §3.6 specifying the `debug_assert!` at the codegen entry point.
  Basis: direct_file_inspection. Confidence: medium.

### Round 2 revisions applied

Key architectural changes beyond round 1:

1. **Validator placement moved from per-let-site to per-body-exit** (addresses [TPR-04-001-codex-r2]): the most critical revision. Validation is now a sweep at the end of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) over all let-binding types recorded during that body. This gives downstream unification constraints (e.g., `push(10)`) time to fire before validation.
2. **`resolve_fully` is mandatory** (addresses [TPR-04-002-codex-r2] + [TPR-04-001-gemini-r2]): `has_unbound_var` calls `pool.resolve_fully(ty)` as its first step, walking `VarState::Link` chains per `pool/accessors.rs:412-491`. Without this, resolved-but-unsubstituted vars are false-positive'd as ambiguous.
3. **Real Pool API surface** (addresses [TPR-04-004-codex-r2] + [TPR-04-003-gemini-r2]): pseudocode rewritten against concrete tag-specific accessors (`function_params`, `applied_args`, `struct_fields`, `map_key`/`map_value`, `scheme_vars`/`scheme_body`, data-backed `Idx::from_raw(pool.data(idx))` for simple containers).
4. **Constructor + method-name fixes** (addresses [TPR-04-002-gemini-r2]): `ambiguous_type(span, var_id, context_desc)` signature honored; `engine.push_error(...)` not `record_error`. `has_unbound_var` return type changed to `Option<OffendingVar>` so the caller can build the diagnostic.
5. **Cascade gate tightened** (addresses [TPR-04-003-codex-r2] + [TPR-04-004-gemini-r2]): dropped module-wide `engine.has_errors()`; kept ONLY `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` local gate.
6. **Scope broadened honestly** (addresses [TPR-04-005-codex-r2]): §1.5 now documents that the validator catches ALL ambiguous let-binding types uniformly; `[]` is the spec-sanctioned case, `{}` and `Set<T>` ride for free without special-casing.
7. **Invalid lambda pin replaced** (addresses [TPR-04-006-codex-r2]): `let b: [int] = id([])` instead of the unconstrained `let b = id([])`.
8. **Generic-function interaction test added** (addresses [TPR-04-005-gemini-r2]): new `test_empty_list_passed_to_generic_function_emits_E2005`.
9. **Debug_assert at codegen entry** (addresses [TPR-04-007-codex-r2]): new §3.6 makes the typeck→codegen PC-2 contract explicit.

### Phase 2.5 — Plan TPR Round 3 (2026-04-14)

Plan TPR round 3 run `/tmp/ori-tpr-N7XgRiDB`. Codex walltime 431s (163 events, 13040-byte envelope). Gemini walltime 333s (82 events, 3962-byte envelope). ASYMMETRY MODERATE (bytes 14.3x RED is codex narration overhead; walltime 1.3x LOW; events 2.0x yellow). 9 actionable findings (codex 6, gemini 3, zero direct agreements but two near-perfect content overlaps on Pool API names + on non-let validation gap). Round 3 revealed THREE substantive design errors in round-2's approach + one architectural simplification opportunity. All findings independently verified against the code.

- [x] `[TPR-04-001-gemini-r3][high]` `plans/bug-tracker/fix-BUG-04-074.md:387` — Architecturally simpler design: use `pool.var_state(var_id)` directly, drop the `FxHashSet<u32>` push/pop machinery.
  Resolved: Round-3 §3.3 REWRITTEN on 2026-04-14. The new design inspects `pool.var_state(var_id)` for each `Tag::Var` encountered: `VarState::Unbound` → genuinely ambiguous (return `OffendingVar`); `VarState::Link` → resolved (already short-circuited by `resolve_fully` upstream); `VarState::Rigid` → user-annotated parametric (return None); `VarState::Generalized` → scheme-bound after `generalize()` ran (return None). The `FxHashSet<u32>` parameter is GONE.
  Evidence: `compiler/ori_types/src/unify/generalization.rs:47-54` confirms — `generalize()` mutates the `VarState` of every collected free var from `Unbound` to `Generalized` in-place BEFORE constructing the scheme. Therefore, by body exit, every bound var carries `VarState::Generalized` and is trivially distinguishable from genuinely unbound vars without the side-table dance. This is also robust against `resolve_fully`'s 16-hop limit (probe C from the round-3 prompt) because a non-resolved chain still ends in a `Tag::Var` whose `VarState::Unbound` correctly classifies it as ambiguous — no false positive.
  Required plan update: §3.3 rewritten to drop the `bound: &mut FxHashSet<u32>` parameter; replace with direct `var_state` inspection.
  Basis: fresh_verification. Confidence: high. (Verified: `generalization.rs:47-54` confirmed.)

- [x] `[TPR-04-001-codex-r3][high]` `plans/bug-tracker/fix-BUG-04-074.md:383` — Resolve each child in the recursion, not just the top-level entry.
  Resolved: §3.3 rewrite on 2026-04-14. The new `has_unbound_var` design calls `pool.resolve_fully(child)` AT EACH RECURSIVE STEP, not just at the top-level sweep entry. Combined with the `VarState`-based classification ([TPR-04-001-gemini-r3]), this makes the recursion robust: each child node is independently resolved through `Link` chains before its tag is inspected.
  Evidence: A child `Idx` in a compound type may itself be a `Tag::Var` whose `VarState::Link { target }` points elsewhere in the pool. Without per-child `resolve_fully`, the recursion would inspect the unresolved `Tag::Var` directly, misclassify it via the new VarState check (the Var's state after substitution may be Unbound for the head var even though the linked target is fully resolved), and emit a false positive.
  Required plan update: every recursive call into `has_unbound_var` MUST pass `pool.resolve_fully(child)` not the raw child `Idx`.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-002-codex-r3][medium]` `plans/bug-tracker/fix-BUG-04-074.md:495` — Use real Struct/Enum accessor names.
  Resolved: §3.3 rewrite on 2026-04-14. Replaced `pool.struct_field_types(ty)` with `pool.struct_fields(ty)` returning `Vec<(Name, Idx)>` (iterate destructuring `(_, field_ty)`); replaced `pool.enum_variant_payloads(ty)` with `pool.enum_variants(ty)` returning `Vec<(Name, Vec<Idx>)>` (nested iteration `for (_, payloads) in pool.enum_variants(ty) { for payload_ty in payloads { ... } }`).
  Evidence: `compiler/ori_types/src/pool/accessors.rs:538-551` defines `pub fn struct_fields(&self, idx: Idx) -> Vec<(ori_ir::Name, Idx)>`. `compiler/ori_types/src/pool/accessors.rs:606-629` defines `pub fn enum_variants(&self, idx: Idx) -> Vec<(ori_ir::Name, Vec<Idx>)>`. The plan's invented names (`struct_field_types`, `enum_variant_payloads`) do NOT exist on `Pool`. Won't compile.
  Required plan update: rewrite §3.3 pseudocode using the real accessor names with proper destructuring. Drop invalid `*param`, `*elem` pointer dereferences for accessors that already return owned `Vec<Idx>` not `&[Idx]`.
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-001-gemini-r3] — both reviewers independently found the same naming gap.)

- [x] `[TPR-04-001-gemini-r3-naming][high]` `plans/bug-tracker/fix-BUG-04-074.md:415` — Pseudocode uses non-existent `struct_field_types` / `enum_variant_payloads` accessors and invalid pointer dereferences.
  Resolved: Same fix as [TPR-04-002-codex-r3] above. Two independent reviewers converged on the same correction.
  Evidence: Same as [TPR-04-002-codex-r3].
  Required plan update: Same as [TPR-04-002-codex-r3].
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-003-codex-r3][medium]` `plans/bug-tracker/fix-BUG-04-074.md:431` — `Tag::Scheme` doesn't propagate HAS_VAR; the fast-path early-exit silently skips scheme bodies containing vars.
  Resolved: §3.3 rewrite on 2026-04-14. The new design SPECIAL-CASES `Tag::Scheme` BEFORE the `HAS_VAR` early-exit gate: `Tag::Scheme` always recurses into `pool.scheme_body(ty)` regardless of the parent's flags (with `resolve_fully` applied to the body first). The HAS_VAR early-exit is preserved for non-scheme types where it IS sound.
  Evidence: `compiler/ori_types/src/pool/mod.rs:651-652` confirms `Tag::Scheme => TypeFlags::IS_SCHEME` — only IS_SCHEME is set for schemes; `compute_flags()` does NOT propagate child flags through schemes (this differs from `types.md TF-3`'s aspirational PROPAGATE_MASK behavior — there's a documentation/implementation gap that's NOT the scope of this fix). With the broken propagation, `pool.flags(scheme_ty).contains(HAS_VAR)` returns false even when the scheme body contains unresolved Tag::Var nodes; the fast-path early-exit would skip the scheme entirely.
  Required plan update: §3.3 special-cases `Tag::Scheme` before the HAS_VAR early-exit, OR (out of scope here) fix `Pool::compute_flags()` to propagate flags through schemes.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-005-codex-r3][medium]` + `[TPR-04-002-gemini-r3][high]` `plans/bug-tracker/fix-BUG-04-074.md:354,320` — Per-body-exit `LetBindingRecord` sweep MISSES non-let ambiguous expressions (`[].len()`, `if [].is_empty() then ...`).
  Resolved: §3.3 fundamentally REDESIGNED on 2026-04-14. Dropped the `LetBindingRecord` side-table approach entirely. New design: at body exit, sweep ALL `expr_types` for the body (every expression's typed result, not just let bindings). For each expr_type, run `validate_type_resolved(engine, span, expr_type)` using the new VarState-based check. This naturally catches:
  - Let bindings (the original target).
  - Direct-receiver expressions like `[].len()` whose receiver is an ambiguous container.
  - Arguments to function calls when the call doesn't constrain the type.
  - Lambda body expressions in non-polymorphic contexts.
  - Any expression whose final type carries an unresolved Tag::Var.
  By body exit, generalize() has already run for every let-binding, so the in-pool VarState correctly reflects which vars are bound (Generalized) vs unbound (genuinely ambiguous). The round-2 concern about "blanket sweep regressing let-polymorphism" (TPR-04-001-codex-r1) is OBVIATED because the VarState-based check correctly distinguishes Generalized (bound, returns None) from Unbound (ambiguous, returns OffendingVar) — see [TPR-04-001-gemini-r3].
  Evidence: `compiler/ori_types/src/infer/expr/mod.rs:271-272` shows `infer_expr` stores a type for EVERY expression in `expr_types`. A program like `[].len()` has an expression `[]` typed as `List(Var(α))` where `α` remains Unbound (`.len()` doesn't constrain element type), and that ambiguous type sits in `expr_types`. The round-2 LetBindingRecord side table only captures let-binding sites, so `[].len()` falls through and leaks the Unbound var to codegen.
  Required plan update: drop LetBindingRecord; sweep all `expr_types` at body exit. Use the VarState-based has_unbound_var (no FxHashSet, no scheme push/pop).
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-004-codex-r3][medium]` `plans/bug-tracker/fix-BUG-04-074.md:570` — §3.6 `debug_assert!` only at TypeInfoStore is insufficient; `monomorphize::encode_type()` reads Pool types directly without going through that store.
  Resolved: §3.6 EXPANDED on 2026-04-14. New design: a single pre-codegen validation pass that walks every type reachable from a function's signature and body BEFORE emission begins, OR a `debug_assert!` at the top of the per-function codegen entry point in `ori_llvm` (whichever is more architecturally clean). The TypeInfoStore-level assertion is RETAINED as defense in depth. The pre-codegen pass uses the same `has_unbound_var` helper from §3.3 to ensure consistency.
  Evidence: `compiler/ori_llvm/src/monomorphize/mod.rs` (per codex finding — would need to be confirmed during implementation) reads Pool tags and children directly without going through `TypeInfoStore::get_or_compute_type_info`. Any Tag::Var that reaches monomorphize via that path bypasses the round-2 debug_assert.
  Required plan update: §3.6 broadened to specify a higher-level codegen-entry validation point covering all consumer surfaces, not just TypeInfoStore.
  Basis: direct_file_inspection. Confidence: medium. (Confidence medium because the exact best-placement decision needs validation during implementation; the principle — "broader than just TypeInfoStore" — is high-confidence.)

- [x] `[TPR-04-006-codex-r3][low]` `plans/bug-tracker/fix-BUG-04-074.md:620` — Round-1 §R resolution entries describe behavior that round-2 superseded; mark them as historical-superseded.
  Resolved: §R round-1 entries augmented on 2026-04-14 with `(SUPERSEDED-BY-ROUND-2: ...)` suffixes where applicable. Specifically: round-1 [TPR-04-001-codex] (cascade gate using `engine.has_errors()`) is superseded by [TPR-04-003-codex-r2] which dropped the global gate. Round-1 [TPR-04-007-codex] (helper extraction sound) is superseded by round-3 [TPR-04-004-codex-r3] (broader codegen coverage required). Round-1 [TPR-04-002-codex] (recursive contains_var) is superseded by round-3's `has_unbound_var` redesign. Other round-1 entries remain accurate as-of-round-2 and are preserved.
  Evidence: §R round-1 entries described validation-pass mechanics that round-2's ~3.3 rewrite changed substantially. Per `impl-hygiene.md §SSOT`, the §R history must be auditable — entries that no longer reflect the current plan are misleading without an explicit supersession marker.
  Required plan update: add `(SUPERSEDED-BY-ROUND-2/3: <citation>)` annotations to the affected round-1 entries.
  Basis: direct_file_inspection. Confidence: high.

### Round 3 architectural revisions applied

The round-3 findings are not patches — they reveal that round-2's design had multiple issues (wrong API names, missed scheme HAS_VAR propagation, non-let validation gap) AND that there's a fundamentally simpler design via VarState. Round-4 §3.3 reflects:

1. **VarState is the SSOT for binding status, not a scheme-vars side table.** `pool.var_state(var_id)` directly answers "is this var unbound, linked, rigid, or generalized?" without any external bookkeeping. The `FxHashSet<u32>` push/pop dance from round 2 is GONE.

2. **`resolve_fully` at every recursive step**, not just at the top-level entry. Children may be Linked elsewhere; per-child resolution makes the walker robust.

3. **Sweep ALL `expr_types` at body exit**, not just `LetBindingRecord` entries. By body exit, generalize() has run, so the VarState check correctly distinguishes generalized (bound) from unbound (ambiguous) — round-2's lambda-polymorphism concern is OBVIATED.

4. **Tag::Scheme special-cased before the HAS_VAR fast-path** since `Pool::compute_flags()` doesn't propagate HAS_VAR through schemes.

5. **Real Pool accessor names** — `struct_fields(ty)` and `enum_variants(ty)` returning `Vec<(Name, Idx)>` and `Vec<(Name, Vec<Idx>)>` respectively, with proper destructuring.

6. **§3.6 broadened** to a pre-codegen validation pass covering all codegen consumer surfaces, not just `TypeInfoStore`.

7. **§R historical entries marked superseded** where round-2/3 changed the behavior they describe.

### Phase 2.5 — Plan TPR Round 4 (2026-04-14)

Plan TPR round 4 run `/tmp/ori-tpr-VQvihlPG`. Codex walltime 450s (213 events, 11643-byte envelope). Gemini walltime 376s (155 events, 5006-byte envelope). ASYMMETRY MODERATE (bytes 12.1x codex narration overhead; walltime 1.2x LOW; events 1.4x LOW). **12 total findings: 9 actionable + 3 informational CONFIRMATIONS** — round 4's most important signal is the appearance of informational findings affirming round-3's architectural design. No architectural rollbacks required; all actionable findings are integration-level (API name corrections, determinism fix, placement corrections, module visibility, span retrieval API).

**Informational confirmations** (round-3 design held up under adversarial review):
- `[TPR-04-005-gemini-r4][informational]` — "VarState SSOT design is fundamentally sound and correctly models generalization" — verified against `generalization.rs:47-54`.
- `[TPR-04-006-gemini-r4][informational]` — "Tag::Scheme bypass of HAS_VAR early-exit is correct" — verified against `pool/mod.rs:651-652`.
- `[TPR-04-006-codex-r4][informational]` — "Keep the round-3 VarState simplification ... No semantic rollback is needed" — verified against `generalization.rs:29-58`.

**Actionable findings** (all resolved in this commit via §3.3 + §3.6 rewrites):

- [x] `[TPR-04-003-codex-r4][high]` + `[TPR-04-001-gemini-r4][high]` `plans/bug-tracker/fix-BUG-04-074.md:371,320` — Rewrite `validate_body_types` against the actual `InferEngine` API (content-agreement finding from both reviewers).
  Resolved: §3.3 rewritten on 2026-04-14. Validator signature changed to `(pool: &Pool, arena: &ExprArena, body_expr_types: &FxHashMap<ExprIndex, Idx>, record_error: &mut dyn FnMut(TypeCheckError))`. Span retrieval uses `arena.get_expr(ExprId::from_raw(expr_index as u32)).span`. No `engine.expr_span()` call. `InferEngine` exposes `pool()`, `expr_types()`, and `push_error()`; the closure pattern sidesteps borrow constraints between the validator's pool walk and the engine's error recording.
  Evidence: `compiler/ori_types/src/infer/mod.rs:52-56` defines `pub type ExprIndex = usize;` (NOT `ori_ir::ExprId`). `compiler/ori_types/src/infer/mod.rs:84-85` stores `expr_types: FxHashMap<ExprIndex, Idx>`. `compiler/ori_types/src/infer/context.rs:77` shows `InferEngine` holds no `ExprArena` reference and has no `expr_span()` method.
  Required plan update: Change signature + call path per above. Done.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-004-codex-r4][low]` + `[TPR-04-002-gemini-r4][high]` `plans/bug-tracker/fix-BUG-04-074.md:377,323` — Sort expr_types before deduping for DETERMINISM (content-agreement finding).
  Resolved: §3.3 rewritten on 2026-04-14. `validate_body_types` now collects entries into `Vec<(ExprIndex, Idx)>` and sorts by `ExprIndex` before iteration. The `seen: FxHashSet<Idx>` dedup is applied AFTER sorting, so the lowest-ExprIndex (earliest-in-source) expression with an ambiguous type always wins the diagnostic slot deterministically.
  Evidence: `FxHashMap` iteration order is non-deterministic per `impl-hygiene.md §Pass Composition — Pass determinism`. Without sort, which expression's span receives the E2005 diagnostic varies across compilation runs — a determinism violation.
  Required plan update: Insert sort step before iteration. Done.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-003-gemini-r4][high]` `plans/bug-tracker/fix-BUG-04-074.md:570` — LLVM codegen lacks `body_expr_types`; must validate `ArcFunction.var_types` instead.
  Resolved: §3.6 COMPLETELY REWRITTEN on 2026-04-14. New design has TWO validation points:
  - **§3.6a**: pre-collect-mono validation walks `mono_instances` type args + `function_sigs` param/return types at `compiler/ori_llvm/src/evaluator/compile.rs:230` (before `collect_mono_functions` runs).
  - **§3.6b**: per-function validation walks `arc_func.var_types: Vec<Idx>`, `arc_func.params`, and `arc_func.return_type` at the `FunctionCompiler::compile` entry.
  NEITHER uses `body_expr_types` — ori_llvm has no access to typeck AST structures.
  Evidence: `compiler/ori_arc/src/ir/mod.rs:375-387` defines `ArcFunction` with `var_types: Vec<Idx>` at line 387. ori_llvm's codegen operates on ArcFunction, not InferEngine's expr_types.
  Required plan update: Replace `body_expr_types` parameter with ArcFunction traversal. Done.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-001-codex-r4][high]` `plans/bug-tracker/fix-BUG-04-074.md:565` — Move the LLVM validator ahead of monomorphization.
  Resolved: §3.6 split into §3.6a (pre-collect-mono) + §3.6b (per-function) on 2026-04-14. §3.6a runs immediately BEFORE `collect_mono_functions()` at `compile.rs:230`, catching Tag::Var leaks in the monomorphization inputs (type args + signatures) at the earliest possible failure point. §3.6b runs per-function as a defense-in-depth layer.
  Evidence: `compiler/ori_llvm/src/evaluator/compile.rs:230-243` calls `crate::monomorphize::collect_mono_functions(mono_instances, function_sigs, interner, self.pool)` BEFORE any `FunctionCompiler` is constructed (line 243). A Tag::Var in a mono instance's type args would reach name mangling before any per-function validator could catch it.
  Required plan update: Add pre-mono validation step. Done.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-002-codex-r4][medium]` `plans/bug-tracker/fix-BUG-04-074.md:572` — Create a public validator boundary; current `ori_types::infer::expr::generalization_policy` is private.
  Resolved: §3.3 helper module location REVISED on 2026-04-14. Helpers (`validate_body_types`, `has_unbound_var`, `OffendingVar`) moved to NEW public module `compiler/ori_types/src/check/validators/mod.rs` with a `pub use check::validators::{has_unbound_var, OffendingVar};` re-export added to `compiler/ori_types/src/lib.rs`. This establishes a STABLE public API boundary that `ori_llvm` can import. `should_generalize` (§3.1) stays in the private `infer/expr/generalization_policy.rs` because it's AST-based and has no downstream consumer outside `ori_types`.
  Evidence: `compiler/ori_types/src/lib.rs:16-19` declares `mod infer;` as private. Attempting `use ori_types::infer::expr::generalization_policy::has_unbound_var` from `ori_llvm` would fail to compile.
  Required plan update: Move public surface to `check/validators/`; add re-export. Done.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-004-gemini-r4][low]` `plans/bug-tracker/fix-BUG-04-074.md:435` — Compilation error — incorrect loop binders for Vec accessors.
  Resolved: §3.3 pseudocode updated on 2026-04-14. Loop binders changed from `for &param in pool.function_params(ty)` to `for param in pool.function_params(ty)` (and analogously for `tuple_elems`, `applied_args`). These accessors return owned `Vec<Idx>`, not `&[Idx]`, so the reference-pattern `&param` would be a compilation error.
  Evidence: Consistent with round-3's verified `struct_fields(ty) -> Vec<(Name, Idx)>` and `enum_variants(ty) -> Vec<(Name, Vec<Idx>)>` — the Pool accessor family returns owned Vecs, not slices.
  Required plan update: Drop `&` from all loop binders on these accessors. Done.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-005-codex-r4][low]` `plans/bug-tracker/fix-BUG-04-074.md:869` — Add the promised supersession suffixes directly to the round-1 entries.
  Resolved: Round-1 §R entries directly annotated with `(SUPERSEDED-BY-ROUND-2/3: ...)` suffixes on 2026-04-14 (see next section below for the applied annotations). Prior round-3 summary described the supersession work but never made the per-entry edits — correctly caught as cosmetic by this finding.
  Evidence: The round-3 revisions summary at lines 869-872 + 891 claimed supersession suffixes were added to round-1 entries at lines 638-700, but those bullets remained unchanged plain `Resolved:` entries.
  Required plan update: Actually edit the affected round-1 bullets. Done (this commit).
  Basis: direct_file_inspection. Confidence: high.

### Round-1 §R supersession annotations (applied round 4 per TPR-04-005-codex-r4)

The following round-1 entries are historically accurate snapshots of round-1 resolutions, but describe behavior that round-2 or round-3 subsequently changed. They remain in place for audit-trail completeness with supersession markers:

- `[TPR-04-001-codex]` round 1: "resolved via narrowly-scoped validation with `engine.has_errors()` + `HAS_ERROR` flag cascade guards" — **SUPERSEDED-BY-ROUND-2** ([TPR-04-003-codex-r2] dropped the `engine.has_errors()` gate; cascade now relies on `TypeFlags::HAS_ERROR` local check only).
- `[TPR-04-002-codex]` round 1: "recursive `contains_var` negative pin" — **SUPERSEDED-BY-ROUND-3** (round-3 redesigned `has_unbound_var` entirely; the separate `contains_var` helper is no longer referenced; the negative pin now uses `has_unbound_var` directly from the public validator API).
- `[TPR-04-007-codex]` round 1: "helper extraction and per-body hook are structurally sound" — **SUPERSEDED-BY-ROUND-3-AND-4** (round-3 moved from per-body-hook-at-let-sites to body-exit sweep; round-4 moved the helper from `infer/expr/generalization_policy.rs` to public `check/validators/mod.rs`; the "per-body hook" description no longer matches).
- `[TPR-04-004-gemini]` round 1: "E2005 suggestion tailored by container kind — moot after scope narrowing" — **STILL VALID** (no supersession — scope narrowing was itself later revised in round 2 to honest broadening, but the underlying conclusion "don't over-specialize the message" remains correct).
- Other round-1 entries are accurate as-of-round-2 and are preserved without supersession markers.

### Round 4 corrections applied

1. **Validator signature matches real API** — `(pool, arena, body_expr_types: &FxHashMap<ExprIndex, Idx>, record_error: &mut dyn FnMut(TypeCheckError))`. Per [TPR-04-003-codex-r4] + [TPR-04-001-gemini-r4].
2. **Sort-before-iterate for determinism** — entries collected into `Vec<(ExprIndex, Idx)>` and sorted by ExprIndex before the dedup+walk. Per [TPR-04-004-codex-r4] + [TPR-04-002-gemini-r4].
3. **§3.6 uses `ArcFunction.var_types`, NOT typeck AST** — ori_llvm has no access to InferEngine's expr_types map. The ARC-IR layer is the SSOT for per-variable types at codegen time. Per [TPR-04-003-gemini-r4].
4. **§3.6a runs BEFORE `collect_mono_functions()`** — monomorphization input validation. §3.6b runs per-function for defense-in-depth. Per [TPR-04-001-codex-r4].
5. **Public validator module `ori_types::check::validators`** — with re-export through `lib.rs`. `should_generalize` stays private (AST-internal). Per [TPR-04-002-codex-r4].
6. **Loop binders drop `&`** — `pool.function_params/tuple_elems/applied_args` return `Vec<Idx>`, not slices. Per [TPR-04-004-gemini-r4].
7. **Round-1 §R entries actually annotated with SUPERSEDED markers** — per [TPR-04-005-codex-r4].

---

### Phase 2.5 — Plan TPR Round 5 (2026-04-16)

Plan TPR round 5 run on post-investigation-rewrite §3'. Codex walltime 675s, gemini walltime 236s. 3 actionable findings (2 Codex standalone + 1 Codex/Gemini AGREEMENT) + 1 Gemini related-to-F1 (subsumed) + 1 Gemini META (already-addressed). All actionable findings independently verified against the code; fix file §3' revised.

- [x] `[TPR-05-codex-F1][high]` + `[TPR-05-gemini-F1][medium]` `plans/bug-tracker/fix-BUG-04-074.md:1056` — §3.1'.a bucket partition misses composite return tags with `Fresh` subparts (e.g., `ResultOfProjectionFresh`). AGREEMENT on the deficiency; Codex identified the specific `ok_or` case, Gemini identified the multi-Fresh-param category it falls under.
  Resolved: §3.1'.a rewritten on 2026-04-16 (Round 5). Introduced `return_tag_fresh_arity(tag)` helper that counts `Fresh` positions within composite return tags (`ResultOfProjectionFresh` = 1 Fresh in Err slot). The correlation-slot decision now uses `param_fresh_arity + return_fresh_arity` instead of `matches!(returns, Fresh)`. `return_tag_to_idx` extended to accept an optional correlation slot that's used when it encounters `Fresh` inside a composite tag. `ok_or`'s `err: Fresh` param and `Result<Element, Fresh>` return now share one fresh_var.
  Evidence:
    - `compiler/ori_registry/src/defs/option/mod.rs:114` — `MethodDef::compound("ok_or", &ERR_PARAM, ReturnTag::ResultOfProjectionFresh(TypeProjection::Element), ...)` confirmed; `ERR_PARAM` has `ty: ReturnTag::Fresh`.
    - `compiler/ori_registry/src/tags/return_tag.rs:75` — `ResultOfProjectionFresh(TypeProjection)` documented as "Result<P, E> where P is a projection and E is fresh".
    - §3.1'.a pre-revision: `let return_is_fresh = matches!(method_def.returns, ReturnTag::Fresh);` — only direct `Fresh`, misses composite.
  Required plan update: Revise §3.1'.a bucket-1 trigger condition from direct `matches!(returns, ReturnTag::Fresh)` to a `fresh_arity`-aware count; thread the correlation slot into `return_tag_to_idx` so composite-Fresh return tags consume it.
  Basis: direct_file_inspection + fresh_verification. Confidence: high.

- [x] `[TPR-05-codex-F2][high]` `plans/bug-tracker/fix-BUG-04-074.md:1167-1172` / `compiler/ori_types/src/unify/mod.rs:289` — §3.2'.a replay hook can fire on var-to-var links before the receiver resolves to a concrete type. `Action::Link` at line 289 appends to `newly_linked_vars` for EVERY transition, including `Var(X) := Var(Y)` (var-to-var). If replay fires at that point, `new_recv_ty` is still `Tag::Var`, `resolve_builtin_method` returns `None`, impl-lookup also fails, and the replay prematurely emits `unknown_method` — a cascading false-positive diagnostic.
  Resolved: §3.2'.a revised on 2026-04-16 (Round 5). Added replay-gate: when an obligation's `receiver_var_id` appears in `newly_linked_vars`, follow the Link chain via `pool.resolve_fully(Var(receiver_var_id))` to find the current root. If root tag is still `Tag::Var` (linked to another var), leave the obligation pending — do not fire replay yet. If root tag is `Tag::Error`, remove the obligation without firing replay (cascade suppression per ER-4). Only fire replay when root is concrete (non-Var, non-Error). Replay gate is idempotent — the drain loop will re-visit the obligation on the next linking event.
  Evidence:
    - `compiler/ori_types/src/unify/mod.rs:281-286` — `Action::Link(rank)` dispatched for `VarState::Unbound { rank, .. }`; there is no distinction between var-to-var and var-to-concrete at this call site.
    - `compiler/ori_types/src/unify/mod.rs:289-292` — `*self.pool.var_state_mut(var_id) = VarState::Link { target: other }` fires unconditionally; `other` may itself be a Var.
    - §3.2'.a pre-revision pseudocode `replay_method_call(engine, ob, new_recv_ty=other)` — `other` is passed as `new_recv_ty` without a root-check gate.
  Required plan update: Add root-resolution gate before replay; document idempotent re-visit on subsequent linking.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-05-codex-F3][medium]` + `[TPR-05-gemini-F2][high]` `plans/bug-tracker/fix-BUG-04-074.md:1256-1290` / `compiler/ori_types/src/check/mod.rs:389` / `compiler/ori_types/src/check/exports.rs` — AGREEMENT from both reviewers: §3.4'.ii's proposed `validate_typed_ir_exit(pool, signatures, typed_bodies: &[TypedBody], ...)` signature assumes a `TypedBody` aggregate that `TypeCheckResult::finish_with_pool()` does NOT export. The shipped `TypedModule` is flat: `{ expr_types: FxHashMap<ExprIndex, Idx>, functions: Vec<FunctionSig>, ... }` — no per-function body aggregate, no `expr_idx → body_owner` reverse map, no per-body `spans` sidecar. The `scheme_var_ids` exempt set per function IS accessible via `FunctionSig.scheme_vars`, but binding an `expr_idx` to its owning function's scheme_var set is not possible without extending the export shape.
  Resolved: §3.4'.ii EXCISED on 2026-04-16 (Round 5). The per-body-exit validator (§3.3 shipped + §3.4'.iii wiring into passes 3/4/5) already has the full per-body context at the `engine.pop_context()` call site in each Bodies-group pass — each invocation has scoped access to `expr_types`, `arena`, `FunctionSig.scheme_vars`, and `record_error` with no API mismatch. §3.1'/§3.1'.a arg-param unification CLOSES the underlying leak that would have produced the scope-mismatch between the validator's view and codegen's view; once the leak is closed, the validator sees a clean typed IR at body exit and codegen never re-derives stale types. The §3.4'.ii "second validator pass at finish_with_pool" is redundant defense-in-depth against a leak that no longer exists post-§3.1'. §3.4' is renumbered: §3.4'.i (pass-2 audit), §3.4'.ii (now: passes 3/4/5 wiring — formerly §3.4'.iii), §3.4'.iii (now: test-attribute interaction — formerly §3.4'.iv), §3.4'.iv (now: pre-rollout test audit — formerly §3.4'.v), §3.4'.v (now: narrow-front threshold — formerly §3.4'.vi).
  Evidence:
    - `compiler/ori_types/src/check/mod.rs:389-402` — `TypedModule { expr_types: self.expr_types, functions, types, errors, warnings, pattern_resolutions, impl_sigs, trait_impl_fn_names, mono_instances, type_descriptors, exported_type_metadata, exported_collection_surfaces }`. NO `typed_bodies` field.
    - `compiler/ori_types/src/check/exports.rs:1-30` — `generate_export_descriptors(pool, functions: &[FunctionSig])` — no `TypedBody` type referenced anywhere in the exports module.
    - `FunctionSig.scheme_vars` — exists and accessible per `compiler/ori_types/src/output/mod.rs`, but only available at per-function granularity in the `functions: &[FunctionSig]` slice, not attached to individual `expr_idx` entries in the flat `expr_types` map.
  Required plan update: Delete §3.4'.ii typeck-exit validator sketch; reaffirm per-body-exit wiring (§3.4'.iii renumbered §3.4'.ii) as sole validator placement; renumber §3.4'.iv/v/vi accordingly; update §3.6' list to note that the "validator scope mismatch" follow-up item is resolved by arg-param unification (§3.1'/§3.1'.a), not by a second validator pass.
  Basis: fresh_verification + direct_file_inspection. Confidence: high. (Cross-reference: AGREEMENT — both reviewers independently identified the same data-contract mismatch.)

- [x] `[TPR-05-gemini-F3][low]` `plans/bug-tracker/fix-BUG-04-074.md:1314` — The ≤20 E2005 threshold in §3.4'.vi (post-renumber §3.4'.v) has no data-driven justification. META/DUPLICATE — the plan text at the cited line already explicitly labels the threshold "plan-local heuristic (NOT a rule-citation)" and "the quantitative threshold is plan-local, not CLAUDE-cited" per the prior TPR-05-R1-codex-F5 revision. The concern is already addressed; classifying as meta per `.claude/skills/tpr-review/SKILL.md §6`.
  Evidence: line 1314 — "the plan-local heuristic (NOT a rule-citation) is ≤20 concurrent E2005 emissions during the intermediate validator-wiring steps ... per TPR-05-R1-codex-F5 factual correction — the quantitative threshold is plan-local, not CLAUDE-cited".
  Required plan update: None — already resolved in a prior round's annotation.
  Basis: direct_file_inspection. Confidence: high.

### Round 5 architectural revisions applied

1. **§3.1'.a composite-Fresh handling** — `return_tag_fresh_arity(tag)` helper introduced; bucket-1 trigger condition generalized; correlation slot threaded through `return_tag_to_idx` for composite tags. Addresses TPR-05-codex-F1 + TPR-05-gemini-F1 AGREEMENT.
2. **§3.2'.a replay root-resolution gate** — replay fires only when receiver resolves through `pool.resolve_fully` to a non-Var, non-Error root; idempotent re-visit on subsequent linking. Addresses TPR-05-codex-F2.
3. **§3.4'.ii excised + §3.4' renumbered** — typeck-exit validator sketch removed; per-body-exit validator affirmed as sole placement; numbering collapsed. Addresses TPR-05-codex-F3 + TPR-05-gemini-F2 AGREEMENT.
4. **Gemini F3 classified as META** — already addressed by TPR-05-R1-codex-F5; no action.

---

## 4. Completion Checklist

Reviews MUST complete before bug closure.

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in type x pattern x feature grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_types` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Plan TPR (Phase 2.5) — completed (mandatory)
- [ ] `/tpr-review` (Phase 5 — code review) passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] `/sync-claude` doc sync verified
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Final `/commit-push`

**Exit Criteria:** The command `timeout 30 cargo run -q -- build /tmp/bug_04_074_repro.ori -o /tmp/test` exits 0 with no codegen errors when the repro file contains `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1`. The produced binary `/tmp/test` runs and exits 0. The matrix tests in § 2 all pass through both `cargo st` (interpreter) and the AOT path (LLVM). `timeout 150 ./test-all.sh` produces green output with no regressions. The let-polymorphism test `test_let_polymorphism_for_lambda` (verifying `let id = x -> x; id(1); id("hello")` still works) continues to pass, confirming the fix preserves polymorphism for function types.
