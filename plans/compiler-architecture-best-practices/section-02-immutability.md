---
section: "02"
title: "AST/IR Immutability Contract"
status: not-started
reviewed: false
goal: "Explicitly document the AST/IR immutability invariant in rule files — the type-system enforcement already exists, this section makes it an auditable rule"
success_criteria:
  - "impl-hygiene.md contains an explicit 'IR nodes are immutable after their construction phase' rule"
  - "parse.md references the immutability invariant with the &ExprArena enforcement mechanism"
  - "No new code changes needed — the existing type-system enforcement is already correct"
inspired_by:
  - "TypeScript immutable AST (compiler/types.ts — readonly modifiers on AST node types)"
  - "Lean 4 immutable IR (src/Lean/Compiler/IR/ — functional transformation, no mutation)"
  - "Rust interned types via TyCtxt — structurally immutable after interning"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Document Immutability Rule"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: AST/IR Immutability Contract

**Status:** Not Started
**Goal:** Make the existing AST/IR immutability invariant an explicit, auditable rule. Research confirmed that the enforcement already exists via Rust's type system (`&ExprArena`, `&Pool` at all downstream boundaries). This section documents what's true, not changes what exists.

**Success Criteria:**
- [ ] `impl-hygiene.md` §Phase Boundaries contains an explicit immutability rule — satisfies mission criterion "AST/IR immutability explicitly documented"
- [ ] `parse.md` references the immutability enforcement mechanism (`&ExprArena` at phase boundaries)
- [ ] No regression — `timeout 150 ./test-all.sh` green

**Context:** Research confirmed the codebase already enforces immutability correctly:
- `&ExprArena` (immutable) at all downstream boundaries — canonicalization receives `lower(src: &ExprArena, ...)` at `ori_canon/src/lower/mod.rs:42`
- `&mut ExprArena` appears only in `ori_parse/src/incremental/copier/` (AST copying for incremental) and `ori_fmt` test files — parser-internal, not cross-phase
- `&Pool` at all downstream boundaries — canonicalization, eval, codegen all receive `&Pool`
- `&mut Pool` only within `ori_types` (type checker needs it for interning during inference/substitution) — not passed downstream

The gap is documentation, not enforcement. Gemini's insight was correct: adding `debug_assert!` on arena-indexed data is impractical and redundant with Rust's borrow checker. The type system IS the enforcement mechanism.

**Reference implementations:**
- **TypeScript** `compiler/types.ts`: AST node types use `readonly` modifiers
- **Lean 4** `src/Lean/Compiler/IR/`: functional IR transformation — new IR produced, old untouched
- **Rust** `rustc_middle/src/ty/mod.rs`: `Ty<'tcx>` is an interned, structurally immutable handle

**Depends on:** Section 01 (policy language foundation).

---

## 02.1 Document Immutability Rule

**File(s):** `.claude/rules/impl-hygiene.md`, `.claude/rules/parse.md`

- [ ] Add rule text to `impl-hygiene.md` §Phase Boundaries (after "Clean ownership transfer" bullet, approximately line 77):
  ```
  - **IR immutability after construction**: after a phase completes, its output IR nodes are
    deeply immutable. No downstream phase may mutate arena-allocated AST (`ExprArena`), type
    pool entries (`Pool`), or canonical IR (`CanExpr`) in place. Enforcement: Rust's type
    system — downstream phases receive `&ExprArena`, `&Pool`, `&CanExpr`, never `&mut`.
    Transformations produce new IR via arena allocation in the consuming phase's own arena.
    `&mut Pool` is permitted ONLY within `ori_types` (interning during inference requires
    append-only mutation per `types.md` §TY-6). `&mut ExprArena` is permitted ONLY within
    `ori_parse` (construction) and `ori_parse/incremental/copier` (AST copying for incremental
    reparsing). Violations: introducing `&mut ExprArena` or `&mut Pool` parameters in
    downstream consumer functions is a LEAK:phase-bleeding finding.
  ```

- [ ] Add reference in `parse.md` §AR-1 or nearby: "After parsing completes, the `ExprArena` is immutable — all downstream phases receive `&ExprArena`. See `impl-hygiene.md` §Phase Boundaries for the cross-phase immutability rule."

- [ ] Add reference in `canon.md` §5 Phase Purity: add a bullet noting the immutability invariant with cross-reference to `impl-hygiene.md`

- [ ] **Subsection close-out (02.1)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Subsection 02.1 complete
- [ ] Rule text is accurate — verified against actual code (`&ExprArena`, `&Pool` at boundaries)
- [ ] `timeout 150 ./test-all.sh` passes (no regressions from rule-only changes)
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
