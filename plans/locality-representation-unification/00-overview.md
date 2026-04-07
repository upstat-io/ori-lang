---
plan: "locality-representation-unification"
title: "Locality Representation Unification: Exhaustive Implementation Plan"
status: not-started
references:
  - "plans/repr-opt/section-08-escape-analysis.md"
  - "plans/repr-opt/section-09-arc-header.md"
  - "plans/repr-opt/section-10-thread-local-arc.md"
  - "plans/repr-opt/00-overview.md"
  - "compiler/ori_arc/src/aims/lattice/dimensions.rs"
  - "compiler/ori_arc/src/aims/lattice/mod.rs"
  - "compiler/ori_arc/src/aims/contract/mod.rs"
  - "compiler/ori_arc/src/aims/interprocedural/extract.rs" # 517 lines, pre-split in Section 00.4b
  - "compiler/ori_arc/src/aims/intraprocedural/state_map.rs" # 646 lines, pre-split in Section 00.4c
  - "compiler/ori_arc/src/aims/interprocedural/mod.rs" # 536 lines, pre-split in Section 00.4c
  - "compiler/ori_repr/src/escape/mod.rs"
  - "compiler/ori_repr/src/plan/query.rs"
---

# Locality Representation Unification: Exhaustive Implementation Plan

## Mission

Establish single-source-of-truth for **escape-scope classification** in AIMS by extending the existing `ori_arc::Locality` ordered lattice with one new variant (`ArgEscaping`, sitting between `FunctionLocal` and `HeapEscaping`), defining the `ori_repr::EscapeInfo` per-function storage shape that consumes the unified type, replacing the hardcoded `ReprPlan::escapes()` query body, eliminating the `ParamContract::may_escape` `LEAK:scattered-knowledge`, and coordinating the cross-plan text in `repr-opt §08`, `§09`, `§10`, `00-overview.md`, and `index.md`. Thread-sharing stays in `RcStrategy::NonAtomic` per the existing `repr-opt §10` design; owner-rooted regions stay in `BorrowSource` (the current home for projection-aware tracking). The plan preserves the ordered escape-scope chain that `Locality::join`, cross-block widening, and canonicalize Rules 4/6/8 depend on.

## Mission Success Criteria

The mission is complete when ALL of these are true. Each criterion is concrete, testable, and traces to at least one section that delivers it.

- [ ] `compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality` enum has 5 variants (`BlockLocal`, `FunctionLocal`, `ArgEscaping`, `HeapEscaping`, `Unknown`) with discriminant order preserving `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown` — verified by a rank test in `aims/lattice/tests.rs` (delivered by Section 02)
- [ ] `grep -rE "enum (EscapeState|EscapeKind|ThreadLocality|ThreadEscape|EscapeLevel)\b" compiler/` returns **0 matches** — no parallel escape enums anywhere in the codebase (delivered by Sections 02, 03, 04)
- [ ] `compiler/ori_repr/src/escape/mod.rs` no longer contains a placeholder ZST — it defines a concrete `EscapeInfo { var_escape: FxHashMap<ArcVarId, Locality> }` consuming `ori_arc::aims::lattice::Locality` (delivered by Section 03)
- [ ] `compiler/ori_repr/src/plan/query.rs::ReprPlan::escapes()` body no longer hardcodes `let result = true;` — it consults `escape_info` for the given function and variable, mirroring the `rc_strategy()` pattern at `query.rs:124-134` (delivered by Section 03)
- [ ] `ParamContract::may_escape` is no longer a stored `bool` field. It is a derived method `may_escape(&self) -> bool` returning `self.locality_bound > Locality::BlockLocal`. The CONSERVATIVE and OPTIMISTIC constants no longer maintain it as parallel state. `ParamContract::join()` no longer ORs it. Verified by `grep -n "may_escape: " compiler/ori_arc/src/aims/contract/mod.rs` returning only function-definition lines, not field accesses (delivered by Section 02)
- [ ] `ParamContract::may_escape()` **derivation matrix exists** in `compiler/ori_arc/src/aims/contract/tests.rs`: 5 per-variant tests (one for each `Locality` variant), a self-verifying matrix completeness test that iterates all variants and asserts `may_escape() == (locality > BlockLocal)`, and a negative pin test rejecting the broken `>=` derivation. Without this matrix, a subtle off-by-one in the derivation would go undetected (delivered by Section 02.8)
- [ ] **Return-widening producer-site soundness pin exists** in `compiler/ori_arc/src/aims/intraprocedural/tests.rs`: `return_widening_promotes_arg_escaping_to_heap_escaping` asserts that `block.rs:155`'s `if entry.locality < Locality::HeapEscaping { ... }` branch correctly upgrades `ArgEscaping → HeapEscaping` for returned values. This is the test gate for soundness condition 4 (no heap persistence) from Section 01.4 and the only place where the interaction between the new variant and the return widening predicate is exercised directly (delivered by Section 02.8)
- [ ] Canonicalize Rules 4, 6, 8 in `aims/lattice/mod.rs::canonicalize_single_pass()` fire identically before and after the refactor. Verified by extending the cross-dimension rule test suite to include `ArgEscaping` cases AND by a behavioral pin test asserting `ArgEscaping + Unique` does NOT collapse to `MaybeShared` (Rule 6 does NOT fire on `ArgEscaping`). Evidence: Go's `leakCallee` and Lean 4's `borrow=true` both preserve uniqueness across the call boundary (delivered by Sections 02 and 05)
- [ ] `AimsState::CHAIN_HEIGHT` constant is updated from `15` to `16` to reflect the new chain height of `Locality` (3 → 4). The `iteration_limit()` formula recomputes correctly. The test at `aims/lattice/tests.rs:2054` is updated and passes (delivered by Section 02)
- [ ] `plans/repr-opt/section-08-escape-analysis.md` is updated: §08.1's task list rewritten to consume `ori_arc::Locality` instead of defining `EscapeState`; `depends_on: ["02", "locality-representation-unification"]` added; explicit "Soundness conditions to enforce" subsection added with conditions 2 (ownership preservation) and 4 (no heap persistence) from this plan's Section 01 (delivered by Section 04)
- [ ] `plans/repr-opt/section-09-arc-header.md` is updated: the `compute_sharing_bound()` example at `§09:75` changed from `EscapeState::NoEscape` to `EscapeInfo::is_non_escaping(var)`; `depends_on` includes `locality-representation-unification` (delivered by Section 04)
- [ ] `plans/repr-opt/section-10-thread-local-arc.md` is updated: confirms (and documents in the section text) that thread-locality routes through `RcStrategy::NonAtomic` per `repr-opt/00-overview.md:192` and does NOT define a parallel `ThreadLocality` enum; `depends_on` includes `locality-representation-unification` if §10 reads `EscapeInfo` for any input signal (delivered by Section 04)
- [ ] `plans/repr-opt/00-overview.md` lines 191-192 reference the unified `Locality` type; `plans/repr-opt/index.md` keyword clusters for §08/§09/§10 no longer mention `EscapeState` or `ThreadLocality` as standalone enums (delivered by Section 04)
- [ ] **All five BLOAT files this plan touches are pre-split below 450 lines:** `compiler/ori_arc/src/aims/lattice/mod.rs` (currently 552 → ≤80 dispatch hub), `compiler/ori_arc/src/aims/transfer/mod.rs` (524 → ≤80), `compiler/ori_arc/src/aims/interprocedural/extract.rs` (517 → leaf-to-directory promotion ≤250 mod.rs + 2 siblings; Agent 3 hygiene addition, Section 02.6 modifies the `may_escape` literal here), `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` (646 → leaf-to-directory promotion ≤300 mod.rs + 5 sibling impl files; Agent 3 hygiene addition, Section 00.6 rewrites 14 stale annotations here), `compiler/ori_arc/src/aims/interprocedural/mod.rs` (536 → ≤300 with `scc_loop.rs` extracted; Agent 3 hygiene addition, Section 00.6 rewrites 2 stale annotations here). All splits use architectural seams (responsibility-based), not mechanical chunking (delivered by Section 00)

- [ ] No source file under `compiler/ori_arc/src/aims/` contains the strings `Section 09.2`, `Section 09.3`, or `Section 09.5` as plan annotations. The **99 references across 22 files** (re-counted via `Grep "Section 09\." compiler/ori_arc/src/aims` during plan accuracy review) are rewritten to preserve their load-bearing rationale (e.g., "requires precise locality") while dropping the navigation pointers to a plan that does not exist in `plans/` or `plans/completed/`. Verified by `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` returning 0 stale Section 09 annotations in AIMS files (delivered by Section 00)

- [ ] `./test-all.sh` green — no regressions across the full test suite (delivered by Section 05)
- [ ] `./clippy-all.sh` green (delivered by Section 05)
- [ ] `/tpr-review`, `/impl-hygiene-review`, and `/improve-tooling` retrospective complete with all findings triaged (delivered by Section 05)

## Architecture

```
                          ┌─────────────────────────────┐
                          │ ori_arc::aims::lattice::    │
                          │   Locality enum (SSOT)      │
                          │                             │
                          │ BlockLocal                  │
                          │   < FunctionLocal           │
                          │   < ArgEscaping  (NEW)      │
                          │   < HeapEscaping            │
                          │   < Unknown                 │
                          └──────────────┬──────────────┘
                                         │
            ┌────────────────────────────┼────────────────────────────┐
            │                            │                            │
            ▼                            ▼                            ▼
  ┌───────────────────┐       ┌────────────────────┐      ┌─────────────────────┐
  │ AimsState         │       │ ParamContract      │      │ ori_repr::escape::  │
  │ .locality field   │       │ .locality_bound    │      │ EscapeInfo          │
  │                   │       │                    │      │                     │
  │ canonicalize      │       │ may_escape() now   │      │ var_escape:         │
  │   Rules 4, 6, 8   │       │   DERIVED from     │      │   FxHashMap<        │
  │   (preserved)     │       │   locality_bound   │      │     ArcVarId,       │
  │                   │       │   (was field)      │      │     Locality>       │
  │ .join = max       │       │                    │      │                     │
  └─────────┬─────────┘       │ join: max          │      │ join_escape_scope() │
            │                 │ (no may_escape OR) │      │ escape_scope()      │
            │                 └────────┬───────────┘      │ escapes()           │
            │                          │                  │ is_non_escaping()   │
            ▼                          │                  └──────────┬──────────┘
  ┌───────────────────┐                │                             │
  │ ori_arc producers │                │                             │
  │ (11 sites,        │                │                             │
  │  see Section 01)  │                │                             │
  │                   │                │                             │
  │ Today: produce    │                │                             │
  │   BlockLocal,     │                │                             │
  │   FunctionLocal,  │                │                             │
  │   HeapEscaping,   │                │                             │
  │   Unknown         │                │                             │
  │                   │                │                             │
  │ Future (§08):     │                │                             │
  │   ArgEscaping     │                │                             │
  │   from new        │                │                             │
  │   producer site   │                │                             │
  └───────────────────┘                │                             │
                                       │                             │
                                       ▼                             ▼
                       ┌─────────────────────────┐      ┌──────────────────────┐
                       │ MemoryContract          │      │ ReprPlan             │
                       │ (interprocedural        │      │   .escape_info:      │
                       │  fixpoint, computed     │      │     FxHashMap<       │
                       │  via SCC analysis)      │      │       Name,          │
                       │                         │      │       EscapeInfo>    │
                       └─────────────────────────┘      │                      │
                                                        │ .escapes(func, var): │
                                                        │   reads escape_info  │
                                                        │   (was hardcoded     │
                                                        │   `true`)            │
                                                        └──────────┬───────────┘
                                                                   │
                                                                   ▼
                                                        ┌──────────────────────┐
                                                        │ Future repr-opt §08  │
                                                        │   (escape analysis)  │
                                                        │ Future repr-opt §09  │
                                                        │   (sharing bound)    │
                                                        │ Future repr-opt §10  │
                                                        │   (thread-local ARC) │
                                                        │                      │
                                                        │ All consume the      │
                                                        │ unified Locality via │
                                                        │ EscapeInfo helpers   │
                                                        └──────────────────────┘
```

## Design Principles

These principles drive every decision in this plan. Each cites the concrete pain point that motivated it.

**1. Single Source of Truth (SSOT) for escape classification.**

Currently, escape classification is split across three places:
- `ori_arc::Locality` (4-variant enum, the actual canonical analysis)
- `ParamContract::may_escape: bool` (parallel state, co-maintained but never queried in production — confirmed `LEAK:scattered-knowledge`)
- `ori_repr::escape::EscapeInfo` (12-line ZST placeholder waiting for a real shape)

Plus `repr-opt §08` plans to define a fourth (`EscapeState { NoEscape, ArgEscape, GlobalEscape }` in `compiler/ori_repr/src/escape/mod.rs`), and `repr-opt §10` originally planned a fifth (`ThreadLocality { ThreadLocal, ThreadShared }` — though `repr-opt/00-overview.md:192` already specified this routes through `RcStrategy::NonAtomic` instead, eliminating the fifth).

The cascade is the textbook degradation pattern from `impl-hygiene.md`: *"The cascade: one side-logic shortcut invites another. Within months, the canonical source becomes 'one of several places' that defines behavior, and eventually no single location is authoritative."*

This plan establishes `ori_arc::aims::lattice::Locality` as the canonical home and migrates all consumers to query through it.

**2. Add ONE variant, do not refactor the representation.**

The original framing (Round 1 of the consensus loop) proposed a multi-axis struct with sub-axes for escape, thread, and future regions. Round 2 rejected this as architecturally wrong:
- The codebase relies on **total ordering** (`Locality::join = max`, Rule 8 uses `> Locality::FunctionLocal`)
- Thread-locality already routes through `RcStrategy::NonAtomic`, not a Locality axis
- Owner-rooted regions belong in `BorrowSource`, not Locality

The chosen representation is **the smallest possible change**: add `ArgEscaping` as a single new variant in the existing flat enum, slotted into the chain between `FunctionLocal` and `HeapEscaping`. PartialOrd/Ord are preserved automatically via discriminant order. All existing canonicalize rules continue to work without semantic change (Rules 4 and 6 fire on specific variants `BlockLocal` and `HeapEscaping`; Rule 8 uses `>` which slots correctly).

**3. Implementation-first ordering with hygiene foundation.**

Section 0 lays the hygiene foundation (file splits + stale annotation rewrites — all *no-semantic-change* work) before any `ArgEscaping` work begins. This is a discovery from Phase 2 research: `lattice/mod.rs` is currently 552 lines and `transfer/mod.rs` is 524 lines, both over the 500-line limit. The plan would violate `impl-hygiene.md` the moment it added a single line otherwise. (Pass 1's claim of "inline test blocks at `transfer/mod.rs:17-22` and `contract/mod.rs:21-23`" was a misread — those are correct sibling-file `mod tests;` declarations, not inline blocks. No extraction work is needed.)

After Section 0, sections proceed in **implementation-first order** (per Codex's round 2 pushback): `ori_arc` representation lands first (Section 02), then `ori_repr` plumbing (Section 03), then plan corpus coordination (Section 04). A plan that references nonexistent code is a hygiene violation; therefore the code lands before the plan text references it.

**4. Producer obligations live with producers, not with lattice helpers.**

Pass 4 surfaced 5 soundness conditions for `ArgEscaping`:
1. Monotonicity in call chains (automatic via `Locality::join`)
2. Ownership preservation: `ArgEscaping + Owned` means callee borrows, caller retains
3. Lifetime bound: callee lifetime ≤ argument lifetime (verified by `MemoryContract` effects)
4. No heap persistence: ArgEscaping must upgrade to HeapEscaping if escaped further
5. Multi-callee join (automatic via `Locality::join`)

Conditions 1 and 5 are automatic. Condition 3 lives in `MemoryContract` effects, not Locality. Conditions 2 and 4 are **producer obligations** that belong in `repr-opt §08` (the future plan that will populate `ArgEscaping`), not in this plan's lattice helpers. Adding `debug_assert!`s in `Locality::join` or `ParamContract` constructors now would either be tautologies (no producer creates the bad state yet) or wrong coupling (forcing the lattice to know about producer responsibilities).

This plan **documents** the conditions in Section 01 and **transfers** them into `repr-opt §08`'s plan text in Section 04. §08 inherits the obligation and adds asserts at the producer sites (`block.rs:97`, `block.rs:155`) and the call-lowering/arg-ownership consumer.

**5. Soundness preserved by prior-art validation, not by speculative reasoning.**

The decision that Rule 6 (`HeapEscaping + Unique → MaybeShared`) does **NOT** fire on `ArgEscaping` is grounded in two reference compilers:

- **Go**: `golang/src/cmd/compile/internal/escape/leaks.go` defines `type leaks [8]uint8` with byte 0 = `leakHeap`, byte 2 = `leakCallee`. These are independent dataflow sinks. The `flow()` function in `graph.go:199-226` only sets `attrEscapes` on values that reach a sink with `attrEscapes`. `calleeLoc` does NOT have `attrEscapes`, so a parameter that flows only to the callee is provably non-escaping.
- **Lean 4**: `lean4/src/Lean/Compiler/IR/Borrow.lean:58-60` initializes every parameter as `borrow=true` (caller retains ownership) and only narrows to `borrow=false` when ownership transfer is proven. Same distinction, inverted framing.

Two unrelated production compilers independently confirm the architectural choice. This is not a speculative invariant.

## Section Dependency Graph

```
                  ┌─────────────────────────────────────┐
                  │ Section 00 — Hygiene Foundation     │
                  │ (no semantic change)                │
                  │                                     │
                  │ - Split lattice/mod.rs (552→≤80)    │
                  │ - Split transfer/mod.rs (524→≤80)   │
                  │ - Split interprocedural/extract.rs  │
                  │   (517→≤250 mod.rs + 2 siblings)    │
                  │ - Split intraprocedural/state_map   │
                  │   (646→≤300 mod.rs + 5 siblings)    │
                  │ - Split interprocedural/mod.rs      │
                  │   (536→≤300 + scc_loop.rs)          │
                  │ - Rewrite 99 stale Section 09.x     │
                  │   annotations across AIMS subtree   │
                  │ - /add-bug for any residuals        │
                  └────────────────┬────────────────────┘
                                   │
                                   │ Gate: all touched files ≤450 lines,
                                   │       no stale Section 09 annotations
                                   ▼
                  ┌─────────────────────────────────────┐
                  │ Section 01 — Representation Decision│
                  │ (research-only, no code)            │
                  │                                     │
                  │ - Document chain ordering           │
                  │ - Document Rule 6 decision          │
                  │ - Document EscapeInfo API           │
                  │ - Document 5 soundness conditions   │
                  │ - Cite Pass 4 prior art             │
                  └────────────────┬────────────────────┘
                                   │
                                   │ Gate: architecture document complete,
                                   │       no code yet
                                   ▼
                  ┌─────────────────────────────────────┐
                  │ Section 02 — ori_arc Implementation │
                  │                                     │
                  │ - Add ArgEscaping variant           │
                  │ - Verify Rules 4/6/8                │
                  │ - Audit 13 predicate sites          │
                  │ - Convert may_escape to derived     │
                  │   (incl. 7 struct literal sites)    │
                  │ - Update CHAIN_HEIGHT (15→16)       │
                  │ - Extend lattice tests              │
                  │ - Add Lean 4 inversion comment      │
                  │   to dimensions.rs                  │
                  └────────────────┬────────────────────┘
                                   │
                                   │ Gate: cargo test -p ori_arc green,
                                   │       clippy clean, CHAIN_HEIGHT == 16
                                   ▼
                  ┌─────────────────────────────────────┐
                  │ Section 03 — ori_repr EscapeInfo    │
                  │                                     │
                  │ - Replace EscapeInfo placeholder    │
                  │ - Replace escapes() body in         │
                  │   plan/query.rs:111                 │
                  │ - Add EscapeInfo tests              │
                  └────────────────┬────────────────────┘
                                   │
                                   │ Gate: cargo test -p ori_repr green,
                                   │       behavioral parity with hardcode
                                   ▼
                  ┌─────────────────────────────────────┐
                  │ Section 04 — Plan Corpus            │
                  │              Coordination           │
                  │                                     │
                  │ - Update repr-opt §08 §08.1         │
                  │ - Update repr-opt §09 example       │
                  │ - Update repr-opt §10 (RcStrategy   │
                  │   confirmation)                     │
                  │ - Update repr-opt 00-overview.md    │
                  │ - Update repr-opt index.md          │
                  │ - Add soundness conditions to §08   │
                  └────────────────┬────────────────────┘
                                   │
                                   │ Gate: all cross-references resolve,
                                   │       /review-plan repr-opt clean
                                   ▼
                  ┌─────────────────────────────────────┐
                  │ Section 05 — Verification & Docs    │
                  │                                     │
                  │ - Test matrix: 5 variants × 3 rules │
                  │ - Soundness pin: ArgEscape + Unique │
                  │   stays Unique                      │
                  │ - Cross-crate behavioral test       │
                  │ - Plan annotation cleanup scan      │
                  │ - Update lattice/mod.rs module doc  │
                  │ - /tpr-review                       │
                  │ - /impl-hygiene-review              │
                  │ - /improve-tooling retrospective    │
                  └─────────────────────────────────────┘
```

**Sections are strictly sequential — no parallelization.** Each section's gate must pass before the next begins. Rationale:

- **00 → 01**: Section 01 documents the chosen representation; Section 00 makes the files writable (under 450 lines) so Section 02's writes don't trip the bloat limit immediately.
- **01 → 02**: Section 02 implements the decisions from Section 01. Without the locked decision, Section 02 has no spec to follow.
- **02 → 03**: Section 03's `EscapeInfo` imports `ori_arc::aims::lattice::Locality` — that import target must exist with the new variant before Section 03 can compile.
- **03 → 04**: Section 04's plan-text updates reference the unified `ori_arc::Locality` and `EscapeInfo` API — both must exist as code first, otherwise the plan text references nonexistent symbols.
- **04 → 05**: Section 05's verification depends on the full system being in place across all four prior sections.

**Cross-section interactions (must be co-implemented):**

- **Section 02 + Section 03**: The `EscapeInfo::escape_scope(var) -> Locality` API in Section 03 returns a `Locality` value defined in Section 02. If Section 03 lands before Section 02, the `Locality` type from `ori_arc` won't have `ArgEscaping` and the type signature is technically valid but produces an EscapeInfo that can never return the new variant. Section 02 must complete first.
- **Section 02 + Section 04**: The `repr-opt §08` plan text update in Section 04 references `Locality::ArgEscaping` and explains how §08 should populate it. If Section 04 lands before Section 02, the plan text references a nonexistent enum variant — a hygiene drift in the plan corpus.
- **Section 01 + Section 04**: Section 04's `repr-opt §08` plan text update adds a "Soundness conditions to enforce" subsection containing conditions 2 (ownership preservation) and 4 (no heap persistence). These conditions are **sourced from Section 01's locked decision document**, not restated independently. If Section 01's decision document drifts from what Section 04 copies into §08's plan text, the soundness obligations §08 inherits no longer match the rationale that justified them. Section 04 must literally copy the condition text from Section 01, not paraphrase.
- **Section 00 + Section 02**: Section 00 splits `lattice/mod.rs` and `transfer/mod.rs`. Section 02 then writes new code into the *split* files. If Section 02 ran first, it would put the new variant in a 552-line file that's already over the bloat limit, doubling the work because the split must then be done after.

## Implementation Sequence

```
Phase 0 — Hygiene Foundation (no semantic change)
  └─ Section 00.1: Define seams for lattice/mod.rs split (architectural, not mechanical)
  └─ Section 00.2: Execute lattice/mod.rs split into submodule structure
  └─ Section 00.3: Define seams for transfer/mod.rs split
  └─ Section 00.4: Execute transfer/mod.rs split into submodule structure
  └─ Section 00.4a: Define seams for interprocedural/extract.rs split (Agent 3 addition;
                     extract.rs is 517 lines and Section 02.6 touches its `may_escape`
                     literal — pre-split required by impl-hygiene.md "touching > 500
                     lines without splitting" rule)
  └─ Section 00.4b: Execute interprocedural/extract.rs split (leaf-to-directory promotion:
                     extract.rs → extract/{mod,consumed_params,return_info}.rs)
  └─ Section 00.4c: Pre-split BLOAT files state_map.rs (646→≤300 in 6 files via leaf-to-
                     directory + multiple impl blocks) and interprocedural/mod.rs
                     (536→≤300 with scc_loop.rs extracted) (Agent 3 addition; both files
                     are touched by Section 00.6's annotation rewrites and exceed the
                     500-line BLOAT limit)
  └─ Section 00.5: Rewrite ~19 stale Section 09.x annotations in lattice files
  └─ Section 00.6: Scan and rewrite ~80 stale annotations across 21 remaining AIMS files
  └─ Section 00.7: Safety net — /add-bug for any residual stale annotations
  Gate: lattice/mod.rs, transfer/mod.rs, AND interprocedural/extract/ files all ≤450
        lines; plan-annotations.sh returns 0 stale Section 09 references in AIMS files;
        ./test-all.sh green (no behavioral changes expected)

Phase 1 — Representation Decision (research-only)
  └─ Section 01.1: Lock the chain ordering: BlockLocal < FunctionLocal < ArgEscaping
                    < HeapEscaping < Unknown
  └─ Section 01.2: Lock the Rule 6 decision: ArgEscaping + Unique stays Unique
                    (with Go and Lean 4 evidence)
  └─ Section 01.3: Lock the EscapeInfo API: per-function FxHashMap<ArcVarId, Locality>
                    with monotone join_escape_scope writer
  └─ Section 01.4: Document the 5 soundness conditions and which apply where
                    (Conditions 1, 5: automatic. 3: MemoryContract. 2, 4: §08 obligations)
  └─ Section 01.5: Brief Pass 4 prior art citation (full inversion explanation
                    will live in dimensions.rs source comment in Section 02)
  Gate: Decision document complete; reviewed against round 2 consensus loop output;
        no code changes

Phase 2 — Core Implementation
  └─ Section 02.1: Add ArgEscaping variant to Locality enum
  └─ Section 02.2: Add Lean 4 inversion comment to dimensions.rs Locality doc
  └─ Section 02.3: Verify canonicalize Rules 4, 6, 8 fire identically
                    (preserve semantics, just add match coverage)
  └─ Section 02.4: Update CHAIN_HEIGHT constant (15 → 16) and verify iteration_limit()
  └─ Section 02.5: Audit 13 predicate sites across 7 files (no exhaustive matches
                    exist on Locality — every site is an order-based or matches!()
                    predicate that compiles cleanly without changes; per-site
                    SEMANTIC judgment required)
  └─ Section 02.6: Convert ParamContract::may_escape from field to derived method
                    (delete field, remove from 7 struct literal sites including
                    extract.rs and builtins/mod.rs PRODUCTION code, add fn,
                    remove from CONSERVATIVE/OPTIMISTIC, remove from join())
  └─ Section 02.7: Extend `all_locality()` AND `representative_states()` helpers
                    at lattice/tests.rs:26 and tests.rs:68 for ArgEscaping coverage
                    (no pre-existing bug — Pass 1 misread; `all_locality()` already
                    returns all 4 current variants, `representative_states()` is an
                    intentionally small property-test sample)

  └─ Section 02.8: Extend lattice tests for the new variant (commutativity, associativity,
                    canonicalize rules, helpers, exhaustive enumerations) PLUS add the
                    `ParamContract::may_escape()` derivation matrix (7 tests in
                    contract/tests.rs) AND the return-widening producer-site soundness
                    pins (3 tests in intraprocedural/tests.rs) — the lattice tests alone
                    do not exercise the non-lattice surfaces this plan changes
  Gate: cargo test -p ori_arc green; ./clippy-all.sh green; CHAIN_HEIGHT == 16
        in test assertion at tests.rs:2054; soundness pin (ArgEscape + Unique stays
        Unique) passes

Phase 3 — Cross-Crate Plumbing  [CRITICAL PATH]
  └─ Section 03.1: Replace ori_repr/src/escape/mod.rs placeholder with concrete
                    EscapeInfo
  └─ Section 03.2: Replace ReprPlan::escapes() body in plan/query.rs:111 to consult
                    escape_info (mirroring rc_strategy() pattern at query.rs:124-134)
  └─ Section 03.3: Add tests for EscapeInfo API (escape_scope, escapes, is_non_escaping,
                    join_escape_scope)
  └─ Section 03.4: Add cross-crate behavioral test verifying EscapeInfo round-trips
                    through ori_arc::Locality
  Gate: cargo test -p ori_repr green; behavioral parity with current hardcode verified
        (escapes() returns true for any variable not in EscapeInfo)

Phase 4 — Plan Corpus Coordination
  └─ Section 04.1: Update plans/repr-opt/section-08-escape-analysis.md (rewrite §08.1
                    task list to consume Locality + add depends_on + add Soundness
                    Conditions subsection with conditions 2 and 4)
  └─ Section 04.2: Update plans/repr-opt/section-09-arc-header.md (compute_sharing_bound
                    example uses EscapeInfo::is_non_escaping + add depends_on)
  └─ Section 04.3: Update plans/repr-opt/section-10-thread-local-arc.md (confirm
                    RcStrategy routing in section text + add depends_on if §10 reads
                    EscapeInfo)
  └─ Section 04.4: Update plans/repr-opt/00-overview.md lines 191-192 to reference
                    unified Locality
  └─ Section 04.5: Update plans/repr-opt/index.md keyword clusters to remove
                    EscapeState/ThreadLocality as standalone enums
  Gate: All cross-references in repr-opt plan text resolve to existing symbols;
        manual /review-plan plans/repr-opt/ pass

Phase 5 — Verification
  └─ Section 05.1: Test matrix — 5 Locality variants × 3 canonicalize rules
                    (includes the soundness pin from Section 02.8: ArgEscape + Unique
                    stays Unique, Rule 6 does NOT fire)
  └─ Section 05.2: Cross-crate behavioral test — full pipeline EscapeInfo writes
                    Locality, ReprPlan::escapes() reads it correctly
  └─ Section 05.3: Plan annotation cleanup scan via plan-annotations.sh
  └─ Section 05.4: Update lattice/mod.rs module doc to describe unified representation
                    + 5 soundness conditions + Go/Lean 4 prior art citations
  └─ Section 05.5: Final test suite + clippy gate (./test-all.sh + ./clippy-all.sh green)
  └─ Section 05.6: /tpr-review (full plan)
  └─ Section 05.7: /impl-hygiene-review (after TPR clean)
  └─ Section 05.8: /improve-tooling retrospective
  Gate: All success criteria from this overview met
```

**Why this order:**

- **Phase 0 must come first** because Sections 02 and 03 will write code into files currently over the 500-line limit. Without the splits, the bloat finding fires immediately.
- **Phase 1 (decision) before Phase 2 (code)** because Phase 2 needs a locked spec. The decision document is the spec.
- **Phase 2 before Phase 3** because Section 03's `EscapeInfo` imports the new `Locality` variant. The variant must exist as code first.
- **Phase 3 before Phase 4** because Section 04's plan-text references must resolve to existing symbols. Plan text that references nonexistent code is a hygiene drift in the plan corpus.
- **Phase 4 before Phase 5** because verification scans the full system including the plan text.

**Known failing tests (expected during plan execution):**

None expected. This plan is purely additive at the code level (one new variant, one method conversion, one storage shape replacement, one query body replacement) and the existing test suite should pass at every gate. If `./test-all.sh` fails after Section 02, the failure is a real bug introduced by the migration — not an expected intermediate state.

## Metrics (Current State)

Baseline measurements before implementation. These establish the starting point so progress and regressions can be measured.

| File | Production LOC | Tests (sibling) | Total | Notes |
|---|---|---|---|---|
| `compiler/ori_arc/src/aims/lattice/mod.rs` | 552 | 2365 (sibling `tests.rs`) | 2917 | **Over 500-line limit by 52** |
| `compiler/ori_arc/src/aims/lattice/dimensions.rs` | 281 | (no sibling tests; covered by `lattice/tests.rs`) | 281 | `Locality` enum here |
| `compiler/ori_arc/src/aims/transfer/mod.rs` | 524 | 1117 (sibling `tests.rs`) | 1641 | **Over 500-line limit by 24** |
| `compiler/ori_arc/src/aims/interprocedural/extract.rs` | 517 | (no sibling tests; covered by `interprocedural/tests.rs`) | 517 | **Over 500-line limit by 17** — Section 02.6 modifies the `may_escape` literal here, so Section 00.4b pre-splits it into `extract/{mod,consumed_params,return_info}.rs` |
| `compiler/ori_arc/src/aims/contract/mod.rs` | 478 | 398 (sibling `tests.rs`) | 876 | Under 500 but over the 450 proactive-split threshold; this plan REMOVES the `may_escape` field so it shrinks slightly |
| `compiler/ori_arc/src/aims/contract/context.rs` | 113 | (in `tests.rs`) | 113 | Already-extracted submodule (`ContextBehavior`, `ContextRegion`) |
| `compiler/ori_arc/src/aims/intraprocedural/block.rs` | 480 (NOT 330 — Agent 3 re-measured) | (sibling `tests.rs`) | 480 | Producer sites at lines 96-100, 150-159; at proactive-split threshold; Section 00.6 rewrites 17 stale annotations |
| `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` | 646 | 535 (sibling `state_map/tests.rs`) | 1181 | **Over 500-line limit by 146** — Section 00.4c pre-splits into directory; Section 02.5 reads predicate sites at lines 429-440; Section 00.6 rewrites 14 stale annotations |
| `compiler/ori_arc/src/aims/interprocedural/mod.rs` | 536 | (in `tests.rs`) | 536 | **Over 500-line limit by 36** — Section 00.4c pre-splits `scc_loop.rs` out; Section 00.6 rewrites 2 stale annotations |
| `compiler/ori_arc/src/aims/realize/decide.rs` | 485 | (in sibling `tests.rs` 1572 lines) | 2057 | Under 500 but at proactive-split threshold; Section 00.6 rewrites 1 stale annotation; flagged as future split candidate |
| `compiler/ori_arc/src/aims/intraprocedural/effects.rs` | ~80 | (none) | ~80 | Order predicate at line 38 |
| `compiler/ori_repr/src/escape/mod.rs` | 12 | (none) | 12 | Placeholder unit struct only |
| `compiler/ori_repr/src/plan/query.rs` | ~140 | (in tests) | ~140 | Hardcoded escapes() at line 111 |
| **AIMS Locality consumers in ori_arc** | **15 files** | — | **~450 call sites** | Per Pass 1 inventory |
| **Stale Section 09.x annotations** | **99** | — | — | Across 22 files in `aims/` — see Section 00 Context table for distribution |

## Estimated Effort

| Section | Est. Lines (new+modified) | Complexity | Depends On |
|---|---|---|---|
| 00 Hygiene Foundation | ~480 (5 file splits + 99 annotation rewrites; includes Agent 3 additions) | Medium-High | — |
| ↳ 00.1-00.4 lattice + transfer file splits | ~120 | Medium | — |
| ↳ 00.4a-00.4b extract.rs split (Agent 3 addition) | ~60 | Medium | 00.1-00.4 |
| ↳ 00.4c state_map.rs + interprocedural/mod.rs splits (Agent 3 addition) | ~140 | Medium-High | 00.1-00.4b |
| ↳ 00.5 Lattice annotation rewrites (~19 occurrences in lattice/) | ~40 | Low | — |
| ↳ 00.6 Remaining ~27 files (~80 occurrences, post-split distribution) | ~120 | Medium | — |
| ↳ 00.7 Residual scan outside `aims/` | ~10 | Low | — |
| 01 Representation Decision | ~100 (decision doc) | Low | 00 |
| 02 ori_arc Implementation | ~280 (code) + ~200 (tests, +50 for may_escape derivation matrix and return-widening pins) | Medium-High | 00, 01 |
| ↳ 02.1-02.4 Variant + chain height | ~30 | Low | 01 |
| ↳ 02.5 Predicate migration (13 sites, 7 files, semantic per-site) | ~100 | Medium | 02.1 |
| ↳ 02.6 may_escape conversion (field + 7 literal sites + reads) | ~70 | Medium | 02.1 |
| ↳ 02.7-02.8 Test extension (+9 commutativity, +61 associativity, +7 may_escape derivation, +3 return-widening) | ~200 | Medium | 02.1 |
| 03 ori_repr EscapeInfo | ~80 (code) + ~60 (tests) | Low | 02 |
| 04 Plan Corpus Coordination | ~150 (plan text edits) | Low | 03 |
| 05 Verification & Docs | ~100 (tests + docs) | Low | 04 |
| **Total new** | **~1,090 lines across 6 sections** | — | — |
| **Total deleted** | **~30 lines** (may_escape field, placeholder, dead annotation refs) | — | — |

The bulk of the line count is **test extension** (Sections 02.7–02.8) and **documentation** (Sections 01, 04). The actual semantic code change (Sections 02.1, 02.6, 03.1, 03.2) is small — under 200 lines.

## Known Bugs (Pre-existing)

Discovered during Phase 2 research. Each entry is tracked here so it doesn't get lost during execution.

| Bug | Root Cause | Fix Location | Status |
|---|---|---|---|
| `ParamContract::may_escape` is `LEAK:scattered-knowledge` — co-maintained with `locality_bound` but never queried in production | The field was added at the same time as `locality_bound` but the consumer migration was never completed. Confirmed by Pass 1 Agent 1: zero non-test readers anywhere in the codebase | Section 02.6 (convert to derived method) | Tracked here, not filed separately because the fix is part of this plan |
| **99 stale `Section 09.x` annotations across 22 files** in `compiler/ori_arc/src/aims/` (concentrated in `lattice/mod.rs:19`, `intraprocedural/block.rs:17`, `intraprocedural/state_map.rs:14`, `intraprocedural/tests.rs:14`, plus 18 other files with 1-4 each). The matching plan section does NOT exist in `plans/` or `plans/completed/`. Phrase "Effect Activation" appears in many source files but in zero plan files | The originating plan was either renamed during a restructuring, deleted without cleanup, or never tracked formally. Per `impl-hygiene.md`: *"Stale annotations from completed plans are hygiene violations (DRIFT category)"* | Section 00.5 (lattice files), Section 00.6 (remaining 21 files), Section 00.7 (residual scan outside `aims/`) | Tracked here |
| **`compiler/ori_arc/src/aims/interprocedural/extract.rs` is 517 lines (BLOAT)** — over the 500-line limit by 17 lines. Section 02.6 modifies the `may_escape` literal at lines 79-92, which under `compiler.md` §File Size and `impl-hygiene.md` §File Organization triggers the "touching a file > 500 lines without splitting is a finding" rule | The file accumulated three responsibilities (`extract_contract` orchestration, `detect_consumed_params` alias-tracking, `extract_return_info` Return-uniqueness analysis) without a split when the third was added | Section 00.4a (define seams) + 00.4b (execute leaf-to-directory promotion) | Tracked here (Agent 3 hygiene addition) |
| **`compiler/ori_arc/src/aims/intraprocedural/state_map.rs` is 646 lines (BLOAT)** — over the 500-line limit by 146 lines. Section 02.5 reads predicate sites at lines 429-440 (no write), but Section 00.6 rewrites 14 stale `Section 09.x` annotations in this file (clear write — full BLOAT trigger) | The `AimsStateMap` struct accumulated ~40 methods grouped by responsibility (constructors, block-state queries, cross-dim counters, borrow provenance, invoke edge state, var shapes, events, FIP balance) without ever extracting helper modules | Section 00.4c (leaf-to-directory promotion via multiple `impl AimsStateMap` blocks across 5 sibling files) | Tracked here (Agent 3 codebase scan find) |
| **`compiler/ori_arc/src/aims/interprocedural/mod.rs` is 536 lines (BLOAT)** — over the 500-line limit by 36 lines. Section 00.6 rewrites 2 stale `Section 09.x` annotations in this file | The SCC fixpoint loop (`analyze_scc_fixpoint`) and post-fixpoint demand propagation (`tighten_uniqueness_from_callers`) live in the same file as the orchestration entry (`analyze_program`) instead of a sibling | Section 00.4c (extract `scc_loop.rs`) | Tracked here (Agent 3 codebase scan find) |

None of these bugs require `/fix-bug` because all are absorbed into this plan's sections per the "fix when you find it" rule in `impl-hygiene.md`. If any stale annotation outside our touched set is discovered during execution, Section 00.9 files an `/add-bug` for the residual.

## Quick Reference

| ID | Title | File | Status |
|---|---|---|---|
| 00 | Hygiene Foundation | `section-00-hygiene-foundation.md` | Not Started |
| 01 | Representation Decision | `section-01-representation-decision.md` | Not Started |
| 02 | ori_arc Implementation | `section-02-ori-arc-implementation.md` | Not Started |
| 03 | ori_repr EscapeInfo Storage | `section-03-ori-repr-escape-info.md` | Not Started |
| 04 | Plan Corpus Coordination | `section-04-plan-corpus-coordination.md` | Not Started |
| 05 | Verification & Documentation | `section-05-verification.md` | Not Started |
