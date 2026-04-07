---
section: "04"
title: "Plan Corpus Coordination"
status: not-started
reviewed: false
goal: "Update the 5 cross-plan coordination targets in the repr-opt plan corpus so they reference the unified ori_arc::Locality type and the new EscapeInfo API instead of inventing parallel enums. Transfer soundness conditions 2 and 4 from this plan's Section 01 into repr-opt §08's plan text as future-producer obligations."
success_criteria:
  - "plans/repr-opt/section-08-escape-analysis.md frontmatter has `depends_on: [\"02\", \"locality-representation-unification\"]` (added the cross-plan reference)"
  - "plans/repr-opt/section-08-escape-analysis.md §08.1 task list rewritten to consume ori_arc::Locality (NOT define EscapeState)"
  - "plans/repr-opt/section-08-escape-analysis.md has a new \"Soundness conditions to enforce\" subsection containing conditions 2 and 4 LITERALLY copied from this plan's Section 01.4"
  - "plans/repr-opt/section-09-arc-header.md compute_sharing_bound() example at line 75 uses EscapeInfo::is_non_escaping(var) instead of EscapeState::NoEscape"
  - "plans/repr-opt/section-10-thread-local-arc.md confirms RcStrategy::NonAtomic routing in section text and notes no parallel ThreadLocality enum"
  - "plans/repr-opt/00-overview.md lines 191-192 (the §08 + §01 and §10 + §01 coupling notes) reference the unified ori_arc::Locality type"
  - "plans/repr-opt/index.md keyword clusters for §08, §09, §10 no longer mention EscapeState or ThreadLocality as standalone enums"
  - "All cross-references in the updated repr-opt plan files resolve to existing symbols (no broken `[link]`s)"
inspired_by:
  - "plans/repr-opt/00-overview.md:191-192 (the canonical statement of where escape and thread-locality storage live)"
  - "Section 01 of THIS plan (the locked decision document — Section 04 literally copies conditions 2 and 4 from there, does not paraphrase)"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Update plans/repr-opt/section-08-escape-analysis.md"
    status: not-started
  - id: "04.2"
    title: "Update plans/repr-opt/section-09-arc-header.md"
    status: not-started
  - id: "04.3"
    title: "Update plans/repr-opt/section-10-thread-local-arc.md"
    status: not-started
  - id: "04.4"
    title: "Update plans/repr-opt/00-overview.md"
    status: not-started
  - id: "04.5"
    title: "Update plans/repr-opt/index.md"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Plan Corpus Coordination

**Status:** Not Started
**Goal:** Update the 5 cross-plan coordination targets in the `repr-opt` plan corpus so they reference the unified `ori_arc::Locality` type and the new `EscapeInfo` API. Transfer soundness conditions 2 and 4 from this plan's Section 01 into `repr-opt §08`'s plan text as future-producer obligations. After this section completes, no `repr-opt` section will define a parallel `EscapeState` or `ThreadLocality` enum — they all consume the unified type from `ori_arc`.

**Success Criteria:**

- [ ] `plans/repr-opt/section-08-escape-analysis.md` frontmatter has `depends_on: ["02", "locality-representation-unification"]` (added the cross-plan reference)
- [ ] `plans/repr-opt/section-08-escape-analysis.md` §08.1 task list **rewritten** to consume `ori_arc::Locality` (NOT define `EscapeState`). The current §08.1 says: *"Define escape lattice: `pub enum EscapeState { NoEscape, ArgEscape, GlobalEscape }`"*. The replacement says: *"Reuse `ori_arc::aims::lattice::Locality` (provided by the `locality-representation-unification` plan). Do NOT define a parallel enum."*
- [ ] `plans/repr-opt/section-08-escape-analysis.md` has a new **"Soundness conditions to enforce"** subsection (likely §08.6 or §08.7) containing conditions 2 (ownership preservation) and 4 (no heap persistence) **literally copied** from this plan's Section 01.4 — not paraphrased
- [ ] `plans/repr-opt/section-09-arc-header.md` `compute_sharing_bound()` example at the location currently around line 75 uses `EscapeInfo::is_non_escaping(var)` instead of `escape_info.escape_state(alloc) == EscapeState::NoEscape`
- [ ] `plans/repr-opt/section-09-arc-header.md` frontmatter has `depends_on` updated to include `"locality-representation-unification"` (or `"02"` of this plan, depending on convention)
- [ ] `plans/repr-opt/section-10-thread-local-arc.md` section text (in the body, not just frontmatter) explicitly **confirms** that thread-locality routes through `RcStrategy::NonAtomic` per `repr-opt/00-overview.md:192`, and **notes that no parallel `ThreadLocality` enum is defined**. The frontmatter `depends_on` may include this plan if §10 reads `EscapeInfo` for any input signal — verify by re-reading §10's task list
- [ ] `plans/repr-opt/00-overview.md` lines 191-192 (the §08+§01 and §10+§01 coupling notes) reference the unified `ori_arc::Locality` type — they likely already do (`§10 + §01: thread-locality via RcStrategy` is already correct), but verify and adjust phrasing if it implies a separate `EscapeState`
- [ ] `plans/repr-opt/index.md` keyword clusters for §08, §09, §10 no longer mention `EscapeState` or `ThreadLocality` as standalone enums. The keywords for §08 should reference `ori_arc::Locality` and `EscapeInfo`; the keywords for §09 should reference `EscapeInfo::is_non_escaping`; the keywords for §10 should reference `RcStrategy::NonAtomic` (already there, but verify)
- [ ] All cross-references in the updated `repr-opt` plan files resolve to existing symbols. Run a manual scan for `EscapeState` in `plans/repr-opt/` and verify zero matches after this section completes
- [ ] Connects upward to mission criteria: "repr-opt §08 plan updated", "repr-opt §09 plan updated", "repr-opt §10 plan updated", "repr-opt 00-overview.md lines 191-192 reference unified Locality", "repr-opt index.md keyword clusters cleaned up"

**Context:** This section performs **plan-text edits only** — no code changes. The work is mechanical but requires careful attention to cross-references: when the `repr-opt §08` plan text says "define `EscapeState`," that text becomes a stale instruction once Section 02 has added the variant to `ori_arc::Locality`. The implementer of `§08` (whenever they pick up the work) would either define a parallel enum (the LEAK this plan is preventing) or get confused by the contradictory direction.

The fix is to **rewrite the affected paragraphs in `repr-opt`'s plan files** to point at the new SSOT. This is one of the rare cases where this plan modifies files outside its own directory — it's necessary because plan-text references are part of the cross-plan contract.

### Re-read-before-edit protocol (MANDATORY for every Section 04 subsection)

This section was authored against a snapshot of `plans/repr-opt/`. Between the time this plan was written and the time Section 04 actually runs, another contributor may have partially advanced `repr-opt §08`, `§09`, or `§10` — adding new task items, restructuring subsections, fixing typos, updating line numbers, or amending the enum definitions. If Section 04 blindly applies its planned edits against the assumed-snapshot text, it will:

1. **Silently clobber concurrent contributor work** — the diff looks tidy, files compile, but newly-added intent is overwritten with no warning.
2. **Fail to update sites that shifted** — line numbers like "around line 75" or "lines 188-200" are advisory; if §09 grew by 30 lines, the example moved.
3. **Reference stale anchors** — if a contributor renamed `escape_state` to `escape_kind` in a partial commit, every planned `EscapeState::NoEscape` → `is_non_escaping(var)` rewrite must be re-derived under the new name.

**Therefore, every Section 04 subsection (04.1 through 04.5) MUST follow this protocol:**

**Step A — Re-read the current state.** Before applying ANY edit, read the target file end-to-end (`Read plans/repr-opt/section-NN-*.md`) and compare against the assumptions in this plan. Specifically check:
- Does the file still contain the strings/symbols this plan expects to rewrite (`EscapeState`, `escape_state(alloc)`, `EscapeState::NoEscape`, `pub enum EscapeState`, etc.)?
- Have the line numbers shifted? (Almost always yes — re-derive them via grep before editing.)
- Have any new subsections been added? Have any been renumbered?
- Does the frontmatter `depends_on` already include `locality-representation-unification` (added by some other workflow)?
- Has any prior contributor work introduced `Locality` references that conflict with or duplicate this plan's edits?

**Step B — Derive update RULES, not literal edits.** Translate this plan's intended edits into a set of regex/replace rules that operate on the *current* text, not the snapshot text:
- "Replace any `EscapeState::NoEscape` with `EscapeInfo::is_non_escaping(var)`"
- "Replace any `escape_state(<expr>) == EscapeState::NoEscape` with `escape_info.is_non_escaping(<expr>)`"
- "Delete any `pub enum EscapeState { ... }` block entirely"
- "Insert the soundness-conditions subsection after the existing last numbered subsection (whatever number that is now)"
- Apply each rule by reading the current text, finding matches, and editing in place. If a rule has zero matches because the contributor already removed the symbol, that is GOOD news — record it in the completion notes and move on. If a rule has matches in unexpected locations, STOP and re-derive.

**Step C — Preserve concurrent contributor work.** Any text in the target file that does NOT match this plan's update rules is concurrent contributor work — leave it alone. Do not delete, restructure, or "clean up" anything outside the rules. The rule is: this plan owns the unification of `EscapeState`/`ThreadLocality` references; it does NOT own §08's analysis algorithm, §09's compute_sharing_bound signature, or §10's strategy table — those are §08/§09/§10's authors' work.

**Step D — Verify the diff before commit.** Before committing the edit, run `git diff plans/repr-opt/section-NN-*.md` and confirm the diff:
- Only removes parallel-enum references (`EscapeState`, standalone `ThreadLocality`)
- Only adds unified `Locality` / `EscapeInfo` references
- Adds the soundness-conditions subsection (only in 04.1)
- Adds frontmatter `depends_on` entries
- Adds `<!-- resolves: ... -->` comment markers
- **Does NOT** delete unrelated paragraphs, change unrelated method signatures, or restructure unrelated subsections.

If the diff shows any unrelated removals, the contributor's newer work was accidentally clobbered — STOP, restore the file (`git checkout plans/repr-opt/section-NN-*.md`), and re-derive the edits from the current text. NEVER force the edit through.

**Step E — Document the actual state vs assumed state.** In the section's completion notes, record any divergence between the assumed snapshot and the actual current state. Examples:
- "§08 had been partially advanced — `pub enum EscapeState` was already moved from line 70 to line 84. Re-derived line numbers from grep before edit."
- "§09 already had `depends_on` updated to include `locality-representation-unification` by another workflow. Skipped the frontmatter add; verified the existing entry."
- "§10 had a new `ThreadStrategy` enum added by a contributor (NOT a parallel `ThreadLocality` — different concept). Left it untouched; no conflict."

This protocol prevents the most subtle failure mode of any plan that modifies files outside its own directory. It is **non-negotiable** — every subsection must include Step A as its first task and Step D as its final task.

Per Codex Step 6B Finding 2 + the cross-section interaction documented in `00-overview.md:278` (Section 01 + Section 04): *"Section 04's `repr-opt §08` plan text update adds a 'Soundness conditions to enforce' subsection containing conditions 2 (ownership preservation) and 4 (no heap persistence). These conditions are sourced from Section 01's locked decision document, not restated independently. Section 04 must literally copy the condition text from Section 01, not paraphrase."*

This means Section 04 reads Section 01.4's enforcement-layer table and **copies the rows for conditions 2 and 4 verbatim** into the new `repr-opt §08` subsection. If Section 01.4 says "Ownership preservation: `ArgEscaping + Owned` means callee borrows, caller retains. Codegen must NOT transfer ownership into the callee," that exact sentence appears in `repr-opt §08`'s new subsection. No rephrasing.

**Reference implementations:**

- **`plans/repr-opt/00-overview.md:191-192`** (the canonical cross-section coupling statements):
  - `§08 + §01 (set_escape_info writer + escapes query body)`: defines the existing contract for how `§08` interacts with `ReprPlan::escape_info`
  - `§10 + §01 (thread-locality via RcStrategy)`: defines that thread-locality routes through `RcStrategy::NonAtomic`, NOT a separate enum
  These two lines are the load-bearing reference. After Section 04, both should be consistent with the unified `Locality`.
- **Section 01.4 of this plan**: the enforcement-layer table for the 5 soundness conditions. Section 04 reads this and copies rows 2 and 4 into `repr-opt §08`.
- **`compiler/ori_repr/src/escape/mod.rs`** (post-Section-03): the new `EscapeInfo` API. The `repr-opt §08` plan text references this file path and the public methods (`escape_scope`, `join_escape_scope`).

**Depends on:** Section 03 (the new `EscapeInfo` API must exist as code before the plan text references it; otherwise the plan text has dangling references to nonexistent code).

---

## 04.1 Update plans/repr-opt/section-08-escape-analysis.md

**File(s):** `plans/repr-opt/section-08-escape-analysis.md`

**Context:** This is the largest cross-plan edit. The current `§08.1` task list (per Pass 1 Agent 2 + Phase 2 verification) says:

> **File(s):** `compiler/ori_repr/src/escape/mod.rs`, `compiler/ori_repr/src/escape/intraprocedural.rs`
>
> Define escape lattice:
> ```rust
> pub enum EscapeState { NoEscape, ArgEscape, GlobalEscape }
> ```

After this plan's Section 03, `compiler/ori_repr/src/escape/mod.rs` no longer needs an `EscapeState` definition — it consumes `ori_arc::aims::lattice::Locality` directly. The §08.1 instructions are now stale and would cause confusion if not updated.

The replacement strategy:

1. Update §08's frontmatter `depends_on` list
2. Rewrite §08.1's "Define escape lattice" task to "Reuse the unified Locality type"
3. Update §08.1's file list to reference `escape/intraprocedural.rs` (the new file §08 will create) but NOT `escape/mod.rs` (which is now Section 03's responsibility — §08 only populates the storage, doesn't define it)
4. Add a new "Soundness conditions to enforce" subsection (call it §08.7 or §08.6 — pick the next available number that doesn't collide with the existing subsection list) containing conditions 2 and 4 from this plan's Section 01.4

- [ ] **Step A (re-read-before-edit, MANDATORY — see Section 04 context block).** Re-read the current `plans/repr-opt/section-08-escape-analysis.md` end-to-end. Note the current subsection numbering, the current frontmatter, the current §08.1 content, and the current line numbers of every `EscapeState` reference. Compare against the assumptions in this plan (e.g., "§08.1 currently says `pub enum EscapeState { NoEscape, ArgEscape, GlobalEscape }`" — does it still?). If the file has been partially advanced by another contributor since this plan was created, STOP and re-derive update rules from the actual current text. Record any divergence in the completion notes.

- [ ] Update the frontmatter `depends_on` field. The current value is `depends_on: ["02"]` (where `"02"` refers to `repr-opt`'s own Section 02 — Transitive Triviality — NOT to this plan's Section 02). Change to:
  ```yaml
  depends_on: ["02", "locality-representation-unification"]
  ```
  Disambiguation: the `"02"` entry is `repr-opt`'s Section 02 (an intra-plan dependency, unchanged). The `"locality-representation-unification"` entry is this plan as a whole (a cross-plan dependency, the new addition). If `repr-opt`'s convention for cross-plan dependencies uses a different format (e.g., `"plans/locality-representation-unification/"` or section-specific references), adopt whichever convention the existing `repr-opt` plan corpus already uses — re-read `plans/repr-opt/00-overview.md` and other section files to find an existing cross-plan dependency for the format reference.

- [ ] Update the frontmatter `inspired_by` if it currently lists "Define `EscapeState` enum" or similar. Add:
  ```yaml
  inspired_by:
    - "..."  # existing entries
    - "ori_arc::aims::lattice::Locality (provided by locality-representation-unification plan)"
  ```

- [ ] Rewrite §08.1's "Define escape lattice" task. The current task list says (approximately):
  > Define escape lattice:
  > ```rust
  > pub enum EscapeState { NoEscape, ArgEscape, GlobalEscape }
  > ```
  > Implement connection graph: ...

  Replace with:
  > **Reuse the unified `ori_arc::aims::lattice::Locality` type. Do NOT define a parallel enum.**
  >
  > The locality lattice was unified by the `locality-representation-unification` plan. The 5 variants `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown` form the SSOT for escape classification across the compiler. The semantic mapping is:
  > - `EscapeState::NoEscape` ≈ `Locality::BlockLocal | Locality::FunctionLocal` (use `EscapeInfo::is_non_escaping(var)`)
  > - `EscapeState::ArgEscape` → `Locality::ArgEscaping` (the new variant; produce it when a value flows to a callee but is not retained)
  > - `EscapeState::GlobalEscape` → `Locality::HeapEscaping` (existing variant; semantics fit)
  >
  > §08's storage target is `compiler/ori_repr/src/escape/intraprocedural.rs` (a new file this section creates) writing into the existing `EscapeInfo` storage at `compiler/ori_repr/src/escape/mod.rs` (defined by `locality-representation-unification` Section 03). Use `EscapeInfo::join_escape_scope(var, scope)` for monotone widening.

- [ ] Update the §08.1 connection graph implementation tasks. The task list previously said `escape_state(alloc) == EscapeState::NoEscape` or similar. Update the references to use `EscapeInfo::escape_scope(var)` and `EscapeInfo::is_non_escaping(var)` instead.

- [ ] Find the next available subsection number (probably §08.6 or §08.7 — re-read the existing subsection list to confirm) and add a new subsection titled **"Soundness conditions to enforce"**:
  ```markdown
  ## 08.7 Soundness conditions to enforce

  **Source:** This subsection is sourced literally from `plans/locality-representation-unification/section-01-representation-decision.md` §01.4 (the enforcement-layer table for the 5 soundness conditions for the `ArgEscaping` variant). The text is **literally copied**, not paraphrased — if the upstream decision document changes, this subsection must be re-synced.

  The `locality-representation-unification` plan documented 5 soundness conditions for the `ArgEscaping` variant. Three of them (1, 3, 5) are enforced automatically or by `MemoryContract`. **Two of them (2 and 4) are producer obligations that this section (`repr-opt §08`) must enforce** at the producer sites in `compiler/ori_arc/src/aims/intraprocedural/block.rs` (lines 97 and 155 per Phase 2 research) and the call-lowering/arg-ownership consumer.

  ### Condition 2: Ownership preservation

  **Statement (from `locality-representation-unification` §01.4):** `ArgEscaping + Owned` means *callee borrows, caller retains*. Codegen must NOT transfer ownership into the callee.

  **Enforcement:** When `§08`'s connection-graph analysis observes a value flowing to a callee parameter (without being stored, returned, or written to the heap), it sets the value's locality to `ArgEscaping`. If the value's `AccessClass` is `Owned`, codegen must continue to treat it as borrowed-by-the-callee, NOT moved-into-the-callee. The `MemoryContract::ParamContract` for the callee must reflect this — the parameter's `access` is `Borrowed`, not `Owned`.

  **Test obligation:** Add a test asserting that a unique owned value passed to a callee that takes it by reference and does not retain it has locality `ArgEscaping` AND `AccessClass::Borrowed` after the call site. This is a producer-site test, not a lattice-level test.

  ### Condition 4: No heap persistence

  **Statement (from `locality-representation-unification` §01.4):** If a parameter is observed to be stored in a global, returned, or written to the heap, its locality must be **upgraded** to `HeapEscaping`, not stay `ArgEscaping`.

  **Enforcement:** When `§08`'s connection-graph analysis discovers that a value initially classified as `ArgEscaping` is later observed flowing to a heap-write operation (`Set { obj: heap_root, ... }`), a return value, or a global, it must call `EscapeInfo::join_escape_scope(var, Locality::HeapEscaping)` to widen the locality. Because `join_escape_scope` is monotone (only widens), this operation correctly upgrades the value without risking accidental narrowing.

  **Test obligation:** Add a test asserting that a value initially observed flowing to a callee (`ArgEscaping`) and later observed flowing to a heap-write has final locality `HeapEscaping`, NOT `ArgEscaping`. This is a regression guard against analysis ordering bugs.

  ### Test obligations summary

  Both conditions translate into producer-site tests at the §08 connection-graph analysis layer. They are NOT enforced by `debug_assert!` in the lattice helpers — per `locality-representation-unification` §01.4 and Codex Step 6B Finding 3, asserting them at the lattice level would be either a tautology (no producer creates the bad state until §08 ships) or wrong coupling (forcing the lattice to know about producer responsibilities).

  Instead, §08 adds the assertions at the producer sites:
  - `compiler/ori_arc/src/aims/intraprocedural/block.rs:97` (cross-block widening producer)
  - `compiler/ori_arc/src/aims/intraprocedural/block.rs:155` (return widening producer)
  - The call-lowering/arg-ownership consumer (the exact file location is §08's responsibility to discover during its connection-graph implementation; this plan deliberately does not pin it because the producer site is §08's choice, not a constraint imposed by the unified Locality)

  These are §08's responsibility, not `locality-representation-unification`'s.
  ```

- [ ] Add a `<!-- resolves: locality-representation-unification §04.1 -->` comment marker at the top of §08.1's task list, indicating that this content was rewritten by the `locality-representation-unification` plan.

- [ ] Verify the updated §08 file is internally consistent. Run a manual cross-reference scan: search for `EscapeState` in the file and confirm zero matches; search for `Locality::ArgEscaping` and confirm at least one match; search for `EscapeInfo::join_escape_scope` and confirm at least one match.

- [ ] **Update every reference to `EscapeState` in §08, not just §08.1's task list.** Per accuracy review, the current `section-08-escape-analysis.md` has at least 4 distinct `EscapeState` references:

  - Line ~70: `pub enum EscapeState { NoEscape, ArgEscape, GlobalEscape }` — DELETE this enum definition entirely (the unified `Locality` is the SSOT)
  - Line ~91: `Alloc { id: AllocId, escape: EscapeState }` — change `EscapeState` to `Locality`
  - Line ~93: `Param { index: usize, escape: EscapeState }` — change `EscapeState` to `Locality`
  - Line ~163: `pub param_escapes: Vec<EscapeState>` — change to `pub param_escapes: Vec<Locality>` or restructure as `EscapeInfo` field
  - Add an `inspired_by` link to `compiler/ori_arc/src/aims/lattice/dimensions.rs` (where the unified `Locality` lives).
  - Run `grep -n EscapeState plans/repr-opt/section-08-escape-analysis.md` after the rewrite and verify zero matches.

- [ ] **Step D (verify diff before commit, MANDATORY — see Section 04 context block).** Run `git diff plans/repr-opt/section-08-escape-analysis.md` and confirm the diff only: (a) removes `EscapeState` enum definitions and references, (b) adds `Locality`/`EscapeInfo` references, (c) adds the new "Soundness conditions to enforce" subsection, (d) adds the `<!-- resolves: ... -->` marker, (e) adds the cross-plan `depends_on` entry. The diff MUST NOT delete any unrelated paragraphs (§08's analysis algorithm prose, other subsections, frontmatter fields outside `depends_on`/`inspired_by`). If the diff shows ANY unrelated removals, the contributor's newer work was clobbered — STOP, run `git checkout plans/repr-opt/section-08-escape-analysis.md`, and re-derive the update rules against the current text. Record the actual-vs-assumed divergence in the completion notes.

---

## 04.2 Update plans/repr-opt/section-09-arc-header.md

**File(s):** `plans/repr-opt/section-09-arc-header.md`

**Context:** Pass 1 Agent 1 found that `§09:71-77` (the `compute_sharing_bound()` example) currently reads:

```rust
pub fn compute_sharing_bound(
    alloc: AllocId,
    arc_func: &ArcFunction,
    escape_info: &EscapeInfo,
    loop_info: &LoopInfo,
) -> SharingBound {
    // If value doesn't escape → Unique (refcount always 1)
    if escape_info.escape_state(alloc) == EscapeState::NoEscape {
        return SharingBound::Unique;
    }
    // ...
}
```

After this plan's Section 03, `EscapeInfo` no longer has an `escape_state` method or an `EscapeState` enum. The replacement uses `EscapeInfo::is_non_escaping(var)`. The example code in §09 must be updated to match.

Note also that `EscapeInfo` is now keyed by `ArcVarId` (per Section 01.3 + Section 03.1), not by `AllocId`. The §09 example uses `AllocId` — the implementer of §09 will need to either: (a) translate from `AllocId` to `ArcVarId` at the call site, or (b) extend `EscapeInfo` to support per-allocation tracking. **This decision is §09's responsibility, not this plan's.** This subsection just updates the example to use the post-unification API; whether the API takes `ArcVarId` or `AllocId` is §09's call.

- [ ] **Step A (re-read-before-edit, MANDATORY — see Section 04 context block).** Re-read `plans/repr-opt/section-09-arc-header.md` end-to-end. Find the current `compute_sharing_bound()` example and re-derive its line numbers via `grep -n compute_sharing_bound plans/repr-opt/section-09-arc-header.md` (Pass 1 said around line 75 but it may have shifted). Run `grep -n EscapeState plans/repr-opt/section-09-arc-header.md` to find ALL `EscapeState` references — if the count differs from this plan's enumerated 3 sites (lines 75, 108, 117), re-derive the rewrite list against the actual current text. If a contributor has already partially advanced §09 (e.g., renamed `escape_state` to something else), STOP and re-derive. Record any divergence.

- [ ] Update the example code to use `EscapeInfo::is_non_escaping`:
  ```rust
  pub fn compute_sharing_bound(
      alloc: AllocId,
      arc_func: &ArcFunction,
      escape_info: &EscapeInfo,
      loop_info: &LoopInfo,
  ) -> SharingBound {
      // If value doesn't escape its function → Unique (refcount always 1).
      // EscapeInfo is the per-function escape storage from
      // locality-representation-unification (replaces the placeholder ZST).
      // Note: The example uses ArcVarId for EscapeInfo lookup. §09 must
      // decide whether to translate from AllocId to ArcVarId at this call
      // site OR extend EscapeInfo to support per-allocation tracking.
      let var = alloc_to_var(alloc, arc_func); // §09's translation function
      if escape_info.is_non_escaping(var) {
          return SharingBound::Unique;
      }
      // ...
  }
  ```

- [ ] Update the §09 frontmatter `depends_on` to include `locality-representation-unification` (or `"02"` if the plan corpus uses section numbers — match repr-opt's convention).

- [ ] Add a `<!-- resolves: locality-representation-unification §04.2 -->` comment marker near the updated example.

- [ ] Search the entire `section-09-arc-header.md` file for any remaining references to `EscapeState`, `EscapeState::NoEscape`, `EscapeState::ArgEscape`, or `EscapeState::GlobalEscape`. Per accuracy review, there are AT LEAST 3 sites in §09 (lines 75, 108, 117) that reference `EscapeState`, not just the line-75 example. Update **every** site:

  - Line 75: `escape_info.escape_state(alloc) == EscapeState::NoEscape` → `escape_info.is_non_escaping(var)`
  - Line 108: `escape_info.escape_state(alloc) == EscapeState::GlobalEscape` → `escape_info.escape_scope(var) == Locality::HeapEscaping`
  - Line 117: `EscapeState::ArgEscape` → `Locality::ArgEscaping` (in whatever context the constant appears)
  - Any additional sites surfaced by `grep -n EscapeState plans/repr-opt/section-09-arc-header.md` after the edit.

- [ ] **Step D (verify diff before commit, MANDATORY — see Section 04 context block).** Run `git diff plans/repr-opt/section-09-arc-header.md` and confirm the diff only: (a) replaces `EscapeState`/`escape_state` references with the unified `Locality`/`EscapeInfo` API, (b) updates the `compute_sharing_bound()` example, (c) adds the cross-plan `depends_on` entry, (d) adds the `<!-- resolves: ... -->` marker. The diff MUST NOT delete any unrelated paragraphs (§09's allocation header layout discussion, the `SharingBound` definition, other subsections). If the diff shows ANY unrelated removals, STOP, run `git checkout plans/repr-opt/section-09-arc-header.md`, and re-derive against the current text.

---

## 04.3 Update plans/repr-opt/section-10-thread-local-arc.md

**File(s):** `plans/repr-opt/section-10-thread-local-arc.md`

**Context:** Per Codex round 1 Finding (verified in `repr-opt/00-overview.md:192`): *"§10's thread-locality analysis result is expressed through `RcStrategy::NonAtomic` (vs `Atomic`). There is no separate `ThreadLocality` storage field — the RC strategy IS the thread-locality decision."*

This means **§10 was never going to define a parallel `ThreadLocality` enum**. The mission's "thread-sharing stays in `RcStrategy::NonAtomic`" boundary is already correct in the existing plan corpus. Section 04.3's job is to:

1. Confirm this is documented in §10's section text (not just in the overview)
2. Add `depends_on: ["locality-representation-unification"]` ONLY if §10 reads `EscapeInfo` for any input signal (likely yes, since §10 cares whether a value escapes the thread)
3. Add a brief note in §10's text explicitly acknowledging the unified `Locality` type for any cross-references

- [ ] **Step A (re-read-before-edit, MANDATORY — see Section 04 context block).** Re-read `plans/repr-opt/section-10-thread-local-arc.md` end-to-end. Note the current frontmatter, subsection numbering, and the current line numbers of any `RcStrategy::NonAtomic` / `ThreadLocality` / `EscapeInfo` references via `grep -n 'RcStrategy\|ThreadLocality\|EscapeInfo' plans/repr-opt/section-10-thread-local-arc.md`. Compare against the assumptions in this plan: (a) §10's body should already route thread-locality through `RcStrategy::NonAtomic`, (b) §10 should NOT contain a standalone `ThreadLocality` enum, (c) §10 may or may not already reference `EscapeInfo` as an input signal. If the file has been partially advanced by another contributor since this plan was created (e.g., a parallel `ThreadLocality` enum was introduced, or the `RcStrategy::NonAtomic` routing was renamed), STOP and re-derive update rules from the actual current text. Record any divergence in the completion notes.

- [ ] Re-read `plans/repr-opt/section-10-thread-local-arc.md` to find:
  - Whether the section text explicitly mentions `RcStrategy::NonAtomic` as the storage path
  - Whether there is any reference to `ThreadLocality` as a standalone enum (there should NOT be, but verify)
  - Whether §10 reads `EscapeInfo` for any input signal

- [ ] **If the section text already correctly documents the `RcStrategy::NonAtomic` routing**: Add a **confirmation note** explicitly cross-referencing `repr-opt/00-overview.md:192` and this plan's `00-overview.md` Section 04.3. The note prevents future confusion if anyone tries to reintroduce a parallel `ThreadLocality` enum:
  > **No parallel ThreadLocality enum.** Per `plans/repr-opt/00-overview.md:192` (the §10 + §01 coupling note) and `plans/locality-representation-unification/00-overview.md` Section 04.3, thread-locality is **expressed through `RcStrategy::NonAtomic`**, not a separate storage axis. §10 writes its analysis result via `set_rc_strategy(idx, NonAtomic { width }, ThreadLocal)` and codegen reads via `rc_strategy(idx)`. There is no `ThreadLocality` enum in either `ori_arc` or `ori_repr`. The unified `ori_arc::aims::lattice::Locality` (with the `ArgEscaping` variant added by `locality-representation-unification`) covers escape distance only — thread sharing is a separate concern routed through `RcStrategy`.

- [ ] **If §10 reads `EscapeInfo` for any input signal** (e.g., "use escape analysis to detect values that don't cross thread boundaries"): add `depends_on` to point at `locality-representation-unification`. Otherwise, no `depends_on` change is needed because §10 is independent of the unification.

- [ ] **If the section text contains stale references to a parallel `ThreadLocality` enum**: rewrite to remove them. The replacement uses `RcStrategy::NonAtomic` and the `set_rc_strategy(idx, NonAtomic { ... }, ThreadLocal)` call.

- [ ] Add a `<!-- resolves: locality-representation-unification §04.3 -->` comment marker near the confirmation note.

---

## 04.4 Update plans/repr-opt/00-overview.md

**File(s):** `plans/repr-opt/00-overview.md` (specifically the §08 + §01 and §10 + §01 coupling notes around lines 191-192)

**Context:** Pass 2 verified that lines 191-192 currently read:

> **§08 + §01 (set_escape_info writer + escapes query body)**: §08 computes `EscapeInfo` per function and needs to store it in `ReprPlan::escape_info`. §01.2 has the field, and `set_escape_info(func, info)` is a `pub` writer. However, §08 must ALSO update the `escapes()` query body in `plan/query.rs` — it currently hardcodes `true` (safe default: "everything escapes") and does not consult `escape_info`. §08 must replace the hardcoded `true` with an actual lookup into `self.escape_info` for the given function and variable. Without both the writer AND the query body update, §09 always sees `escapes = true` and cannot compress headers.

> **§10 + §01 (thread-locality via RcStrategy)**: §10's thread-locality analysis result is expressed through `RcStrategy::NonAtomic` (vs `Atomic`). There is no separate `ThreadLocality` storage field — the RC strategy IS the thread-locality decision. ...

**The §10 + §01 line is already correct** — it's the canonical statement that thread-locality routes through `RcStrategy::NonAtomic`. No change needed there.

**The §08 + §01 line is partially stale**: it says "§08 must ALSO update the `escapes()` query body" — but per this plan's Section 03, the query body is replaced by `locality-representation-unification`, not by §08. The line must be updated to reflect that §08 only populates the storage, while the query body plumbing has already been done.

- [ ] **Step A (re-read-before-edit, MANDATORY — see Section 04 context block).** Re-read `plans/repr-opt/00-overview.md` end-to-end. Re-derive the current line numbers for the §08 + §01 and §10 + §01 coupling notes via `grep -n '§08 + §01\|§10 + §01' plans/repr-opt/00-overview.md` (Pass 2 said lines 191-192 but they may have shifted). Verify the §08 + §01 note still contains the "must ALSO update the `escapes()` query body" language this subsection intends to rewrite, and the §10 + §01 note still says "thread-locality via RcStrategy" (this plan expects it to already be correct). Run `grep -n 'EscapeState\|ThreadLocality' plans/repr-opt/00-overview.md` — if any matches appear beyond the two expected coupling-note regions, surface them for re-derivation. If the file has been partially advanced by another contributor since this plan was created (e.g., the coupling notes were already rewritten, or new coupling notes were added that reference `EscapeInfo`), STOP and re-derive update rules from the actual current text. Record any divergence in the completion notes.

- [ ] Re-read `plans/repr-opt/00-overview.md` lines 188-200 to find the current §08 + §01 coupling note. Confirm the line numbers — they may have shifted slightly.

- [ ] Update the §08 + §01 line to reflect the new ownership split:
  > **§08 + §01 (set_escape_info writer)**: §08 computes per-function escape facts and stores them in `ReprPlan::escape_info` via `set_escape_info(func, info)`. The `EscapeInfo` storage shape and the `ReprPlan::escapes()` query body are now defined by the `locality-representation-unification` plan (which replaced the placeholder ZST and the hardcoded `true`). §08's responsibility is the connection-graph analysis that POPULATES `EscapeInfo`; the storage and query plumbing already exist. §08 uses `EscapeInfo::join_escape_scope(var, scope)` for monotone widening and reads back via `EscapeInfo::escape_scope(var)` / `is_non_escaping(var)` / `escapes(var)`. The unified `ori_arc::aims::lattice::Locality` type (5 variants: `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown`) is the SSOT for escape classification — §08 must NOT define a parallel `EscapeState` enum.

- [ ] Verify the §10 + §01 line (the "thread-locality via RcStrategy" line) is unchanged and still correct. If it has drifted, restore it to its canonical form per `repr-opt/00-overview.md:192`.

- [ ] Add a `<!-- resolves: locality-representation-unification §04.4 -->` comment marker near the updated §08 line.

---

## 04.5 Update plans/repr-opt/index.md

**File(s):** `plans/repr-opt/index.md`

**Context:** The keyword clusters in `index.md` are search aids — they help future contributors find the right section by keyword. After this plan's Section 02 + 03, the keywords for §08, §09, §10 should reference `ori_arc::Locality` and `EscapeInfo` instead of `EscapeState` and `ThreadLocality` (which never existed but might be in the keywords as forward-looking hints).

Per Pass 1 Agent 1 + Phase 2 verification, the current `index.md` keyword clusters for §08 likely include `EscapeState` as a forward-looking keyword. Section 04.5 removes those and replaces with the post-unification API names.

- [ ] **Step A (re-read-before-edit, MANDATORY — see Section 04 context block).** Re-read `plans/repr-opt/index.md` end-to-end. Locate the current §08, §09, and §10 keyword cluster blocks (their exact format and anchor structure depends on repr-opt's index.md conventions — do NOT assume a specific heading or bullet format). Run `grep -n 'EscapeState\|escape_state\|NoEscape\|ArgEscape\|GlobalEscape\|ThreadLocality' plans/repr-opt/index.md` to enumerate every stale-keyword site; compare against this plan's assumed rewrite targets (§08 cluster likely contains `EscapeState`/`escape_state`/`NoEscape`/`ArgEscape`/`GlobalEscape`; §09 cluster may contain `EscapeState`; §10 cluster should NOT contain `ThreadLocality` but verify). If the file has been partially advanced by another contributor since this plan was created (e.g., clusters already cleaned up, or new clusters added that already reference `Locality`/`EscapeInfo`), STOP and re-derive update rules from the actual current text — skip any rewrite whose target no longer matches, record it in completion notes. If a contributor added entirely new keyword entries inside §08/§09/§10 clusters, leave them alone (they are concurrent work; only touch stale-enum entries). Record any divergence in the completion notes.

- [ ] Re-read `plans/repr-opt/index.md` and find the keyword clusters for §08, §09, §10.

- [ ] **§08 keyword cluster**: Search for `EscapeState`, `escape_state`, `NoEscape`, `ArgEscape`, `GlobalEscape`. Replace each with the post-unification equivalent:
  - `EscapeState` → `EscapeInfo` (the storage type) and `Locality` (the value type)
  - `escape_state` → `escape_scope` (the query method)
  - `NoEscape` → `is_non_escaping` or `BlockLocal | FunctionLocal`
  - `ArgEscape` → `ArgEscaping` (the unified variant name)
  - `GlobalEscape` → `HeapEscaping` (the unified variant name)
  Add new keywords: `Locality::ArgEscaping`, `EscapeInfo::join_escape_scope`, `is_non_escaping`, `locality-representation-unification`.

- [ ] **§09 keyword cluster**: Look for any mention of `EscapeState` in the §09 cluster. Replace with `EscapeInfo`. Add `is_non_escaping` and `locality-representation-unification` if not already present.

- [ ] **§10 keyword cluster**: Look for any mention of `ThreadLocality` as a standalone enum. The cluster should already say `RcStrategy::NonAtomic`, but verify. Add `locality-representation-unification` to the cluster only if §10 has a `depends_on` to this plan (per 04.3's decision).

- [ ] After the updates, verify by `grep -n 'EscapeState\|ThreadLocality' plans/repr-opt/index.md` that no standalone-enum names remain (except perhaps in comments explaining "no parallel enum, see locality-representation-unification").

- [ ] Add a `<!-- resolves: locality-representation-unification §04.5 -->` comment near the modified clusters.

---

## 04.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers. -->

- None.

---

## 04.N Completion Checklist

- [ ] `plans/repr-opt/section-08-escape-analysis.md` frontmatter `depends_on` includes `"locality-representation-unification"` (or its equivalent in the repr-opt convention)
- [ ] `plans/repr-opt/section-08-escape-analysis.md` §08.1 task list rewritten to consume `ori_arc::Locality`, NOT define `EscapeState`
- [ ] `plans/repr-opt/section-08-escape-analysis.md` has a new "Soundness conditions to enforce" subsection with conditions 2 and 4 LITERALLY copied from this plan's Section 01.4 (not paraphrased — verify by side-by-side diff)
- [ ] `plans/repr-opt/section-09-arc-header.md` `compute_sharing_bound()` example uses `EscapeInfo::is_non_escaping(var)` instead of `EscapeState::NoEscape`
- [ ] `plans/repr-opt/section-09-arc-header.md` frontmatter `depends_on` updated
- [ ] `plans/repr-opt/section-10-thread-local-arc.md` has the "no parallel ThreadLocality enum" confirmation note in the section body
- [ ] `plans/repr-opt/00-overview.md` §08 + §01 coupling note (around line 191) updated to reflect that the query body is plumbed by this plan, not §08
- [ ] `plans/repr-opt/00-overview.md` §10 + §01 coupling note (line 192) verified unchanged (already correct)
- [ ] `plans/repr-opt/index.md` keyword clusters for §08/§09/§10 no longer reference `EscapeState` or `ThreadLocality` as standalone enums
- [ ] `grep -rn 'EscapeState' plans/repr-opt/` returns zero matches (or only matches in `<!-- ... -->` comments explaining the historical name)
- [ ] `grep -rn 'enum ThreadLocality' plans/repr-opt/` returns zero matches
- [ ] Every modified file has a `<!-- resolves: locality-representation-unification §04.N -->` comment marker for traceability
- [ ] No code changes were made — `git diff --stat compiler/` returns zero modified files (only `plans/` was touched)
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale annotations introduced by this section
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 04
  - [ ] `00-overview.md` mission success criteria checkboxes updated (5 cross-plan targets done)
  - [ ] `index.md` Section 04 status updated
  - [ ] Cross-links to repr-opt plans updated (`<!-- resolved-by: locality-representation-unification/section-04 -->` in each modified repr-opt file's relevant location, OR a single link in repr-opt/00-overview.md noting the cross-plan dependency)
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed — verifies the cross-plan text is internally consistent and doesn't introduce new LEAKs in the plan corpus
- [ ] `/improve-tooling` retrospective completed — for this section: was there a missing tool to "find every cross-reference between two plan files"? Was there a missing "verify literal copy of text from one section to another" check (since §04 must literally copy from §01.4, paraphrase detection would be useful)? Was there a missing "find all stale plan-text references to a deleted enum" sweep? Implement every accepted improvement NOW (zero deferral).

**Exit Criteria:** A reviewer running `grep -rn 'EscapeState\|enum ThreadLocality' plans/repr-opt/` finds zero matches. Reading `plans/repr-opt/section-08-escape-analysis.md` end-to-end, the reviewer sees that §08 only POPULATES the existing `EscapeInfo` storage (does not DEFINE the type) and that conditions 2 and 4 from `locality-representation-unification` §01.4 are present in the new "Soundness conditions to enforce" subsection — verifiable as a literal copy of two sentences from the upstream plan. `plans/repr-opt/00-overview.md` lines around 191-192 are internally consistent with the new ownership split. Every modified file has a `<!-- resolves: ... -->` marker so future readers can trace why the change was made.
