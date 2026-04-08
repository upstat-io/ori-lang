---
section: "07"
title: "/tp-help dual-source + consolidation"
status: not-started
reviewed: true
goal: "Rewrite .claude/skills/tp-help/SKILL.md for dual-source AND consolidate with .claude/commands/tp-help.md (resolving R10 SSOT violation). /tp-help uses a LIGHTER response envelope and CONCATENATION mode (not synthesis) — raw perspectives from both reviewers returned to the user. Verify that /impl-hygiene-review Phase 4 cross-check (which invokes /tp-help internally) still works with dual-source responses."
success_criteria:
  - ".claude/skills/tp-help/SKILL.md rewritten for dual-source using concatenation mode (not findings envelope)"
  - ".claude/commands/tp-help.md consolidated with the skill file — single source of truth for /tp-help content (R10 resolved)"
  - "The consolidated file is either .claude/commands/tp-help.md as a thin pointer to the skill, OR vice versa — one contains the implementation, the other references it"
  - "Both reviewers' raw responses are concatenated into the output, with clear reviewer attribution headers"
  - "/impl-hygiene-review Phase 4 cross-check still functions correctly under dual-source /tp-help — verified by integration test"
  - "At least 1 real /tp-help scenario runs successfully with both reviewers producing responses"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Consolidate .claude/commands/tp-help.md with .claude/skills/tp-help/SKILL.md (R10)"
    status: not-started
  - id: "07.2"
    title: "Rewrite for dual-source concatenation mode (not findings envelope)"
    status: not-started
  - id: "07.3"
    title: "Verify downstream consumers still work with dual-source /tp-help"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: /tp-help dual-source + consolidation

**Status:** Not Started
**Goal:** Apply dual-source to `/tp-help` with two twists: (1) `/tp-help` uses a LIGHTER response format (concatenation, not the findings envelope — per the Step 1E design decision that "when you're stuck and asking for help, you want raw perspectives from two models, not a smoothed merge"), and (2) this section ALSO consolidates the R10 SSOT violation where `.claude/commands/tp-help.md` (179 lines) duplicates `.claude/skills/tp-help/SKILL.md` (121 lines).

**Success Criteria:**

- [ ] `.claude/skills/tp-help/SKILL.md` rewritten for dual-source using concatenation mode — the output is BOTH reviewers' raw responses, clearly attributed, NOT a findings envelope
- [ ] `.claude/commands/tp-help.md` consolidated with the skill file — one file is the source of truth, the other is either deleted or reduced to a thin pointer that references the canonical file
- [ ] R10 (the two-sources-of-truth SSOT violation) is resolved — verified by grepping for divergent content between the two files (there should be no divergence because one is now a pointer)
- [ ] Both reviewers' raw responses are included in the output with clear attribution (e.g., `## Codex response` header, `## Gemini response` header)
- [ ] `/impl-hygiene-review` Phase 4 cross-check (which invokes `/tp-help` internally — empirically verified at `.claude/skills/impl-hygiene-review/SKILL.md:318-352`; the `/tp-help` call sites are lines 322 and 343, under the `### Phase 4: Third-Party Cross-Check` header) continues to work with dual-source `/tp-help` responses — the impl-hygiene-review receives both reviewers' feedback and can incorporate it with its existing `[TP-CONFIRMED]` and `[TP-SURFACED]` tagging
  - [ ] `.claude/commands/review-plan.md` internal `/tp-help` calls (lines 95 and 305 of the command file, under the "Step 3B: Third-Party Blind Spot Check via /tp-help" and "Midpoint Check: /tp-help Between Agent 2 and Agent 3" sections) continue to work with dual-source `/tp-help` responses — regression test. The command file is promised to remain byte-identical (Section 06), so these internal calls will start receiving concatenated codex+gemini output after Section 07 lands. Verify the command file's 4-agent pipeline still parses and incorporates the new dual-source response format without breakage; if it breaks, fix the incompatibility in 07.2's concatenation format (do NOT modify `.claude/commands/review-plan.md`).
- [ ] At least 1 real `/tp-help` scenario runs successfully with a real question, both reviewers respond, and the concatenated output is returned to the user

**Context:** `/tp-help` is the only review skill in this plan that does NOT use the findings envelope schema. Per the Step 1E architectural decision, it uses concatenation: both reviewers' raw responses are returned adjacent with clear attribution, giving the user two independent perspectives without an editorial synthesis layer. This is because `/tp-help` is called when the user is stuck — they want raw opinions, not a smoothed consensus that might hide useful disagreement.

This section also resolves R10 (the SSOT violation between the command file and the skill file). Both currently exist and divergently describe the same `/tp-help` workflow. The consolidation picks ONE file as the source of truth and makes the other a thin pointer that references it. Per the Step 1E decision, the content belongs in the skill file (because that's where the auto-trigger behavior lives); the command file becomes a thin pointer.

The downstream consumer verification is important and TWO files are affected:

1. **`.claude/skills/impl-hygiene-review/SKILL.md`** calls `/tp-help` internally under the `### Phase 4: Third-Party Cross-Check` header (empirically verified: the two `/tp-help` call sites are at lines 322 and 343; earlier Phase 2 research cited lines 291-308 incorrectly — those line numbers were from a pre-edit snapshot and are stale). When `/tp-help` becomes dual-source, that internal call starts receiving concatenated responses from two reviewers. The impl-hygiene-review wrapper needs to continue processing these responses correctly, tagging findings with `[TP-CONFIRMED]` / `[TP-SURFACED]` as it already does. No changes to impl-hygiene-review are needed — just verification that the change doesn't break it.

2. **`.claude/commands/review-plan.md`** (the 595-line 4-agent Claude pipeline that Section 06 leaves byte-identical) ALSO calls `/tp-help` internally: once at line 95 under "Step 3B: Third-Party Blind Spot Check via /tp-help" and once at line 305 under "Midpoint Check: /tp-help Between Agent 2 and Agent 3". These internal calls will start receiving concatenated codex+gemini responses after this section lands. Because the command file is promised to remain byte-identical (Section 06 regression test), we CANNOT modify the command file to adapt its parsing; the concatenation format must be backward-compatible with whatever text-parsing logic the command file already uses. If the command file relies on single-source response assumptions, we either (a) adjust the concatenation format in 07.2 to preserve backward compatibility, or (b) escalate to the user as a scope expansion decision.

Two downstream consumers × one format change = two regression tests in 07.3.

**Reference implementations:**
- Existing `.claude/skills/tp-help/SKILL.md` (121 lines) and `.claude/commands/tp-help.md` (179 lines) — the two files being consolidated
- `.claude/skills/impl-hygiene-review/SKILL.md:291-308` — the downstream consumer that invokes `/tp-help`
- Section 04's dual-source transport pattern — the same transport scripts, different output mode

**Depends on:** Section 04 (validated dual-source pattern).

---

## 07.1 Consolidate .claude/commands/tp-help.md with .claude/skills/tp-help/SKILL.md (R10)

**File(s):** `.claude/commands/tp-help.md` (rewrite as thin pointer), `.claude/skills/tp-help/SKILL.md` (becomes canonical source)

**Context:** The two files currently divergently describe the same workflow. The skill file is smaller (121 lines) and cleaner; the command file (179 lines) has more aggressive auto-trigger documentation but the core workflow is the same. Consolidation: make the skill file the canonical source (incorporating the useful auto-trigger documentation from the command file), and reduce the command file to a thin pointer.

Tasks:

- [ ] Read both files in full to identify divergence points.

- [ ] Update `.claude/skills/tp-help/SKILL.md` to be the canonical source. **This step is CONSOLIDATION ONLY — do NOT yet rewrite the workflow for dual-source. 07.2 handles the dual-source rewrite as a separate edit pass so any breakage in the consolidation can be diagnosed without confounding it with transport changes.**
  - Incorporate the aggressive auto-trigger documentation from the command file (concrete trigger conditions, example scenarios that MUST trigger auto-invoke)
  - Add explicit mention that this file is the canonical source for `/tp-help` workflow content
  - Preserve the existing single-source codex workflow verbatim — 07.2 rewrites it for dual-source

- [ ] Rewrite `.claude/commands/tp-help.md` as a thin pointer:
  ```markdown
  ---
  name: tp-help
  description: Get third-party help from Codex CLI + Gemini CLI (dual-source). Use this proactively when stuck on a problem, unsure about an implementation approach, want a second opinion on code you just wrote, need help debugging, or want to verify your reasoning.
  allowed-tools: Bash, Read, Grep, Glob
  argument-hint: "[question or context]"
  ---

  # /tp-help — Third-Party Help

  The canonical implementation of `/tp-help` lives in the skill file at
  `.claude/skills/tp-help/SKILL.md`. When the `/tp-help` slash command
  is invoked, load and follow that skill file exactly.

  See `.claude/skills/tp-help/SKILL.md` for:
  - Auto-trigger conditions
  - Workflow (prompt construction, dual-source transport invocation, response handling)
  - Output format (concatenated reviewer responses with attribution)
  - Failure handling
  ```

- [ ] Verify R10 is resolved: grep for any divergent implementation content between the two files. There should be no operational detail duplicated — only the thin pointer in `commands/tp-help.md` and the canonical implementation in `skills/tp-help/SKILL.md`.
  ```bash
  wc -l .claude/commands/tp-help.md .claude/skills/tp-help/SKILL.md
  # Expected: commands/tp-help.md is ~25 lines (thin pointer), skills/tp-help/SKILL.md is the canonical source
  ```

- [ ] **Post-consolidation smoke test (before 07.2's dual-source rewrite)**: the thin-pointer rewrite can break `/tp-help` dispatch independently of any transport changes, so we MUST verify the consolidation is wired correctly BEFORE 07.2 changes the output format. Run a minimal `/tp-help` invocation with a trivial question (e.g., `/tp-help what is 2+2`) and verify:
  - The `/tp-help` slash command reaches the canonical skill file (either directly, or via the thin-pointer command file loading the skill)
  - The single-source codex workflow still runs end-to-end (since 07.2 has not yet rewritten it for dual-source)
  - A codex response is returned to the user
  - `.claude/skills/impl-hygiene-review/SKILL.md` Phase 4 `/tp-help` calls still dispatch correctly (run `/impl-hygiene-review` on a trivial scope and verify Phase 4 completes without dispatch errors)
  - `.claude/commands/review-plan.md` Step 3B `/tp-help` call still dispatches correctly (dry-run against a tiny test plan; abort after Step 3B completes so the rest of the 4-agent pipeline doesn't consume time)

  If ANY of these smoke checks fails, STOP. The consolidation itself is broken — fix it before starting 07.2. Mixing consolidation bugs with dual-source rewrite bugs produces unbounded debugging. The two changes land in sequence: consolidate, smoke-test, then rewrite.

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] Consolidation done, command file is a thin pointer, skill file is canonical
  - [ ] R10 resolved — no operational duplication
  - [ ] Post-consolidation smoke test passed (`/tp-help` still reaches the skill, `impl-hygiene-review` Phase 4 still dispatches, `review-plan.md` Step 3B still dispatches — all BEFORE dual-source changes land)
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was the "canonical vs pointer" pattern clear? Should there be a `lint-no-command-skill-drift.sh` that detects SSOT violations between `.claude/commands/` and `.claude/skills/` for other skills? Implement improvements.

---

## 07.2 Rewrite for dual-source concatenation mode (not findings envelope)

**File(s):** `.claude/skills/tp-help/SKILL.md` (continue from 07.1's edit)

**Context:** `/tp-help` does NOT use the findings envelope — it uses concatenation. The dual-source transport is still the same (both reviewers launched in parallel via `dual-invoke-with-retry.sh`), but the parsing is simpler: extract the reviewer's entire response text (not just a JSON envelope), and concatenate both responses with clear attribution headers.

The codex-side parsing: extract the final `agent_message.text` as raw text (no JSON parsing, no schema validation — it's a response, not findings). The gemini-side parsing: concatenate all `delta: true` assistant message fragments in arrival order (same as the parser from Section 02.3), then extract the text WITHOUT sentinel extraction (there are no sentinels because there's no JSON envelope).

Tasks:

- [ ] Update the skill file's Steps section to use dual-source transport but with concatenation-mode parsing:
  1. Create per-run scratch dir
  2. Write codex prompt (no `envelope-only` keyword needed — `/tp-help` doesn't have plan-write mode for codex to avoid)
  3. Write gemini prompt with "Activate the tp-help skill..." preamble OR treat tp-help as a generic question answering task without a dedicated gemini skill (the Step 1E decision for tp-help was concatenation without envelope, so no dedicated gemini skill is strictly required)
  4. Invoke `dual-invoke.sh` (NOT `dual-invoke-with-retry.sh` — tp-help is one-shot, infra failure surfaces directly to user without retry)
  5. Parse both outputs as RAW TEXT (codex: last agent_message.text; gemini: concatenated assistant content)
  6. Concatenate with attribution:
     ```
     ## Codex response
     <codex raw text>

     ## Gemini response (grounded)
     <gemini raw text>
     ```
  7. Return the concatenated output to the user

- [ ] Note that dual-source tp-help does NOT have a dedicated gemini skill file — it uses gemini as a generic assistant. This is consistent with the concatenation mode: no envelope = no schema = no activation ceremony. The gemini prompt is the user's question + the codex-side prompt context.

- [ ] Update auto-trigger documentation in the skill file to mention that dual-source tp-help is ~10x slower than codex-only tp-help due to gemini's wall time. Document the `ORI_TPR_REVIEWERS=codex` escape hatch (from Section 08) for cases where the user wants the faster single-source version.

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] Rewrite done; /tp-help returns concatenated dual-source output
  - [ ] Attribution headers present in output
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was the concatenation format clear? Should attribution be richer (include wall time per reviewer)? Implement improvements.

---

## 07.3 Verify downstream consumers still work with dual-source /tp-help

**File(s):** Validation only (NO modifications to `.claude/skills/impl-hygiene-review/SKILL.md` and NO modifications to `.claude/commands/review-plan.md`)

**Context:** Two downstream consumers invoke `/tp-help` internally. When `/tp-help` becomes dual-source, both of those internal calls start receiving concatenated responses from two reviewers. Neither downstream consumer can be modified — `impl-hygiene-review` is out-of-scope for modification in this section, and `.claude/commands/review-plan.md` is byte-identical by contract (Section 06). This subsection verifies BOTH consumers continue to work correctly with the new response format.

Downstream consumer inventory (empirically verified):
1. **`.claude/skills/impl-hygiene-review/SKILL.md`** — invokes `/tp-help` under `### Phase 4: Third-Party Cross-Check` (lines 318-352); the actual `/tp-help` call sites are at lines 322 and 343. The wrapper's existing logic tags confirmed findings with `[TP-CONFIRMED]` and surfaces findings with `[TP-SURFACED]`.
2. **`.claude/commands/review-plan.md`** — invokes `/tp-help` twice: once at line 95 under `### Step 3B: Third-Party Blind Spot Check via /tp-help` (blind-spot check before the 4 review agents launch) and once at line 305 under `#### Midpoint Check: /tp-help Between Agent 2 and Agent 3`. Both calls are SEQUENTIAL and FOREGROUND by explicit contract in the command file.

Tasks:

- [ ] Read `.claude/skills/impl-hygiene-review/SKILL.md` lines 318-360 to understand exactly how it calls `/tp-help` and processes the response. (Earlier plan drafts cited lines 291-308; that reference was stale — the actual Phase 4 header is on line 318, with call sites at 322 and 343.)

- [ ] Read `.claude/commands/review-plan.md` lines 95-135 (Step 3B) and lines 305-340 (Midpoint Check) to understand how the command file builds its `/tp-help` prompts and consumes the responses.

- [ ] **Scenario 1 — impl-hygiene-review integration test**: Run `/impl-hygiene-review` on a small test scope (a single file or small module). Verify:
  - Phase 4 invokes `/tp-help` internally and receives the dual-source concatenated response
  - impl-hygiene-review correctly parses both reviewer responses (it doesn't care about the attribution headers — it just uses the response text for validation)
  - `[TP-CONFIRMED]` and `[TP-SURFACED]` tagging still works correctly in the output
  - No errors or regressions in the hygiene review workflow

- [ ] **Scenario 2 — `.claude/commands/review-plan.md` command-file integration test**: Run `/review-plan` (which dispatches to `.claude/commands/review-plan.md`, NOT the new Section 06 skill) against a small test plan directory (a completed plan in `plans/completed/` works well). Verify:
  - Step 3B's blind-spot `/tp-help` call succeeds, receives the dual-source concatenated response, and the 4 review agents consume its output without parse errors
  - The midpoint `/tp-help` call (between Agent 2 and Agent 3) succeeds the same way
  - The command file's 4-agent pipeline runs to completion
  - `git diff --exit-code .claude/commands/review-plan.md` still returns 0 after the run (command file unchanged)
  - No errors or regressions in the review-plan workflow

- [ ] **Scenario 3 — byte-identity regression test for `.claude/commands/review-plan.md`**: After running Scenario 2 verify:
  ```bash
  git diff --exit-code .claude/commands/review-plan.md
  echo "review-plan.md exit=$?"  # expected: 0
  ```

- [ ] If either downstream consumer breaks because of the new response format: flag immediately. Do NOT silently fix `impl-hygiene-review` or modify `.claude/commands/review-plan.md` as part of this section — that would be scope creep and/or violate the byte-identity contract. Either (a) adjust the concatenation format in 07.2 to match what the downstream consumers expect, or (b) escalate to the user as a separate scope expansion decision.

- [ ] Record all three scenario results in working notes.

- [ ] **Subsection close-out (07.3)** — MANDATORY before section completion:
  - [ ] impl-hygiene-review integration test passes (Scenario 1)
  - [ ] `.claude/commands/review-plan.md` command-file integration test passes (Scenario 2)
  - [ ] `.claude/commands/review-plan.md` byte-identity check passes (Scenario 3)
  - [ ] No modifications to `impl-hygiene-review` or `.claude/commands/review-plan.md` required
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — should there be an automated integration test that runs BOTH downstream consumers and verifies their `/tp-help` integration still works, as part of the Section 02 transport test suite or a new `tp-help-consumer-tests.sh`? Implement improvements.

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] All three subsections (07.1, 07.2, 07.3) marked `complete`
- [ ] `.claude/skills/tp-help/SKILL.md` is canonical; `.claude/commands/tp-help.md` is a thin pointer
- [ ] R10 resolved (no operational duplication between the two files)
- [ ] `/tp-help` returns concatenated dual-source output with attribution headers
- [ ] `/impl-hygiene-review` Phase 4 cross-check passes integration test
- [ ] At least 1 real /tp-help scenario passes
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Plan annotation cleanup clean
- [ ] **Plan sync**: Section 07 frontmatter → `complete`, Quick Reference updated, R10 marked resolved
- [ ] `/tpr-review` (dual-source) passed
- [ ] `/impl-hygiene-review` passed — note: this is now the SECOND run of impl-hygiene-review in this session (first was in 07.3's integration test; second is the section-close verification)
- [ ] `/improve-tooling` section-close sweep done

**Exit Criteria:** `/tp-help` is dual-source with concatenation output; the R10 SSOT violation is fixed; `/impl-hygiene-review` Phase 4 cross-check works unchanged. Section 08 (integration + cleanup) can begin.
