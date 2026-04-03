---
section: "07"
title: "Stale Annotations and Decorative Banners"
status: not-started
reviewed: false
goal: "Remove all stale plan annotations (TPR-01/03/04 from completed plans), decorative banners, and bare TODOs from production code"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Remove Stale TPR Annotations"
    status: not-started
  - id: "07.2"
    title: "Remove Decorative Banners"
    status: not-started
  - id: "07.3"
    title: "Resolve Bare TODOs"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Stale Annotations and Decorative Banners

**Status:** Not Started
**Goal:** Remove ~71 stale plan annotations from completed plans, ~37 decorative banner comments, and 8+ bare TODO comments from production code across all compiler crates. Preserve annotations from active plans (repr-opt §07, aot-perf).

**Context:** Plan annotations are temporary scaffolding — they aid navigation during active development but MUST be removed when the plan completes. Stale annotations from completed plans (hygiene-full sections 01/03/04, codegen-purity, rc-header-elem-dec) remain in production code. Decorative banners (`// ===`, `// ---`) violate style rules.

---

## 07.1 Remove Stale TPR Annotations

**File(s):** Multiple files across ori_parse, ori_llvm, ori_repr

**Active plans (PRESERVE these annotations):**
- `§07.x`, `TPR-07-*` — repr-opt Section 07 (Enum Repr, in-progress)
- `Section 01.x` in ori_llvm — repr-opt Section 01 (if still active)
- `BUG-04-*` — bug tracker references (always acceptable)

**Stale (REMOVE):**
- `TPR-01-*` in `ori_parse/src/module_parse.rs` (lines 55, 210, 214, 243, 271) and `grammar/attr/repr.rs` (line 4)
- `TPR-01-*` in `ori_parse/src/incremental/tests.rs` (~11 test name references)
- `TPR-03-*` in `ori_llvm/src/evaluator/compile.rs` and `lib.rs` — audit each: remove if from completed plan, preserve if from active repr-opt
- `TPR-04-*` in `ori_llvm/src/evaluator/compile.rs` — audit: remove if from completed plan
- `TPR-03-*` in `ori_repr/src/range/fixpoint/` — these reference repr-opt Section 03 (range analysis). Check if section is complete; if so, remove

- [ ] Run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` to get the full list
- [ ] For each annotation: check if the referenced plan section is complete or active
- [ ] Remove annotations from completed plan sections (keep the behavioral comment, remove the plan reference)
- [ ] Preserve annotations from active plan sections
- [ ] Verify: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` reports only active plan annotations

---

## 07.2 Remove Decorative Banners

**File(s):** Multiple files across ori_types, ori_rt, ori_parse

Decorative banners (`// ===`, `// ---`, `// ───`) in production code violate style rules. Replace with plain `// Section name` comments.

Known locations:
- `ori_types/src/unify/mod.rs` — 10 decorative banners
- `ori_types/src/infer/mod.rs` — 22 decorative banners
- `ori_types/src/check/bodies/mod.rs` — 2 banners
- `ori_types/src/check/signatures/mod.rs` — 6 banners
- `ori_rt/src/format/mod.rs` — 12 banners
- `ori_parse/src/cursor/mod.rs` — 2 banners
- `ori_parse/src/outcome/mod.rs` — 5 banners
- `ori_parse/src/error/kind/mod.rs` — 8 banners

- [ ] `grep -rn "// ===\|// ---\|// ───" compiler/*/src/ --include="*.rs" | grep -v test` to find all
- [ ] Replace each with a plain `// Section name` comment (keep the descriptive text, remove the decoration)
- [ ] Verify: no decorative banners remain in production code

---

## 07.3 Resolve Bare TODOs

**File(s):** Multiple files across ori_types, ori_eval

Bare TODOs without plan references are non-actionable. Each must be either: (a) filed as a bug via `/add-bug`, (b) tracked in an existing plan, or (c) removed if already resolved.

Known locations:
- `ori_types/src/infer/expr/control_flow.rs:576` — TODO(inference): ForLoopParams struct
- `ori_types/src/infer/expr/constructors.rs:94` — TODO: await inference
- `ori_types/src/infer/expr/type_resolution.rs:53` — TODO: fixed list support
- `ori_types/src/check/registration/traits.rs:244` — TODO: bounds on associated type
- `ori_eval/src/interpreter/can_eval/mod.rs:119` — TODO(canonicalization)
- `ori_eval/src/exec/decision_tree/mod.rs:266,276` — TODO(section-07)

- [ ] For each TODO: determine if the work is already tracked elsewhere
- [ ] If untracked: file via `/add-bug` or add to relevant plan section
- [ ] If resolved: remove the comment
- [ ] If genuinely deferred with plan reference: convert to `<!-- blocked-by:plan/section -->` format

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] All stale plan annotations removed (only active plan annotations remain)
- [ ] All decorative banners replaced with plain section comments
- [ ] All bare TODOs resolved (filed, tracked, or removed)
- [ ] `timeout 150 ./test-all.sh` passes (zero behavioral changes)
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 07
- [ ] `/impl-hygiene-review last commit`
