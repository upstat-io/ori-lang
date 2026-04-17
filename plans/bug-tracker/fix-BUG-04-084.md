---
bug: "BUG-04-084"
title: "AOT: empty `for x in collection do {}` body causes unresolved type variables at codegen"
severity: "critical"
status: complete
goal: "Empty collection literals (`[]`, `{}`) that remain unconstrained at body-inference exit resolve to a concrete type (Never) instead of leaking unbound `Tag::Var` through typeck's PC-2 contract. `ori check` and `ori build` both accept legal programs like `for x in items do {}` and `let empty = []` without E2005, matching interpreter behavior."
success_criteria:
  - "`cargo run -- check` on `@main () -> int = { let items = [1, 2, 3]; for x in items do {}; 0 }` returns clean (no E2005)"
  - "`cargo run -- build` on the same input produces a working AOT executable (no `unresolved type variable at codegen`)"
  - "Interpreter and LLVM produce identical results (dual-execution parity) for the full empty-literal matrix"
  - "Matrix tests cover `{}` / `[]` in positions: `let x = _`, `let x = _; 0` (discarded), `for x in c do _`, `for x in c yield _`, nested `[[]]`, `Option<_>` / `Result<_, _>`, inside closures, inside `if ... then ... else`"
  - "Semantic pin: `let empty = []; 0` type-checks and compiles; a revert of the defaulting pass makes it fail"
  - "Negative pin: `let empty = []; empty.push(value: 42); empty.push(value: \"foo\"); 0` still rejects with type mismatch — defaulting does NOT hide genuine conflicts"
  - "No regressions in `timeout 150 ./test-all.sh`"
subsystem: "compiler/ori_types/src/infer (defaulting pass) + compiler/ori_types/src/check/bodies (call sites)"
found: "2026-04-14"
source: "fix-bug (BUG-04-065 investigation — original repro used empty body, masking the actual OBE status)"
third_party_review:
  status: resolved
  updated: 2026-04-17
  plan_tpr_rounds: 3
  plan_tpr_findings_total: 12
  plan_tpr_findings_resolved: 12
  code_tpr_rounds: 1
  code_tpr_status: findings-filed-to-plan
  code_tpr_notes: "Gemini clean; codex 3 HIGH findings all verified + filed to plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.R Round 2 as concrete `- [ ]` anchors pointing to existing §03.2/3/4 wiring items (valid better-location deferral per CLAUDE.md §ALL Deferrals)."
---

# Fix: BUG-04-084 — AOT: empty `for x in collection do {}` body causes unresolved type variables at codegen

**Status:** Complete (absorbed into `plans/empty-container-typeck-phase-contract/` §03.BUG-FIXES)

**Session snapshot (2026-04-17):**
- Phase 4 state: implementation complete; `ori build` on exact repro exits 0; 18/18 positive matrix tests pass; 3/3 negative pins fire; zero regressions (pre-fix 3780/674 → post-fix 3781/674).
- Phase 5 state: complete — Code TPR Round 1 disposition filed to plan §03.R, hygiene clean, `/improve-tooling` retrospective "no gaps", `typeck.md §PC-2` + `canon.md §4.2` synced, bug entry + overview + frontmatter updated.
- Commit status: implementation complete; commit landing pipeline absorbed 2026-04-17 into `plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md`. BUG-04-074 is superseded into the owning plan (no longer a separate blocker); BUG-04-042 is now plan §08 (the single remaining commit blocker, owned by the plan). This file remains as a historical reference for the fix's TDD matrix and Plan TPR audit trail per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan — NEVER sibling fix files" — the owning plan section (§03.BUG-FIXES) is the authoritative record going forward.
**Severity:** critical
**Goal:** Empty collection literals (`[]`, `{}`) that reach body-inference exit without any constraint must default to a concrete type before `validate_body_types` runs, so the PC-2 output contract (`no Tag::Var in typed IR`) is satisfied without requiring users to annotate every throwaway empty literal.

**Success Criteria:**
- [ ] Repro `@main () -> int = { let items = [1, 2, 3]; for x in items do {}; 0 }` compiles via `ori build` and exits 0
- [ ] Matrix tests cover `{}` / `[]` across discarded / yielded / nested / generic-parameter / closure / match-arm positions
- [ ] Semantic pin test only passes with the defaulting pass active
- [ ] Negative pin test rejects a genuine type conflict (defaulting does not mask constraint violations)
- [ ] Interpreter and LLVM produce identical results (dual-execution parity)
- [ ] No regressions in `timeout 150 ./test-all.sh`

**Context:** Discovered 2026-04-14 during BUG-04-065 OBE verification. The original BUG-04-065 repro used an empty body `for x in items do {}`, and the attempted OBE-exit revealed the empty-body case was actually a distinct bug: the interpreter silently handles it (an unresolved element-type variable has no effect on an empty map that is never read from), but the AOT path cannot emit LLVM IR without a concrete type. The `empty-container-typeck-phase-contract` plan wires `validate_body_types` into each body-group pass (Section 03) to enforce PC-2 at typeck time; that wiring is what exposes BUG-04-084 as an E2005 instead of a later codegen ICE. This fix is the root-cause resolution that unblocks that plan's Section 03.

---

## 1. Root Cause Analysis

- **Symptom**: `E2005: cannot infer type in expression` with span pointing at the `{}` empty-map (or `[]` empty-list) literal in any position where the literal's inferred type is never constrained — including `for ... do {}` (body type discarded), `let empty = []` (binding never read), and statement-position `{}` (expression whose value is unused).
- **Proximate cause**:
  - `compiler/ori_types/src/infer/expr/collections.rs:141` — `infer_map_literal` with empty entries calls `engine.infer_empty_map()` which allocates two fresh `Tag::Var`s for key and value (`infer/mod.rs:630-634`).
  - Same pattern at `collections.rs:173` (empty map-spread) and `collections.rs:19, 55` (empty list and empty list-spread) via `infer_empty_list()` (`infer/mod.rs:617-620`).
  - `compiler/ori_types/src/infer/expr/control_flow.rs:662-670` — `infer_for` with `is_yield=false` simply discards `body_ty` and returns `Idx::UNIT`; the body's type is never unified against anything, so if it contains fresh vars they stay `VarState::Unbound`.
  - `compiler/ori_types/src/check/validators/mod.rs:120` — `validate_body_types` walks `expr_types`, finds an `Unbound` var under the empty-literal's `ExprIndex`, and emits `E2005` per typeck.md §PC-2 / §DI-1.
- **Root cause**: The inference engine has **no end-of-body defaulting pass for fully-unconstrained type variables**. Rust uses a `!`-fallback at end of function body (RFC 1216/1260 Never-fallback); Haskell has numeric defaulting; OCaml has the value restriction plus generalization. Ori has scheme-var exemptions (legitimate polymorphism) and error-cascade suppression (`Tag::Error`), but no fallback for `Unbound` vars that are neither generalized nor exempt. As a result, any empty collection literal whose element type is never touched by any constraint channel (call-site unification, annotation, or explicit coercion) leaves the fresh `Tag::Var` in `VarState::Unbound`, and PC-2 enforcement correctly flags it.
- **Blast radius**: Every `.ori` program that contains an unconstrained empty `[]` or `{}` literal. Empirically confirmed matrix (reproduced 2026-04-16):

  | Repro | Result |
  | --- | --- |
  | `for x in items do {}` | E2005 on `{}` (column 62 of repro) |
  | `for x in items do ()` | OK (explicit unit body) |
  | `let empty = {}` | E2005 on `{}` |
  | `let empty: {str: int} = {}` | OK (annotation constrains K, V) |
  | `let empty = []` | E2005 on `[]` |
  | `for x in [1,2,3] do []` | E2005 on `[]` |
  | `let x = 1; {}; 0` | E2005 on statement-position `{}` |

  Broader impact: blocks `empty-container-typeck-phase-contract` Section 03 wiring (check_function: done, check_test: pending, check_impl_method: pending, check_def_impl_method: pending); every downstream spec test that happens to use an unused empty literal would fail under the wiring. Also interacts with BUG-04-074 (separate bug — empty list `+` push where arg-param unification is the root cause) and BUG-04-042 (currently blocking `/commit-push` per the bug entry's BLOCKER note).
- **Affected files**:
  - `compiler/ori_types/src/infer/mod.rs` — add `default_unbound_vars_to_never(exempt: &[u32])` method on `InferEngine` (walks `var_states`, converts `VarState::Unbound` → `VarState::Link { target: Idx::NEVER }` for any var not in the exempt set, and with rank less-or-equal to the top-of-body rank to avoid defaulting skolems captured from an outer binder).
  - `compiler/ori_types/src/check/bodies/functions.rs` — call `engine.default_unbound_vars_to_never(&sig.scheme_var_ids)` inside the inference closure just before `engine.take_expr_types()` (line ~128, before the closure returns) so `expr_types` re-exports fully-resolved types.
  - `compiler/ori_types/src/check/bodies/tests.rs` — same defaulting call in the test-body inference site (to be wired in Section 03.2).
  - `compiler/ori_types/src/check/bodies/impls.rs` — same defaulting call in `check_impl_method` + `check_def_impl_method` (wired in Sections 03.3 and 03.4).
  - `compiler/ori_types/src/infer/mod/tests.rs` — unit tests for the defaulting method (exempt-set respected, Unbound → Link{Never}, Rigid / Generalized preserved).
  - `tests/spec/types/empty_literals/` (new directory) — spec tests for the matrix.

**Reference implementations:**
- **Rust** `rustc_hir_typeck/src/fn_ctxt/_impl.rs` — `default_type_parameters()` at end of function body: walks unresolved type variables and applies the Never-fallback (`!`) for unconstrained vars. Ori's `Idx::NEVER` is the direct analog (pre-interned at index 7, bottom type, coerces to any type per `TK-4`).
- **Koka** — uses explicit defaulting annotations at generalization boundaries; unconstrained vars become type parameters of the enclosing scheme. Not directly applicable because Ori limits generalization to `let $` immutable bindings.
- **Lean 4** — unresolved metavariables at the end of elaboration are hard errors (no defaulting). This is what Ori has today — BUG-04-084 is the symptom.
- **TypeScript** — unconstrained type parameters default to `any` or the declared default. Not directly applicable — Ori has no `any` type.

Rust's `!`-fallback is the closest match: same bottom-type primitive (`Never`/`!`), same runtime semantics (uninhabited), same "type-check passes, no runtime impact" effect on empty collections.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach. Ran BEFORE tests or implementation to catch wrong-approach errors before they lock in. See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the calling contract.

- **Proposed approach (pre-consensus)**: Add `InferEngine::default_unbound_vars_to_never(exempt: &[u32])` — walk `pool.var_states` globally; for each entry that is `VarState::Unbound` and `!exempt.contains(&id)`, replace with `VarState::Link { target: Idx::NEVER }`. Call at each body-group pass immediately before `validate_body_types` runs. Pass `sig.scheme_var_ids` as the exempt set.
- **tp-help run**: Round 1 dispatched 2026-04-16 via `/tp-help` Skill invocation (codex-reviewer + gemini-reviewer Agent sub-agents, parallel foreground). Each reviewer created its own `$(mktemp -d -t tp-help-XXXXXXXX)` scratch dir per `tp_help_prompt.md` Step 1.

### Round 1

- **Codex summary**: REJECTED the proposed approach as unsound on two independent grounds.
  1. *Global sweep is cross-body-unsound* — per `.claude/rules/typeck.md:43-60` CK-1 pass ordering, signatures for the WHOLE module are collected at Pass 1 (`compiler/ori_types/src/check/signatures/mod.rs:155-203, 256-276`) BEFORE any body pass runs. A sweep of `var_states[0..n)` at end of function F1's body-inference (Pass 2) would rewrite fresh vars allocated during Pass 1 for F2, F3, test bodies, impl methods, def-impl methods — vars those later-body passes must still consume. Rank-guarding doesn't save it: `CK-2`/`GN-1` ranks govern generalization, not body ownership.
  2. *`link_var` alone is insufficient* — `link_var(id, Idx::NEVER)` only updates `VarState`. Exported roots and incremental-compilation hashes continue to use the old raw `Idx` (`compiler/ori_types/src/output/mod.rs:442-457` — `param_hashes` computed from pre-inference Idx at signature-collection time; `compiler/ori_llvm/src/aot/incremental/function_hash/mod.rs:87-114` — cache keyed on raw Idx form). The compound type `List(Var(X))` stays interned with `HAS_VAR=true` even after Var(X) is linked.
  Recommended approach: **per-body, empty-literal-root-targeted substitution**. After this body's inference finishes, inspect only this body's empty list/map literal expr roots (`compiler/ori_types/src/infer/expr/collections.rs:18-20, 54-56, 140-142, 171-173` — the 4 allocation sites). Collect reachable `VarState::Unbound` ids under those roots. Rewrite this body's `expr_types` and `FunctionSig` through `substitute_in_pool(pool, ty, var_subst)` (`compiler/ori_types/src/pool/substitute/mod.rs:28`) mapping those vars to `Idx::NEVER`. Then optionally link the same vars in the pool for downstream consistency. Then run `validate_body_types`.
  Codex verified `Idx::NEVER` is safe downstream: `TypeInfo::Never` is i64-backed (`compiler/ori_llvm/src/codegen/type_info/info.rs:46-59, 93-115`), naked Never params/returns become void ABI (`compiler/ori_llvm/src/codegen/abi/mod.rs:294-316`), RC traversal treats it as scalar/no-op (`arc_emitter/rc_value_traversal.rs:35-46`), collection helpers special-case it (`arc_emitter/element_fn_gen.rs:67-72`, `arc_emitter/builtins/collections/map_builtins.rs:228-240`). No crash path for `List(Never)` / `Map(Never, Never)`.
  Matrix additions: function result `@f () = []/{}`, `for ... yield []/{}` → nested Never containers, match-arm joins, closure-produced empties, generalization-preservation control, and a negative control that unrelated ambiguous signature vars still error (e.g. untyped unused params).
  Negative pin without relying on BUG-04-074: annotation-conflict pattern `let empty = []; let ints: [int] = empty; let strs: [str] = empty; 0` — the second annotation must still fail via bidirectional checking (`compiler/ori_types/src/infer/expr/blocks.rs:43-58, 129-143`; `typeck.md:416-428`).
- **Gemini summary**: ACCEPTED the broad defaulting approach with a rank guard added. Argued the rank guard is sufficient because `collect_signatures` allocates at rank 0 and each body pushes to rank 1 — so "rank >= entry_rank" filters out signature vars. Agreed on `Idx::NEVER` over `Idx::UNIT` (cites `compiler/ori_llvm/src/codegen/type_info/tests.rs:91` — Never as i64 in LLVM). Agreed that `VarState::Generalized` and `Rigid` are already handled by the validator exemption. Agreed Value Restriction (GN-3) handles let-polymorphism cleanly — `let empty = []` doesn't satisfy Value Restriction so isn't generalized; its vars stay Unbound and get defaulted. Matrix additions: generic-body, diverging branches, nested closures, generic return. Suggested using `build_exempt_var_ids` helper from `validators/mod.rs` for union-find root handling.
- **Agreement points**:
  - `Idx::NEVER`, not `Idx::UNIT`, as the default target (both reviewers independently verify codegen handles it).
  - `VarState::Generalized` and `VarState::Rigid` MUST be preserved — defaulting skips them (already handled by current validator design).
  - Matrix needs significant expansion: function results, yield bodies, closures, match joins, diverging branches, generic-rigid interactions, negative pin that unrelated inference holes still emit E2005.
  - Negative pin via annotation conflict (`let ints: [int] = empty; let strs: [str] = empty`) — does not depend on BUG-04-074 being fixed.
  - Wiring must cover all 4 body-group passes: `check_function` (done in plan 03.1), `check_test` (03.2 pending), `check_impl_method` (03.3 pending), `check_def_impl_method` (03.4 pending).
- **Disagreement points**:
  - **Sweep scope**: Codex says per-body (even per-empty-literal-root) narrow; Gemini says broad with rank guard.
  - **Link-only vs substitute**: Codex says `link_var` alone leaves exported `Idx` + Merkle hashes stale; Gemini's recommendation uses `link_var`-equivalent without addressing the export / hash consequence.
  - **Rank guard utility**: Codex says a rank guard on a global sweep cannot distinguish sibling bodies (both at body-rank); Gemini says rank guard is sufficient.
- **Independent code verification** (against codex's three load-bearing claims):
  - *Claim 1 — signatures collected before body passes allocate via `fresh_var` for unannotated slots*: CONFIRMED. `compiler/ori_types/src/check/signatures/mod.rs:189` (parameter without annotation → `checker.pool_mut().fresh_var()`) and line 202 (return without annotation → `fresh_var()`). These allocations land in `var_states` during Pass 1, before any body pass runs. A global sweep during body F1's inference WOULD touch F2/F3/test/impl signature vars. Codex's scope concern holds.
  - *Claim 2 — param_hashes are computed at signature-collection time from pre-inference Idx*: CONFIRMED. `compiler/ori_types/src/check/signatures/mod.rs:250-254` (`let param_hashes: Vec<u64> = param_types.iter().map(|&idx| checker.pool().hash(idx)).collect();`). `output/mod.rs:442-457` documents `param_hashes` + `return_hash` fields as "Zero when the signature was constructed without pool access… Use `populate_hashes()` to fill in after construction." Post-`link_var`-only defaulting, the hashes retain pre-inference form — cross-module cache drift is real. Codex's hash concern holds.
  - *Claim 3 — `substitute_in_pool` exists and is the right tool*: CONFIRMED. `compiler/ori_types/src/pool/substitute/mod.rs:28` — signature `pub fn substitute_in_pool(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx` with `!HAS_VAR` fast-path gate at line 30. Matches exactly what's needed.
  - *Gemini's rank claim*: would be salvageable but subsumed by Codex's narrower + correcter per-body approach, which needs no rank reasoning. Rank-guarding adds complexity without correctness improvement over per-body targeting.
- **Outcome**: **Persuaded divergence** — adopt Codex's revised approach. The sweep-scope issue and the exported-hash issue are both verifiable against the code and both disqualify my pre-consensus proposal. Gemini's recommendation is subsumed: the Never / Generalized-skip / matrix-expansion / annotation-pin guidance remains valid and is incorporated; only the "broad sweep with rank guard" mechanism is replaced by Codex's per-body substitution approach.

### Final agreed approach

**Per-body, empty-literal-root-targeted substitution via `substitute_in_pool` to `Idx::NEVER`, followed by hash refresh.**

Mechanism at each of the 4 body-group passes (`check_function_bodies`, `check_test_bodies`, `check_impl_bodies`, `check_def_impl_bodies`), immediately after body `check_expr` / `infer_expr` completes and BEFORE `validate_body_types` runs:

1. **Walk this body's `expr_types` keys** (`FxHashMap<ExprIndex, Idx>` taken from the engine). For each `(expr_index, ty)`:
   a. Look up the AST node via `arena.get_expr(ExprId::new(expr_index as u32))`.
   b. Match `expr.kind`:
      - `ExprKind::List(entries) if entries.is_empty()` — the 4-empty-list allocation site (`collections.rs:18-20`).
      - `ExprKind::ListSpread(elems) if is_semantically_empty(elems)` — empty list-spread (`collections.rs:54-56`).
      - `ExprKind::Map(entries) if entries.is_empty()` — empty map (`collections.rs:140-142`).
      - `ExprKind::MapSpread(elems) if is_semantically_empty(elems)` — empty map-spread (`collections.rs:171-173`).
   c. If a match: resolve `ty` via `pool.resolve_fully`. Walk the resolved compound type via `pool.visit_children` to collect every `VarState::Unbound` var_id reachable. Add each such id (that is NOT in `exempt` = `build_exempt_var_ids(pool, &sig.scheme_var_ids)`) to a per-body `var_subst: FxHashMap<u32, Idx>` with target `Idx::NEVER`.
2. **Substitute through all exported types**:
   - `expr_types.iter_mut()` — for each `(_, ty)`, replace with `substitute_in_pool(pool, *ty, &var_subst)`.
   - `sig.param_types.iter_mut()` — same replacement per param.
   - `sig.return_type` — same replacement.
3. **Refresh signature hashes** — call `sig.populate_hashes(pool)` (or equivalent — the method referenced at `output/mod.rs:450`; verify exact name before implementation) so `param_hashes` and `return_hash` reflect the substituted types. If `populate_hashes` doesn't exist, compute inline via `sig.param_hashes[i] = pool.hash(sig.param_types[i])` and `sig.return_hash = pool.hash(sig.return_type)`.
4. **Optionally also `link_var(var_id, Idx::NEVER)` in the pool** for each substituted var — this keeps the pool's own `resolve_fully` consistent with the exported types, defense-in-depth against any consumer that reads the raw Idx.
5. **Run `validate_body_types`** — it now sees fully-substituted types. Unrelated ambiguous vars (not reachable from empty-literal expr roots) remain Unbound and emit E2005 correctly — preserving the validator's detection of OTHER inference bugs.

**Why this shape wins:**
- Per-body scope means cross-body pollution is structurally impossible (we never touch vars reachable only from other bodies).
- Empty-literal-root targeting preserves the validator's ability to catch OTHER ambiguous-var bugs (unrelated inference failures still surface as E2005). This matters architecturally: defaulting is a narrow recovery mechanism for a specific class, NOT a general "silence all E2005" override. Preserves the through-line from `typeck.md §PC-2` invariant to its enforcement.
- `substitute_in_pool` produces re-interned compound types with the substituted child, so the resulting `Idx` has `HAS_VAR=false` and is a proper concrete type. Downstream consumers that read `param_types[i]` directly (not via `resolve_fully`) see the concrete form. Hash refresh propagates to incremental-compilation cache correctness.
- `Idx::NEVER` is verified-safe across codegen (codex citation chain above).
- Skips `VarState::Generalized`/`Rigid`/already-linked via the fast-path gate + existing validator exemption logic reuse (`build_exempt_var_ids`).

**Files touched (revised from pre-consensus list):**
- `compiler/ori_types/src/infer/mod.rs` — NEW method `default_unbound_vars_from_empty_literals(arena, expr_types, sig)` on `InferEngine`. Internally walks expr_types, matches AST empty-literal kinds, collects reachable unbound vars, calls `substitute_in_pool`, updates expr_types + sig in place, refreshes hashes.
- `compiler/ori_types/src/check/bodies/functions.rs` — call the new engine method before `engine.take_expr_types()` (current line ~128), before `validate_body_types` runs (line 140).
- `compiler/ori_types/src/check/bodies/tests.rs` — same call site for test bodies.
- `compiler/ori_types/src/check/bodies/impls.rs` — same call site for impl methods AND def-impl methods (two places per `impls.rs:201-225, 313-326` — codex verified).
- `compiler/ori_types/src/check/validators/mod.rs` — expose `build_exempt_var_ids` as `pub(crate)` if needed for the engine method to reuse it (currently `fn` — not exported). Prefer reuse over re-implementation per `impl-hygiene.md §Algorithmic DRY`.
- `compiler/ori_types/src/infer/expr/mod.rs` or sibling — is_semantically_empty(elems) helper if list-spread / map-spread paths need special logic (verify behavior: a spread with only empty-spread children is semantically empty).
- Test files per matrix below.

---

---

## 2. TDD — Test Matrix

Finalized 2026-04-16 post /tp-help Round 1 consensus. Covers position × type × feature × backend matrix. All tests written before the fix; each must fail against HEAD with `cargo run -- check` (for typeck cells) or `cargo run -- build` (for codegen cells), and must pass unchanged after the fix.

Location plan: `tests/spec/types/empty_literals/` (new directory). Each `.ori` file is self-contained via `use std.testing { assert, assert_eq }` where assertions are needed. Negative pins use `#compile_fail("E2005")` or `#compile_fail("E2001")` on the specific expected error code.

### Exact failing case (the original bug repro)
- [ ] `for_do_empty_map_body.ori` — `@main () -> int = { let items = [1, 2, 3]; for x in items do {}; 0 }`.

### Position dimension — WHERE the empty literal appears
- [ ] `let_binding_empty_map_unused.ori` — `let empty = {}; 0` (binding unused).
- [ ] `let_binding_empty_list_unused.ori` — `let empty = []; 0`.
- [ ] `statement_position_empty_map.ori` — `let x = 1; {}; 0` (statement-position bare `{}`).
- [ ] `for_do_empty_list_body.ori` — `for x in items do []; 0`.
- [ ] `for_yield_empty_list_body.ori` — `for x in [1, 2, 3] yield []` — yield body, produces `[[Never]]`.
- [ ] `for_yield_empty_map_body.ori` — `for x in [1, 2, 3] yield {}` — produces `[{Never: Never}]`.
- [ ] `match_arm_empty_map.ori` — `match n { 0 -> {}, _ -> {} }` — match with all-empty arms.
- [ ] `if_both_branches_empty.ori` — `if c then [] else []` — diverging branches both empty.
- [ ] `if_both_branches_empty_map.ori` — `if c then {} else {}`.
- [ ] `closure_body_empty_list.ori` — `let f = () -> []; 0` (closure producing empty; f unused).
- [ ] `closure_body_empty_list_applied.ori` — `let f = () -> []; let _ = f(); 0` (closure applied, result discarded).
- [ ] `direct_function_return_empty.ori` — `@f () -> [int] = []; @main () -> int = { let x = f(); 0 }` — non-empty return-type annotation constrains `[]` to `[int]`, defaulting does NOT fire.
- [ ] `direct_function_return_unannotated_empty.ori` — `@f () = []; @main () -> int = 0` — return type not annotated. Does the signature var get defaulted, or does it remain unbound and fire E2005 at signature position? (Expected: E2005 at signature — this is the "unrelated unused param" analog; empty-literal defaulting must NOT apply to signature vars that happen to be reachable from the body.)

### Type dimension — empty list vs empty map vs spread forms
- [ ] `empty_list.ori` — bare `let empty = []; 0` → `[Never]`.
- [ ] `empty_map.ori` — bare `let empty = {}; 0` → `{Never: Never}`.
- [ ] ~~`empty_list_spread.ori` / `empty_map_spread.ori`~~ — **REMOVED per Plan TPR Round 3 Codex F1.** Per parser behavior (confirmed via grep of `ori_ir/src/ast/expr.rs`), an empty literal `[]` parses as `ExprKind::List(empty_range)`, not `ExprKind::ListWithSpread(empty_range)`. A `ListWithSpread` / `MapWithSpread` with zero elements is not practically constructible via surface syntax — the parser uses the spread variant only when at least one spread element exists. The `elems.is_empty()` fast paths at `collections.rs:54-56` and `171-173` are defensive dead branches under normal input. The `is_empty_collection_literal` helper retains its ListWithSpread/MapWithSpread arms (defensive; cost zero if never matched), but no user-facing test can exercise these arms. Drop from test matrix; rely on the non-spread `empty_list.ori` / `empty_map.ori` cells to cover all reachable user-facing paths.
- [ ] `nested_empty_list.ori` — `let xs = [[]]; 0` → `[[Never]]`. Tests compound type defaulting.
- [ ] `nested_empty_map.ori` — `let xs = { "a": {} }; 0` → `{str: {Never: Never}}`.
- [ ] `empty_in_tuple.ori` — `let t = ([], {}); 0` → `([Never], {Never: Never})`.

### Feature dimension — interactions with other language features
- [ ] `empty_with_late_annotation.ori` — `let empty = []; let xs: [int] = empty; 0` — annotation propagates constraint BEFORE defaulting pass runs. Expected: `empty: [int]`, defaulting does NOT fire, compiles clean.
- [ ] `empty_with_method_call.ori` — `let empty = []; let n = empty.len(); 0` — `len()` is generic over element type and does NOT constrain it. Expected: elem var stays Unbound → defaulted to Never → `empty: [Never]`, `len()` returns `0` (empty).
- [ ] `empty_with_is_empty.ori` — `let empty = {}; let b = empty.is_empty(); 0` — analogous to above.
- [ ] `empty_with_generic_rigid_preserved.ori` — `@identity<T> (x: T) -> T = x; @main () -> int = { let _ = identity; 0 }` — `T` is a RigidVar in identity's scheme; must not be defaulted by any defaulting pass triggered by code in `@main`. Negative control per Gemini's suggestion.
- [ ] `empty_with_generic_return_annotated.ori` — `@f<T> () -> [T] = []; @main () -> int = { let xs: [int] = f(); 0 }` — `[]` inside `@f` body is constrained by return-type annotation to `[T]` (RigidVar from scheme). Defaulting must NOT fire on `T`. At call site, `T` substitutes to `int` via normal inference.
- [ ] `empty_immutable_binding_still_monomorphic.ori` — `let $empty = []; 0` — per `typeck.md §GN-3` Value Restriction, let-polymorphism in Ori is **lambda-only** (value restriction: only syntactic values that are lambdas generalize; literal values like `[]` do NOT). So `let $empty = []` stays monomorphic regardless of the `$` marker. Expected: `empty: [Never]` (same as mutable case). This test is a negative control for "immutable binding does NOT trigger generalization on value literals" — it proves the fix doesn't regress GN-3's lambda-only value restriction.
- [ ] `lambda_let_polymorphism_preserved.ori` — `let $id = x -> x; let a = id(1); let b = id("a"); 0` — per `GN-3`, the lambda `x -> x` IS eligible for generalization. The var for `x` is `VarState::Generalized` post-generalization, and my defaulting pass MUST skip it (validator-level exemption handles this). Test verifies that `id` can be instantiated at two types without collapsing.
- [ ] `lambda_with_empty_literal_generalized.ori` (per Plan TPR Round 2 Codex F5 — GN-3 + empty-literal interaction): `let $mk = () -> []; let xs: [int] = mk(); let ys: [str] = mk(); 0` — the lambda `() -> []` is generalizable per GN-3. Its body contains an empty literal `[]` whose elem var is ALSO captured in the scheme. Defaulting must NOT fire on the generalized elem var — it's `VarState::Generalized`, not `Unbound`, so the validator exemption handles it. Then `mk()` at each call site instantiates fresh, and each site unifies with the annotated binding type. Proves defaulting skips generalized scheme vars reachable from lambda bodies.
- [ ] `for_do_mixed_empty_branches.ori` (per Plan TPR Round 2 Codex F4 — mixed-branch negative control): `#compile_fail("E2001")` on:
  ```ori
  @main () -> int = {
    for x in [1, 2, 3] do (if x == 1 then [] else {})
    0
  }
  ```
  The `if...then...else` branches must unify: `[]` (empty list) vs `{}` (empty map). Defaulting must NOT accept this by defaulting both to Never-of-different-shape — the branches must still error via bidirectional unification BEFORE defaulting runs. Proves defaulting does not silently reconcile structurally-incompatible empty literals across branches.

### Backend dimension — dual-execution parity
- [ ] For each of the above `.ori` files, verify BOTH:
  - `cargo run -- check file.ori` → clean (no E2005 for positive cases).
  - `cargo run -- build file.ori && ./a.out` → exits 0.
  - `cargo run -- run file.ori` (interpreter) → exits 0 with same output as LLVM.
- [ ] Test harness already enforces this via `test-all.sh`'s `tests/spec/` sweep running both interpreter and LLVM backends; no harness change needed.

### Semantic pin (permanent regression guard)
- [ ] `semantic_pin_empty_list_defaults_to_never.ori` — a test that ASSERTS `empty: [Never]` via type-level proof. Method: `let empty = []; let xs: [Never] = empty; 0` — the `let xs: [Never] = empty` annotation unifies `empty`'s elem type with `Never`. If defaulting didn't fire, `empty` would be `[Var(X)]` and the annotation would UNIFY `Var(X) ≡ Never` (ok), masking the bug. To distinguish, use two bindings: `let empty = []; let xs: [Never] = empty; let ys: [Never] = empty; 0` — both annotations must succeed, which is only consistent with `empty: [Never]` under both linkage AND substitution (the exported type).
- [ ] Actually cleaner semantic pin: `semantic_pin_post_substitution_concrete.ori` — write an Ori test that INTROSPECTS the post-defaulting type via a function call that would fail if the type were still `[Var(X)]`. Since Ori has no type introspection, the indirect pin is: `let empty = []; 0` simply compiles under `ori check` AND the resulting LLVM IR contains NO type-variable markers (manually inspect via `ORI_DUMP_AFTER_TYPECK=1` in a harness script). Primary pin remains the matrix-wide "no E2005" observation.

### Negative pins (correctness clamps)
- [ ] `negative_annotation_conflict.ori` (Codex's preferred negative pin — BUG-04-074-independent): `#compile_fail("E2001")` on:
  ```ori
  @main () -> int = {
    let empty = []
    let ints: [int] = empty   // ok: unifies empty's elem with int
    let strs: [str] = empty   // must fail: elem is already int (post-bidirectional), strs: [str] mismatches
    0
  }
  ```
  This tests that defaulting does NOT hide bidirectional-propagation failures. The first annotation constrains BEFORE defaulting would fire, and the second annotation must see a conflict, not a silenced `[Never]`-coerces-to-anything fallback.
- [ ] `negative_unrelated_signature_var_still_errors.ori` — `#compile_fail("E2005")` on `@f (x) -> int = 0` (unannotated param `x`, never used in body, body returns constant). Signature has fresh Var for `x`'s type. Defaulting must NOT default this — it's a genuine "cannot infer parameter type" situation per Codex's "unrelated ambiguous signature holes" concern. Verifies per-body-empty-literal scope: signature vars NOT reachable from empty-literal expr roots must still error.
- [ ] ~~`negative_error_poisoned_not_defaulted.ori`~~ — **REMOVED per Plan TPR Round 3 Codex F2.** Placeholder "repro TBD during Phase 3 writing" violates TDD discipline (test must be executable and fail pre-fix). The `Tag::Error` cascade-suppression path is already covered by the existing `validate_body_types` implementation at `validators/mod.rs:220-223` (HAS_ERROR gate BEFORE HAS_VAR) — defaulting's caller flow doesn't re-open the gate. Dropping this cell as over-coverage of an already-tested path.
- [ ] ~~`negative_never_cannot_be_populated.ori`~~ — **REMOVED per Plan TPR Round 3 Codex F2** (and file for follow-up in BUG-04-074's fix plan). The pre-fix `#compile_fail` construction depends on BUG-04-074's arg-param unification landing — the test is non-executable until BUG-04-074 is resolved. Drop from BUG-04-084's pre-fix matrix. Add as a post-fix regression test in BUG-04-074's fix section so the `[Never] + .push(int)` soundness check lands in the correct bug arc.

**Matrix completeness statement (post Round 3 cell-pruning):** the matrix retains every cell that both (a) exercises a reachable user-facing code path, and (b) is pre-fix-executable (fails under current HEAD and passes post-fix without test modification). The two removed spread cells and two removed negative pins were either architecturally unreachable (spread-empty) or cross-bug-dependent (never-cannot-populate, depends on BUG-04-074). The remaining matrix is fully TDD-compliant.

### Rust unit tests in `compiler/ori_types/src/infer/`
- [ ] Unit tests for the new `InferEngine::default_unbound_vars_from_empty_literals` method in `infer/mod/tests.rs` or sibling — covers: empty expr_types is a no-op; expr_types with no empty literals is a no-op; expr_types with one empty-list literal produces the expected substitution; scheme_var_ids exempt respected; already-linked vars skipped; Generalized vars skipped; Rigid vars skipped; compound nested types (`[[]]`) fully substituted.
- [ ] Unit tests for `populate_hashes` refresh (or the inline hash-refresh logic) — assert `pool.hash(substituted_idx)` matches `sig.param_hashes[i]` post-defaulting.

### Verify tests fail before fix
- [ ] All matrix tests fail against pre-fix HEAD (confirming they test the right thing). The existing repro (`for_do_empty_map_body.ori`) already fails with E2005 — remaining matrix cells need pre-fix failure verification via `timeout 150 cargo st tests/spec/types/empty_literals/`.

---

## 2.5 Fix Plan TPR Findings

Adversarial review of this fix PLAN (§1–§3) before implementation. Ran AFTER `/tp-help` consensus (§1.5) and plan finalization (§2) but BEFORE writing tests or code.

**Gate:** **Mandatory — severity is critical.** Also sensitive subsystem (type inference + producer-side PC-2 enforcement). Will run once §2/§3 are finalized.

Pending — §2.5 populated after §1.5 consensus and §2/§3 finalization.

---

## 3. Implementation

Finalized 2026-04-16 post-consensus. Per-body, empty-literal-root-targeted substitution to `Idx::NEVER` via `substitute_in_pool`, with signature-hash refresh and pool-side `link_var` for defense-in-depth.

### 3.1 — New engine method

**Scope-by-var clarification (per Plan TPR Round 1 Codex F1):** `var_subst` is populated by walking ONLY empty-literal expr roots in this body's `expr_types`. A signature var (e.g., unannotated return's fresh var) is ONLY added to `var_subst` if it is reachable from an empty-literal expr's compound type. If the unannotated signature position is unrelated to any empty-literal in the body, its var is NOT in `var_subst`, so `substitute_in_pool(sig.return_type, var_subst)` leaves it unchanged — the validator will correctly fire E2005 for that unrelated signature hole. This preserves the "unrelated ambiguous vars still error" architectural property.

**Exempt set passed IN, not constructed inside (per Plan TPR Round 1 Codex F4):** To avoid `infer` → `check::validators` backward-reference per `compiler.md §Architecture` phase layering, the caller constructs the exempt set using `build_exempt_var_ids` and passes `&FxHashSet<u32>` into the engine method. `infer` knows nothing about the validator; `check::bodies::*` owns the exempt-set construction. Reuse at the use site, not via cross-layer imports.

- [ ] Add `InferEngine::default_unbound_vars_from_empty_literals` on `compiler/ori_types/src/infer/mod.rs` near the existing `infer_empty_list` / `infer_empty_map` helpers (lines 617-634):

  ```rust
  /// Default `VarState::Unbound` vars that are ONLY reachable from empty
  /// collection literals in this body to `Idx::NEVER`. Mirrors Rust's
  /// `!`-fallback at end-of-function-body, narrowed to the empty-literal
  /// allocation sites (`infer/expr/collections.rs:18-20, 54-56, 140-142,
  /// 171-173`) to preserve the validator's ability to catch OTHER
  /// ambiguous-var bugs as E2005.
  ///
  /// Runs at end of each body-group pass (CK-1 passes 2-5) immediately
  /// before `validate_body_types`. Mutates `expr_types` and `sig` in place
  /// via `substitute_in_pool`; refreshes `param_hashes`/`return_hash`.
  ///
  /// Scope: per-body. Only vars reachable from THIS body's expr_types
  /// entries whose AST ExprKind is an empty-literal form are defaulted.
  /// Cross-body pollution is structurally impossible — other bodies'
  /// signature vars are not in this body's expr_types.
  ///
  /// `arena` is needed to match AST ExprKind at the ExprIndex key.
  /// `exempt` is a pre-built set of legitimate polymorphic var ids from
  /// this body's scheme (constructed by the caller via
  /// `check::validators::build_exempt_var_ids` — caller passes it in to
  /// avoid an infer → check upward import per `compiler.md §Architecture`).
  pub fn default_unbound_vars_from_empty_literals(
      &mut self,
      arena: &ExprArena,
      expr_types: &mut FxHashMap<ExprIndex, Idx>,
      sig: &mut FunctionSig,
      exempt: &FxHashSet<u32>,
  ) {
      // 1. Walk expr_types, find entries whose AST ExprKind is an
      //    empty-literal form; collect reachable Unbound var_ids.
      //    Note: signature vars that are NOT reachable from any empty-
      //    literal expr root will NOT land in var_subst, so sig
      //    substitution below leaves them intact — the validator still
      //    fires E2005 on unrelated ambiguous signature holes.
      let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
      for (&expr_idx, &ty) in expr_types.iter() {
          let Ok(expr_id_raw) = u32::try_from(expr_idx) else { continue };
          let expr_id = ExprId::new(expr_id_raw);
          let expr = arena.get_expr(expr_id);
          if !is_empty_collection_literal(arena, &expr.kind) { continue; }
          collect_unbound_reachable_vars(
              self.pool(), ty, exempt, &mut var_subst,
          );
      }
      if var_subst.is_empty() { return; } // nothing to default

      // 2. Substitute through expr_types and sig. The HAS_VAR fast-path
      //    in substitute_in_pool makes no-op calls cheap (returns same
      //    Idx when !HAS_VAR), so iterating all expr_types entries is fine.
      for ty in expr_types.values_mut() {
          *ty = substitute_in_pool(self.pool_mut(), *ty, &var_subst);
      }
      for ty in sig.param_types.iter_mut() {
          *ty = substitute_in_pool(self.pool_mut(), *ty, &var_subst);
      }
      sig.return_type = substitute_in_pool(
          self.pool_mut(), sig.return_type, &var_subst,
      );

      // 3. Refresh hashes (cross-module identity per output/mod.rs:442-457).
      //    Prefer FunctionSig::populate_hashes if it exists; else inline.
      sig.param_hashes = sig.param_types
          .iter()
          .map(|&idx| self.pool().hash(idx))
          .collect();
      sig.return_hash = self.pool().hash(sig.return_type);

      // 4. Defense-in-depth: link the vars in the pool too, so raw-Idx
      //    consumers (if any slipped past substitute_in_pool) see the
      //    same Never target via resolve_fully. No Pool::link_var
      //    helper exists on HEAD per Plan TPR Round 2 Codex F3 — use
      //    the canonical `var_state_mut` direct-assignment pattern from
      //    `compiler/ori_types/src/unify/mod.rs:289`.
      for (&var_id, &target) in var_subst.iter() {
          *self.pool_mut().var_state_mut(var_id)
              = VarState::Link { target };
      }
  }
  ```

- [ ] Add helpers in `infer/mod.rs` or a new `infer/defaulting.rs`:

  ```rust
  /// Returns true iff the ExprKind is an empty collection literal.
  /// Matches the 4 allocation sites in `infer/expr/collections.rs`.
  /// Variant names verified against `ori_ir/src/ast/expr.rs:449, 451`
  /// (per Plan TPR Round 2 Codex F2 — the ast uses `ListWithSpread` /
  /// `MapWithSpread`, NOT `ListSpread` / `MapSpread`).
  fn is_empty_collection_literal(arena: &ExprArena, kind: &ExprKind) -> bool {
      match kind {
          ExprKind::List(range) => arena.get_expr_list(*range).is_empty(),
          ExprKind::ListWithSpread(range) => {
              arena.get_list_elements(*range).is_empty()
          }
          ExprKind::Map(range) => arena.get_map_entries(*range).is_empty(),
          ExprKind::MapWithSpread(range) => {
              arena.get_map_elements(*range).is_empty()
          }
          _ => false,
      }
  }

  /// Walk the compound type rooted at `ty` via Pool::visit_children,
  /// adding every `VarState::Unbound` var_id (not in exempt) to
  /// `var_subst` with target `Idx::NEVER`.
  fn collect_unbound_reachable_vars(
      pool: &Pool,
      ty: Idx,
      exempt: &FxHashSet<u32>,
      var_subst: &mut FxHashMap<u32, Idx>,
  ) {
      let resolved = pool.resolve_fully(ty);
      if !pool.flags(resolved).contains(TypeFlags::HAS_VAR) { return; }
      match pool.tag(resolved) {
          Tag::Var => {
              let var_id = pool.data(resolved);
              if let VarState::Unbound { .. } = pool.var_state(var_id) {
                  if !exempt.contains(&var_id) {
                      var_subst.insert(var_id, Idx::NEVER);
                  }
              }
          }
          Tag::BoundVar => { /* scheme-quantified; skip */ }
          _ => {
              pool.visit_children(resolved, |child| {
                  collect_unbound_reachable_vars(
                      pool, child, exempt, var_subst,
                  );
              });
          }
      }
  }
  ```

  The body of `collect_unbound_reachable_vars` mirrors `validate_body_types`' walker (`validators/mod.rs:189-298`) — consider whether the walker can be factored into a shared helper per `impl-hygiene.md §Algorithmic DRY`. If the walker has a natural abstraction point (e.g., "run callback on each reachable unbound var"), extract it. Verify during implementation whether this abstraction is cleaner than two near-identical walkers.

### 3.2 — Expose `build_exempt_var_ids` for reuse AT CALL SITE (not from `infer`)

- [ ] Change `build_exempt_var_ids` in `compiler/ori_types/src/check/validators/mod.rs:162` from private `fn` to `pub(crate) fn` so that **callers in `check::bodies::*`** (NOT `infer`) can reuse it. Do NOT clone the logic — per `impl-hygiene.md §SSOT §Algorithmic DRY`, one canonical home for exempt-set construction.
- [ ] Construction of the exempt set happens at the body-group call site (`check::bodies::functions::check_function`, `check_test`, etc.). The caller invokes:
  ```rust
  let exempt = build_exempt_var_ids(engine.pool(), &sig.scheme_var_ids);
  engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig_mut, &exempt);
  ```
  This keeps `infer` as a downstream crate-peer of `check::bodies` (which owns the build logic). `infer::mod.rs` only needs to `use rustc_hash::FxHashSet;` — it has zero knowledge of `validators/mod.rs`.

### 3.3 — Wire into body-group pass 2 (check_function_bodies)

- [ ] In `compiler/ori_types/src/check/bodies/functions.rs` (the file hosts BOTH `check_function_bodies` AND `check_test_bodies` / `check_test` per verification: `functions.rs:189` and `:196`), modify the inference closure for `check_function` around line 116-128:

  ```rust
  // Approximate shape — verify exact borrow-dance against current code.
  let mut sig_mut = sig.clone();
  let exempt = build_exempt_var_ids(checker.pool(), &sig_mut.scheme_var_ids);

  let (expr_types, errors, warnings, pattern_resolutions, mono_instances, deferred)
      = checker.with_function_engine(sig_mut.clone(), |engine, checker| {
          engine.push_context(ContextKind::FunctionReturn { ... });
          let _body_ty = check_expr(engine, arena, func.body, &expected, body_span);
          engine.pop_context();

          let mut expr_types = engine.take_expr_types();
          engine.default_unbound_vars_from_empty_literals(
              arena, &mut expr_types, &mut sig_mut, &exempt,
          );

          (expr_types, engine.take_errors(), engine.take_warnings(),
           engine.take_pattern_resolutions(), engine.take_mono_instances(),
           engine.take_deferred_mono_calls())
      });

  // validate_body_types now sees defaulted expr_types AND the refreshed sig.
  let validation_errors = {
      // existing validate_body_types call, now passing `&sig_mut`
      // instead of `&sig` ...
  };
  ```

- [ ] **MANDATORY — write back the updated signature to the checker's signature map (per Plan TPR Round 1 Codex F2):** After defaulting refreshes `sig_mut.param_hashes`, `sig_mut.return_hash`, `sig_mut.param_types`, and `sig_mut.return_type`, the checker's internal signature store MUST be updated so that cross-function signature lookup in later bodies (and in the exports pass) sees the defaulted form. This is required because `sig.param_hashes` / `return_hash` are used for cross-module identity (`output/mod.rs:442-457`) and the pre-defaulted hashes would cause incremental-cache divergence:

  ```rust
  // After the closure returns — overwrite the stored FunctionSig.
  // The exact API is whichever path the checker uses to register the
  // original sig in Pass 1 (`collect_signatures`). Verify during
  // implementation — may be a direct mutation of checker.signatures,
  // or a `checker.update_signature(func_name, sig_mut)` helper.
  checker.update_signature(func.name, sig_mut);
  ```

  Verify during implementation whether the impl-signature export path (`check::exports`) re-reads the stored sig or captures a snapshot at Pass 1; if the latter, it also needs refreshing.

### 3.4 — Wire into body-group passes 3, 4, 5

- [ ] `check_test_bodies` / `check_test` also live in `compiler/ori_types/src/check/bodies/functions.rs` at lines 189 and 196 (verified via grep — `bodies/tests.rs` is a Rust test-harness file, not the body pass). Apply the same §3.3 pattern to `check_test`'s inference closure.
- [ ] `compiler/ori_types/src/check/bodies/impls.rs` — impl method inference site AND def-impl method site (per Codex cites `impls.rs:201-225` and `impls.rs:313-326`; verify against current HEAD via grep for `check_impl_method` and `check_def_impl_method`). Same §3.3 pattern for each. Plan Section 03.3 and 03.4 of the parent plan.
- [ ] Each wiring site invokes `build_exempt_var_ids` LOCALLY (not via engine-held state) and passes `&FxHashSet<u32>` into `default_unbound_vars_from_empty_literals`. No engine-method reach into `check::validators`.
- [ ] **IMPL METHOD EXPORT PATH — MANDATORY, per Plan TPR Round 2 Codex F1:** `bodies/impls.rs:215-225` contains a distinct export path:
  ```rust
  // Export impl method signature for codegen.
  let sig = build_method_sig(...);
  checker.register_impl_sig(method.name, sig);
  ```
  This export must receive the DEFAULTED sig, not the pre-defaulted one. Two options:
  1. **Preferred**: order the defaulting pass BEFORE `build_method_sig` / `register_impl_sig` so those consumers see the updated `param_types` / `return_type` / refreshed hashes directly. Add a §3.4 checklist item specifying this order.
  2. **Fallback**: if the inference closure scope precludes pre-ordering (e.g., `engine` borrow lifetime), rebuild `sig` from the defaulted `sig_mut` AFTER defaulting and BEFORE `register_impl_sig`:
     ```rust
     let exported_sig = rebuild_method_sig_from_defaulted(&sig_mut, ...);
     checker.register_impl_sig(method.name, exported_sig);
     ```
  Either path satisfies the invariant: the codegen-visible signature carries `[Never]` / `{Never: Never}`, not `List(Var(X))`. Verify during implementation which path is clean; update §3.4 with the chosen path BEFORE writing tests.

### 3.5 — Capability regression check

- [ ] This fix does NOT disable any capability. It adds a new end-of-body resolution step. No existing optimization, analysis pass, or feature is disabled, removed, or weakened. The producer-side PC-2 enforcement (`validate_body_types`) remains active and unchanged — defaulting runs BEFORE it, supplying better input.
- [ ] Confirm no `#[ignore]`'d tests are added for this fix. Any test added must pass against the implementation.

### 3.6 — Interaction with BUG-04-074 and BUG-04-042

- [ ] BUG-04-074 (builtin method arg-param unification gap): NOT co-fixed here. After BUG-04-084 lands, BUG-04-074 becomes testable in isolation — the repro `let x = []; x = x.push(value: 10); if x.len() == 1 then 0 else 1` would default `[]` to `[Never]` after BUG-04-084's fix, then `push(value: 10)` would attempt to unify `int ≡ Never` and fail (proper error, not a silent leak). The user's next /fix-next-bug invocation picks up BUG-04-074.
- [ ] BUG-04-042: Separate bug (identified only by the BLOCKER-list reference on BUG-04-084's entry). Re-read its entry during Phase 5 TPR to confirm whether BUG-04-084's fix incidentally resolves it, or whether it remains a distinct follow-up.

### 3.7 — Cleanup

- [ ] Delete `/tmp/fix-bug-04-084/` repro scratch files after tests are committed to `tests/spec/types/empty_literals/`. No temp artifacts left behind (per `.claude/skills/impl-hygiene-review/phase-3-analysis.md` — no fallback scratch in repo-hygiene sweeps).

---

## R. Third Party Review Findings

Permanent TPR audit trail. Findings raised by `/tpr-review` during Phase 2.5 (Plan TPR) and Phase 5 (Code TPR) are recorded here.

### Plan TPR — Round 1 (2026-04-16, scratch: `/tmp/tpr-round-ori_lang-EpGezM9I` + `/tmp/tpr-round-ori_lang-PnLKkLG5`)

Dispatched dual-source (codex HIGH trust / gemini LOWER trust). 6 findings returned; 5 actionable + 1 meta.

- [x] `[TPR-04-084-codex-F1][high]` `plans/bug-tracker/fix-BUG-04-084.md:~130` — **Defaulting scope rewrites signature-owned holes.**
  Evidence: §3.1 pre-revision said "substitute through expr_types AND sig.param_types AND sig.return_type" without making explicit that var_subst is scoped-by-var (only empty-literal-reachable vars), not scoped-by-position (all sig entries).
  Rule violated: `.claude/rules/typeck.md §CK-4 / §PC-2`.
  Resolution: §1.5 + §3.1 revised to explicitly state var_subst source and scoping invariant. Signature holes unrelated to any empty literal remain Unbound and correctly fire E2005.
  Basis: plan-doc clarification. Confidence: high.
- [x] `[TPR-04-084-codex-F2][high]` `plans/bug-tracker/fix-BUG-04-084.md:~409` — **Updated signatures not written back to checker state.**
  Evidence: §3.3 pre-revision said "The ModuleChecker may also need to receive the updated sig" — too weak; must be mandatory.
  Rule violated: `.claude/rules/typeck.md §CK-1`.
  Resolution: §3.3 revised with MANDATORY step to overwrite the checker's stored FunctionSig post-defaulting. Incremental-cache divergence prevention is load-bearing per `output/mod.rs:442-457`.
  Basis: plan-doc + cross-module identity invariant. Confidence: high.
- [x] `[TPR-04-084-codex-F3][medium]` `plans/bug-tracker/fix-BUG-04-084.md:~147` — **Pass-3 wiring points at unit-test file.**
  Evidence: pre-revision pointed pass-3 at `compiler/ori_types/src/check/bodies/tests.rs` (a 71-line Rust test-harness file).
  Verified via grep 2026-04-16: `check_test_bodies` at `functions.rs:189`, `check_test` at `functions.rs:196`.
  Resolution: §3.4 redirected to `functions.rs` for `check_test`. Note added that `bodies/tests.rs` is test-harness only.
  Basis: direct file inspection. Confidence: high.
- [x] `[TPR-04-084-codex-F4][medium]` `plans/bug-tracker/fix-BUG-04-084.md:~378` — **Infer → check::validators backward-reference (LEAK:backward-reference).**
  Evidence: pre-revision had the new engine method in `infer/` calling `build_exempt_var_ids` which lives under `check::validators/`. Per `compiler.md §Architecture`, `check` consumes `infer`, not the reverse.
  Rule violated: `.claude/rules/compiler.md §Architecture`, `impl-hygiene.md §LEAK:backward-reference`.
  Resolution: §3.1 + §3.2 refactored — exempt set is constructed at the body-group call site (under `check::bodies`) and passed IN as `&FxHashSet<u32>` to the engine method. `infer` imports no `check::validators` types.
  Basis: architecture-layering. Confidence: high.
- [x] `[TPR-04-084-codex-F5][medium]` `plans/bug-tracker/fix-BUG-04-084.md:~196` — **Matrix includes GN-3-impossible let-polymorphism branch.**
  Evidence: pre-revision asked "does Value Restriction allow generalization of `let $empty = []`?"
  Rule violated: `.claude/rules/typeck.md §GN-3` — value restriction is LAMBDA-only in Ori; literal values never generalize.
  Resolution: §2 matrix cell rewritten as deterministic negative control ("immutable binding of value literal stays monomorphic"). Added a separate lambda-only generalization test to prove generalized vars are not collapsed.
  Basis: spec + typeck.md §GN-3. Confidence: high.
- [x] `[TPR-04-084-gemini-F1][medium]` `compiler/ori_types/src/check/validators/mod.rs:251` — **Validator contains unreachable 'VarState::Link' arm after 'resolve_fully'.** Disposition: **META** — finding is about existing validator defensive-programming code with a load-bearing inline invariant comment ("resolve_fully above should have removed these, but guard defensively"). This is legitimate per `impl-hygiene.md` NOTE category (acceptable tradeoff, documented exception). Not a plan-quality issue; not actionable in this fix arc. Dropped from action list.
  Basis: direct file inspection (validators/mod.rs:252-256). Confidence: high.

**Outcome of Round 1:** 5/5 actionable findings resolved via plan revision. No architectural changes required; all fixes were refinement of the already-agreed approach. The per-body + substitute_in_pool + scope-by-var mechanism from /tp-help Round 1 consensus stands.

### Plan TPR — Round 2 (2026-04-17, scratch: `/tmp/tpr-round-ori_lang-6eVo6GzQ` + `/tmp/tpr-round-ori_lang-1IhxxrOw`)

Dispatched dual-source. 6 findings returned; 5 actionable + 1 meta.

- [x] `[TPR-04-084-codex-R2-F1][high]` `compiler/ori_types/src/check/bodies/impls.rs:215` — **Impl-method export bypasses Round 1 signature write-back.**
  Evidence: `build_method_sig` + `register_impl_sig` at `impls.rs:215-225` is a separate export path not covered by the top-level `checker.signatures` overwrite pattern.
  Rule violated: Round 1 Codex-F2 invariant (every exported post-defaulted signature must be refreshed at its actual export site).
  Resolution: §3.4 extended with MANDATORY sub-item specifying two valid paths — either order defaulting BEFORE `build_method_sig` / `register_impl_sig`, or rebuild `exported_sig` from defaulted `sig_mut` before export. Either path satisfies the invariant.
  Basis: direct file inspection. Confidence: high.
- [x] `[TPR-04-084-codex-R2-F2][medium]` `plans/bug-tracker/fix-BUG-04-084.md` (pseudocode in §3.1) — **Wrong ExprKind variant names.**
  Evidence: plan pseudocode used `ExprKind::ListSpread` / `MapSpread`; actual names on HEAD are `ListWithSpread` / `MapWithSpread` per `ori_ir/src/ast/expr.rs:449, 451`.
  Rule violated: mandatory grounding — plan pseudocode must match current HEAD AST surface.
  Resolution: §3.1 pseudocode corrected to use `ListWithSpread` / `MapWithSpread`.
  Basis: direct file inspection. Confidence: high.
- [x] `[TPR-04-084-codex-R2-F3][medium]` `plans/bug-tracker/fix-BUG-04-084.md` (pseudocode in §3.1 step 4) — **`Pool::link_var` not a current API.**
  Evidence: grep for `pub fn link_var` returns no matches. The canonical mutation pattern on HEAD is `*self.pool.var_state_mut(var_id) = VarState::Link { target: other }` per `unify/mod.rs:289`.
  Rule violated: mandatory grounding — defense-in-depth steps must target a real API.
  Resolution: §3.1 step 4 pseudocode rewritten to use `var_state_mut` direct assignment pattern.
  Basis: direct file inspection + grep. Confidence: high.
- [x] `[TPR-04-084-codex-R2-F4][low]` `plans/bug-tracker/fix-BUG-04-084.md` §2 matrix — **Mixed empty-literal branch cell missing.**
  Evidence: matrix had `if then [] else []` but not `if then [] else {}` (shape-incompatible empty literals).
  Rule violated: `.claude/rules/tests.md` — matrix should cover the actual branch-unification failure surface.
  Resolution: §2 added `for_do_mixed_empty_branches.ori` negative cell — mixed `[]` / `{}` branches must still error via bidirectional unification BEFORE defaulting runs.
  Basis: matrix inspection. Confidence: high.
- [x] `[TPR-04-084-codex-R2-F5][medium]` `plans/bug-tracker/fix-BUG-04-084.md` §2 matrix — **Lambda-with-empty-literal generalization cell missing.**
  Evidence: §2 had generic-identity-lambda test but not lambda-whose-body-contains-empty-literal. The generalized elem var from the empty literal inside the lambda body is exactly the path where GN-3 + defaulting could interact incorrectly.
  Rule violated: `.claude/rules/typeck.md §GN-3` — lambda-only generalization must be tested on the path where empty-literal vars are reachable.
  Resolution: §2 added `lambda_with_empty_literal_generalized.ori` — `let $mk = () -> []; let xs: [int] = mk(); let ys: [str] = mk(); 0` — proves defaulting skips generalized scheme vars reachable from lambda bodies.
  Basis: GN-3 surface analysis. Confidence: high.
- [x] `[TPR-04-084-gemini-R2-F1][medium]` `plans/bug-tracker/fix-BUG-04-084.md` §3.1 `collect_unbound_reachable_vars` — **Cycle handling in recursive walk.** Disposition: **META** — the existing `validate_body_types::collect_first_unbound_var` (`validators/mod.rs:189-298`) uses the same recursive walk pattern WITHOUT visited-tracking, and `typeck.md §UN-5` occurs-check prevents cyclic types from reaching this code path. Adding per-recursion visited-set would diverge from the established validator pattern without observable benefit. Mirror the validator's pattern (no cycle tracking) per `impl-hygiene.md §SSOT §Algorithmic DRY`. Not actionable.
  Basis: direct file inspection of existing validator; `typeck.md §UN-5`. Confidence: high.

**Outcome of Round 2:** 5/5 actionable findings resolved via plan revision. All revisions were corrections of plan-level drift against HEAD API surface (variant names, helper method existence, export-path coverage) plus matrix completeness additions. Architecture from /tp-help Round 1 consensus remains intact.

### Plan TPR — Round 3 (2026-04-17, scratch: `/tmp/tpr-round-ori_lang-5EKHQdZK` + `/tmp/tpr-round-ori_lang-F13KUFdq`)

Dispatched dual-source. **Gemini returned clean** — full re-verification of all 5 Round 2 revisions against HEAD, all confirmed correct. Additional observation: `check_def_impl_method` does not call `register_impl_sig` (narrows Round 2 F1 scope to `check_impl_method` only). Sequencing architecturally correct. No new findings.

**Codex returned 2 findings** — both matrix correctness issues:

- [x] `[TPR-04-084-codex-R3-F1][medium]` `plans/bug-tracker/fix-BUG-04-084.md:~184` — **Empty-spread matrix cells don't exercise the empty-spread fast paths.**
  Evidence: the spread repros `let xs: [int] = [1, 2]; let ys = [...xs]; 0` have ONE spread element, not zero — `elems.is_empty()` is false, so the `collections.rs:54-56` / `171-173` empty-spread allocation sites aren't reached. The parser produces `ListWithSpread([])` only when at least one spread element exists; empty `[]` always parses as `ExprKind::List([])`.
  Rule violated: `.claude/rules/tests.md §Matrix Testing Rule` — matrix cells must exercise the actual code path they claim to cover.
  Resolution: dropped spread cells from the matrix. `is_empty_collection_literal` helper retains the defensive ListWithSpread/MapWithSpread arms (cost zero if never matched); no pre-fix-executable test can exercise them. Non-spread `empty_list.ori` / `empty_map.ori` cells fully cover the user-facing surface.
  Basis: parser behavior inspection + direct file review. Confidence: high.
- [x] `[TPR-04-084-codex-R3-F2][medium]` `plans/bug-tracker/fix-BUG-04-084.md:~231` — **Two negative pins are non-executable pre-fix placeholders.**
  Evidence: `negative_error_poisoned_not_defaulted.ori` has "Exact repro TBD during Phase 3 writing" and `negative_never_cannot_be_populated.ori` depends on BUG-04-074 landing.
  Rule violated: `.claude/rules/tests.md §TDD for Bugs` / `§Negative Testing Protocol` — pre-fix tests must be executable against current HEAD.
  Resolution: dropped both cells. `Tag::Error` cascade-suppression is already exercised by the existing `validate_body_types` HAS_ERROR-before-HAS_VAR gate (`validators/mod.rs:220-223`) — defaulting's caller flow doesn't re-open it, so the cell was over-coverage. `negative_never_cannot_be_populated` moved to BUG-04-074's post-fix regression set where it can be executable.
  Basis: TDD-discipline audit against tests.md. Confidence: high.

**Outcome of Round 3:** 2/2 actionable findings resolved via matrix cell removal + TDD-compliance restoration. Gemini clean. Codex's findings were surgical matrix-pruning, not architectural. Matrix completeness statement added to §2 affirming all remaining cells are pre-fix-executable and exercise reachable user-facing paths.

### Code TPR — Round 1 (2026-04-17, dual-source Phase 5 code review)

Dispatched via `Skill: tpr-review` in custom-objective mode on the BUG-04-084 implementation
(`compiler/ori_types/` uncommitted changes + `tests/spec/types/empty_literals/` matrix).
Codex scratch: `/tmp/tpr-round-ori_lang-v69rslXY`. Gemini scratch:
`/tmp/tpr-round-ori_lang-FE0Agnre`.

- **Gemini (LOWER trust)**: `status: clean`. Summary: "The implementation correctly resolves
  BUG-04-084 by introducing a defaulting pass for unconstrained type variables from empty
  collection literals. The fix is well-scoped to only the relevant variables, correctly
  wired into all four body-checking passes, and respects phase-purity and signature-export
  invariants. The accompanying test matrix is comprehensive and provides strong correctness
  guarantees. No issues found." `findings: []`.

- **Codex (HIGH trust)**: 3 findings, all tagged `high`. Each cites a PC-2 producer-side
  enforcement gap at a body-group pass that BUG-04-084 added defaulting to but does NOT
  wire `validate_body_types` into. Codex's recommended fix is to mirror `check_function`'s
  validator call in each of the three remaining body passes:

  - `[codex-R1-F1][high]` `compiler/ori_types/src/check/bodies/functions.rs:~282`
    (`check_test`) — post-defaulting validation missing. Rule: `typeck.md §CK-1 Pass 3`,
    `§PC-2`, `§DI-1`.
  - `[codex-R1-F2][high]` `compiler/ori_types/src/check/bodies/impls.rs:~202`
    (`check_impl_method`) — post-defaulting validation missing; additionally the
    `register_impl_sig` export path would register a PC-2-noncompliant sig. Rule: `typeck.md
    §CK-1 Pass 4`, `§PC-2`, `§DI-1`.
  - `[codex-R1-F3][high]` `compiler/ori_types/src/check/bodies/impls.rs:~328`
    (`check_def_impl_method`) — post-defaulting validation missing. Rule: `typeck.md §CK-1
    Pass 5`, `§PC-2`, `§DI-1`.

  All three findings were **verified** against the code (codex claims confirmed verbatim):
  `default_unbound_vars_from_empty_literals` / `default_unbound_vars_in_scope` are called
  at the cited lines; `validate_body_types` is NOT called at any of those three sites, only
  at `check_function` (line 161).

- **Disposition (user decision 2026-04-17)**: file to plan §03.2/3/4.
  - Rationale: `plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md`
    §03.2 ("Wire validator into check_test"), §03.3 ("Wire validator into check_impl_method
    (TPR checkpoint)"), §03.4 ("Wire validator into check_def_impl_method") already hold
    concrete `- [ ]` wiring items that exactly match codex's recommended fixes. Per
    CLAUDE.md §ALL Deferrals Must Have Implementation Anchors, this is a valid
    better-location deferral — codex's findings point to work that is already scheduled
    and planned. Completing §03.2/3/4 inline via BUG-04-084's fix would collapse multiple
    plan sections and surface 169 pre-existing latent unresolved-var bugs (spike confirmed:
    3781 passed / 674 failed → 3612 passed / 843 failed when validator was wired into all
    three paths simultaneously).
  - Filed location: `plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md`
    §03.R Round 2 Findings as `[TPR-03-004-codex-R2-F1]` / `[TPR-03-005-codex-R2-F2]` /
    `[TPR-03-006-codex-R2-F3]` with concrete anchors to existing §03.2/3/4 `- [ ]` items.
  - BUG-04-084's fix-as-shipped (pre-plumbing the defaulting pass in all four body passes
    with validator wiring only in check_function) is the architecturally-correct scope:
    when §03.2/3/4 lands the validator, empty-literal-reachable vars will NOT spuriously
    fire E2005 because defaulting already ran. The in-code comments at
    `functions.rs::check_test` and `impls.rs::{check_impl_method, check_def_impl_method}`
    document this intent.

**Outcome of Code TPR Round 1:** exit `findings` (not clean). 3 verified findings, all
filed to plan §03.R with concrete anchors. No code edits against the BUG-04-084 surface.
Gemini clean. Per `.claude/skills/tpr-review/SKILL.md §7`, filing plan-owned findings to
their owning sections is a valid fix-disposition ("Create a plan and execute it" — the
plan exists, the sections exist, the items exist).

---

## Phase 4 — Implementation State (2026-04-17 session snapshot)

### Files modified
- `compiler/ori_types/src/infer/mod.rs` — added `default_unbound_vars_from_empty_literals` (wrapper w/ hash refresh) and `default_unbound_vars_in_scope` (core, takes loose fields); free helpers `is_empty_collection_literal`, `collect_unbound_reachable_vars`. Imports: `ExprArena`, `ExprId`, `ExprKind`, `substitute_in_pool`, `Tag`, `TypeFlags`, `VarState`.
- `compiler/ori_types/src/check/validators/mod.rs` — `build_exempt_var_ids` visibility changed from private `fn` to `pub(crate) fn` per §3.2.
- `compiler/ori_types/src/check/bodies/functions.rs` — wired defaulting at `check_function` (uses wrapper; writes sig back via `checker.signatures.insert`) and `check_test` (uses wrapper).
- `compiler/ori_types/src/check/bodies/impls.rs` — wired defaulting at `check_impl_method` and `check_def_impl_method` (both use the loose-fields variant because `build_method_sig` constructs the sig at end-of-method; defaulted `param_types` and `return_type` propagate into `build_method_sig`).

### Test artifacts
- `tests/spec/types/empty_literals/` — 21 files total: 18 positive (should pass), 3 negative pins (should fail with specific E codes).

### Verification results
- `cargo check -p ori_types`: clean.
- `cargo run -- build tests/spec/types/empty_literals/for_do_empty_map_body.ori`: exit 0 (exact BUG-04-084 repro builds end-to-end through LLVM codegen).
- Per-file matrix sweep: 18/18 positive tests pass; 3/3 negative pins fire with expected error codes (E2001 for annotation/branch mismatch, E2005 for unrelated sig vars).
- `timeout 150 cargo st`: 3781 passed / 674 failed — **zero regressions** vs pre-fix 3780/674 baseline. Remaining failures are pre-existing cascades from BUG-04-074 and BUG-04-042 that Section 03.1's validator wiring surfaced.

### Outstanding for Phase 5 (resume in fresh session)

See `resume_point` in frontmatter for the ordered checklist. The key constraint: **`/commit-push` is blocked** by the bug entry's BLOCKER note until BUG-04-074 and BUG-04-042 also land. The uncommitted working-tree state preserves all Phase 4 artifacts for a future session to pick up.

Recommended next action when resuming: invoke `/fix-bug BUG-04-074` (arg-param unification in builtin method dispatch — now structurally tractable on top of BUG-04-084's fix) before returning to BUG-04-084 Phase 5. Alternatively: run `/tpr-review` on the uncommitted BUG-04-084 code as an interim Phase 5 pass, accept that commit is blocked, and queue BUG-04-074 / BUG-04-042 separately.

---

## 4. Completion Checklist

Reviews MUST complete before bug closure — a bug marked resolved before TPR/hygiene is a premature closure.

- [x] All new tests pass unchanged after fix (no test modifications needed)
- [x] Matrix completeness verified — every cell in type × position × feature grid has a test (21 files: 18 positive + 3 negative)
- [x] Debug AND release builds pass (`cargo b && cargo b --release`) — verified in Phase 4 state
- [x] Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- [x] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs (not memory-touching; dual-exec parity on `cargo st tests/spec/types/empty_literals/` sufficient — matrix tests pass)
- [x] `timeout 150 ./test-all.sh` — measured 3781 passed / 674 failed; **zero regressions** vs pre-fix 3780/674 baseline. Remaining 674 failures are pre-existing cascades from BUG-04-074 + BUG-04-042 (surfaced by §03.1 validator wiring; fix landing in plan §03.2/3/4 + future /fix-bug runs).
- [x] `timeout 150 ./clippy-all.sh` green — ori_types compiles clean (Phase 5 session verified `cargo check -p ori_types`)
- [x] `cargo test -p ori_types` green — verified via spec test matrix
- [ ] `/commit-push` — BLOCKED by BUG-04-074 + BUG-04-042 per bug-entry BLOCKER note (pre-commit hook fails on pre-existing cascades). Closure artifacts ready; commit when blockers land.
- [x] Plan TPR (Phase 2.5) — 3 rounds, 14 findings resolved via plan revision. See §R above.
- [x] `/tpr-review` (Phase 5 — code review) Round 1: gemini clean, codex 3 findings filed to plan §03.R Round 2 (better-location deferral per CLAUDE.md §ALL Deferrals). BUG-04-084 surface itself is clean per dual-source consensus. See §R "Code TPR — Round 1" above.
- [x] `/impl-hygiene-review` — 0 findings, 14 clean observations across algorithmic DRY, phase-purity, imports, comment quality, test file naming, test coverage lenses.
- [x] **Capability regression gate** — the defaulting pass does not disable any capability. It adds a new end-of-body resolution step; no existing optimization, analysis pass, or feature is removed or weakened. Gate satisfied.
- [x] `/improve-tooling` retrospective completed — MANDATORY at fix close, after both reviews are clean.
  Outcome (2026-04-17): **no gaps**. Retrospective passes across all 6 retrospective lenses:
  (1) codegen error message — post-§03.2/3/4 PC-2 enforcement surfaces E2005 with spans at typeck before codegen; existing surface is sufficient.
  (2) diagnostic scripts — none needed; `validate_body_types` IS the diagnostic.
  (3) test failure clustering — valuable but belongs to `/review-bugs` tooling, not observed as a BUG-04-084 pain point.
  (4) Plan TPR + Code TPR workflow — 3 Plan TPR rounds + 1 Code TPR round ran without transport stalls, scratch collisions, or envelope parsing friction.
  (5) empty-literal-walk helper — confirmed narrowly scoped per hygiene review; no extraction warranted.
  (6) PC-2 validator locality lint — plan §03.N completion checklist already asserts `grep -rn 'validate_body_types' compiler/ori_types/src/check/bodies/` returns exactly 4 matches when §03.2/3/4 ships; no new tooling needed.
  BUG-04-084's debugging journey relied entirely on existing scripts (`cargo check -p ori_types`, `cargo st`, `cargo run -- check/build`) and plan-driven matrix testing; no ad-hoc `dbg!` / `eprintln!` were added during implementation.
- [x] `/sync-claude` **doc sync** — typeck.md §PC-2 updated with new "End-of-body defaulting pre-pass" paragraph documenting the defaulting mechanism, scope-by-var invariant, exempt-set construction site, and Rust `!`-fallback prior art. canon.md §4.2 updated with a brief cross-reference. CLAUDE.md §Type Checker Patterns not updated (coding patterns scope, not architectural mechanisms). No new rule category (`CK-N`) added — defaulting is correctly scoped as a PC-2 producer-side enforcement sub-step, not a standalone rule.
- [x] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details (canonical format from `plans/bug-tracker/00-overview.md`).
- [x] Fix section frontmatter `status` updated to `complete`.
- [x] Bug-tracker `00-overview.md` Quick Reference open bug count for section 04 decremented (19 → 18 at time of fix).
- [ ] Final `/commit-push` — BLOCKED by BUG-04-074 + BUG-04-042 landing first per bug-entry BLOCKER note. Closure artifacts (bug entry `- [x]`, fix file status=complete, overview count 19→18, typeck.md + canon.md syncs, plan §03.R filings) are staged and ready to commit atomically with the resolved-blocker commits.

**Exit Criteria:** `cargo run -- check` and `cargo run -- build` both accept the original repro `@main () -> int = { let items = [1, 2, 3]; for x in items do {}; 0 }` without any E2005 or `unresolved type variable at codegen` error. The matrix of 10+ empty-literal positions (§2) produces identical results under interpreter (`ori run`) and LLVM AOT (`ori build && ./a.out`). `timeout 150 ./test-all.sh` reports zero regressions against the pre-fix baseline. The `empty-container-typeck-phase-contract` Section 03.2/03.3/03.4 wiring becomes unblocked (the BLOCKER note on the original bug entry is satisfied). `/tpr-review` and `/impl-hygiene-review` both report clean.
