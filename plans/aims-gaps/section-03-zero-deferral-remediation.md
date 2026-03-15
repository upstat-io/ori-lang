---
section: "03"
title: "Deferred Comment Remediation"
status: complete
goal: "Classify every deferred/future/pending comment in AIMS code and reword for ZERO DEFERRAL compliance."
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Deferred Item Inventory"
    status: complete
  - id: "03.2"
    title: "Tracked vs Untracked Classification"
    status: complete
  - id: "03.3"
    title: "Comment Reword Execution"
    status: complete
  - id: "03.4"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Deferred Comment Remediation

**Status:** Complete (2026-03-15)
**Goal:** Classify every "deferred"/"future work"/"pending" comment in AIMS code paths and reword each to comply with ZERO DEFERRAL policy. No actual bugs were found — all deferred items are either language dependencies or optimization opportunities, so the work is comment rewords and classification, not code fixes.

## 03.1 Deferred Item Inventory

**File(s):** `compiler/ori_arc/src/aims/**/*.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/**/*.rs`

**Baseline:** The AIMS codebase has ZERO `TODO`/`FIXME`/`HACK`/`XXX` comments and ZERO
`#[allow(dead_code)]` attributes. There is exactly one `#[expect(dead_code)]`
(`EffectPurityViolation` in `aims/normalize/verify.rs:64-69`, documented reason:
effect-handler dependency).

**Grep count vs actual deferred items:** A raw `grep -rci "deferred\|pending\|future" compiler/ori_arc/src/aims/` returns ~104 matches, but most are code identifiers (struct/variable names), not deferred-work comments. The classification below separates them:

- **Code identifiers (NOT policy violations):** `PendingRc` (struct), `pending_decs` (variable),
  `terminator_deferred` (field — deferred parent RcDec, a legitimate optimization technique),
  `flush_pending`, `accumulate_*_pending`, `pending map`, `deferred parent decs`,
  `deferred.*borrowed children`. These are found in `emit_rc/coalesce/mod.rs` (most of the 27),
  `realize/walk.rs` (most of the 19), `realize/mod.rs` (~8 of 13), `realize/decide.rs` (~4 of 6),
  `emit_rc/edge_cleanup.rs` (4 of 7), `emit_rc/dead_cleanup.rs` (all 3).
- **Actual deferred-work comments (potential policy violations, ~8-10):**
  - `normalize/verify.rs:184` — "deferred to effect-handler implementation" (language dependency)
  - `normalize/rewrite.rs:45` — "Effect purity: deferred to effect-handler implementation"
  - `normalize/mod.rs:109` — "Effect purity gate (may_share) is deferred to effect-handler"
  - `emit_reuse/planner.rs:15` — "MaybeShared... requires two-point CFG expansion and is deferred"
  - `emit_reuse/planner.rs:78` — "MaybeShared cross-block requires two-point CFG expansion (deferred)"
  - `interprocedural/mod.rs:180` — "Deferred — the clone cost is bounded"
  - `contract/context.rs:25` — "the effect gate is a pending design decision"

**Root deferred issues (deduplicated, 3 root causes):**
1. **Effect purity gate** — 4 occurrences (normalize/verify.rs, rewrite.rs, mod.rs, contract/context.rs). Language dependency: Ori has no effect handlers. NOT a ZERO DEFERRAL violation — it's an out-of-scope language feature dependency, documented in overview §Scope.
2. **MaybeShared cross-block reuse** — 2 occurrences (emit_reuse/planner.rs). Optimization enhancement, not a bug. MaybeShared reuse falls back to runtime IsShared check instead of compile-time CFG expansion. No correctness impact.
3. **Interprocedural LayeredMap clone** — 1 occurrence (interprocedural/mod.rs). Performance note about bounded clone cost; not a deferred bug.

- [x] Run `grep -rn "deferred\|future work\|pending.*design\|pending.*decision" compiler/ori_arc/src/aims/ --include="*.rs"` and classify each match as either a code identifier or a deferred-work comment using the classification above. (2026-03-15) ~104 raw matches; 7 policy-relevant comments identified, rest are code identifiers (`PendingRc`, `terminator_deferred`, `deferred_parent_decs`, etc.).
- [x] Verify the `#[expect(dead_code)]` on `EffectPurityViolation` in `aims/normalize/verify.rs` is still present and its reason ("deferred to effect-handler implementation") is accurate. (2026-03-15) Present at line 64-68. Reason reworded to "out of scope" phrasing.
- [x] Deduplicate deferred-work comments by root issue (3 root causes identified above). (2026-03-15) 3 root causes confirmed: effect purity gate (4 occurrences), MaybeShared cross-block (2 occurrences), LayeredMap clone (1 occurrence). Plus 1 scope-adjacent item in `aims_pipeline.rs`.
- [x] For each root issue, assign one of: (a) language dependency (out of scope — no action beyond reword), (b) optimization opportunity (no correctness impact — reword only), or (c) actual ZERO DEFERRAL violation (requires immediate code fix). (2026-03-15) Results: (a) effect purity gate = language dependency, (b) MaybeShared = optimization opportunity, (c) LayeredMap clone = optimization note. No ZERO DEFERRAL violations found.

**Deferred items in scope-adjacent files (outside `aims/` but in the plan's file scope):**
- `pipeline/aims_pipeline.rs:51` — "deferred" comment about Section 11 regression guards (SynergyMetrics comparison). Classify: resolved or still needed.
- `arc_emitter/mod.rs:143` — "Deferred phi incoming values" — **code identifier** (NOT policy violation). The "deferred" here refers to phi node construction ordering, not deferred work.
- `arc_emitter/builtins/prelude.rs:77` — `str()` for struct/enum needs derived Printable::to_str — **actual gap**, but NOT AIMS-specific. General LLVM codegen gap.
- `arc_emitter/builtins/prelude.rs:102` — `int(str)` needs runtime `ori_int_from_str` — **actual gap**, NOT AIMS-specific.
- `arc_emitter/builtins/prelude.rs:124` — `float(str)` needs runtime `ori_float_from_str` — **actual gap**, NOT AIMS-specific.

**Recommendation:** The 3 `prelude.rs` items are general LLVM codegen gaps, not AIMS-specific. They should be classified as out of scope for this plan (they exist in `builtins/`, not in AIMS analysis or realization code). The `aims_pipeline.rs` item needs evaluation.

**Lint discipline findings in scope-adjacent `arc_emitter/` files:**

Per `impl-hygiene.md` rule: "Never bare `#[allow(clippy::...)]`" — use `#[expect(` instead.
These are NOT AIMS-specific but fall within this section's declared file scope:
- `arc_emitter/builtins/prelude.rs:20` — `#[allow(dead_code)]` on `HANDLED_PRELUDE_NAMES`
- `arc_emitter/builtins/prelude.rs:30` — `#[allow(dead_code)]` on `PENDING_PRELUDE_NAMES`
- `arc_emitter/mod.rs:84` — `#[allow(dead_code)]` on `FuncletPadKind::Catch`
- `arc_emitter/context.rs:185` — `#[allow(dead_code)]` on convenience method
- `arc_emitter/builtins/mod.rs:54,106,160,166,234,238` — 6 `#[allow(dead_code)]` instances

These should be converted to `#[expect(dead_code, reason = "...")]` if any of these
files are modified during this plan. Classified as out of scope for this plan (not
AIMS code), but flagged for tracking.

**Bare TODO without plan reference:**
- `arc_emitter/operators/mod.rs:76` — `// TODO(typeck): register missing methods so Error type doesn't propagate to codegen` — no plan/roadmap item referenced. Per hygiene rules, TODOs must reference a plan item.

## 03.2 Tracked vs Untracked Classification

**File(s):** `plans/aims/*.md`

- [x] For each of the 3 root deferred issues, search `plans/aims/*.md` for explicit tracking (grep for issue keywords). Record whether tracked or untracked. (2026-03-15) Effect purity: tracked (overview §Scope, Section 13). MaybeShared: partially tracked (Section 05, not in §6 Remaining Work). LayeredMap clone: untracked.
- [x] Assign severity (`critical`/`high`/`medium`/`low`) and class (`bug`/`optimization`/`scope-dependency`) to each root issue. (2026-03-15) See pre-classification table above — all Low severity, no bugs.
- [x] If any untracked item is classified as `bug` with severity `high` or above, it is a ZERO DEFERRAL violation requiring immediate code fix before proceeding. (2026-03-15) No bugs found. No ZERO DEFERRAL violations.

**Pre-classification (from code verification):**

| Root issue | Tracked? | Location in plans/aims/ | Severity | Class | Action |
|-----------|----------|------------------------|----------|-------|--------|
| Effect purity gate | Yes | Overview §Scope; Section 13 `EffectPurityViolation` | Low | Scope-dependency | Reword comment from "deferred" to "out of scope." |
| MaybeShared cross-block | Partially | Not in overview §6 Remaining Work | Low | Optimization | Reword comment; document as optimization opportunity with working runtime fallback. |
| LayeredMap clone | No | Not tracked anywhere | Low | Optimization | Reword comment to remove "Deferred" phrasing. |

## 03.3 Comment Reword Execution

**File(s):** `aims/normalize/verify.rs`, `aims/normalize/rewrite.rs`, `aims/normalize/mod.rs`, `aims/contract/context.rs`, `aims/emit_reuse/planner.rs`, `aims/interprocedural/mod.rs`

**Note:** Pre-classification (03.2) found no actual bugs — all 3 root issues are language dependencies or optimization notes. This section is comment rewords, not code fixes. The TDD protocol (write failing test first) does not apply to comment-only changes.

- [x] **Effect purity gate (4 files):** In `normalize/verify.rs`, `normalize/rewrite.rs`, `normalize/mod.rs`, and `contract/context.rs`, change "deferred to effect-handler implementation" to "out of scope" phrasing. (2026-03-15) Also reworded `#[expect(dead_code)]` reason on `EffectPurityViolation` and `aims_pipeline.rs:51`.
- [x] **MaybeShared cross-block (2 locations):** In `emit_reuse/planner.rs` lines 15 and 78, change "deferred" to "optimization opportunity" phrasing. (2026-03-15)
- [x] **LayeredMap clone (1 location):** In `interprocedural/mod.rs` line 180, change "Deferred" to "Performance note" phrasing. (2026-03-15)
- [x] After all rewords, run `cargo test -p ori_arc --lib` to verify no test regressions. (2026-03-15) 986/986 passed.

### Files exceeding 500-line limit (informational)

The files touched by comment rewords above exceed the 500-line limit. These are not blocking
for comment-only changes but should be split when next modified substantively:

| File | Lines | Suggested split |
|------|-------|-----------------|
| `normalize/rewrite.rs` | 569 | Extract `emit_prologue`/`emit_recursive_path` to `rewrite/blocks.rs` |
| `normalize/verify.rs` | 559 | Extract `verify_trmc_soundness` to `verify/soundness.rs` |
| `interprocedural/mod.rs` | 507 | Extract `tighten_uniqueness_from_callers` to `demand_propagation.rs` |
| `pipeline/aims_pipeline.rs` | 553 | Extract `run_second_pass` to `second_pass.rs` |

## 03.4 Completion Checklist

- [x] All 3 root deferred issues are classified (language dependency, optimization opportunity, or bug) and no bugs were found. (2026-03-15)
- [x] All 7 deferred-work comments are reworded to remove "deferred" phrasing (replaced with "out of scope" or "optimization opportunity" as appropriate). (2026-03-15) 8 rewords total (7 in AIMS core + 1 in aims_pipeline.rs).
- [x] `grep -rn "deferred" compiler/ori_arc/src/aims/ --include="*.rs"` returns only code identifiers (`PendingRc`, `terminator_deferred`, `pending_decs`, etc.) and no policy-violating deferred-work comments. (2026-03-15) Verified — all remaining matches are code identifiers in `emit_rc/edge_cleanup.rs` and `realize/`.
- [x] `cargo test -p ori_arc --lib` passes after comment rewords (no regressions). (2026-03-15) 986/986 passed.
