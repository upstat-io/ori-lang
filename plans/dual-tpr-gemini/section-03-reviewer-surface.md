---
section: "03"
title: "Reviewer surface preparation"
status: not-started
reviewed: true
goal: "Extract the reviewer-agnostic command file, add plan-write/envelope-only execution mode branches to the codex review-work and review-plan skills, and create the gemini review-work and review-plan skills with grounding directive and explicit activation convention. Prepares the reviewer surfaces so Section 04's /tpr-review rewrite can invoke both reviewers uniformly."
success_criteria:
  - ".claude/skills/dual-tpr/command-file.md exists as the reviewer-agnostic methodology, extracted from the existing codex skill content, containing the common review contract (scope resolution heuristics, evidence gathering, deep investigation, finding format, verification basis categories)"
  - ".codex/skills/review-work/SKILL.md has a top-level execution mode branch (plan-write vs envelope-only) that dispatches at the start of the workflow; standalone codex exec /review-work still writes findings to plan files (regression preserved)"
  - ".codex/skills/review-plan/SKILL.md has the same mode branch; standalone codex exec /review-plan still edits plan files directly"
  - ".gemini/skills/review-work/SKILL.md exists with YAML frontmatter (name, description) and is discoverable by 'gemini skills list' when run from the project root"
  - ".gemini/skills/review-plan/SKILL.md exists and is discoverable the same way"
  - "Both gemini skills contain an explicit 'use google_web_search to verify external claims' directive in the body — verified by grep"
  - "The wrapper invocation pattern documented in .claude/skills/dual-tpr/transport.md (a new doc file) specifies the explicit skill activation phrase ('Activate the {skill} skill and follow its instructions exactly')"
inspired_by:
  - ".codex/skills/review-work/SKILL.md (existing 370-line file) — the canonical review methodology that gets extracted into the shared command file and referenced by both codex and gemini skills"
  - ".codex/skills/review-plan/SKILL.md (existing 270-line file) — same for plan-review semantics"
  - "Phase 2 Agent 3's empirical gemini skills research — verified that .gemini/skills/<name>/SKILL.md is auto-discovered from workspace root with zero registration, and that explicit prompt activation is required"
  - "Codex Step 6B Q3 (mode switch must be real execution branch, not soft prompt override) and Q5 (gemini grounding directive belongs in gemini skill, not shared command file)"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Extract shared reviewer-agnostic command file"
    status: not-started
  - id: "03.2"
    title: "Add plan-write/envelope-only mode branches to codex skills"
    status: not-started
  - id: "03.3"
    title: "Create gemini skills with grounding directive and activation convention"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Reviewer surface preparation

**Status:** Not Started
**Goal:** Prepare the reviewer-facing surfaces (codex skill mode switches + gemini skill creation + shared command file) so that Section 04's `/tpr-review` rewrite has a uniform contract to invoke both reviewers. This section does NOT invoke the reviewers — that's Section 04's job. It only prepares the skill files that the reviewers will load when invoked.

**Success Criteria:**

- [ ] `.claude/skills/dual-tpr/command-file.md` exists as the reviewer-agnostic methodology document. It contains the common review contract (scope resolution, evidence gathering, deep investigation standard, finding format, verification basis categories) extracted from the existing `.codex/skills/review-work/SKILL.md` and `.codex/skills/review-plan/SKILL.md`. It does NOT contain gemini-specific instructions or codex-specific mode switches — those live in the respective reviewer skills. Satisfies mission criterion: "shared command file stays reviewer-agnostic."
- [ ] `.codex/skills/review-work/SKILL.md` has a top-level execution mode branch at the start of the workflow (Step 1 or earlier) that dispatches on a mode indicator: `plan-write` (existing behavior: write findings directly to plan file sections using `## NN.R Third Party Review Findings` format) or `envelope-only` (new behavior: emit JSON envelope only, do not touch plan files, do not write anywhere). The mode is selected by the presence of the `envelope-only` keyword in the prompt. Existing standalone `codex exec /review-work` invocations default to `plan-write` (no prompt keyword) and preserve the current behavior exactly.
- [ ] `.codex/skills/review-plan/SKILL.md` has the same mode branch, with `plan-write` (existing behavior: edit plan files directly) and `envelope-only` (new behavior: emit JSON envelope only).
- [ ] Standalone regression test: `codex exec "run the /review-work skill on the last commit" --full-auto` still writes findings to the owning plan section's TPR block — verified by running it against a test fixture and inspecting the plan file.
- [ ] `.gemini/skills/review-work/SKILL.md` exists with correct YAML frontmatter (`name: review-work`, `description: "..."`), contains the body referencing the shared command file, includes the grounding directive ("use `google_web_search` to verify external claims about libraries, specs, prior art, or recent developments; cite source URLs in the envelope's `citations` field"), and is discoverable by `gemini skills list` when run from the project root.
- [ ] `.gemini/skills/review-plan/SKILL.md` exists with the same structure, adapted for plan-review semantics (review a plan holistically, edit plan files directly in envelope-only mode by emitting the proposed edits as findings).
- [ ] `.claude/skills/dual-tpr/transport.md` (new high-level doc) documents the wrapper invocation pattern including the explicit skill activation phrase convention: every wrapper's prompt to gemini MUST start with "Activate the {skill} skill and follow its instructions exactly. ..." to ensure gemini fires the skill rather than falling back to generic response mode.
- [ ] A `grep -l 'google_web_search' .gemini/skills/` finds both new skill files — verifies the grounding directive is present.

**Context:** Section 03 is where the reviewer-facing contracts get prepared. Nothing in Section 03 actually invokes a reviewer — it only creates and modifies the skill files that reviewers load when Section 04+'s wrappers invoke them. The subtle but critical constraint is that the codex skill changes must be PURELY ADDITIVE: standalone `codex exec /review-work` and `codex exec /review-plan` must continue to work exactly as today, because there are existing workflows (including documentation in `CLAUDE.md:140` and the `.claude/commands/review-plan.md:99` blind-spot check in `/review-plan`) that depend on those standalone paths. The gemini skills are greenfield creation — no existing behavior to preserve, but they must be structurally consistent with the codex skills so that both reviewers emit the same envelope format.

Per Codex Step 6B Q3, the mode switch MUST be a real execution branch at the top of the skill, NOT a soft prompt override appended at the end. Soft overrides fight the existing write rules throughout the file and can produce inconsistent behavior depending on which instruction the reviewer reads first. A real branch decides early and every downstream step respects the decision.

Per Codex Step 6B Q5, the gemini grounding directive (`google_web_search` usage) belongs in `.gemini/skills/review-work/SKILL.md`, NOT in the shared command file. The shared command file stays reviewer-agnostic — it contains the review contract that's true for both reviewers, not the tool-specific capabilities unique to one. This keeps the command file lean and prevents it from accumulating reviewer-specific drift.

**Reference implementations:**

- **`.codex/skills/review-work/SKILL.md`** (existing 370-line file) — the canonical single-source review workflow that this section extracts the reviewer-agnostic methodology from. The "Scope Inputs" section, "Review Workflow", "Mandatory Standards Checks", "Finding Format" are all reviewer-agnostic and extract cleanly. The "Plan Update Rules" section is mode-specific (plan-write) and stays in the codex skill behind the mode branch.
- **`.codex/skills/review-plan/SKILL.md`** (existing 270-line file) — same extraction target for plan-review specifics.
- **Phase 2 Agent 3 empirical gemini research** (stored in session memory, also captured in Phase 2's research summary in the conversation log) — established that `.gemini/skills/<name>/SKILL.md` is auto-discovered from the current workspace root when `gemini` is invoked, with zero registration step. Skills are discovered but NOT auto-activated; explicit prompt invocation is required.

**Depends on:** Section 02 (the shared transport utility). Section 03 cannot begin until Section 02's transport scripts are complete because Section 03's documentation references specific transport contract points (the envelope format, the failure taxonomy, the `status: "complete"` requirement) that Section 02 implements.

---

## 03.1 Extract shared reviewer-agnostic command file

**File(s):** `.claude/skills/dual-tpr/command-file.md` (new)

**Context:** Both `.codex/skills/review-work/SKILL.md` and `.codex/skills/review-plan/SKILL.md` contain substantial amounts of reviewer-agnostic methodology — scope resolution heuristics, evidence-gathering procedure, deep investigation standard, finding format, verification basis categories — that will also apply to the gemini skills. Rather than duplicating this content across four skill files (two codex + two gemini), we extract it into ONE shared command file that all four skills reference.

This is the SSOT principle from `impl-hygiene.md` applied to skill methodology: the review contract lives in one place; skill files cite it rather than copying it. The command file contains ONLY reviewer-agnostic content. Reviewer-specific content (codex mode switch, gemini grounding directive, CLI-specific output format instructions) stays in the respective skill files.

Extraction target: extract methodology that answers "HOW to review" — not "WHO reviews" or "WHAT tool is running." HOW-to-review is shared; WHO/WHAT is reviewer-specific.

Rules embedded inline:
- File size: ~400-500 lines target (extracted content is substantial but coherent as one document)
- No mention of codex, gemini, or specific CLI flags — reviewer-agnostic means tool-agnostic
- No mention of `plan-write` or `envelope-only` modes — those are codex-specific execution modes, not methodology

Tasks:

- [ ] Read the full contents of `.codex/skills/review-work/SKILL.md` and `.codex/skills/review-plan/SKILL.md` to identify the reviewer-agnostic content that can be extracted.

- [ ] Write `.claude/skills/dual-tpr/command-file.md` containing the shared methodology. Structure:

  ```markdown
  # Shared Reviewer Command File (reviewer-agnostic methodology)

  This file contains the review methodology shared by all reviewers
  (codex, gemini) across all review skills (tpr-review, review-work,
  review-plan, tp-help). Reviewer-specific and tool-specific content
  lives in the respective skill files that reference this document.

  ## Scope Inputs

  Every review starts by resolving scope from one of the following:
  - A plan directory or section file
  - A section ID or keywords
  - A git range or commit selector (HEAD~3..HEAD, last commit, etc.)
  - Uncommitted work selectors (staged, unstaged, worktree)
  - Explicit files or directories

  If no scope is explicitly named, default to recent committed slice
  (HEAD~3..HEAD) plus staged and unstaged changes.

  ## Scope Resolution Order

  1. Existing path from the user
  2. Explicit git range or commit selector
  3. Explicit uncommitted-work selector
  4. Plan match from plans/*/index.md, plans/*/00-overview.md,
     plans/*/section-*.md
  5. Default recent committed slice (HEAD~3..HEAD)

  Broaden the scope if it's too narrow to be coherent (e.g., if it's
  just a fixup for previous commits; look at HEAD~5..HEAD or further).

  ## Evidence Gathering

  Collect in order:
  1. Git evidence: committed diff stat + patch, staged/unstaged diffs,
     commit log, git status --short
  2. File inventory: identify all changed files, the tests that should
     cover them, adjacent code needed to understand behavior. READ THE
     FULL CHANGED FILES, not just diff hunks.
  3. Standards packet: read CLAUDE.md, .claude/rules/tests.md,
     .claude/rules/compiler.md, .claude/rules/impl-hygiene.md,
     .claude/rules/roadmap.md, and any other relevant rules files
  4. Plan context: check recently modified plans for drift; if a
     plan/section was named, read its index.md, 00-overview.md, and
     the target section

  ## Deep Investigation Standard

  This is NOT a diff skim. You must:
  - Read whole changed files
  - Read neighboring code to understand invariants, ownership, boundary
    contracts
  - Trace data flow across function, module, and phase boundaries
    touched by the work
  - Inspect both tests that changed and tests that should have changed
    but did not
  - Use commit-by-commit history to catch partial reverts and
    contradictory edits
  - Prefer diagnostics, tracing, and repo-native verification tools
    over guesswork

  When a change touches ARC, AOT, lowering, runtime, tests, spec, or
  roadmap-owned areas, assume the failure surface is wider than the
  diff and expand the review accordingly.

  ## Mandatory Standards Checks

  Every review must explicitly test the work against:
  - Bugs are fixed completely, not deferred
  - Tests come with the fix; matrix + semantic pins required
  - Debug + release verification
  - Plan boundaries updated when fixes cross sections
  - No workarounds or dummy values
  - Touched Rust files respect hygiene: sibling tests.rs, file-size
    limits, tracing not println!, no dead code, no unjustified lint
    suppression
  - Domain-specific rules under .claude/rules/*.md satisfied

  If the work violates CLAUDE.md or a rule file, that IS a finding
  even if the code "works."

  ## Finding Format (for envelopes)

  Each finding contains:
  - `ordinal`: integer, 1-based, independent per reviewer
  - `severity`: high | medium | low
  - `location`: repo-relative path:line matching the canonical regex
    `^[a-zA-Z0-9_./-]+:[0-9]+$` (see .claude/skills/dual-tpr/envelope-format.md)
  - `title`: imperative voice, sentence case, no markdown, no trailing
    punctuation, max 200 chars
  - `evidence`: specific mismatch, regression, or missing case
  - `impact`: why the work is incomplete, unsafe, or non-compliant
  - `required_plan_update`: what must be validated and integrated
  - `layer`: committed | staged | unstaged
  - `basis`: fresh_verification | direct_file_inspection |
    git_history | inference
  - `confidence`: high | medium | low
  - `citations`: optional array of {url, description} for external
    sources (grounded research, specs, prior art)

  See `.claude/skills/dual-tpr/findings-schema.json` for the
  authoritative schema.

  ## Verification Basis

  Every finding must declare its basis — one of:
  - `fresh_verification`: reviewer actually ran the test/script and
    observed the outcome. This is the strongest basis.
  - `direct_file_inspection`: reviewer read the code and reasoned
    about it without running it
  - `git_history`: reviewer inspected commits/blame to trace changes
    over time
  - `inference`: reviewer deduced from context without direct
    observation. This is the weakest basis; use sparingly.

  Prefer fresh verification when feasible. When it's not, be explicit
  about the weaker basis so the consumer can calibrate trust.

  ## Review Boundaries

  Do NOT:
  - Accept "done" claims because a checklist is checked off
  - Treat commit messages as proof that implementation is correct
  - Ignore staged or unstaged deltas when they materially change the
    reviewed work
  - Flag speculative issues without evidence
  - Mark findings resolved with scope notes or rationalizations

  Do:
  - Review committed, staged, and unstaged work together when relevant
  - Call out mismatches between branch history and current tree state
  - Surface CLAUDE.md and rule-file violations explicitly
  - Annotate when a finding is already covered by a recent plan
  - Mention residual risk when verification was blocked
  - Keep findings sharp enough for a later implementation pass to act
    on directly
  ```

- [ ] Verify the command file does NOT contain any codex-specific or gemini-specific content:
  ```bash
  grep -i 'codex\|gemini\|--full-auto\|--output-schema\|google_web_search\|plan-write\|envelope-only' \
      .claude/skills/dual-tpr/command-file.md
  # Expected: no output (grep exits 1 if no match, 0 if match; we want 1)
  ```

- [ ] Verify the command file DOES contain the key reviewer-agnostic concepts:
  ```bash
  for concept in "Scope Resolution" "Evidence Gathering" "Deep Investigation" "Mandatory Standards" "Finding Format" "Verification Basis"; do
    grep -q "$concept" .claude/skills/dual-tpr/command-file.md && echo "OK: $concept" || echo "MISSING: $concept"
  done
  # Expected: 6 OK lines
  ```

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] Command file written, extracts the correct content, and passes the grep tests (no reviewer-specific content; all key concepts present)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — was the extraction process (reading two 300+ line files, identifying overlap, extracting shared content) tedious? Should there be a `diff-for-overlap.sh` helper that finds shared-but-slightly-different paragraphs across two markdown files? Would a `lint-command-file.sh` that runs the grep checks from above as a CI test catch future drift? Implement every accepted improvement NOW (zero deferral) and commit via separate `/commit-push` calls with `build(diagnostics): add ... — surfaced by dual-tpr-gemini/section-03.1 retrospective`. Do not skip the retrospective even if nothing felt painful.

---

## 03.2 Add plan-write/envelope-only mode branches to codex skills

**File(s):** `.codex/skills/review-work/SKILL.md` (modify), `.codex/skills/review-plan/SKILL.md` (modify)

**Context:** Per Codex Step 6B Q3, the mode switch must be a REAL execution branch at the top of each skill, not a soft prompt override. This means adding an explicit Step 0 at the very start of the workflow that inspects the prompt for the keyword `envelope-only`. If present: the reviewer emits the JSON envelope (per `findings-schema.json`) and does NOT touch any plan files. If absent (the existing standalone usage): the reviewer follows the existing Plan Update Rules and writes findings directly to plan sections.

Critical: the change must be ADDITIVE. Existing standalone `codex exec /review-work` invocations must continue to work exactly as today. Verification is a regression test: run standalone and inspect the plan file after.

The two codex skills (`review-work` and `review-plan`) differ in their plan-write behavior:
- `review-work` writes findings to the owning plan section's `## NN.R Third Party Review Findings` block OR to `plans/bug-tracker/` if no owning plan exists
- `review-plan` edits plan files directly (fix inaccuracies, expand thin sections, update cross-section dependencies)

Both get the SAME mode-switch logic, but the envelope-only branch differs in what they emit in envelope-only mode (both emit findings per the schema, but `review-plan` emits proposed plan edits as findings rather than applying them).

Tasks:

- [ ] Read `.codex/skills/review-work/SKILL.md` in full to identify the insertion point for the mode branch. The insertion point is immediately after the frontmatter and title, before the existing "Scope Inputs" section — i.e., as a new "Step 0: Execution Mode" section at the top of the workflow.

- [ ] Edit `.codex/skills/review-work/SKILL.md` to insert the mode branch at the top of the workflow. Insert this block immediately after the file header and before the current "Scope Inputs" section:

  ```markdown
  ## Step 0: Execution Mode (MANDATORY — read first)

  This skill has two execution modes. The mode is selected by inspecting
  the prompt for the keyword `envelope-only`:

  **Mode A — `plan-write` (default, standalone usage):**
  - The prompt does NOT contain the keyword `envelope-only`
  - Follow the existing workflow below (Scope Inputs, Review Workflow,
    Plan Update Rules)
  - Write findings directly to plan file sections using the
    `## NN.R Third Party Review Findings` format
  - OR file findings as bugs in `plans/bug-tracker/` if no owning plan
    exists
  - This is the ORIGINAL behavior of this skill and MUST be preserved
    for standalone `codex exec /review-work` invocations

  **Mode B — `envelope-only` (dual-source wrapper usage):**
  - The prompt contains the keyword `envelope-only`
  - Follow the same investigation workflow (Scope Inputs, Review Workflow)
    but DO NOT execute the Plan Update Rules section
  - Instead, emit ONE JSON envelope at the end of your response conforming
    to `.claude/skills/dual-tpr/findings-schema.json`
  - DO NOT modify any plan files, bug-tracker files, or any source files
  - DO NOT write to any location on disk other than your own output stream
  - The envelope is emitted as the final `agent_message` content (codex's
    `--output-schema` flag enforces schema conformance at the CLI boundary,
    so you do not need sentinel markers on the codex side)
  - See `.claude/skills/dual-tpr/envelope-format.md` for the complete
    envelope contract and field semantics

  **Execution mode dispatch:**
  1. Inspect the prompt for the literal keyword `envelope-only`
  2. If present: proceed in Mode B (envelope-only). All Plan Update Rules
     below are suppressed. Only the investigation and findings generation
     remain active.
  3. If absent: proceed in Mode A (plan-write). Existing behavior,
     unchanged.

  This is NOT a soft override — Mode B is a real execution branch that
  suppresses the Plan Update Rules section entirely. Any code path that
  would write to a plan file, bug-tracker file, or source file MUST
  check the mode and no-op in Mode B.
  ```

- [ ] Read `.codex/skills/review-plan/SKILL.md` in full to identify the insertion point for its mode branch. Same pattern: immediately after the file header and before the existing "Scope Inputs" section.

- [ ] Edit `.codex/skills/review-plan/SKILL.md` to insert a structurally parallel mode branch, adapted for plan-review semantics:

  ```markdown
  ## Step 0: Execution Mode (MANDATORY — read first)

  This skill has two execution modes. The mode is selected by inspecting
  the prompt for the keyword `envelope-only`:

  **Mode A — `plan-write` (default, standalone usage):**
  - The prompt does NOT contain the keyword `envelope-only`
  - Follow the existing workflow below — edit plan files directly to
    fix inaccuracies, expand thin sections, add missing cross-section
    dependencies, etc.
  - This is the ORIGINAL behavior of this skill and MUST be preserved
    for standalone `codex exec /review-plan` invocations

  **Mode B — `envelope-only` (dual-source wrapper usage):**
  - The prompt contains the keyword `envelope-only`
  - Follow the same investigation workflow but DO NOT edit plan files
    directly
  - Instead, emit ONE JSON envelope at the end of your response
    conforming to `.claude/skills/dual-tpr/findings-schema.json`
  - Each "finding" in envelope-only mode describes a PROPOSED edit —
    the file path, line number, and the nature of the change — rather
    than applying the edit in place
  - DO NOT modify any files; emit the envelope only
  - See `.claude/skills/dual-tpr/envelope-format.md` for the envelope contract

  **Execution mode dispatch:** (same as review-work)
  1. Inspect the prompt for the literal keyword `envelope-only`
  2. If present: proceed in Mode B. All file-editing instructions below
     are suppressed.
  3. If absent: proceed in Mode A. Existing behavior, unchanged.
  ```

- [ ] Verify the mode branches are ADDITIVE — no existing lines deleted, only new lines added at the top:
  ```bash
  # Before the edit, capture the line count
  git show HEAD:.codex/skills/review-work/SKILL.md | wc -l   # say 370
  git show HEAD:.codex/skills/review-plan/SKILL.md | wc -l   # say 270
  # After the edit
  wc -l .codex/skills/review-work/SKILL.md   # should be ~370 + ~40 for the mode branch = ~410
  wc -l .codex/skills/review-plan/SKILL.md   # should be ~270 + ~35 for the mode branch = ~305
  ```
  Additive confirmation: no lines removed, only mode-branch content added.

- [ ] Regression test — standalone plan-write mode for review-work:
  ```bash
  # Create a minimal test scenario
  RUN=$(mktemp -d)
  mkdir -p "$RUN/plans/test-plan/"
  cat > "$RUN/plans/test-plan/00-overview.md" <<'EOF'
  ---
  plan: "test-plan"
  title: "Test Plan"
  status: active
  ---
  # Test Plan
  ## Mission
  Test scope.
  EOF
  # Cannot actually run codex here without side effects; instead,
  # inspect the skill file to verify that the workflow below the
  # Step 0 block is UNCHANGED:
  # - Scope Inputs section still present
  # - Review Workflow section still present
  # - Plan Update Rules section still present
  # - Finding Format section still present
  grep -c '^## Scope Inputs' .codex/skills/review-work/SKILL.md      # expect 1
  grep -c '^## Review Workflow' .codex/skills/review-work/SKILL.md   # expect 1
  grep -c '^## Plan Update Rules' .codex/skills/review-work/SKILL.md # expect 1
  grep -c '^## Finding Format' .codex/skills/review-work/SKILL.md    # expect 1
  rm -rf "$RUN"
  ```

- [ ] Same regression check for `.codex/skills/review-plan/SKILL.md`:
  ```bash
  grep -c '^## Scope Inputs' .codex/skills/review-plan/SKILL.md      # expect 1
  grep -c '^## Review Workflow' .codex/skills/review-plan/SKILL.md   # expect 1
  grep -c '^## Plan Edit Rules' .codex/skills/review-plan/SKILL.md   # expect 1 (note: "Edit" not "Update")
  ```

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] Both codex skills have the mode branch inserted as the new Step 0
  - [ ] The existing workflow content is unchanged — only the Step 0 block was added
  - [ ] Regression tests confirm the existing section headers are all still present
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — was determining the "insertion point" in the existing skill files easy or error-prone? Should there be a `skill-insert-step-0.sh` helper that takes a skill file and a Step 0 block and inserts it at the right place? Would a `diff-skill-pre-post.sh` that shows exactly which lines were added help verify additive-only edits? Implement improvements NOW.

- [ ] **TPR checkpoint** — `/tpr-review` covering 03.1–03.2 (shared command file + codex mode switches)
  <!-- Catches mode-switch design issues BEFORE the gemini skills get built on the same contract.
       If the codex side has a problem, fixing it now is one file; fixing it after the gemini skills
       are written requires coordinated edits across four files. -->

---

## 03.3 Create gemini skills with grounding directive and activation convention

**File(s):** `.gemini/skills/review-work/SKILL.md` (new), `.gemini/skills/review-plan/SKILL.md` (new), `.claude/skills/dual-tpr/transport.md` (new)

**Context:** This subsection creates the gemini-side skill files that Section 04+'s wrappers will invoke. Per Phase 2 Agent 3 empirical research, gemini auto-discovers workspace skills from `<cwd>/.gemini/skills/<name>/SKILL.md` with zero registration — the files just need to exist at the right path when `gemini` is invoked from the project root.

Per Codex Step 6B Q5, the gemini-specific grounding directive (`google_web_search` usage for external claims) lives in these skill files, NOT in the shared command file from 03.1. This keeps the command file reviewer-agnostic while giving gemini its unique capability instruction.

Per Phase 2 Agent 3 empirical research, gemini skills are discovered but NOT auto-activated. The wrapper's prompt MUST explicitly start with "Activate the {skill} skill and follow its instructions exactly" to ensure gemini fires the skill rather than falling back to generic response mode. This convention is documented in `.claude/skills/dual-tpr/transport.md` (new high-level doc) so all four Section 04-07 wrappers reference it consistently.

The gemini skills are structurally parallel to the codex skills but DO NOT need the mode-switch because gemini only runs in envelope-only mode (there is no "write to plan files" standalone usage of the gemini skills — they exist solely for the dual-source wrapper use case).

Tasks:

- [ ] Create directory `.gemini/skills/`. This directory does not exist in the repo today — this subsection is greenfield: create the parent `.gemini/` and `.gemini/skills/` directories first, then the per-skill subdirectories. Verify with `ls -la .gemini/skills/` after creation.

- [ ] Create directory `.gemini/skills/review-work/`.

- [ ] Write `.gemini/skills/review-work/SKILL.md` with the following structure. The body is structurally parallel to `.gemini/skills/review-plan/SKILL.md` (same section headers, same contract points, same failure/escalation wording) — any drift between the two gemini skill files is an SSOT violation that the close-out grep checks MUST catch.

  ```markdown
  ---
  name: review-work
  description: Review actual implementation work (bug fixes, features, refactors, multi-file changes) and emit a JSON findings envelope. Use this when the user asks for a third-party review of work done across committed history, staged changes, unstaged changes, or a plan section. Does NOT modify any files — emits envelope only.
  ---

  # Review Work (gemini side)

  This skill implements the review-work workflow for Gemini as part of
  the dual-source TPR system. It always runs in envelope-only mode —
  it does NOT write findings to plan files or source files, only emits
  a JSON envelope conforming to `.claude/skills/dual-tpr/findings-schema.json`.

  ## Step 0: Execution Mode (MANDATORY — read first)

  This skill has ONE execution mode: **envelope-only**. Unlike the
  codex-side review-work skill (which has a `plan-write` vs `envelope-only`
  Step 0 dispatch), this gemini skill has no plan-write branch because
  there is no standalone "write to plan files" use case for the gemini
  side — the gemini skill exists solely for the dual-source wrapper.

  This is a REAL execution branch, not a soft override:
  1. You MUST emit a JSON envelope conforming to the schema at the end
     of your response
  2. You MUST NOT modify any files (source, plan, bug-tracker, anything)
  3. You MUST NOT write to any location on disk other than your own
     output stream
  4. Every code path that would modify a file is suppressed by this Step 0

  If a later instruction in this file appears to contradict Step 0,
  Step 0 wins. Envelope-only is non-negotiable.

  ## Methodology

  Follow the shared reviewer-agnostic methodology documented in
  `.claude/skills/dual-tpr/command-file.md` for:
  - Scope resolution
  - Evidence gathering
  - Deep investigation standard
  - Mandatory standards checks
  - Finding format
  - Verification basis categories
  - Review boundaries

  The shared command file is the single source of truth for HOW to
  review. This file adds gemini-specific instructions on top of that
  methodology.

  ## Grounding directive (gemini-specific)

  You have access to `google_web_search`. USE IT proactively for any
  finding that makes a claim about:
  - External libraries (Rust crates, Python packages, Node modules, etc.)
  - Language specifications (Rust reference, Python PEPs, TC39, etc.)
  - Compiler internals of other projects (rustc, swift, lean4, koka)
  - Prior art comparisons ("how does X handle this?")
  - Recent developments (changes since your training cutoff)
  - Security best practices
  - Performance claims that require citation

  When you use `google_web_search`, cite the source URL in the finding's
  `citations` array. Each citation is an object:
  ```json
  {
    "url": "https://doc.rust-lang.org/std/sync/atomic/",
    "description": "Rust atomic ordering reference"
  }
  ```

  Grounded findings are strictly more valuable than ungrounded ones —
  they can be independently verified by the reader. Prefer grounded
  analysis over confident assertion for external claims.

  ## Envelope output requirement

  Your response MUST end with a JSON envelope bracketed by sentinels.
  The format is:

      (free-form prose about what you investigated and why)

      <!-- BEGIN-ORI-DUAL-TPR-V1 -->
      ```json
      { ...complete envelope per findings-schema.json... }
      ```
      <!-- END-ORI-DUAL-TPR-V1 -->

  Critical envelope contract points:
  - The envelope MUST conform to `.claude/skills/dual-tpr/findings-schema.json`
  - The `status` field MUST be `"complete"` if you finished the review
    successfully; use `"failed_partial"` only if you were unable to
    complete the investigation for a stated reason
  - The `reviewer` field MUST be `"gemini"`
  - The `skill` field MUST be `"review-work"`
  - The `scope_actually_reviewed.expanded_beyond_packet` field is
    REQUIRED — set it to `true` if you investigated beyond the starting
    packet the wrapper gave you, with a one-sentence `expansion_reason`
  - Each finding's `basis` field MUST be one of `fresh_verification |
    direct_file_inspection | git_history | inference`
  - Each finding's `location` MUST match the canonical regex
    `^[a-zA-Z0-9_./-]+:[0-9]+$` (repo-relative path:line)
  - Each finding's `title` MUST be imperative voice, sentence case, no
    markdown, no trailing punctuation, ≤200 chars

  See `.claude/skills/dual-tpr/envelope-format.md` for the full contract
  including positive and negative examples.

  ## What you must NOT do

  - DO NOT modify any files (source, plan, bug-tracker, anything)
  - DO NOT attempt to edit plan sections directly
  - DO NOT emit multiple envelopes — only ONE at the end of your response
  - DO NOT skip the sentinels even if you think the JSON block is
    unambiguous without them
  - DO NOT use fresh_verification basis for findings you did not actually
    verify by running tests or scripts — use direct_file_inspection or
    inference instead
  ```

- [ ] Create directory `.gemini/skills/review-plan/`.

- [ ] Write `.gemini/skills/review-plan/SKILL.md` with a structurally parallel body adapted for plan-review semantics. Parity target: this file MUST mirror `.gemini/skills/review-work/SKILL.md` section-for-section (Step 0, Methodology, Grounding directive, Envelope output requirement with Critical envelope contract points, What you must NOT do). Only the plan-review-specific content differs; structural skeleton is identical.

  ```markdown
  ---
  name: review-plan
  description: Review an entire plan as one cohesive implementation strategy and emit a JSON findings envelope with proposed plan edits. Use this when the user asks for a third-party review of a plan directory, plan file, or section as part of its owning plan. Does NOT modify any files — emits envelope only.
  ---

  # Review Plan (gemini side)

  This skill implements the review-plan workflow for Gemini as part of
  the dual-source TPR system. It always runs in envelope-only mode.
  Unlike the codex-side review-plan which edits plan files directly
  in its plan-write mode, this gemini skill ONLY emits envelopes —
  each finding describes a PROPOSED plan edit rather than applying it.

  ## Step 0: Execution Mode (MANDATORY — read first)

  This skill has ONE execution mode: **envelope-only**. Unlike the
  codex-side review-plan skill (which has a `plan-write` vs `envelope-only`
  Step 0 dispatch), this gemini skill has no plan-write branch because
  there is no standalone "edit plan files directly" use case for the
  gemini side — the gemini skill exists solely for the dual-source wrapper.

  This is a REAL execution branch, not a soft override:
  1. You MUST emit a JSON envelope conforming to the schema at the end
     of your response
  2. You MUST NOT edit plan files directly (plan-write is the codex side's
     job, not this skill's)
  3. You MUST NOT flip any section's `reviewed: true` frontmatter value
     during whole-plan review (preserved from the existing review-plan
     semantics)
  4. Each "finding" describes a PROPOSED plan edit — the file path, line
     number, and the nature of the change — rather than applying it
  5. Every code path that would modify a file is suppressed by this Step 0

  If a later instruction in this file appears to contradict Step 0,
  Step 0 wins. Envelope-only is non-negotiable.

  ## Methodology

  Follow the shared reviewer-agnostic methodology documented in
  `.claude/skills/dual-tpr/command-file.md`, plus the plan-review
  extensions below.

  ## Plan-review specific extensions

  Beyond the shared command file methodology, plan review requires:
  - Reading the entire plan directory (index.md, 00-overview.md, all
    section-*.md files) — not just the file the user named
  - Checking plan-wide accuracy (status metadata vs actual checkbox state)
  - Checking cross-section dependencies for accuracy
  - Checking that mission success criteria trace to sections that
    deliver them
  - Checking for contradictions, gaps, redundancy, broken references,
    ordering issues, sync-point completeness, and overview alignment
  - Preserving every existing `reviewed` frontmatter value in the
    findings (do not propose flipping `reviewed` during whole-plan review)

  ## Grounding directive (gemini-specific)

  Same as the review-work skill: use `google_web_search` proactively
  for any finding that makes a claim about external libraries, specs,
  prior art, recent developments, security, or performance. Cite
  sources in the finding's `citations` array.

  For plan review specifically, grounding is particularly valuable for:
  - Verifying that cited reference implementations actually exist at
    the claimed paths in upstream repos
  - Checking that language spec claims match the current spec version
  - Verifying that test strategies cited as "standard" are actually
    standard in the relevant ecosystem

  ## Envelope output requirement

  Your response MUST end with a JSON envelope bracketed by sentinels.
  The format is:

      (free-form prose about what you investigated and why)

      <!-- BEGIN-ORI-DUAL-TPR-V1 -->
      ```json
      { ...complete envelope per findings-schema.json... }
      ```
      <!-- END-ORI-DUAL-TPR-V1 -->

  Critical envelope contract points (same shape as the review-work gemini skill):
  - The envelope MUST conform to `.claude/skills/dual-tpr/findings-schema.json`
  - The `status` field MUST be `"complete"` if you finished the review
    successfully; use `"failed_partial"` only if you were unable to
    complete the investigation for a stated reason
  - The `reviewer` field MUST be `"gemini"`
  - The `skill` field MUST be `"review-plan"`
  - The `scope_actually_reviewed.expanded_beyond_packet` field is
    REQUIRED — set it to `true` if you investigated beyond the starting
    packet the wrapper gave you, with a one-sentence `expansion_reason`
  - Each finding's `basis` field MUST be one of `fresh_verification |
    direct_file_inspection | git_history | inference`
  - Each finding's `location` MUST match the canonical regex
    `^[a-zA-Z0-9_./-]+:[0-9]+$` — but for review-plan the path is a
    plan file (e.g., `plans/dual-tpr-gemini/section-02-transport.md:45`)
    not a source file
  - Each finding's `title` MUST be imperative voice, sentence case, no
    markdown, no trailing punctuation, ≤200 chars

  **Finding semantics for plan-review (additional to the shared contract):**
  - Each finding in envelope-only mode describes a PROPOSED edit to
    the plan
  - The `title` describes the proposed edit in imperative form
    (e.g., "Add worktree guard description to Section 02 success criteria")
  - The `evidence` cites the current plan content that is inaccurate
    or missing
  - The `impact` explains why the plan is incomplete or wrong without
    the edit
  - The `required_plan_update` contains the proposed replacement text
    or addition

  **Note on apply-ability:** the `required_plan_update` field is
  free-text prose describing the proposed change, not a structured
  patch. Claude (the single writer) interprets and applies each edit
  after user approval — see Section 06 for the approval and application
  workflow. If finer-grained apply semantics are needed, Section 06's
  wrapper can extend the schema with a structured `patch` field in
  a future revision; the current envelope treats edit application as
  Claude-mediated, not reviewer-deterministic.

  See `.claude/skills/dual-tpr/envelope-format.md` for the full contract.

  ## What you must NOT do

  - DO NOT edit plan files directly (that's the codex-side plan-write
    mode, not this skill)
  - DO NOT change the `reviewed` frontmatter values of any section
    during whole-plan review
  - DO NOT emit multiple envelopes
  - DO NOT skip sentinels
  ```

- [ ] Write `.claude/skills/dual-tpr/transport.md` (new high-level doc) that documents the wrapper invocation pattern including the explicit skill activation phrase convention:

  ```markdown
  # Dual-TPR Transport — Wrapper Invocation Pattern

  This document specifies the wrapper invocation pattern that all four
  dual-source review skill wrappers (Sections 04-07) use to launch
  both reviewers and parse their output via the shared transport
  utility.

  ## Wrapper invocation structure

  Every dual-source review wrapper follows this pattern:

  1. Build the prompt from the user's request + starting packet (scope
     hint, plan section name, recent git activity). The packet is
     INFORMATIONAL, not authoritative — reviewers expand as they see fit.

  2. Write the prompts to per-run scratch files:
     - `$RUN/codex.prompt.md` — codex-side prompt
     - `$RUN/gemini.prompt.md` — gemini-side prompt

     The codex and gemini prompts share the same evidence packet but
     differ in their activation preamble (see below).

  3. Invoke the transport launcher with retry:
     ```bash
     .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh \
         --run "$RUN" \
         --skill {skill-name} \
         --codex-prompt "$RUN/codex.prompt.md" \
         --gemini-prompt "$RUN/gemini.prompt.md" \
         --schema .claude/skills/dual-tpr/findings-schema.json
     ```

  4. On success, parse both envelopes (already cached by the transport):
     - `$RUN/codex.envelope.json`
     - `$RUN/gemini.envelope.json`

  5. Merge findings with reviewer tagging:
     ```bash
     .claude/skills/dual-tpr/scripts/merge-findings.py \
         --codex "$RUN/codex.envelope.json" \
         --gemini "$RUN/gemini.envelope.json" \
         --section {section-number} \
         --out "$RUN/merged.json"
     ```

  6. Write merged findings to the target location (plan section TPR
     block, bug-tracker, or direct presentation to user — depending on
     the wrapper's loop semantics).

  ## Codex prompt preamble

  The codex prompt MUST include the literal keyword `envelope-only`
  somewhere in its first 500 characters. This triggers the Step 0 mode
  branch in `.codex/skills/review-work/SKILL.md` or
  `.codex/skills/review-plan/SKILL.md` and dispatches to envelope-only
  mode.

  Recommended preamble (first line of the prompt):

      Run the /review-work skill in envelope-only mode. Emit the JSON
      envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
      write findings to plan files.

  (Substitute `review-plan` for `review-work` as appropriate.)

  ## Gemini prompt preamble — EXPLICIT ACTIVATION REQUIRED

  Per Phase 2 empirical research, gemini skills are discovered from
  `.gemini/skills/<name>/SKILL.md` but are NOT auto-activated by
  description matching. The prompt MUST start with an explicit
  activation phrase to ensure gemini loads and follows the skill.

  MANDATORY first line of every gemini prompt:

      Activate the review-work skill and follow its instructions exactly.

  (Substitute `review-plan` for `review-work` as appropriate.)

  Do NOT rely on gemini noticing the skill on its own — the activation
  phrase is load-bearing and MUST be present on every invocation.

  ## Scripts consumed by wrappers

  All wrappers consume the same set of transport scripts from Section 02:
  - `.claude/skills/dual-tpr/scripts/scratch-dir.sh` — per-run scratch dir
  - `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` — launcher + retry
  - `.claude/skills/dual-tpr/scripts/parse-codex.py` — codex parser
  - `.claude/skills/dual-tpr/scripts/parse-gemini.py` — gemini parser
  - `.claude/skills/dual-tpr/scripts/validate-envelope.py` — standalone validator
  - `.claude/skills/dual-tpr/scripts/worktree-guard.sh` — git worktree safety
  - `.claude/skills/dual-tpr/scripts/merge-findings.py` — reviewer-tagged merger

  See Section 02 (`section-02-transport.md`) for the full scripts contract.

  ## Failure handling

  The transport layer (Section 02) handles infra retries internally —
  3 retries per reviewer per round with exponential backoff (1s, 2s, 4s).
  After 3 retries, `dual-invoke-with-retry.sh` exits non-zero and prints
  the failure category and postmortem directory path.

  Wrappers should:
  - On success: proceed to parse + merge + write
  - On failure: surface the failure category and postmortem path to the
    user via AskUserQuestion, including the `$RUN` directory where the
    JSONL streams and error messages are retained for inspection
  - NEVER consume a semantic iteration of the wrapper's outer loop on
    infra failure — the 10-iteration loop is for finding-fixing rounds,
    not transport failures

  ## Wrapper loop semantics

  `/tpr-review` and `/review-work` use the 10-iteration find+fix+rerun
  loop. Each iteration:
  1. Runs the dual-source transport (both reviewers per round, max
     3 infra retries per reviewer)
  2. Claude reads the merged findings
  3. If zero actionable findings: clean pass, exit loop
  4. Otherwise: Claude fixes findings, commits, re-runs (increment
     semantic iteration counter)
  5. After 10 iterations: surface remaining findings to user via
     AskUserQuestion

  `/review-plan` does NOT loop — it emits proposed edits once per
  invocation. The wrapper applies them (or presents them for user
  approval) and does not re-invoke.

  `/tp-help` does NOT loop and does NOT use the findings schema — it
  emits raw concatenated responses from both reviewers (see Section 07
  for the tp-help-specific envelope).
  ```

- [ ] Verify gemini skill discovery from the project root:
  ```bash
  cd /home/eric/projects/ori_lang
  gemini skills list 2>&1 | grep -E "review-(work|plan)"
  # Expected: both review-work and review-plan appear in the skill list
  ```

- [ ] Verify the grounding directive is present in both gemini skills:
  ```bash
  grep -l 'google_web_search' .gemini/skills/review-work/SKILL.md .gemini/skills/review-plan/SKILL.md
  # Expected: both file paths printed
  ```

- [ ] **Structural parity check** between the two gemini skill files. Both MUST have identical top-level section headers in the same order so the failure/escalation semantics are identical across review-work and review-plan. Run:
  ```bash
  for f in .gemini/skills/review-work/SKILL.md .gemini/skills/review-plan/SKILL.md; do
    echo "=== $f ==="
    grep -E '^## ' "$f"
  done
  ```
  Expected: both files print the SAME sequence of `## ` headers — at minimum:
  - `## Step 0: Execution Mode (MANDATORY — read first)`
  - `## Methodology`
  - `## Grounding directive (gemini-specific)`
  - `## Envelope output requirement`
  - `## What you must NOT do`
  Any drift in headers (different names, different order, missing sections, extra sections) is a parity violation that MUST be fixed before the subsection closes. The two skills are structurally parallel by contract — any divergence creates inconsistent reviewer behavior across the two wrappers that invoke them.

- [ ] **Envelope contract parity check**: both gemini skill files MUST restate the same "Critical envelope contract points" bulleted list (the one that requires schema conformance, `status: "complete"`, `reviewer: "gemini"`, `expanded_beyond_packet`, canonical `location`, and canonical `title`). Verify with:
  ```bash
  for f in .gemini/skills/review-work/SKILL.md .gemini/skills/review-plan/SKILL.md; do
    grep -q 'Critical envelope contract points' "$f" && echo "OK: $f" || echo "MISSING: $f"
  done
  ```
  Expected: both files print `OK`.

- [ ] Verify the transport.md doc mentions the explicit activation convention:
  ```bash
  grep -q "Activate the review-work skill" .claude/skills/dual-tpr/transport.md
  grep -q "Activate the review-plan skill" .claude/skills/dual-tpr/transport.md
  # Expected: both grep exit 0
  ```

- [ ] **Subsection close-out (03.3)** — MANDATORY before section completion:
  - [ ] Both gemini skill files exist, are discoverable by `gemini skills list`, and contain the grounding directive
  - [ ] The transport.md doc documents the explicit activation phrase convention
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — was writing two structurally-parallel gemini skill files tedious (lots of repetition between review-work and review-plan versions)? Should there be a `scaffold-gemini-skill.sh` helper that takes a skill name and generates the boilerplate? Was verifying `gemini skills list` awkward (required `cd` + `grep`)? Should there be a `verify-gemini-skills.sh` that runs the discovery check as part of the test suite? Implement improvements NOW.

---

## 03.R Third Party Review Findings

<!-- Reserved for codex/gemini reviewers running /tpr-review against this section.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`
-->

- None.

---

## 03.N Completion Checklist

- [ ] All three implementation subsections (03.1, 03.2, 03.3) marked `complete` in section frontmatter
- [ ] `.claude/skills/dual-tpr/command-file.md` exists, contains the reviewer-agnostic methodology, and passes the no-reviewer-specific-content grep test
- [ ] `.codex/skills/review-work/SKILL.md` has the Step 0 mode branch inserted, existing workflow content unchanged (verified by grep for `## Scope Inputs`, `## Review Workflow`, `## Plan Update Rules`, `## Finding Format` still present)
- [ ] `.codex/skills/review-plan/SKILL.md` has the same Step 0 mode branch, existing workflow content unchanged
- [ ] Both codex skill files have net-additive diffs (line counts increased by the size of the Step 0 block, nothing removed)
- [ ] `.gemini/skills/review-work/SKILL.md` exists with YAML frontmatter (`name`, `description`), body references the shared command file, contains the grounding directive, and is discoverable by `gemini skills list` from the project root
- [ ] `.gemini/skills/review-plan/SKILL.md` exists with the same structure, adapted for plan-review semantics
- [ ] `.claude/skills/dual-tpr/transport.md` exists and documents the wrapper invocation pattern, the codex `envelope-only` keyword requirement, and the gemini explicit activation phrase requirement
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan dual-tpr-gemini` returns 0 annotations from this section's work in source files
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 03
  - [ ] `index.md` section status updated for Section 03
  - [ ] Section 04's `depends_on: ["03"]` precondition is satisfied
- [x] `/tpr-review` passed (final, full-section) — independent codex review clean or findings triaged. Note: this is still the single-source `/tpr-review` — the dual-source rewrite is Section 04, which has not yet landed.
  Resolved: **Deferred** at section start on 2026-04-07 by the user's standing direction "we aren't running the gates" (mirrors the section-01 closure in commit 982fcef5 and the section-02 closure in commit 55a99905). Section 03's work product is purely reviewer-surface preparation under `.claude/skills/dual-tpr/`, `.codex/skills/`, and `.gemini/skills/` with zero touch on compiler crates. The local subsection-level grep, structural-parity, and envelope-contract-parity verification specified by the 03.N checklist provides dense coverage of the surfaces this section produces. Note also the meta-circularity: running the existing single-source `/tpr-review` against the very skill files that build the dual-source `/tpr-review` (Section 04) would generate findings about scaffolding the next section is designed to remove. The TPR can still be run as a follow-up before Section 04 begins; the deferral is "not now" rather than "never". When run, this checkbox should be flipped to `[x] Resolved: Ran on YYYY-MM-DD with N findings, all triaged.`
- [x] `/impl-hygiene-review` passed — implementation hygiene review clean. MUST run AFTER `/tpr-review` is clean.
  Resolved: **Skipped** at section start on 2026-04-07 by the user's standing direction (mirrors the section-01 and section-02 closures). Same rationale: Section 03's work product is entirely in `.claude/skills/dual-tpr/`, `.codex/skills/`, `.gemini/skills/`, and `plans/dual-tpr-gemini/` with zero touch on compiler crates (`ori_types`, `ori_eval`, `ori_llvm`, `ori_arc`, `ori_parse`, `ori_lexer`, `ori_rt`, `ori_registry`, `library/std`). The hygiene review's primary value is catching SSOT violations, scattered knowledge, phase boundary leaks, and algorithmic DRY issues in compiler code — its scope does not naturally extend to harness/skill content where those failure modes don't apply. Should an issue surface later that would have been caught by an impl-hygiene pass on this section's work, the fix can reference this skip and the gate can be opened then.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY. Verify each subsection has either improvements made or a documented "no gaps" negative finding. Look for cross-subsection patterns: did the same kind of "edit existing skill file" friction recur across 03.1-03.2? Did the grep-based verification pattern recur across all three subsections? Could a single `verify-skill-edits.sh` replace multiple ad-hoc greps? Add only new items that emerged from cross-cutting patterns. Implement immediately, commit separately. Most sweeps produce zero new findings when per-subsection captures are thorough — that is the expected, healthy outcome and must be documented if so. Per section-02 precedent: half 1 (per-subsection retrospective audit) PASSES the audit independently and runs at section close; half 2 (cross-subsection pattern hunt) is workflow-dependent on the deferred reviews and is documented as deferred-with-the-reviews when those resolution paragraphs are written.

**Exit Criteria:** The shared command file is extracted and contains only reviewer-agnostic methodology. Both codex skills (`review-work` and `review-plan`) have the `plan-write`/`envelope-only` mode branch inserted as Step 0 of their workflows, with existing content preserved intact (standalone `codex exec` regression-clean). Both gemini skills exist under `.gemini/skills/` with YAML frontmatter, reference to the shared command file, and the explicit `google_web_search` grounding directive. Both gemini skills are discoverable by `gemini skills list` from the project root. The transport documentation (`.claude/skills/dual-tpr/transport.md`) specifies the mandatory `envelope-only` keyword for codex prompts and the mandatory "Activate the {skill} skill and follow its instructions exactly" preamble for gemini prompts. Section 04 can begin its `/tpr-review` rewrite against the locked reviewer-surface contracts.
