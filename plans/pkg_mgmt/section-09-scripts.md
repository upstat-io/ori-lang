---
section: "09"
title: "Scripts"
status: not-started
goal: "Project-defined task runner"
depends_on: ["08"]
sections:
  - id: "9.1"
    title: "Script Parsing"
    status: not-started
  - id: "9.2"
    title: "Script Execution"
    status: not-started
  - id: "9.3"
    title: "Single-File Mode"
    status: not-started
  - id: "9.4"
    title: "Script Stripping"
    status: not-started
  - id: "9.5"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 9: Scripts

**Goal**: Project-defined task runner

**Status**: ⬜ Not Started

---

## 9.1 Script Parsing

- [ ] **Implement**: Parse `[scripts]` section
  - [ ] Simple string commands
  - [ ] **Rust Tests**: `ori_pkg/src/scripts/parse.rs`

- [ ] **Subsection close-out (9.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-9.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 9.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 9.2 Script Execution

- [ ] **Implement**: `ori run`
  - [ ] List available scripts
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: `ori run <script>`
  - [ ] Execute named script
  - [ ] Run in project root
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: `ori run <script> -- <args>`
  - [ ] Pass additional arguments
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Subsection close-out (9.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-9.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 9.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 9.3 Single-File Mode

- [ ] **Implement**: `ori run file.ori`
  - [ ] Detect .ori extension
  - [ ] Run without project
  - [ ] Stdlib only
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: Shebang support
  - [ ] `#!/usr/bin/env ori`
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Subsection close-out (9.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-9.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 9.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 9.4 Script Stripping

- [ ] **Implement**: Strip scripts on publish
  - [ ] Remove `[scripts]` from published manifest
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

- [ ] **Subsection close-out (9.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-9.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 9.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 9.5 Phase Completion Checklist

- [ ] Script parsing
- [ ] Script execution
- [ ] Argument passing
- [ ] Single-file mode
- [ ] Shebang support
- [ ] Stripped on publish
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: `ori run` works like `npm run`

- [ ] **Subsection close-out (9.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-9.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 9.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

