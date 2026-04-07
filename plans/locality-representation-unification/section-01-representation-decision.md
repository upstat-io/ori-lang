---
section: "01"
title: "Representation Decision"
status: not-started
reviewed: false
goal: "Lock the architectural decisions for the unified Locality representation: chain ordering, Rule 6 firing, EscapeInfo API shape, and the 5 soundness conditions — with zero code changes. Section 02 implements against this locked decision."
success_criteria:
  - "A decision document exists at compiler/ori_arc/src/aims/lattice/DECISION.md (or inline in this section's text) capturing: chain ordering rationale, Rule 6 decision with Go and Lean 4 evidence, EscapeInfo API contract, 5 soundness conditions"
  - "The decision document is internally consistent: every claim about prior art is sourced, every soundness condition is named with its enforcement layer (auto via join, MemoryContract, or §08 producer)"
  - "Section 02's implementation can be derived mechanically from the decision document with no further design judgement"
  - "No code is changed in this section — verified by `git diff --stat compiler/` returning zero modified lines"
inspired_by:
  - "Go cmd/compile/internal/escape/leaks.go (leakCallee semantics)"
  - "Lean 4 src/Lean/Compiler/IR/Borrow.lean:58-60 (initBorrow inverted framing)"
  - "Racordon 2019 thesis §6.2.2 transient ownership at conditional merge points"
  - "OxCaml modes documentation (deep locality refinement)"
depends_on: ["00"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Lock the chain ordering and rationale"
    status: not-started
  - id: "01.2"
    title: "Lock the Rule 6 decision (does NOT fire on ArgEscaping)"
    status: not-started
  - id: "01.3"
    title: "Lock the EscapeInfo API shape"
    status: not-started
  - id: "01.4"
    title: "Document the 5 soundness conditions and their enforcement layers"
    status: not-started
  - id: "01.5"
    title: "Brief Pass 4 prior art citation"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Representation Decision

**Status:** Not Started
**Goal:** Lock the architectural decisions for the unified Locality representation. This section produces a **decision document** — no code, no tests, no plan-text edits. The decision document is the spec that Section 02 implements against. If Section 02 has to make a design judgement that isn't in this decision document, that's a gap in this section.

**Success Criteria:**

- [ ] A decision document exists capturing all 5 subsection outcomes (chain ordering, Rule 6, EscapeInfo API, soundness conditions, prior art citation). Whether the document lives as a `DECISION.md` file in the plan directory, as inline text in this section file, or as both is the implementer's call — but it must be machine-readable enough that Section 02 can copy from it
- [ ] The decision document is internally consistent: every prior-art claim has a file/line citation; every soundness condition is named with its enforcement layer
- [ ] Section 02's implementation can be derived **mechanically** from the decision document with no further design judgement
- [ ] No code is changed: `git diff --stat compiler/` returns zero modified lines after this section completes
- [ ] Connects upward to mission criterion: "Locality enum has 5 variants with discriminant order preserving BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown — verified by a rank test in `aims/lattice/tests.rs`"
- [ ] Connects upward to mission criterion: "Canonicalize Rules 4, 6, 8 fire identically before and after the refactor... AND a behavioral pin test asserting `ArgEscaping + Unique` does NOT collapse to `MaybeShared`"

**Context:** The plan creation process (Steps 1D, 6B, 8B) already produced these decisions through three rounds of consensus with codex. This section's purpose is to **commit them to a durable location** in the plan corpus so the Section 02 implementer doesn't have to re-derive them from the consensus-loop transcripts. A decision that lives only in a transient consultation log is not a decision — it's a memory.

The section is intentionally short. Each subsection is 1-2 paragraphs locking one decision. The implementation work is in Section 02; this section is the spec.

**Reference implementations:**

- **Go** `~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/escape/leaks.go`: `type leaks [8]uint8` with byte 0 = `leakHeap`, byte 2 = `leakCallee`. Independent dataflow sinks per `~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/escape/graph.go:232-234`. The `flow()` function at `graph.go:199-226` only sets `attrEscapes` on values reaching a sink with `attrEscapes` — `calleeLoc` does NOT have it, so a value flowing only to the callee is provably non-escaping.
- **Lean 4** `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/Borrow.lean:58-60`: `def initBorrow := ps.map fun p => { p with borrow := p.ty.isPossibleRef }`. Every parameter starts as borrowed (caller retains ownership) and narrows to owned only when ownership transfer is proven.
- **Swift** `~/projects/reference_repos/lang_repos/swift/SwiftCompilerSources/Sources/Optimizer/FunctionPasses/ComputeEscapeEffects.swift:152-156`: `enum EscapeDestination { case notSet; case toReturn(SmallProjectionPath); case toArgument(Int, SmallProjectionPath) }`. Validates that separating locality from uniqueness is sound — Swift uses ownership conventions (`@guaranteed`, `@owned`) at the signature for lifetime safety, distinct from the escape analysis dataflow.

**Depends on:** Section 00 (hygiene foundation must complete first so the decision document references the post-split file structure, not the pre-split monolith).

---

## 01.1 Lock the chain ordering and rationale

**File(s):** `compiler/ori_arc/src/aims/lattice/DECISION.md` (NEW — can be inline in this section's body if preferred)

**Decision (locked):** The unified `Locality` enum has **5 variants** in this discriminant order:

```
BlockLocal  <  FunctionLocal  <  ArgEscaping  <  HeapEscaping  <  Unknown
    0              1                2                3              4
```

`ArgEscaping` is **slotted between `FunctionLocal` and `HeapEscaping`**, not at the end. This position is load-bearing for three reasons:

1. **`Locality::join = max` continues to work.** Adding the variant in the middle of the chain preserves the existing join semantics. A join of `(FunctionLocal, ArgEscaping)` returns `ArgEscaping`; a join of `(ArgEscaping, HeapEscaping)` returns `HeapEscaping`. The widening direction (more escaping = larger discriminant) is preserved.
2. **Existing canonicalize Rule 8 (`Borrowed → locality ≤ FunctionLocal`) continues to widen `ArgEscaping → FunctionLocal` for borrows.** This is the correct semantics: a borrow cannot outlive its parameter passing. The rule uses `>` comparison and the new variant slots correctly.
3. **The order matches Go's `leakCallee` semantics**: a value that flows to a callee is more escaping than one that stays function-local but less escaping than one that escapes to the heap or a return value.

**Alternative orderings considered and rejected:**

| Ordering | Rejected because |
|---|---|
| `... < HeapEscaping < ArgEscaping < Unknown` (above heap) | Wrong direction: ArgEscape is *less* escaping than heap escape, not more. Would break Rule 8's widening semantics. |
| `... < ArgEscaping < FunctionLocal < ...` (below function) | Wrong direction: ArgEscape is *more* escaping than purely function-local. Would cause Rule 4 (`BlockLocal + Owned + ≤Once → Unique`) to fire on values that have crossed a call boundary. |
| Multi-axis struct with `escape` and `region` sub-axes | Rejected in round 2 of the consensus loop. Breaks total ordering, breaks `Locality::join = max`, requires refactoring 22 consumers' comparison sites. |

- [ ] Write the chain ordering decision into the decision document with the table above
- [ ] Document the three load-bearing reasons (rule preservation, comparison preservation, semantic match)
- [ ] Document the three rejected alternatives with rejection reasoning
- [ ] Reference Section 00's split layout — the decision document points to `compiler/ori_arc/src/aims/lattice/dimensions.rs` (which still hosts the enum after Section 00) as the file Section 02 will modify

---

## 01.2 Lock the Rule 6 decision: does NOT fire on ArgEscaping

**File(s):** Decision document

**Decision (locked):** `AimsState::canonicalize_single_pass` Rule 6 (`HeapEscaping + Unique → MaybeShared`) **does NOT fire on `ArgEscaping + Unique`**. An `ArgEscaping` value with `Uniqueness::Unique` stays `Unique` after canonicalization. The current rule's match arm:

```rust
// Rule 6 (current behavior, preserved unchanged):
if self.locality == Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique {
    self.uniqueness = Uniqueness::MaybeShared;
}
```

uses the specific value `Locality::HeapEscaping` (not a `>=` comparison), so Section 02 does NOT need to modify this rule at all. The new variant is automatically excluded from Rule 6's preconditions.

**Soundness rationale (the load-bearing argument):**

Rule 6 exists because a value reachable from the heap may be aliased there — the heap has potentially infinite, unobservable references. `ArgEscaping` values are categorically different: they flow to a callee but the callee cannot retain them past the call return (by the definition of the new variant). The callee may *temporarily alias* the value during its execution, but that aliasing is bounded by the call's lifetime. After the call returns, the caller's uniqueness is preserved.

If a callee actually does retain a reference (stores it somewhere persistent), the value's locality should be **upgraded** to `HeapEscaping` by the producer, not stay `ArgEscaping`. This is **soundness condition 4** (see 01.4). The producer responsibility lives in `repr-opt §08`, not in this plan.

**Prior art validation:**

- **Go** (`graph.go:199-226`): The `flow()` function only sets `attrEscapes` on values that reach a sink with `attrEscapes`. The `calleeLoc` location does NOT have `attrEscapes` set, so a value flowing only to a callee never gets marked as escaping for stack-promotion purposes. A `Unique` value passed to a callee stays `Unique` from the caller's perspective.
- **Lean 4** (`Borrow.lean:58-60`): Borrowed parameters (`borrow=true`) preserve the caller's ownership across the fixpoint. The narrowing-direction equivalent of "ArgEscaping + Unique stays Unique."
- **Swift**: Doesn't have a `Unique`/`MaybeShared` lattice at the escape level — uses ownership conventions for lifetime safety. Validates the architectural separation but doesn't directly answer the rule-firing question.

**Section 02 implication:** Section 02 must add a **negative pin test** asserting `ArgEscaping + Unique` survives `canonicalize()` unchanged. The pin test is the permanent regression guard against accidentally adding Rule 6 firing on the new variant.

- [ ] Document the Rule 6 decision in the decision document with the soundness rationale
- [ ] Cite the Go and Lean 4 evidence with file paths and line numbers
- [ ] Note that Section 02 has zero work in `canonicalize_single_pass` for Rules 4, 6, 8 — they all use specific variant matching and the new variant slots in transparently
- [ ] Note that Section 02 must add a negative pin test for `ArgEscaping + Unique → Unique`

---

## 01.3 Lock the EscapeInfo API shape

**File(s):** Decision document

**Decision (locked):** The new `compiler/ori_repr/src/escape/mod.rs` (replacing the 12-line ZST placeholder) defines `EscapeInfo` as:

```rust
//! Per-function escape analysis storage.
//!
//! Replaces the placeholder `EscapeInfo` ZST with concrete per-function
//! escape facts. Populated by repr-opt §08's connection-graph escape
//! analysis. Consumed by `ReprPlan::escapes()`, repr-opt §09's sharing
//! bound analysis, and repr-opt §10's thread-locality analysis (which
//! routes through `RcStrategy::NonAtomic`, not a parallel ThreadLocality
//! enum — see plans/repr-opt/00-overview.md:192).

use ori_arc::aims::lattice::Locality;
use ori_arc::ArcVarId; // re-exported from ori_arc::ir, matches plan.rs:17 convention
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscapeInfo {
    /// Per-variable escape scope. Variables not present default to
    /// `Locality::Unknown` (conservative — may escape).
    var_escape: FxHashMap<ArcVarId, Locality>,
}

impl EscapeInfo {
    /// Query the escape scope of a variable. Returns `Unknown` if not set.
    #[must_use]
    pub fn escape_scope(&self, var: ArcVarId) -> Locality {
        self.var_escape.get(&var).copied().unwrap_or(Locality::Unknown)
    }

    /// Whether the variable escapes its function (locality > FunctionLocal).
    /// Used by `ReprPlan::escapes()` query body.
    #[must_use]
    pub fn escapes(&self, var: ArcVarId) -> bool {
        self.escape_scope(var) > Locality::FunctionLocal
    }

    /// Whether the variable is provably non-escaping (locality ≤ FunctionLocal).
    /// Convenience for repr-opt §09's `SharingBound::Unique` check.
    #[must_use]
    pub fn is_non_escaping(&self, var: ArcVarId) -> bool {
        self.escape_scope(var) <= Locality::FunctionLocal
    }

    /// Monotone widening: only widens, never narrows. Prevents accidental
    /// loss of precision in the ordered lattice. Used by repr-opt §08's
    /// dataflow as it discovers escape paths.
    pub fn join_escape_scope(&mut self, var: ArcVarId, scope: Locality) {
        let entry = self.var_escape.entry(var).or_insert(Locality::BlockLocal);
        *entry = (*entry).join(scope);
    }
}
```

**Locked design choices:**

1. **Storage shape**: `FxHashMap<ArcVarId, Locality>` per function. The map lives inside `ReprPlan::escape_info: FxHashMap<Name, EscapeInfo>` (which already exists at `compiler/ori_repr/src/plan.rs:91`). One `EscapeInfo` per function, indexed by variable.
2. **Default value**: `Locality::Unknown` for unset variables. This matches the current `ReprPlan::escapes()` body's `let result = true;` semantics — anything not analyzed is conservatively assumed to escape.
3. **Writer is monotone (`join_escape_scope`)**: The writer never narrows. Codex Step 6B refined this — since the lattice is ordered, the public mutator should not allow accidental precision loss. A write that would *narrow* a variable's locality is silently widened to the join of old and new.
4. **Boolean helpers (`escapes`, `is_non_escaping`) are derived**: Both are 1-line wrappers around `escape_scope`. Their existence aligns the API surface with the existing `ReprPlan::escapes()` query and the `repr-opt §09` `compute_sharing_bound` example at `section-09-arc-header.md:75` (which currently calls `escape_state(alloc) == EscapeState::NoEscape` and will be rewritten to `is_non_escaping(var)`).
5. **Cross-crate import is allowed**: `ori_repr` already depends on `ori_arc` (`plan.rs:21` imports `ArcVarId`). The new imports of `Locality` follow the same dependency direction.

**Locked NON-choices (things this decision deliberately does not cover):**

- **Producer**: The decision says nothing about *how* `EscapeInfo` gets populated. That's `repr-opt §08`'s responsibility.
- **Element type variants**: No `enum EscapeKind` or similar parallel structure. The single `Locality` value is the storage element.
- **Per-allocation vs per-variable**: Per-variable. Per-allocation tracking is a future refinement; v1 maps `ArcVarId → Locality`.

- [ ] Document the locked API shape with the full code block
- [ ] Document the 5 design choices and their reasoning
- [ ] Document the NON-choices with reasoning
- [ ] Note that Section 03 implements this exactly — no further design judgement

---

## 01.4 Document the 5 soundness conditions and their enforcement layers

**File(s):** Decision document

**Decision (locked):** Pass 4 (Go + Lean 4 prior art study) surfaced 5 soundness conditions for the new `ArgEscaping` variant. Each condition has a specific **enforcement layer** that determines who is responsible for ensuring it holds.

| # | Condition | Enforcement layer | Owner |
|---|---|---|---|
| 1 | **Monotonicity in call chains**: if `f` calls `g(x)` and `g` says `x` is `ArgEscaping`, then `f`'s argument must be at least `ArgEscaping` | Automatic via `Locality::join = max`. The interprocedural fixpoint already takes the join of caller-side and callee-side facts | No work needed; this plan does not enforce explicitly because it cannot be violated by construction |
| 2 | **Ownership preservation**: `ArgEscaping + Owned` means *callee borrows, caller retains*. Codegen must NOT transfer ownership into the callee | `repr-opt §08` producer site (`block.rs:97` for cross-block widening, `block.rs:155` for return widening, plus the call-lowering/arg-ownership consumer) | **`repr-opt §08`** — this plan transfers the obligation in Section 04 by adding a "Soundness conditions to enforce" subsection to §08's plan text |
| 3 | **Lifetime bound**: `ArgEscaping` is only sound if the callee's lifetime is bounded by the argument's lifetime at the call site | `MemoryContract` effects path (`may_share`, `may_escape`-derived, contract effects) — verified during interprocedural fixpoint. NOT a Locality concern | This plan does not enforce explicitly; lives in `MemoryContract` machinery |
| 4 | **No heap persistence**: if a parameter is observed to be stored in a global, returned, or written to the heap, its locality must be **upgraded** to `HeapEscaping`, not stay `ArgEscaping` | `repr-opt §08` producer site (the connection-graph analysis must promote on heap-write detection) | **`repr-opt §08`** — transferred in Section 04 alongside condition 2 |
| 5 | **Multi-callee join**: if a value is passed to multiple callees with different localities, take the **max** (least local) | Automatic via `Locality::join = max`. Already handled by the interprocedural fixpoint | No work needed; cannot be violated by construction |

**Key insight from Codex Step 6B Finding 3**: Conditions 2 and 4 are **producer obligations**, not invariants of `Locality::join` or `ParamContract`. Adding `debug_assert!`s in this plan's lattice helpers would either be tautologies (no producer creates the bad state yet — `repr-opt §08` hasn't shipped) or wrong coupling (forcing the lattice to know about producer responsibilities). The right place for the assertions is **at the producer sites in `repr-opt §08`**.

This plan **documents the conditions** and **transfers them to `repr-opt §08`** via Section 04's plan-text update. Codex's exact words: *"make the future enforcement explicit in §08 with concrete tests/asserts at the producer sites `block.rs:97` and `block.rs:155`, plus the call-lowering/arg-ownership consumer."*

- [ ] Document the 5 conditions with the enforcement-layer table
- [ ] Document the rationale for not adding `debug_assert!`s in this plan (Codex Step 6B Finding 3)
- [ ] Note that Section 04 will literally copy conditions 2 and 4 into `repr-opt §08`'s plan text — this plan does not paraphrase, it sources directly
- [ ] Cross-reference the Section 01 + Section 04 interaction in `00-overview.md`

---

## 01.5 Brief Pass 4 prior art citation

**File(s):** Decision document

**Decision (locked):** The decision document includes a **brief** prior art section citing:

- **Go's `leakCallee` semantics**: `golang/src/cmd/compile/internal/escape/leaks.go` (the `[8]uint8` storage) and `graph.go:199-226` (the `flow()` function that demonstrates `calleeLoc` is independent of `attrEscapes`)
- **Lean 4's `borrow` flag**: `lean4/src/Lean/Compiler/IR/Borrow.lean:58-60` (`initBorrow` initializing every parameter as borrowed)
- **Swift's escape destinations**: `swift/SwiftCompilerSources/Sources/Optimizer/FunctionPasses/ComputeEscapeEffects.swift:152-156` (`EscapeDestination` enum)
- **Roc's value semantics**: no escape analysis at all — confirms ArgEscape distinction is only relevant in languages with borrowing
- **Racordon 2019 thesis §6.2.2**: transient ownership at conditional merge points (the same distinction in PhD-thesis form)

**The full inversion explanation between Lean 4's narrowing direction (`borrow=true → false`) and Ori's widening direction (`BlockLocal → Unknown`) does NOT live in this section.** Per Codex Step 6B Finding 4, that explanation belongs in the **source comment** in `compiler/ori_arc/src/aims/lattice/dimensions.rs` near the `Locality` enum, where future contributors will encounter it when reading the actual code.

This section's prior art is a **citation-sized mention** (one paragraph), not an explanatory deep dive. The deep dive lives in dimensions.rs, added by Section 02.

- [ ] Document the brief prior art section with citations
- [ ] Note that the Lean 4 inversion explanation is intentionally *not* here — it lives in dimensions.rs source comment (Section 02 adds it)
- [ ] Note that Pass 4's full Cross-Compiler Verdict matrix (Go/Lean 4/Swift/Roc/Racordon) is preserved in this plan's Phase 2 research notes for future reference, but this section only carries the citation-sized version

---

## 01.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 01.N Completion Checklist

- [ ] Decision document exists (in this section's body or as `compiler/ori_arc/src/aims/lattice/DECISION.md`) covering all 5 subsection outcomes
- [ ] Chain ordering documented with rationale and rejected alternatives (01.1)
- [ ] Rule 6 decision documented with Go and Lean 4 evidence (01.2)
- [ ] EscapeInfo API documented with full code block and locked design choices (01.3)
- [ ] 5 soundness conditions documented with enforcement-layer table (01.4)
- [ ] Brief prior art citation (01.5) — full Lean 4 inversion explanation deferred to Section 02's dimensions.rs comment
- [ ] No code changes made: `git diff --stat compiler/` returns zero modified files (with the possible exception of `DECISION.md` if the implementer chose to create it as a separate file)
- [ ] No tests added or modified
- [ ] Cross-section interaction with Section 04 documented: Section 04 will literally copy conditions 2 and 4 into repr-opt §08's plan text
- [ ] Plan annotation cleanup: this section adds no plan annotations to source code, so `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` should still report 0 references in source files
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] This section's `reviewed` field flipped from `false` to `true` (Section 02 will read this section's locked decisions)
  - [ ] `00-overview.md` Quick Reference table status updated for Section 01
  - [ ] `index.md` Section 01 status updated
- [ ] `/tpr-review` passed — Codex review confirms the decision document is internally consistent and Section 02 can derive its implementation from it without further design judgement
- [ ] `/impl-hygiene-review` passed — verifies no spurious code or test changes were made in this research-only section
- [ ] `/improve-tooling` retrospective completed — for a research-only section, the retrospective may be brief. Reflect on: was there a need for a "decision document template" tool that this and future plan creation runs would benefit from? Was the cross-referencing to `00-overview.md` mission criteria error-prone? Did the implementer have to manually verify the Pass 4 file:line citations are still accurate after Section 00's split? Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. If genuinely no gaps, document briefly: "Retrospective: research-only section, no diagnostic tooling exercised, no gaps."

**Exit Criteria:** A reviewer can read the decision document and answer all 5 of these questions without consulting any other source:
1. What variants does `Locality` have, in what order, and why is `ArgEscaping` between `FunctionLocal` and `HeapEscaping` specifically?
2. Does `canonicalize_single_pass` Rule 6 fire on `ArgEscaping + Unique`, and what is the soundness argument either way?
3. What is the public API of `ori_repr::escape::EscapeInfo`, and what are the signatures of its 4 methods?
4. What are the 5 soundness conditions, and which are enforced by this plan vs by future `repr-opt §08`?
5. Where in Go and Lean 4 are the prior-art validations for the Rule 6 decision?

If a reviewer cannot answer any of these from the decision document alone, this section is not complete and an additional subsection is needed.
