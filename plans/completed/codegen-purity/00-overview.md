---
plan: "codegen-purity"
title: "Codegen Purity: Hand-Written Assembly Quality at -O0"
status: complete
reviewed: false
supersedes: []
references:
  - "plans/code-journeys/overview.md"
  - "plans/code-journeys/journey1-results.md through journey12-results.md"
  - "plans/llvm-codegen-fixes/section-11-verification.md"
  - "docs/compiler/design/10-llvm-backend/codegen-verification.md"
---

# Codegen Purity: Hand-Written Assembly Quality at -O0

## Mission

Achieve hand-written-quality C-level assembly from Ori programs at `-O0` by eliminating every known codegen purity deficiency across code journeys 1–12. Functional correctness is already 12/12; this plan targets structural IR quality, attribute precision, and backend parity so debug-mode output does not depend on `-O1+` cleanup passes.

## Scope Boundaries

**In scope**
- Make emitted LLVM IR cleaner and more explicit at `-O0` (CFG shape, attributes, payload extraction, loop quality, constants).
- Fix remaining semantic correctness gaps that surfaced in journey review (`-INT_MIN` unary negation overflow, closure lifetime leak).
- Preserve eval/AOT behavioral parity while improving AOT code generation quality.

**Out of scope**
- Rewriting ARC IR architecture end-to-end.
- Reliance on LLVM optimization passes as the "fix"; this plan is emission-quality first.
- Runtime/COW subsystem redesign unrelated to findings listed in this plan.
- Resolving pre-existing non-codegen benchmark/runtime mismatches that do not map to findings M-1..L-12.

## Purity Contract (Definition of Done)

A finding is considered resolved only when all of the following are true:

1. **Emission-level fix exists.** The issue is eliminated in compiler-generated `-O0` IR/asm before LLVM cleanup passes.
2. **Behavior stays correct.** Eval and AOT outputs remain aligned on affected tests and journeys.
3. **Verifier-clean output.** Generated IR remains valid (`opt-21 -passes=verify` clean on affected fixtures).
4. **Permanent regression test added or updated.** At least one durable test guards the exact failure mode.
5. **Artifact evidence captured.** Before/after IR (and asm when relevant) is archived under `build/codegen-purity/`.

If a case must be deferred, it requires an explicit note in `section-10-verification.md` with ID, rationale, and owner section.

## Execution Guardrails

- Land changes section-by-section; avoid cross-section mega-patches that hide regressions.
- Reproduce the finding before coding the fix (journey fixture or dedicated test).
- Keep fixes local to the owning subsystem unless dependency notes require coordinated changes.
- Do not "fix tests" by weakening assertions; update assertions only when they encode incorrect assumptions.
- Keep verification artifacts deterministic: same fixture, same command, same target triple, same optimization level (`-O0`).

**TDD mandate (from CLAUDE.md):** Every section MUST follow TDD:
1. Write tests capturing the current (broken/suboptimal) behavior FIRST
2. Verify tests detect the issue (fail if checking for the fix, or pass showing the current state)
3. Implement the fix
4. Tests pass unchanged — if tests need changing, the tests were wrong or the fix is wrong

No section may begin implementation before its test infrastructure is in place. See individual section files for specific TDD instructions.

## Plan Quality Bar

This plan is only acceptable if it remains strong on all five axes:

- **Thoroughness:** every finding ID has an owner section, concrete implementation steps, and explicit verification tasks.
- **Accuracy:** file paths, symbols, and commands are validated against the current repository state; avoid brittle line-number coupling.
- **Completeness:** each section has clear exit criteria, regression coverage, and a definition of what is out of scope.
- **Exhaustiveness:** no silent deferrals; unresolved items must be listed in `§10.8` with rationale and follow-up.
- **Codegen purity:** all claims are proven on compiler-emitted `-O0` IR/asm before LLVM optimization cleanup.

## Architecture

```
Source (.ori)
    │
    ▼
┌─────────────────────────────────────────────┐
│  Compiler Frontend (parse → typeck → ARC)   │
│                                              │
│  ┌─────────────────────────────────────┐    │
│  │ ori_arc pipeline (run_arc_pipeline)  │    │
│  │  §01 block merging                  │    │
│  │  §04 closure lifecycle              │    │
│  │  §09 tail call loop lowering        │    │
│  └─────────────────────────────────────┘    │
└─────────────┬───────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│           LLVM IR Emission                   │
│                                              │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ arc_emitter  │  │ function_compiler    │  │
│  │  §03 arith   │  │  §02 attributes     │  │
│  │  §05 payload │  │                      │  │
│  │  §06 dead    │  │                      │  │
│  │  §08 loops   │  │                      │  │
│  └─────────────┘  └──────────────────────┘  │
│                                              │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ ir_builder   │  │ drop_gen (ARC)       │  │
│  │  §07 consts  │  │  §04 closure leak   │  │
│  └─────────────┘  └──────────────────────┘  │
│                                              │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ derive_cg    │  │ runtime_decl        │  │
│  │  §02 attrs   │  │  §02 attrs          │  │
│  └─────────────┘  └──────────────────────┘  │
└─────────────┬───────────────────────────────┘
              │
              ▼
         LLVM IR (pure)
              │
              ▼
         Machine Code (-O0 = hand-written quality)
```

## Design Principles

**1. Every instruction earns its place.** If a basic block contains only `br label %next`, it should not exist. If a phi node has one predecessor, it should be an SSA value. If a struct load touches 4 fields but only 2 are used, only 2 should be loaded. The IR should read like a competent C programmer wrote it — no redundancy, no waste.

*Motivation:* Code journeys 1–12 revealed 19 distinct codegen deficiencies. While LLVM's optimizer handles most at `-O1+`, the debug build (`-O0`) is the primary development experience. Redundant blocks inflate IR size, confuse IR-level debugging, and slow JIT execution.

**2. Correct before clean.** Arithmetic overflow on unary negation (M-5) and closure environment leaks (M-3) are correctness bugs. These take priority over cosmetic improvements. A wrong program that looks clean is worse than a correct program with redundant branches.

**3. Attributes are contracts.** `noreturn`, `nounwind`, and `noundef` are not hints — they are guarantees that enable LLVM to reason about control flow, exception handling, and value validity. Missing attributes leave optimization opportunities on the table even at higher optimization levels.

## Section Dependency Graph

```
§03 Arithmetic Correctness  ───────────────────────────────┐
§07 Constant Deduplication ────────────────────────────────┤
§05 Payload Extraction  ───────────────────────────────────┤
§01 Block Merging ───────────┐                             │
                             ├─ §08 Loop IR Quality ───────┤
§02 Function Attributes ──┐  │                             ├─ §10 Verification
                          └─ §06 Dead Code Pruning ────────┤
§04 ARC Closure Lifecycle ────────┐                        │
                                  └─ §09 Tail Calls ───────┘
```

- ~~`§03`, `§05`, and `§07` are mostly independent and can start immediately.~~ §03 and §05 complete; §07 can start immediately.
- ~~`§02` should land before `§06` so noreturn/nounwind metadata is available for dead-path pruning.~~ Both complete.
- `§04` and `§09` should be coordinated because tail-call eligibility depends on ARC cleanup placement. §04 complete; §09 pending.
- `§08` benefits from `§01` block param simplification first. §01 complete; §08 can start.
- `§10` requires sections 01–09 to be complete. Sections 01–06 complete; 07–09 pending.

**Roadmap integration:** Codegen-purity is a vertical quality pass across roadmap sections 21A (LLVM Backend) and 21B (AOT Compilation). It is tracked separately but contributes to both completion checkpoints.

**Cross-section interactions (must be coordinated)**
- **`§02` + `§06`**: `noreturn` on panic declarations is required for reliable "emit `unreachable` and stop" behavior. (Both complete.)
- **`§04` + `§09`**: Tail-call lowering must not regress closure/environment lifetime correctness. (§04 complete; §09 pending — coordination still required.) Additionally, the `__recurse` sentinel from `recurse()` pattern lowering (`ori_arc/src/lower/constructs.rs:171`) must be resolved to the actual function name before or during §09's detection pass. This is a prerequisite for both TCO detection and correct AOT compilation of `recurse()` programs.
- **`§09` + `block_merge`**: §09's loop lowering pass runs BEFORE `block_merge` in `run_arc_pipeline()`. Block merge Phase 1 (compact) removes dead merge blocks created by the rewrite. Phase 4 does NOT merge loop headers (multi-predecessor). No conflict.
- **`§01` + `§08`**: Block param cleanup and block-merging touch the same CFG construction path. (§01 complete; §08 pending.) Block merging (§01) merges sequential blocks within loop bodies, which can make duplicate computations co-resident in a single block — this HELPS 08.1 CSE (more opportunities). Block merging does NOT affect loop headers (multi-predecessor blocks are never merged). §08.2's invariant-param elimination extends Phase 6 (dead params) in `block_merge/` — same infrastructure, compatible.
- **`§08` + `§09`**: TCO-generated loops have the same structure as source-level loops (header + body + back-edge). §08's optimizations (CSE, invariant-param elimination, range specialization) apply to TCO-generated loops automatically via `block_merge`. Implement §08 before §09 so the loop optimization infrastructure is tested on simpler loops first.
- **`§06` + `§08`**: Dead code after break/continue (§06) and loop body optimizations (§08) are independent. §06.2's noreturn pruning handles `panic()` inside loop bodies. §06.1's surgical loading operates on function params, not loop iteration variables. No conflict.
- **`§07` + `§03`**: §07 refactors `emit_checked_binop()` in `checked_ops.rs` (extracted from `arithmetic.rs`) to use the IrBuilder wrapper for string dedup. §03 is complete, so verify §03's overflow paths still work after §07's refactoring.
- **`§02` + `§02.3`**: Derived methods bypass the two-pass nounwind pipeline (compiled before `compute_nounwind_set`). Any future pipeline refactoring must account for this ordering.

## Implementation Sequence

```
Phase 0 - Baseline & Instrumentation
  └─ Capture baseline IR/disassembly/audit for journeys 1-12
  └─ Record baseline finding counts by ID (table below)
  Gate: Baseline artifacts committed under build/codegen-purity/baseline/; regression comparisons possible

Phase 1 - Correctness (fix wrong behavior first) ✓ COMPLETE
  └─ §03: Unary negation overflow check ✓
  └─ §04: Closure environment RC leak ✓
  Gate: `-INT_MIN` parity tests pass; closure leak checks and RC accounting pass

Phase 2 - IR Structure (eliminate redundant IR constructs) ✓ COMPLETE
  └─ §01: Block merging, select lowering, phi simplification ✓
  └─ §05: extractvalue for sum type payloads ✓
  Gate: No avoidable bridge blocks in target functions; no SSA-to-stack roundtrip for targeted payload extraction paths

Phase 3 - Attributes, Constants, and Dead-Path Pruning (§07 remaining)
  └─ §02: noreturn, nounwind, noundef on all applicable functions ✓
  └─ §06: Dead field loads, dead code after noreturn (§06.2 requires §02.1 first) ✓
  └─ §07: Overflow message string deduplication (prerequisite split of arithmetic.rs already done — now 352 lines + checked_ops.rs 162 lines)
  Gate: Attribute assertions pass; 0 duplicate targeted overflow strings; no post-noreturn emission in audited paths

Phase 4 - Loop Quality (optimize loop IR patterns)
  └─ §08: CSE for compound assignment, loop-invariant block param elimination, range specialization
  │   Recommended sub-order: 08.3 (range specialization, lowest risk, highest impact on J7)
  │                         → 08.2 (invariant block params, extends existing block_merge infra)
  │                         → 08.1 (CSE, highest complexity, may be deferred if 08.3 makes it moot)
  └─ §09: Tail call optimization via loop lowering (musttail as fallback)
  Gate: Journey loop findings resolved; tail-recursive stress test is stack-safe

Phase 5 - Verification
  └─ §10: Re-run all 12 code journeys, verify hand-written assembly quality
  Gate: All required checks in §10 pass; all finding IDs M-1..L-12 closed or explicitly deferred with rationale
```

**Why this order:**
- Phase 1 fixes semantic bugs — wrong behavior must be fixed before optimizing IR shape.
- Phase 2 tackles the most visible IR bloat (redundant blocks and payload extraction issues appear across the majority of journeys).
- Phase 3 adds metadata and constant identity guarantees that unblock cleaner emission.
- Phase 4 handles loop and recursion-specific patterns after CFG/attribute cleanup is stable.
- Phase 5 is the final gate — nothing ships without re-verification.

**Crate dependency ordering:** Within each phase, changes to `ori_arc` (upstream) MUST land before changes to `ori_llvm` (downstream). `ori_arc` has NO LLVM dependency. Specifically:
- ~~§01 (block merging): `ori_arc/src/block_merge/` first → `ori_llvm` emission changes second~~ **Complete**
- §07 (constant dedup): `arithmetic.rs` already split (352 lines + `checked_ops.rs` 162 lines) — no prerequisite work needed.
- §08 (loop IR quality): **08.2** (invariant block params): `ori_arc/src/block_merge/` (extend Phase 6 or add Phase 7) — ARC-only, no `emit_function.rs` changes. **08.3** (range specialization): `ori_arc/src/lower/control_flow/for_loops/for_range.rs` + new `get_literal_int()` on `ArcIrBuilder` — ARC-only. **08.1** (CSE): cache in `IrBuilder` (`ori_llvm/src/codegen/ir_builder/checked_ops.rs`), NOT in `ArcIrEmitter`. **BLOAT:** `emit_function.rs` is 579 lines (limit 500) — must split before any modifications (extract `scan_used_fields()` into `field_scan.rs`). Recommended order: 08.3 → 08.2 → 08.1 (simplest first, most impactful first, highest risk last).
- §09 (tail call): `ori_arc/src/` detection/rewriting first → `ori_llvm` emission second. These are coordinated but `ori_arc` changes are independent of `ori_llvm`. **Pipeline placement:** New pass in `run_arc_pipeline()` AFTER `rc_identity` + `rc_elim` and BEFORE `block_merge` — the rewrite needs RC ops in final positions (after identity normalization and dead-pair elimination), and block_merge cleans up dead blocks from the rewrite. **Prerequisite:** `__recurse` sentinel resolution — add `func_name: Name` field to `ArcLowerer` (Option 1, preferred) or resolve as first step of detection pass (Option 2). **New module:** `compiler/ori_arc/src/tail_call/` (detection + rewrite + sibling `tests.rs`). Must follow module hygiene: `//!` docs, `pub(crate)` visibility, `#[tracing::instrument]`, `///` doc comments, 500-line limit per file. **`lib.rs` exception:** `ori_arc/src/lib.rs` already contains pipeline function bodies (382 lines) — adding a single call is acceptable, but if it exceeds ~450 lines, extract pipeline into `pipeline.rs`.

## Known Failing Tests (Expected Until Completion)

These are expected to fail until their owning section lands; do not patch around them ad hoc.

- ~~Unary negation overflow parity tests (`-INT_MIN`) until `§03` is complete.~~ **Resolved** — §03 complete.
- ~~Leak-check closure lifecycle tests until `§04` is complete.~~ **Resolved** — §04 complete.
- ~~IR-quality assertions for select lowering/payload extraction until `§01`, `§05` are complete.~~ **Resolved** — §01, §05 complete.
- IR-quality assertions for loop simplification until `§08` is complete.

**Pre-existing `#[ignore]` tests in `compiler/ori_llvm/tests/aot/ir_quality.rs`:**
These 4 tests document the exact issues this plan targets. **ACTION NEEDED**: Their owning sections (§01, §02, §06) are now complete. These tests should be un-ignored and verified:
- `test_nounwind_program_has_no_unreachable_blocks` → §02 + §06 (nounwind + dead block pruning) — **both complete**
- `test_nounwind_generic_call_no_unreachable` → §02 + §06 — **both complete**
- `test_mixed_calls_no_dead_unreachable` → §02 + §06 — **both complete**
- `test_constant_main_minimal_ir` → §01 + §02 (block merging + attributes) — **both complete**

**Journeys 13–19:** Previously existed but were removed. Only journeys 1–12 are active verification targets.

## Findings Summary (from Code Journeys 1–12)

| ID | Severity | Category | Description | Baseline Journeys |
|----|----------|----------|-------------|-------------------|
| M-1 | MEDIUM | CFG | Redundant unconditional branches at let-binding boundaries | J1, J2, J5, J6, J7, J8, J12 |
| M-1b | MEDIUM | CFG | Trivial if/else emits diamond instead of `select` | J2 |
| M-1c | MEDIUM | CFG | Break bridge blocks with dead phi values | J7 |
| M-2 | MEDIUM | Attrs | `ori_panic_cstr` missing `noreturn` | J1, J5 |
| M-3 | MEDIUM | ARC | Missing `ori_rc_dec` on closure environment | J5 |
| M-4 | MEDIUM | Payload | alloca+store+GEP+load for sum type payload extraction | J6, J11 |
| M-5 | MEDIUM | Arith | Unary negation lacks overflow check for INT_MIN | J2 |
| L-1 | LOW | Attrs | C `main()` wrapper missing `nounwind` | J1, J5, J6 |
| L-2 | LOW | Attrs | Derived `$eq` methods missing `nounwind` | J11 |
| L-3 | LOW | Attrs | `ori_str_from_raw` declaration missing `nounwind` | J9 |
| L-4 | LOW | Consts | Identical overflow message constants not merged | J2, J3, J4, J6, J7, J8, J9, J10, J11, J12 |
| L-5 | LOW | Dead | All struct/list fields loaded when only some needed | J4, J10 |
| L-6 | LOW | Loop | `i+1` computed twice per loop iteration | J7 |
| L-7 | LOW | Dead | Cleanup code after noreturn `ori_panic` | J7 |
| L-8 | LOW | Loop | Unchanging value carried through loop block param (invariant phi in LLVM IR) | J10 |
| L-9 | LOW | Loop | 8-instruction bounds check for common `1..=n` case | J7 |
| L-10 | LOW | TCO | Tail-recursive gcd not optimized to loop | J3 |
| L-11 | LOW | Attrs | Missing `noundef` on i64 parameters | J1 |
| L-12 | LOW | Attrs | Indirect closure calls conservatively lack `nounwind` | J5 |

## Issue Traceability

| Finding | Owner Section(s) | Primary Verification |
|---------|-------------------|----------------------|
| M-1, M-1b, M-1c | §01 | Journey IR dumps + section tests |
| M-2, L-1, L-2, L-3, L-11, L-12 | §02 | Function signature/attr assertions in LLVM tests |
| M-5 | §03 | Spec + AOT operator tests (`-INT_MIN`) |
| M-3 | §04 | `ORI_CHECK_LEAKS=1`, RC stats, valgrind |
| M-4 | §05 | J6/J11 IR instruction-shape assertions |
| L-5, L-7 | §06 | IR assertions: no unused field loads, no post-panic emission |
| L-4 | §07 | IR global string uniqueness assertions |
| L-6, L-8, L-9 | §08 | Loop IR pattern assertions + execution parity |
| L-10 | §09 | Tail recursion stress test + IR check (loop lowering; `musttail` fallback) |

## Metrics (Current State)

| Metric | Baseline | Target |
|--------|----------|--------|
| Journey functional correctness (eval vs AOT) | 12/12 pass | 12/12 pass (no regressions) |
| Medium-severity purity findings | ~~7~~ 0 (all owning sections §01-§05 complete) | 0 |
| Low-severity purity findings | 12 → 5 remaining: L-4 (§07), L-6/L-8/L-9 (§08), L-10 (§09) | 0 unresolved |
| Findings with permanent regression tests | Needs audit — §01-§06 complete, verify tests exist | 19/19 |
| Journey artifacts captured (IR/asm/audit) | 0/12 | 12/12 |
| Sections complete | 10/10 | 10/10 |

## Estimated Effort

| Section | Est. New/Changed LOC | Complexity | Depends On |
|---------|----------------------|------------|------------|
| 01 Block Merging & CFG | ~250-450 | High | — |
| 02 Function Attributes | ~120-220 | Medium | — |
| 03 Arithmetic Correctness | ~40-90 | Low | — |
| 04 ARC Closure Lifecycle | ~180-320 | High | — |
| 05 Payload Extraction | ~90-170 | Medium | — |
| 06 Dead Code Pruning | ~120-220 | **High** (06.1 is ABI-level) | 02 |
| 07 Constant Deduplication | ~50-120 | Low | — |
| 08 Loop IR Quality | ~220-380 | **Very High** (08.1 CSE cache + 08.2 invariant block params + 08.3 range specialization + `emit_function.rs` split prerequisite) | 01 |
| 09 Tail Call Optimization | ~400-600 | **Very High** (ARC interaction is research-grade; `block_merge` — a comparable CFG transform — is ~600 lines excl. tests; budget 2-3x) | 04 (coordination) |
| 10 Verification | ~120-200 (tests/docs) | Medium | 01-09 |

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Unsound `nounwind`/`noreturn` annotation | Miscompilation/UB | Keep conservative defaults, add negative tests, verify with dual-exec + LLVM verifier |
| ARC lifecycle changes regress memory safety | Leaks or over-release | Leak checks + RC stats + valgrind in section and final gates |
| Tail-call work conflicts with ARC cleanup | Incorrect drops or no TCO | Treat `§04` and `§09` as coordinated landing; require dedicated recursion tests |
| Over-specialized loop paths | Behavioral regressions on uncommon ranges | Keep general fallback path and add property-style tests for range variants |
| Verification drift over time | Plan appears complete but gaps reappear | Section 10 requires artifact regeneration and explicit unresolved-ID accounting before close |
| ~~§07 pushes `arithmetic.rs` over 500 lines~~ | ~~BLOAT finding~~ | ~~Split already done~~: `arithmetic.rs` (352 lines) + `checked_ops.rs` (162 lines) |
| §08 requires modifying `emit_function.rs` (579 lines) | BLOAT finding | Must split before modifications: extract `scan_used_fields()` (~190 lines) into `field_scan.rs`. Note: 08.2 does NOT modify this file (fix is in ori_arc); only 08.1 may need it if cache-clearing hooks are added. |
| §06.1 changes struct param loading ABI | Could break all struct-passing codegen | Requires extensive AOT test coverage; implement behind feature flag initially |
| §08.1 CSE invalidation correctness | Stale cache entries cause miscompilation | Cache in `IrBuilder` (not `ArcIrEmitter`). Clear via explicit `clear_cse_cache()` at ARC block boundaries — do NOT clear on internal `position_at_end` calls within `emit_checked_binop` (which creates panic/continue blocks). Key on LLVM `ValueId`, only cache checked arithmetic intrinsics; add negative tests for cross-block, side-effect, and nested-loop scenarios |
| §09 ARC+TCO interaction | Hoisting RcDec before tail call may cause use-after-free | Safety proof required per-variable; conservative fallback (no TCO) if proof fails |
| §09 `__recurse` sentinel unresolved in AOT | `recurse()` programs fail in AOT compilation (unresolved function) | Resolve sentinel to actual function name at lowering time or as first step of TCO detection pass — prerequisite for §09, fixes latent AOT bug |
| §09 cross-block tail call detection | Existing `check_tail_call()` only checks same-block Apply→Return, misses the actual Apply→Jump→Return pattern | New detection pass must trace through Jump terminators; do not extend existing `check_tail_call()` (different purpose/phase) |

## Sign-Off Requirements

Before marking this plan `complete`, all of these must be true:

- Section frontmatter statuses are updated (01–10).
- `section-10-verification.md` contains final command outputs summary and unresolved-ID table (empty or justified).
- `section-10-verification.md` includes a completed finding-closure matrix for every ID M-1..L-12 with artifact links.
- `plans/codegen-purity/index.md` status labels reflect actual section states.
- Build artifacts exist for final verification run under `build/codegen-purity/`.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Block Merging & CFG Simplification | `section-01-block-merging.md` | Complete |
| 02 | Function Attributes | `section-02-function-attributes.md` | Complete |
| 03 | Arithmetic Correctness | `section-03-arithmetic-correctness.md` | Complete |
| 04 | ARC Closure Lifecycle | `section-04-arc-closure-lifecycle.md` | Complete |
| 05 | Sum Type Payload Extraction | `section-05-payload-extraction.md` | Complete |
| 06 | Dead Code Pruning | `section-06-dead-code-pruning.md` | Complete |
| 07 | Constant Deduplication | `section-07-constant-dedup.md` | Complete |
| 08 | Loop IR Quality | `section-08-loop-ir-quality.md` | Complete |
| 09 | Tail Call Optimization | `section-09-tail-call.md` | Complete |
| 10 | Verification | `section-10-verification.md` | Complete |
